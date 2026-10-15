//! 应用级生成状态（原版 `stores/generation.ts` 的 Pinia store）。
//!
//! 原版的 store 挂在应用上，比任何页面活得久：离开项目时生成继续跑，回到项目时看板接着动，
//! 生成事件也不会因为页面被重建而丢掉。这里照做——**由 [`Shell`](crate::shell::Shell) 创建、
//! 经 [`ShellHandle`](crate::shell::ShellHandle) 发给各页面**的一个 `Entity`：
//!
//! - 页面（`WorkflowPage` / `Step2`）`cx.observe` 它，页面来去都不影响它；
//! - 事件接收用 [`tasks::listen_events_in`]（无窗口变体）常驻在本实体上，store 自己过滤
//!   「这一批事件是不是我这次生成的」（[`GenerationStore::receiving`]，对应原版的
//!   `startListening` / `stopListening`）。
//!
//! 规则判断（看板条目、逐页状态、整体进度、文案表）一律调 `huanshu_core::generation_progress`，
//! 这里只做状态搬运；与 Tauri 版逐字对照。
//!
//! # 与原版 store 的差异（有意）
//!
//! 1. **一个 store 只描述一个项目**（原版也是），但打开别的项目时**不 reset**：原版的
//!    `onMounted` 会在打开任何项目时 `loadFromJson` / `reset`，把另一个项目正在跑的生成
//!    从 store 里抹掉（进度界面死掉，事件还在往一个被清空的 store 上写）。这里的
//!    [`GenerationStore::open_project`] 在 store 正为别的项目跑时**不碰 store**，那个项目
//!    按库里的数据显示（[`GenerationStore::state_for`] 的静态分支）。
//! 2. **库里停在 `running` 的旧运行按「已中断」显示**：原版 `loadLatestRun` 见到
//!    `status === 'running'` 就 `running = true` 并等事件——进程上次崩了的话它会一直转圈。
//!    这里只要 store 并没有在为这个项目跑（例如上次进程崩了），就保留阶段文案但
//!    `running = false`，看板与阶段标签走 `slide_status` 的非运行分支，并允许重新生成。
//!    **不改库**（那次运行仍留在数据库里）。
//! 3. 原版的 `completedSlides` 只在本 store 内部使用（没有任何界面消费），这里不存；
//!    原版的 `repairSlide` 无人调用，也没有移植。
//!
//! 单页重试（`retry_page`）与看板 / 详情段的消费在下一步接上，因此本文件里有一部分
//! 访问器暂时没有调用方。

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui_kit::Context;
use huanshu_core::db::GenerationRun;
use huanshu_core::events::{CoreEvent, EventSink, GenerationEvent, GenerationLogEvent};
use huanshu_core::generation_progress::{self as progress, PageProgressStatus, SlideProgressItem};
use huanshu_core::services::generation;
use huanshu_core::types::{AspectRatio, SlideKind};
use serde_json::Value;

use crate::app::AppShared;
use crate::tasks::{self, run_async_in, run_bg_in, ChannelSink};

/// 原版 store 的 `statuses`（`Map<number, PageProgressStatus>`）与 `retryingPages`（`Set`）。
type Statuses = BTreeMap<i64, PageProgressStatus>;
type PageSet = BTreeSet<i64>;

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

/// 原版 `ensureBlueprintSlot`：数组按页下标落位，前面的空位补 `null`。
fn ensure_slot(blueprints: &mut Vec<Option<Value>>, index: i64) {
    let index = index.max(0) as usize;
    while blueprints.len() <= index {
        blueprints.push(None);
    }
}

/// 一页蓝图是不是封面（原版 `blueprints[0]?.kind === 'cover'`）。
fn first_is_cover(blueprints: &[Option<Value>]) -> bool {
    blueprints
        .first()
        .and_then(|slide| slide.as_ref())
        .and_then(|slide| slide.get("kind"))
        .and_then(Value::as_str)
        == Some("cover")
}

/// 原版 `loadFromJson` 里的 `contentPages`：首张是封面时切掉前 2 张（封面、目录）与最后 1 张（结尾）。
fn content_slots(blueprints: &[Option<Value>]) -> &[Option<Value>] {
    if first_is_cover(blueprints) {
        let start = 2.min(blueprints.len());
        let end = blueprints.len().saturating_sub(1);
        if start < end {
            &blueprints[start..end]
        } else {
            &[]
        }
    } else {
        blueprints
    }
}

/// 界面读到的生成状态：store 正持有本项目时是它的实时状态，否则是库里的静态状态
/// （见模块文档「与原版 store 的差异」第 1 条）。
#[derive(Debug, Clone, PartialEq)]
pub struct GenState {
    /// 这份状态是不是 store 在实时更新。
    pub live: bool,
    /// 原版 `gen.stage`。
    pub stage: String,
    /// 原版 `gen.message`。
    pub message: String,
    /// 原版 `gen.progress`。
    pub progress: f32,
    /// 原版 `gen.running`。
    pub running: bool,
    /// 原版 `gen.retryingPages.size`。
    pub retrying: usize,
    /// 原版 `gen.slideCount`：非空蓝图数。
    pub slide_count: usize,
    /// 原版 `slideItems.length`：看板条目数（内容页数）。
    pub board_items: usize,
    /// 原版 `gen.blueprints.length > 0`（含生成中的空位）。
    pub has_blueprints: bool,
    /// 原版 `Array.from(gen.pageStatuses.values()).some(status => status.stage === 'error')`。
    pub failed_pages: bool,
}

impl GenState {
    /// 原版 `gen.stageLabel`。
    pub fn stage_label(&self) -> String {
        progress::stage_label(&self.stage).to_string()
    }
}

/// 库里读出来的静态生成状态（原版 `loadLatestRun` 的非运行分支）。
///
/// - `error` → 生成出错；
/// - `running` → **有意偏离**：那次运行并没有真的在跑（store 不在为它跑，例如上次进程崩了），
///   按「已中断」保留它当时的阶段文案、`running` 为假（看板走 `slide_status` 的非运行分支）；
/// - `done` 且有牌组 → 完成；否则等待。
pub fn stored_stage(run: Option<&GenerationRun>, deck_len: usize) -> String {
    match run.map(|run| run.status.as_str()) {
        Some("error") => "error".to_string(),
        Some("running") => stored_run_stage(run),
        Some("done") if deck_len > 0 => "done".to_string(),
        _ => "idle".to_string(),
    }
}

/// 静态状态下的阶段文案：原版 `loadLatestRun` 里 `stage = run.current_stage ?? 'start'`。
pub fn stored_run_stage(run: Option<&GenerationRun>) -> String {
    run.and_then(|run| run.current_stage.clone())
        .unwrap_or_else(|| "start".to_string())
}

/// 原版 `loadFromJson` 的 `message`。
pub fn stored_message(deck_len: usize) -> String {
    if deck_len > 0 {
        format!("已加载 {deck_len} 张幻灯片")
    } else {
        String::new()
    }
}

pub struct GenerationStore {
    shared: AppShared,
    /// 原版 `activeProjectId`：这份状态描述哪个项目。
    project_id: Option<i64>,
    stage: String,
    message: String,
    progress: f32,
    /// 原版 `blueprints`：生成中按下标落位，`None` = 还没到的空位。
    blueprints: Vec<Option<Value>>,
    page_statuses: Statuses,
    retrying_pages: PageSet,
    /// 原版 `queuedRetries`（生成中收到的重试请求排队，生成结束后依次执行）。
    queued_retries: BTreeMap<i64, Option<SlideKind>>,
    logs: Vec<GenerationLogEvent>,
    current_run: Option<GenerationRun>,
    running: bool,
    last_error: String,
    /// 原版 `generationVersion`：作废迟到的重试结果。
    version: u64,
    /// 原版 `startListening` / `stopListening` 的活跃期：只有本次生成请求在途时才收事件。
    receiving: bool,
    /// 事件通道（常驻；见模块文档）。
    sink: ChannelSink,
}

impl GenerationStore {
    pub fn new(shared: AppShared, cx: &mut Context<Self>) -> Self {
        let sink = tasks::listen_events_in(cx, |store: &mut Self, event, cx| {
            store.on_event(event, cx);
        });
        Self {
            shared,
            project_id: None,
            stage: "idle".to_string(),
            message: String::new(),
            progress: 0.0,
            blueprints: Vec::new(),
            page_statuses: Statuses::new(),
            retrying_pages: PageSet::new(),
            queued_retries: BTreeMap::new(),
            logs: Vec::new(),
            current_run: None,
            running: false,
            last_error: String::new(),
            version: 0,
            receiving: false,
            sink,
        }
    }

    // ── 只读状态 ──────────────────────────────────────────────────────────────

    pub fn project_id(&self) -> Option<i64> {
        self.project_id
    }

    pub fn stage(&self) -> &str {
        &self.stage
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn progress(&self) -> f32 {
        self.progress
    }

    pub fn blueprints(&self) -> &[Option<Value>] {
        &self.blueprints
    }

    pub fn page_statuses(&self) -> &Statuses {
        &self.page_statuses
    }

    pub fn retrying_pages(&self) -> &PageSet {
        &self.retrying_pages
    }

    pub fn logs(&self) -> &[GenerationLogEvent] {
        &self.logs
    }

    pub fn current_run(&self) -> Option<&GenerationRun> {
        self.current_run.as_ref()
    }

    pub fn running(&self) -> bool {
        self.running
    }

    pub fn last_error(&self) -> &str {
        &self.last_error
    }

    /// 原版 `gen.stageLabel`。
    pub fn stage_label(&self) -> String {
        progress::stage_label(&self.stage).to_string()
    }

    /// 原版 `gen.slideCount`：非空蓝图数。
    pub fn slide_count(&self) -> usize {
        progress::count_ready_blueprints(&self.blueprints)
    }

    /// 原版 `slideItems`：看板的条目（内容页）。
    pub fn items(&self) -> Vec<SlideProgressItem> {
        progress::slide_items(&self.blueprints, &self.logs, &self.page_statuses)
    }

    /// 原版 `canGenerate` 里的全局条件：同一时刻只有一次生成或重试（core 的
    /// `generation_running` 也这么限制）。
    pub fn busy(&self) -> bool {
        self.running || !self.retrying_pages.is_empty()
    }

    /// store 是不是正持有这个项目（只有成立时 store 的状态才描述它）。
    pub fn live_for(&self, project_id: Option<i64>) -> Option<&Self> {
        if self.project_id != project_id {
            return None;
        }
        // 还没有 id 的项目（原版 `/new`）只有在 store 真的为它跑时才算它的。
        if self.project_id.is_none() && !self.busy() {
            return None;
        }
        Some(self)
    }

    /// 界面读生成状态的入口：store 正持有本项目就用实时状态，否则用库里的静态状态。
    /// `deck` 与 `run` 是页面从库里读出来的牌组与最近一次运行。
    pub fn state_for(
        &self,
        project_id: Option<i64>,
        run: Option<&GenerationRun>,
        deck: &[Value],
    ) -> GenState {
        match self.live_for(project_id) {
            Some(live) => GenState {
                live: true,
                stage: live.stage.clone(),
                message: live.message.clone(),
                progress: live.progress,
                running: live.running,
                retrying: live.retrying_pages.len(),
                slide_count: live.slide_count(),
                board_items: live.items().len(),
                has_blueprints: !live.blueprints.is_empty(),
                failed_pages: live
                    .page_statuses
                    .values()
                    .any(|status| status.stage == "error"),
            },
            None => {
                let slots: Vec<Option<Value>> = deck.iter().cloned().map(Some).collect();
                let statuses = Statuses::new();
                GenState {
                    live: false,
                    stage: stored_stage(run, deck.len()),
                    message: stored_message(deck.len()),
                    progress: if deck.is_empty() { 0.0 } else { 1.0 },
                    running: false,
                    retrying: 0,
                    slide_count: deck.len(),
                    board_items: progress::slide_items(&slots, &[], &statuses).len(),
                    has_blueprints: !deck.is_empty(),
                    failed_pages: false,
                }
            }
        }
    }

    // ── 事件（原版 `startListening` 里那几个监听器） ──────────────────────────

    /// 事件只在本次生成请求在途时落地（对应原版 `stopListening` 之后收不到事件）。
    fn on_event(&mut self, event: CoreEvent, cx: &mut Context<Self>) {
        if !self.receiving {
            return;
        }
        match event {
            CoreEvent::Progress(event) => self.on_progress(event),
            CoreEvent::SlideReady(event) => {
                if let (Some(index), Some(blueprint)) = (event.slide_index, event.blueprint) {
                    if let Ok(slide) = serde_json::to_value(blueprint) {
                        let index = index as i64;
                        ensure_slot(&mut self.blueprints, index);
                        self.blueprints[index as usize] = Some(slide);
                    }
                }
            }
            CoreEvent::PageStatus(event) => {
                // 原版 `gen:page_status`：先写逐页状态（连续两次 error 保留最早的阶段），
                // 事件里带蓝图时同样按下标落位。
                progress::apply_page_status_event(&mut self.page_statuses, &event);
                if let Some(blueprint) = event.blueprint.as_ref() {
                    if let Ok(slide) = serde_json::to_value(blueprint) {
                        let index = event.slide_index as i64;
                        ensure_slot(&mut self.blueprints, index);
                        self.blueprints[index as usize] = Some(slide);
                    }
                }
            }
            CoreEvent::Error(event) => {
                self.stage = "error".to_string();
                self.message = event.message.clone();
                self.last_error = event.message;
                self.running = false;
                if let Some(run) = self.current_run.as_mut() {
                    run.status = "error".to_string();
                    run.current_stage = Some("error".to_string());
                    run.updated_at = now_secs();
                }
            }
            CoreEvent::Log(event) => self.on_log(event),
        }
        cx.notify();
    }

    /// 原版 `gen:progress` 监听器。
    fn on_progress(&mut self, event: GenerationEvent) {
        self.stage = event.stage.clone();
        self.message = event.message;
        if let Some(run) = self.current_run.as_mut() {
            run.status = if self.stage == "done" {
                "done"
            } else {
                "running"
            }
            .to_string();
            run.current_stage = Some(self.stage.clone());
            run.updated_at = now_secs();
        }
        if let Some(progress) = event.progress {
            self.progress = progress;
        }
    }

    /// 原版 `gen:log` 监听器。
    fn on_log(&mut self, event: GenerationLogEvent) {
        self.logs.push(event.clone());
        match self.current_run.as_mut() {
            Some(run) if run.id == event.run_id => {
                run.status = "running".to_string();
                run.current_stage = Some(event.stage);
                run.updated_at = event.created_at;
            }
            _ => {
                self.current_run = Some(GenerationRun {
                    id: event.run_id,
                    project_id: self.project_id,
                    status: "running".to_string(),
                    current_stage: Some(event.stage),
                    source_markdown: String::new(),
                    frontmatter_title: None,
                    debug_dir: String::new(),
                    created_at: event.created_at,
                    updated_at: event.created_at,
                    finished_at: None,
                });
            }
        }
    }

    // ── 生成（原版 `generate`） ──────────────────────────────────────────────

    /// 原版 `generate(mdContent, frontmatterTitle, granularity, aspectRatio, projectId)`。
    pub fn generate(
        &mut self,
        md_content: String,
        frontmatter_title: Option<String>,
        granularity: Option<String>,
        aspect_ratio: AspectRatio,
        project_id: Option<i64>,
        cx: &mut Context<Self>,
    ) {
        // 原版 `if (running.value || retryingPages.value.size > 0) return`。
        if self.busy() {
            return;
        }
        self.version += 1;
        let version = self.version;
        self.running = true;
        // 原版 `await startListening()`：这里只是打开接收（通道常驻）。
        self.receiving = true;
        self.last_error.clear();
        self.blueprints.clear();
        self.logs.clear();
        self.current_run = None;
        self.project_id = project_id;
        self.page_statuses.clear();
        self.stage = "start".to_string();
        self.message.clear();
        self.progress = 0.0;
        cx.notify();

        let shared = self.shared.clone();
        let sink: Arc<dyn EventSink> = Arc::new(self.sink.clone());
        run_async_in(
            cx,
            async move {
                // 生成前把 active project 对准本项目（core 会把结果存进当前项目）。
                if let Some(id) = project_id {
                    let _ = huanshu_core::services::projects::open_project(&shared, id);
                }
                let result = generation::generate_slides(
                    sink,
                    &shared,
                    md_content,
                    frontmatter_title,
                    granularity,
                    Some(aspect_ratio),
                )
                .await;
                // 原版紧接着 `invoke('get_blueprints')`。成功与失败都要读：
                // 失败时它是「已经组装出来的牌组」（原版只在首张是封面时采用）。
                let blueprints = generation::get_blueprints(&shared)
                    .map(|slides| {
                        slides
                            .into_iter()
                            .map(|slide| serde_json::to_value(slide).ok())
                            .collect::<Vec<Option<Value>>>()
                    })
                    .unwrap_or_default();
                (result, blueprints)
            },
            move |store, (result, blueprints), cx| {
                store.finish_generate(version, result, blueprints, cx);
            },
        );
    }

    /// 原版 `generate` 的 `try` / `catch` / `finally`。
    fn finish_generate(
        &mut self,
        version: u64,
        result: Result<usize, String>,
        final_blueprints: Vec<Option<Value>>,
        cx: &mut Context<Self>,
    ) {
        // 版本号对不上说明期间被 `reset` / `load_from_deck` 作废（换了项目）：结果不要了，
        // 但收尾照做（那些路径自己也会清这两个标志）。
        if version == self.version {
            match result {
                Ok(count) => {
                    self.blueprints = final_blueprints;
                    // 原版：失败的页保留出错状态，其余一律 done。
                    for status in self.page_statuses.values_mut() {
                        if status.stage != "error" {
                            *status = PageProgressStatus::new("done");
                        }
                    }
                    self.stage = "done".to_string();
                    self.progress = 1.0;
                    self.message = format!("生成完成，共 {count} 张幻灯片");
                }
                Err(error) => {
                    self.last_error = error.clone();
                    self.stage = "error".to_string();
                    self.message = error;
                    // 原版：最终校验可能在组装之后才失败，把已组装的牌组捡回来供单页重试。
                    if first_is_cover(&final_blueprints) {
                        self.blueprints = final_blueprints;
                    }
                }
            }
        }
        // `finally`：收尾 + 执行排队中的重试。
        self.running = false;
        self.receiving = false;
        cx.notify();
        self.drain_queued_retries(cx);
    }

    // ── 单页重试（原版 `retryPage` / `executePageRetry`） ─────────────────────

    /// 原版 `retryPage(index, layoutKind?)`。
    pub fn retry_page(
        &mut self,
        index: i64,
        layout_kind: Option<SlideKind>,
        cx: &mut Context<Self>,
    ) {
        if self.retrying_pages.contains(&index)
            || (!self.running && !self.retrying_pages.is_empty())
            || self
                .page_statuses
                .get(&index)
                .map(|status| status.stage.as_str())
                != Some("error")
        {
            return;
        }
        self.retrying_pages.insert(index);
        if self.running {
            // 生成还没结束：排队，等 `generate` 的 finally 依次执行。
            self.queued_retries.insert(index, layout_kind);
            cx.notify();
            return;
        }
        self.execute_page_retry(index, layout_kind, cx);
    }

    /// 原版 `generate` 的 finally：把排队中的重试依次执行。
    fn drain_queued_retries(&mut self, cx: &mut Context<Self>) {
        if self.running || !self.retrying_pages.is_empty() {
            return;
        }
        let Some(index) = self.queued_retries.keys().next().copied() else {
            return;
        };
        let layout_kind = self.queued_retries.remove(&index).flatten();
        self.execute_page_retry(index, layout_kind, cx);
    }

    /// 原版 `executePageRetry(index, layoutKind?)`。
    pub fn execute_page_retry(
        &mut self,
        index: i64,
        layout_kind: Option<SlideKind>,
        cx: &mut Context<Self>,
    ) {
        let version = self.version;
        let previous = self.page_statuses.get(&index).cloned();
        self.page_statuses.insert(
            index,
            PageProgressStatus {
                stage: "content".to_string(),
                message: Some("正在重试该页".to_string()),
                previous_stage: None,
                waiting: false,
            },
        );
        cx.notify();

        let Some(run) = self.current_run.clone() else {
            // 原版 `throw new Error('缺少生成记录，请重新生成')` → 走 catch 分支。
            self.page_statuses.insert(
                index,
                PageProgressStatus {
                    stage: "error".to_string(),
                    message: Some("缺少生成记录，请重新生成".to_string()),
                    previous_stage: previous
                        .and_then(|status| status.previous_stage)
                        .or_else(|| Some("content".to_string())),
                    waiting: false,
                },
            );
            if version == self.version {
                self.retrying_pages.remove(&index);
            }
            cx.notify();
            // 排队中的重试继续（原版在 `generate` 的 finally 里依次 await）。
            self.drain_queued_retries(cx);
            return;
        };

        let shared = self.shared.clone();
        let sink: Arc<dyn EventSink> = Arc::new(self.sink.clone());
        run_async_in(
            cx,
            async move {
                generation::retry_generation_page(
                    sink,
                    &shared,
                    run.id,
                    index.max(0) as usize,
                    layout_kind,
                )
                .await
            },
            move |store, result, cx| {
                store.finish_page_retry(version, index, previous, result, cx);
            },
        );
    }

    /// 原版 `executePageRetry` 的 `try` / `catch` / `finally`。
    fn finish_page_retry(
        &mut self,
        version: u64,
        index: i64,
        previous: Option<PageProgressStatus>,
        result: Result<generation::RetryPageResult, String>,
        cx: &mut Context<Self>,
    ) {
        // 原版：版本号对不上（期间换了项目 / 重新生成）时，结果整体作废。
        if version == self.version {
            match result {
                Ok(result) => {
                    let slot = index + 2;
                    ensure_slot(&mut self.blueprints, slot);
                    self.blueprints[slot as usize] = serde_json::to_value(result.slide).ok();
                    self.page_statuses.insert(
                        index,
                        PageProgressStatus {
                            stage: "done".to_string(),
                            message: Some("重试完成".to_string()),
                            previous_stage: None,
                            waiting: false,
                        },
                    );
                    if result.deck_ready {
                        self.stage = "done".to_string();
                        self.progress = 1.0;
                        self.last_error.clear();
                        let now = now_secs();
                        if let Some(run) = self.current_run.as_mut() {
                            run.status = "done".to_string();
                            run.current_stage = Some("done".to_string());
                            run.updated_at = now;
                            run.finished_at = Some(now);
                        }
                        self.message = "该页重试完成，可继续编辑".to_string();
                    } else {
                        for page in result.invalid_pages {
                            let page = page as i64;
                            if self.page_statuses.get(&page).map(|s| s.stage.as_str())
                                != Some("error")
                            {
                                self.page_statuses.insert(
                                    page,
                                    PageProgressStatus {
                                        stage: "error".to_string(),
                                        message: Some("未通过最终校验".to_string()),
                                        previous_stage: Some("content".to_string()),
                                        waiting: false,
                                    },
                                );
                            }
                        }
                        self.last_error = result.remaining.clone();
                        self.message = format!("该页已更新；{}", result.remaining);
                    }
                }
                Err(error) => {
                    self.page_statuses.insert(
                        index,
                        PageProgressStatus {
                            stage: "error".to_string(),
                            message: Some(error),
                            previous_stage: previous
                                .and_then(|status| status.previous_stage)
                                .or_else(|| Some("content".to_string())),
                            waiting: false,
                        },
                    );
                }
            }
        }
        if version == self.version {
            self.retrying_pages.remove(&index);
        }
        cx.notify();
        self.drain_queued_retries(cx);
    }

    // ── 项目切换（原版 `loadFromJson` / `loadLatestRun` / `reset`） ────────────

    /// 原版 `WorkflowView` 的 `onMounted` 顺序：先 `loadLatestRun`，没有进行中的生成时
    /// `loadFromJson`（或 `reset`）再 `loadLatestRun` 一次——日志恢复出来的失败状态要盖过
    /// `loadFromJson` 写下的「全部 done」。
    ///
    /// `done(has_active_generation)` 在 store 对齐完之后回调，页面据此决定进第几步。
    /// store 正为别的项目跑时**不碰 store**（模块文档「差异」第 1 条）。
    pub fn open_project(
        &mut self,
        project_id: i64,
        blueprints_json: &str,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, bool, &mut Context<Self>) + 'static,
    ) {
        if self.busy() && self.project_id != Some(project_id) {
            done(self, false, cx);
            return;
        }
        let shared = self.shared.clone();
        let blueprints_json = blueprints_json.to_string();
        run_bg_in(
            cx,
            move || read_run_and_logs(&shared, project_id),
            move |store, result, cx| {
                let Ok((run, logs)) = result else {
                    done(store, false, cx);
                    return;
                };
                let active = store.apply_run(project_id, run, logs);
                if !active {
                    // 原版：没有进行中的生成时先按库里的牌组铺状态，再读一次运行 / 日志。
                    if blueprints_json.trim().is_empty() {
                        store.reset_inner();
                    } else {
                        store.load_from_deck(&blueprints_json);
                    }
                    let shared = store.shared.clone();
                    run_bg_in(
                        cx,
                        move || read_run_and_logs(&shared, project_id),
                        move |store, result, cx| {
                            if let Ok((run, logs)) = result {
                                store.apply_run(project_id, run, logs);
                            }
                            done(store, active, cx);
                        },
                    );
                } else {
                    done(store, active, cx);
                }
            },
        );
    }

    /// 原版 `loadLatestRun(projectId)` 的状态部分。返回「store 真的在为本项目跑」。
    fn apply_run(
        &mut self,
        project_id: i64,
        run: Option<GenerationRun>,
        logs: Vec<GenerationLogEvent>,
    ) -> bool {
        // 进来时 store 若在忙，只可能是为本项目忙（`open_project` 已经挡掉别的项目）。
        let was_active = self.busy() && self.project_id == Some(project_id);
        self.project_id = Some(project_id);
        self.current_run = run;
        let Some(run) = self.current_run.clone() else {
            self.logs.clear();
            return false;
        };
        self.logs = logs;
        // 原版：从日志恢复失败 / 修好的页状态（后写的覆盖先写的）。
        for (index, status) in progress::statuses_from_logs(&self.logs) {
            self.page_statuses.insert(index, status);
        }
        if was_active {
            // 原版 `loadLatestRun` 的 running 分支：恢复进度、接着收事件，**不动牌组**。
            if run.status == "running" {
                self.stage = stored_run_stage(Some(&run));
                if self.message.trim().is_empty() {
                    self.message = "正在恢复生成进度...".to_string();
                }
                self.receiving = true;
            }
            return true;
        }
        if run.status == "running" {
            // 有意偏离：库里停在 running、我们却没在跑（上次进程崩了）——按「已中断」显示。
            self.running = false;
            self.stage = stored_run_stage(Some(&run));
            return false;
        }
        if !self.running {
            self.running = false;
            if run.status == "error" {
                self.stage = "error".to_string();
            } else if run.status == "done" && !self.blueprints.is_empty() {
                self.stage = "done".to_string();
            }
        }
        false
    }

    /// 原版 `loadFromJson(json)`。解析失败与原版一样静默忽略。
    pub fn load_from_deck(&mut self, json: &str) {
        let Ok(parsed) = serde_json::from_str::<Vec<Value>>(json) else {
            return;
        };
        self.version += 1;
        self.queued_retries.clear();
        self.retrying_pages.clear();
        // 原版是数组里可能有的空位（`null`）；空位与「还没到的页」一样按 `None` 存。
        self.blueprints = parsed
            .into_iter()
            .map(|slide| (!slide.is_null()).then_some(slide))
            .collect();
        self.page_statuses = content_slots(&self.blueprints)
            .iter()
            .enumerate()
            .map(|(index, _)| (index as i64, PageProgressStatus::new("done")))
            .collect();
        self.stage = "done".to_string();
        self.progress = 1.0;
        self.message = format!("已加载 {} 张幻灯片", self.blueprints.len());
    }

    /// 原版 `reset()`。
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.reset_inner();
        cx.notify();
    }

    fn reset_inner(&mut self) {
        self.version += 1;
        self.receiving = false;
        self.stage = "idle".to_string();
        self.message.clear();
        self.progress = 0.0;
        self.blueprints.clear();
        self.running = false;
        self.last_error.clear();
        self.logs.clear();
        self.current_run = None;
        self.project_id = None;
        self.page_statuses.clear();
        self.queued_retries.clear();
        self.retrying_pages.clear();
    }
}

/// 读最近一次运行与它的日志（原版 `get_latest_generation_run` + `get_generation_logs`）。
fn read_run_and_logs(
    shared: &AppShared,
    project_id: i64,
) -> Result<(Option<GenerationRun>, Vec<GenerationLogEvent>), String> {
    let run = generation::get_latest_generation_run(shared, project_id)?;
    let logs = match &run {
        Some(run) => generation::get_generation_logs(shared, run.id)?,
        None => Vec::new(),
    };
    Ok((run, logs))
}
