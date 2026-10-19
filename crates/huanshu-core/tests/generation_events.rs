//! 生成流水线事件的**线上格式**快照对照。
//!
//! 期望值在 Tauri 版 `emit` 还在时由一次性测试打印生成，锁住事件名与 `serde_json` 载荷，
//! 防止重构改动界面层依赖的格式（原生界面经 `tasks::listen_events` 消费同一份载荷）。
//!
//! 比对一律**按值**（`serde_json::Value` 相等），不关心键的顺序。

use huanshu_core::events::{CoreEvent, GenerationEvent, GenerationLogEvent, PageStatusEvent};
use huanshu_core::types::SlideBlueprint;
use serde_json::{json, Value};

/// 在 Tauri 版 `emit` 存在时生成的快照（`crates/huanshu-core/tests/fixtures/`）。
const WIRE_SNAPSHOT: &str = include_str!("fixtures/generation-events.json");

fn blueprint() -> SlideBlueprint {
    serde_json::from_value(json!({
        "kind": "feature_grid",
        "title": "标题",
        "section": "02"
    }))
    .unwrap()
}

fn assert_matches_snapshot(event_name: &str, payload: Value) {
    let snapshot: Value = serde_json::from_str(WIRE_SNAPSHOT).unwrap();
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
    for (name, payload) in [
        CoreEvent::Progress(GenerationEvent {
            stage: "page_plan".into(),
            message: "推导了 3 个页面".into(),
            progress: Some(0.2),
            slide_index: None,
            blueprint: None,
        })
        .wire(),
        CoreEvent::Progress(GenerationEvent {
            stage: "content".into(),
            message: "幻灯片 1/3 完成".into(),
            progress: Some(0.48),
            slide_index: Some(0),
            blueprint: Some(blueprint()),
        })
        .wire(),
    ] {
        assert_eq!(name, "gen:progress");
        assert_matches_snapshot(name, payload);
    }
}

#[test]
fn slide_ready_events_keep_their_name_and_payload() {
    let (name, payload) = CoreEvent::SlideReady(GenerationEvent {
        stage: "content".into(),
        message: "幻灯片 1/3 完成".into(),
        progress: Some(0.48),
        slide_index: Some(0),
        blueprint: Some(blueprint()),
    })
    .wire();
    assert_eq!(name, "gen:slide_ready");
    assert_matches_snapshot(name, payload);
}

#[test]
fn error_events_keep_their_name_and_payload() {
    let (name, payload) = CoreEvent::Error(GenerationEvent {
        stage: "error".into(),
        message: "boom".into(),
        progress: None,
        slide_index: None,
        blueprint: None,
    })
    .wire();
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
        let (name, payload) = CoreEvent::PageStatus(event).wire();
        assert_eq!(name, "gen:page_status");
        assert_matches_snapshot(name, payload);
    }
}

#[test]
fn log_events_keep_their_name_and_payload() {
    let (name, payload) = CoreEvent::Log(GenerationLogEvent {
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
    })
    .wire();
    assert_eq!(name, "gen:log");
    assert_matches_snapshot(name, payload);
}
