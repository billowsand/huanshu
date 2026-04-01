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

    // Stage 1: Page plan (deterministic — derived from document heading structure)
    emit_progress(&app, &db, run_id, "page_plan", "从文档结构推导页面...", 0.15);
    let page_plans = planning::run_page_plan(&client, config, &doc, &asset_paths, &config.debug_dir, config.granularity).await?;
    emit_progress(&app, &db, run_id, "page_plan", format!("推导了 {} 个页面", page_plans.len()).as_str(), 0.25);
    record_log(
        &app,
        &db,
        run_id,
        None,
        "page_plan",
        "prompt_io",
        "页面规划",
        &format!("产出 {} 个页面规划", page_plans.len()),
        json!({
            "system_prompt": read_optional_text(&config.debug_dir.join("01-page-plan.system.txt")),
            "user_prompt": read_optional_text(&config.debug_dir.join("01-page-plan.user.txt")),
            "raw_output": read_optional_text(&config.debug_dir.join("01-page-plan.raw.txt")),
            "parsed_output": read_optional_text(&config.debug_dir.join("01-page-plan.parsed.json")),
            "input_hash": read_optional_text(&config.debug_dir.join("01-page-plan.input-hash.txt")),
            "page_plans": page_plans,
        }),
        true,
    );

    // Stage 2: Layout plan
    emit_progress(&app, &db, run_id, "layout_plan", "选择布局模板...", 0.3);
    let layout_plans = planning::run_layout_plan(&client, config, &page_plans, &config.debug_dir).await?;
    emit_progress(&app, &db, run_id, "layout_plan", "布局规划完成", 0.45);
    record_log(
        &app,
        &db,
        run_id,
        None,
        "layout_plan",
        "prompt_io",
        "布局规划",
        "记录布局选择阶段的提示词、模型输出与最终布局方案",
        json!({
            "system_prompt": read_optional_text(&config.debug_dir.join("02-layout-plan.system.txt")),
            "user_prompt": read_optional_text(&config.debug_dir.join("02-layout-plan.user.txt")),
            "raw_output": read_optional_text(&config.debug_dir.join("02-layout-plan.raw.txt")),
            "parsed_output": read_optional_text(&config.debug_dir.join("02-layout-plan.parsed.json")),
            "final_output": read_optional_text(&config.debug_dir.join("02-layout-plan.final.parsed.json")),
            "audit": read_optional_text(&config.debug_dir.join("02-layout-plan.audit.txt")),
            "repair_prompt": read_optional_text(&config.debug_dir.join("02-layout-plan.repair.user.txt")),
            "repair_output": read_optional_text(&config.debug_dir.join("02-layout-plan.repair.raw.txt")),
            "layout_plans": layout_plans,
        }),
        true,
    );

    // Stage 3: Content generation (slide blueprints)
    // Note: precompute_semantic_candidates is called inside generate_content_slides_with_progress
    // and covers icon pre-selection in one batch embedding call.
    emit_progress(&app, &db, run_id, "content", "预选图标候选集（语义匹配）...", 0.47);
    emit_progress(&app, &db, run_id, "content", "生成幻灯片内容...", 0.5);
    let app_ref = app.clone();
    let db_ref = db.clone();
    let debug_dir = config.debug_dir.clone();
    let page_plans_for_logs = page_plans.clone();
    let layout_plans_for_logs = layout_plans.clone();
    let total = page_plans.len();
    let mut slides = crate::generator::slides::generate_content_slides_with_progress(
        &client,
        config,
        &doc,
        &page_plans,
        &layout_plans,
        &icon_index,
        &asset_paths,
        &config.debug_dir,
        config.concurrency,
        move |idx, blueprint| {
            let blueprint_for_event = blueprint.clone();
            let page = &page_plans_for_logs[idx];
            let layout = &layout_plans_for_logs[idx];
            let prefix = format!(
                "03-slide-{:02}-{}",
                crate::generator::utils::parse_page_display_index(&page.page_id).unwrap_or(idx + 1),
                crate::generator::utils::sanitize_filename(&page.page_id)
            );
            let _ = app_ref.emit(
                "gen:slide_ready",
                GenerationEvent {
                    stage: "content".to_string(),
                    message: format!("幻灯片 {}/{} 完成", idx + 1, total),
                    progress: Some(0.5 + 0.35 * (idx + 1) as f32 / total as f32),
                    slide_index: Some(idx),
                    blueprint: Some(blueprint_for_event),
                },
            );
            record_log(
                &app_ref,
                &db_ref,
                run_id,
                Some(idx as i32),
                "content",
                "slide",
                &format!("幻灯片 {}/{}", idx + 1, total),
                &format!(
                    "{} · {}",
                    page.page_title,
                    serde_json::to_string(&layout.kind).unwrap_or_default()
                ),
                json!({
                    "page_plan": page,
                    "layout_plan": layout,
                    "system_prompt": read_optional_text(&debug_dir.join(format!("{prefix}.system.txt"))),
                    "user_prompt": read_optional_text(&debug_dir.join(format!("{prefix}.user.txt"))),
                    "raw_output": read_optional_text(&debug_dir.join(format!("{prefix}.raw.txt"))),
                    "parsed_output": read_optional_text(&debug_dir.join(format!("{prefix}.parsed.json"))),
                    "blueprint": blueprint,
                }),
                true,
            );
        },
    )
    .await?;

    // Stage 4: Normalize + repair
    emit_progress(&app, &db, run_id, "normalize", "规范化与修复...", 0.85);
    crate::generator::normalize::normalize_blueprints(
        &mut slides,
        &client,
        &config.embedding_model,
        &icon_index,
        &asset_paths,
    )
    .await?;

    slides = crate::generator::normalize::repair_until_valid(
        &client,
        config,
        &doc,
        &page_plans,
        &layout_plans,
        slides,
        &icon_index,
        &asset_paths,
        &config.debug_dir,
    )
    .await?;
    record_log(
        &app,
        &db,
        run_id,
        None,
        "normalize",
        "repair",
        "规范化与修复",
        "记录修复阶段的重要提示词与结果",
        json!({
            "system_prompt": read_optional_text(&config.debug_dir.join("04-repair.system.txt")),
            "user_prompt": read_optional_text(&config.debug_dir.join("04-repair.user.txt")),
            "raw_output": read_optional_text(&config.debug_dir.join("04-repair.raw.txt")),
            "parsed_output": read_optional_text(&config.debug_dir.join("04-repair.parsed.json")),
            "normalized_slides": slides,
        }),
        true,
    );

    // Assemble final slide list (prepend cover/overview)
    let final_slides = crate::generator::planning::assemble_slides(
        &client,
        config,
        &doc,
        &page_plans,
        slides,
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
