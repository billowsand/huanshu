use crate::config::GenerationConfig;
use crate::db::{self, GenerationLogEntry, GenerationRun};
use crate::icon::IconIndex;
use crate::lmstudio::LmStudioClient;
use crate::types::SlideBlueprint;
use crate::AppState;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

/// Progress event emitted during generation
#[derive(Debug, Clone, serde::Serialize)]
pub struct GenerationEvent {
    pub stage: String,
    pub message: String,
    pub progress: Option<f32>,
    pub slide_index: Option<usize>,
    pub blueprint: Option<SlideBlueprint>,
}

/// Per-page status event for the concurrent pipeline
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PageStatusEvent {
    pub slide_index: usize,
    pub stage: String,
    pub message: Option<String>,
    pub blueprint: Option<SlideBlueprint>,
    pub waiting: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GenerationLogEvent {
    pub id: i64,
    pub run_id: i64,
    pub seq: i64,
    pub slide_index: Option<i32>,
    pub stage: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub detail: Value,
    pub important: bool,
    pub created_at: i64,
}

impl From<GenerationLogEntry> for GenerationLogEvent {
    fn from(value: GenerationLogEntry) -> Self {
        Self {
            id: value.id,
            run_id: value.run_id,
            seq: value.seq,
            slide_index: value.slide_index,
            stage: value.stage,
            kind: value.kind,
            title: value.title,
            summary: value.summary,
            detail: value.detail,
            important: value.important,
            created_at: value.created_at,
        }
    }
}

fn strip_markdown_fences(raw: &str) -> String {
    let trimmed = raw.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }
    let mut lines = trimmed.lines();
    let _ = lines.next();
    let mut body: Vec<&str> = lines.collect();
    if matches!(body.last(), Some(line) if line.trim_start().starts_with("```")) {
        body.pop();
    }
    body.join("\n").trim().to_string()
}

fn reset_debug_dir(path: &Path) -> anyhow::Result<()> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err.into()),
    }
    std::fs::create_dir_all(path)?;
    Ok(())
}

fn record_log(
    app: &AppHandle,
    db: &std::sync::Arc<std::sync::Mutex<Connection>>,
    run_id: i64,
    slide_index: Option<i32>,
    stage: &str,
    kind: &str,
    title: &str,
    summary: &str,
    detail: Value,
    important: bool,
) {
    let entry = {
        let conn = db.lock().unwrap();
        db::append_generation_log(
            &*conn,
            run_id,
            slide_index,
            stage,
            kind,
            title,
            summary,
            &detail,
            important,
        )
    };
    if let Ok(entry) = entry {
        let _ = app.emit("gen:log", GenerationLogEvent::from(entry));
    }
}

fn update_run_stage(
    db: &std::sync::Arc<std::sync::Mutex<Connection>>,
    run_id: i64,
    status: &str,
    current_stage: Option<&str>,
) {
    let conn = db.lock().unwrap();
    let _ = db::update_generation_run_stage(&*conn, run_id, status, current_stage);
}

fn finish_run(
    db: &std::sync::Arc<std::sync::Mutex<Connection>>,
    run_id: i64,
    status: &str,
    current_stage: Option<&str>,
) {
    let conn = db.lock().unwrap();
    let _ = db::finish_generation_run(&*conn, run_id, status, current_stage);
}

fn emit_progress(
    app: &AppHandle,
    db: &std::sync::Arc<std::sync::Mutex<Connection>>,
    run_id: i64,
    stage: &str,
    msg: &str,
    progress: f32,
) {
    update_run_stage(db, run_id, "running", Some(stage));
    let _ = app.emit(
        "gen:progress",
        GenerationEvent {
            stage: stage.to_string(),
            message: msg.to_string(),
            progress: Some(progress),
            slide_index: None,
            blueprint: None,
        },
    );
    record_log(
        app,
        db,
        run_id,
        None,
        stage,
        "status",
        stage,
        msg,
        json!({ "progress": progress }),
        false,
    );
}

/// Return the recommended granularity level for the given markdown.
#[tauri::command]
pub fn detect_granularity(md_content: String) -> String {
    match crate::input::detect_granularity(&md_content) {
        crate::input::HeadingLevel::H2 => "h2".to_string(),
        crate::input::HeadingLevel::H3 => "h3".to_string(),
    }
}

#[tauri::command]
pub async fn optimize_markdown_headings(
    state: State<'_, Mutex<AppState>>,
    raw_content: String,
    title_hint: Option<String>,
) -> Result<String, String> {
    let raw = raw_content.trim();
    if raw.is_empty() {
        return Err("文稿内容为空，无法优化".to_string());
    }

    let settings = {
        let st = state.lock().unwrap();
        st.settings.clone()
    };

    let llm_client = LmStudioClient::from_service(&settings.llm).map_err(|e| e.to_string())?;
    let embedding_client =
        LmStudioClient::from_service(&settings.embedding).map_err(|e| e.to_string())?;
    crate::generator::planning::ensure_models_ready(
        &llm_client,
        &settings.llm.model,
        &embedding_client,
        &settings.embedding.model,
    )
    .await
    .map_err(|e| e.to_string())?;

    let title_hint = title_hint
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let system = "你是一个 Markdown 结构整理助手。你的任务是把杂乱的中文文稿重写为可直接用于自动生成演示文稿的 Markdown。只返回 Markdown，不要解释，不要加代码块围栏，不要输出额外说明。";
    let user = format!(
        "任务：\n将下面文稿整理成结构清晰的 Markdown。\n\n规则：\n- 输出必须是合法 Markdown。\n- 必须只有一个 H1 主标题，且放在第一行。\n- 必须包含多个 H2 章节；必要时可在 H2 下补充 H3 子标题。\n- 保留原文事实、结论、顺序和关键信息，不要无根据扩写。\n- 删除明显重复、噪声、口语化占位和无意义格式。\n- 如果原文没有明确标题结构，请根据内容自行归纳合适的多级标题。\n- 每个标题下保留对应正文，正文尽量精炼，但不能只剩标题。\n- 不要添加 YAML frontmatter。\n- 输出语言保持为中文。\n\n标题提示：{}\n\n原始文稿：\n{}",
        title_hint.clone().unwrap_or_else(|| "无".to_string()),
        raw
    );

    let response = llm_client
        .generate_text(&settings.llm.model, system, &user)
        .await
        .map_err(|e| e.to_string())?;
    let cleaned = strip_markdown_fences(&response);

    let final_markdown = if cleaned
        .lines()
        .any(|line| line.trim_start().starts_with("# "))
    {
        cleaned
    } else if let Some(title) = title_hint {
        format!("# {}\n\n{}", title, cleaned.trim())
    } else {
        format!("# 未命名演示\n\n{}", cleaned.trim())
    };

    let h2_count = final_markdown
        .lines()
        .filter(|line| line.trim_start().starts_with("## "))
        .count();
    if h2_count == 0 {
        return Err("AI 优化结果缺少二级标题，无法用于生成".to_string());
    }

    Ok(final_markdown)
}

/// Run the full generation pipeline.
/// Emits `gen:progress` and `gen:slide_ready` events during execution.
#[tauri::command]
pub async fn generate_slides(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    md_content: String,
    frontmatter_title: Option<String>,
    granularity: Option<String>,
    aspect_ratio: Option<crate::types::AspectRatio>,
) -> Result<usize, String> {
    // Default to 16:9 if not specified
    let resolved_aspect_ratio = aspect_ratio.unwrap_or(crate::types::AspectRatio::Ratio16x9);

    // Get config from state
    let (settings, project_dir, active_project_id, db) = {
        let st = state.lock().unwrap();
        (
            st.settings.clone(),
            st.project_dir.clone(),
            st.active_project_id,
            std::sync::Arc::clone(&st.db),
        )
    };

    // Mark generation as running, clear old blueprints
    {
        let mut st = state.lock().unwrap();
        if st.generation_running {
            return Err("请等待当前生成或重试完成".into());
        }
        st.generation_running = true;
        st.blueprints.clear();
        st.last_error = None;
    }

    let resolved_granularity = match granularity.as_deref() {
        Some("h2") => crate::input::HeadingLevel::H2,
        Some("h3") => crate::input::HeadingLevel::H3,
        // None = auto-detect from the markdown content
        _ => crate::input::detect_granularity(&md_content),
    };

    let mut config = GenerationConfig::from_settings(&settings, project_dir, resolved_aspect_ratio);
    config.frontmatter_title = frontmatter_title;
    config.granularity = resolved_granularity;
    let run_id = {
        let conn = db.lock().unwrap();
        db::create_generation_run(
            &*conn,
            active_project_id,
            &md_content,
            config.frontmatter_title.as_deref(),
            &config.debug_dir.display().to_string(),
        )
        .map_err(|e| e.to_string())?
    };

    emit_progress(&app, &db, run_id, "start", "开始生成流程", 0.0);
    record_log(
        &app,
        &db,
        run_id,
        None,
        "start",
        "input",
        "本次生成输入",
        "记录原始文稿、标题和模型配置",
        json!({
            "source_markdown": md_content,
            "frontmatter_title": config.frontmatter_title,
            "llm_model": config.llm.model,
            "embedding_model": config.embedding.model,
            "multimodal_model": config.multimodal.model,
            "debug_dir": config.debug_dir.display().to_string(),
            "aspect_ratio": config.aspect_ratio,
        }),
        true,
    );

    // Run the generator pipeline
    let result = run_pipeline(
        app.clone(),
        &config,
        &md_content,
        db.clone(),
        run_id,
        active_project_id,
    )
    .await;

    match result {
        Ok(blueprints) => {
            let count = blueprints.len();
            {
                let mut st = state.lock().unwrap();
                // Auto-save blueprints to the active project if one is set
                if let Some(pid) = st.active_project_id {
                    if let Ok(json) = serde_json::to_string(&blueprints) {
                        let db = st.db.lock().unwrap();
                        crate::db::update_project_blueprints(&*db, pid, &json).ok();
                    }
                }
                st.blueprints = blueprints;
                st.generation_running = false;
            }
            finish_run(&db, run_id, "done", Some("done"));
            let _ = app.emit(
                "gen:progress",
                GenerationEvent {
                    stage: "done".to_string(),
                    message: format!("生成完成，共 {count} 张幻灯片"),
                    progress: Some(1.0),
                    slide_index: None,
                    blueprint: None,
                },
            );
            record_log(
                &app,
                &db,
                run_id,
                None,
                "done",
                "result",
                "生成完成",
                &format!("生成完成，共 {count} 张幻灯片"),
                json!({ "slide_count": count }),
                true,
            );
            Ok(count)
        }
        Err(e) => {
            let msg = e.to_string();
            {
                let mut st = state.lock().unwrap();
                st.generation_running = false;
                st.last_error = Some(msg.clone());
            }
            finish_run(&db, run_id, "error", Some("error"));
            let _ = app.emit(
                "gen:error",
                GenerationEvent {
                    stage: "error".to_string(),
                    message: msg.clone(),
                    progress: None,
                    slide_index: None,
                    blueprint: None,
                },
            );
            record_log(
                &app,
                &db,
                run_id,
                None,
                "error",
                "error",
                "生成失败",
                &msg,
                json!({ "error": msg }),
                true,
            );
            Err(msg)
        }
    }
}

/// Get the current set of blueprints (called by viewer to render slides)
#[tauri::command]
pub fn get_blueprints(state: State<'_, Mutex<AppState>>) -> Result<Vec<SlideBlueprint>, String> {
    let st = state.lock().unwrap();
    Ok(st.blueprints.clone())
}

/// Get generation status
#[tauri::command]
pub fn get_generation_status(
    state: State<'_, Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    let st = state.lock().unwrap();
    Ok(serde_json::json!({
        "running": st.generation_running,
        "slide_count": st.blueprints.len(),
        "last_error": st.last_error,
    }))
}

#[tauri::command]
pub fn get_latest_generation_run(
    state: State<'_, Mutex<AppState>>,
    project_id: i64,
) -> Result<Option<GenerationRun>, String> {
    let st = state.lock().unwrap();
    let db = st.db.lock().unwrap();
    db::get_latest_generation_run(&*db, project_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_generation_logs(
    state: State<'_, Mutex<AppState>>,
    run_id: i64,
) -> Result<Vec<GenerationLogEvent>, String> {
    let st = state.lock().unwrap();
    let db = st.db.lock().unwrap();
    db::get_generation_logs(&*db, run_id)
        .map(|items| items.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

/// Repair a single slide given user feedback
#[tauri::command]
pub async fn repair_slide(
    state: State<'_, Mutex<AppState>>,
    index: usize,
    feedback: String,
) -> Result<SlideBlueprint, String> {
    let (settings, project_dir, blueprints, active_project_id, db) = {
        let st = state.lock().unwrap();
        (
            st.settings.clone(),
            st.project_dir.clone(),
            st.blueprints.clone(),
            st.active_project_id,
            std::sync::Arc::clone(&st.db),
        )
    };

    // Use the project's selected aspect ratio (from DB), falling back to
    // the blueprint's own ratio, then 16:9.  This ensures a repaired slide
    // matches the project's current setting even if the blueprint is stale.
    let aspect_ratio = active_project_id
        .and_then(|pid| {
            let db_lock = db.lock().unwrap();
            crate::db::get_project(&*db_lock, pid)
                .ok()
                .and_then(|p| p.aspect_ratio)
        })
        .or_else(|| blueprints.get(index).and_then(|bp| bp.aspect_ratio))
        .unwrap_or(crate::types::AspectRatio::Ratio16x9);
    let config = GenerationConfig::from_settings(&settings, project_dir.clone(), aspect_ratio);
    let blueprint = blueprints
        .get(index)
        .ok_or_else(|| format!("slide index {index} out of range"))?
        .clone();

    // Load icon index and asset paths so the repair prompt can provide
    // valid icon candidates and prevent hallucinated image paths.
    let icon_index = {
        let conn = state.lock().unwrap();
        let db = conn.db.lock().unwrap();
        IconIndex::load_with_cache(&project_dir, &settings.embedding.model, &db).unwrap_or_else(
            |_| IconIndex::load(&project_dir).unwrap_or_else(|_| IconIndex::empty()),
        )
    };
    let assets = load_asset_catalog(&db, active_project_id, &project_dir);

    let mut repaired = crate::generator::slides::repair_single_slide(
        &config,
        &blueprint,
        &feedback,
        Some(&icon_index),
        Some(&assets),
    )
    .await
    .map_err(|e| e.to_string())?;
    // Feedback repair rewrites content, not the slide's place in the deck: keep its chapter number.
    if blueprint.section.is_some() {
        repaired.section = blueprint.section.clone();
    }

    {
        let mut st = state.lock().unwrap();
        if let Some(slot) = st.blueprints.get_mut(index) {
            *slot = repaired.clone();
        }
    }

    Ok(repaired)
}

// ---------------------------------------------------------------------------
// Internal pipeline
// ---------------------------------------------------------------------------

/// Retry a content page using its original source and assigned layout.
/// The assembled deck has two leading slides (cover and overview).
#[tauri::command]
pub async fn retry_generation_page(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    run_id: i64,
    page_index: usize,
    layout_kind: Option<crate::types::SlideKind>,
) -> Result<SlideBlueprint, String> {
    let (settings, project_dir, project_id, db, run, page, layout, ratio) = {
        let mut st = state.lock().unwrap();
        if st.generation_running {
            return Err("请等待当前生成或重试完成".into());
        }
        let db = std::sync::Arc::clone(&st.db);
        let (run, logs) = {
            let conn = db.lock().unwrap();
            let latest_id: Option<i64> = conn
                .query_row(
                    "SELECT MAX(id) FROM generation_runs WHERE project_id IS ?1",
                    rusqlite::params![st.active_project_id],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            if latest_id != Some(run_id) {
                return Err("生成记录已过期，请重试当前生成记录中的页面".into());
            }
            (
                db::get_generation_run(&conn, run_id).map_err(|e| e.to_string())?,
                db::get_generation_logs(&conn, run_id).map_err(|e| e.to_string())?,
            )
        };
        if run.project_id != st.active_project_id {
            return Err("生成记录不属于当前项目".into());
        }
        let assignment = logs
            .iter()
            .rev()
            .find(|log| log.detail.get("layout_plans").is_some())
            .ok_or("缺少页面规划，请重新生成整副幻灯片")?;
        let pages: Vec<crate::types::PagePlan> =
            serde_json::from_value(assignment.detail["page_plans"].clone())
                .map_err(|e| e.to_string())?;
        let layouts: Vec<crate::types::LayoutPlan> =
            serde_json::from_value(assignment.detail["layout_plans"].clone())
                .map_err(|e| e.to_string())?;
        let page = pages.get(page_index).cloned().ok_or("页面索引无效")?;
        let mut layout = layouts.get(page_index).cloned().ok_or("缺少该页布局")?;
        if st.blueprints.len() != pages.len() + 3 {
            return Err("整副幻灯片尚未组装完成，请使用整副重试".into());
        }
        let ratio = st.blueprints[page_index + 2]
            .aspect_ratio
            .unwrap_or(crate::types::AspectRatio::Ratio16x9);
        if let Some(kind) = layout_kind {
            if let Some(reason) = crate::generator::layout_rules::ineligibility(&kind, &page, ratio)
            {
                return Err(format!("所选布局不适用于这一页：{reason}"));
            }
            layout.kind = kind;
            layout.reason = "用户手动选择布局进行重试".into();
        }
        st.generation_running = true;
        (
            st.settings.clone(),
            st.project_dir.clone(),
            st.active_project_id,
            db,
            run,
            page,
            layout,
            ratio,
        )
    };
    let result: anyhow::Result<SlideBlueprint> = async {
        let mut config = GenerationConfig::from_settings(&settings, project_dir.clone(), ratio);
        config.frontmatter_title = run.frontmatter_title.clone();
        config.debug_dir = std::path::PathBuf::from(&run.debug_dir).join(format!(
            "retry-page-{}-{}",
            page_index + 1,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis()
        ));
        let llm = config.llm_client()?;
        let embedding = config.embedding_client()?;
        let mut doc = crate::input::parse_markdown(&run.source_markdown)?;
        if let Some(title) = &run.frontmatter_title {
            doc.title = title.clone();
        }
        let icons = {
            let conn = db.lock().unwrap();
            IconIndex::load_with_cache(&project_dir, &config.embedding.model, &conn)?
        };
        let assets = load_asset_catalog(&db, project_id, &project_dir);
        let slide = crate::generator::slides::generate_single_page_pipeline(
            &llm,
            &embedding,
            &config,
            &doc,
            page.clone(),
            layout.clone(),
            page_index,
            &icons,
            &assets,
            &config.debug_dir,
            std::sync::Arc::new(|_, _, _| {}),
        )
        .await?;
        if slide.kind != layout.kind {
            anyhow::bail!("所选布局生成未成功，请选择其他布局后重试");
        }
        let issues =
            crate::validate::validate_blueprints(std::slice::from_ref(&slide), &icons, assets.refs());
        if !issues.is_empty() {
            anyhow::bail!(
                "重试结果未通过校验：{}",
                issues
                    .iter()
                    .map(|i| i.message.as_str())
                    .collect::<Vec<_>>()
                    .join("；")
            );
        }
        let mut st = state.lock().unwrap();
        if st.active_project_id != project_id {
            anyhow::bail!("当前项目已切换，重试结果未写入");
        }
        let mut updated = st.blueprints.clone();
        *updated
            .get_mut(page_index + 2)
            .ok_or_else(|| anyhow::anyhow!("幻灯片列表已改变"))? = slide.clone();
        // A recovered run may only become editable after the assembled deck passes validation.
        let issues = crate::validate::validate_blueprints(&updated, &icons, assets.refs());
        if !issues.is_empty() {
            anyhow::bail!(
                "整副幻灯片未通过校验：{}",
                issues.iter().map(|i| i.message.as_str()).collect::<Vec<_>>().join("；")
            );
        }
        let mut conn = db.lock().unwrap();
        let tx = conn.transaction()?;
        if let Some(pid) = project_id {
            db::update_project_blueprints(
                &tx,
                pid,
                &serde_json::to_string(&updated)?,
            )?;
        }
        db::finish_generation_run(&tx, run_id, "done", Some("done"))?;
        tx.commit()?;
        st.blueprints = updated;
        st.last_error = None;
        Ok(slide)
    }
    .await;
    state.lock().unwrap().generation_running = false;
    let error = result.as_ref().err().map(|e| format!("{e:#}"));
    record_log(
        &app,
        &db,
        run_id,
        Some(page_index as i32),
        "content",
        "retry",
        "单页重试",
        error.as_deref().unwrap_or("该页重试完成"),
        json!({"page_plan": page, "layout_plan": layout, "error": error, "retry_succeeded": result.is_ok()}),
        true,
    );
    result.map_err(|e| format!("{e:#}"))
}

#[derive(serde::Serialize)]
pub struct RetryLayoutOptions {
    current_kind: crate::types::SlideKind,
    kinds: Vec<crate::types::SlideKind>,
}

/// Use the same layout eligibility table as automatic generation.
#[tauri::command]
pub fn get_retry_layouts(
    state: State<'_, Mutex<AppState>>,
    run_id: i64,
    page_index: usize,
) -> Result<RetryLayoutOptions, String> {
    let st = state.lock().unwrap();
    let conn = st.db.lock().unwrap();
    let run = db::get_generation_run(&conn, run_id).map_err(|e| e.to_string())?;
    if run.project_id != st.active_project_id {
        return Err("生成记录不属于当前项目".into());
    }
    let logs = db::get_generation_logs(&conn, run_id).map_err(|e| e.to_string())?;
    let assignment = logs
        .iter()
        .rev()
        .find(|log| log.detail.get("layout_plans").is_some())
        .ok_or("缺少页面布局规划")?;
    let pages: Vec<crate::types::PagePlan> =
        serde_json::from_value(assignment.detail["page_plans"].clone())
            .map_err(|e| e.to_string())?;
    let layouts: Vec<crate::types::LayoutPlan> =
        serde_json::from_value(assignment.detail["layout_plans"].clone())
            .map_err(|e| e.to_string())?;
    let page = pages.get(page_index).ok_or("页面索引无效")?;
    let layout = layouts.get(page_index).ok_or("缺少该页布局")?;
    let ratio = logs
        .iter()
        .find_map(|log| serde_json::from_value(log.detail["aspect_ratio"].clone()).ok())
        .or_else(|| {
            st.blueprints
                .get(page_index + 2)
                .and_then(|bp| bp.aspect_ratio)
        })
        .unwrap_or(crate::types::AspectRatio::Ratio16x9);
    Ok(RetryLayoutOptions {
        current_kind: layout.kind.clone(),
        kinds: crate::generator::planning::eligible_fallback_kinds(page, ratio),
    })
}

/// Per-page stage callback that forwards progress to the frontend as `gen:page_status`.
fn page_stage_callback(
    app: AppHandle,
    slide_index: usize,
) -> std::sync::Arc<dyn Fn(usize, crate::generator::slides::PageStage, Option<&str>) + Send + Sync>
{
    use crate::generator::slides::PageStage;
    std::sync::Arc::new(
        move |_slide_idx: usize, stage: PageStage, msg: Option<&str>| {
            let stage_str = match stage {
                PageStage::Planning => "planning",
                PageStage::Layout => "layout",
                PageStage::Content => "content",
                PageStage::Normalizing => "normalizing",
                PageStage::Validating => "validating",
                PageStage::Done => "done",
            };
            let _ = app.emit(
                "gen:page_status",
                PageStatusEvent {
                    slide_index,
                    stage: stage_str.to_string(),
                    message: msg.map(str::to_string),
                    blueprint: None,
                    waiting: false,
                },
            );
        },
    )
}

/// Media-library images for a project, or an empty list when there is no project or the
/// read fails.
fn load_project_media_assets(
    db: &std::sync::Arc<std::sync::Mutex<Connection>>,
    project_id: Option<i64>,
) -> Vec<crate::db::MediaAsset> {
    let Some(project_id) = project_id else {
        return Vec::new();
    };
    let conn = db.lock().unwrap();
    crate::db::get_media_assets_for_project(&conn, project_id).unwrap_or_default()
}

/// Everything the generator may use as an image: project files plus the media library.
fn load_asset_catalog(
    db: &std::sync::Arc<std::sync::Mutex<Connection>>,
    project_id: Option<i64>,
    project_dir: &std::path::Path,
) -> crate::assets::AssetCatalog {
    let media_assets = load_project_media_assets(db, project_id);
    crate::assets::AssetCatalog::collect(project_dir, &media_assets)
}

/// Media library root, read from the app settings.
fn app_media_dir(app: &AppHandle) -> std::path::PathBuf {
    let state = app.state::<Mutex<AppState>>();
    let media_dir = state.lock().unwrap().app_settings.media_dir.clone();
    std::path::PathBuf::from(media_dir)
}

/// Write the article-aware captions from the placement pass back to the media library.
///
/// This intentionally overwrites the caption generated at import time, which only ever saw
/// the picture itself.
fn apply_placement_captions(
    db: &std::sync::Arc<std::sync::Mutex<Connection>>,
    placements: &[crate::assets::MediaPlacement],
) {
    let Ok(conn) = db.lock() else {
        return;
    };
    for placement in placements {
        let Some(caption) = placement
            .caption
            .as_deref()
            .map(str::trim)
            .filter(|caption| !caption.is_empty())
        else {
            continue;
        };
        let _ = db::update_media_asset_caption_only(&conn, placement.media_id, caption);
    }
}

/// One log line describing what the placement pass decided.
fn summarize_placements(
    placements: &[crate::assets::MediaPlacement],
    pages: &[crate::types::PagePlan],
) -> String {
    if placements.is_empty() {
        return "没有可配对的素材图片".to_string();
    }
    let matched = placements
        .iter()
        .filter(|placement| placement.page_id.is_some())
        .count();
    let detail = placements
        .iter()
        .filter_map(|placement| {
            let page_id = placement.page_id.as_deref()?;
            let index = pages
                .iter()
                .position(|page| page.page_id == page_id)
                .map(|position| position + 1)
                .unwrap_or_default();
            Some(format!(
                "第{}页 {} — {}",
                index, placement.reference, placement.purpose
            ))
        })
        .collect::<Vec<_>>()
        .join("；");
    if detail.is_empty() {
        format!("{} 张素材均未匹配到具体页面", placements.len())
    } else {
        format!("{}/{} 张素材已配对：{}", matched, placements.len(), detail)
    }
}

async fn run_pipeline(
    app: AppHandle,
    config: &GenerationConfig,
    md_content: &str,
    db: std::sync::Arc<std::sync::Mutex<Connection>>,
    run_id: i64,
    project_id: Option<i64>,
) -> anyhow::Result<Vec<SlideBlueprint>> {
    use crate::generator::planning;
    use crate::generator::slides::{generate_single_page_pipeline, plan_single_page};
    use crate::icon::IconIndex;
    use crate::input::parse_markdown;

    reset_debug_dir(&config.debug_dir)?;

    let media_assets = load_project_media_assets(&db, project_id);
    let mut assets = crate::assets::AssetCatalog::collect(&config.project_dir, &media_assets);
    let llm_client = config.llm_client()?;
    let embedding_client = config.embedding_client()?;

    // Load icon index with embedding cache (sync DB read)
    let mut icon_index = {
        let conn = db.lock().unwrap();
        IconIndex::load_with_cache(&config.project_dir, &config.embedding.model, &*conn)?
    };
    if !icon_index.is_embedded() {
        emit_progress(
            &app,
            &db,
            run_id,
            "init",
            "首次使用，正在预计算图标向量...",
            0.04,
        );
        icon_index
            .embed_all(&embedding_client, &config.embedding.model, &db)
            .await?;
        emit_progress(&app, &db, run_id, "init", "图标向量缓存已生成", 0.05);
    }

    // Verify models are available
    emit_progress(&app, &db, run_id, "init", "验证大模型连接...", 0.05);
    planning::ensure_models_ready(
        &llm_client,
        &config.llm.model,
        &embedding_client,
        &config.embedding.model,
    )
    .await?;
    record_log(
        &app,
        &db,
        run_id,
        None,
        "init",
        "config",
        "模型连接检查",
        "模型与向量模型已通过连通性验证",
        json!({
            "llm_model": config.llm.model,
            "llm_base_url": config.llm.base_url,
            "embedding_model": config.embedding.model,
            "embedding_base_url": config.embedding.base_url,
        }),
        true,
    );

    // Parse document directly — no LLM cleaning step
    let doc = parse_markdown(md_content)?;
    record_log(
        &app,
        &db,
        run_id,
        None,
        "page_plan",
        "parsed",
        "文档结构",
        &format!("文稿解析为 {} 个章节", doc.sections.len()),
        json!({
            "title": doc.title,
            "section_count": doc.sections.len(),
            "document": doc,
        }),
        false,
    );

    // Phase 1: Derive page plans deterministically
    emit_progress(
        &app,
        &db,
        run_id,
        "page_plan",
        "从文档结构推导页面...",
        0.15,
    );
    let page_plans = planning::derive_page_plans(&doc, config.granularity);
    if page_plans.is_empty() {
        return Err(anyhow::anyhow!(
            "page derivation produced no pages — check that the document has content"
        ));
    }
    emit_progress(
        &app,
        &db,
        run_id,
        "page_plan",
        format!("推导了 {} 个页面", page_plans.len()).as_str(),
        0.2,
    );
    record_log(
        &app,
        &db,
        run_id,
        None,
        "page_plan",
        "status",
        "页面规划",
        &format!("确定性推导 {} 个页面", page_plans.len()),
        json!({
            "page_plans": page_plans,
        }),
        true,
    );

    // Phase 1b: match media-library images to pages by reading the article with them.
    // A caption alone cannot say where an image belongs, so this runs before per-page
    // planning and seeds each page's preferred_assets.
    if !media_assets.is_empty() && !config.multimodal.model.trim().is_empty() {
        emit_progress(
            &app,
            &db,
            run_id,
            "media_match",
            format!("比对 {} 张素材与文稿的对应关系...", media_assets.len()).as_str(),
            0.18,
        );
        match config.multimodal_client() {
            Ok(vision_client) => {
                let placements = crate::generator::media_match::match_media_to_pages(
                    &vision_client,
                    &config.multimodal.model,
                    &media_assets,
                    &app_media_dir(&app),
                    &doc.title,
                    &page_plans,
                    config.concurrency,
                )
                .await;
                let _ = crate::generator::utils::write_debug(
                    &config.debug_dir,
                    "00-media-match.json",
                    &serde_json::to_string_pretty(&placements).unwrap_or_default(),
                );
                record_log(
                    &app,
                    &db,
                    run_id,
                    None,
                    "media_match",
                    "status",
                    "图文配对",
                    &summarize_placements(&placements, &page_plans),
                    json!({ "placements": placements }),
                    true,
                );
                apply_placement_captions(&db, &placements);
                assets.set_placements(placements);
            }
            Err(e) => eprintln!("media match skipped: {e}"),
        }
    }

    // Phase 2: Per-page concurrent pipeline
    let total = page_plans.len();
    emit_progress(
        &app,
        &db,
        run_id,
        "generating",
        format!("并发规划 {total} 个页面并评估候选布局...").as_str(),
        0.2,
    );

    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(config.concurrency.max(1)));

    // Phase 2a: enrich page plans + collect layout candidates (concurrent, never fails)
    let mut plan_handles = Vec::with_capacity(total);
    for (idx, page_plan) in page_plans.iter().enumerate() {
        let llm_client_clone = llm_client.clone();
        let config_clone = config.clone();
        let doc_clone = doc.clone();
        let page_plan_owned = page_plan.clone();
        let assets_clone = assets.clone();
        let debug_dir_owned = config.debug_dir.clone();
        let sem_clone = sem.clone();
        let on_stage_cb = page_stage_callback(app.clone(), idx);

        plan_handles.push(tokio::task::spawn(async move {
            let _permit = sem_clone.acquire().await.unwrap();
            plan_single_page(
                &llm_client_clone,
                &config_clone,
                &doc_clone,
                page_plan_owned,
                idx,
                &assets_clone,
                &debug_dir_owned,
                on_stage_cb,
            )
            .await
        }));
    }

    let mut enriched_plans = Vec::with_capacity(total);
    let mut layout_candidates = Vec::with_capacity(total);
    for (handle, base_plan) in plan_handles.into_iter().zip(&page_plans) {
        match handle.await {
            Ok((plan, candidates)) => {
                enriched_plans.push(plan);
                layout_candidates.push(candidates);
            }
            Err(e) => {
                eprintln!(
                    "page {} planning task panicked or was cancelled: {e}",
                    base_plan.page_id
                );
                enriched_plans.push(base_plan.clone());
                // Empty list: assign_layouts substitutes deterministic fallback candidates.
                layout_candidates.push(Vec::new());
            }
        }
    }

    // Phase 2b: deck-level layout assignment (deterministic, CPU only)
    let layout_plans =
        planning::assign_layouts(&enriched_plans, &layout_candidates, config.aspect_ratio);
    let assignment_report = planning::layout_assignment_report(&layout_candidates, &layout_plans);
    let _ = crate::generator::utils::write_debug(
        &config.debug_dir,
        "02-layout-assignment.json",
        &serde_json::to_string_pretty(&assignment_report).unwrap_or_default(),
    );
    record_log(
        &app,
        &db,
        run_id,
        None,
        "layout_plan",
        "status",
        "布局分配",
        &planning::layout_assignment_summary(&layout_candidates, &layout_plans),
        json!({ "assignment": assignment_report, "page_plans": enriched_plans, "layout_plans": layout_plans }),
        true,
    );
    emit_progress(
        &app,
        &db,
        run_id,
        "generating",
        format!("布局分配完成，并发生成 {total} 张幻灯片内容...").as_str(),
        0.3,
    );

    // Phase 2c: content → normalize → validate + repair (concurrent)
    // Layout assignment has finished for all pages. Only pages that acquire a
    // semaphore permit are actively generating; the others are queued.
    for idx in 0..total {
        let _ = app.emit(
            "gen:page_status",
            PageStatusEvent {
                slide_index: idx,
                stage: "content".to_string(),
                message: Some("等待内容生成".to_string()),
                blueprint: None,
                waiting: true,
            },
        );
    }
    let blueprints_arc =
        std::sync::Arc::new(tokio::sync::Mutex::new(vec![None::<SlideBlueprint>; total]));
    let completed_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0usize));
    let failed_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0usize));

    let mut handles = Vec::with_capacity(total);

    for (idx, (page_plan, layout_plan)) in enriched_plans.iter().zip(&layout_plans).enumerate() {
        let app_clone = app.clone();
        let db_clone = db.clone();
        let llm_client_clone = llm_client.clone();
        let embedding_client_clone = embedding_client.clone();
        let config_clone = config.clone();
        let doc_clone = doc.clone();
        let page_plan_owned = page_plan.clone();
        let layout_plan = layout_plan.clone();
        let icon_index_clone = icon_index.clone();
        let assets_clone = assets.clone();
        let debug_dir_owned = config.debug_dir.clone();
        let blueprints_ref = blueprints_arc.clone();
        let sem_clone = sem.clone();
        let completed_ref = completed_count.clone();
        let failed_ref = failed_count.clone();
        let on_stage_cb = page_stage_callback(app.clone(), idx);

        let handle = tokio::task::spawn(async move {
            let _permit = sem_clone.acquire().await.unwrap();

            match generate_single_page_pipeline(
                &llm_client_clone,
                &embedding_client_clone,
                &config_clone,
                &doc_clone,
                page_plan_owned.clone(),
                layout_plan.clone(),
                idx,
                &icon_index_clone,
                &assets_clone,
                &debug_dir_owned,
                on_stage_cb,
            )
            .await
            {
                Ok(blueprint) => {
                    let kind_str = format!("{:?}", layout_plan.kind);
                    let title_str = page_plan_owned.page_title.clone();
                    // Store the result
                    {
                        let mut bp = blueprints_ref.lock().await;
                        bp[idx] = Some(blueprint.clone());
                    }
                    // Emit gen:page_status with blueprint for the frontend
                    let _ = app_clone.emit(
                        "gen:page_status",
                        PageStatusEvent {
                            slide_index: idx,
                            stage: "done".to_string(),
                            message: Some(format!("幻灯片 {}/{} 完成", idx + 1, total)),
                            blueprint: Some(blueprint.clone()),
                            waiting: false,
                        },
                    );
                    // Also emit gen:slide_ready for backward compatibility
                    let done_count =
                        completed_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    let _ = app_clone.emit(
                        "gen:slide_ready",
                        GenerationEvent {
                            stage: "content".to_string(),
                            message: format!("幻灯片 {}/{} 完成", idx + 1, total),
                            progress: Some(0.3 + 0.55 * done_count as f32 / total as f32),
                            slide_index: Some(idx),
                            blueprint: Some(blueprint),
                        },
                    );
                    record_log(
                        &app_clone,
                        &db_clone,
                        run_id,
                        Some(idx as i32),
                        "content",
                        "slide",
                        &format!("幻灯片 {}/{}", idx + 1, total),
                        &format!("{} · {}", title_str, kind_str),
                        json!({
                            "page_plan": page_plan_owned,
                            "layout_plan": layout_plan,
                        }),
                        true,
                    );
                }
                Err(e) => {
                    failed_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    record_log(
                        &app_clone,
                        &db_clone,
                        run_id,
                        Some(idx as i32),
                        "content",
                        "page_error",
                        "页面生成失败",
                        &format!("{e:#}"),
                        json!({"page_plan": page_plan_owned, "layout_plan": layout_plan, "error": format!("{e:#}")}),
                        true,
                    );
                    eprintln!("page {} pipeline failed: {e}", idx + 1);
                    let _ = app_clone.emit(
                        "gen:page_status",
                        PageStatusEvent {
                            slide_index: idx,
                            stage: "error".to_string(),
                            message: Some(format!("幻灯片 {} 生成失败: {e}", idx + 1)),
                            blueprint: None,
                            waiting: false,
                        },
                    );
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all pages, counting panics/cancellations as failures too
    for handle in handles {
        match handle.await {
            Ok(()) => {} // task completed (success or failure already counted inside)
            Err(e) => {
                // JoinError: task panicked or was cancelled
                failed_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                eprintln!("page task panicked or was cancelled: {e}");
            }
        }
    }

    let failed = failed_count.load(std::sync::atomic::Ordering::Relaxed);
    if failed > 0 {
        let msg = format!("有 {failed}/{total} 张幻灯片生成失败，将使用简化兜底页替代");
        eprintln!("WARNING: {msg}");
        record_log(
            &app,
            &db,
            run_id,
            None,
            "generating",
            "warning",
            "部分页面失败",
            &msg,
            json!({ "failed": failed, "total": total }),
            true,
        );
    }

    // Collect results, fill gaps with fallback slides
    let mut content_slides: Vec<SlideBlueprint> = {
        let bp = blueprints_arc.lock().await;
        enriched_plans
            .iter()
            .enumerate()
            .map(|(idx, page)| {
                bp.get(idx).cloned().flatten().unwrap_or_else(|| {
                    eprintln!("slide {} missing, using fallback", idx + 1);
                    crate::generator::normalize::make_fallback_slide(
                        page,
                        None,
                        config.aspect_ratio,
                    )
                })
            })
            .collect()
    };

    // Ensure fallback slides have all required fields filled (e.g. OutcomeGrid
    // needs icon/tag/top_bar_class on each card).  Slides already normalized by
    // the per-page pipeline are not harmed by a second pass.
    for slide in content_slides.iter_mut() {
        crate::generator::normalize::apply_component_defaults(slide);
        crate::generator::normalize::normalize_lengths(slide);
        crate::generator::normalize::normalize_tones(slide);
    }

    // Repair / fallback may have changed kinds after assignment; record what actually renders.
    let rendered_kinds: Vec<_> = content_slides
        .iter()
        .map(|slide| slide.kind.clone())
        .collect();
    let _ = crate::generator::utils::write_debug(
        &config.debug_dir,
        "05-layout-final.json",
        &serde_json::to_string_pretty(&planning::final_layout_report(
            &layout_plans,
            &rendered_kinds,
        ))
        .unwrap_or_default(),
    );

    // Phase 3: Assemble (cover + overview + content + closing)
    emit_progress(&app, &db, run_id, "assembling", "组装封面与目录...", 0.9);
    let final_slides = planning::assemble_slides(
        &llm_client,
        config,
        &doc,
        &page_plans,
        content_slides,
        &config.llm.model,
    )
    .await;

    // Keep the assembled deck even when final validation fails, so a content-page
    // retry can replace its slot rather than requiring another full generation.
    {
        let state = app.state::<Mutex<AppState>>();
        let mut st = state.lock().unwrap();
        if st.active_project_id != project_id {
            anyhow::bail!("当前项目已切换，生成结果未写入");
        }
        if let Some(pid) = project_id {
            db::update_project_blueprints(
                &db.lock().unwrap(),
                pid,
                &serde_json::to_string(&final_slides)?,
            )?;
        }
        st.blueprints = final_slides.clone();
    }

    emit_progress(&app, &db, run_id, "validate", "最终校验...", 0.95);
    let issues = crate::validate::validate_blueprints(&final_slides, &icon_index, assets.refs());
    if !issues.is_empty() {
        let issue_messages = issues
            .iter()
            .map(|issue| issue.message.clone())
            .collect::<Vec<_>>();
        record_log(
            &app,
            &db,
            run_id,
            None,
            "validate",
            "validation",
            "最终校验失败",
            &format!("发现 {} 个校验问题", issues.len()),
            json!({ "issues": issue_messages }),
            true,
        );
        let msg = issues
            .iter()
            .map(|i| i.message.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(anyhow::anyhow!("validation failed: {msg}"));
    }
    record_log(
        &app,
        &db,
        run_id,
        None,
        "validate",
        "validation",
        "最终校验通过",
        &format!("最终产出 {} 张幻灯片", final_slides.len()),
        json!({
            "slide_count": final_slides.len(),
            "final_blueprints": final_slides,
            "debug_files": std::fs::read_dir(&config.debug_dir)
                .ok()
                .into_iter()
                .flat_map(|iter| iter.filter_map(|entry| entry.ok()))
                .filter_map(|entry| {
                    entry.file_name().into_string().ok().map(|name| json!({ "name": name }))
                })
                .collect::<Vec<_>>(),
        }),
        true,
    );

    Ok(final_slides)
}

/// Ensure icon embeddings are computed and cached.
/// Call this on app startup to avoid first-generation delay.
#[tauri::command]
pub async fn ensure_icon_embeddings(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let (settings, project_dir, db) = {
        let st = state.lock().unwrap();
        (
            st.settings.clone(),
            st.project_dir.clone(),
            std::sync::Arc::clone(&st.db),
        )
    };

    let config = GenerationConfig::from_settings(
        &settings,
        project_dir,
        crate::types::AspectRatio::Ratio16x9,
    );

    let embedding_client = config.embedding_client().map_err(|e| e.to_string())?;

    let mut icon_index = {
        let conn = db.lock().unwrap();
        IconIndex::load_with_cache(&config.project_dir, &config.embedding.model, &*conn)
            .map_err(|e| e.to_string())?
    };

    if !icon_index.is_embedded() {
        println!("[icon-index] embedding icons at startup...");
        icon_index
            .embed_all(&embedding_client, &config.embedding.model, &db)
            .await
            .map_err(|e| e.to_string())?;
        println!("[icon-index] startup embedding complete");
    }

    {
        let mut st = state.lock().unwrap();
        st.app_settings.embeddings_ready = true;
        st.app_settings.initialized_embedding_model = config.embedding.model.clone();
        st.persist_app_settings().map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub async fn recommend_icons_for_query(
    query: String,
    limit: Option<usize>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<String>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let max = limit.unwrap_or(6).clamp(1, 12);

    let (settings, project_dir, db) = {
        let st = state.lock().unwrap();
        (
            st.settings.clone(),
            st.project_dir.clone(),
            std::sync::Arc::clone(&st.db),
        )
    };

    let config = GenerationConfig::from_settings(
        &settings,
        project_dir,
        crate::types::AspectRatio::Ratio16x9,
    );

    let icon_index = {
        let conn = db.lock().unwrap();
        IconIndex::load_with_cache(&config.project_dir, &config.embedding.model, &*conn)
            .map_err(|e| e.to_string())?
    };

    if icon_index.is_empty() {
        return Ok(Vec::new());
    }

    if icon_index.is_embedded() {
        let embedding_client = config.embedding_client().map_err(|e| e.to_string())?;
        if let Ok(embeddings) = embedding_client
            .embed(&config.embedding.model, &[query.to_string()])
            .await
        {
            if let Some(query_emb) = embeddings.first() {
                let semantic = icon_index
                    .semantic_search_with_emb(query_emb, max)
                    .into_iter()
                    .map(|(_, icon)| icon.full_name)
                    .collect::<Vec<_>>();
                if !semantic.is_empty() {
                    return Ok(semantic);
                }
            }
        }
    }

    Ok(icon_index
        .top_candidates(&format!("carbon {query}"), max)
        .into_iter()
        .map(|icon| icon.full_name)
        .collect())
}
