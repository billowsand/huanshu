//! 生成进度规则：原版 `src/utils/generationProgress.ts`、`src/views/steps/Step2Generate.vue`
//! 与 `src/stores/generation.ts` 里的一批纯函数（看板条目、逐页状态、整体进度与文案表）。
//!
//! **改这里必须改原版**（`src/utils/generationProgress.ts`、`src/views/steps/Step2Generate.vue`、
//! `src/stores/generation.ts`），反过来也一样——两边用 `tests/generation_progress.rs` 对着
//! `tests/fixtures/generation-progress.json` 快照逐条比对（快照由 `target/gen-generation-progress.ts`
//! 从原版 TypeScript **逐字拷贝**的函数生成）。
//!
//! 移植注意（与 JS 的语义差，快照已覆盖）：
//! - `statuses` 在原版是 JS `Map<number, …>`，只按键读取（`get` / `set`），这里用 [`BTreeMap`]
//!   确保遍历与序列化的顺序确定；`retrying_pages` 同理用 [`BTreeSet`]。
//! - 原版用 `??`（只对 `null` / `undefined` 生效）与 `!value`（JS 真值判断）区分缺失与假值，
//!   这里分别用 [`Option`] 与 [`is_truthy`] 还原；`false` / `0` / `""` 在 JS 里是假值，
//!   而 `{}` / `[]` 是真值，`is_truthy` 逐条对齐。
//! - `percent` 与原版一样是 `Math.round`：JS 是 `floor(x + 0.5)`，Rust 的 `f64::round`
//!   对 `.5` 远离零（`Math.round(-2.5) = -2` 而 `(-2.5).round() = -3`）。这里统一走
//!   [`js_round`]；调用处的输入恒为非负，两种写法同值，但避免以后引入负数时悄悄走偏。
//! - `items` / `statuses` 的下标在原版是 JS number，这里统一 `i64`（数组下标语义上恒为非负）。

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::events::{GenerationLogEvent, PageStatusEvent};

/// 原版 `PAGE_STAGES`：流水线的五个阶段（键 + 中文标签）。
pub const PAGE_STAGES: &[(&str, &str)] = &[
    ("planning", "页面规划"),
    ("layout", "布局排版"),
    ("content", "内容生成"),
    ("normalizing", "规范修复"),
    ("validating", "校验修复"),
];

/// 原版 `generationProgress.ts` 的 `order`：阶段先后顺序（`error` 不在其中，单独判断）。
const ORDER: &[&str] = &[
    "pending",
    "planning",
    "layout",
    "content",
    "normalizing",
    "validating",
    "done",
];

/// 原版 store 的 `STAGE_LABELS`：整体生成阶段的标签。
pub const STAGE_LABELS: &[(&str, &str)] = &[
    ("idle", "等待"),
    ("start", "初始化"),
    ("init", "验证连接"),
    ("page_plan", "规划页面"),
    ("generating", "生成中"),
    ("assembling", "组装中"),
    ("validate", "校验中"),
    ("done", "完成"),
    ("error", "出错"),
];

/// 原版 `Step2Generate.vue` 的 `LOG_STAGE_LABELS`：日志阶段的标签。
pub const LOG_STAGE_LABELS: &[(&str, &str)] = &[
    ("start", "开始"),
    ("init", "初始化"),
    ("clean", "文档清理"),
    ("page_plan", "页面规划"),
    ("layout_plan", "布局规划"),
    ("content", "内容生成"),
    ("normalize", "规范修复"),
    ("validate", "最终校验"),
    ("done", "完成"),
    ("error", "错误"),
];

/// 原版 `Step2Generate.vue` 的 `KIND_LABELS`：看板与详情里的版式名。
/// 注意这是 Step2 自己的一张表，**不是** `editor_schema` 的中文名。
pub const STEP2_KIND_LABELS: &[(&str, &str)] = &[
    ("cover", "封面"),
    ("closing", "结束页"),
    ("overview", "目录"),
    ("section_intro", "章节导览"),
    ("feature_grid", "特性格"),
    ("spotlight", "聚光灯"),
    ("split_layers", "分层"),
    ("section_list", "列表"),
    ("focus_example", "焦点例"),
    ("outcome_grid", "成果格"),
    ("center_grid", "中心格"),
    ("timeline", "时间轴"),
    ("step_flow", "步骤流"),
    ("process", "流程"),
    ("compare", "对比"),
    ("swot", "SWOT"),
];

/// 原版 `PageProgressStatus`：逐页状态。原版字段除 `stage` 外都可缺。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PageProgressStatus {
    pub stage: String,
    pub message: Option<String>,
    /// 原版 `previousStage`：出错时保留出错前所处的阶段。
    pub previous_stage: Option<String>,
    pub waiting: bool,
}

impl PageProgressStatus {
    /// 只给 `stage`、其余缺省的构造（原版 `{ stage: 'done' }` 这类写法）。
    pub fn new(stage: impl Into<String>) -> Self {
        Self {
            stage: stage.into(),
            message: None,
            previous_stage: None,
            waiting: false,
        }
    }
}

/// 原版 `SlideProgressItem`：看板的一条。`page_plan` / `layout_plan` / `blueprint`
/// 原版字段全部可缺，这里保留宽松的 JSON 结构。
/// （不比 `PartialEq`：`GenerationLogEvent` 没有实现它，看板条目的一致性靠快照 JSON 比对。）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SlideProgressItem {
    pub index: i64,
    pub page_plan: Option<Value>,
    pub layout_plan: Option<Value>,
    pub blueprint: Option<Value>,
    /// 该页的 `content` / `slide` 日志（原版 `contentLog`）。
    pub content_log: Option<GenerationLogEvent>,
}

/// 原版 `slideStatus` 的返回值：`tone` 用于着色（`done` / `active` / `pending` / `error`）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SlideStatus {
    pub tone: String,
    pub label: String,
}

/// 原版 `summarizeProgress` 的一个阶段卡。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StageCard {
    pub key: &'static str,
    pub label: &'static str,
    pub completed: usize,
    pub active: usize,
    /// 同 `ProgressSummary::percent`，也是 `Math.round`。
    pub percent: i64,
    /// `done` / `active` / `pending`。
    pub state: &'static str,
}

/// 原版 `summarizeProgress` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProgressSummary {
    pub total: usize,
    pub done: usize,
    pub error: usize,
    pub active: usize,
    pub interrupted: usize,
    pub pending: usize,
    pub percent: i64,
    /// 原版是模板串 `` `${…}%` ``，进度条宽度直接用它（保留 JS 的数字转字符串结果）。
    pub done_width: String,
    pub stage_cards: Vec<StageCard>,
}

/// JS 真值判断：`false` / `0` / `""` / `null` 为假，`{}` 与 `[]` 为真。
fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(inner) => *inner,
        // JSON 不会有 NaN，数字只有 0 是假值。
        Value::Number(number) => number.as_f64().map(|n| n != 0.0).unwrap_or(true),
        Value::String(text) => !text.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// 原版 `!value`：`null` / `undefined` 与上面几种假值一样为假。
fn truthy(value: &Option<Value>) -> bool {
    value.as_ref().map(is_truthy).unwrap_or(false)
}

/// 原版 `extractArrayDetail`：从最后一条日志往前找第一个带该键的数组（空数组也算）。
fn array_field<'a>(logs: &'a [GenerationLogEvent], key: &str) -> &'a [Value] {
    logs.iter()
        .rev()
        .find_map(|log| {
            log.detail
                .as_object()
                .and_then(|detail| detail.get(key))
                .and_then(Value::as_array)
        })
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// 原版 `extractPagePlanFromLog` / `extractLayoutPlanFromLog` / `extractBlueprintFromLog`：
/// `detail` 与 `detail[key]` 都必须是非数组对象。
fn record_field(log: &GenerationLogEvent, key: &str) -> Option<Value> {
    let value = log.detail.as_object()?.get(key)?;
    if value.is_object() {
        Some(value.clone())
    } else {
        None
    }
}

/// JS `Math.round`（`floor(x + 0.5)`）；负数与 `.5` 的边界见模块文档。
fn js_round(value: f64) -> f64 {
    if value.is_finite() {
        (value + 0.5).floor()
    } else {
        value
    }
}

fn percent_of(part: usize, total: usize) -> i64 {
    if total == 0 {
        return 0;
    }
    js_round(part as f64 / total as f64 * 100.0) as i64
}

/// 阶段在流水线里的位置（原版 `order.indexOf`，未知阶段为 `-1`）。
fn stage_rank(stage: &str) -> i64 {
    ORDER
        .iter()
        .position(|known| *known == stage)
        .map(|index| index as i64)
        .unwrap_or(-1)
}

/// 原版 `countReadyBlueprints`：数非空蓝图。
pub fn count_ready_blueprints(blueprints: &[Option<Value>]) -> usize {
    blueprints.iter().filter(|item| truthy(item)).count()
}

/// 原版 `resolveProgressStage`。
pub fn resolve_progress_stage<'a>(
    item: &'a SlideProgressItem,
    status: Option<&'a PageProgressStatus>,
) -> &'a str {
    if let Some(status) = status {
        if ORDER.contains(&status.stage.as_str()) || status.stage == "error" {
            return status.stage.as_str();
        }
    }
    if truthy(&item.blueprint) {
        return "done";
    }
    if truthy(&item.layout_plan) {
        return "content";
    }
    "pending"
}

/// 原版 `pageStepState`：某一页在某个流水线阶段的样式（`done` / `active` / `pending`）。
///
/// 原版调用处传的是 `running || retryingPages.size > 0`，这里由调用方算好传入。
pub fn page_step_state(
    item: &SlideProgressItem,
    status: Option<&PageProgressStatus>,
    key: &str,
    running: bool,
) -> &'static str {
    let stage = resolve_progress_stage(item, status);
    let effective_stage = if stage == "error" {
        status
            .and_then(|status| status.previous_stage.as_deref())
            .unwrap_or("pending")
    } else {
        stage
    };
    let rank = stage_rank(effective_stage);
    let key_rank = stage_rank(key);

    if rank > key_rank
        || (key == "layout" && truthy(&item.layout_plan))
        || (key == "planning" && truthy(&item.layout_plan))
    {
        return "done";
    }

    let waiting = status.map(|status| status.waiting).unwrap_or(false);
    if rank == key_rank && stage != "error" && running && !waiting && status.is_some() {
        return "active";
    }
    "pending"
}

/// 原版 `summarizeProgress`。
pub fn summarize_progress(
    items: &[SlideProgressItem],
    statuses: &BTreeMap<i64, PageProgressStatus>,
    running: bool,
) -> ProgressSummary {
    let mut done = 0usize;
    let mut error = 0usize;
    let mut active = 0usize;
    let mut interrupted = 0usize;

    for item in items {
        let status = statuses.get(&item.index);
        let stage = resolve_progress_stage(item, status);
        if stage == "done" {
            done += 1;
        } else if stage == "error" {
            error += 1;
        } else if stage != "pending"
            && !status.map(|status| status.waiting).unwrap_or(false)
            && status.is_some()
        {
            if running {
                active += 1;
            } else {
                interrupted += 1;
            }
        }
    }

    let total = items.len();
    let stage_cards = PAGE_STAGES
        .iter()
        .map(|(key, label)| {
            let states: Vec<&str> = items
                .iter()
                .map(|item| page_step_state(item, statuses.get(&item.index), key, running))
                .collect();
            let completed = states.iter().filter(|state| **state == "done").count();
            let active = states.iter().filter(|state| **state == "active").count();
            StageCard {
                key,
                label,
                completed,
                active,
                percent: percent_of(completed, total),
                state: if total > 0 && completed == total {
                    "done"
                } else if active > 0 {
                    "active"
                } else {
                    "pending"
                },
            }
        })
        .collect();

    ProgressSummary {
        total,
        done,
        error,
        active,
        interrupted,
        pending: total - (done + error + active + interrupted),
        percent: percent_of(done, total),
        done_width: format!(
            "{}%",
            if total > 0 {
                done as f64 / total as f64 * 100.0
            } else {
                0.0
            }
        ),
        stage_cards,
    }
}

/// 原版 `contentLogBySlideIndex`：`content` + `slide` 日志按页下标取最后一条。
fn content_log_by_slide_index(logs: &[GenerationLogEvent]) -> BTreeMap<i64, &GenerationLogEvent> {
    let mut map = BTreeMap::new();
    for log in logs {
        if log.stage != "content" || log.kind != "slide" {
            continue;
        }
        let Some(index) = log.slide_index else {
            continue;
        };
        map.insert(index as i64, log);
    }
    map
}

/// 原版 `slideItems`（含 `extractArrayDetail` / `contentLogBySlideIndex` /
/// `extractPagePlanFromLog` / `extractLayoutPlanFromLog` / `extractBlueprintFromLog`）。
///
/// 看板条目数是 `page_plans`、`layout_plans`、content 日志、`page_statuses`、内容蓝图
/// 五者的最大值；首张蓝图是 `cover` 时牌组前面两张（封面、目录）与最后一张（结尾页）要切掉。
pub fn slide_items(
    blueprints: &[Option<Value>],
    logs: &[GenerationLogEvent],
    page_statuses: &BTreeMap<i64, PageProgressStatus>,
) -> Vec<SlideProgressItem> {
    let page_plans = array_field(logs, "page_plans");
    let layout_plans = array_field(logs, "layout_plans");
    let content_logs = content_log_by_slide_index(logs);

    let first_is_cover = blueprints
        .first()
        .and_then(|item| item.as_ref())
        .and_then(|item| item.get("kind"))
        .and_then(Value::as_str)
        == Some("cover");
    let content_blueprints: &[Option<Value>] = if first_is_cover {
        // JS 的 `slice(2, -1)`：起止越界时自动夹到区间内，可能为空。
        let start = 2.min(blueprints.len());
        let end = blueprints.len().saturating_sub(1);
        if start < end {
            &blueprints[start..end]
        } else {
            &[]
        }
    } else {
        blueprints
    };

    let max_log_index = content_logs
        .keys()
        .next_back()
        .map(|key| key + 1)
        .unwrap_or(0);
    let max_status_index = page_statuses
        .keys()
        .next_back()
        .map(|key| key + 1)
        .unwrap_or(0);
    // JS `Array.from({ length: 负数 })` 直接得到空数组。
    let total = (page_plans.len() as i64)
        .max(layout_plans.len() as i64)
        .max(max_log_index)
        .max(max_status_index)
        .max(content_blueprints.len() as i64)
        .max(0);

    (0..total)
        .map(|index| {
            let content_log = content_logs.get(&index).copied();
            SlideProgressItem {
                index,
                // `??` 只对 null / undefined 生效：数组里显式的 null 也回落到日志。
                page_plan: page_plans
                    .get(index as usize)
                    .filter(|value| !value.is_null())
                    .cloned()
                    .or_else(|| content_log.and_then(|log| record_field(log, "page_plan"))),
                layout_plan: layout_plans
                    .get(index as usize)
                    .filter(|value| !value.is_null())
                    .cloned()
                    .or_else(|| content_log.and_then(|log| record_field(log, "layout_plan"))),
                blueprint: content_blueprints
                    .get(index as usize)
                    .and_then(|value| value.as_ref())
                    .cloned()
                    .or_else(|| content_log.and_then(|log| record_field(log, "blueprint"))),
                content_log: content_log.cloned(),
            }
        })
        .collect()
}

/// 原版 `slideStatus`：看板卡片右上角的状态标签。
///
/// `stage` 是整体生成阶段（原版 `props.stage`），`running` 是原版 `props.running`
/// （调用处不看 `retryingPages`，与 `pageStepState` 不同）。
pub fn slide_status(
    item: &SlideProgressItem,
    stage: &str,
    running: bool,
    page_statuses: &BTreeMap<i64, PageProgressStatus>,
    retrying_pages: &BTreeSet<i64>,
) -> SlideStatus {
    if retrying_pages.contains(&item.index) {
        return SlideStatus {
            tone: "active".to_string(),
            label: if running { "等待重试" } else { "重试中" }.to_string(),
        };
    }

    if stage == "error" {
        let status = page_statuses.get(&item.index);
        if status.map(|status| status.stage.as_str()) == Some("error") {
            return SlideStatus {
                tone: "error".to_string(),
                label: "生成失败".to_string(),
            };
        }
        if truthy(&item.blueprint) {
            return SlideStatus {
                tone: "done".to_string(),
                label: "已完成".to_string(),
            };
        }
        return SlideStatus {
            tone: "error".to_string(),
            label: "生成中断".to_string(),
        };
    }

    let status = page_statuses.get(&item.index);
    let resolved = resolve_progress_stage(item, status);
    if resolved == "done" {
        return SlideStatus {
            tone: "done".to_string(),
            label: "已完成".to_string(),
        };
    }
    if resolved == "error" {
        return SlideStatus {
            tone: "error".to_string(),
            label: "生成失败".to_string(),
        };
    }
    if !running && resolved != "pending" {
        return SlideStatus {
            tone: "pending".to_string(),
            label: "已中断".to_string(),
        };
    }
    if status.map(|status| status.waiting).unwrap_or(false) || resolved == "pending" {
        // 原版 `ps?.message || '等待开始'`：空串也算没有消息。
        let label = status
            .and_then(|status| status.message.clone())
            .filter(|message| !message.is_empty())
            .unwrap_or_else(|| "等待开始".to_string());
        return SlideStatus {
            tone: "pending".to_string(),
            label,
        };
    }
    let label = PAGE_STAGES
        .iter()
        .find(|(key, _)| *key == resolved)
        .map(|(_, label)| *label)
        .unwrap_or("生成");
    SlideStatus {
        tone: "active".to_string(),
        label: format!("{label}中"),
    }
}

/// 原版 `preferredSlideIndex`：从后往前找第一个已经有结果的页，都没有就选第 0 页。
pub fn preferred_slide_index(items: &[SlideProgressItem]) -> Option<usize> {
    for (index, item) in items.iter().enumerate().rev() {
        if truthy(&item.blueprint) || truthy(&item.layout_plan) || truthy(&item.page_plan) {
            return Some(index);
        }
    }
    if items.is_empty() {
        None
    } else {
        Some(0)
    }
}

/// 原版 `importantLogCount`。
pub fn important_log_count(logs: &[GenerationLogEvent]) -> usize {
    logs.iter().filter(|log| log.important).count()
}

/// 原版 `promptLogCount`。
pub fn prompt_log_count(logs: &[GenerationLogEvent]) -> usize {
    logs.iter()
        .filter(|log| {
            matches!(
                log.kind.as_str(),
                "input" | "prompt_io" | "slide" | "repair"
            )
        })
        .count()
}

/// 原版 `recentLogs`：最后 10 条，倒序。
pub fn recent_logs(logs: &[GenerationLogEvent]) -> Vec<&GenerationLogEvent> {
    logs.iter().rev().take(10).collect()
}

/// 原版 store 的 `stageLabel`（`STAGE_LABELS[stage] ?? stage`）。
pub fn stage_label<'a>(stage: &'a str) -> &'a str {
    STAGE_LABELS
        .iter()
        .find(|(key, _)| *key == stage)
        .map(|(_, label)| *label)
        .unwrap_or(stage)
}

/// 原版 `logStageLabel`（`LOG_STAGE_LABELS[stage] ?? stage`）。
pub fn log_stage_label<'a>(stage: &'a str) -> &'a str {
    LOG_STAGE_LABELS
        .iter()
        .find(|(key, _)| *key == stage)
        .map(|(_, label)| *label)
        .unwrap_or(stage)
}

/// 原版 `kindLabel`：空版式 → 「待定」，其余查表、查不到就用原值。
pub fn step2_kind_label<'a>(kind: Option<&'a str>) -> &'a str {
    let Some(kind) = kind.filter(|kind| !kind.is_empty()) else {
        return "待定";
    };
    STEP2_KIND_LABELS
        .iter()
        .find(|(key, _)| *key == kind)
        .map(|(_, label)| *label)
        .unwrap_or(kind)
}

/// 原版 `loadLatestRun` 里从日志恢复页状态的那段：
/// `page_error` → 该页出错；`retry` → 看 `detail.retry_succeeded` 判定成功还是失败。
/// 后写的日志覆盖先写的（原版按顺序 `pageStatuses.set`）。
pub fn statuses_from_logs(logs: &[GenerationLogEvent]) -> BTreeMap<i64, PageProgressStatus> {
    let mut statuses = BTreeMap::new();
    for log in logs {
        let Some(index) = log.slide_index else {
            continue;
        };
        let index = index as i64;
        if log.kind == "page_error" {
            statuses.insert(
                index,
                PageProgressStatus {
                    stage: "error".to_string(),
                    message: Some(log.summary.clone()),
                    previous_stage: Some("content".to_string()),
                    waiting: false,
                },
            );
        } else if log.kind == "retry" {
            let succeeded = log
                .detail
                .get("retry_succeeded")
                .map(is_truthy)
                .unwrap_or(false);
            statuses.insert(
                index,
                if succeeded {
                    PageProgressStatus {
                        stage: "done".to_string(),
                        message: Some("重试完成".to_string()),
                        previous_stage: None,
                        waiting: false,
                    }
                } else {
                    PageProgressStatus {
                        stage: "error".to_string(),
                        message: Some(log.summary.clone()),
                        previous_stage: Some("content".to_string()),
                        waiting: false,
                    }
                },
            );
        }
    }
    statuses
}

/// 原版 `gen:page_status` 监听里写 `previousStage` 的那段：
/// 出错时记下出错前所处的阶段，连续两次 error 保留最早的那次（非 error）阶段。
pub fn apply_page_status_event(
    statuses: &mut BTreeMap<i64, PageProgressStatus>,
    event: &PageStatusEvent,
) -> PageProgressStatus {
    let index = event.slide_index as i64;
    let previous = statuses.get(&index);
    let previous_stage = if event.stage == "error" {
        match previous {
            Some(previous) if previous.stage == "error" => previous.previous_stage.clone(),
            Some(previous) => Some(previous.stage.clone()),
            None => None,
        }
    } else {
        None
    };
    let status = PageProgressStatus {
        stage: event.stage.clone(),
        message: event.message.clone(),
        previous_stage,
        // 原版是 `ev.waiting ?? false`；core 的 `PageStatusEvent::waiting` 是必填的布尔。
        waiting: event.waiting,
    };
    statuses.insert(index, status.clone());
    status
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_round_matches_javascript_halves() {
        // Math.round 是 floor(x + 0.5)：2.5 → 3，而 -2.5 → -2（Rust 的 f64::round 会给 -3）。
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(12.5), 13.0);
        assert_eq!(js_round(0.0), 0.0);
        assert_eq!(js_round(f64::NAN).is_nan(), true);
    }
}
