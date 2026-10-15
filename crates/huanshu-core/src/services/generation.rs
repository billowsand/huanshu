//! 生成流水线：文稿 → 页面规划 → 布局分配 → 逐页内容 → 校验 → 组装。
//!
//! 这一层的「副作用出口」有两个：数据库（`db`）与 [`EventSink`]（进度、逐页状态、日志）。
//! 它不持有界面句柄，也不决定跑在哪个线程上；调用方（Tauri 命令层或将来的 GPUI 界面）
//! 传入状态与事件通道。带 `async` 的入口要求调用方已处于 tokio 运行时中。

use std::sync::Arc;

use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::Path;

use crate::config::GenerationConfig;
use crate::db::{self, GenerationLogEntry, GenerationRun};
use crate::events::{CoreEvent, EventSink, GenerationEvent, GenerationLogEvent, PageStatusEvent};
use crate::icon::IconIndex;
use crate::lmstudio::LmStudioClient;
use crate::state::SharedState;
use crate::types::SlideBlueprint;

use super::Result;

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
    events: &dyn EventSink,
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
        let _ = events.emit(CoreEvent::Log(GenerationLogEvent::from(entry)));
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
    events: &dyn EventSink,
    db: &std::sync::Arc<std::sync::Mutex<Connection>>,
    run_id: i64,
    stage: &str,
    msg: &str,
    progress: f32,
) {
    update_run_stage(db, run_id, "running", Some(stage));
    let _ = events.emit(CoreEvent::Progress(GenerationEvent {
        stage: stage.to_string(),
        message: msg.to_string(),
        progress: Some(progress),
        slide_index: None,
        blueprint: None,
    }));
    record_log(
        events,
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
pub fn detect_granularity(md_content: String) -> String {
    match crate::input::detect_granularity(&md_content) {
        crate::input::HeadingLevel::H2 => "h2".to_string(),
        crate::input::HeadingLevel::H3 => "h3".to_string(),
    }
}

pub async fn optimize_markdown_headings(
    state: &SharedState,
    raw_content: String,
    title_hint: Option<String>,
) -> Result<String> {
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
pub async fn generate_slides(
    events: Arc<dyn EventSink>,
    state: &SharedState,
    md_content: String,
    frontmatter_title: Option<String>,
    granularity: Option<String>,
    aspect_ratio: Option<crate::types::AspectRatio>,
) -> Result<usize> {
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

    emit_progress(&events, &db, run_id, "start", "开始生成流程", 0.0);
    record_log(
        &events,
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
        events.clone(),
        state,
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
                // The deck is already stored in the project this run was started for (the
                // assembly step wrote it there). `st.blueprints` is the open project's deck,
                // so mirror the result into it only while that is still the same project:
                // the native app lets the user open another project mid-run.
                if st.active_project_id == active_project_id {
                    st.blueprints = blueprints;
                }
                st.generation_running = false;
            }
            finish_run(&db, run_id, "done", Some("done"));
            let _ = events.emit(CoreEvent::Progress(GenerationEvent {
                stage: "done".to_string(),
                message: format!("生成完成，共 {count} 张幻灯片"),
                progress: Some(1.0),
                slide_index: None,
                blueprint: None,
            }));
            record_log(
                &events,
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
            let _ = events.emit(CoreEvent::Error(GenerationEvent {
                stage: "error".to_string(),
                message: msg.clone(),
                progress: None,
                slide_index: None,
                blueprint: None,
            }));
            record_log(
                &events,
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

/// The current set of blueprints (the viewer renders from it).
pub fn get_blueprints(state: &SharedState) -> Result<Vec<SlideBlueprint>> {
    let st = state.lock().unwrap();
    Ok(st.blueprints.clone())
}

/// Has this run reached assembly?
///
/// The log written right after the assembled deck is stored is the only marker for it: a
/// run that failed earlier left the project's **previous** deck in the database, and that
/// deck is not this run's result.
fn reached_assembly(logs: &[GenerationLogEntry]) -> bool {
    logs.iter()
        .any(|log| log.stage == "assembling" && log.kind == "assembled")
}

/// The deck a run's own project holds in the database right now.
fn run_deck(conn: &Connection, run: &GenerationRun) -> Result<Vec<SlideBlueprint>> {
    let Some(project_id) = run.project_id else {
        return Err("该生成不属于任何项目，读不到牌组".to_string());
    };
    let project = db::get_project(conn, project_id).map_err(|e| e.to_string())?;
    Ok(serde_json::from_str(&project.blueprints_json).unwrap_or_default())
}

/// The deck a project's **latest** generation produced, addressed by the project it
/// belongs to rather than by whichever project is open.
///
/// Empty when that run never assembled: the database then still holds the project's
/// previous deck, which must not be mistaken for this run's result.
pub fn get_generation_deck(state: &SharedState, project_id: i64) -> Result<Vec<SlideBlueprint>> {
    let st = state.lock().unwrap();
    let conn = st.db.lock().unwrap();
    let Some(run) = db::get_latest_generation_run(&conn, project_id).map_err(|e| e.to_string())?
    else {
        return Ok(Vec::new());
    };
    let logs = db::get_generation_logs(&conn, run.id).map_err(|e| e.to_string())?;
    if !reached_assembly(&logs) {
        return Ok(Vec::new());
    }
    run_deck(&conn, &run)
}

/// Get generation status
pub fn get_generation_status(state: &SharedState) -> Result<serde_json::Value> {
    let st = state.lock().unwrap();
    Ok(serde_json::json!({
        "running": st.generation_running,
        "slide_count": st.blueprints.len(),
        "last_error": st.last_error,
    }))
}

pub fn get_latest_generation_run(
    state: &SharedState,
    project_id: i64,
) -> Result<Option<GenerationRun>> {
    let st = state.lock().unwrap();
    let db = st.db.lock().unwrap();
    db::get_latest_generation_run(&*db, project_id).map_err(|e| e.to_string())
}

pub fn get_generation_logs(state: &SharedState, run_id: i64) -> Result<Vec<GenerationLogEvent>> {
    let st = state.lock().unwrap();
    let db = st.db.lock().unwrap();
    db::get_generation_logs(&*db, run_id)
        .map(|items| items.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

/// Repair a single slide given user feedback
pub async fn repair_slide(
    state: &SharedState,
    index: usize,
    feedback: String,
) -> Result<SlideBlueprint> {
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
    // Chart data comes from the document; feedback may reword the interpretation, never the data.
    if repaired.kind == crate::types::SlideKind::ChartFocus && blueprint.chart.is_some() {
        repaired.chart = blueprint.chart.clone();
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

/// Outcome of a single-page retry.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RetryPageResult {
    pub slide: SlideBlueprint,
    /// Whether the whole deck now passes validation (the run is then marked done).
    pub deck_ready: bool,
    /// What still blocks the deck when it is not ready; empty otherwise.
    pub remaining: String,
    /// Content pages that still fail validation.
    pub invalid_pages: Vec<usize>,
}

/// What a single-page retry needs from the database before it may call a model.
#[derive(Debug, Clone)]
struct RetryPlan {
    page: crate::types::PagePlan,
    layout: crate::types::LayoutPlan,
    ratio: crate::types::AspectRatio,
    /// The deck the run's own project assembled; the retried page replaces its slot.
    deck: Vec<SlideBlueprint>,
}

/// Read **and check** everything a page retry needs: the original page plan, its layout,
/// the project's aspect ratio and the deck that page replaces.
///
/// The base deck is the run's own project's deck, not the open project's, and the run must
/// have assembled it: without that log the database holds the project's previous deck, and
/// replacing a slot in *that* deck would edit slides this run never produced.
///
/// Synchronous on purpose — so the rules can be tested without a model.
fn plan_page_retry(
    conn: &Connection,
    run: &GenerationRun,
    page_index: usize,
    layout_kind: Option<crate::types::SlideKind>,
) -> Result<RetryPlan> {
    let logs = db::get_generation_logs(conn, run.id).map_err(|e| e.to_string())?;
    if !reached_assembly(&logs) {
        return Err("整副幻灯片尚未组装完成，请使用整副重试".to_string());
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
    let deck = run_deck(conn, run)?;
    if deck.len() != pages.len() + 3 {
        return Err("整副幻灯片尚未组装完成，请使用整副重试".to_string());
    }
    let ratio = deck[page_index + 2]
        .aspect_ratio
        .unwrap_or(crate::types::AspectRatio::Ratio16x9);
    if let Some(kind) = layout_kind {
        if let Some(reason) = crate::generator::layout_rules::ineligibility(&kind, &page, ratio) {
            return Err(format!("所选布局不适用于这一页：{reason}"));
        }
        layout.kind = kind;
        layout.reason = "用户手动选择布局进行重试".into();
    }
    Ok(RetryPlan {
        page,
        layout,
        ratio,
        deck,
    })
}

/// Retry a content page using its original source and assigned layout.
/// The assembled deck has two leading slides (cover and overview).
///
/// The retry belongs to the run's project (`generation_runs.project_id`): it reads that
/// project's deck and writes the result back there, whether or not the interface still has
/// it open. `st.blueprints` is only mirrored while that project is the open one.
pub async fn retry_generation_page(
    events: Arc<dyn EventSink>,
    state: &SharedState,
    run_id: i64,
    page_index: usize,
    layout_kind: Option<crate::types::SlideKind>,
) -> Result<RetryPageResult> {
    let (settings, project_dir, project_id, db, run, plan) = {
        let mut st = state.lock().unwrap();
        if st.generation_running {
            return Err("请等待当前生成或重试完成".into());
        }
        let db = std::sync::Arc::clone(&st.db);
        let (run, plan) = {
            let conn = db.lock().unwrap();
            let run = db::get_generation_run(&conn, run_id).map_err(|e| e.to_string())?;
            // Only the newest run of the run's own project can be retried; whichever project
            // is open has nothing to do with it.
            let latest_id: Option<i64> = conn
                .query_row(
                    "SELECT MAX(id) FROM generation_runs WHERE project_id IS ?1",
                    rusqlite::params![run.project_id],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            if latest_id != Some(run_id) {
                return Err("生成记录已过期，请重试当前生成记录中的页面".into());
            }
            let plan = plan_page_retry(&conn, &run, page_index, layout_kind)?;
            (run, plan)
        };
        st.generation_running = true;
        (
            st.settings.clone(),
            st.project_dir.clone(),
            run.project_id,
            db,
            run,
            plan,
        )
    };
    let RetryPlan {
        page,
        layout,
        ratio,
        deck,
    } = plan;
    let result: anyhow::Result<RetryPageResult> = async {
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
        let issues = crate::validate::validate_blueprints(
            std::slice::from_ref(&slide),
            &icons,
            assets.refs(),
        );
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
        let mut updated = deck;
        *updated
            .get_mut(page_index + 2)
            .ok_or_else(|| anyhow::anyhow!("幻灯片列表已改变"))? = slide.clone();
        // The page passed on its own, so keep it even when other pages are still broken:
        // requiring a clean deck here would make a deck with two bad pages unrecoverable.
        // The run only becomes "done" once the whole deck validates.
        let issues = crate::validate::validate_blueprints(&updated, &icons, assets.refs());
        let invalid_pages = invalid_content_pages(&issues, updated.len())
            .into_keys()
            .collect::<Vec<_>>();
        let remaining = if issues.is_empty() {
            String::new()
        } else {
            describe_deck_issues(&invalid_pages, &issues, updated.len())
        };
        let mut st = state.lock().unwrap();
        let mut conn = db.lock().unwrap();
        let tx = conn.transaction()?;
        if let Some(pid) = project_id {
            db::update_project_blueprints(&tx, pid, &serde_json::to_string(&updated)?)?;
        }
        if issues.is_empty() {
            db::finish_generation_run(&tx, run_id, "done", Some("done"))?;
        }
        tx.commit()?;
        // The retried deck is the run's project's deck; `st.blueprints` is the open project's
        // deck, so mirror it there only while that is still the same project.
        if st.active_project_id == project_id {
            st.blueprints = updated;
        }
        st.last_error = (!issues.is_empty()).then(|| remaining.clone());
        Ok(RetryPageResult {
            slide,
            deck_ready: issues.is_empty(),
            remaining,
            invalid_pages,
        })
    }
    .await;
    state.lock().unwrap().generation_running = false;
    let error = result.as_ref().err().map(|e| format!("{e:#}"));
    record_log(
        &events,
        &db,
        run_id,
        Some(page_index as i32),
        "content",
        "retry",
        "单页重试",
        error.as_deref().unwrap_or("该页重试完成"),
        json!({
            "page_plan": page,
            "layout_plan": layout,
            "error": error,
            "retry_succeeded": result.is_ok(),
            "deck_ready": result.as_ref().is_ok_and(|r| r.deck_ready),
            "remaining": result.as_ref().map(|r| r.remaining.as_str()).unwrap_or_default(),
        }),
        true,
    );
    result.map_err(|e| format!("{e:#}"))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RetryLayoutOptions {
    pub current_kind: crate::types::SlideKind,
    pub kinds: Vec<crate::types::SlideKind>,
}

/// Use the same layout eligibility table as automatic generation.
///
/// The ratio and the plans come from the run's own project, never from whichever project the
/// interface has open.
pub fn get_retry_layouts(
    state: &SharedState,
    run_id: i64,
    page_index: usize,
) -> Result<RetryLayoutOptions> {
    let st = state.lock().unwrap();
    let conn = st.db.lock().unwrap();
    let run = db::get_generation_run(&conn, run_id).map_err(|e| e.to_string())?;
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
    // The run recorded the ratio it ran with; fall back to the deck it assembled in its own
    // project (a run whose project row is gone has neither and reads as 16:9).
    let ratio = logs
        .iter()
        .find_map(|log| serde_json::from_value(log.detail["aspect_ratio"].clone()).ok())
        .or_else(|| {
            run_deck(&conn, &run)
                .ok()
                .and_then(|deck| deck.get(page_index + 2).and_then(|bp| bp.aspect_ratio))
        })
        .unwrap_or(crate::types::AspectRatio::Ratio16x9);
    Ok(RetryLayoutOptions {
        current_kind: layout.kind.clone(),
        kinds: crate::generator::planning::eligible_fallback_kinds(page, ratio),
    })
}

/// Per-page stage callback that forwards progress to the interface as `gen:page_status`.
fn page_stage_callback(
    events: Arc<dyn EventSink>,
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
            let _ = events.emit(CoreEvent::PageStatus(PageStatusEvent {
                slide_index,
                stage: stage_str.to_string(),
                message: msg.map(str::to_string),
                blueprint: None,
                waiting: false,
            }));
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
fn app_media_dir(state: &SharedState) -> std::path::PathBuf {
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

/// Content-page index for a slide of the assembled deck. Cover and overview lead the
/// deck and the closing slide ends it; those three have no content page.
fn content_page_of(deck_index: usize, deck_len: usize) -> Option<usize> {
    (deck_index >= 2 && deck_index + 1 < deck_len).then(|| deck_index - 2)
}

/// Validation messages of the assembled deck, grouped by content page.
pub(crate) fn invalid_content_pages(
    issues: &[crate::validate::ValidationIssue],
    deck_len: usize,
) -> std::collections::BTreeMap<usize, Vec<String>> {
    let mut pages = std::collections::BTreeMap::<usize, Vec<String>>::new();
    for issue in issues {
        if let Some(page) = content_page_of(issue.slide_index, deck_len) {
            pages.entry(page).or_default().push(issue.message.clone());
        }
    }
    pages
}

/// Flag every content page that failed final validation as a page error (log + live
/// status), the same way a failed content generation is flagged, so the page can be
/// retried. Returns the flagged content-page indices.
fn report_invalid_pages(
    events: &dyn EventSink,
    db: &std::sync::Arc<std::sync::Mutex<Connection>>,
    run_id: i64,
    issues: &[crate::validate::ValidationIssue],
    deck_len: usize,
) -> Vec<usize> {
    let pages = invalid_content_pages(issues, deck_len);
    for (&page, messages) in &pages {
        let summary = messages.join("；");
        record_log(
            events,
            db,
            run_id,
            Some(page as i32),
            "validate",
            "page_error",
            "页面未通过最终校验",
            &summary,
            json!({ "issues": messages }),
            true,
        );
        let _ = events.emit(CoreEvent::PageStatus(PageStatusEvent {
            slide_index: page,
            stage: "error".to_string(),
            message: Some(format!("未通过最终校验：{summary}")),
            blueprint: None,
            waiting: false,
        }));
    }
    pages.into_keys().collect()
}

/// Human-readable account of what still blocks the deck: retryable content pages, and
/// problems on cover/overview/closing that a page retry cannot fix.
pub(crate) fn describe_deck_issues(
    pages: &[usize],
    issues: &[crate::validate::ValidationIssue],
    deck_len: usize,
) -> String {
    let frame_issues = issues
        .iter()
        .filter(|issue| content_page_of(issue.slide_index, deck_len).is_none())
        .count();
    let mut parts = Vec::new();
    if !pages.is_empty() {
        let list = pages
            .iter()
            .map(|page| (page + 1).to_string())
            .collect::<Vec<_>>()
            .join("、");
        parts.push(format!("第 {list} 页需要重试"));
    }
    if frame_issues > 0 {
        parts.push(format!(
            "封面、目录或结尾有 {frame_issues} 处问题，需整副重新生成或手动编辑"
        ));
    }
    parts.join("；")
}

async fn run_pipeline(
    events: Arc<dyn EventSink>,
    state: &SharedState,
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
            &events,
            &db,
            run_id,
            "init",
            "首次使用，正在预计算图标向量...",
            0.04,
        );
        icon_index
            .embed_all(&embedding_client, &config.embedding.model, &db)
            .await?;
        emit_progress(&events, &db, run_id, "init", "图标向量缓存已生成", 0.05);
    }

    // Verify models are available
    emit_progress(&events, &db, run_id, "init", "验证大模型连接...", 0.05);
    planning::ensure_models_ready(
        &llm_client,
        &config.llm.model,
        &embedding_client,
        &config.embedding.model,
    )
    .await?;
    record_log(
        &events,
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
        &events,
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
        &events,
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
        &events,
        &db,
        run_id,
        "page_plan",
        format!("推导了 {} 个页面", page_plans.len()).as_str(),
        0.2,
    );
    record_log(
        &events,
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
            &events,
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
                    &app_media_dir(state),
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
                    &events,
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
        &events,
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
        let on_stage_cb = page_stage_callback(events.clone(), idx);

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
        &events,
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
        &events,
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
        let _ = events.emit(CoreEvent::PageStatus(PageStatusEvent {
            slide_index: idx,
            stage: "content".to_string(),
            message: Some("等待内容生成".to_string()),
            blueprint: None,
            waiting: true,
        }));
    }
    let blueprints_arc =
        std::sync::Arc::new(tokio::sync::Mutex::new(vec![None::<SlideBlueprint>; total]));
    let completed_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0usize));
    let failed_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0usize));

    let mut handles = Vec::with_capacity(total);

    for (idx, (page_plan, layout_plan)) in enriched_plans.iter().zip(&layout_plans).enumerate() {
        let events = events.clone();
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
        let on_stage_cb = page_stage_callback(events.clone(), idx);

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
                    let _ = events.emit(CoreEvent::PageStatus(PageStatusEvent {
                        slide_index: idx,
                        stage: "done".to_string(),
                        message: Some(format!("幻灯片 {}/{} 完成", idx + 1, total)),
                        blueprint: Some(blueprint.clone()),
                        waiting: false,
                    }));
                    // Also emit gen:slide_ready for backward compatibility
                    let done_count =
                        completed_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    let _ = events.emit(CoreEvent::SlideReady(GenerationEvent {
                        stage: "content".to_string(),
                        message: format!("幻灯片 {}/{} 完成", idx + 1, total),
                        progress: Some(0.3 + 0.55 * done_count as f32 / total as f32),
                        slide_index: Some(idx),
                        blueprint: Some(blueprint),
                    }));
                    record_log(
                        &events,
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
                        &events,
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
                    let _ = events.emit(CoreEvent::PageStatus(PageStatusEvent {
                        slide_index: idx,
                        stage: "error".to_string(),
                        message: Some(format!("幻灯片 {} 生成失败: {e}", idx + 1)),
                        blueprint: None,
                        waiting: false,
                    }));
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
            &events,
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
    emit_progress(&events, &db, run_id, "assembling", "组装封面与目录...", 0.9);
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
    //
    // The deck is the run's own project's (`generation_runs.project_id`), written there
    // whatever the interface has open. `st.blueprints` is the open project's deck, so it
    // only follows along while that is still the project this run belongs to.
    {
        let mut st = state.lock().unwrap();
        if let Some(pid) = project_id {
            db::update_project_blueprints(
                &db.lock().unwrap(),
                pid,
                &serde_json::to_string(&final_slides)?,
            )?;
        }
        if st.active_project_id == project_id {
            st.blueprints = final_slides.clone();
        }
    }
    // The marker `get_generation_deck` and the page retry look for: this run's deck is now
    // in the project's row, so the row holds this run's result and not the previous deck.
    record_log(
        &events,
        &db,
        run_id,
        None,
        "assembling",
        "assembled",
        "已组装牌组",
        &format!("共 {} 张幻灯片", final_slides.len()),
        json!({ "slide_count": final_slides.len() }),
        false,
    );

    emit_progress(&events, &db, run_id, "validate", "最终校验...", 0.95);
    let issues = crate::validate::validate_blueprints(&final_slides, &icon_index, assets.refs());
    if !issues.is_empty() {
        let issue_messages = issues
            .iter()
            .map(|issue| issue.message.clone())
            .collect::<Vec<_>>();
        record_log(
            &events,
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
        // Mark each offending content page as failed so it can be retried on its own;
        // otherwise only pages that failed during content generation are retryable.
        let pages = report_invalid_pages(&events, &db, run_id, &issues, final_slides.len());
        return Err(anyhow::anyhow!(
            "最终校验未通过：{}。{}",
            describe_deck_issues(&pages, &issues, final_slides.len()),
            issue_messages.join("; ")
        ));
    }
    record_log(
        &events,
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
pub async fn ensure_icon_embeddings(state: &SharedState) -> Result<()> {
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

pub async fn recommend_icons_for_query(
    query: String,
    limit: Option<usize>,
    state: &SharedState,
) -> Result<Vec<String>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use crate::types::SlideKind;
    use crate::validate::ValidationIssue;

    fn issue(slide_index: usize) -> ValidationIssue {
        ValidationIssue {
            slide_index,
            message: format!("slide {}: broken", slide_index + 1),
        }
    }

    /// 临时目录 + `db::open`（建好 schema）+ 最小状态，不碰真实数据目录。
    fn test_state() -> SharedState {
        let dir =
            std::env::temp_dir().join(format!("huanshu-generation-test-{}", uuid::Uuid::new_v4()));
        let conn = db::open(&dir).expect("open test database");
        std::fs::remove_dir_all(&dir).ok();
        std::sync::Mutex::new(AppState::for_tests(conn))
    }

    fn project(state: &SharedState, name: &str, deck: &Value) -> i64 {
        let st = state.lock().unwrap();
        let conn = st.db.lock().unwrap();
        let id = db::create_project(&conn, name, "# 文稿").expect("create project");
        db::update_project_blueprints(&conn, id, &deck.to_string()).expect("store deck");
        id
    }

    /// 两个内容页的页面规划与布局规划（页面重试读的就是这条日志的 detail）。
    fn plans() -> (Value, Value) {
        (
            json!([
                {
                    "page_id": "p01",
                    "section_title": "第一章",
                    "page_title": "第一页",
                    "objective": "先看结论",
                    "key_points": ["甲", "乙"],
                    "content_shape": "list",
                },
                {
                    "page_id": "p02",
                    "section_title": "第二章",
                    "page_title": "第二页",
                    "objective": "再看做法",
                    "key_points": ["丙", "丁"],
                    "content_shape": "grid",
                },
            ]),
            json!([
                { "page_id": "p01", "kind": "section_list", "title": "第一页", "reason": "自动分配" },
                { "page_id": "p02", "kind": "feature_grid", "title": "第二页", "reason": "自动分配" },
            ]),
        )
    }

    /// 两个内容页对应的整副牌组：封面 + 目录 + 2 页 + 结尾。
    fn five_slide_deck(title: &str) -> Value {
        json!([
            { "kind": "cover", "title": title },
            { "kind": "overview", "title": "目录" },
            { "kind": "section_list", "title": "第一页", "aspect_ratio": "ratio_16x9" },
            { "kind": "feature_grid", "title": "第二页" },
            { "kind": "closing", "title": "结束" },
        ])
    }

    /// 给项目造一次运行：规划日志、库里的牌组，可选地补组装日志。
    fn seed_run(state: &SharedState, project_id: Option<i64>, assembled: bool) -> i64 {
        let st = state.lock().unwrap();
        let conn = st.db.lock().unwrap();
        let run_id = db::create_generation_run(
            &conn,
            project_id,
            "# 文稿",
            Some("标题"),
            "/tmp/huanshu-debug",
        )
        .expect("create run");
        let (pages, layouts) = plans();
        db::append_generation_log(
            &conn,
            run_id,
            None,
            "layout_plan",
            "status",
            "布局分配",
            "分配完成",
            &json!({
                "page_plans": pages,
                "layout_plans": layouts,
                "aspect_ratio": "ratio_16x9",
            }),
            true,
        )
        .expect("layout log");
        if assembled {
            db::append_generation_log(
                &conn,
                run_id,
                None,
                "assembling",
                "assembled",
                "已组装牌组",
                "共 5 张幻灯片",
                &json!({ "slide_count": 5 }),
                false,
            )
            .expect("assembled log");
        }
        run_id
    }

    #[test]
    fn content_pages_exclude_cover_overview_and_closing() {
        // cover, overview, 3 content pages, closing
        let len = 6;
        assert_eq!(content_page_of(0, len), None);
        assert_eq!(content_page_of(1, len), None);
        assert_eq!(content_page_of(2, len), Some(0));
        assert_eq!(content_page_of(4, len), Some(2));
        assert_eq!(content_page_of(5, len), None);
    }

    #[test]
    fn issues_group_by_content_page_and_frame_issues_are_reported_separately() {
        let issues = vec![issue(2), issue(4), issue(4), issue(0)];
        let pages = invalid_content_pages(&issues, 6);
        assert_eq!(pages.keys().copied().collect::<Vec<_>>(), vec![0, 2]);
        assert_eq!(pages[&2].len(), 2);

        let keys = pages.into_keys().collect::<Vec<_>>();
        let text = describe_deck_issues(&keys, &issues, 6);
        assert!(text.contains("第 1、3 页需要重试"), "{text}");
        assert!(text.contains("1 处问题"), "{text}");
    }

    /// 生成属于它开始时的项目：别的项目打开着也要读得到它的牌组，读的是库里的那份。
    #[test]
    fn the_generation_deck_is_the_runs_project_deck_while_another_project_is_open() {
        let state = test_state();
        let a = project(&state, "甲", &five_slide_deck("甲的封面"));
        let b = project(
            &state,
            "乙",
            &json!([{ "kind": "cover", "title": "乙的封面" }]),
        );
        seed_run(&state, Some(a), true);

        // 打开的是乙。
        crate::services::projects::open_project(&state, b).unwrap();
        assert_eq!(
            get_blueprints(&state).unwrap().len(),
            1,
            "开着的还是乙的牌组"
        );

        let a_deck = get_generation_deck(&state, a).unwrap();
        assert_eq!(a_deck.len(), 5, "甲这次运行组装出的牌组");
        assert_eq!(a_deck[0].title, "甲的封面");
        // 乙没有运行：没有可取的运行就是空，而不是「乙的牌组」。
        assert!(get_generation_deck(&state, b).unwrap().is_empty());
        // 没跑过生成的项目同样是空。
        assert!(get_generation_deck(&state, 4242).unwrap().is_empty());
    }

    /// 没到达组装的运行不能把库里上一次的旧牌组当成自己的结果。
    #[test]
    fn the_generation_deck_is_empty_until_the_latest_run_assembled() {
        let state = test_state();
        let a = project(&state, "甲", &five_slide_deck("上一次的封面"));
        seed_run(&state, Some(a), true);
        assert_eq!(get_generation_deck(&state, a).unwrap().len(), 5);

        // 第二次运行在组装前失败：库里还是上一次的牌组，但它不是这次的结果。
        seed_run(&state, Some(a), false);
        assert!(get_generation_deck(&state, a).unwrap().is_empty());
    }

    /// 重试的布局选项读运行所属项目，与当前打开的项目无关。
    #[test]
    fn retry_layouts_follow_the_run_while_another_project_is_open() {
        let state = test_state();
        let a = project(&state, "甲", &five_slide_deck("甲的封面"));
        let b = project(
            &state,
            "乙",
            &json!([{ "kind": "cover", "title": "乙的封面" }]),
        );
        let run_id = seed_run(&state, Some(a), true);
        crate::services::projects::open_project(&state, b).unwrap();

        let options = get_retry_layouts(&state, run_id, 1).unwrap();
        assert_eq!(options.current_kind, SlideKind::FeatureGrid);
        assert!(!options.kinds.is_empty(), "该页至少有一个可选版式");
        assert!(get_retry_layouts(&state, run_id, 9).is_err(), "越界页");
    }

    /// 重试前置检查（同步，不需要模型）也是按运行所属项目取的：底线是那次运行的牌组。
    #[test]
    fn a_page_retry_plans_against_the_runs_projects_deck() {
        let state = test_state();
        let a = project(&state, "甲", &five_slide_deck("甲的封面"));
        let b = project(
            &state,
            "乙",
            &json!([{ "kind": "cover", "title": "乙的封面" }]),
        );
        let run_id = seed_run(&state, Some(a), true);
        crate::services::projects::open_project(&state, b).unwrap();

        let st = state.lock().unwrap();
        let conn = st.db.lock().unwrap();
        let run = db::get_generation_run(&conn, run_id).unwrap();
        let plan = plan_page_retry(&conn, &run, 1, None).unwrap();
        assert_eq!(plan.page.page_id, "p02");
        assert_eq!(plan.layout.kind, SlideKind::FeatureGrid);
        assert_eq!(plan.ratio, crate::types::AspectRatio::Ratio16x9);
        assert_eq!(plan.deck.len(), 5, "底稿是甲这次运行的牌组");
        assert_eq!(plan.deck[2].title, "第一页");

        // 越界页、不属于该页的版式都要拒。
        assert_eq!(
            plan_page_retry(&conn, &run, 9, None).unwrap_err(),
            "页面索引无效"
        );
        let err = plan_page_retry(&conn, &run, 1, Some(SlideKind::ChartFocus)).unwrap_err();
        assert!(err.starts_with("所选布局不适用于这一页："), "{err}");
    }

    /// 没到达组装（或牌组页数对不上规划）的运行不能重试单页，没有所属项目的也不行。
    #[test]
    fn a_page_retry_is_refused_without_an_assembled_deck() {
        let state = test_state();
        let a = project(&state, "甲", &five_slide_deck("甲的封面"));
        let before = seed_run(&state, Some(a), false);
        {
            let st = state.lock().unwrap();
            let conn = st.db.lock().unwrap();
            let run = db::get_generation_run(&conn, before).unwrap();
            let err = plan_page_retry(&conn, &run, 0, None).unwrap_err();
            assert!(err.contains("尚未组装完成"), "{err}");
        }

        // 到达过组装，但库里的牌组页数与规划对不上（3 页 vs 规划的 2 页）。
        {
            let st = state.lock().unwrap();
            let conn = st.db.lock().unwrap();
            db::update_project_blueprints(
                &conn,
                a,
                &json!([
                    { "kind": "cover", "title": "甲" },
                    { "kind": "overview", "title": "目录" },
                    { "kind": "closing", "title": "结束" },
                ])
                .to_string(),
            )
            .unwrap();
        }
        let assembled = seed_run(&state, Some(a), true);
        {
            let st = state.lock().unwrap();
            let conn = st.db.lock().unwrap();
            let run = db::get_generation_run(&conn, assembled).unwrap();
            let err = plan_page_retry(&conn, &run, 0, None).unwrap_err();
            assert!(err.contains("尚未组装完成"), "{err}");
        }

        // 不属于任何项目的运行没有可取的底稿。
        let orphan = seed_run(&state, None, true);
        let st = state.lock().unwrap();
        let conn = st.db.lock().unwrap();
        let orphan_run = db::get_generation_run(&conn, orphan).unwrap();
        let err = plan_page_retry(&conn, &orphan_run, 0, None).unwrap_err();
        assert!(err.contains("不属于任何项目"), "{err}");
    }
}
