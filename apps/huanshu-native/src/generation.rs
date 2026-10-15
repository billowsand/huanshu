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
//!    按库里的数据显示（[`GenerationStore::state_for`] 的静态分支）。**静态分支的数据照样完整**：
//!    [`GenerationStore::ensure_static`] 在项目打开与第二步实体创建时按项目读一次「最近一次运行 +
//!    它的日志」并缓存（`StaticCache`），第二步据此显示失败页、关键事件与最近日志——原版这些
//!    一定在，因为它不管在给谁跑生成，总会 `loadLatestRun` 把日志装进 store。第二步的渲染只读
//!    缓存（[`GenerationStore::snapshot_for`]），不读库。
//! 2. **库里停在 `running` 的旧运行按「已中断」显示**：原版 `loadLatestRun` 见到
//!    `status === 'running'` 就 `running = true` 并等事件——进程上次崩了的话它会一直转圈。
//!    这里只要 store 并没有在为这个项目跑（例如上次进程崩了），就保留阶段文案但
//!    `running = false`，看板与阶段标签走 `slide_status` 的非运行分支，并允许重新生成。
//!    **不改库**（那次运行仍留在数据库里）。
//! 3. 原版的 `completedSlides` 只在本 store 内部使用（没有任何界面消费），这里不存；
//!    原版的 `repairSlide` 无人调用，也没有移植。
//!
//! 界面读状态的入口是 [`GenerationStore::state_for`]（工作流侧栏与初始步骤用摘要）与
//! [`GenerationStore::snapshot_for`]（第二步的看板与详情要逐页状态、日志、牌组）。

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

/// 第二步要的完整生成状态（[`GenerationStore::snapshot_for`]）：原版 `Step2Generate.vue`
/// 从 `useGenerationStore()` 读的那些字段，加上界面要的看板条目与缩略图牌组。
#[derive(Debug, Clone)]
pub struct GenSnapshot {
    /// 原版 `gen.stage`。
    pub stage: String,
    /// 原版 `gen.message`。
    pub message: String,
    /// 原版 `gen.running`。
    pub running: bool,
    /// 这份快照是不是 store 在实时更新（同 [`GenState::live`]；原版没有这个字段，它总是把
    /// store 对准当前项目）。静态分支里「重试这一页」要经 store 而 store 不持有本项目
    /// （`retry_page` 用的是它手上的运行与逐页状态，也就是**别人**的），所以界面按它禁用——
    /// 否则重试会排到别的项目的运行上。
    pub live: bool,
    /// store 正为**别的**项目跑生成 / 重试（原版没有这个字段）。全局规则「同一时刻只许一次
    /// 生成或重试」是**全应用**的（第一步的 `canGenerate` 也是），界面据此把「重新生成全部」
    /// 禁用：那条路会静默返回，点了什么都不发生（`WorkflowPage::start_generation`）。
    pub busy_elsewhere: bool,
    /// 原版 `slideItems`。
    pub items: Vec<SlideProgressItem>,
    /// 原版 `gen.pageStatuses`。
    pub statuses: Statuses,
    /// 原版 `gen.retryingPages`。
    pub retrying: PageSet,
    /// 原版 `recentLogs`（最后 10 条，倒序）。
    pub recent_logs: Vec<GenerationLogEvent>,
    /// 原版 `importantLogCount`。
    pub important: usize,
    /// 原版 `promptLogCount`。
    pub prompt_count: usize,
    /// 原版 `gen.currentRun`（单页重试要它的 id）。
    pub current_run: Option<GenerationRun>,
    /// 缩略图牌组：看板条目的蓝图（含取自日志的蓝图，空位是 `None`），下标 = 看板条目下标。
    pub thumbs: Vec<Option<Value>>,
    /// 原版「页面比例」的来源：`props.blueprints[0]`。
    pub aspect_source: Option<Value>,
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

/// 静态分支（store 不持有本项目）的完整快照：牌组来自模型、整体阶段来自库里的运行，
/// 逐页状态、看板条目、关键事件与最近日志来自 [`StaticCache`] 里的那一份
/// （原版对应 `loadFromJson` + `loadLatestRun`，它的日志与逐页状态一定在）。
///
/// 抽成自由函数有两个原因：不碰 GPUI 就能单测；[`GenerationStore::snapshot_for`] 的
/// 两条分支在这里只差数据来源。
fn static_snapshot(
    run: Option<&GenerationRun>,
    deck: &[Value],
    logs: &[GenerationLogEvent],
    statuses: Statuses,
    busy_elsewhere: bool,
) -> GenSnapshot {
    let slots: Vec<Option<Value>> = deck.iter().cloned().map(Some).collect();
    let items = progress::slide_items(&slots, logs, &statuses);
    GenSnapshot {
        stage: stored_stage(run, deck.len()),
        message: stored_message(deck.len()),
        running: false,
        live: false,
        busy_elsewhere,
        thumbs: items.iter().map(|item| item.blueprint.clone()).collect(),
        items,
        statuses,
        // 静态分支没有正在跑的重试（重试要经 store，而 store 不持有本项目）。
        retrying: PageSet::new(),
        recent_logs: progress::recent_logs(logs).into_iter().cloned().collect(),
        important: progress::important_log_count(logs),
        prompt_count: progress::prompt_log_count(logs),
        current_run: run.cloned(),
        // 原版 `props.blueprints[0]?.aspect_ratio`；静态分支的牌组就是库里的那一副。
        aspect_source: slots.first().cloned().flatten(),
    }
}

/// 静态分支（store 正为别的项目跑）要用的一份库数据：某个项目的最近一次运行、它的日志，
/// 以及从日志恢复出来的逐页状态（[`progress::statuses_from_logs`]，含失败页）。
///
/// 原版没有这份东西：它 `loadLatestRun` + `loadFromJson` 把日志与状态装进 store，代价是把
/// 另一个项目正在跑的生成从 store 里抹掉（模块文档「差异」第 1 条）。这里留着那个生成，
/// 改为按项目缓存一份只读数据。
struct StaticEntry {
    project_id: i64,
    /// 读这份数据时库里的运行 id（`None` = 那时还没有运行）。
    run_id: Option<i64>,
    statuses: Statuses,
    logs: Vec<GenerationLogEvent>,
}

/// 静态分支的缓存与读库去重（**单槽**：同时只有一个工作流页在显示，换项目就整槽换掉）。
///
/// 与 GPUI 无关的纯逻辑，便于单测。
#[derive(Default)]
struct StaticCache {
    entry: Option<StaticEntry>,
    /// 已经为哪一对 `(项目 id, 运行 id)` 排过读库。它在读库在途时挡住重复请求；结果回来之后
    /// 即使与调用方手上的运行对不上（例如模型里的 `latest_run` 还是旧的），也不再重读——
    /// 库里就是那份数据，反复读只会每帧打库。
    requested: Option<(i64, Option<i64>)>,
    /// 读库的代次：每排一次 +1。回调里对不上就说明期间又排过别的（换了项目 / 运行），整条作废。
    seq: u64,
}

impl StaticCache {
    /// 这个项目的静态数据；没有（或缓存属于别的项目）就是 `None`。
    fn entry_for(&self, project_id: Option<i64>) -> Option<&StaticEntry> {
        let project_id = project_id?;
        self.entry
            .as_ref()
            .filter(|entry| entry.project_id == project_id)
    }

    /// 要不要为这一对 `(项目 id, 运行 id)` 排一次读库：项目或运行变了、还没有数据，
    /// 且这一对没有排过。
    fn should_read(&self, project_id: i64, run_id: Option<i64>) -> bool {
        if self.requested == Some((project_id, run_id)) {
            return false;
        }
        self.entry_for(Some(project_id))
            .map(|entry| entry.run_id != run_id)
            .unwrap_or(true)
    }

    /// 排一次读库，返回这次的代次（结果回来时用它比对身份）。
    fn begin_read(&mut self, project_id: i64, run_id: Option<i64>) -> u64 {
        self.seq += 1;
        self.requested = Some((project_id, run_id));
        self.seq
    }

    /// 读回来的数据落进缓存；代次对不上（期间排过别的读，例如打开了别的项目）时整条丢弃，
    /// 返回有没有落进去。
    fn put(&mut self, seq: u64, entry: StaticEntry) -> bool {
        if seq != self.seq {
            return false;
        }
        self.entry = Some(entry);
        true
    }

    /// store 接手这个项目（打开 / 开始生成 / 重试）时丢掉它的静态缓存：那时本项目的数据
    /// 由 store 实时持有，旧的一份只会过期。
    fn invalidate(&mut self, project_id: i64) {
        if self
            .entry
            .as_ref()
            .is_some_and(|entry| entry.project_id == project_id)
        {
            self.entry = None;
            self.requested = None;
        }
    }

    fn clear(&mut self) {
        self.entry = None;
        self.requested = None;
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
    /// 有一次单页重试正在执行（`retrying_pages` 里还含排队的页，不能用它判断）。
    retry_in_flight: bool,
    logs: Vec<GenerationLogEvent>,
    current_run: Option<GenerationRun>,
    running: bool,
    last_error: String,
    /// 原版 `generationVersion`：作废迟到的重试结果。
    version: u64,
    /// 原版 `startListening` / `stopListening` 的活跃期：只有本次生成请求在途时才收事件。
    receiving: bool,
    /// 静态分支（store 不持有本项目）的库数据缓存，见 [`Self::ensure_static`]。
    static_cache: StaticCache,
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
            retry_in_flight: false,
            logs: Vec::new(),
            current_run: None,
            running: false,
            last_error: String::new(),
            version: 0,
            receiving: false,
            static_cache: StaticCache::default(),
            sink,
        }
    }

    // ── 只读状态 ──────────────────────────────────────────────────────────────

    /// 原版 `gen.blueprints`（工作流把牌组镜像进 `ProjectModel.deck`，第三步用它）。
    /// 第三步的编辑不直接改它，走 [`Self::set_deck`]。
    pub fn blueprints(&self) -> &[Option<Value>] {
        &self.blueprints
    }

    /// 生成用的共享状态（「选择重试布局」对话框读 `get_retry_layouts` 要用）。
    pub fn shared(&self) -> &AppShared {
        &self.shared
    }

    /// 原版 `gen.slideCount`：非空蓝图数。
    fn slide_count(&self) -> usize {
        progress::count_ready_blueprints(&self.blueprints)
    }

    /// 原版 `slideItems`：看板的条目（内容页）。
    fn items(&self) -> Vec<SlideProgressItem> {
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
                // 日志里恢复出来的失败页也算数（原版从日志装 pageStatuses，`hasFailedPages`
                // 会挡住「完成后 800ms 自动进第 3 步」）：静态分支的摘要因此与第二步一致。
                // 有失败页时看板条目也按日志算（规划 / 布局结果来自日志）。
                let cached = self.static_cache.entry_for(project_id);
                let statuses = cached
                    .map(|entry| entry.statuses.clone())
                    .unwrap_or_default();
                let logs = cached.map(|entry| entry.logs.as_slice()).unwrap_or(&[]);
                GenState {
                    live: false,
                    stage: stored_stage(run, deck.len()),
                    message: stored_message(deck.len()),
                    progress: if deck.is_empty() { 0.0 } else { 1.0 },
                    running: false,
                    retrying: 0,
                    slide_count: deck.len(),
                    board_items: progress::slide_items(&slots, logs, &statuses).len(),
                    has_blueprints: !deck.is_empty(),
                    failed_pages: statuses.values().any(|status| status.stage == "error"),
                }
            }
        }
    }

    /// 第二步（看板与详情）读生成状态的入口：与 [`Self::state_for`] 判同一套
    /// 「store 是不是正持有本项目」，但把逐页状态、日志、牌组一并交出来
    /// （原版 `Step2Generate.vue` 的 props 全部来自 `useGenerationStore()`）。
    ///
    /// 静态分支的数据来自 [`Self::ensure_static`] 缓存的那一份（这里**只读缓存、不读库**）；
    /// 缓存还没到或与调用方手上的运行对不上时先按现有的数据显示，下一次
    /// [`Self::ensure_static`] 会给上新的。
    pub fn snapshot_for(
        &self,
        project_id: Option<i64>,
        run: Option<&GenerationRun>,
        deck: &[Value],
    ) -> GenSnapshot {
        match self.live_for(project_id) {
            Some(live) => {
                let items = live.items();
                GenSnapshot {
                    stage: live.stage.clone(),
                    message: live.message.clone(),
                    running: live.running,
                    live: true,
                    busy_elsewhere: false,
                    thumbs: items.iter().map(|item| item.blueprint.clone()).collect(),
                    items,
                    statuses: live.page_statuses.clone(),
                    retrying: live.retrying_pages.clone(),
                    recent_logs: progress::recent_logs(&live.logs)
                        .into_iter()
                        .cloned()
                        .collect(),
                    important: progress::important_log_count(&live.logs),
                    prompt_count: progress::prompt_log_count(&live.logs),
                    current_run: live.current_run.clone(),
                    aspect_source: live.blueprints.first().cloned().flatten(),
                }
            }
            None => {
                let cached = self.static_cache.entry_for(project_id);
                static_snapshot(
                    run,
                    deck,
                    cached.map(|entry| entry.logs.as_slice()).unwrap_or(&[]),
                    cached
                        .map(|entry| entry.statuses.clone())
                        .unwrap_or_default(),
                    // 到这里 store 一定不持有本项目，`busy()` 只可能是为别的项目忙。
                    self.busy(),
                )
            }
        }
    }

    /// 确保静态分支（[`Self::snapshot_for`] 的 `None` 分支）的数据按这个项目读过一次：
    /// 读库由这里排到后台（`read_run_and_logs`），第二步的渲染只读缓存。
    ///
    /// 调用点：项目打开（[`Self::open_project`] 的忙早退分支）与第二步实体创建
    /// （`pages/workflow/step2.rs::Step2::new`）。项目或运行变了（`run.id` 不同）才算失效，
    /// 同一对 `(项目, 运行)` 只读一次；还没落库的项目（`/new`）库里没有它的运行，不读。
    ///
    /// store 正持有本项目时，这一步读出来的数据暂时用不上（第二步读的是实时状态）；照样读
    /// 是为了它后来接手别的项目、本项目落到静态分支时缓存已经在了——同一对只读一次，
    /// 代价就一次查询。
    pub fn ensure_static(
        &mut self,
        project_id: Option<i64>,
        run: Option<&GenerationRun>,
        cx: &mut Context<Self>,
    ) {
        let Some(project_id) = project_id else {
            return;
        };
        let run_id = run.map(|run| run.id);
        if !self.static_cache.should_read(project_id, run_id) {
            return;
        }
        let seq = self.static_cache.begin_read(project_id, run_id);
        let shared = self.shared.clone();
        run_bg_in(
            cx,
            move || read_run_and_logs(&shared, project_id),
            move |store, result, cx| {
                // 读失败（项目被删了之类）当作没有数据；迟到的结果由代次作废。
                let Ok((run, logs)) = result else {
                    return;
                };
                let entry = StaticEntry {
                    project_id,
                    run_id: run.as_ref().map(|run| run.id),
                    statuses: progress::statuses_from_logs(&logs),
                    logs,
                };
                if !store.static_cache.put(seq, entry) {
                    return;
                }
                cx.notify();
            },
        );
    }

    // ── 第三步的写入口（原版 `useSlideEditor` 直接改 `gen.blueprints`） ────────

    /// 第三步唯一的牌组写入口（原版 `useSlideEditor` 改完 `gen.blueprints` 再整副存库）。
    ///
    /// 只有 store **正持有这个项目**（[`Self::live_for`]）且**不在生成中**（生成与重试都不行）
    /// 时才写：store 正为别的项目跑时它不描述本项目（写进去是污染别人的牌组），本项目自己在
    /// 生成时牌组里还有空位（下标对不上，写进去会错位）。
    ///
    /// 条件不满足返回 `false`，**调用方照常改模型**——store 不描述本项目时模型就是唯一状态
    /// （见 `WorkflowPage::commit_deck`）。
    pub fn set_deck(
        &mut self,
        project_id: Option<i64>,
        deck: Vec<Value>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.live_for(project_id).is_none() || self.busy() {
            return false;
        }
        self.blueprints = deck.into_iter().map(Some).collect();
        cx.notify();
        true
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
        // 这个项目的数据从此由 store 实时持有：静态缓存里那一份（上一次运行）现在过期了。
        if let Some(id) = project_id {
            self.static_cache.invalidate(id);
        }
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
                // 原版紧接着 `invoke('get_blueprints')`：这里换成**按发起生成的项目**取牌组
                // （core 的 `get_generation_deck`），不看「当前打开的项目」——生成中用户可能
                // 已经打开了别的项目，那时 `get_blueprints` 拿到的是别人的牌组。
                // 成功与失败两条路都要读：失败时它是「已经组装出来的牌组」（原版只在首张是
                // 封面时采用）；运行没到达过组装时 core 返回空，库里上一次的旧牌组不会被当成
                // 这次的结果。也不等事件里的运行 id——事件可能比这个结果晚到。
                let blueprints = match project_id {
                    Some(id) => generation::get_generation_deck(&shared, id),
                    // 还没落库的项目（原版 `/new` 的 `activeProjectId` 为 null）：退回当前牌组。
                    None => generation::get_blueprints(&shared),
                }
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
    ///
    /// 前置条件不满足时原版是静默 `return`；这里把原因交回调用方——「选择重试布局」对话框
    /// 据此给用户一个轻提示，否则点「按此布局重试」只关框、什么都没发生。
    /// 重试被 core 拒绝（`executePageRetry` 里的失败）不在其中：那条路会把原因写进该页状态，
    /// 第二步的失败段与看板都能看到。
    pub fn retry_page(
        &mut self,
        index: i64,
        layout_kind: Option<SlideKind>,
        cx: &mut Context<Self>,
    ) -> Result<(), &'static str> {
        if self.retrying_pages.contains(&index) {
            return Err("这一页正在重试");
        }
        // 原版：不在生成中且已经有页在重试时不许再排（`!running && retryingPages.size > 0`）。
        if !self.running && !self.retrying_pages.is_empty() {
            return Err("已有页面正在重试，请等它完成");
        }
        if self
            .page_statuses
            .get(&index)
            .map(|status| status.stage.as_str())
            != Some("error")
        {
            return Err("这一页不是生成失败的页，无需重试");
        }
        self.retrying_pages.insert(index);
        if self.running {
            // 生成还没结束：排队，等 `generate` 的 finally 依次执行。
            self.queued_retries.insert(index, layout_kind);
            cx.notify();
            return Ok(());
        }
        self.execute_page_retry(index, layout_kind, cx);
        Ok(())
    }

    /// 原版 `generate` 的 finally：把排队中的重试依次执行（一次一个）。
    ///
    /// 排队的页已经在 `retrying_pages` 里（界面据此显示「已排队」），所以「还有重试在跑」
    /// 要看 `retry_in_flight`，不能看 `retrying_pages` 是否为空——否则排队的重试永远轮不到，
    /// `busy()` 一直为真，全应用再也不能生成。
    fn drain_queued_retries(&mut self, cx: &mut Context<Self>) {
        if self.running || self.retry_in_flight {
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

        self.retry_in_flight = true;
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
        self.retry_in_flight = false;
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
    /// store 正为别的项目跑时**不碰 store**（模块文档「差异」第 1 条），但**照样读一次**这个
    /// 项目的最近一次运行 + 日志并缓存（[`Self::ensure_static`]），第二步据此显示失败页与日志。
    pub fn open_project(
        &mut self,
        project_id: i64,
        run: Option<&GenerationRun>,
        blueprints_json: &str,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, bool, &mut Context<Self>) + 'static,
    ) {
        if self.busy() && self.project_id != Some(project_id) {
            // store 正为别的项目跑：不碰 store，直接按静态状态回调。
            //
            // **不能在这里同步回调**：调用方是 `WorkflowPage::load_project`，此刻正持着页面的
            // 租约（`page.generation.update(...)` 在 `page.update(...)` 里面），回调里再
            // `page.update()` 会撞上「already being updated」——release 构建下就是生成中回首页、
            // 再点别的项目卡片时的 0xc0000409 崩溃。推到本次 effect 跑完（租约已还）再回调。
            // （`ensure_static` 只是排一个后台读，不在这次调用的同步路径上。）
            self.ensure_static(Some(project_id), run, cx);
            let store = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = store.update(cx, |store, cx| done(store, false, cx));
            });
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
        // store 从此描述这个项目：静态分支不会再用到它的缓存，旧的那一份直接丢掉。
        self.static_cache.invalidate(project_id);
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
        // store 不再描述任何项目：静态缓存也一起清掉（下次打开项目再读）。
        self.static_cache.clear();
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 一条日志，只填断言用得到的字段（title / detail 另有断言时自己改）。
    #[allow(clippy::too_many_arguments)]
    fn log(
        run_id: i64,
        seq: i64,
        slide_index: Option<i32>,
        stage: &str,
        kind: &str,
        summary: &str,
        important: bool,
    ) -> GenerationLogEvent {
        GenerationLogEvent {
            id: seq,
            run_id,
            seq,
            slide_index,
            stage: stage.to_string(),
            kind: kind.to_string(),
            title: String::new(),
            summary: summary.to_string(),
            detail: json!({}),
            important,
            created_at: 1_700_000_000 + seq,
        }
    }

    fn a_run(id: i64, status: &str) -> GenerationRun {
        GenerationRun {
            id,
            project_id: Some(8),
            status: status.to_string(),
            current_stage: Some("validate".to_string()),
            source_markdown: String::new(),
            frontmatter_title: None,
            debug_dir: String::new(),
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
            finished_at: None,
        }
    }

    /// 「生成记录演示」那样的牌组：封面 + 内容页 + 结尾（首张是封面，`content_slots` 会切掉
    /// 前两张与最后一张）。
    fn a_deck(content: usize) -> Vec<Value> {
        let mut deck = vec![
            json!({"kind": "cover", "title": "园区能耗治理方案"}),
            json!({"kind": "overview", "title": "提纲总览"}),
        ];
        for index in 0..content {
            deck.push(json!({"kind": "section_list", "title": format!("第 {} 页", index + 1)}));
        }
        deck.push(json!({"kind": "closing", "title": "感谢聆听"}));
        deck
    }

    fn an_entry(project_id: i64, run_id: i64, logs: Vec<GenerationLogEvent>) -> StaticEntry {
        StaticEntry {
            project_id,
            run_id: Some(run_id),
            statuses: progress::statuses_from_logs(&logs),
            logs,
        }
    }

    /// 库里读回来的那一次失败运行（10 条日志，第 3 页即下标 2 失败）——与「生成记录演示」
    /// 的演示数据同形（`services::seed`）：整副规划 / 布局结果在一条日志的 `detail` 里，
    /// 成功页各有一条 `content` / `slide` 日志，失败页是一条 `content` / `page_error`。
    fn demo_logs(run_id: i64) -> Vec<GenerationLogEvent> {
        let page_plans: Vec<Value> = (0..5)
            .map(|page| json!({"page_title": format!("第 {} 页", page + 1)}))
            .collect();
        let layout_plans: Vec<Value> = (0..5).map(|_| json!({"kind": "section_list"})).collect();
        let mut logs = vec![
            log(run_id, 1, None, "start", "status", "开始生成流程", false),
            log(run_id, 2, None, "start", "input", "本次生成输入", true),
            log(
                run_id,
                3,
                None,
                "init",
                "status",
                "验证大模型连接...",
                false,
            ),
        ];
        let mut plans = log(
            run_id,
            4,
            None,
            "layout_plan",
            "status",
            "布局规划完成",
            true,
        );
        plans.detail = json!({"page_plans": page_plans, "layout_plans": layout_plans});
        logs.push(plans);
        for page in 0..5i32 {
            if page == 2 {
                let mut failed = log(
                    run_id,
                    20,
                    Some(page),
                    "content",
                    "page_error",
                    "最终校验未通过：第 3 页缺少时间线事件",
                    true,
                );
                failed.title = "页面生成失败".to_string();
                logs.push(failed);
                continue;
            }
            let mut content = log(
                run_id,
                10 + page as i64,
                Some(page),
                "content",
                "slide",
                "生成内容",
                true,
            );
            content.title = format!("幻灯片 {}/5", page + 1);
            content.detail = json!({
                "page_plan": {"page_title": format!("第 {} 页", page + 1)},
                "layout_plan": {"kind": "section_list"},
                "blueprint": {"kind": "section_list", "title": format!("第 {} 页", page + 1)},
            });
            logs.push(content);
        }
        logs.push(log(run_id, 21, None, "done", "status", "生成结束", true));
        logs
    }

    #[test]
    fn the_static_branch_restores_failed_pages_logs_and_board_items() {
        let logs = demo_logs(7);
        let run = a_run(7, "error");
        let deck = a_deck(5);
        let statuses = progress::statuses_from_logs(&logs);
        let snap = static_snapshot(Some(&run), &deck, &logs, statuses, true);

        // 整体阶段来自库里的运行；静态分支不在跑，但别的项目正在跑。
        assert_eq!(snap.stage, "error");
        assert!(!snap.running);
        assert!(!snap.live);
        assert!(snap.busy_elsewhere);
        assert_eq!(snap.message, "已加载 8 张幻灯片");
        // 失败页从日志恢复出来了（失败段与「重试这一页」就靠它）。
        assert_eq!(
            snap.statuses.get(&2).map(|status| status.stage.as_str()),
            Some("error")
        );
        assert_eq!(
            snap.statuses
                .get(&2)
                .and_then(|status| status.message.clone()),
            Some("最终校验未通过：第 3 页缺少时间线事件".to_string())
        );
        // 看板条目：五页内容页（规划 / 布局 / 蓝图都从日志里取回来）。
        assert_eq!(snap.items.len(), 5);
        assert_eq!(
            snap.items[2].layout_plan.as_ref().unwrap()["kind"],
            "section_list"
        );
        assert!(snap.items.iter().all(|item| item.blueprint.is_some()));
        assert_eq!(snap.thumbs.len(), snap.items.len());
        // 头部统计与「查看最近日志」的数据（10 条日志里 8 条关键事件、5 条算生成记录）。
        assert_eq!(snap.important, 8);
        assert_eq!(snap.prompt_count, 5);
        assert_eq!(snap.recent_logs.len(), 10);
        assert_eq!(snap.recent_logs.first().map(|log| log.seq), Some(21));
        // 倒序：最旧的一条在最后（10 条全在里面）。
        assert_eq!(snap.recent_logs.last().map(|log| log.seq), Some(1));
        // 单页重试要运行 id。
        assert_eq!(snap.current_run.as_ref().map(|run| run.id), Some(7));
        // 页面比例取牌组第一张（封面）。
        assert_eq!(snap.aspect_source.as_ref().unwrap()["kind"], "cover");
    }

    #[test]
    fn the_static_branch_picks_up_a_succeeded_retry_in_the_logs() {
        let mut logs = demo_logs(7);
        let mut retry = log(7, 22, Some(2), "validate", "retry", "重试完成", true);
        retry.detail = json!({"retry_succeeded": true});
        logs.push(retry);
        let run = a_run(7, "error");
        let snap = static_snapshot(
            Some(&run),
            &a_deck(5),
            &logs,
            progress::statuses_from_logs(&logs),
            true,
        );
        // 后写的 retry 日志盖掉 page_error：这一页不再是失败页。
        assert_eq!(
            snap.statuses.get(&2).map(|status| status.stage.as_str()),
            Some("done")
        );
    }

    #[test]
    fn the_static_branch_without_cached_logs_shows_no_failures() {
        // 缓存还没到（或项目没有运行）：与旧行为一致，只有牌组与运行决定显示。
        let snap = static_snapshot(
            Some(&a_run(7, "error")),
            &a_deck(5),
            &[],
            Statuses::new(),
            false,
        );
        assert_eq!(snap.stage, "error");
        assert!(!snap.live);
        assert!(!snap.busy_elsewhere);
        assert!(snap.statuses.is_empty());
        assert!(snap.recent_logs.is_empty());
        assert_eq!(snap.important, 0);
        assert_eq!(snap.prompt_count, 0);
        assert_eq!(snap.items.len(), 5);
        // 库里停在 running 的旧运行按「已中断」显示（有意偏离，`stored_stage`）。
        let interrupted = static_snapshot(
            Some(&a_run(7, "running")),
            &a_deck(5),
            &[],
            Statuses::new(),
            false,
        );
        assert_eq!(interrupted.stage, "validate");
        assert!(!interrupted.running);
    }

    #[test]
    fn the_static_cache_reads_once_per_project_and_run() {
        let mut cache = StaticCache::default();
        assert!(cache.should_read(8, Some(1)));
        let seq = cache.begin_read(8, Some(1));
        // 读库在途：同一对不重复排。
        assert!(!cache.should_read(8, Some(1)));
        assert!(cache.put(seq, an_entry(8, 1, demo_logs(1))));
        // 已经有这一对的数据：不重读。
        assert!(!cache.should_read(8, Some(1)));
        assert_eq!(cache.entry_for(Some(8)).map(|e| e.logs.len()), Some(10));
        // 缓存按项目分开：别的项目（或没有项目）读不到它。
        assert!(cache.entry_for(Some(9)).is_none());
        assert!(cache.entry_for(None).is_none());
        assert!(cache.should_read(9, Some(3)));
    }

    #[test]
    fn the_static_cache_drops_late_results_and_re_reads_after_a_change() {
        let mut cache = StaticCache::default();
        let first = cache.begin_read(8, Some(1));
        // 期间又打开了别的项目（换了项目与运行）：第一次的结果迟到，整条作废。
        let second = cache.begin_read(9, Some(3));
        assert!(!cache.put(first, an_entry(8, 1, demo_logs(1))));
        assert!(cache.put(second, an_entry(9, 3, demo_logs(3))));
        assert!(cache.entry_for(Some(8)).is_none());
        assert_eq!(cache.entry_for(Some(9)).map(|e| e.run_id), Some(Some(3)));
        // 同一个项目、运行变了（又生成过一次）：要重读。
        assert!(cache.should_read(9, Some(4)));
        // 已经为这一对读过：不再重读（前面那次 `should_read` 不改变状态）。
        assert!(!cache.should_read(9, Some(3)));
        // store 接手这个项目（实时状态）时缓存失效，下次要重新读。
        cache.invalidate(9);
        assert!(cache.entry_for(Some(9)).is_none());
        assert!(cache.should_read(9, Some(3)));
    }

    #[test]
    fn the_static_cache_keeps_data_when_the_model_run_lags_the_database() {
        // 打开项目时模型读到的运行与 store 那次读回来的可能不是同一对（模型手上是旧的、
        // 甚至还没有）。已经为这一对排过读、数据也落了地，就不再打库——否则每帧一次查询。
        let mut cache = StaticCache::default();
        let seq = cache.begin_read(8, None);
        assert!(cache.put(seq, an_entry(8, 1, demo_logs(1))));
        assert!(!cache.should_read(8, None));
        assert_eq!(cache.entry_for(Some(8)).map(|e| e.run_id), Some(Some(1)));
        // 模型手上的运行正好是库里那条：也不必重读。
        assert!(!cache.should_read(8, Some(1)));
    }
}
