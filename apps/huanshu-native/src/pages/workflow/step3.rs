//! 第三步「编辑完善」（原版 `steps/Step3Editor.vue` + `composables/useSlideEditor.ts`）。
//!
//! 逐字对照原版：三栏（`220px | 1fr | 390px`，每栏自己的栏头）、左栏幻灯片列表（悬停行才显示
//! 垃圾桶、只剩一页时不显示）、中栏实时预览（**只发当前这一页**、120ms 防抖、换页清图、
//! 「渲染中…」持续 150ms 才显示、裁切提示）、右栏按 [`editor_schema`] 生成的表单
//! （幻灯片类型 / 编辑模式 / 基础信息 / 标签组 / 图片素材 / 列表区 / 对象区 / 保存按钮三态 +
//! 保存失败行）、KindPicker（新建 / 更换版式）。
//!
//! 规则一律在 core：草稿与牌组的编辑走 `huanshu_core::slide_edit`（快照测试守住），表单结构走
//! `huanshu_core::editor_schema`——本文件里一行规则都不写死。牌组只有一条写入路径：所有编辑都经
//! `WorkflowPage::commit_deck`（模型 → 应用级 store → 落库）；生成进行中
//! （`WorkflowPage::editing_locked`）第三步只读。
//!
//! 四块「剩下的」也都照原版做完：图标助手（`recommendIcon` / `applySuggestedIcon`）、素材插图
//! （`.ej-media-strip`：缩略图 / 设为主图 / 加入图集）、图表数据（`.ej-chart-json`：150ms 防抖走
//! `chart_check`，**只有通过校验的 JSON 才写回草稿**）、高级 Blueprint JSON（`.ej-textarea` +
//! 行号栏：草稿与文本双向，应用写回的是**文本解析出来的那一页**）。
//!
//! GPUI 下的几处做法（原版机制写在各自注释里）：
//! - 输入框是有状态的实体，不能每帧新建：按**完整路径**（`cards[2].title`）缓存 + 每框订阅，
//!   结构变化时用「表单代次」整组重建（见 `Step3::bump_form`）；
//! - 换页清图用 `SlideImages::reset`（不是重建 `SlideImages`）；
//! - 键盘只在第三步根节点**自己有焦点**时换页（`focus_handle.is_focused`，焦点在输入框里就不处理）；
//! - 图表 JSON 框与高级 JSON 框各是**一个**长期实体（原版就是同一个 `<textarea>` / 行号栏），
//!   render 里同步值时跳过有焦点的那个；界面里的 150ms 防抖一律用 GPUI 的计时器
//!   （订阅回调里拿不到 window，走 `Context::spawn` + `AsyncApp::background_executor`，不是 tokio）。

use std::collections::HashMap;
use std::time::Duration;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::input::{
    Editor, EditorState, Input, InputEvent, InputState, Textarea, TextareaState,
};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{Disableable as _, IndexPath, Sizable as _, Size, WindowExt as _};
use gpui_kit::prelude::*;
use gpui_kit::{
    div, px, AnyElement, App, Context, Div, Entity, FocusHandle, Focusable as _, KeyDownEvent,
    Render, SharedString, Subscription, Task, WeakEntity, Window,
};
use huanshu_core::chart_check;
use huanshu_core::editor_schema::{
    self, EditorField, EditorSchema, KindEntry, ListSectionDef, NestedListDef, ObjectSectionDef,
};
use huanshu_core::preview::PreviewOptions;
use huanshu_core::services::generation;
use huanshu_core::slide_edit;
use serde_json::Value;

use super::{parse_aspect, ProjectModel, WorkflowPage};
use crate::icons::carbon;
use crate::present::PresentArgs;
use crate::router::Step;
use crate::shell::ShellHandle;
use crate::slides::{slide_frame, SlideImages};
use crate::tasks::{run_async, run_bg};
use crate::theme::StudioExt as _;
use crate::widgets::{self, Tone2};

/// 左栏 / 右栏宽度（原版 `.edit-three-col` 的 `220px 1fr 390px`）。
const LIST_W: f32 = 220.0;
const PANEL_W: f32 = 390.0;
/// 工作流侧栏宽度（`mod.rs` 的 `sidebar`）。
const SIDEBAR_W: f32 = 204.0;
/// 栏头高度（原版 `--workflow-header-height: 3.5rem`）。
const COL_HEAD_H: f32 = 56.0;
/// 舞台四周留白（原版 `.edit-preview-stage-wrap` 的 `padding: 1rem`）。
const STAGE_PADDING: f32 = 16.0;
/// 草稿变化后等这么久再发预览请求（原版 `debounceMs: 120`）。
const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(120);
/// 「渲染中…」的 `pending` 要持续这么久才显示（原版 `renderBusy` 的 150ms）。
const RENDER_BUSY_DELAY: Duration = Duration::from_millis(150);
/// 图表 JSON 的校验防抖（原版 `scheduleChartCheck` / `advancedChartErrors` 的 150ms）。
const CHART_CHECK_DEBOUNCE: Duration = Duration::from_millis(150);
/// 图表 JSON 框的行数（原版 `rows="12"`）。
const CHART_ROWS: usize = 12;
/// 图表 JSON 框的占位文案（逐字照原版模板）。
const CHART_PLACEHOLDER: &str =
    r#"{ "type": "bar", "title": "…", "data": [], "encoding": { "x": "…", "y": "…" } }"#;
/// 高级 JSON 编辑框的高度（原版 `.ej-textarea` 撑满右栏剩余空间，这里给一个固定高度）。
const JSON_EDITOR_H: f32 = 300.0;
/// 裁切提示占的高度（算舞台可用高度时扣掉）。
const CLIP_HINT_H: f32 = 30.0;

/// KindPicker 的两种模式（原版 `kindPickerMode`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KindPickerMode {
    Add,
    Change,
}

/// 缓存的输入框（单行 `Input` 与多行 `Textarea` 是两种实体）。
#[derive(Clone)]
enum FieldInput {
    Single(Entity<InputState>),
    Multi(Entity<TextareaState>),
}

impl FieldInput {
    fn value(&self, cx: &App) -> String {
        match self {
            Self::Single(state) => state.read(cx).value().to_string(),
            Self::Multi(state) => state.read(cx).value().to_string(),
        }
    }

    fn is_focused(&self, window: &Window, cx: &App) -> bool {
        match self {
            Self::Single(state) => state.read(cx).focus_handle(cx).is_focused(window),
            Self::Multi(state) => state.read(cx).focus_handle(cx).is_focused(window),
        }
    }

    /// 程序化写值：`set_value` **不发** `InputEvent::Change`，不会回环到草稿。
    fn set_value(&self, value: String, window: &mut Window, cx: &mut App) {
        match self {
            Self::Single(state) => {
                state.update(cx, |state, cx| state.set_value(value, window, cx));
            }
            Self::Multi(state) => {
                state.update(cx, |state, cx| state.set_value(value, window, cx));
            }
        }
    }
}

/// 表单控件的缓存项：代次变了就作废（列表增删移之后，同一个路径指向的是别的项）。
struct CachedField {
    input: FieldInput,
    generation: u64,
}

struct CachedSelect {
    entity: Entity<SelectState<Vec<String>>>,
    generation: u64,
}

pub struct Step3 {
    model: Entity<ProjectModel>,
    /// 工作流页面：牌组的编辑走它的 `WorkflowPage::commit_deck`（唯一写入路径）、
    /// 生成中只读看 `WorkflowPage::editing_locked`。
    page: WeakEntity<WorkflowPage>,
    handle: ShellHandle,
    schema: EditorSchema,
    preview: SlideImages,
    exporting: bool,
    focus: FocusHandle,

    // ── 编辑草稿（原版 `useSlideEditor` 的状态） ──────────────────────────────
    /// 当前页的**未过滤**整页克隆（原版 `liveSlide`）。
    draft: Option<Value>,
    /// 有未应用的修改（原版 `hasUnsavedChanges`）。
    has_unsaved: bool,
    /// 应用更改已经发出、正等落库回执；「保存中」本身读 `ProjectModel.deck_saving`。
    applying: bool,
    /// 高级 JSON 展开与否（原版 `advancedMode`）。
    advanced_json: bool,
    /// KindPicker 开着时的模式（原版 `showKindPicker` + `kindPickerMode`）。
    kind_picker: Option<KindPickerMode>,

    // ── 图标助手（原版 `recommendIcon` / `applySuggestedIcon`） ──────────────
    /// 正在推荐的那张卡片的路径（原版 `iconLoadingKey`）。每条出口都复位。
    icon_loading: Option<String>,
    /// 候选按卡片路径存（原版 `iconSuggestions`；原版换页不清，这里照做）。
    icon_suggestions: HashMap<String, Vec<String>>,
    /// 推荐请求的代次：发起时记下，与「页 + 表单代次 + 项目」一起判断迟到的结果。
    icon_seq: u64,

    // ── 图表数据（JSON）框（原版 `chartJson*`） ───────────────────────────────
    /// 框里的文本：**本地状态**，不是草稿字段（框有焦点时不被草稿变化覆盖）。
    chart_text: String,
    /// 语法错误 / 校验问题；空 = 没问题。
    chart_error: String,
    /// 框有焦点（原版 `chartJsonFocused`；失焦时决定写回后要不要规范化输入）。
    chart_focused: bool,
    /// 正在等防抖（原版 `chartJsonChecking`）：提示显示「正在校验…」，等待期间不写回。
    chart_checking: bool,
    /// 校验的代次：每次输入 / 取消 +1，迟到的结果作废。
    chart_check_seq: u64,
    /// 防抖计时（drop 即取消）。
    chart_check_task: Option<Task<()>>,
    /// 这一次校验的是哪个内容（原版 `pendingChart`）。
    pending_chart: Option<Value>,
    /// 这一次校验是在哪一页发起的（原版 `pendingSlide`）：结果只写回那一页。
    chart_check_page: usize,
    /// 图表 JSON 框：单例，与原版那个 `<textarea>` 一样不随表单重建。
    chart_input: Entity<TextareaState>,

    // ── 高级 Blueprint JSON（原版 `jsonText` / `jsonError`） ──────────────────
    /// 框里的文本：表单改动 / 换版式 / 换页 / 应用 / 格式化后 = 过滤后草稿的两空格缩进 JSON。
    json_text: String,
    /// JSON 语法错误（serde_json 的报错；显示时前面拼「第 N 行：」）。
    json_error: String,
    /// 行号栏编辑框（GPUI Kit 的代码编辑器：多行、等宽、Tab 两个空格）。
    json_editor: Entity<EditorState>,
    /// 图表的语义错误（原版 `advancedChartErrors`：`chart：{问题}`），150ms 防抖、只认最新一次。
    json_chart_errors: Vec<String>,
    json_chart_seq: u64,
    json_chart_task: Option<Task<()>>,

    // ── 预览 ─────────────────────────────────────────────────────────────────
    /// 草稿变了、待 render 时排入 120ms 防抖（订阅回调里拿不到 window）。
    preview_dirty: bool,
    preview_task: Option<Task<()>>,
    /// 「渲染中…」的计时（`pending` 持续超过 150ms 才置 `render_busy`）。
    preview_probe: Option<Task<()>>,
    render_busy: bool,

    // ── 表单控件缓存 ─────────────────────────────────────────────────────────
    /// 「表单代次」：换页、应用成功、换版式、列表增删移时 +1，整组清空缓存与订阅。
    form_generation: u64,
    fields: HashMap<String, CachedField>,
    selects: HashMap<String, CachedSelect>,
    /// 控件订阅与缓存同生共死（drop 即退订）。
    field_subs: Vec<Subscription>,
    /// 持有订阅即保活：字段不读，但 drop 时会退订。
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,
}

impl Step3 {
    pub fn new(
        model: Entity<ProjectModel>,
        handle: ShellHandle,
        page: WeakEntity<WorkflowPage>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let id = model.read(cx).id;
        // 只发当前这一页：缓存上限不需要大。
        let preview = SlideImages::new(
            handle.shared().clone(),
            id,
            PreviewOptions {
                max_cached: Some(4),
                ..PreviewOptions::default()
            },
        );
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let schema = editor_schema::editor_schema();
        let draft = current_page(&model, cx);
        // 原版 `initEditorSlide`：高级 JSON 框的初值 = 过滤后草稿的两空格缩进 JSON。
        let json_text = draft
            .as_ref()
            .map(|draft| pretty_json(&slide_edit::filter_slide_json(&schema, draft)))
            .unwrap_or_default();
        let json_editor = cx.new(|cx| {
            // 代码编辑器：多行、等宽、带行号（原版 `.ej-gutter` + `.ej-textarea`）。
            // 不设语言：原版那个 `<textarea>` 也不高亮，这里只要行号与 Tab 两个空格。
            EditorState::new(window, cx)
                .line_number(true)
                .default_value(json_text.clone())
        });
        // 原版那条图表 watch 是 `immediate: true`：一进来框里就是草稿里的图表。
        let chart_text = chart_json_of(draft.as_ref());
        let chart_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(CHART_ROWS)
                .placeholder(CHART_PLACEHOLDER)
                .default_value(chart_text.clone())
        });
        let subscriptions = vec![
            cx.observe(&model, |this, _model, cx| this.on_model_changed(cx)),
            cx.subscribe(
                &chart_input,
                |this: &mut Self, state, event: &InputEvent, cx| match event {
                    InputEvent::Change => {
                        let value = state.read(cx).value().to_string();
                        this.on_chart_text_changed(value, cx);
                    }
                    InputEvent::Focus => {
                        this.chart_focused = true;
                        cx.notify();
                    }
                    InputEvent::Blur => this.on_chart_blur(cx),
                    _ => {}
                },
            ),
            cx.subscribe(
                &json_editor,
                |this: &mut Self, state, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        let value = state.read(cx).value().to_string();
                        this.on_json_text_changed(value, cx);
                    }
                },
            ),
        ];
        Self {
            model,
            page,
            handle,
            schema,
            preview,
            exporting: false,
            focus,
            draft,
            has_unsaved: false,
            applying: false,
            advanced_json: false,
            kind_picker: None,
            icon_loading: None,
            icon_suggestions: HashMap::new(),
            icon_seq: 0,
            chart_text,
            chart_error: String::new(),
            chart_focused: false,
            chart_checking: false,
            chart_check_seq: 0,
            chart_check_task: None,
            pending_chart: None,
            chart_check_page: 0,
            chart_input,
            json_text,
            json_error: String::new(),
            json_editor,
            json_chart_errors: Vec::new(),
            json_chart_seq: 0,
            json_chart_task: None,
            preview_dirty: true,
            preview_task: None,
            preview_probe: None,
            render_busy: false,
            form_generation: 0,
            fields: HashMap::new(),
            selects: HashMap::new(),
            field_subs: Vec::new(),
            subscriptions,
        }
    }

    /// 离开页面：位图、在途的图标推荐、两处 JSON 防抖与图表校验全部停下来。
    pub fn release(&mut self, cx: &mut Context<Self>) {
        self.preview.release(cx);
        self.preview_task = None;
        self.preview_probe = None;
        self.kind_picker = None;
        self.icon_loading = None;
        self.icon_seq += 1;
        self.cancel_chart_check();
        self.json_chart_task = None;
        self.json_chart_seq += 1;
    }

    // ── 当前页与草稿（原版 `activeSlide` + `initEditorSlide`） ────────────────

    /// 当前页（原版 `activeSlide`）：它是 [`ProjectModel`] 的字段，切步骤回来还在。
    fn active(&self, cx: &Context<Self>) -> usize {
        let model = self.model.read(cx);
        model.active_slide.min(model.deck.len().saturating_sub(1))
    }

    fn deck(&self, cx: &Context<Self>) -> Vec<Value> {
        self.model.read(cx).deck.clone()
    }

    fn count(&self, cx: &Context<Self>) -> usize {
        self.model.read(cx).deck.len()
    }

    /// 本页此刻只读（生成 / 重试在跑）。
    fn editing_locked(&self, cx: &App) -> bool {
        self.page
            .upgrade()
            .is_some_and(|page| page.read(cx).editing_locked(cx))
    }

    fn saving(&self, cx: &Context<Self>) -> bool {
        self.model.read(cx).deck_saving
    }

    fn save_error(&self, cx: &Context<Self>) -> Option<String> {
        self.model.read(cx).deck_save_error.clone()
    }

    /// 原版 `initEditorSlide`：草稿 = 牌组当前页的克隆、`hasUnsavedChanges = false`。
    ///
    /// **换页会丢掉未应用的修改**——原版如此（不加确认框）。换页同时清掉旧预览位图。
    fn init_draft(&mut self, cx: &mut Context<Self>) {
        self.draft = current_page(&self.model, cx);
        self.has_unsaved = false;
        self.bump_form();
        self.preview.reset(cx);
        self.preview_dirty = true;
        // 图表 JSON 框、图标助手、高级 JSON 都是「这一页」的状态（原版 watch + `initEditorSlide`）。
        self.reset_chart(cx);
        self.refresh_json_text(cx);
        cx.notify();
    }

    /// 表单代次 +1：整组清空控件缓存与订阅，下一帧按新草稿重建。
    ///
    /// 结构变化（换页、换版式、列表增删移、应用成功）之后，卡片路径可能已经指向别的项：
    /// 图标助手的「推荐中」与在途结果一并作废（原版没有代次，这里补上）。
    fn bump_form(&mut self) {
        self.form_generation += 1;
        self.fields.clear();
        self.selects.clear();
        self.field_subs.clear();
        self.icon_loading = None;
        self.icon_seq += 1;
    }

    /// 换页（原版 `setActiveSlide`）：换页本身不改牌组，写的是模型的字段。
    fn select(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.count(cx) || index == self.active(cx) {
            // 两端：原版直接 return（当前页与草稿都不动），焦点仍拿回根节点。
            self.focus.focus(window, cx);
            return;
        }
        self.model.update(cx, |model, cx| {
            model.active_slide = index;
            cx.notify();
        });
        self.init_draft(cx);
        // 点左栏行 / 翻页按钮后把焦点拿回根节点，否则点过输入框之后方向键就不灵了。
        self.focus.focus(window, cx);
    }

    // ── 编辑动作（全部经 `slide_edit` 的纯函数） ─────────────────────────────

    /// 表单改动：`set_at_path` 算出新草稿 → 未保存 → 预览防抖。
    fn set_path(&mut self, path: &str, value: Value, cx: &mut Context<Self>) {
        let Some(draft) = self.draft.as_mut() else {
            return;
        };
        slide_edit::set_at_path(draft, path, value);
        self.mark_unsaved(cx);
    }

    /// 列表操作（增 / 删 / 移 / 字符串项）的统一入口：`slide_edit` 给出新草稿就换掉。
    fn apply_draft_edit(&mut self, updated: Option<Value>, cx: &mut Context<Self>) {
        let Some(updated) = updated else {
            // `None` = 原版没有 `replaceEditorDraft`（越界不动、草稿不是对象）：什么都不动。
            return;
        };
        self.draft = Some(updated);
        // 下标变了，路径就指向别的项：整组重建控件。
        self.bump_form();
        self.mark_unsaved(cx);
    }

    fn mark_unsaved(&mut self, cx: &mut Context<Self>) {
        self.has_unsaved = true;
        self.preview_dirty = true;
        // 原版 `updatePath` → `replaceDraft` → `replaceEditorDraft`：表单每改一次，高级 JSON
        // 框就跟着换成过滤后草稿的两空格缩进 JSON（没打开这个框时也维护着）。
        self.refresh_json_text(cx);
        cx.notify();
    }

    fn add_item(&mut self, path: &str, new_item: &Value, cx: &mut Context<Self>) {
        let Some(draft) = self.draft.clone() else {
            return;
        };
        let updated = slide_edit::add_item(&draft, path, new_item);
        self.apply_draft_edit(updated, cx);
    }

    fn remove_item(&mut self, path: &str, index: usize, cx: &mut Context<Self>) {
        let Some(draft) = self.draft.clone() else {
            return;
        };
        let updated = slide_edit::remove_item(&draft, path, index);
        self.apply_draft_edit(updated, cx);
    }

    fn move_item(&mut self, path: &str, index: usize, dir: i64, cx: &mut Context<Self>) {
        let Some(draft) = self.draft.clone() else {
            return;
        };
        let updated = slide_edit::move_item(&draft, path, index, dir);
        self.apply_draft_edit(updated, cx);
    }

    fn add_string_item(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(draft) = self.draft.clone() else {
            return;
        };
        let updated = slide_edit::add_string_item(&draft, path);
        self.apply_draft_edit(updated, cx);
    }

    fn remove_string_item(&mut self, path: &str, index: usize, cx: &mut Context<Self>) {
        let Some(draft) = self.draft.clone() else {
            return;
        };
        let updated = slide_edit::remove_string_item(&draft, path, index);
        self.apply_draft_edit(updated, cx);
    }

    /// 原版 `appendGalleryImage`：把素材引用追加到 `images`，**已有的不重复**。
    ///
    /// 取值与写值都走 `slide_edit`（`array_value` / `set_at_path`），不在这里手写路径逻辑。
    fn append_gallery_image(&mut self, image_ref: &str, cx: &mut Context<Self>) {
        let Some(draft) = self.draft.clone() else {
            return;
        };
        let mut images: Vec<String> = slide_edit::array_value(&draft, "images")
            .iter()
            .map(|item| item.as_str().unwrap_or("").to_string())
            .collect();
        if !images.iter().any(|item| item == image_ref) {
            images.push(image_ref.to_string());
        }
        self.set_path(
            "images",
            Value::Array(images.into_iter().map(Value::String).collect()),
            cx,
        );
    }

    /// 原版 `applyEditorChanges`：写回的是**高级 JSON 框里解析出来的那一页**（`JSON.parse(jsonText)`）。
    ///
    /// 平时它就是过滤后的草稿（`json_text` 每次表单改动都由 [`Self::refresh_json_text`] 维护），
    /// 用户在 JSON 里手加的字段会原样保存。有 JSON 错误时应用无效（原版按钮禁用、Ctrl+S 无效）。
    fn apply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.json_error.is_empty()
            || !self.has_unsaved
            || self.saving(cx)
            || self.editing_locked(cx)
        {
            return;
        }
        // 框里的文本解析不出来时什么都不做（不是「写回草稿」而是「什么都没发生」）。
        let Ok(parsed) = serde_json::from_str::<Value>(&self.json_text) else {
            return;
        };
        let active = self.active(cx);
        let mut deck = self.deck(cx);
        if active >= deck.len() {
            return;
        }
        deck[active] = parsed;
        self.commit(deck, active, true, window, cx);
        // 落库真的发出去了才有回执可等（没有 id 的项目原版也是直接返回、什么都不改）。
        self.applying = self.model.read(cx).deck_saving;
    }

    /// 牌组的唯一写入路径（[`WorkflowPage::commit_deck`]）。
    fn commit(
        &mut self,
        deck: Vec<Value>,
        active: usize,
        save: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = self.page.upgrade() else {
            return;
        };
        page.update(cx, |page, cx| {
            page.commit_deck(deck, active, save, window, cx)
        });
    }

    /// 原版 `deleteEditorSlide`：删掉某页（只剩一页时 `slide_edit` 给 `None`，不删不动）。
    fn delete_slide(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing_locked(cx) {
            return;
        }
        let deck = self.deck(cx);
        let Some(edit) = slide_edit::delete_slide(&deck, index) else {
            return;
        };
        // 删除立即存库（原版 `saveBlueprints`）。
        self.commit(edit.deck, edit.active, true, window, cx);
        self.init_draft(cx);
    }

    /// 原版 `moveEditorSlide`：当前页与相邻一页交换，立即存库。
    fn move_slide(&mut self, dir: i64, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing_locked(cx) {
            return;
        }
        let deck = self.deck(cx);
        let active = self.active(cx);
        let Some(edit) = slide_edit::move_slide(&deck, active, dir) else {
            return;
        };
        self.commit(edit.deck, edit.active, true, window, cx);
        self.init_draft(cx);
    }

    /// 回第 1 步（原版 `go-to-step-1`）。
    fn go_to_step_1(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(page) = self.page.upgrade() else {
            return;
        };
        page.update(cx, |page, cx| page.show_step(Step::Prepare, window, cx));
    }

    // ── KindPicker（原版 `openKindPicker` / `onKindPicked`） ─────────────────

    /// 打开 KindPicker。外框只让 GPUI Kit 的 `Dialog` 画（上一轮重试对话框画了两层）。
    fn open_kind_picker(
        &mut self,
        mode: KindPickerMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editing_locked(cx) {
            return;
        }
        self.kind_picker = Some(mode);
        let kinds = self.schema.kinds.clone();
        let current = self
            .draft
            .as_ref()
            .and_then(|draft| draft.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let after = self.active(cx) + 1;
        let weak = cx.weak_entity();
        let close_weak = weak.clone();
        window.open_dialog(cx, move |dialog: Dialog, _window, cx| {
            // 点遮罩、按 Esc、点关闭按钮都会走 `on_close`：**原版点遮罩 / 点关闭按钮关不掉**
            // （事件写成了再次打开），这里修掉。
            dialog
                .overlay(true)
                // 关闭按钮画在我们自己的栏头里（原版 `.kp-head` 的右上角 X），
                // 不要 `Dialog` 自带的那一个（它会挤在标题旁边）。
                .close_button(false)
                .w(px(680.0))
                .on_close({
                    let weak = close_weak.clone();
                    move |_event, _window, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.kind_picker = None;
                            cx.notify();
                        });
                    }
                })
                .child(kind_picker_body(
                    mode,
                    &kinds,
                    &current,
                    after,
                    weak.clone(),
                    cx,
                ))
        });
    }

    /// 原版 `onKindPicked`。
    fn pick_kind(
        &mut self,
        mode: KindPickerMode,
        kind: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editing_locked(cx) {
            self.kind_picker = None;
            cx.notify();
            return;
        }
        match mode {
            // 更换：新 kind 的默认内容 + 草稿里可迁移的公共字段；不改牌组，等「应用更改」。
            KindPickerMode::Change => {
                let Some(draft) = self.draft.clone() else {
                    self.kind_picker = None;
                    return;
                };
                let updated = slide_edit::create_kind_blueprint(&self.schema, kind, Some(&draft));
                self.draft = Some(updated);
                self.has_unsaved = true;
                self.bump_form();
                // 原版那两条 watch 观察 `editorKind`：换版式后图表框与 `jsonText` 都按新草稿重置。
                self.reset_chart(cx);
                self.refresh_json_text(cx);
                self.preview_dirty = true;
            }
            // 新建：插在当前页之后（章节号继承），只改内存不落库（原版如此）。
            KindPickerMode::Add => {
                let deck = self.deck(cx);
                let active = self.active(cx);
                let edit = slide_edit::insert_new_slide(&self.schema, &deck, active, kind);
                self.commit(edit.deck, edit.active, false, window, cx);
                self.draft = current_page(&self.model, cx);
                self.has_unsaved = false;
                self.bump_form();
                self.preview.reset(cx);
                self.preview_dirty = true;
                // 换了一页：图表框与高级 JSON 都跟着换（`init_draft` 里那两下）。
                self.reset_chart(cx);
                self.refresh_json_text(cx);
            }
        }
        self.kind_picker = None;
        cx.notify();
    }

    // ── 键盘 ─────────────────────────────────────────────────────────────────

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = &event.keystroke.modifiers;
        // Ctrl+S 在第三步任何位置都生效（**有意的小改进**：原版只在高级 JSON 文本框里）。
        if (modifiers.control || modifiers.platform)
            && event.keystroke.key.eq_ignore_ascii_case("s")
        {
            self.apply(window, cx);
            return;
        }
        // KindPicker 开着时禁第三步键盘。
        if self.kind_picker.is_some() {
            return;
        }
        // 原版 `handleEditorArrowKeydown`：带修饰键不处理。
        if modifiers.alt || modifiers.control || modifiers.platform || modifiers.shift {
            return;
        }
        // 只有第三步根节点**自己有焦点**才换页（原版 `shouldIgnoreEditorKeydown`：
        // 焦点在输入框 / 下拉里就不处理）。
        if !self.focus.is_focused(window) {
            return;
        }
        let active = self.active(cx);
        match event.keystroke.key.as_str() {
            "up" => self.select(active.saturating_sub(1), window, cx),
            "down" => self.select(active + 1, window, cx),
            _ => {}
        }
    }

    // ── 预览、演示、导出 ─────────────────────────────────────────────────────

    /// 草稿变化：120ms 防抖后才把 `[draft]` 交给 [`SlideImages`]（原版 `debounceMs: 120`）。
    fn schedule_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // 替换 Task 即取消上一轮计时。
        self.preview_task = None;
        self.preview_task = Some(cx.spawn_in(window, async move |this, window| {
            // GPUI 自己的计时器（界面里的延时一律不用 tokio，见 README）。
            window.background_executor().timer(PREVIEW_DEBOUNCE).await;
            let _ = this.update_in(window, |this, _window, cx| {
                this.preview_task = None;
                let deck = this.draft.clone();
                // 只发当前这一页：牌组就是 `[draft]`，下标恒为 0。
                this.preview.set_deck(vec![deck]);
                cx.notify();
            });
        }));
    }

    /// 原版 `renderBusy` 的 watch：`pending` 持续超过 150ms 才显示「渲染中…」。
    fn sync_render_busy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pending = self.preview.pending();
        if !pending {
            self.render_busy = false;
            self.preview_probe = None;
            return;
        }
        if self.render_busy || self.preview_probe.is_some() {
            return;
        }
        self.preview_probe = Some(cx.spawn_in(window, async move |this, window| {
            window.background_executor().timer(RENDER_BUSY_DELAY).await;
            let _ = this.update_in(window, |this, _window, cx| {
                this.preview_probe = None;
                this.render_busy = true;
                cx.notify();
            });
        }));
    }

    fn present(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let start = self.active(cx);
        let model = self.model.read(cx);
        // 演示用**内存里的牌组**（含刚新建、还没落库的页），从当前页开始。
        let args = PresentArgs {
            project_id: model.id,
            deck: model.deck.clone(),
            start,
            aspect: model.aspect_ratio(),
        };
        self.handle.present(args, window, cx);
    }

    /// 原版 `WorkflowView.exportPdf`：导出**内存里的牌组**（含未存库的新建页），
    /// 有未应用的修改时结果对话框里加一行提示。
    fn export_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.exporting {
            return;
        }
        let (id, name) = {
            let model = self.model.read(cx);
            (model.id, model.name.clone())
        };
        let Some(id) = id else {
            return;
        };
        let Some(path) = crate::export::ask_save_path(&format!("{name}.pdf"), "PDF", "pdf") else {
            return;
        };
        self.exporting = true;
        cx.notify();
        let shared = self.handle.shared().clone();
        let theme = cx.studio().theme.id.to_string();
        let slides = self.deck(cx);
        let note = self
            .has_unsaved
            .then(|| "编辑器里还有未应用的修改，PDF 按应用前的内容导出。".to_string());
        run_bg(
            window,
            cx,
            move || crate::app::export_pdf(&shared, id, Some(slides), path, theme),
            move |this: &mut Self, result, window, cx| {
                this.exporting = false;
                crate::export::show_report(result, note.as_deref(), window, cx);
                cx.notify();
            },
        );
    }

    // ── 模型变化：应用更改的回执 ─────────────────────────────────────────────

    fn on_model_changed(&mut self, cx: &mut Context<Self>) {
        // 应用更改的回执：`deck_saving` 落下（成功或失败）时处理一次。
        if self.applying && !self.model.read(cx).deck_saving {
            self.applying = false;
            if self.model.read(cx).deck_save_error.is_none() {
                // 成功：草稿 = 落库后的这一页，未保存标记清掉（原版 `applyEditorChanges`）。
                self.draft = current_page(&self.model, cx);
                self.has_unsaved = false;
                self.bump_form();
                // 应用之后 `jsonText`（图表框、图表语义错误）按落库后的这一页重来一遍。
                self.refresh_json_text(cx);
                self.preview_dirty = true;
            }
            // 失败：草稿与未保存标记都留着（错误行由 `deck_save_error` 显示）。
        }
        cx.notify();
    }

    // ── 图标助手（原版 `recommendIcon` / `applySuggestedIcon`） ───────────────

    /// 原版 `buildIconQuery`：`iconQueryKeys` 各字段的值 trim 后非空的用空格连接。
    fn icon_query(&self, base_path: &str, keys: &[String]) -> String {
        let Some(draft) = self.draft.as_ref() else {
            return String::new();
        };
        keys.iter()
            .map(|key| {
                slide_edit::string_value(draft, &format!("{base_path}.{key}"))
                    .trim()
                    .to_string()
            })
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// 原版 `recommendIcon`：查询词空就不发；成功把候选记在这张卡片上，并把**第一个候选**
    /// 写进 `{卡片路径}.icon`（走草稿路径，等「应用更改」才落库，与原版 `updatePath` 一致）。
    fn recommend_icon(
        &mut self,
        base_path: &str,
        keys: &[String],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let query = self.icon_query(base_path, keys);
        if query.is_empty() {
            return;
        }
        // 发起时的身份：页 + 表单代次 + 项目；迟到的结果与它们比对后作废。
        let (seq, page, generation, project) = (
            {
                self.icon_seq += 1;
                self.icon_seq
            },
            self.active(cx),
            self.form_generation,
            self.model.read(cx).id,
        );
        self.icon_loading = Some(base_path.to_string());
        cx.notify();
        let shared = self.handle.shared().clone();
        let base_path = base_path.to_string();
        run_async(
            window,
            cx,
            async move { generation::recommend_icons_for_query(query, Some(6), &shared).await },
            move |this, result, _window, cx| {
                this.finish_icon_recommendation(
                    &base_path, seq, page, generation, project, result, cx,
                )
            },
        );
    }

    /// 推荐请求的每条出口。`icon_loading` 的复位规则：
    /// 1. 成功（身份命中）：记候选、写第一个候选 → 复位；
    /// 2. 失败（身份命中）：候选记空 → 复位；
    /// 3. 成功 / 失败但结果过期（换页、换版式、列表增删移之后）：结果丢掉，只把**自己那一次**
    ///    的「推荐中」复位（期间若又发起了别的卡片，那个请求的标志不动）；
    /// 4. 换页 / 结构变化：`bump_form` 直接清掉标志并 +代次（在途结果从此过期）；
    /// 5. 离开页面：`release` 清掉标志并 +代次。
    #[allow(clippy::too_many_arguments)]
    fn finish_icon_recommendation(
        &mut self,
        base_path: &str,
        seq: u64,
        page: usize,
        generation: u64,
        project: Option<i64>,
        result: Result<Vec<String>, String>,
        cx: &mut Context<Self>,
    ) {
        if self.icon_seq == seq && self.icon_loading.as_deref() == Some(base_path) {
            self.icon_loading = None;
        }
        let stale = seq != self.icon_seq
            || page != self.active(cx)
            || generation != self.form_generation
            || project != self.model.read(cx).id;
        if !stale {
            match result {
                Ok(candidates) => {
                    self.icon_suggestions
                        .insert(base_path.to_string(), candidates.clone());
                    if let Some(first) = candidates.first() {
                        self.set_path(
                            &format!("{base_path}.icon"),
                            Value::String(first.clone()),
                            cx,
                        );
                    }
                }
                // 失败：候选记空（错误不显示，原版也只在 `catch` 里记空）。
                Err(_) => {
                    self.icon_suggestions
                        .insert(base_path.to_string(), Vec::new());
                }
            }
        }
        cx.notify();
    }

    /// 原版 `applySuggestedIcon(cardPath, icon)`：点候选写进这张卡片的 `icon`。
    /// `card_path` 是**卡片**的路径（`cards[0]`），`.icon` 在本函数里拼——候选小片那边传进来
    /// 的也是卡片路径（原版模板传的就是卡片路径）。
    fn apply_suggested_icon(&mut self, base_path: &str, icon: &str, cx: &mut Context<Self>) {
        if self.editing_locked(cx) {
            return;
        }
        self.set_path(
            &format!("{base_path}.icon"),
            Value::String(icon.to_string()),
            cx,
        );
    }

    /// 原版 `.ej-icon-box`：小标签「图标助手」+ 推荐按钮（进行中是「推荐中…」，`renew` 图标）
    /// + 候选小片（图标预览 + 去掉 `i-carbon:` 前缀的名字，点一下写进这张卡片的 `icon`）。
    fn icon_box(
        &mut self,
        base_path: &str,
        keys: &[String],
        locked: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = cx.studio().colors;
        let loading = self.icon_loading.as_deref() == Some(base_path);
        let button = {
            let base_path = base_path.to_string();
            let keys = keys.to_vec();
            widgets::small(widgets::ghost_button(
                SharedString::from(format!("icon-rec:{base_path}")),
                if loading {
                    "推荐中…"
                } else {
                    "AI 推荐图标"
                },
            ))
            .icon(carbon(if loading { "renew" } else { "magic-wand" }))
            .disabled(locked || loading)
            .on_click(cx.listener(move |this, _event, window, cx| {
                this.recommend_icon(&base_path, &keys, window, cx)
            }))
        };
        let mut icon_box = div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .p(px(10.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(c.border)
            // 原版 `.ej-icon-box` 与 `.ej-card-block` 同底（`--studio-panel-2`）。
            .bg(c.panel_2)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(qf_label("图标助手", cx))
                    .child(div().flex_1())
                    .child(button),
            );
        let suggestions = self
            .icon_suggestions
            .get(base_path)
            .cloned()
            .unwrap_or_default();
        if !suggestions.is_empty() {
            let mut chips = div().flex().flex_wrap().gap(px(6.0));
            for icon in suggestions {
                // 候选小片点一下 = 原版 `applySuggestedIcon(cardPath, icon)`：传**卡片**路径，
                // `.icon` 由 `apply_suggested_icon` 拼（这里再拼一次会写成 `…​.icon.icon`，
                // 点上去什么都不发生）。
                let chip_path = base_path.to_string();
                let chip_icon = icon.clone();
                let name = icon
                    .strip_prefix("i-carbon:")
                    .unwrap_or(icon.as_str())
                    .to_string();
                chips = chips.child(
                    div()
                        .id(SharedString::from(format!("icon-chip:{base_path}:{icon}")))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(5.0))
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded(px(999.0))
                        .border_1()
                        .border_color(c.border)
                        // 原版 `.ej-icon-chip` 的底是 `--studio-surface`。
                        .bg(c.surface)
                        .text_size(px(10.5))
                        .text_color(c.text)
                        .cursor_pointer()
                        .when(locked, |chip| chip.opacity(0.5))
                        .child(carbon(&icon).text_color(c.primary))
                        .child(name)
                        .on_click(cx.listener(move |this, _event, _window, cx| {
                            this.apply_suggested_icon(&chip_path, &chip_icon, cx)
                        })),
                );
            }
            icon_box = icon_box.child(chips);
        }
        icon_box.into_any_element()
    }

    // ── 图表数据（JSON）框（原版 `onChartJsonInput` / `scheduleChartCheck` / `runChartCheck`） ──

    /// 原版 `chartJsonFromDraft`：草稿里 `chart` 的两空格缩进 JSON；没有就是空串。
    fn chart_json_from_draft(&self) -> String {
        chart_json_of(self.draft.as_ref())
    }

    /// 原版那条 watch（`[editorKind, getFieldValue('chart')]`）：草稿里的图表一变、且框没有
    /// 焦点时，框换成草稿里的实际内容、错误清掉、等待中的校验作废。
    fn sync_chart_box(&mut self) {
        if self.chart_focused {
            return;
        }
        self.cancel_chart_check();
        self.chart_error.clear();
        self.chart_text = self.chart_json_from_draft();
    }

    /// 换页 / 换版式：框强制重置成草稿里的实际内容，等待中的校验作废。
    ///
    /// 换页一定是先失焦再点（Blur 事件在点击之前），这里再把 `chart_focused` 拍平一次，
    /// 免得框被移出渲染树时 Blur 没到、标志一直挂着。
    fn reset_chart(&mut self, cx: &mut Context<Self>) {
        self.chart_focused = false;
        self.sync_chart_box();
        cx.notify();
    }

    /// 原版 `onChartJsonInput`：语法错立即报错并作废等待中的校验；语法对就排 150ms 防抖。
    fn on_chart_text_changed(&mut self, text: String, cx: &mut Context<Self>) {
        self.chart_text = text;
        match serde_json::from_str::<Value>(&self.chart_text) {
            Err(error) => {
                self.cancel_chart_check();
                self.chart_error = format!("JSON 语法错误：{error}");
            }
            Ok(parsed) => self.schedule_chart_check(parsed, cx),
        }
        cx.notify();
    }

    /// 原版 `scheduleChartCheck`：清掉上一轮计时，记下这次的内容与页（等待期间不写回草稿）。
    fn schedule_chart_check(&mut self, chart: Value, cx: &mut Context<Self>) {
        self.chart_check_task = None;
        self.chart_checking = true;
        self.chart_error.clear();
        self.chart_check_seq += 1;
        let seq = self.chart_check_seq;
        self.chart_check_page = self.active(cx);
        self.pending_chart = Some(chart);
        // 订阅回调里拿不到 window：用 GPUI 自己的计时器（`AsyncApp::background_executor`，
        // 与 `window.background_executor()` 是同一个执行器，不是 tokio sleep）。
        self.chart_check_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(CHART_CHECK_DEBOUNCE).await;
            let _ = this.update(cx, |this, cx| {
                this.chart_check_task = None;
                let (seq, chart, page) = (seq, this.pending_chart.clone(), this.chart_check_page);
                if let Some(chart) = chart {
                    this.run_chart_check(seq, chart, page, cx);
                }
                cx.notify();
            });
        }));
    }

    /// 原版 `cancelChartCheck`：取消计时、清「正在校验」、作废在途结果。
    fn cancel_chart_check(&mut self) {
        self.chart_check_task = None;
        self.chart_checking = false;
        self.pending_chart = None;
        // 作废在途请求：它的结果回来时序号已经不匹配。
        self.chart_check_seq += 1;
    }

    /// 原版 `onChartJsonBlur`：还在等防抖就**立即校验**（粘贴后马上点别处不能丢）；
    /// 没有错误时把框规范化成草稿里的实际内容，有错误保留输入让用户继续改。
    fn on_chart_blur(&mut self, cx: &mut Context<Self>) {
        self.chart_focused = false;
        if self.chart_checking {
            let (seq, chart, page) = (
                self.chart_check_seq,
                self.pending_chart.clone(),
                self.chart_check_page,
            );
            self.chart_check_task = None;
            if let Some(chart) = chart {
                self.run_chart_check(seq, chart, page, cx);
            }
            cx.notify();
            return;
        }
        if self.chart_error.is_empty() {
            self.chart_text = self.chart_json_from_draft();
        }
        cx.notify();
    }

    /// 原版 `runChartCheck`：只有**最新一次输入**、且仍停在发起时那一页的结果才生效；
    /// 只有通过校验的 JSON 才写回草稿（未通过时预览保持上一次合法的图表）。
    ///
    /// 与原版的一处差异：原版经 `invoke('chart_problems')` 走异步命令，可能失败（「校验失败：…」）；
    /// 这里直接调 core 的纯函数（`chart_check::chart_problems_json`），没有失败这条出口。
    fn run_chart_check(&mut self, seq: u64, chart: Value, page: usize, cx: &mut Context<Self>) {
        if seq != self.chart_check_seq {
            return;
        }
        self.chart_checking = false;
        if page != self.active(cx) {
            // 换页之后的迟到结果：不写回（那一页的框已由 `reset_chart` 重置）。
            return;
        }
        let problems: Vec<String> = chart_check::chart_problems_json(&chart)
            .iter()
            .map(|problem| problem.message_zh())
            .collect();
        if !problems.is_empty() {
            self.chart_error = problems.join("；");
            cx.notify();
            return;
        }
        self.chart_error.clear();
        self.set_path("chart", chart, cx);
        // 已失焦：把输入规范化成牌组里的实际内容（内容没变时上面那步不会触发刷新）。
        if !self.chart_focused {
            self.chart_text = self.chart_json_from_draft();
        }
    }

    // ── 高级 Blueprint JSON（原版 `jsonText` / `onEditorJsonInput` / `formatEditorJson`） ──

    /// 原版 `replaceEditorDraft` 的那一半：`jsonText` = 过滤后草稿的两空格缩进 JSON、清错误。
    /// 表单改动 / 换版式 / 换页 / 应用 / 格式化之后都要走一遍。
    fn refresh_json_text(&mut self, cx: &mut Context<Self>) {
        self.json_error.clear();
        self.json_text = self
            .draft
            .as_ref()
            .map(|draft| pretty_json(&slide_edit::filter_slide_json(&self.schema, draft)))
            .unwrap_or_default();
        // 文本框一换：图表框跟着草稿走（原版那条观察 `chart` 的 watch），并重新排图的语义错误
        // （原版观察 `parsedJson.chart` 的 watch）。
        self.sync_chart_box();
        self.schedule_json_chart_check(cx);
    }

    /// 原版 `onEditorJsonInput`：能解析 → 草稿 = 解析结果（**不过滤**）、清错误、未保存、
    /// 表单按新草稿重建；解析失败 → 记错误，草稿不动。
    fn on_json_text_changed(&mut self, text: String, cx: &mut Context<Self>) {
        if text == self.json_text {
            // 程序化写值（`set_value` 不发 Change）或没有变化。
            return;
        }
        self.json_text = text;
        match serde_json::from_str::<Value>(&self.json_text) {
            Ok(parsed) => {
                self.draft = Some(parsed);
                self.json_error.clear();
                self.has_unsaved = true;
                self.bump_form();
                self.preview_dirty = true;
                // 原版那条 watch 也一样会跟着这个新草稿走（图表框 + 图表语义错误）。
                self.sync_chart_box();
                self.schedule_json_chart_check(cx);
            }
            Err(error) => self.json_error = json_error_text(&error),
        }
        cx.notify();
    }

    /// 原版 `formatEditorJson`：能解析就按 `replaceEditorDraft` 规整（会经过 `filter_slide_json`），
    /// 不能解析就不动。
    fn format_json(&mut self, cx: &mut Context<Self>) {
        if self.editing_locked(cx) {
            return;
        }
        let Ok(parsed) = serde_json::from_str::<Value>(&self.json_text) else {
            return;
        };
        self.draft = Some(parsed);
        self.has_unsaved = true;
        self.bump_form();
        self.preview_dirty = true;
        self.refresh_json_text(cx);
        cx.notify();
    }

    /// 原版 `advancedChartErrors`：文本框里的这一页是 `chart_focus` 且有 `chart` 时，
    /// 150ms 防抖后校验图表，问题前面拼 `chart：`；只认最新一次。
    fn schedule_json_chart_check(&mut self, cx: &mut Context<Self>) {
        self.json_chart_task = None;
        self.json_chart_seq += 1;
        let seq = self.json_chart_seq;
        if self.draft_chart().is_none() {
            self.json_chart_errors.clear();
            return;
        }
        self.json_chart_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(CHART_CHECK_DEBOUNCE).await;
            let _ = this.update(cx, |this, cx| {
                this.json_chart_task = None;
                if seq != this.json_chart_seq {
                    return;
                }
                this.json_chart_errors = match this.draft_chart() {
                    None => Vec::new(),
                    Some(chart) => chart_check::chart_problems_json(&chart)
                        .iter()
                        .map(|problem| format!("chart：{}", problem.message_zh()))
                        .collect(),
                };
                cx.notify();
            });
        }));
    }

    /// 语义错误用的图表（原版那条 watch 的取值：只有 `chart_focus` 页、且有 `chart` 才算）。
    fn draft_chart(&self) -> Option<Value> {
        let draft = self.draft.as_ref()?;
        if draft.get("kind").and_then(Value::as_str) != Some("chart_focus") {
            return None;
        }
        match draft.get("chart") {
            None | Some(Value::Null) => None,
            Some(chart) => Some(chart.clone()),
        }
    }

    /// 原版 `syncSemanticErrors`：只在没有语法错误时算（规则在 core：`slide_edit::semantic_errors`）。
    /// 显示时再拼上图表的语义错误（原版 `semanticErrors`）。
    fn semantic_errors(&self) -> Vec<String> {
        if !self.json_error.is_empty() {
            return Vec::new();
        }
        let Some(draft) = self.draft.as_ref() else {
            return Vec::new();
        };
        let mut errors = slide_edit::semantic_errors(&self.schema, draft);
        errors.extend(self.json_chart_errors.iter().cloned());
        errors
    }

    // ── 表单控件（按完整路径缓存 + 订阅） ─────────────────────────────────────

    /// 按路径取（必要时创建）单行 / 多行输入框；创建时初值取草稿。
    fn input_for(
        &mut self,
        path: &str,
        initial: &str,
        placeholder: Option<&str>,
        multiline: bool,
        rows: Option<u16>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> FieldInput {
        if let Some(cached) = self.fields.get(path) {
            if cached.generation == self.form_generation {
                return cached.input.clone();
            }
        }
        let placeholder = placeholder.map(str::to_string);
        let initial = initial.to_string();
        let input = if multiline {
            let rows = rows.unwrap_or(3).max(1) as usize;
            let entity = cx.new(|cx| {
                let state = TextareaState::new(window, cx).rows(rows);
                let state = match &placeholder {
                    Some(text) => state.placeholder(text.clone()),
                    None => state,
                };
                state.default_value(initial.clone())
            });
            let path = path.to_string();
            let sub = cx.subscribe(
                &entity,
                move |this: &mut Self, state: Entity<TextareaState>, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        let value = state.read(cx).value().to_string();
                        this.set_path(&path, Value::String(value), cx);
                    }
                },
            );
            self.field_subs.push(sub);
            FieldInput::Multi(entity)
        } else {
            let entity = cx.new(|cx| {
                let state = InputState::new(window, cx);
                let state = match &placeholder {
                    Some(text) => state.placeholder(text.clone()),
                    None => state,
                };
                state.default_value(initial.clone())
            });
            let path = path.to_string();
            let sub = cx.subscribe(
                &entity,
                move |this: &mut Self, state: Entity<InputState>, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        let value = state.read(cx).value().to_string();
                        this.set_path(&path, Value::String(value), cx);
                    }
                },
            );
            self.field_subs.push(sub);
            FieldInput::Single(entity)
        };
        self.fields.insert(
            path.to_string(),
            CachedField {
                input: input.clone(),
                generation: self.form_generation,
            },
        );
        input
    }

    /// 按路径取（必要时创建）下拉；候选只来自字段的 `options`。
    fn select_for(
        &mut self,
        path: &str,
        value: &str,
        options: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<SelectState<Vec<String>>> {
        if let Some(cached) = self.selects.get(path) {
            if cached.generation == self.form_generation {
                return cached.entity.clone();
            }
        }
        // 没有任何一项选中时显示「请选择」（原版 `<option value="">请选择</option>`）。
        let selected = options
            .iter()
            .position(|option| option == value)
            .map(IndexPath::new);
        let entity = cx.new(|cx| SelectState::new(options, selected, window, cx));
        let key = path.to_string();
        let path = key.clone();
        let sub = cx.subscribe(
            &entity,
            move |this: &mut Self, _state, event: &SelectEvent<Vec<String>>, cx| match event {
                SelectEvent::Confirm(Some(value)) => {
                    this.set_path(&path, Value::String(value.clone()), cx)
                }
                SelectEvent::Confirm(None) => {
                    this.set_path(&path, Value::String(String::new()), cx)
                }
            },
        );
        self.field_subs.push(sub);
        self.selects.insert(
            key,
            CachedSelect {
                entity: entity.clone(),
                generation: self.form_generation,
            },
        );
        entity
    }

    /// 输入框元素：**不回写正在编辑的框**（focused 就不同步值，免得丢光标 / 选区）。
    fn input_element(
        &mut self,
        path: &str,
        value: &str,
        placeholder: Option<&str>,
        multiline: bool,
        rows: Option<u16>,
        locked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let input = self.input_for(path, value, placeholder, multiline, rows, window, cx);
        if !input.is_focused(window, cx) && input.value(cx) != value {
            input.set_value(value.to_string(), window, cx);
        }
        match input {
            FieldInput::Single(state) => Input::new(&state).disabled(locked).into_any_element(),
            FieldInput::Multi(state) => Textarea::new(&state).disabled(locked).into_any_element(),
        }
    }

    /// 一个表单字段（原版 `.ej-struct-field`）。
    fn field_card(
        &mut self,
        field: &EditorField,
        path: &str,
        draft: &Value,
        locked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = cx.studio().colors;
        let mut card = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .min_w_0()
            .p(px(8.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(c.border)
            .bg(c.surface);
        match field.field_type.as_str() {
            // 勾选框自带标签，不再显示上方小标签（原版 `.ej-bool-row`）。
            "boolean" => {
                let checked = slide_edit::bool_value(draft, path);
                let path = path.to_string();
                card = card.child(
                    Checkbox::new(SharedString::from(format!("cb:{path}")))
                        .checked(checked)
                        .label(SharedString::from(field.label.clone()))
                        .disabled(locked)
                        .on_click(cx.listener(move |this, checked: &bool, _window, cx| {
                            this.set_path(&path, Value::Bool(*checked), cx);
                        })),
                );
            }
            "select" => {
                let value = slide_edit::string_value(draft, path);
                let options = field.options.clone().unwrap_or_default();
                let entity = self.select_for(path, &value, options, window, cx);
                card = card.child(qf_label(&field.label, cx)).child(
                    Select::new(&entity)
                        .placeholder("请选择")
                        .cleanable(true)
                        .disabled(locked),
                );
            }
            "textarea" => {
                let value = slide_edit::string_value(draft, path);
                card = card
                    .child(qf_label(&field.label, cx))
                    .child(self.input_element(
                        path,
                        &value,
                        field.placeholder.as_deref(),
                        true,
                        field.rows,
                        locked,
                        window,
                        cx,
                    ));
            }
            _ => {
                let value = slide_edit::string_value(draft, path);
                card = card
                    .child(qf_label(&field.label, cx))
                    .child(self.input_element(
                        path,
                        &value,
                        field.placeholder.as_deref(),
                        false,
                        None,
                        locked,
                        window,
                        cx,
                    ));
            }
        }
        card.into_any_element()
    }

    /// 一组字段（原版 `.ej-structured-grid`：两列，`textarea` 占整行）。
    fn fields_grid(
        &mut self,
        fields: &[EditorField],
        prefix: &str,
        draft: &Value,
        locked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut grid = div().flex().flex_col().gap(px(9.0));
        for row in grid_rows(fields) {
            let mut row_div = div().flex().flex_row().gap(px(9.0));
            for field in row {
                let path = join_path(prefix, &field.key);
                row_div = row_div.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(self.field_card(field, &path, draft, locked, window, cx)),
                );
            }
            grid = grid.child(row_div);
        }
        grid.into_any_element()
    }

    /// 字符串数组（标签组、图集、卡片要点……）：每项一个输入框 + 删除，「添加」。
    fn string_list(
        &mut self,
        draft: &Value,
        path: &str,
        label: &str,
        placeholder: Option<&str>,
        add_label: &str,
        locked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = cx.studio().colors;
        let items: Vec<String> = slide_edit::array_value(draft, path)
            .iter()
            .map(|item| item.as_str().unwrap_or("").to_string())
            .collect();
        let add = {
            let path = path.to_string();
            widgets::small(widgets::ghost_button(
                SharedString::from(format!("add:{path}")),
                add_label.to_string(),
            ))
            .icon(carbon("add"))
            .disabled(locked)
            .on_click(cx.listener(move |this, _event, _window, cx| this.add_string_item(&path, cx)))
        };
        // 标签组 / 图集在原版里**没有**这一行小标签，它们的「添加」按钮放在**列表末尾**
        // （`.ej-list-stack` 里的 `edit-add-btn`，满宽）；卡片里的要点列表才有小标签，
        // 「添加」跟在标签行右端。
        //
        // 已知差异（⚠️ 见 docs/native/audit-step3.md）：原版这个按钮也带
        // `.edit-add-btn { width: 100% }`，在 `.ej-label-row` 里被压成「标签右侧的剩余宽度」、
        // 文字居中（浏览器实测），代价是同行的标签被挤成竖排两行（`.ej-label` 没有 nowrap）。
        // 原生保持紧凑靠右——宽按钮要包一层 `flex_1` + `w_full()`，也仍然复现不了那个折行。
        let mut block = div().flex().flex_col().gap(px(6.0));
        let mut tail_add = None;
        if label.is_empty() {
            tail_add = Some(add);
        } else {
            block = block.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(qf_label(label, cx))
                    .child(div().flex_1())
                    .child(add),
            );
        }
        for (index, value) in items.iter().enumerate() {
            let item_path = format!("{path}[{index}]");
            let del_path = path.to_string();
            block = block.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .p(px(6.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(c.border)
                    .bg(c.surface)
                    .child(div().flex_1().min_w_0().child(self.input_element(
                        &item_path,
                        value,
                        placeholder,
                        false,
                        None,
                        locked,
                        window,
                        cx,
                    )))
                    .child(
                        small_icon_button(format!("del:{item_path}"), "trash-can", "删除", locked)
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                this.remove_string_item(&del_path, index, cx)
                            })),
                    ),
            );
        }
        if let Some(add) = tail_add {
            block = block.child(add.w_full());
        }
        block.into_any_element()
    }

    /// 列表区（原版 `listSections`：区标题 + 添加 + 空提示 + 每项一张卡片）。
    fn list_block(
        &mut self,
        draft: &Value,
        path: &str,
        def: &ListSectionDef,
        hint: Option<&str>,
        locked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let count = slide_edit::array_value(draft, path).len();
        let add = {
            let path = path.to_string();
            let new_item = def.new_item.clone();
            widgets::small(widgets::ghost_button(
                SharedString::from(format!("add:{path}")),
                "添加",
            ))
            .icon(carbon("add"))
            .disabled(locked)
            .on_click(
                cx.listener(move |this, _event, _window, cx| this.add_item(&path, &new_item, cx)),
            )
        };
        let mut block = div().flex().flex_col().gap(px(6.0)).child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(group_label(&def.label, cx))
                .child(div().flex_1())
                .child(add),
        );
        if let Some(hint) = hint {
            block = block.child(hint_text(hint, cx));
        }
        if count == 0 {
            block = block.child(hint_text(&def.empty_text, cx));
        }
        for index in 0..count {
            block = block.child(self.list_card(draft, path, def, index, count, locked, window, cx));
        }
        block.into_any_element()
    }

    /// 列表区里的一张卡片（原版 `.ej-card-block`）。`path` 是数组路径。
    fn list_card(
        &mut self,
        draft: &Value,
        path: &str,
        def: &ListSectionDef,
        index: usize,
        count: usize,
        locked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = cx.studio().colors;
        let item_path = format!("{path}[{index}]");
        let mut card = div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .p(px(10.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(c.border)
            .bg(c.panel_2)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(div().text_size(px(12.0)).text_color(c.text).child(format!(
                        "{} {}",
                        def.label,
                        index + 1
                    )))
                    .child(div().flex_1())
                    .children(list_item_actions(path, index, count, locked, cx)),
            );
        card = card.child(self.fields_grid(&def.fields, &item_path, draft, locked, window, cx));
        // 图标助手（原版 `.ej-icon-box`：`iconQueryKeys` 有值才有这一块）。
        if let Some(keys) = def.icon_query_keys.clone() {
            card = card.child(self.icon_box(&item_path, &keys, locked, cx));
        }
        for string_list in &def.string_lists {
            let string_path = format!("{item_path}.{}", string_list.key);
            card = card.child(self.string_list(
                draft,
                &string_path,
                &string_list.label,
                string_list.placeholder.as_deref(),
                "添加",
                locked,
                window,
                cx,
            ));
        }
        for nested in &def.nested_lists {
            let nested_path = format!("{item_path}.{}", nested.key);
            card = card.child(self.nested_block(draft, &nested_path, nested, locked, window, cx));
        }
        card.into_any_element()
    }

    /// 嵌套列表（原版 `nestedLists`：嵌套卡片 + 删除 + 添加；字段只有 text / textarea）。
    fn nested_block(
        &mut self,
        draft: &Value,
        path: &str,
        def: &NestedListDef,
        locked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = cx.studio().colors;
        let count = slide_edit::array_value(draft, path).len();
        let add = {
            let path = path.to_string();
            let new_item = def.new_item.clone();
            widgets::small(widgets::ghost_button(
                SharedString::from(format!("nadd:{path}")),
                "添加",
            ))
            .icon(carbon("add"))
            .disabled(locked)
            .on_click(
                cx.listener(move |this, _event, _window, cx| this.add_item(&path, &new_item, cx)),
            )
        };
        let mut block = div().flex().flex_col().gap(px(6.0)).child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(qf_label(&def.label, cx))
                .child(div().flex_1())
                .child(add),
        );
        for index in 0..count {
            let item_path = format!("{path}[{index}]");
            let del_path = path.to_string();
            block = block.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .p(px(8.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(c.border)
                    .bg(c.surface)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .child(div().text_size(px(12.0)).text_color(c.text).child(format!(
                                "{} {}",
                                def.label,
                                index + 1
                            )))
                            .child(div().flex_1())
                            .child(
                                small_icon_button(
                                    format!("ndel:{item_path}"),
                                    "trash-can",
                                    "删除",
                                    locked,
                                )
                                .on_click(cx.listener(
                                    move |this, _event, _window, cx| {
                                        this.remove_item(&del_path, index, cx)
                                    },
                                )),
                            ),
                    )
                    .child(self.fields_grid(&def.fields, &item_path, draft, locked, window, cx)),
            );
        }
        block.into_any_element()
    }

    /// 对象区（原版 `objectSections`：字段网格 + 子列表区，路径前缀 `section.key`）。
    fn object_block(
        &mut self,
        draft: &Value,
        def: &ObjectSectionDef,
        locked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut block = div().flex().flex_col().gap(px(6.0)).child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .child(group_label(&def.label, cx)),
        );
        if !def.fields.is_empty() {
            block = block.child(self.fields_grid(&def.fields, &def.key, draft, locked, window, cx));
        }
        for sub in &def.list_sections {
            let path = format!("{}.{}", def.key, sub.key);
            block = block.child(self.list_block(draft, &path, sub, None, locked, window, cx));
        }
        for string_list in &def.string_lists {
            let path = format!("{}.{}", def.key, string_list.key);
            block = block.child(self.string_list(
                draft,
                &path,
                &string_list.label,
                string_list.placeholder.as_deref(),
                "添加",
                locked,
                window,
                cx,
            ));
        }
        block.into_any_element()
    }

    /// 右栏表单（顺序与文案逐字照原版 `Step3Editor.vue` 的模板）。
    fn render_form(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let c = cx.studio().colors;
        let Some(draft) = self.draft.clone() else {
            return hint_text("这一页还没有内容", cx).into_any_element();
        };
        let kind = draft
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let Some(entry) = self.schema.kind(&kind).cloned() else {
            return error_row(&format!("未知版式：{kind}"), cx).into_any_element();
        };
        let locked = self.editing_locked(cx);
        let simple: Vec<EditorField> = self
            .schema
            .simple_fields
            .iter()
            .filter(|field| entry.fields.contains(&field.key))
            .cloned()
            .collect();
        let list_sections: Vec<ListSectionDef> = self.schema.list_sections(&kind).to_vec();
        let object_sections: Vec<ObjectSectionDef> = self.schema.object_sections(&kind).to_vec();

        let mut column = div().flex().flex_col().gap(px(14.0));

        // ① 幻灯片类型
        let change_weak = cx.weak_entity();
        column = column.child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(group_label("幻灯片类型", cx))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .p(px(7.0))
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(c.border)
                        .bg(c.surface)
                        .child(carbon(&entry.icon))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(c.text)
                                .child(entry.label.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(c.muted)
                                .child(kind.clone()),
                        )
                        .child(div().flex_1())
                        .child(
                            widgets::small(widgets::ghost_button("change-kind", "更换"))
                                .icon(carbon("chevron-down"))
                                .disabled(locked)
                                .on_click(move |_event, window, cx: &mut App| {
                                    let _ = change_weak.update(cx, |this, cx| {
                                        this.open_kind_picker(KindPickerMode::Change, window, cx)
                                    });
                                }),
                        ),
                ),
        );

        // ② 编辑模式 + 高级 JSON
        column = column.child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(group_label("编辑模式", cx))
                        .child({
                            let advanced = self.advanced_json;
                            widgets::small(widgets::ghost_button(
                                "toggle-advanced",
                                if advanced {
                                    "收起高级 JSON"
                                } else {
                                    "打开高级 JSON"
                                },
                            ))
                            .icon(carbon(if advanced {
                                "chevron-up"
                            } else {
                                "chevron-down"
                            }))
                            .on_click(cx.listener(
                                |this, _event, _window, cx| {
                                    this.advanced_json = !this.advanced_json;
                                    cx.notify();
                                },
                            ))
                        }),
                )
                .child(hint_text(
                    "默认只编辑文字、结构、图片和图标推荐。JSON 保留给高级调整和排错。",
                    cx,
                )),
        );

        // ③ 基础信息
        if !simple.is_empty() {
            column = column.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(group_label("基础信息", cx))
                            .child(div().flex_1())
                            .child(hint_text("优先编辑用户能理解的内容字段", cx)),
                    )
                    .child(self.fields_grid(&simple, "", &draft, locked, window, cx)),
            );
        }

        // ④ 标签组
        if entry.fields.iter().any(|field| field == "badges") {
            column = column.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(group_label("标签组", cx))
                            .child(div().flex_1())
                            .child(hint_text("用于封面、结束页等标签型内容", cx)),
                    )
                    .child(self.string_list(
                        &draft,
                        "badges",
                        "",
                        Some("标签文案"),
                        "添加标签",
                        locked,
                        window,
                        cx,
                    )),
            );
        }

        // ⑤ 图片素材（原版 `.ej-media-group`：主图 / 图集 + 素材条）
        let show_images = entry.fields.iter().any(|field| field == "images");
        let primary = slide_edit::string_value(&draft, "image");
        if show_images || !primary.is_empty() {
            let mut group = div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(group_label("图片素材", cx))
                        .child(div().flex_1())
                        .child(hint_text("主图直接设置，图集可追加多张", cx)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .p(px(8.0))
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(c.border)
                        .bg(c.surface)
                        .child(qf_label("主图", cx))
                        .child(self.input_element(
                            "image",
                            &primary,
                            Some("media:12 或 https://..."),
                            false,
                            None,
                            locked,
                            window,
                            cx,
                        )),
                );
            if show_images {
                group = group.child(self.string_list(
                    &draft,
                    "images",
                    "",
                    Some("图集中的一张图片"),
                    "添加图集图片",
                    locked,
                    window,
                    cx,
                ));
            }
            // 素材条（原版 `mediaCandidates()`：项目素材里 `type == image` 的每一项一张小卡；
            // 没有图片素材时整条不显示）。
            let images: Vec<(i64, String, String)> = self
                .model
                .read(cx)
                .media
                .iter()
                .filter(|asset| asset.media_type == "image")
                .map(|asset| (asset.id, asset.original_name.clone(), asset.url.clone()))
                .collect();
            if !images.is_empty() {
                let mut strip = div()
                    .id("media-strip")
                    .flex()
                    .flex_wrap()
                    .gap(px(6.0))
                    .p(px(6.0))
                    .bg(c.surface)
                    .border_1()
                    .border_color(c.border)
                    .rounded(px(6.0))
                    .max_h(px(120.0))
                    .overflow_y_scroll();
                for (id, name, url) in images {
                    let media_ref = format!("media:{id}");
                    let primary_ref = media_ref.clone();
                    let mut actions = vec![widgets::small(widgets::ghost_button(
                        SharedString::from(format!("set-primary-{id}")),
                        "设为主图",
                    ))
                    .xsmall()
                    .w_full()
                    .text_size(px(9.0))
                    .disabled(locked)
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.set_path("image", Value::String(primary_ref.clone()), cx)
                    }))
                    .into_any_element()];
                    if show_images {
                        let gallery_ref = media_ref.clone();
                        actions.push(
                            widgets::small(widgets::ghost_button(
                                SharedString::from(format!("add-gallery-{id}")),
                                "加入图集",
                            ))
                            .xsmall()
                            .w_full()
                            .text_size(px(9.0))
                            .disabled(locked)
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                this.append_gallery_image(&gallery_ref, cx)
                            }))
                            .into_any_element(),
                        );
                    }
                    strip = strip.child(widgets::media_tile(
                        SharedString::from(format!("media-tile-{id}")),
                        &url,
                        &name,
                        actions,
                        cx,
                    ));
                }
                group = group.child(strip);
            }
            column = column.child(group);
        }

        // ⑥ 列表区
        for def in &list_sections {
            // issue_stack 的 cards 区上方那句提示按张数二选一（逐字照原版）。
            let hint = (kind == "issue_stack" && def.key == "cards")
                .then(|| issue_stack_hint(&draft, &def.key));
            column = column.child(self.list_block(
                &draft,
                &def.key,
                def,
                hint.as_deref(),
                locked,
                window,
                cx,
            ));
        }

        // ⑦ 对象区
        for def in &object_sections {
            column = column.child(self.object_block(&draft, def, locked, window, cx));
        }

        // ⑧ chart_focus 的图表数据（JSON）（原版 `.ej-chart-json` 一整段）
        if kind == "chart_focus" {
            // 框里的值与本地状态同步（有焦点时不动：用户正在改，不能被草稿覆盖，同 `input_element`）。
            let chart_focused = self
                .chart_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window);
            let shown = self.chart_input.read(cx).value().to_string();
            if !chart_focused && shown != self.chart_text {
                let text = self.chart_text.clone();
                self.chart_input
                    .update(cx, |state, cx| state.set_value(text, window, cx));
            }
            let checking = self.chart_checking;
            let chart_error = self.chart_error.clone();
            column = column.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(group_label("图表数据（JSON）", cx))
                            .child(div().flex_1())
                            .child(hint_text(
                                if checking {
                                    "正在校验…"
                                } else {
                                    "type / data / encoding / series，校验通过才会应用到预览"
                                },
                                cx,
                            )),
                    )
                    // `.ej-wrap`：错误时整块框的边框变红。
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .rounded(px(6.0))
                            .border_1()
                            .border_color(if chart_error.is_empty() {
                                c.border
                            } else {
                                c.error
                            })
                            .bg(c.panel)
                            .overflow_hidden()
                            .child(
                                Textarea::new(&self.chart_input)
                                    .font_family(widgets::code_font(&self.handle))
                                    .text_size(px(12.8))
                                    .disabled(locked),
                            ),
                    )
                    .when(!chart_error.is_empty(), |group| {
                        group.child(error_row(&chart_error, cx))
                    }),
            );
        }

        // ⑨ 高级 Blueprint JSON（原版 `.ej-textarea` + 行号栏；`advancedMode` 打开才有）
        if self.advanced_json {
            // 同上：有焦点时不动（用户正在这个框里打字）。
            let json_focused = self
                .json_editor
                .read(cx)
                .focus_handle(cx)
                .is_focused(window);
            let shown = self.json_editor.read(cx).value().to_string();
            if !json_focused && shown != self.json_text {
                let text = self.json_text.clone();
                self.json_editor
                    .update(cx, |state, cx| state.set_value(text, window, cx));
            }
            let json_error = self.json_error.clone();
            let semantic = self.semantic_errors();
            let mut group = div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(group_label("高级 Blueprint JSON", cx))
                        .child(div().flex_1())
                        .child(hint_text("保留给高级用户和特殊字段", cx))
                        .child(
                            widgets::small(widgets::ghost_button("format-json", "格式化"))
                                .icon(carbon("clean"))
                                .disabled(locked)
                                .on_click(
                                    cx.listener(|this, _event, _window, cx| this.format_json(cx)),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(if json_error.is_empty() {
                            c.border
                        } else {
                            c.error
                        })
                        .bg(c.panel)
                        .overflow_hidden()
                        .child(
                            // 行号栏由代码编辑器自己画（`.ej-gutter` / `.ej-line-num`）。
                            Editor::new(&self.json_editor)
                                .h(px(JSON_EDITOR_H))
                                .font_family(widgets::code_font(&self.handle))
                                .disabled(locked),
                        ),
                );
            if !json_error.is_empty() {
                group = group.child(error_row(&json_error, cx));
            } else if !semantic.is_empty() {
                group = group.child(semantic_errors_block(&semantic, cx));
            }
            column = column.child(group);
        }

        // ⑩ 保存按钮不在这里：它在右栏底部的固定操作区（`render_actions`），不跟着表单滚。
        column.into_any_element()
    }

    /// ⑩ 右栏底部**固定**的操作区：应用更改按钮（三态）+ 生成中只读提示 + 保存失败行。
    ///
    /// 原版把按钮放在右栏的滚动容器里，靠 `WorkflowView.vue:2061-2065` 的
    /// `position: sticky; bottom: 0` 吸底（模板 `Step3Editor.vue:1220`）；GPUI 没有 sticky，
    /// 原生按自己的做法把右栏拆成「可滚动的表单区 + 固定在栏底的操作区」，长表单下按钮
    /// 始终可见；短表单时操作区落在栏底（不模拟原版「按钮紧跟内容」）。
    fn render_actions(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let locked = self.editing_locked(cx);
        let saving = self.saving(cx);
        let unsaved = self.has_unsaved;
        let (label, icon) = if saving {
            ("保存中…", "renew")
        } else if unsaved {
            ("应用更改 (Ctrl+S)", "save")
        } else {
            ("已保存", "checkmark")
        };
        // 原版 `.ej-save-btn` 是右栏里撑满整行的块级按钮；有 JSON 错误时禁用。
        let mut actions = div().flex().flex_col().gap(px(6.0)).child(
            widgets::primary_button("apply-changes", label)
                .icon(carbon(icon))
                .w_full()
                .disabled(saving || !unsaved || locked || !self.json_error.is_empty())
                .on_click(cx.listener(|this, _event, window, cx| this.apply(window, cx))),
        );
        if locked {
            actions = actions.child(hint_text("生成进行中，完成后可编辑", cx));
        }
        if let Some(error) = self.save_error(cx) {
            actions = actions.child(error_row(&error, cx));
        }
        // 左右内边距与表单区（`#editor-panel` 的 `p(12)`）对齐，按钮才与表单同宽。
        div()
            .flex_none()
            .px(px(12.0))
            .pt(px(6.0))
            .pb(px(12.0))
            .child(actions)
            .into_any_element()
    }

    /// 左栏一行（原版 `.edit-slide-row`）。
    fn slide_row(
        &mut self,
        index: usize,
        slide: &Value,
        selected: bool,
        count: usize,
        locked: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = cx.studio().colors;
        let title = slide
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let kind = slide
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let label = self
            .schema
            .kind(&kind)
            .map(|entry| entry.label.clone())
            .unwrap_or_else(|| kind.clone());
        // 行尾垃圾桶**只在悬停该行时显示**（原版 `.edit-slide-row:hover .edit-row-del`；
        // GPUI 用 `group` + `group_hover` 做同一件事），只剩一页时不显示（原版 `disabled` 且透明）。
        let group = SharedString::from(format!("slide-row-{index}"));
        div()
            .id(SharedString::from(format!("slide-row-{index}")))
            .group(group.clone())
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .px(px(8.0))
            .py(px(6.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(if selected {
                c.primary
            } else {
                gpui_kit::transparent_black()
            })
            .when(selected, |row| row.bg(c.primary_bg))
            .cursor_pointer()
            .child(
                div()
                    .w(px(16.0))
                    .text_size(px(10.0))
                    .text_color(c.muted)
                    .child((index + 1).to_string()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(1.0))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(12.0))
                            .text_color(c.text)
                            .child(title),
                    )
                    .child(div().text_size(px(10.0)).text_color(c.muted).child(label)),
            )
            .child(
                widgets::small(widgets::icon_button(
                    SharedString::from(format!("del-row-{index}")),
                    "trash-can",
                    "删除",
                ))
                .disabled(locked || count <= 1)
                .when(count > 1, |button| {
                    button
                        .opacity(0.0)
                        .group_hover(group, |style| style.opacity(1.0))
                })
                // 只剩一页时原版是 `opacity: 0 !important`：一直不显示。
                .when(count <= 1, |button| button.opacity(0.0))
                .on_click(cx.listener(move |this, _event, window, cx| {
                    // 不要冒泡到整行的选中（原版是 `@click.stop`）。
                    cx.stop_propagation();
                    this.delete_slide(index, window, cx)
                })),
            )
            .on_click(cx.listener(move |this, _event, window, cx| this.select(index, window, cx)))
            .into_any_element()
    }
}

/// 当前页的克隆（原版 `blueprints[activeSlide]`）。
fn current_page(model: &Entity<ProjectModel>, cx: &App) -> Option<Value> {
    let model = model.read(cx);
    let last = model.deck.len().saturating_sub(1);
    model.deck.get(model.active_slide.min(last)).cloned()
}

/// `prefix` 为空时就是字段本身（基础信息），否则拼一层。
fn join_path(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

/// 原版 `.ej-structured-grid`：两列；`textarea` 占整行（`grid-column: 1 / -1`）。
fn grid_rows(fields: &[EditorField]) -> Vec<Vec<&EditorField>> {
    let mut rows: Vec<Vec<&EditorField>> = Vec::new();
    let mut pending: Vec<&EditorField> = Vec::new();
    for field in fields {
        if field.field_type == "textarea" {
            if !pending.is_empty() {
                rows.push(std::mem::take(&mut pending));
            }
            rows.push(vec![field]);
        } else {
            pending.push(field);
            if pending.len() == 2 {
                rows.push(std::mem::take(&mut pending));
            }
        }
    }
    if !pending.is_empty() {
        rows.push(pending);
    }
    rows
}

/// `issue_stack` 的 `cards` 区那句提示（按张数二选一，逐字照原版）。
fn issue_stack_hint(draft: &Value, path: &str) -> String {
    if slide_edit::array_value(draft, path).len() > 4 {
        "问题卡片超过 4 张，建议拆成两页，保证说明和关键词完整显示。".to_string()
    } else {
        "2–3 张使用上下结构，4 张使用左右结构。每张保留一句说明和 2–4 个短关键词；说明较长时建议拆页。"
            .to_string()
    }
}

/// 分组标题（原版 `.ej-label`）。
fn group_label(text: &str, cx: &App) -> Div {
    div()
        .text_size(px(11.5))
        .text_color(cx.studio().colors.muted)
        .child(text.to_string())
}

/// 字段小标签（原版 `.ej-qf-label`）。
fn qf_label(text: &str, cx: &App) -> Div {
    div()
        .text_size(px(10.5))
        .text_color(cx.studio().colors.muted)
        .child(text.to_string())
}

/// 弱化说明（原版 `.ej-hint`）。
fn hint_text(text: &str, cx: &App) -> Div {
    div()
        .text_size(px(11.0))
        .text_color(cx.studio().colors.muted)
        .child(text.to_string())
}

/// 语义错误块（原版 `.ej-semantic-errors` / `.ej-semantic-item`：每条前面一个 `warning` 图标）。
fn semantic_errors_block(errors: &[String], cx: &App) -> Div {
    let c = cx.studio().colors;
    div()
        .flex()
        .flex_col()
        .gap(px(3.0))
        .px(px(9.0))
        .py(px(5.0))
        .bg(c.panel_2)
        .border_t_1()
        .border_color(c.border)
        .children(errors.iter().map(|error| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(5.0))
                .text_size(px(10.0))
                .text_color(c.text)
                .opacity(0.72)
                .child(carbon("warning").text_color(c.primary))
                .child(error.clone())
        }))
}

/// `JSON.stringify(value, null, 2)`：两空格缩进的 JSON（草稿 → 文本框，两边一致）。
fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

/// 原版 `chartJsonFromDraft`：草稿里 `chart` 的两空格缩进 JSON；缺了 / 是 `null` 就是空串。
fn chart_json_of(draft: Option<&Value>) -> String {
    match draft.and_then(|draft| draft.get("chart")) {
        None | Some(Value::Null) => String::new(),
        Some(chart) => pretty_json(chart),
    }
}

/// 原版 `.ej-error` 里的「第 N 行：{原因}」：行号取自 serde_json 的报错（没有行号时只给原因）。
fn json_error_text(error: &serde_json::Error) -> String {
    let message = error.to_string();
    // serde_json 在消息末尾附了「 at line N column M」，行号由我们自己显示，去掉这段尾巴。
    let reason = match message.find(" at line ") {
        Some(at) => &message[..at],
        None => message.as_str(),
    };
    if error.line() == 0 {
        reason.to_string()
    } else {
        format!("第 {} 行：{reason}", error.line())
    }
}

/// 错误行（原版 `.ej-error`）。
fn error_row(text: &str, cx: &App) -> Div {
    let c = cx.studio().colors;
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(6.0))
        .p(px(7.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(c.error_border)
        .bg(c.error_bg)
        .text_size(px(11.5))
        .text_color(c.error)
        .child(carbon("warning-filled"))
        .child(div().flex_1().child(text.to_string()))
}

/// 卡片头上的小图标按钮（上移 / 下移 / 删除）。
fn small_icon_button(
    id: impl Into<gpui_kit::ElementId>,
    icon: &str,
    tooltip: &str,
    locked: bool,
) -> Button {
    widgets::small(widgets::icon_button(id, icon, tooltip).disabled(locked))
}

/// 列表项的「上移 / 下移 / 删除」（原版 `.edit-json-acts` 三个图标按钮）。
fn list_item_actions(
    path: &str,
    index: usize,
    count: usize,
    locked: bool,
    cx: &mut Context<Step3>,
) -> Vec<AnyElement> {
    let up_path = path.to_string();
    let down_path = path.to_string();
    let del_path = path.to_string();
    vec![
        small_icon_button(
            SharedString::from(format!("up:{path}[{index}]")),
            "arrow-up",
            "上移",
            locked || index == 0,
        )
        .on_click(
            cx.listener(move |this, _event, _window, cx| this.move_item(&up_path, index, -1, cx)),
        )
        .into_any_element(),
        small_icon_button(
            SharedString::from(format!("down:{path}[{index}]")),
            "arrow-down",
            "下移",
            locked || index + 1 >= count,
        )
        .on_click(
            cx.listener(move |this, _event, _window, cx| this.move_item(&down_path, index, 1, cx)),
        )
        .into_any_element(),
        small_icon_button(
            SharedString::from(format!("del:{path}[{index}]")),
            "trash-can",
            "删除",
            locked,
        )
        .on_click(
            cx.listener(move |this, _event, _window, cx| this.remove_item(&del_path, index, cx)),
        )
        .into_any_element(),
    ]
}

/// 栏头（原版 `.edit-col-head`：同高、同底色，三栏连成一条）。
fn col_head(icon: &str, title: &str, cx: &App) -> Div {
    let c = cx.studio().colors;
    div()
        .flex_none()
        .h(px(COL_HEAD_H))
        .px(px(12.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .bg(c.panel_2)
        .border_b_1()
        .border_color(c.border)
        .text_size(px(11.5))
        .text_color(c.muted)
        .child(carbon(icon))
        .child(title.to_string())
}

/// KindPicker 的内容（外框由 `Dialog` 画）。`after` 是「插入到第 N 张之后」的 N。
fn kind_picker_body(
    mode: KindPickerMode,
    kinds: &[KindEntry],
    current: &str,
    after: usize,
    weak: WeakEntity<Step3>,
    cx: &App,
) -> AnyElement {
    let c = cx.studio().colors;
    let title = match mode {
        KindPickerMode::Add => "新建幻灯片 — 选择类型",
        KindPickerMode::Change => "更换幻灯片类型",
    };
    let icon = match mode {
        KindPickerMode::Add => "add-alt",
        KindPickerMode::Change => "arrows-horizontal",
    };
    let sub = match mode {
        KindPickerMode::Add => format!("新幻灯片将插入到第 {after} 张之后"),
        KindPickerMode::Change => "选择新类型后将自动保留可迁移的公共字段".to_string(),
    };
    let head = div()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .px(px(16.0))
        .py(px(12.0))
        .border_b_1()
        .border_color(c.border)
        .child(carbon(icon))
        .child(div().text_size(px(14.0)).text_color(c.text).child(title))
        .child(div().flex_1())
        .child({
            let weak = weak.clone();
            widgets::icon_button("kp-close", "close", "关闭").on_click(
                move |_event, window, cx: &mut App| {
                    let _ = weak.update(cx, |this, cx| {
                        this.kind_picker = None;
                        cx.notify();
                    });
                    window.close_dialog(cx);
                },
            )
        });
    let mut grid = div().flex().flex_col().gap(px(8.0));
    for chunk in kinds.chunks(3) {
        let mut row = div().flex().flex_row().gap(px(8.0));
        for entry in chunk {
            let is_current = mode == KindPickerMode::Change && entry.kind == current;
            let weak = weak.clone();
            let kind = entry.kind.clone();
            row = row.child(
                div()
                    .id(SharedString::from(format!("kp-{}", entry.kind)))
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(4.0))
                    .p(px(10.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(if is_current { c.primary } else { c.border })
                    .bg(if is_current { c.primary_bg } else { c.surface })
                    // 原版 `.kp-card:hover { border-color: var(--studio-primary); background: var(--studio-primary-bg) }`。
                    .hover(|style| style.border_color(c.primary).bg(c.primary_bg))
                    .cursor_pointer()
                    // 原版 `.kp-card-icon { font-size: 1.5rem }`（1.5rem = 24px）：
                    // 图标默认跟随文字大小（这里只有 ~14px），显式给 1.5rem。
                    .child(carbon(&entry.icon).with_size(Size::Large))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(c.text)
                            .child(entry.label.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(c.muted)
                            .child(entry.kind.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(c.muted)
                            .child(entry.desc.clone()),
                    )
                    .on_click(move |_event, window, cx: &mut App| {
                        let _ = weak.update(cx, |this, cx| {
                            this.pick_kind(mode, &kind, window, cx);
                        });
                        window.close_dialog(cx);
                    }),
            );
        }
        // 每行补齐到三列，避免最后一行被拉宽。
        for _ in chunk.len()..3 {
            row = row.child(div().flex_1());
        }
        grid = grid.child(row);
    }
    div()
        .w_full()
        .flex()
        .flex_col()
        .child(head)
        .child(
            div()
                .px(px(16.0))
                .py(px(10.0))
                .text_size(px(11.5))
                .text_color(c.muted)
                .border_b_1()
                .border_color(c.border)
                .child(sub),
        )
        .child(
            div()
                .id("kp-grid")
                .max_h(px(560.0))
                .overflow_y_scroll()
                .p(px(14.0))
                .child(grid),
        )
        .into_any_element()
}

impl Render for Step3 {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 观察者 / 订阅里攒下的待办（那里拿不到 window）。
        if std::mem::take(&mut self.preview_dirty) {
            self.schedule_preview(window, cx);
        }
        let c = cx.studio().colors;
        let deck = self.deck(cx);
        let count = deck.len();
        // 渲染里不改模型（那会在渲染途中触发观察者）；越界只在这一帧按最后一页显示，
        // 真正的收敛在 `on_generation_changed` 与 `commit_deck` 里。
        let selected = self.active(cx);
        let locked = self.editing_locked(cx);

        // ── 舞台：中栏内容区里取最大的、保持比例的框，四周留 16px ──
        // 原版按这一页的 `aspect_ratio`（缺省 16:9）；这里以草稿的比例为准、回落项目比例。
        let draft_aspect = self
            .draft
            .as_ref()
            .and_then(|draft| draft.get("aspect_ratio"))
            .and_then(Value::as_str)
            .and_then(parse_aspect)
            .map(|aspect| aspect.height() as f32 / aspect.width() as f32);
        let aspect = draft_aspect.unwrap_or_else(|| self.model.read(cx).aspect_ratio());
        let clipped = self.preview.overflow(0);
        let clip_h = if clipped { CLIP_HINT_H } else { 0.0 };
        let viewport = window.viewport_size();
        let avail_w =
            (viewport.width.as_f32() - SIDEBAR_W - LIST_W - PANEL_W - 2.0 * STAGE_PADDING)
                .max(160.0);
        let avail_h = (viewport.height.as_f32()
            - crate::theme::TITLEBAR_HEIGHT
            - COL_HEAD_H
            - 2.0 * STAGE_PADDING
            - clip_h)
            .max(90.0);
        let (mut stage_w, mut stage_h) = (avail_w, avail_w * aspect);
        if stage_h > avail_h {
            stage_h = avail_h;
            stage_w = stage_h / aspect;
        }

        // 预览：只发当前这一页（下标恒为 0），宽度 = 舞台显示宽。
        self.preview.set_width(stage_w, window);
        self.preview.set_wanted(&[0]);
        self.sync_render_busy(window, cx);
        self.preview
            .pump(Some(0), |this: &mut Self| &mut this.preview, window, cx);
        let rendering = self.render_busy && self.preview.pending();

        // ── 左栏 ──
        let regen_weak = cx.weak_entity();
        let list = div()
            .flex_none()
            .w(px(LIST_W))
            .h_full()
            .flex()
            .flex_col()
            .bg(c.panel)
            .border_r_1()
            .border_color(c.border)
            .child(
                col_head("layers", "幻灯片", cx)
                    .child(widgets::chip(count.to_string(), Tone2::Muted, cx))
                    .child(div().flex_1())
                    .child(
                        widgets::small(widgets::icon_button("regen-slides", "reset", "重新生成"))
                            .on_click(move |_event, window, cx: &mut App| {
                                let _ =
                                    regen_weak.update(cx, |this, cx| this.go_to_step_1(window, cx));
                            }),
                    ),
            )
            .child(
                div()
                    .id("slide-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(6.0))
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .children(deck.iter().enumerate().map(|(index, slide)| {
                        self.slide_row(index, slide, index == selected, count, locked, cx)
                    })),
            )
            .child(
                div()
                    .flex_none()
                    .p(px(8.0))
                    .border_t_1()
                    .border_color(c.border)
                    .child(
                        widgets::ghost_button("new-slide", "新建幻灯片")
                            .icon(carbon("add"))
                            .disabled(locked)
                            .on_click({
                                let weak = cx.weak_entity();
                                move |_event, window, cx: &mut App| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.open_kind_picker(KindPickerMode::Add, window, cx)
                                    });
                                }
                            }),
                    ),
            );

        // ── 中栏（预览） ──
        let (prev_weak, next_weak, present_weak, export_weak) = (
            cx.weak_entity(),
            cx.weak_entity(),
            cx.weak_entity(),
            cx.weak_entity(),
        );
        let exporting = self.exporting;
        let unsaved = self.has_unsaved;
        let stage = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(c.bg)
            .border_r_1()
            .border_color(c.border)
            .child(
                col_head("view", "实时预览", cx)
                    .when(unsaved, |head| {
                        head.child(div().text_size(px(11.5)).child("● 未保存"))
                    })
                    .when(rendering, |head| {
                        head.child(div().text_size(px(11.0)).child("渲染中…"))
                    })
                    .child(div().flex_1())
                    .child(
                        widgets::small(widgets::icon_button(
                            "preview-prev",
                            "arrow-left",
                            "上一张",
                        ))
                        .disabled(selected == 0)
                        .on_click(move |_event, window, cx: &mut App| {
                            let _ = prev_weak.update(cx, |this, cx| {
                                let index = this.active(cx).saturating_sub(1);
                                this.select(index, window, cx);
                            });
                        }),
                    )
                    .child(
                        div()
                            .min_w(px(40.0))
                            .text_center()
                            .text_size(px(11.0))
                            .child(format!("{} / {}", selected + 1, count)),
                    )
                    .child(
                        widgets::small(widgets::icon_button(
                            "preview-next",
                            "arrow-right",
                            "下一张",
                        ))
                        .disabled(selected + 1 >= count)
                        .on_click(move |_event, window, cx: &mut App| {
                            let _ = next_weak.update(cx, |this, cx| {
                                let index = this.active(cx) + 1;
                                this.select(index, window, cx);
                            });
                        }),
                    )
                    .child(
                        widgets::small(widgets::ghost_button("export-pdf", "导出 PDF"))
                            .icon(carbon(if exporting { "renew" } else { "document-pdf" }))
                            // 原版 `title="导出为 PDF 文档"`。
                            .tooltip("导出为 PDF 文档")
                            .disabled(exporting || count == 0)
                            .on_click(move |_event, window, cx: &mut App| {
                                let _ =
                                    export_weak.update(cx, |this, cx| this.export_pdf(window, cx));
                            }),
                    )
                    .child(
                        widgets::primary_button("present", "演示")
                            .small()
                            .icon(carbon("presentation-file"))
                            .disabled(count == 0)
                            .on_click(move |_event, window, cx: &mut App| {
                                let _ =
                                    present_weak.update(cx, |this, cx| this.present(window, cx));
                            }),
                    ),
            )
            .child(
                div()
                    .id("preview-stage-wrap")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .p(px(STAGE_PADDING))
                    // 点预览区把焦点拿回根节点，否则方向键不灵。
                    .on_click(cx.listener(|this, _event, window, cx| {
                        this.focus.focus(window, cx);
                    }))
                    .child(slide_frame(
                        "editor-stage",
                        self.preview.frame_state(0),
                        stage_w,
                        stage_h,
                        false,
                        Some(Box::new({
                            let this = cx.weak_entity();
                            move |_window, cx| {
                                let _ = this.update(cx, |this, cx| {
                                    this.preview.refresh();
                                    cx.notify();
                                });
                            }
                        })),
                        cx,
                    )),
            )
            // 原版：内容缩到最小仍放不下时，预览下方提示。
            .when(clipped, |col| {
                col.child(
                    div()
                        .flex_none()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .px(px(16.0))
                        .pb(px(10.0))
                        .text_size(px(11.5))
                        .text_color(c.warning)
                        .child(carbon("warning-alt"))
                        .child("内容过多，已缩到最小字号仍放不下，超出部分被裁切"),
                )
            });

        // ── 右栏（表单） ──
        let (up_weak, down_weak, del_weak) = (cx.weak_entity(), cx.weak_entity(), cx.weak_entity());
        let mut panel = div()
            .flex_none()
            .w(px(PANEL_W))
            .h_full()
            .flex()
            .flex_col()
            .bg(c.panel)
            .child(
                col_head("edit", &format!("编辑 第 {} 张", selected + 1), cx)
                    .child(div().flex_1())
                    .child(
                        widgets::small(widgets::icon_button("slide-up", "arrow-up", "上移"))
                            .disabled(locked || selected == 0)
                            .on_click(move |_event, window, cx: &mut App| {
                                let _ =
                                    up_weak.update(cx, |this, cx| this.move_slide(-1, window, cx));
                            }),
                    )
                    .child(
                        widgets::small(widgets::icon_button("slide-down", "arrow-down", "下移"))
                            .disabled(locked || selected + 1 >= count)
                            .on_click(move |_event, window, cx: &mut App| {
                                let _ =
                                    down_weak.update(cx, |this, cx| this.move_slide(1, window, cx));
                            }),
                    )
                    .child(
                        // 原版 `.edit-del-btn { color: var(--studio-error) }`：栏头这三个按钮里
                        // 只有删除是红的（`.edit-json-acts` 里的卡片按钮仍是默认色）。
                        widgets::small(
                            Button::new("slide-del")
                                .ghost()
                                .icon(carbon("trash-can").text_color(c.error))
                                .tooltip("删除"),
                        )
                        .disabled(locked || count <= 1)
                        .on_click(move |_event, window, cx: &mut App| {
                            let _ = del_weak.update(cx, |this, cx| {
                                let index = this.active(cx);
                                this.delete_slide(index, window, cx)
                            });
                        }),
                    ),
            )
            .child(
                div()
                    .id("editor-panel")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(12.0))
                    .child(self.render_form(window, cx)),
            );
        // 操作区固定在栏底，不参与上面那块滚动（没有草稿＝这一页没内容时不画）。
        if self.draft.is_some() {
            panel = panel.child(self.render_actions(cx));
        }

        div()
            .id("step3")
            .track_focus(&self.focus)
            .on_key_down(
                cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    this.on_key(event, window, cx)
                }),
            )
            .size_full()
            .flex()
            .flex_row()
            .child(list)
            .child(stage)
            .child(panel)
    }
}
