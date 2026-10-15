//! 项目工作流（原版 `WorkflowView.vue`）：左侧边栏 + 三步内容。
//!
//! 项目数据放在一个共享实体 [`ProjectModel`] 里，三步页面都持有它、`cx.observe` 它：
//! Step1 改文稿、Step2 生成、Step3 改蓝图，都改这一个模型，侧栏与其他步骤自动更新
//! （原版是 Pinia 的 `generation` / `projects` store 加一堆 props）。
//!
//! **生成状态不在这里**：它是应用级的 [`GenerationStore`]（原版 pinia 的 `generation` store，
//! 由 `Shell` 创建、经 `ShellHandle` 发给页面）。本页只观察它、按原版规则显示
//! （侧栏说明、初始步骤、自动跳页）；`ProjectModel.deck` 跟着 store 的牌组走，第三步用它。
//!
//! **牌组只有一条写入路径**：第三步的编辑一律经 [`WorkflowPage::commit_deck`]（模型 → store 的
//! 写入口 → 落库），`WorkflowPage`、`ProjectModel`、store 三处不会各改各的。**本项目**正在
//! 生成 / 重试时（[`WorkflowPage::editing_locked`]）第三步只读；别的项目在生成不影响本项目的编辑。
//!
//! 已实现：加载 / 加载失败、侧栏（返回、项目名与改名、三步状态与说明、主题圆点、设置）、
//! 按原版规则选初始步骤与判断能否点击、步骤切换时交还位图、第一步草稿防抖自动保存
//! （含新建项目落库与路由对准）、项目附加信息读写、生成状态显示与自动跳页。
//! 待实现：第 2 步的整体进度 / 看板逐页状态 / 单页重试（见 step2），都在下一轮。

pub mod step1;
pub mod step2;
pub mod step3;

use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::*;
use gpui_kit::{
    div, px, AnyElement, App, Context, Entity, Focusable as _, Render, SharedString, Subscription,
    Task, Window,
};
use huanshu_core::db::GenerationRun;
use huanshu_core::markdown_tools::{self, Granularity};
use huanshu_core::services::generation;
use huanshu_core::services::media::MediaAssetResponse;
use huanshu_core::services::projects;
use huanshu_core::services::projects::Step1ProjectMeta;
use huanshu_core::types::AspectRatio;

use crate::generation::{GenState, GenerationStore};
use crate::icons::carbon;
use crate::router::{Route, SettingsTab, Step};
use crate::shell::ShellHandle;
use crate::tasks::{run_async, run_bg};
use crate::theme::StudioExt as _;
use crate::widgets;
use crate::widgets::TooltipText as _;

/// 工作流顶栏的统一高度：侧栏的「← 项目」、第一步的工具栏、第二步的状态头、第三步的三个栏头
/// 都是这个高度，左右连成一条线。
pub(super) const HEADER_H: f32 = 64.0;

/// 一个项目在工作流里的全部数据。三步页面共享它。
///
/// 生成状态（阶段、进度、逐页状态、日志）**不在这里**：它在应用级的
/// [`GenerationStore`] 里（原版 `stores/generation.ts`），本页只读它、把牌组镜像进
/// [`ProjectModel::deck`]。
pub struct ProjectModel {
    /// `None` = 新建、尚未落库（原版 `/new`）。
    pub id: Option<i64>,
    pub name: String,
    pub md_content: String,
    /// 上传的文稿文件名（原版 `step1State.mdFilename`，顶栏标签与侧栏说明会显示）。
    pub md_filename: String,
    /// 分页粒度（原版 `step1State.granularity`）。
    pub granularity: Granularity,
    /// 素材库（原版 `mediaLib.mediaItems`，`services::media` 的记录）。
    pub media: Vec<MediaAssetResponse>,
    /// 草稿自动保存状态（原版 `draftSaveState`；防抖保存逻辑在 C 节，这里只存状态与展示）。
    pub draft_save_state: DraftSaveState,
    /// 草稿保存失败原因（原版 `draftSaveError`，格式工具行内错误会显示）。
    pub draft_save_error: String,
    /// 牌组按**原始 JSON** 保存（与 core 的导出、预览一致；强类型往返会挡掉能渲染的牌组）。
    /// 由 [`GenerationStore`] 的牌组镜像而来（生成 / 重试改了牌组就跟着更新，第三步用它）。
    /// 第三步的编辑只经 [`WorkflowPage::commit_deck`] 写它。
    pub deck: Vec<serde_json::Value>,
    /// 第三步当前编辑的页（原版 `WorkflowView.vue` 的 `activeSlide`）：切到别的步骤再回来、
    /// 打开演示都用它。牌组变短时收敛到最后一页。
    pub active_slide: usize,
    /// 第二步看板上选中的页（原版 `WorkflowView.vue:41` 的 `selectedSlideIndex`，`number | null`）：
    /// 与 [`Self::active_slide`] 一样挂在模型上，第二步每次重建（[`WorkflowPage::show_step`]）
    /// 都从这里读，切到别的步骤再回来还是同一页。`None` = 还没选过，由第二步按看板条目
    /// `preferred_slide_index` 归位（归位结果也写回这里，照原版 watch 的 emit）。
    /// 每次「开始 AI 生成」（[`WorkflowPage::start_generation`]）清空。
    pub selected_slide: Option<usize>,
    /// 牌组正在落库（原版 `useSlideEditor.editorSaving`；[`WorkflowPage::commit_deck`] 的
    /// 每条出口都会复位它）。
    // 第三步的保存按钮读它显示「保存中…」并禁用。
    pub deck_saving: bool,
    /// 牌组落库失败的原因（原版 `useSlideEditor.jsonError`：第三步保存失败时按钮下方显示它）。
    pub deck_save_error: Option<String>,
    pub aspect: AspectRatio,
    /// 库里最近一次生成运行（原版 `loadLatestRun` 读回来的那条；store 正为**别的**项目
    /// 跑时，界面的静态状态由它和牌组决定，见 [`GenerationStore::state_for`]）。
    pub latest_run: Option<GenerationRun>,
}

/// 草稿自动保存状态（原版 `'idle' | 'saving' | 'saved' | 'error'`）。
/// Saving / Saved / Error 由 C 节的自动保存逻辑置位，本轮只有展示。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum DraftSaveState {
    #[default]
    Idle,
    Saving,
    Saved,
    Error,
}

impl DraftSaveState {
    /// 原版 `draftStatusText`。
    pub fn text(self) -> &'static str {
        match self {
            Self::Idle => "",
            Self::Saving => "草稿保存中...",
            Self::Saved => "草稿已保存",
            Self::Error => "草稿保存失败",
        }
    }
}

impl ProjectModel {
    /// 页面高宽比（16:9 → 0.5625）。
    pub fn aspect_ratio(&self) -> f32 {
        self.aspect.height() as f32 / self.aspect.width() as f32
    }

    /// 文稿长度（原版 `mdContent.length`，JS 的 UTF-16 码元数；中文每字 1，与原版一致）。
    pub fn md_len(&self) -> usize {
        self.md_content.encode_utf16().count()
    }

    /// 原版 `detectedGranularityLocal`：自动推荐解析成 H2 / H3。
    pub fn detected_granularity(&self) -> Granularity {
        markdown_tools::detect_granularity(&self.md_content)
    }

    /// 原版 `estimatedSlideCount`。
    pub fn estimated_slides(&self) -> usize {
        markdown_tools::estimate_slides(&self.md_content, self.granularity)
    }

    /// 「开始 AI 生成」可用条件（原版 `canGenerate`）：文稿与名称非空，且没有生成或
    /// 重试在跑（`gen.running` / `gen.retryingPages` 是**全应用**的条件：同一时刻只有
    /// 一次生成，别的项目在跑时这一步也不可用）。
    pub fn can_generate_local(&self, generation_busy: bool) -> bool {
        !self.md_content.trim().is_empty() && !self.name.trim().is_empty() && !generation_busy
    }
}

enum Load {
    Loading,
    Failed(String),
    Ready,
}

enum StepView {
    Prepare(Entity<step1::Step1>),
    Generate(Entity<step2::Step2>),
    Edit(Entity<step3::Step3>),
}

/// 第一步草稿的内容指纹（原版对 `step1State` 的 deep watch）：任何一个字段变了
/// 都触发自动保存；落库状态机（draft_save_state）的变化不影响指纹，不会引发循环。
#[derive(Debug, PartialEq)]
struct DraftSnapshot {
    name: String,
    md: String,
    md_filename: String,
    granularity: Granularity,
    aspect: AspectRatio,
}

impl DraftSnapshot {
    fn capture(model: &ProjectModel) -> Self {
        Self {
            name: model.name.clone(),
            md: model.md_content.clone(),
            md_filename: model.md_filename.clone(),
            granularity: model.granularity,
            aspect: model.aspect,
        }
    }
}

/// 比例的存储字符串（`Step1ProjectMeta.aspect_ratio` 用）。
pub(super) fn aspect_str(aspect: AspectRatio) -> &'static str {
    match aspect {
        AspectRatio::Ratio16x9 => "ratio_16x9",
        AspectRatio::Ratio32x9 => "ratio_32x9",
        AspectRatio::Ratio48x9 => "ratio_48x9",
    }
}

pub(super) fn parse_aspect(text: &str) -> Option<AspectRatio> {
    match text {
        "ratio_16x9" => Some(AspectRatio::Ratio16x9),
        "ratio_32x9" => Some(AspectRatio::Ratio32x9),
        "ratio_48x9" => Some(AspectRatio::Ratio48x9),
        _ => None,
    }
}

/// 去掉扩展名的文件名（原版 `filename.replace(/\.[^.]+$/, '')`）。
fn strip_extension(name: &str) -> String {
    match name.rfind('.') {
        Some(i) if i > 0 => name[..i].to_string(),
        _ => name.to_string(),
    }
}

/// 原版 `getDraftProjectName`。
fn draft_project_name(model: &ProjectModel) -> String {
    let name = model.name.trim();
    if !name.is_empty() {
        return name.to_string();
    }
    let filename = model.md_filename.trim();
    if !filename.is_empty() {
        return strip_extension(filename);
    }
    "未命名演示".to_string()
}

/// 原版 `hasMeaningfulStep1Draft`。
fn has_meaningful_draft(model: &ProjectModel) -> bool {
    !model.md_content.trim().is_empty()
        || !model.name.trim().is_empty()
        || !model.md_filename.trim().is_empty()
}

pub struct WorkflowPage {
    handle: ShellHandle,
    model: Entity<ProjectModel>,
    /// 应用级生成状态（原版 `gen`）。页面只读它；它比本页活得久。
    generation: Entity<GenerationStore>,
    load: Load,
    step: Step,
    view: Option<StepView>,
    requested_step: Option<Step>,
    /// 侧栏改名（原版 `editingName` / `newName`）。
    editing_name: bool,
    rename: Entity<gpui_kit::component::input::InputState>,
    /// 改名输入框请求提交（Enter / 失焦在订阅里没有 window，推迟到 render 提交）。
    rename_commit: bool,
    /// 初始加载完成（原版 `step1Bootstrapped`）：置真前不打自动保存。
    step1_bootstrapped: bool,
    /// 草稿防抖计时任务（原版 `step1AutosaveTimer`；Task 被替换即取消）。
    autosave_task: Option<Task<()>>,
    /// 模型在观察器里变了、待 render 时排入防抖（观察器回调拿不到 window）。
    draft_dirty: bool,
    draft_fingerprint: DraftSnapshot,
    /// 生成完成后 800ms 自动跳第 3 步的计时（原版 watch 的 `setTimeout`；Task 被替换即取消）。
    advance_task: Option<Task<()>>,
    /// 上一帧看到的 `[stage, retryingPages.size, 有失败页]`（原版 watch 的触发条件：
    /// **变了才判断**要不要排计时，不是每帧都判断）。
    advance_watch: Option<(String, usize, bool)>,
    /// 生成状态变了、待 render 时处理（观察器回调拿不到 window）。
    generation_dirty: bool,
    /// 加载完成后要进的那一步（store 的回调里没有 window，render 时补上）。
    pending_step: Option<Step>,
    /// 牌组落库的代次（[`Self::commit_deck`] 每提交一次加一）：迟到的落库结果整条作废，
    /// 不会去清掉后一次提交的「保存中」标记、也不会写回旧错误。
    deck_save_seq: u64,
    /// 持有订阅即保活：字段不读，但 drop 时会退订。
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,
}

impl WorkflowPage {
    pub fn new(
        handle: ShellHandle,
        id: Option<i64>,
        step: Option<Step>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let model = cx.new(|_| ProjectModel {
            id,
            name: String::new(),
            md_content: String::new(),
            md_filename: String::new(),
            granularity: Granularity::Auto,
            media: Vec::new(),
            draft_save_state: DraftSaveState::Idle,
            draft_save_error: String::new(),
            deck: Vec::new(),
            active_slide: 0,
            selected_slide: None,
            deck_saving: false,
            deck_save_error: None,
            aspect: AspectRatio::Ratio16x9,
            latest_run: None,
        });
        let generation = handle.generation();
        // 原版 `WorkflowView.onMounted` 无条件预热图标向量
        // （`invoke('ensure_icon_embeddings').catch(() => {})`）：第三步的「AI 推荐图标」要用它。
        // 结果与错误都忽略——没配向量库 / 模型时让它失败就好，不挡界面（`/new` 也预热，
        // 因为从 `/new` 存盘后是同一个页面实例，不会再挂载一次）。
        let preheat_shared = handle.shared().clone();
        run_async(
            window,
            cx,
            async move { generation::ensure_icon_embeddings(&preheat_shared).await },
            |_page: &mut Self,
             _result: Result<(), String>,
             _window: &mut Window,
             _cx: &mut Context<Self>| {},
        );
        let fingerprint = DraftSnapshot::capture(&model.read(cx));
        let rename = cx.new(|cx| {
            gpui_kit::component::input::InputState::new(window, cx).placeholder("演示名称")
        });
        let subscriptions = vec![
            cx.observe(&model, |page, _model, cx| page.on_model_changed(cx)),
            cx.observe(&generation, |page, _store, cx| {
                page.on_generation_changed(cx)
            }),
            cx.subscribe(
                &rename,
                |this: &mut Self, _state, event: &gpui_kit::component::input::InputEvent, cx| {
                    // Enter / 失焦保存（原版 `@keydown.enter="saveName"` / `@blur="saveName"`）。
                    // 订阅回调拿不到 window，推迟到 render 里提交。
                    if matches!(
                        event,
                        gpui_kit::component::input::InputEvent::PressEnter { .. }
                            | gpui_kit::component::input::InputEvent::Blur
                    ) {
                        this.rename_commit = true;
                        cx.notify();
                    }
                },
            ),
        ];
        let mut page = Self {
            handle,
            model,
            generation,
            load: Load::Loading,
            step: Step::Prepare,
            view: None,
            requested_step: step,
            editing_name: false,
            rename,
            rename_commit: false,
            // 新建项目没有加载过程，立即视为已引导（原版 onMounted 末尾置真）。
            step1_bootstrapped: id.is_none(),
            autosave_task: None,
            draft_dirty: false,
            draft_fingerprint: fingerprint,
            advance_task: None,
            advance_watch: None,
            generation_dirty: false,
            pending_step: None,
            deck_save_seq: 0,
            subscriptions,
        };
        match id {
            Some(id) => page.load_project(id, window, cx),
            None => {
                page.load = Load::Ready;
                // 原版 `/new` 路由的 `gen.reset()`；store 正为别的项目跑时不动它。
                if !page.generation.read(cx).busy() {
                    page.generation.update(cx, |store, cx| store.reset(cx));
                }
                page.mount_step(Step::Prepare, window, cx);
            }
        }
        page
    }

    /// 模型变了：更新草稿指纹，待 render 时排入防抖保存。
    fn on_model_changed(&mut self, cx: &mut Context<Self>) {
        let fingerprint = DraftSnapshot::capture(self.model.read(cx));
        if fingerprint != self.draft_fingerprint {
            self.draft_fingerprint = fingerprint;
            self.draft_dirty = true;
        }
        cx.notify();
    }

    // ── 标题栏改名（原版侧栏的 `saveName`：点名字 → 输入框，Enter / 失焦保存，Esc 取消） ──

    /// 标题栏正中的文稿名（`Shell` 每帧取一次）：平时是截断过的名字，点它进入改名；
    /// 改名时换成输入框。两者都 `occlude()`，否则点击落在标题栏的拖动区上、被系统拿去拖窗口。
    pub fn title_element(page: &Entity<Self>, cx: &mut App) -> AnyElement {
        let this = page.read(cx);
        let c = cx.studio().colors;
        if this.editing_name {
            let rename = this.rename.clone();
            let focus = rename.read(cx).focus_handle(cx);
            let page = page.clone();
            return div()
                .id("doc-title-edit")
                .occlude()
                .w(px(360.0))
                .text_color(c.text)
                .track_focus(&focus)
                .on_key_down(move |event: &gpui_kit::KeyDownEvent, _window, cx| {
                    // 原版 `@keydown.escape="editingName = false"`（Esc 不保存）。
                    if event.keystroke.key.eq_ignore_ascii_case("escape") {
                        page.update(cx, |this, cx| {
                            this.editing_name = false;
                            cx.notify();
                        });
                    }
                })
                .child(
                    gpui_kit::component::input::Input::new(&rename)
                        .with_size(gpui_kit::component::Size::Small),
                )
                .into_any_element();
        }
        let name = this.model.read(cx).name.trim().to_string();
        let shown = if name.is_empty() {
            // 原版侧栏的兜底名（`projectName || '未命名项目'`）。
            SharedString::from("未命名项目")
        } else {
            widgets::clip_title(&name)
        };
        let page = page.clone();
        div()
            .id("doc-title")
            .occlude()
            .min_w_0()
            .max_w(px(520.0))
            .px(px(10.0))
            .py(px(3.0))
            .rounded(px(5.0))
            .truncate()
            .text_size(px(13.5))
            .cursor_pointer()
            .hover(|style| style.bg(gpui_kit::white().opacity(0.12)))
            .tooltip_text(if name.is_empty() {
                SharedString::from("点击命名")
            } else {
                SharedString::from(format!("{name}（点击改名）"))
            })
            .child(shown)
            .on_click(move |_event, window, cx| {
                page.update(cx, |this, cx| this.start_rename(window, cx));
            })
            .into_any_element()
    }

    fn start_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.model.read(cx).name.clone();
        self.rename.update(cx, |state, cx| {
            state.set_value(name, window, cx);
        });
        self.editing_name = true;
        let focus = self.rename.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        cx.notify();
    }

    fn save_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editing_name {
            return;
        }
        // 原版 `if (!newName.value.trim()) return`：空名不保存，停在编辑态。
        let new_name = self.rename.read(cx).value().trim().to_string();
        if new_name.is_empty() {
            return;
        }
        let (id, md) = {
            let model = self.model.read(cx);
            (model.id, model.md_content.clone())
        };
        if let Some(id) = id {
            let shared = self.handle.shared().clone();
            let model_id = self.model.entity_id();
            let new_name_work = new_name.clone();
            run_bg(
                window,
                cx,
                move || {
                    projects::update_project_content(&shared, id, new_name_work, md)
                        .map_err(|e| e.to_string())
                },
                move |this: &mut Self, result, _window, cx| {
                    if this.model.entity_id() != model_id {
                        return;
                    }
                    match result {
                        Ok(()) => {
                            this.model.update(cx, |model, cx| {
                                model.name = new_name.clone();
                                cx.notify();
                            });
                            this.editing_name = false;
                        }
                        // 原版没有错误分支（await 抛到 unhandled）；这里保持编辑态并记日志。
                        Err(error) => eprintln!("[native] 改名失败：{error}"),
                    }
                    cx.notify();
                },
            );
        } else {
            // 未落库的项目只改模型名，自动保存会随后落库。
            self.model.update(cx, |model, cx| {
                model.name = new_name;
                cx.notify();
            });
            self.editing_name = false;
            cx.notify();
        }
    }

    // ── 第一步草稿自动保存（原版 `saveStep1Draft` + 800ms 防抖 watch） ──

    /// 排入 800ms 防抖（原版 `step1AutosaveTimer`；替换 Task 即取消上一轮的计时）。
    fn schedule_autosave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.autosave_task = None;
        let snapshot = DraftSnapshot::capture(self.model.read(cx));
        self.autosave_task = Some(cx.spawn_in(window, async move |this, window| {
            // GPUI 自己的计时器。不要在这里用 tokio 的 sleep：那要 `tokio().enter()`，而守卫跨 `.await`
            // 留在 UI 线程上，两个计时同时在等时会乱序释放，tokio 直接 panic。
            window
                .background_executor()
                .timer(std::time::Duration::from_millis(800))
                .await;
            let _ = this.update_in(window, |this, window, cx| {
                this.flush_draft(snapshot, window, cx);
            });
        }));
    }

    /// 防抖到点：内容与排入时不一致（期间又改了）就再等一轮，防止把旧稿写回去。
    fn flush_draft(
        &mut self,
        scheduled: DraftSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.autosave_task = None;
        if !self.step1_bootstrapped {
            return;
        }
        if DraftSnapshot::capture(self.model.read(cx)) != scheduled {
            self.schedule_autosave(window, cx);
            return;
        }
        self.persist_draft(
            window,
            cx,
            None::<fn(&mut Self, &mut Window, &mut Context<Self>)>,
        );
    }

    /// 立即把当前草稿落库（原版 `saveStep1Draft` 的主体）。`then` 在落库回调后执行
    /// （「开始 AI 生成」要先保存再启动流水线）。
    fn persist_draft(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: Option<impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static>,
    ) {
        let (id, draft_name, md, meta, aspect, meaningful) = {
            let model = self.model.read(cx);
            let meta = Step1ProjectMeta {
                md_filename: model.md_filename.clone(),
                granularity: model.granularity.as_str().to_string(),
                aspect_ratio: aspect_str(model.aspect).to_string(),
            };
            (
                model.id,
                draft_project_name(&model),
                model.md_content.clone(),
                meta,
                model.aspect,
                has_meaningful_draft(&model),
            )
        };
        // 原版：新建项目且没有「有意义的草稿」时不落库，状态回 idle。
        if id.is_none() && !meaningful {
            self.model.update(cx, |model, cx| {
                model.draft_save_state = DraftSaveState::Idle;
                model.draft_save_error.clear();
                cx.notify();
            });
            if let Some(then) = then {
                then(self, window, cx);
            }
            return;
        }

        let shared = self.handle.shared().clone();
        let model_id = self.model.entity_id();
        let draft_name_work = draft_name.clone();
        self.model.update(cx, |model, cx| {
            model.draft_save_state = DraftSaveState::Saving;
            model.draft_save_error.clear();
            cx.notify();
        });
        enum Outcome {
            Created(i64),
            Updated,
        }
        run_bg(
            window,
            cx,
            move || match id {
                None => {
                    let created = projects::create_project(&shared, draft_name_work, md)?;
                    projects::open_project(&shared, created)?;
                    projects::set_step1_project_meta(&shared, created, &meta)?;
                    projects::update_project_aspect_ratio(&shared, created, aspect)?;
                    Ok(Outcome::Created(created))
                }
                Some(id) => {
                    projects::update_project_content(&shared, id, draft_name_work, md)?;
                    projects::set_step1_project_meta(&shared, id, &meta)?;
                    projects::update_project_aspect_ratio(&shared, id, aspect)?;
                    Ok(Outcome::Updated)
                }
            },
            move |this: &mut Self, result, window, cx| {
                if this.model.entity_id() != model_id {
                    return;
                }
                match result {
                    Ok(Outcome::Created(id)) => {
                        this.model.update(cx, |model, cx| {
                            model.id = Some(id);
                            model.name = draft_name.clone();
                            model.draft_save_state = DraftSaveState::Saved;
                            cx.notify();
                        });
                        // 原版 `router.replace(/project/<id>)`：只改路由、不重建页面
                        // （重建会丢掉 Step1 的编辑器与撤销历史）。
                        this.handle.retarget_route(
                            Route::Project {
                                id: Some(id),
                                step: None,
                            },
                            cx,
                        );
                    }
                    Ok(Outcome::Updated) => {
                        this.model.update(cx, |model, cx| {
                            model.name = draft_name.clone();
                            model.draft_save_state = DraftSaveState::Saved;
                            cx.notify();
                        });
                    }
                    Err(error) => {
                        this.model.update(cx, |model, cx| {
                            model.draft_save_state = DraftSaveState::Error;
                            model.draft_save_error = error;
                            cx.notify();
                        });
                    }
                }
                if let Some(then) = then {
                    then(this, window, cx);
                }
            },
        );
    }

    // ── 生成状态（应用级 store） ──────────────────────────────────────────────

    /// 本页看到的生成状态：store 正持有本项目时是它的实时状态，否则是库里的静态状态
    /// （原版没有后一种：它会 `reset` 掉 store，把别的项目的在途生成整个抹掉）。
    fn generation_state(&self, cx: &Context<Self>) -> GenState {
        let model = self.model.read(cx);
        self.generation
            .read(cx)
            .state_for(model.id, model.latest_run.as_ref(), &model.deck)
    }

    /// store 变了：把它的牌组镜像进模型（第三步用它），并让自动跳页那一套重新判断。
    fn on_generation_changed(&mut self, cx: &mut Context<Self>) {
        let id = self.model.read(cx).id;
        let deck: Option<Vec<serde_json::Value>> = {
            let store = self.generation.read(cx);
            store.live_for(id).map(|store| {
                store
                    .blueprints()
                    .iter()
                    .filter_map(|slide| slide.clone())
                    .collect()
            })
        };
        if let Some(deck) = deck {
            self.model.update(cx, |model, cx| {
                if model.deck != deck {
                    model.deck = deck;
                    cx.notify();
                }
                // 牌组变短（生成收尾、删页、换项目）时当前页收敛到最后一页，
                // 否则第三步会指着一张不存在的页。
                let last = model.deck.len().saturating_sub(1);
                if model.active_slide > last {
                    model.active_slide = last;
                    cx.notify();
                }
            });
        }
        self.generation_dirty = true;
        cx.notify();
    }

    // ── 第三步的牌组（唯一写入路径） ─────────────────────────────────────────

    /// 第三步此刻是不是只读（**有意偏离原版**：原版没挡，它的第三步遇到生成中的空位会出错）。
    ///
    /// **只在本项目正在生成 / 重试时为真**：store 正持有本项目（[`GenerationStore::live_for`]）
    /// 且它在忙（`busy()` 是**全应用**的，所以两个条件都要）。别的项目在生成时本项目照常可编辑
    /// ——那时 [`GenerationStore::set_deck`] 写不进去，模型就是唯一状态，[`Self::commit_deck`]
    /// 照常把本项目自己的行落库，不会污染别人的牌组。
    /// 第三步的应用 / 新建 / 删除 / 移动 / 换版式动手前都问这里。
    ///
    /// 参数是 `&App` 而不是 `&Context<Self>`：第三步（[`step3::Step3`]）手上只有工作流页的
    /// `WeakEntity`，拿不到 `Context<WorkflowPage>`；这里也只用到 `App` 的那一半。
    pub fn editing_locked(&self, cx: &App) -> bool {
        let id = self.model.read(cx).id;
        self.generation
            .read(cx)
            .live_for(id)
            .is_some_and(|store| store.busy())
    }

    /// **第三步唯一的牌组写入路径**（原版 `useSlideEditor` 自己改 `gen.blueprints`，
    /// `applyEditorChanges` / `deleteEditorSlide` / `moveEditorSlide` / `onKindPicked` 各改各的）。
    ///
    /// 顺序：模型（`deck` + `active_slide`）→ store 的写入口（store 正持有本项目且不在生成中时
    /// 才写得进去，见 [`GenerationStore::set_deck`]；写不进去时模型就是唯一状态）→
    /// `save` 时把整副牌组的 JSON 落库（原版 `saveBlueprints`，紧凑格式）。
    /// 第三步的每一处编辑都只走这里。
    ///
    /// 调用方先看 [`Self::editing_locked`]（**本项目**生成 / 重试中只读）；本函数不自己挡，
    /// 免得悄悄丢掉调用方的改动。
    ///
    /// `deck_saving` 的每条出口（每条都复位）：
    /// 1. `save == false`（原版：新建页只插进牌组，随下一次应用 / 删除 / 移动一起存）；
    /// 2. `save == true` 但项目还没落库（没有 id）：没有可写的地方，同原版 `!projectId` 直接返回；
    /// 3. 落库成功：`deck_saving` 复位、错误清空；
    /// 4. 落库失败：`deck_saving` 复位、原因写进 [`ProjectModel::deck_save_error`]；
    /// 5. 期间又提交过一次（`deck_save_seq` 变了）：这次的结果整条丢掉，不碰这两个字段；
    /// 6. 页面此时已被丢弃：`run_bg` 的回调根本不会执行（标志随模型一起消失）。
    pub fn commit_deck(
        &mut self,
        deck: Vec<serde_json::Value>,
        active: usize,
        save: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let active = active.min(deck.len().saturating_sub(1));
        let id = self.model.read(cx).id;
        // 整副牌组的 JSON（原版 `JSON.stringify(gen.blueprints)`：紧凑、不带空白）。
        let saving =
            save.then(|| serde_json::to_string(&deck).unwrap_or_else(|_| "[]".to_string()));
        // store 正持有本项目时模型是它的镜像，只改模型会被下一次通知盖掉。
        self.generation
            .update(cx, |store, cx| store.set_deck(id, deck.clone(), cx));
        self.model.update(cx, |model, cx| {
            model.deck = deck;
            model.active_slide = active;
            cx.notify();
        });
        let (Some(json), Some(id)) = (saving, id) else {
            return;
        };
        // 本次提交的代次：每提交一次加一，回调里用它判断「期间又提交过一次」。
        // 取**加一之后**的值——取加一之前的值会让每次回调都以为自己迟到（提交 2 的漏网之鱼：
        // `deck_saving` 永远清不掉，保存按钮会一直卡在「保存中…」）。
        self.deck_save_seq += 1;
        let seq = self.deck_save_seq;
        self.model.update(cx, |model, cx| {
            model.deck_saving = true;
            model.deck_save_error = None;
            cx.notify();
        });
        let shared = self.handle.shared().clone();
        run_bg(
            window,
            cx,
            move || projects::update_project_blueprints(&shared, id, json),
            move |this: &mut Self, result, _window, cx| {
                // 迟到的结果：后一次提交已经在等自己的落库结果了。
                if this.deck_save_seq != seq {
                    return;
                }
                this.model.update(cx, |model, cx| {
                    model.deck_saving = false;
                    model.deck_save_error = result.err();
                    cx.notify();
                });
                cx.notify();
            },
        );
    }

    /// 原版 `WorkflowView.vue` 的 watch（`[gen.stage, gen.retryingPages.size, 有失败页]`）：
    /// 值变了才判断——`stage === 'done'`、没有页在重试、没有失败页且用户正停在本项目的
    /// 第 2 步时，800ms 后再检查一遍再跳第 3 步；条件变了就取消计时（替换 `advance_task`）。
    fn sync_advance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.generation_state(cx);
        let watched = (state.stage.clone(), state.retrying, state.failed_pages);
        if self.advance_watch.as_ref() == Some(&watched) {
            return;
        }
        self.advance_watch = Some(watched);
        // 条件变了：先取消上一轮计时。
        self.advance_task = None;
        if state.stage != "done"
            || state.retrying > 0
            || state.failed_pages
            || self.step != Step::Generate
        {
            return;
        }
        self.advance_task = Some(cx.spawn_in(window, async move |this, window| {
            // GPUI 自己的计时器（界面里的延时不用 tokio，见 README）。
            window
                .background_executor()
                .timer(std::time::Duration::from_millis(800))
                .await;
            let _ = this.update_in(window, |this, window, cx| {
                // 原版：到点再检查一遍（用户可能已经离开第 2 步）。
                let state = this.generation_state(cx);
                if this.step == Step::Generate && state.stage == "done" && state.retrying == 0 {
                    this.show_step(Step::Edit, window, cx);
                }
            });
        }));
    }

    // ── 开始 AI 生成（原版 `onStep1StartGenerate`） ───────────────────────────

    pub fn start_generation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (name, md, granularity, aspect) = {
            let model = self.model.read(cx);
            (
                model.name.clone(),
                model.md_content.clone(),
                model.granularity,
                model.aspect,
            )
        };
        // 原版 `canGenerate`：别的项目在生成或重试时也不能开始（第二步「重新生成全部」
        // 在静态状态下也看得到，这里一并挡住，免得切到第 2 步却什么都没发生）。
        if md.trim().is_empty() || name.trim().is_empty() || self.generation.read(cx).busy() {
            return;
        }
        // 原版：取消防抖计时器，立即同步保存一次草稿，然后切第 2 步、启动生成。
        self.autosave_task = None;
        self.persist_draft(
            window,
            cx,
            Some(
                move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
                    // 原版 `onStep1StartGenerate` 的 `selectedSlideIndex.value = null`：重新生成
                    // 时清掉上一次的选择，第二步重建后按新的看板归位。
                    this.model.update(cx, |model, cx| {
                        model.selected_slide = None;
                        cx.notify();
                    });
                    this.show_step(Step::Generate, window, cx);
                    // 原版 `granularity === 'auto' ? undefined : granularity`。
                    let granularity = match granularity {
                        Granularity::Auto => None,
                        other => Some(other.as_str().to_string()),
                    };
                    // 新建的项目刚才才落库，id 要现读（原版 `projectId.value`）。
                    let id = this.model.read(cx).id;
                    this.generation.update(cx, |store, cx| {
                        store.generate(md, Some(name), granularity, aspect, id, cx);
                    });
                },
            ),
        );
    }

    // ── 打开项目（原版 `WorkflowView.onMounted`） ─────────────────────────────

    fn load_project(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let shared = self.handle.shared().clone();
        run_bg(
            window,
            cx,
            move || {
                let project = projects::open_project(&shared, id)?;
                let run = generation::get_latest_generation_run(&shared, id)?;
                let media = huanshu_core::services::media::get_project_media(&shared, id)?;
                let meta = projects::get_step1_project_meta(&shared, id)?;
                Ok::<_, String>((project, run, media, meta))
            },
            move |page: &mut Self, result, _window, cx| {
                match result {
                    Err(error) => page.load = Load::Failed(error),
                    Ok((project, run, media, meta)) => {
                        let blueprints_json = project.blueprints_json.clone();
                        let deck: Vec<serde_json::Value> = if blueprints_json.trim().is_empty() {
                            Vec::new()
                        } else {
                            serde_json::from_str(&blueprints_json).unwrap_or_default()
                        };
                        let project_aspect = project.aspect_ratio.unwrap_or(AspectRatio::Ratio16x9);
                        page.model.update(cx, |model, cx| {
                            model.name = project.name;
                            model.md_content = project.md_content;
                            // 原版 `loadStep1Meta`：附加信息里的文件名 / 粒度 / 比例
                            // 优先于库里的列（localStorage 优先）；非法值回落默认。
                            if let Some(meta) = meta {
                                let meta = meta.resolve(
                                    Granularity::Auto.as_str(),
                                    aspect_str(project_aspect),
                                );
                                model.md_filename = meta.md_filename;
                                model.granularity = Granularity::parse(&meta.granularity)
                                    .unwrap_or(Granularity::Auto);
                                model.aspect =
                                    parse_aspect(&meta.aspect_ratio).unwrap_or(project_aspect);
                            } else {
                                model.aspect = project_aspect;
                            }
                            model.media = media;
                            model.deck = deck;
                            model.latest_run = run;
                            cx.notify();
                        });
                        // 原版 onMounted 的顺序（loadLatestRun → loadFromJson / reset →
                        // loadLatestRun）都在 store 里；它对齐完再决定进哪一步。
                        // 这次读回来的运行也要给 store：store 正为别的项目跑时，它照这份 id
                        // 判断静态分支的缓存要不要重读（`ensure_static`）。
                        let run = page.model.read(cx).latest_run.clone();
                        let page_weak = cx.weak_entity();
                        page.generation.update(cx, |store, cx| {
                            store.open_project(
                                id,
                                run.as_ref(),
                                &blueprints_json,
                                cx,
                                move |store, active, cx| {
                                    let Some(page) = page_weak.upgrade() else {
                                        return;
                                    };
                                    // 注意：这里仍在 store 的借用里，**不能读 store 实体**
                                    // （`state_for` 走手上的 `&mut Self`）。
                                    page.update(cx, |page, cx| {
                                        let state = {
                                            let model = page.model.read(cx);
                                            store.state_for(
                                                Some(id),
                                                model.latest_run.as_ref(),
                                                &model.deck,
                                            )
                                        };
                                        page.finish_open(id, active, state, cx);
                                    });
                                },
                            );
                        });
                    }
                }
                cx.notify();
            },
        );
    }

    /// store 对齐完项目后落第一步：原版
    /// `currentStep = hasActiveGeneration ? 2 : gen.blueprints.length > 0 ? 3 : 1`。
    fn finish_open(
        &mut self,
        id: i64,
        has_active_generation: bool,
        state: GenState,
        cx: &mut Context<Self>,
    ) {
        if self.model.read(cx).id != Some(id) {
            return;
        }
        self.load = Load::Ready;
        let auto = if has_active_generation {
            Step::Generate
        } else if state.has_blueprints {
            Step::Edit
        } else {
            Step::Prepare
        };
        // 下标回调里没有 window，进哪一步记下来、render 里补上（同 `rename_commit`）。
        self.pending_step = Some(self.requested_step.take().unwrap_or(auto));
        // 原版 `step1Bootstrapped`：初始加载完成后才打自动保存。
        // 同时把指纹对齐到刚加载的内容，避免加载本身触发一次回写。
        self.step1_bootstrapped = true;
        self.draft_dirty = false;
        self.draft_fingerprint = DraftSnapshot::capture(self.model.read(cx));
        cx.notify();
    }

    /// 侧栏一步能否点（原版 `steps[].clickable`）。
    fn clickable(&self, step: Step, cx: &Context<Self>) -> bool {
        let state = self.generation_state(cx);
        match step {
            Step::Prepare => true,
            // 原版：只有已经走到第 2 步及以后才能点回第 2 步。
            Step::Generate => self.step >= Step::Generate,
            // 原版 `gen.blueprints.length > 0 && gen.retryingPages.size === 0`：有页在重试时不可点。
            Step::Edit => state.has_blueprints && state.retrying == 0,
        }
    }

    pub fn show_step(&mut self, step: Step, window: &mut Window, cx: &mut Context<Self>) {
        self.release(cx);
        // 步骤同步回路由（原版 `/project/:id` 会在重挂载时按库里的状态重算步骤）：
        // 从工作流侧栏进设置时，外壳记下的来源路由因此带上当前这一步，返回能回到原处。
        let id = self.model.read(cx).id;
        self.handle.retarget_route(
            Route::Project {
                id,
                step: Some(step),
            },
            cx,
        );
        self.mount_step(step, window, cx);
    }

    /// 挂上某一步的视图，不同步路由。
    ///
    /// 构造函数里只能用它：此时外壳正在 `update`（启动时在 `open_window` 里建 `Shell`，
    /// 换页时在 `Shell::navigate` 里），再经 `retarget_route` 更新外壳会直接 panic；
    /// 而外壳随后写入的路由就是正在构建的这一条，不需要再对准。
    fn mount_step(&mut self, step: Step, window: &mut Window, cx: &mut Context<Self>) {
        let (model, handle) = (self.model.clone(), self.handle.clone());
        let page = cx.weak_entity();
        self.view = Some(match step {
            Step::Prepare => {
                StepView::Prepare(cx.new(|cx| step1::Step1::new(model, handle, page, window, cx)))
            }
            Step::Generate => {
                StepView::Generate(cx.new(|cx| step2::Step2::new(model, handle, page, window, cx)))
            }
            Step::Edit => {
                StepView::Edit(cx.new(|cx| step3::Step3::new(model, handle, page, window, cx)))
            }
        });
        self.step = step;
        cx.notify();
    }

    /// 离开工作流或换步骤：交还当前步骤手上的位图。
    pub fn release(&mut self, cx: &mut Context<Self>) {
        match self.view.take() {
            Some(StepView::Generate(view)) => view.update(cx, |view, cx| view.release(cx)),
            Some(StepView::Edit(view)) => view.update(cx, |view, cx| view.release(cx)),
            Some(StepView::Prepare(view)) => view.update(cx, |view, cx| view.release(cx)),
            None => {}
        }
    }
}

impl Render for WorkflowPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 观察器 / 下标回调里攒下的待办（那里没有 window）：
        // 排入防抖保存、提交改名、进加载完成后要进的那一步、重算自动跳页。
        if std::mem::take(&mut self.draft_dirty) && self.step1_bootstrapped {
            self.schedule_autosave(window, cx);
        }
        if std::mem::take(&mut self.rename_commit) {
            self.save_name(window, cx);
        }
        if let Some(step) = self.pending_step.take() {
            self.show_step(step, window, cx);
        }
        if std::mem::take(&mut self.generation_dirty) {
            self.sync_advance(window, cx);
        }

        let c = cx.studio().colors;
        match &self.load {
            Load::Loading => {
                return div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(c.muted)
                    .child("加载项目中...")
                    .into_any_element();
            }
            Load::Failed(error) => {
                let handle = self.handle.clone();
                return div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .items_center()
                    .justify_center()
                    .child(div().text_color(c.error).child(error.clone()))
                    .child(widgets::ghost_button("home", "返回首页").on_click(
                        move |_event, window, cx| handle.navigate(Route::Home, window, cx),
                    ))
                    .into_any_element();
            }
            Load::Ready => {}
        }

        let state = self.generation_state(cx);
        let model = self.model.read(cx);
        // 原版 `steps` 的说明文案。
        let descs = [
            if !model.md_filename.is_empty() {
                model.md_filename.clone()
            } else if !model.md_content.is_empty() {
                format!("{} 字符", model.md_len())
            } else {
                "上传或粘贴文稿".to_string()
            },
            if state.stage == "error" {
                "生成出错".to_string()
            } else if state.running {
                // 原版 `gen.stageLabel`（等待 / 初始化 / 验证连接 / 规划页面 / …）。
                state.stage_label()
            } else if state.slide_count > 0 {
                format!("已生成 {} 张", state.slide_count)
            } else {
                "等待开始".to_string()
            },
            if state.slide_count > 0 {
                format!("{} 张幻灯片", state.slide_count)
            } else {
                "生成后可编辑".to_string()
            },
        ];
        let generation_failed = state.stage == "error";
        // 原版：生成中第 2 步的圆点里是转圈（`.step-spinner`），不是序号。
        let generation_running = state.running;
        let theme_id = cx.studio().theme.id;

        let steps = [
            (Step::Prepare, "准备素材"),
            (Step::Generate, "AI 生成"),
            (Step::Edit, "编辑完善"),
        ];
        let nav = div().flex().flex_col().px(px(14.0)).pt(px(24.0)).children(
            steps.iter().enumerate().map(|(i, (step, label))| {
                let step = *step;
                let status_error = step == Step::Generate && generation_failed;
                let done = step < self.step;
                let active = step == self.step;
                let clickable = self.clickable(step, cx);
                // 原版三种状态的配色（`WorkflowView.vue:703-720`）：active = primary、
                // done = success（**绿**）、error = error；pending 是 border + muted。
                let circle = div()
                    .size(px(40.0))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_2()
                    .border_color(if status_error {
                        c.error
                    } else if active {
                        c.primary
                    } else if done {
                        c.success
                    } else {
                        c.border
                    })
                    .bg(if status_error {
                        c.error_bg
                    } else if active {
                        c.primary_bg
                    } else if done {
                        c.success_bg
                    } else {
                        c.panel
                    })
                    .text_color(if status_error {
                        c.error
                    } else if active {
                        c.primary
                    } else if done {
                        c.success
                    } else {
                        c.muted
                    })
                    .text_size(px(14.0))
                    .child(if status_error {
                        carbon("warning").into_any_element()
                    } else if done {
                        carbon("checkmark").into_any_element()
                    } else if active && step == Step::Generate && generation_running {
                        gpui_kit::component::spinner::Spinner::new()
                            .xsmall()
                            .color(c.primary)
                            .into_any_element()
                    } else {
                        step.number().to_string().into_any_element()
                    });
                let group = SharedString::from(format!("step-{}", step.number()));
                div()
                    .id(group.clone())
                    .group(group.clone())
                    .flex()
                    .flex_row()
                    .gap(px(14.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded(px(8.0))
                    .when(active, |item| item.bg(c.surface))
                    .when(!clickable, |item| item.opacity(0.5))
                    .when(clickable, |item| {
                        item.cursor_pointer().on_click(cx.listener(
                            move |page, _event, window, cx| {
                                if page.step != step {
                                    page.show_step(step, window, cx)
                                }
                            },
                        ))
                    })
                    .child(div().flex().flex_col().items_center().child(circle).when(
                        i < 2,
                        |column| {
                            // 原版 `.step-line`（`WorkflowView.vue:730-759`）：走过的连线是 success。
                            column.child(div().w(px(2.0)).h(px(40.0)).bg(if done {
                                c.success
                            } else {
                                c.border
                            }))
                        },
                    ))
                    .child(
                        div()
                            .pt(px(8.0))
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(15.0))
                                    // 原版：active 与 done 的标签都是 text（`WorkflowView.vue:772-773`）。
                                    .text_color(if active || done { c.text } else { c.muted })
                                    // 悬停时标签变亮（原版 `:680` `.step-item:not(.disabled):hover .step-label`）。
                                    .when(clickable, |text| {
                                        text.group_hover(group.clone(), move |style| {
                                            style.text_color(c.text)
                                        })
                                    })
                                    .child(*label),
                            )
                            .child(
                                div()
                                    // 原版 `.steps-nav .step-desc`：max-width 130px + 省略号（`:775-783`）。
                                    .max_w(px(130.0))
                                    .truncate()
                                    .text_size(px(12.5))
                                    .text_color(c.muted)
                                    .child(descs[i].clone()),
                            ),
                    )
            }),
        );

        let (back, settings, theme) = (
            self.handle.clone(),
            self.handle.clone(),
            self.handle.clone(),
        );
        let sidebar = div()
            .flex_none()
            .w(px(204.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(c.panel)
            .border_r_1()
            .border_color(c.border)
            // 顶上是「回到上一级」的导航（项目列表），文稿名挪到标题栏正中（`title_element`）。
            // 高度与三步的顶栏一致（`HEADER_H`），左右连成一条线。
            .child(
                div()
                    .flex_none()
                    .h(px(HEADER_H))
                    .px(px(10.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .border_b_1()
                    .border_color(c.border)
                    .child(
                        widgets::ghost_button("back", "项目")
                            .icon(carbon("arrow-left"))
                            .tooltip("返回项目列表")
                            .on_click(move |_event, window, cx| {
                                back.navigate(Route::Home, window, cx)
                            }),
                    ),
            )
            .child(nav)
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .p(px(14.0))
                    .border_t_1()
                    .border_color(c.border)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(widgets::section_label("主题", cx))
                            .child(widgets::theme_dots(
                                "workflow-theme",
                                theme_id,
                                move |id, window, cx| theme.set_theme(id, true, window, cx),
                                cx,
                            )),
                    )
                    .child(
                        widgets::ghost_button("settings", "设置")
                            .icon(carbon("settings"))
                            .on_click(move |_event, window, cx| {
                                settings.navigate(
                                    Route::Settings(SettingsTab::default()),
                                    window,
                                    cx,
                                )
                            }),
                    ),
            );

        let content = match &self.view {
            Some(StepView::Prepare(view)) => view.clone().into_any_element(),
            Some(StepView::Generate(view)) => view.clone().into_any_element(),
            Some(StepView::Edit(view)) => view.clone().into_any_element(),
            None => div().into_any_element(),
        };
        div()
            .size_full()
            .flex()
            .flex_row()
            .child(sidebar)
            .child(div().flex_1().min_w_0().h_full().child(content))
            .into_any_element()
    }
}
