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
//! 本轮不做（留 `widgets::todo_block`）：图标推荐、素材插图（缩略图 / 设为主图 / 加入图集）、
//! 图表 JSON 框、高级 Blueprint JSON 模式。
//!
//! GPUI 下的三处做法（原版机制写在各自注释里）：
//! - 输入框是有状态的实体，不能每帧新建：按**完整路径**（`cards[2].title`）缓存 + 每框订阅，
//!   结构变化时用「表单代次」整组重建（见 `Step3::bump_form`）；
//! - 换页清图用 `SlideImages::reset`（不是重建 `SlideImages`）；
//! - 键盘只在第三步根节点**自己有焦点**时换页（`focus_handle.is_focused`，焦点在输入框里就不处理）。

use std::collections::HashMap;
use std::time::Duration;

use gpui_kit::component::button::Button;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{Disableable as _, IndexPath, Sizable as _, WindowExt as _};
use gpui_kit::prelude::*;
use gpui_kit::{
    div, px, AnyElement, App, Context, Div, Entity, FocusHandle, Focusable as _, KeyDownEvent,
    Render, SharedString, Subscription, Task, WeakEntity, Window,
};
use huanshu_core::editor_schema::{
    self, EditorField, EditorSchema, KindEntry, ListSectionDef, NestedListDef, ObjectSectionDef,
};
use huanshu_core::preview::PreviewOptions;
use huanshu_core::slide_edit;
use serde_json::Value;

use super::{parse_aspect, ProjectModel, WorkflowPage};
use crate::icons::carbon;
use crate::present::PresentArgs;
use crate::router::Step;
use crate::shell::ShellHandle;
use crate::slides::{slide_frame, SlideImages};
use crate::tasks::run_bg;
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
        let draft = current_page(&model, cx);
        let subscriptions = vec![cx.observe(&model, |this, _model, cx| this.on_model_changed(cx))];
        Self {
            model,
            page,
            handle,
            schema: editor_schema::editor_schema(),
            preview,
            exporting: false,
            focus,
            draft,
            has_unsaved: false,
            applying: false,
            advanced_json: false,
            kind_picker: None,
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

    pub fn release(&mut self, cx: &mut Context<Self>) {
        self.preview.release(cx);
        self.preview_task = None;
        self.preview_probe = None;
        self.kind_picker = None;
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
        cx.notify();
    }

    /// 表单代次 +1：整组清空控件缓存与订阅，下一帧按新草稿重建。
    fn bump_form(&mut self) {
        self.form_generation += 1;
        self.fields.clear();
        self.selects.clear();
        self.field_subs.clear();
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

    /// 原版 `applyEditorChanges`：草稿经 `filter_slide_json` 写回牌组当前页，整副落库。
    fn apply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.has_unsaved || self.saving(cx) || self.editing_locked(cx) {
            return;
        }
        let Some(draft) = self.draft.clone() else {
            return;
        };
        let active = self.active(cx);
        let mut deck = self.deck(cx);
        if active >= deck.len() {
            return;
        }
        deck[active] = slide_edit::filter_slide_json(&self.schema, &draft);
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
                self.preview_dirty = true;
            }
            // 失败：草稿与未保存标记都留着（错误行由 `deck_save_error` 显示）。
        }
        cx.notify();
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
        // 标签组 / 图集在原版里**没有**这一行小标签（只有卡片里的要点列表有）。
        let mut block = div().flex().flex_col().gap(px(6.0)).child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .when(!label.is_empty(), |row| row.child(qf_label(label, cx)))
                .child(div().flex_1())
                .child(add),
        );
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
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(gpui_kit::FontWeight::BOLD)
                            .text_color(c.text)
                            .child(format!("{} {}", def.label, index + 1)),
                    )
                    .child(div().flex_1())
                    .children(list_item_actions(path, index, count, locked, cx)),
            );
        card = card.child(self.fields_grid(&def.fields, &item_path, draft, locked, window, cx));
        // 图标助手：本轮留占位（下一轮做）。
        if def.icon_query_keys.is_some() {
            card = card.child(widgets::todo_block(
                "图标助手",
                "Step3Editor.vue 的 `.ej-icon-box`（`recommendIcon`）",
                &[
                    "调 core 的图标推荐（`recommend_icons_for_query`，查询词由 iconQueryKeys 拼）",
                    "候选芯片点一下写进这张卡片的 `icon`",
                ],
                cx,
            ));
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
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .text_color(c.text)
                                    .child(format!("{} {}", def.label, index + 1)),
                            )
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
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
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
                            .icon(carbon(if advanced { "chevron-up" } else { "chevron-down" }))
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.advanced_json = !this.advanced_json;
                                cx.notify();
                            }))
                        }),
                )
                .child(hint_text(
                    "默认只编辑文字、结构、图片和图标推荐。JSON 保留给高级调整和排错。",
                    cx,
                ))
                .when(self.advanced_json, |group| {
                    group.child(widgets::todo_block(
                        "高级 Blueprint JSON",
                        "Step3Editor.vue 的 `.ej-textarea`（`onEditorJsonInput` / `formatEditorJson`）",
                        &[
                            "整页 JSON 编辑框（替换草稿）、行号栏、语法错误行高亮",
                            "Ctrl+S 在文本框里也应用更改（本轮已在第三步任何位置生效）",
                        ],
                        cx,
                    ))
                }),
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

        // ⑤ 图片素材（本轮只做主图 / 图集的文本输入，素材条留占位）
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
                group = group.child(widgets::todo_block(
                    "素材插图",
                    "Step3Editor.vue 的 `.ej-media-strip`（`setPrimaryImage` / `appendGalleryImage`）",
                    &[
                        "项目素材的缩略图条（只列图片类素材）",
                        "「设为主图」「加入图集」两个按钮",
                    ],
                    cx,
                ));
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

        // ⑧ chart_focus 的图表数据（本轮留占位）
        if kind == "chart_focus" {
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
                                "type / data / encoding / series，校验通过才会应用到预览",
                                cx,
                            )),
                    )
                    .child(widgets::todo_block(
                        "图表数据（JSON）",
                        "Step3Editor.vue 的 `.ej-chart-json`（`onChartJsonInput` / `runChartCheck`）",
                        &[
                            "150ms 防抖调 `chart_check::chart_problems_json`，通过才写回草稿",
                            "「正在校验…」与校验失败行（失焦立即校验）",
                        ],
                        cx,
                    )),
            );
        }

        // ⑨ 保存按钮（三态）+ 保存失败行
        let saving = self.saving(cx);
        let unsaved = self.has_unsaved;
        let (label, icon) = if saving {
            ("保存中…", "renew")
        } else if unsaved {
            ("应用更改 (Ctrl+S)", "save")
        } else {
            ("已保存", "checkmark")
        };
        let mut save_group = div().flex().flex_col().gap(px(6.0)).child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(
                    widgets::primary_button("apply-changes", label)
                        .icon(carbon(icon))
                        .disabled(saving || !unsaved || locked)
                        .on_click(cx.listener(|this, _event, window, cx| this.apply(window, cx))),
                )
                .when(locked, |row| {
                    row.child(hint_text("生成进行中，完成后可编辑", cx))
                }),
        );
        if let Some(error) = self.save_error(cx) {
            save_group = save_group.child(error_row(&error, cx));
        }
        column = column.child(save_group);

        column.into_any_element()
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
                    .font_weight(gpui_kit::FontWeight::BOLD)
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
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
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
        .font_weight(gpui_kit::FontWeight::BOLD)
        .text_color(cx.studio().colors.muted)
        .child(text.to_string())
}

/// 字段小标签（原版 `.ej-qf-label`）。
fn qf_label(text: &str, cx: &App) -> Div {
    div()
        .text_size(px(10.5))
        .font_weight(gpui_kit::FontWeight::BOLD)
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
        .font_weight(gpui_kit::FontWeight::BOLD)
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
        .child(
            div()
                .text_size(px(14.0))
                .font_weight(gpui_kit::FontWeight::BOLD)
                .text_color(c.text)
                .child(title),
        )
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
                    .cursor_pointer()
                    .child(carbon(&entry.icon))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(gpui_kit::FontWeight::BOLD)
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
        let panel = div()
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
                        widgets::small(widgets::icon_button("slide-del", "trash-can", "删除"))
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
