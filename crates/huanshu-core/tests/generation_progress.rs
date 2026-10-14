//! `generation_progress` 与原版 TypeScript 函数的快照对照。
//!
//! 期望值由 `target/gen-generation-progress.ts`（不提交的一次性脚本，函数体逐字拷贝自
//! `src/utils/generationProgress.ts`、`src/views/steps/Step2Generate.vue`、
//! `src/stores/generation.ts`）生成。原版改动后重跑该脚本，Rust 实现必须跟着改到快照重新全绿。

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use huanshu_core::events::{GenerationLogEvent, PageStatusEvent};
use huanshu_core::generation_progress::{self as gp, PageProgressStatus};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct Snapshot {
    version: u32,
    tables: Tables,
    lookup_probes: Vec<LookupProbe>,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Tables {
    page_stages: Vec<(String, String)>,
    stage_labels: Vec<(String, String)>,
    log_stage_labels: Vec<(String, String)>,
    step2_kind_labels: Vec<(String, String)>,
}

#[derive(Debug, Deserialize)]
struct LookupProbe {
    #[serde(rename = "fn")]
    function: String,
    input: Option<String>,
    output: String,
}

/// 原版 `PageStatusEvent`：core 的 `PageStatusEvent` 只有 `Serialize`，测试里单独接。
#[derive(Debug, Deserialize)]
struct EventInput {
    slide_index: usize,
    stage: String,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    waiting: bool,
}

#[derive(Debug, Deserialize)]
struct Case {
    name: String,
    stage: String,
    running: bool,
    blueprints: Vec<Option<Value>>,
    logs: Vec<GenerationLogEvent>,
    page_statuses: BTreeMap<i64, PageProgressStatus>,
    retrying_pages: Vec<i64>,
    status_event_start: BTreeMap<i64, PageProgressStatus>,
    status_events: Vec<EventInput>,

    slide_items: Value,
    step_states: Value,
    summary: Value,
    slide_status: Value,
    preferred_index: Option<usize>,
    ready_blueprint_count: usize,
    important_log_count: usize,
    prompt_log_count: usize,
    recent_log_ids: Vec<i64>,
    statuses_from_logs: BTreeMap<i64, PageProgressStatus>,
    status_events_results: Vec<BTreeMap<i64, PageProgressStatus>>,
}

fn load() -> Snapshot {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/generation-progress.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let snapshot: Snapshot = serde_json::from_str(&raw).expect("parse generation-progress.json");
    assert_eq!(snapshot.version, 1, "snapshot version");
    snapshot
}

fn pairs(table: &[(&str, &str)]) -> Vec<(String, String)> {
    table
        .iter()
        .map(|(key, label)| ((*key).to_string(), (*label).to_string()))
        .collect()
}

#[test]
fn labels_match_the_original_tables() {
    let snapshot = load();
    assert_eq!(
        pairs(gp::PAGE_STAGES),
        snapshot.tables.page_stages,
        "PAGE_STAGES"
    );
    assert_eq!(
        pairs(gp::STAGE_LABELS),
        snapshot.tables.stage_labels,
        "STAGE_LABELS"
    );
    assert_eq!(
        pairs(gp::LOG_STAGE_LABELS),
        snapshot.tables.log_stage_labels,
        "LOG_STAGE_LABELS"
    );
    assert_eq!(
        pairs(gp::STEP2_KIND_LABELS),
        snapshot.tables.step2_kind_labels,
        "STEP2_KIND_LABELS"
    );
}

#[test]
fn label_lookups_match_the_original() {
    let snapshot = load();
    assert!(!snapshot.lookup_probes.is_empty(), "no lookup probes");
    for probe in &snapshot.lookup_probes {
        let text = probe.input.as_deref();
        let got = match probe.function.as_str() {
            "stage_label" => gp::stage_label(text.unwrap_or("")),
            "log_stage_label" => gp::log_stage_label(text.unwrap_or("")),
            "step2_kind_label" => gp::step2_kind_label(text),
            other => panic!("unknown probe function {other}"),
        };
        assert_eq!(
            got, probe.output,
            "{} with input {:?}",
            probe.function, probe.input
        );
    }
}

#[test]
fn generation_progress_matches_the_original_typescript() {
    let snapshot = load();
    assert!(
        !snapshot.cases.is_empty(),
        "snapshot has no cases — did gen-generation-progress.ts run?"
    );

    for case in &snapshot.cases {
        let name = &case.name;

        assert_eq!(
            gp::count_ready_blueprints(&case.blueprints),
            case.ready_blueprint_count,
            "{name}: count_ready_blueprints"
        );

        let items = gp::slide_items(&case.blueprints, &case.logs, &case.page_statuses);
        assert_eq!(
            serde_json::to_value(&items).expect("serialize items"),
            case.slide_items,
            "{name}: slide_items"
        );

        // 看板与整体进度传的是 `running || retryingPages.size > 0`（原版 Step2Generate.vue），
        // `slide_status` 只传 `running`。
        let effective_running = case.running || !case.retrying_pages.is_empty();

        let steps: Vec<Value> = items
            .iter()
            .map(|item| {
                let status = case.page_statuses.get(&item.index);
                serde_json::json!({
                    "index": item.index,
                    "planning": gp::page_step_state(item, status, "planning", effective_running),
                    "layout": gp::page_step_state(item, status, "layout", effective_running),
                    "content": gp::page_step_state(item, status, "content", effective_running),
                    "normalizing": gp::page_step_state(item, status, "normalizing", effective_running),
                    "validating": gp::page_step_state(item, status, "validating", effective_running),
                })
            })
            .collect();
        assert_eq!(Value::Array(steps), case.step_states, "{name}: step_states");

        let summary = gp::summarize_progress(&items, &case.page_statuses, effective_running);
        assert_eq!(
            serde_json::to_value(&summary).expect("serialize summary"),
            case.summary,
            "{name}: summary"
        );

        let retrying: BTreeSet<i64> = case.retrying_pages.iter().copied().collect();
        let statuses: Vec<Value> = items
            .iter()
            .map(|item| {
                let status = gp::slide_status(
                    item,
                    &case.stage,
                    case.running,
                    &case.page_statuses,
                    &retrying,
                );
                serde_json::json!({
                    "index": item.index,
                    "tone": status.tone,
                    "label": status.label,
                })
            })
            .collect();
        assert_eq!(
            Value::Array(statuses),
            case.slide_status,
            "{name}: slide_status"
        );

        assert_eq!(
            gp::preferred_slide_index(&items),
            case.preferred_index,
            "{name}: preferred_slide_index"
        );
        assert_eq!(
            gp::important_log_count(&case.logs),
            case.important_log_count,
            "{name}: important_log_count"
        );
        assert_eq!(
            gp::prompt_log_count(&case.logs),
            case.prompt_log_count,
            "{name}: prompt_log_count"
        );
        let recent: Vec<i64> = gp::recent_logs(&case.logs)
            .iter()
            .map(|log| log.id)
            .collect();
        assert_eq!(recent, case.recent_log_ids, "{name}: recent_logs");

        assert_eq!(
            gp::statuses_from_logs(&case.logs),
            case.statuses_from_logs,
            "{name}: statuses_from_logs"
        );

        assert_eq!(
            case.status_events.len(),
            case.status_events_results.len(),
            "{name}: status event count"
        );
        let mut event_statuses = case.status_event_start.clone();
        for (index, event) in case.status_events.iter().enumerate() {
            let wire = PageStatusEvent {
                slide_index: event.slide_index,
                stage: event.stage.clone(),
                message: event.message.clone(),
                blueprint: None,
                waiting: event.waiting,
            };
            gp::apply_page_status_event(&mut event_statuses, &wire);
            assert_eq!(
                event_statuses, case.status_events_results[index],
                "{name}: apply_page_status_event step {index}"
            );
        }
    }
}
