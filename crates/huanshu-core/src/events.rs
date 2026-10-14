//! 生成流水线的事件抽象。
//!
//! core 不认识 `tauri::AppHandle`，因此进度、逐页状态与日志都以 [`CoreEvent`] 表达，
//! 由界面层实现的 [`EventSink`] 转回各自的事件名与载荷。每个变体对应一个事件名，
//! 载荷与 Tauri 版 `app.emit(name, payload)` 的线上 JSON 完全一致——
//! `tests/fixtures/generation-events.json` 在 Tauri 版 `emit` 还在时生成，锁住了这份格式。

use serde::Serialize;
use serde_json::Value;

use crate::db::GenerationLogEntry;
use crate::types::SlideBlueprint;

/// Progress event emitted during generation
#[derive(Debug, Clone, Serialize)]
pub struct GenerationEvent {
    pub stage: String,
    pub message: String,
    pub progress: Option<f32>,
    pub slide_index: Option<usize>,
    pub blueprint: Option<SlideBlueprint>,
}

/// Per-page status event for the concurrent pipeline
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PageStatusEvent {
    pub slide_index: usize,
    pub stage: String,
    pub message: Option<String>,
    pub blueprint: Option<SlideBlueprint>,
    pub waiting: bool,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
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

/// 一条要发给界面层的事件。`wire()` 给出线上格式：`(事件名, 载荷)`。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "payload", rename_all = "snake_case")]
pub enum CoreEvent {
    Progress(GenerationEvent),
    SlideReady(GenerationEvent),
    Error(GenerationEvent),
    PageStatus(PageStatusEvent),
    Log(GenerationLogEvent),
}

impl CoreEvent {
    /// 前端 `listen()` 订阅的事件名。
    pub fn name(&self) -> &'static str {
        match self {
            CoreEvent::Progress(_) => "gen:progress",
            CoreEvent::SlideReady(_) => "gen:slide_ready",
            CoreEvent::Error(_) => "gen:error",
            CoreEvent::PageStatus(_) => "gen:page_status",
            CoreEvent::Log(_) => "gen:log",
        }
    }

    /// 与 Tauri 版 `app.emit(name, payload)` 完全相同的 `(事件名, JSON 载荷)`。
    pub fn wire(&self) -> (&'static str, Value) {
        let payload = match self {
            CoreEvent::Progress(event) | CoreEvent::SlideReady(event) | CoreEvent::Error(event) => {
                serde_json::to_value(event)
            }
            CoreEvent::PageStatus(event) => serde_json::to_value(event),
            CoreEvent::Log(event) => serde_json::to_value(event),
        }
        .expect("generation events only hold serializable data");
        (self.name(), payload)
    }
}

/// 界面层提供的发布通道（Tauri 的 `AppHandle::emit`、GPUI 自己的调度器……）。
///
/// core 只保证「这条事件要被发出去」，不关心线程、顺序或失败。
pub trait EventSink: Send + Sync {
    fn emit(&self, event: CoreEvent);
}

/// 什么都不做的通道；单页重试之类的路径不需要事件。
impl EventSink for () {
    fn emit(&self, _event: CoreEvent) {}
}

/// 并发流水线需要把通道带进每个页面任务，因此 `Arc<dyn EventSink>` 本身也是通道。
impl<T: EventSink + ?Sized> EventSink for std::sync::Arc<T> {
    fn emit(&self, event: CoreEvent) {
        (**self).emit(event);
    }
}

/// 收集事件而不是发出去，供测试断言。
#[derive(Default)]
pub struct CollectingSink(pub std::sync::Mutex<Vec<(String, Value)>>);

impl EventSink for CollectingSink {
    fn emit(&self, event: CoreEvent) {
        let (name, payload) = event.wire();
        self.0.lock().unwrap().push((name.to_string(), payload));
    }
}
