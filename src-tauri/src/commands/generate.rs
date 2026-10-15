//! 生成流水线相关的 Tauri 命令：取参数、锁状态、构造事件通道、转错误。
//! 实现见 `huanshu_core::services::generation`。

use std::sync::Arc;

use tauri::{AppHandle, State};

use super::Shared;

use huanshu_core::db::GenerationRun;
use huanshu_core::events::{CoreEvent, EventSink, GenerationLogEvent};
use huanshu_core::services::generation::{self as service, RetryLayoutOptions, RetryPageResult};
use huanshu_core::types::{AspectRatio, SlideBlueprint, SlideKind};

/// The `(event name, payload)` pair Tauri publishes. Identical to what `AppHandle::emit`
/// received before the pipeline moved into core — `tests/fixtures/generation-events.json`
/// was generated from the old code and locks this down.
fn to_tauri(event: CoreEvent) -> (&'static str, serde_json::Value) {
    event.wire()
}

/// 把 `CoreEvent` 转回 Tauri 的 `emit`。
struct TauriEventSink(AppHandle);

impl EventSink for TauriEventSink {
    fn emit(&self, event: CoreEvent) {
        use tauri::Emitter;
        let (name, payload) = to_tauri(event);
        let _ = self.0.emit(name, payload);
    }
}

#[tauri::command]
pub fn detect_granularity(md_content: String) -> String {
    service::detect_granularity(md_content)
}

#[tauri::command]
pub async fn optimize_markdown_headings(
    state: State<'_, Shared>,
    raw_content: String,
    title_hint: Option<String>,
) -> Result<String, String> {
    service::optimize_markdown_headings(&state, raw_content, title_hint).await
}

/// Run the full generation pipeline.
/// Emits `gen:progress` and `gen:slide_ready` events during execution.
#[tauri::command]
pub async fn generate_slides(
    app: AppHandle,
    state: State<'_, Shared>,
    md_content: String,
    frontmatter_title: Option<String>,
    granularity: Option<String>,
    aspect_ratio: Option<AspectRatio>,
) -> Result<usize, String> {
    let events: Arc<dyn EventSink> = Arc::new(TauriEventSink(app));
    service::generate_slides(
        events,
        &state,
        md_content,
        frontmatter_title,
        granularity,
        aspect_ratio,
    )
    .await
}

/// Get the current set of blueprints (called by viewer to render slides)
#[tauri::command]
pub fn get_blueprints(state: State<'_, Shared>) -> Result<Vec<SlideBlueprint>, String> {
    service::get_blueprints(&state)
}

/// The deck the latest generation of `project_id` produced, addressed by that project
/// instead of by whichever project happens to be open. Empty until that run assembled.
#[tauri::command]
pub fn get_generation_deck(
    state: State<'_, Shared>,
    project_id: i64,
) -> Result<Vec<SlideBlueprint>, String> {
    service::get_generation_deck(&state, project_id)
}

/// Get generation status
#[tauri::command]
pub fn get_generation_status(state: State<'_, Shared>) -> Result<serde_json::Value, String> {
    service::get_generation_status(&state)
}

#[tauri::command]
pub fn get_latest_generation_run(
    state: State<'_, Shared>,
    project_id: i64,
) -> Result<Option<GenerationRun>, String> {
    service::get_latest_generation_run(&state, project_id)
}

#[tauri::command]
pub fn get_generation_logs(
    state: State<'_, Shared>,
    run_id: i64,
) -> Result<Vec<GenerationLogEvent>, String> {
    service::get_generation_logs(&state, run_id)
}

/// Repair a single slide given user feedback
#[tauri::command]
pub async fn repair_slide(
    state: State<'_, Shared>,
    index: usize,
    feedback: String,
) -> Result<SlideBlueprint, String> {
    service::repair_slide(&state, index, feedback).await
}

/// Retry a content page using its original source and assigned layout.
/// The assembled deck has two leading slides (cover and overview).
#[tauri::command]
pub async fn retry_generation_page(
    app: AppHandle,
    state: State<'_, Shared>,
    run_id: i64,
    page_index: usize,
    layout_kind: Option<SlideKind>,
) -> Result<RetryPageResult, String> {
    let events: Arc<dyn EventSink> = Arc::new(TauriEventSink(app));
    service::retry_generation_page(events, &state, run_id, page_index, layout_kind).await
}

/// Use the same layout eligibility table as automatic generation.
#[tauri::command]
pub fn get_retry_layouts(
    state: State<'_, Shared>,
    run_id: i64,
    page_index: usize,
) -> Result<RetryLayoutOptions, String> {
    service::get_retry_layouts(&state, run_id, page_index)
}

/// Ensure icon embeddings are computed and cached.
/// Call this on app startup to avoid first-generation delay.
#[tauri::command]
pub async fn ensure_icon_embeddings(state: State<'_, Shared>) -> Result<(), String> {
    service::ensure_icon_embeddings(&state).await
}

#[tauri::command]
pub async fn recommend_icons_for_query(
    query: String,
    limit: Option<usize>,
    state: State<'_, Shared>,
) -> Result<Vec<String>, String> {
    service::recommend_icons_for_query(query, limit, &state).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use huanshu_core::events::{GenerationEvent, PageStatusEvent};
    use serde_json::json;

    fn blueprint() -> SlideBlueprint {
        serde_json::from_value(json!({
            "kind": "feature_grid",
            "title": "标题",
            "section": "02"
        }))
        .unwrap()
    }

    /// 事件名与载荷格式的快照。在 Tauri 版 `emit` 还在时由一次性测试打印生成，
    /// 此处逐字比对，防止重构改动了前端依赖的线上格式。
    const WIRE_SNAPSHOT: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../crates/huanshu-core/tests/fixtures/generation-events.json"
    ));

    fn assert_matches_snapshot(event_name: &str, payload: serde_json::Value) {
        let snapshot: serde_json::Value = serde_json::from_str(WIRE_SNAPSHOT).unwrap();
        let cases = snapshot
            .get(event_name)
            .unwrap_or_else(|| panic!("snapshot has no entry for {event_name}"))
            .as_array()
            .unwrap();
        assert!(
            cases.contains(&payload),
            "{event_name} payload changed:\n  actual:   {payload}\n  snapshot: {cases:?}"
        );
    }

    #[test]
    fn progress_events_keep_their_name_and_payload() {
        let (name, payload) = to_tauri(CoreEvent::Progress(GenerationEvent {
            stage: "page_plan".into(),
            message: "推导了 3 个页面".into(),
            progress: Some(0.2),
            slide_index: None,
            blueprint: None,
        }));
        assert_eq!(name, "gen:progress");
        assert_matches_snapshot(name, payload);
    }

    #[test]
    fn slide_ready_events_keep_their_name_and_payload() {
        let (name, payload) = to_tauri(CoreEvent::SlideReady(GenerationEvent {
            stage: "content".into(),
            message: "幻灯片 1/3 完成".into(),
            progress: Some(0.48),
            slide_index: Some(0),
            blueprint: Some(blueprint()),
        }));
        assert_eq!(name, "gen:slide_ready");
        assert_matches_snapshot(name, payload);
    }

    #[test]
    fn error_events_keep_their_name_and_payload() {
        let (name, payload) = to_tauri(CoreEvent::Error(GenerationEvent {
            stage: "error".into(),
            message: "boom".into(),
            progress: None,
            slide_index: None,
            blueprint: None,
        }));
        assert_eq!(name, "gen:error");
        assert_matches_snapshot(name, payload);
    }

    #[test]
    fn page_status_events_keep_their_name_and_payload() {
        for event in [
            PageStatusEvent {
                slide_index: 2,
                stage: "content".into(),
                message: Some("等待内容生成".into()),
                blueprint: None,
                waiting: true,
            },
            PageStatusEvent {
                slide_index: 1,
                stage: "done".into(),
                message: Some("幻灯片 2/3 完成".into()),
                blueprint: Some(blueprint()),
                waiting: false,
            },
            PageStatusEvent {
                slide_index: 4,
                stage: "error".into(),
                message: Some("幻灯片 5 生成失败: x".into()),
                blueprint: None,
                waiting: false,
            },
        ] {
            let (name, payload) = to_tauri(CoreEvent::PageStatus(event));
            assert_eq!(name, "gen:page_status");
            assert_matches_snapshot(name, payload);
        }
    }

    #[test]
    fn log_events_keep_their_name_and_payload() {
        let (name, payload) = to_tauri(CoreEvent::Log(GenerationLogEvent {
            id: 7,
            run_id: 3,
            seq: 12,
            slide_index: Some(1),
            stage: "content".into(),
            kind: "slide".into(),
            title: "幻灯片 2/3".into(),
            summary: "布局 · FeatureGrid".into(),
            detail: json!({ "page_plan": { "page_id": "p02" }, "n": 1.5 }),
            important: true,
            created_at: 1_700_000_000,
        }));
        assert_eq!(name, "gen:log");
        assert_matches_snapshot(name, payload);
    }
}
