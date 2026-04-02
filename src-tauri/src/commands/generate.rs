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
use tauri::{AppHandle, Emitter, State};

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

fn read_optional_text(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
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
            &*conn, run_id, slide_index, stage, kind, title, summary, &detail, important,
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

    let client = LmStudioClient::new(&settings.base_url).with_api_key(&settings.api_key);
    crate::generator::planning::ensure_models_ready(
        &client,
        &settings.model,
        &settings.embedding_model,
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

    let response = client
        .generate_text(&settings.model, system, &user)
        .await
        .map_err(|e| e.to_string())?;
    let cleaned = strip_markdown_fences(&response);

    let final_markdown = if cleaned.lines().any(|line| line.trim_start().starts_with("# ")) {
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
) -> Result<usize, String> {
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

    let mut config = GenerationConfig::from_settings(&settings, project_dir);
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
            "model": config.model,
            "embedding_model": config.embedding_model,
            "debug_dir": config.debug_dir.display().to_string(),
        }),
        true,
    );

    // Run the generator pipeline
    let result = run_pipeline(app.clone(), &config, &md_content, db.clone(), run_id).await;

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
    let (settings, project_dir, blueprints) = {
        let st = state.lock().unwrap();
        (
            st.settings.clone(),
            st.project_dir.clone(),
            st.blueprints.clone(),
        )
    };

    let config = GenerationConfig::from_settings(&settings, project_dir.clone());
    let blueprint = blueprints
        .get(index)
        .ok_or_else(|| format!("slide index {index} out of range"))?
        .clone();

    // Load icon index and asset paths so the repair prompt can provide
    // valid icon candidates and prevent hallucinated image paths.
    let icon_index = {
        let conn = state.lock().unwrap();
        let db = conn.db.lock().unwrap();
        IconIndex::load_with_cache(&project_dir, &settings.embedding_model, &db)
            .unwrap_or_else(|_| IconIndex::load(&project_dir).unwrap_or_else(|_| IconIndex::empty()))
    };
    let asset_paths = crate::validate::collect_assets(&project_dir);

    let repaired = crate::generator::slides::repair_single_slide(
        &config,
        &blueprint,
        &feedback,
        Some(&icon_index),
        Some(&asset_paths),
    )
    .await
    .map_err(|e| e.to_string())?;

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

async fn run_pipeline(
    app: AppHandle,
    config: &GenerationConfig,
    md_content: &str,
    db: std::sync::Arc<std::sync::Mutex<Connection>>,
    run_id: i64,
) -> anyhow::Result<Vec<SlideBlueprint>> {
    use crate::generator::planning;
    use crate::generator::slides::{generate_single_page_pipeline, PageStage};
    use crate::icon::IconIndex;
    use crate::input::parse_markdown;
    use crate::lmstudio::LmStudioClient;
    use crate::validate::collect_assets;

    reset_debug_dir(&config.debug_dir)?;

    let asset_paths = collect_assets(&config.project_dir);
    let client = LmStudioClient::new(&config.lmstudio_base_url)
        .with_api_key(&config.api_key);

    // Load icon index with embedding cache (sync DB read)
    let mut icon_index = {
        let conn = db.lock().unwrap();
        IconIndex::load_with_cache(&config.project_dir, &config.embedding_model, &*conn)?
    };
    if !icon_index.is_embedded() {
        emit_progress(&app, &db, run_id, "init", "首次使用，正在预计算图标向量...", 0.04);
        icon_index
            .embed_all(&client, &config.embedding_model, &db)
            .await?;
        emit_progress(&app, &db, run_id, "init", "图标向量缓存已生成", 0.05);
    }

    // Verify models are available
    emit_progress(&app, &db, run_id, "init", "验证大模型连接...", 0.05);
    planning::ensure_models_ready(&client, &config.model, &config.embedding_model).await?;
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
            "model": config.model,
            "embedding_model": config.embedding_model,
            "base_url": config.lmstudio_base_url,
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
    emit_progress(&app, &db, run_id, "page_plan", "从文档结构推导页面...", 0.15);
    let page_plans = planning::derive_page_plans(&doc, config.granularity);
    if page_plans.is_empty() {
        return Err(anyhow::anyhow!("page derivation produced no pages — check that the document has content"));
    }
    emit_progress(&app, &db, run_id, "page_plan", format!("推导了 {} 个页面", page_plans.len()).as_str(), 0.2);
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

    // Phase 2: Per-page concurrent pipeline
    let total = page_plans.len();
    emit_progress(&app, &db, run_id, "generating", format!("并发生成 {total} 张幻灯片...").as_str(), 0.2);

    let blueprints_arc = std::sync::Arc::new(tokio::sync::Mutex::new(vec![None::<SlideBlueprint>; total]));
    let used_layouts_arc: std::sync::Arc<tokio::sync::Mutex<Vec<(usize, String)>>> =
        std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(config.concurrency.max(1)));
    let completed_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0usize));
    let failed_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0usize));

    let mut handles = Vec::with_capacity(total);

    for (idx, page_plan) in page_plans.iter().enumerate() {
        let app_clone = app.clone();
        let db_clone = db.clone();
        let client_clone = client.clone();
        let config_clone = config.clone();
        let doc_clone = doc.clone();
        let page_plan_owned = page_plan.clone();
        let icon_index_clone = icon_index.clone();
        let asset_paths_clone = asset_paths.clone();
        let debug_dir_owned = config.debug_dir.clone();
        let blueprints_ref = blueprints_arc.clone();
        let used_layouts_ref = used_layouts_arc.clone();
        let sem_clone = sem.clone();
        let completed_ref = completed_count.clone();
        let failed_ref = failed_count.clone();

        let handle = tokio::task::spawn(async move {
            let _permit = sem_clone.acquire().await.unwrap();

            // Emit page status event
            let emit_stage_app = app_clone.clone();
            let emit_stage_idx = idx;
            let on_stage_cb: std::sync::Arc<dyn Fn(usize, PageStage, Option<&str>) + Send + Sync> =
                std::sync::Arc::new(move |_slide_idx: usize, stage: PageStage, msg: Option<&str>| {
                    let stage_str = match stage {
                        PageStage::Planning => "planning",
                        PageStage::Layout => "layout",
                        PageStage::Content => "content",
                        PageStage::Normalizing => "normalizing",
                        PageStage::Validating => "validating",
                        PageStage::Done => "done",
                        PageStage::Error => "error",
                        PageStage::Pending => "pending",
                    };
                    let _ = emit_stage_app.emit("gen:page_status", PageStatusEvent {
                        slide_index: emit_stage_idx,
                        stage: stage_str.to_string(),
                        message: msg.map(str::to_string),
                        blueprint: None,
                    });
                });

            match crate::generator::slides::generate_single_page_pipeline(
                &client_clone,
                &config_clone,
                &doc_clone,
                page_plan_owned.clone(),
                idx,
                total,
                used_layouts_ref.clone(),
                &icon_index_clone,
                &asset_paths_clone,
                &debug_dir_owned,
                on_stage_cb,
            )
            .await
            {
                Ok((blueprint, layout_plan)) => {
                    let kind_str = format!("{:?}", layout_plan.kind);
                    let title_str = page_plan_owned.page_title.clone();
                    // Store the result
                    {
                        let mut bp = blueprints_ref.lock().await;
                        bp[idx] = Some(blueprint.clone());
                    }
                    // Emit gen:page_status with blueprint for the frontend
                    let _ = app_clone.emit("gen:page_status", PageStatusEvent {
                        slide_index: idx,
                        stage: "done".to_string(),
                        message: Some(format!("幻灯片 {}/{} 完成", idx + 1, total)),
                        blueprint: Some(blueprint.clone()),
                    });
                    // Also emit gen:slide_ready for backward compatibility
                    let done_count = completed_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    let _ = app_clone.emit(
                        "gen:slide_ready",
                        GenerationEvent {
                            stage: "content".to_string(),
                            message: format!("幻灯片 {}/{} 完成", idx + 1, total),
                            progress: Some(0.2 + 0.65 * done_count as f32 / total as f32),
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
                    eprintln!("page {} pipeline failed: {e}", idx + 1);
                    let _ = app_clone.emit("gen:page_status", PageStatusEvent {
                        slide_index: idx,
                        stage: "error".to_string(),
                        message: Some(format!("幻灯片 {} 生成失败: {e}", idx + 1)),
                        blueprint: None,
                    });
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
        page_plans
            .iter()
            .enumerate()
            .map(|(idx, page)| {
                bp.get(idx)
                    .cloned()
                    .flatten()
                    .unwrap_or_else(|| {
                        eprintln!("slide {} missing, using fallback", idx + 1);
                        crate::generator::normalize::make_fallback_slide(page, None)
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

    // Phase 3: Assemble (cover + overview + content + closing)
    emit_progress(&app, &db, run_id, "assembling", "组装封面与目录...", 0.9);
    let final_slides = planning::assemble_slides(
        &client,
        config,
        &doc,
        &page_plans,
        content_slides,
        &config.model,
    )
    .await;

    emit_progress(&app, &db, run_id, "validate", "最终校验...", 0.95);
    let issues = crate::validate::validate_blueprints(&final_slides, &icon_index, &asset_paths);
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
pub async fn ensure_icon_embeddings(
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let (settings, project_dir, db) = {
        let st = state.lock().unwrap();
        (
            st.settings.clone(),
            st.project_dir.clone(),
            std::sync::Arc::clone(&st.db),
        )
    };

    let config = GenerationConfig::from_settings(&settings, project_dir);

    let client = LmStudioClient::new(&config.lmstudio_base_url)
        .with_api_key(&config.api_key);

    let mut icon_index = {
        let conn = db.lock().unwrap();
        IconIndex::load_with_cache(&config.project_dir, &config.embedding_model, &*conn)
            .map_err(|e| e.to_string())?
    };

    if !icon_index.is_embedded() {
        println!("[icon-index] embedding icons at startup...");
        icon_index
            .embed_all(&client, &config.embedding_model, &db)
            .await
            .map_err(|e| e.to_string())?;
        println!("[icon-index] startup embedding complete");
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

    let config = GenerationConfig::from_settings(&settings, project_dir);

    let icon_index = {
        let conn = db.lock().unwrap();
        IconIndex::load_with_cache(&config.project_dir, &config.embedding_model, &*conn)
            .map_err(|e| e.to_string())?
    };

    if icon_index.is_empty() {
        return Ok(Vec::new());
    }

    if icon_index.is_embedded() {
        let client = LmStudioClient::new(&config.lmstudio_base_url).with_api_key(&config.api_key);
        if let Ok(embeddings) = client.embed(&config.embedding_model, &[query.to_string()]).await {
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
