//! 第二步「AI 生成」（原版 `views/steps/Step2Generate.vue` + `components/RetryLayoutDialog.vue`）。
//!
//! 与 Tauri 版逐字对照：头部（状态圆点、四态标题、三个统计）、整体进度（五个阶段、进度条、
//! 「查看详情」展开后的两段说明与四列表格）、看板（空状态、卡片、只给可见卡片要图的虚拟列表）、
//! 右侧详情（规划 / 布局 / 内容 / 调试信息 / 最近日志）、失败段的单页重试与换版式重试、
//! 底栏（前往编辑 / 返回修改 / 重新生成全部）。
//!
//! 规则（看板条目、逐页状态、整体进度、三张文案表）一律调 `huanshu_core::generation_progress`，
//! 界面里不写规则；生成状态一律来自应用级的 [`GenerationStore`]（原版 pinia 的
//! `generation` store，见 `src/generation.rs`），本页只读它、不存生成状态。
//!
//! GPUI 下的三处做法（原版机制写在各自注释里）：
//! - 看板用 `uniform_list`（一行 = 一行卡片）**只给可见行**的卡片要图
//!   （`SlideImages::set_wanted`），滚出视野的图交给 `PreviewCache` 的缓存上限淘汰；
//! - 两个 `<details>` 折叠段用「点标题展开/收起」；
//! - 生成中的实时点与状态圆点脉冲用 GPUI 的重复动画（原版是 CSS keyframes）。

use std::ops::Range;
use std::time::Duration;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{Disableable as _, IndexPath, WindowExt as _};
use gpui_kit::prelude::*;
use gpui_kit::{
    div, px, relative, uniform_list, Animation, AnimationExt as _, AnyElement, App, Context,
    Entity, FocusHandle, FontWeight, Hsla, KeyDownEvent, Render, ScrollStrategy, SharedString,
    Subscription, UniformListScrollHandle, WeakEntity, Window,
};
use huanshu_core::editor_schema::{self, EditorSchema};
use huanshu_core::events::GenerationLogEvent;
use huanshu_core::generation_progress::{
    self as progress, PageProgressStatus, SlideProgressItem, SlideStatus,
};
use huanshu_core::preview::PreviewOptions;
use huanshu_core::services::generation;
use huanshu_core::types::{AspectRatio, SlideKind};
use serde_json::Value;

use super::{parse_aspect, ProjectModel, WorkflowPage};
use crate::generation::GenerationStore;
use crate::icons::carbon;
use crate::router::Step;
use crate::shell::ShellHandle;
use crate::slides::{slide_frame, FrameState, SlideImages};
use crate::tasks::run_bg;
use crate::theme::StudioExt as _;
use crate::widgets::{self, Tone2};

/// 原版 `.slides-grid`：最小列宽、间距、内边距（CSS 逻辑像素）。
const GRID_MIN_COL: f32 = 260.0;
const GRID_GAP: f32 = 16.0;
const GRID_PAD: f32 = 16.0;
/// 原版 `.slide-progress-card`：内边距 `0.9rem`、间距 `0.85rem`、边框 1px。
const CARD_PAD: f32 = 14.4;
const CARD_GAP: f32 = 13.6;
const CARD_BORDER: f32 = 1.0;
/// 卡片四行的高度（原版实测）。写死高度是 `uniform_list` 的前提：一行里的卡片同宽同高，
/// 行与行之间也是等高的，行才排得齐（原版靠 `align-items: start` 容忍参差）。
const CARD_TOP_H: f32 = 23.0;
const CARD_STEPS_H: f32 = 22.0;
const CARD_TAG_H: f32 = 24.0;
/// 原版 `.slides-workbench`：`gap: 1rem`、详情栏 `minmax(340px, 0.9fr)`、看板栏 `1.5fr`。
const PANEL_GAP: f32 = 16.0;
const DETAIL_MIN_W: f32 = 340.0;
/// 详情面板的内边距（原版 `.sdp-head` / `.sdp-preview-shell` / `.sdp-body` 的 `padding: 1rem`）。
const PANEL_PAD: f32 = 16.0;
/// 工作流侧栏宽度（与 `pages/workflow/mod.rs` 的 `sidebar` 一致）。
const SIDEBAR_W: f32 = 204.0;
/// 看板卡片上的五个阶段短名（原版 `PAGE_STAGES` 的顺序）。
const STEP_LABELS: [&str; 5] = ["规划", "布局", "内容", "规范", "校验"];

pub struct Step2 {
    model: Entity<ProjectModel>,
    /// 应用级生成状态（原版 `stores/generation.ts`）：页面只读它，状态不在这里。
    store: Entity<GenerationStore>,
    handle: ShellHandle,
    page: WeakEntity<WorkflowPage>,
    /// 原版 `selectedSlideIndex`（`None` = 还没选，按 `preferredSlideIndex` 取）。
    selected: Option<usize>,
    /// 上一帧算出来的看板条目数与列数（键盘换页要用）。
    board_count: usize,
    columns: usize,
    /// 原版 `progressDetailsOpen` 与两个 `<details>` 的展开状态。
    progress_open: bool,
    debug_open: bool,
    logs_open: bool,
    thumbs: SlideImages,
    detail: SlideImages,
    /// 看板的滚动句柄（换页时把选中的卡片滚进可见区；原版没有这一步）。
    list: UniformListScrollHandle,
    /// 看板当前可见的**行**范围：`uniform_list` 每帧在布局阶段报告，用来只给可见卡片要图。
    visible_rows: (usize, usize),
    focus: FocusHandle,
    /// 「选择重试布局」对话框（打开时持有；关掉之后迟到的结果按版本号作废）。
    retry_dialog: Option<Entity<RetryLayoutDialog>>,
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,
}

impl Step2 {
    pub fn new(
        model: Entity<ProjectModel>,
        handle: ShellHandle,
        page: WeakEntity<WorkflowPage>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let id = model.read(cx).id;
        let store = handle.generation();
        let shared = handle.shared().clone();
        let subscriptions = vec![
            cx.observe(&model, |_this, _model, cx| cx.notify()),
            cx.observe(&store, |_this, _store, cx| cx.notify()),
        ];
        // 原版把 `handleStepKeydown` 挂在 window 上；这里让看板自己拿焦点（见 README 的按键规矩）。
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            model,
            store,
            handle,
            page,
            selected: None,
            board_count: 0,
            columns: 1,
            progress_open: false,
            debug_open: false,
            logs_open: false,
            thumbs: SlideImages::new(
                shared.clone(),
                id,
                PreviewOptions {
                    max_cached: Some(64),
                    ..PreviewOptions::default()
                },
            ),
            detail: SlideImages::new(
                shared,
                id,
                PreviewOptions {
                    max_cached: Some(4),
                    ..PreviewOptions::default()
                },
            ),
            list: UniformListScrollHandle::new(),
            visible_rows: (0, 0),
            focus,
            retry_dialog: None,
            subscriptions,
        }
    }

    pub fn release(&mut self, cx: &mut Context<Self>) {
        self.thumbs.release(cx);
        self.detail.release(cx);
    }

    /// 原版 `handleStepKeydown`：**只有** ↑ / ↓，带修饰键时不处理（焦点在输入框里时，
    /// 按键不会送到这里——输入框不在本页的焦点树里）。
    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = &event.keystroke.modifiers;
        if modifiers.alt || modifiers.control || modifiers.platform || modifiers.shift {
            return;
        }
        let delta = match event.keystroke.key.as_str() {
            "up" => -1i64,
            "down" => 1i64,
            _ => return,
        };
        // 原版 `moveSelectedSlide`：两端夹住，一动就重选。
        if self.board_count == 0 {
            return;
        }
        let current = self.selected.unwrap_or(0).min(self.board_count - 1);
        let next = (current as i64 + delta).clamp(0, self.board_count as i64 - 1) as usize;
        if next != current {
            self.select(next, cx);
        }
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = Some(index);
        self.scroll_to(index);
        cx.notify();
    }

    /// 选中的卡片滚进可见区（原版没有；GPUI 下顺手做）。
    fn scroll_to(&self, index: usize) {
        let columns = self.columns.max(1);
        self.list
            .scroll_to_item(index / columns, ScrollStrategy::Nearest);
    }

    /// 打开「选择重试布局」对话框（原版 `retryLayoutDialog.open(runId, index)`）。
    fn show_retry_dialog(
        &mut self,
        run_id: Option<i64>,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.clone();
        let dialog = cx.new(|cx| RetryLayoutDialog::new(store, cx));
        let entity = dialog.clone();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog.overlay(true).w(px(440.0)).child(entity.clone())
        });
        dialog.update(cx, |dialog, cx| dialog.load(run_id, index, window, cx));
        self.retry_dialog = Some(dialog);
    }
}

impl Render for Step2 {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;

        // ── 数据：生成状态一律来自应用级 store（原版 props 都来自 `useGenerationStore()`） ──
        let (model_id, run, deck) = {
            let model = self.model.read(cx);
            (model.id, model.latest_run.clone(), model.deck.clone())
        };
        let snap = self
            .store
            .read(cx)
            .snapshot_for(model_id, run.as_ref(), &deck);
        let items = snap.items;
        let stage = snap.stage;
        let running = snap.running;
        let retrying = snap.retrying;
        let statuses = snap.statuses;
        let message = snap.message;
        let important = snap.important;
        let prompt_count = snap.prompt_count;
        // 单页重试要这次运行的 id（缺了的话对话框报「缺少生成记录，请重新生成」）。
        let run_id = snap.current_run.as_ref().map(|run| run.id);
        // 原版 `summarizeProgress` 的 running 参数：`running || retryingPages.size > 0`。
        let summary_running = running || !retrying.is_empty();

        // 原版 `slideDims`：页面比例取牌组第一张的 `aspect_ratio`，没有就 16:9。
        let aspect = snap
            .aspect_source
            .as_ref()
            .and_then(|slide| slide.get("aspect_ratio"))
            .and_then(Value::as_str)
            .and_then(parse_aspect)
            .unwrap_or(AspectRatio::Ratio16x9);
        let aspect_ratio = aspect.height() as f32 / aspect.width() as f32;

        // ── 尺寸：先按窗口算出两栏的宽度，再按看板栏内宽算列数（原版 auto-fill） ──
        let viewport = window.viewport_size();
        let panel_w = (viewport.width.as_f32() - SIDEBAR_W).max(GRID_MIN_COL + 2.0 * GRID_PAD);
        let detail_w = ((panel_w - PANEL_GAP) * 0.9 / 2.4).max(DETAIL_MIN_W);
        let grid_w = (panel_w - PANEL_GAP - detail_w).max(GRID_MIN_COL + 2.0 * GRID_PAD);
        let grid = Grid::new(grid_w);
        let columns = grid.columns;
        let column_w = grid.column_w;
        let preview_w = grid.preview_w();
        let preview_h = preview_w * aspect_ratio;
        let card_h = card_height(preview_h);
        let detail_stage_w = (detail_w - 2.0 * (PANEL_PAD + CARD_BORDER)).max(1.0);
        self.columns = columns;

        // ── 选中的页：越界或没有时取 `preferredSlideIndex`（原版 `selectedSlide` 的 watch） ──
        let selected = match self.selected {
            Some(index) if index < items.len() => Some(index),
            _ => progress::preferred_slide_index(&items),
        };
        self.selected = selected;
        self.board_count = items.len();

        // ── 缩略图（原版 `useTypstThumbnails`：牌组 = 看板条目的蓝图，下标 = 条目下标） ──
        self.thumbs.set_deck(snap.thumbs.clone());
        self.thumbs.set_width(preview_w, window);
        // 只给可见行（多预取一行）的卡片要图。`uniform_list` 在布局阶段才报可见范围，
        // 范围变了它会 `cx.notify()` 一次，下一帧就按新范围要图。
        let (row_start, row_end) = self.visible_rows;
        let wanted: Vec<usize> = (row_start * columns..(row_end + 1) * columns)
            .take_while(|index| *index < items.len())
            .collect();
        self.thumbs.set_wanted(&wanted);
        let focus_index = selected.filter(|index| wanted.contains(index));
        self.thumbs
            .pump(focus_index, |this: &mut Self| &mut this.thumbs, window, cx);

        // 详情大图只请求选中的这一页（宽度 = 详情舞台的实测宽度）。
        self.detail.set_deck(snap.thumbs.clone());
        self.detail.set_width(detail_stage_w, window);
        self.detail
            .set_wanted(&selected.map(|index| vec![index]).unwrap_or_default());
        self.detail
            .pump(selected, |this: &mut Self| &mut this.detail, window, cx);
        let detail_frame = selected.map(|index| self.detail.frame_state(index));
        // 每张卡片的图状态：每帧算一遍（只是查表），行闭包里只读它。
        let frames: Vec<FrameState> = (0..items.len())
            .map(|index| self.thumbs.frame_state(index))
            .collect();

        // ── 头部：状态圆点、四态标题、三个统计（原版 `.gen-header`） ──────────────
        let title = if running {
            format!("{}...", progress::stage_label(&stage))
        } else if stage == "done" {
            format!("生成完成 · {} 张幻灯片", items.len())
        } else if stage == "error" {
            "生成遇到问题".to_string()
        } else {
            "准备开始".to_string()
        };
        // 原版 `.gen-orb--*`：生成中的阶段是主色（带脉冲），完成绿、出错红、其余灰。
        let (orb_color, orb_pulse) = match stage.as_str() {
            "start" | "init" | "clean" | "page_plan" | "layout_plan" | "content" | "normalize"
            | "validate" | "generating" | "assembling" => (c.primary, true),
            "done" => (c.success, false),
            "error" => (c.error, false),
            _ => (c.muted, false),
        };
        let orb = div()
            .flex_none()
            .size(px(12.0))
            .rounded_full()
            .bg(orb_color);
        let orb: AnyElement = if orb_pulse {
            orb.with_animation(
                "s2-orb",
                Animation::new(Duration::from_millis(1800)).repeat(),
                |orb, delta| orb.opacity(1.0 - 0.35 * (delta * std::f32::consts::PI).sin().abs()),
            )
            .into_any_element()
        } else {
            orb.into_any_element()
        };
        let header = div()
            .flex_none()
            .px(px(24.0))
            .py(px(14.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(16.0))
            .bg(c.panel)
            .border_b_1()
            .border_color(c.border)
            .child(
                div()
                    .min_w_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(14.0))
                    .child(orb)
                    .child(
                        div()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(15.0))
                                    .font_weight(FontWeight::BOLD)
                                    .child(title),
                            )
                            .when(!message.is_empty(), |column| {
                                column.child(
                                    div()
                                        .mt(px(2.4))
                                        .max_w(px(420.0))
                                        .truncate()
                                        .text_size(px(11.8))
                                        .text_color(c.muted)
                                        .child(message),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .gap(px(7.2))
                    .child(stat_chip(items.len().to_string(), "幻灯片", false, cx))
                    .child(stat_chip(important.to_string(), "关键事件", false, cx))
                    .child(stat_chip(prompt_count.to_string(), "生成记录", true, cx)),
            );

        // ── 整体进度（原版 `.gen-progress-overview` + `summarizeProgress`） ────────
        let progress_section = progress_section(
            &progress::summarize_progress(&items, &statuses, summary_running),
            self.progress_open,
            cx.listener(|this, _event, _window, cx| {
                this.progress_open = !this.progress_open;
                cx.notify();
            }),
            cx,
        );

        // ── 看板：每张卡片要显示的东西先算好，行闭包只读 ─────────────────────────
        let cards: Vec<CardView> = items
            .iter()
            .map(|item| {
                let status = progress::slide_status(item, &stage, running, &statuses, &retrying);
                let steps = [
                    progress::page_step_state(
                        item,
                        statuses.get(&item.index),
                        "planning",
                        summary_running,
                    ),
                    progress::page_step_state(
                        item,
                        statuses.get(&item.index),
                        "layout",
                        summary_running,
                    ),
                    progress::page_step_state(
                        item,
                        statuses.get(&item.index),
                        "content",
                        summary_running,
                    ),
                    progress::page_step_state(
                        item,
                        statuses.get(&item.index),
                        "normalizing",
                        summary_running,
                    ),
                    progress::page_step_state(
                        item,
                        statuses.get(&item.index),
                        "validating",
                        summary_running,
                    ),
                ];
                CardView {
                    index: item.index.max(0) as usize,
                    title: card_title(item),
                    status: (tone_of(&status.tone), status.label),
                    steps,
                    layout: item.layout_plan.as_ref().map(|plan| {
                        // 版式名用 Step2 自己的 `KIND_LABELS`（`step2_kind_label`），不是
                        // `editor_schema` 的中文名；蓝图版式优先。
                        let kind = blueprint_kind(item).or_else(|| field_str(plan, "kind"));
                        progress::step2_kind_label(kind).to_string()
                    }),
                    has_blueprint: is_present(item.blueprint.as_ref()),
                }
            })
            .collect();

        let rows = grid.rows(cards.len());
        let board_grid: AnyElement = if cards.is_empty() {
            empty_state(
                "document-blank",
                "生成开始后，这里会直接显示每一张幻灯片的生成进度。",
                cx,
            )
        } else {
            div()
                .id("s2-board-grid")
                .flex_1()
                .min_h_0()
                .p(px(GRID_PAD))
                .child(
                    uniform_list("s2-board", rows, {
                        let cards = cards.clone();
                        let frames = frames.clone();
                        let this = cx.weak_entity();
                        let reporter = this.clone();
                        let report = move |range: &Range<usize>, cx: &mut App| {
                            let next = (range.start, range.end);
                            let _ = reporter.update(cx, |this, cx| {
                                if this.visible_rows != next {
                                    this.visible_rows = next;
                                    cx.notify();
                                }
                            });
                        };
                        move |range: Range<usize>, _window: &mut Window, cx: &mut App| {
                            report(&range, cx);
                            range
                                .map(|row| {
                                    let start = row * columns;
                                    let end = (start + columns).min(cards.len());
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap(px(GRID_GAP))
                                        .pb(px(GRID_GAP))
                                        .children((start..end).map(|index| {
                                            board_card(
                                                &cards[index],
                                                frames[index].clone(),
                                                selected == Some(index),
                                                column_w,
                                                card_h,
                                                preview_w,
                                                preview_h,
                                                this.clone(),
                                                cx,
                                            )
                                        }))
                                })
                                .collect::<Vec<_>>()
                        }
                    })
                    .size_full()
                    .track_scroll(&self.list),
                )
                .into_any_element()
        };

        let board = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .rounded(px(16.0))
            .border_1()
            .border_color(c.border)
            .bg(c.surface)
            .overflow_hidden()
            .child(
                panel_head("catalog", "幻灯片生成看板", cx)
                    .when(running, |head| head.child(live_dot(cx)))
                    .child(div().flex_1())
                    .child(count_chip(cards.len(), cx)),
            )
            .child(board_grid);

        // ── 详情（右侧） ────────────────────────────────────────────────────────
        let detail = detail_panel(
            DetailArgs {
                item: selected.map(|index| &items[index]),
                status: selected.map(|index| {
                    progress::slide_status(&items[index], &stage, running, &statuses, &retrying)
                }),
                page_status: selected.and_then(|index| statuses.get(&(index as i64))),
                frame: detail_frame,
                width: detail_w,
                stage_w: detail_stage_w,
                aspect_ratio,
                retrying: &retrying,
                running,
                run_id,
                recent_logs: &snap.recent_logs,
                debug_open: self.debug_open,
                logs_open: self.logs_open,
                code_font: code_font(&self.handle),
                this: cx.weak_entity(),
            },
            cx,
        );

        // ── 底栏（原版：生成中不显示） ─────────────────────────────────────────
        let retrying_any = !retrying.is_empty();
        let bottom: AnyElement = if running {
            div().into_any_element()
        } else {
            let to_edit = self.page.clone();
            let to_step1 = self.page.clone();
            let regenerate = self.page.clone();
            div()
                .flex_none()
                .px(px(24.0))
                .py(px(14.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.0))
                .bg(c.panel)
                .border_t_1()
                .border_color(c.border)
                .when(stage == "done", |bar| {
                    bar.child(
                        widgets::primary_button("s2-to-edit", "前往编辑")
                            .icon(carbon("edit"))
                            .disabled(retrying_any)
                            .on_click(move |_event, window, cx| {
                                let _ = to_edit
                                    .update(cx, |page, cx| page.show_step(Step::Edit, window, cx));
                            }),
                    )
                })
                .when(stage == "error", |bar| {
                    bar.child(
                        widgets::ghost_button("s2-back", "返回修改")
                            .icon(carbon("arrow-left"))
                            .on_click(move |_event, window, cx| {
                                let _ = to_step1.update(cx, |page, cx| {
                                    page.show_step(Step::Prepare, window, cx)
                                });
                            }),
                    )
                    .child(
                        // 原版 `@retry="onStep1StartGenerate"`：与第一步「开始 AI 生成」同一条路。
                        widgets::primary_button("s2-regenerate", "重新生成全部")
                            .icon(carbon("renew"))
                            .disabled(retrying_any)
                            .on_click(move |_event, window, cx| {
                                let _ = regenerate
                                    .update(cx, |page, cx| page.start_generation(window, cx));
                            }),
                    )
                })
                .into_any_element()
        };

        div()
            .id("step2")
            .track_focus(&self.focus)
            .on_key_down(
                cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    this.on_key(event, window, cx)
                }),
            )
            .size_full()
            .flex()
            .flex_col()
            .child(header)
            .child(progress_section)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .gap(px(PANEL_GAP))
                    .child(board)
                    .child(detail),
            )
            .child(bottom)
    }
}

// ── 看板几何 ────────────────────────────────────────────────────────────────

/// 看板的网格几何（原版 `.slides-grid` 的 `repeat(auto-fill, minmax(260px, 1fr))`）。
struct Grid {
    columns: usize,
    column_w: f32,
}

impl Grid {
    /// `auto-fill`：内宽能塞下几列就几列（至少一列），列宽平分剩下的宽度。
    fn new(panel_w: f32) -> Self {
        let inner = (panel_w - 2.0 * GRID_PAD).max(GRID_MIN_COL);
        let columns = (((inner + GRID_GAP) / (GRID_MIN_COL + GRID_GAP)).floor() as usize).max(1);
        let column_w = ((inner - GRID_GAP * (columns - 1) as f32) / columns as f32).max(1.0);
        Self { columns, column_w }
    }

    /// 缩略图要图的宽度：卡片内宽（原版 `cardPreviewWidth` 量的就是预览框的内宽，
    /// 位图宽度不再乘页面比例）。
    fn preview_w(&self) -> f32 {
        (self.column_w - 2.0 * (CARD_PAD + CARD_BORDER)).max(1.0)
    }

    fn rows(&self, items: usize) -> usize {
        items.div_ceil(self.columns.max(1))
    }
}

/// 一张卡片的固定高度：`uniform_list` 的一行 = 一行卡片，行高必须一致。
fn card_height(preview_h: f32) -> f32 {
    2.0 * (CARD_PAD + CARD_BORDER)
        + CARD_TOP_H
        + CARD_STEPS_H
        + CARD_TAG_H
        + 3.0 * CARD_GAP
        + preview_h
}

// ── 看板卡片 ────────────────────────────────────────────────────────────────

/// 看板一行里的一张卡片。
#[derive(Clone)]
struct CardView {
    index: usize,
    title: String,
    /// 原版 `slideStatus` 的 `{ tone, label }`。
    status: (Tone2, String),
    /// 五个阶段标签的样式（`done` / `active` / `pending`）。
    steps: [&'static str; 5],
    /// 版式标签文本；`None` = 还没有布局方案（显示「等待布局排版结果」）。
    layout: Option<String>,
    has_blueprint: bool,
}

fn field_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
}

fn is_present(value: Option<&Value>) -> bool {
    value.map(|value| !value.is_null()).unwrap_or(false)
}

/// 蓝图里的版式名。
fn blueprint_kind(item: &SlideProgressItem) -> Option<&str> {
    item.blueprint
        .as_ref()
        .and_then(|slide| field_str(slide, "kind"))
}

/// 卡片标题（原版 `pagePlan?.page_title || layoutPlan?.title || blueprint?.title || 第 n 张`）。
fn card_title(item: &SlideProgressItem) -> String {
    item.page_plan
        .as_ref()
        .and_then(|plan| field_str(plan, "page_title"))
        .or_else(|| {
            item.layout_plan
                .as_ref()
                .and_then(|plan| field_str(plan, "title"))
        })
        .or_else(|| {
            item.blueprint
                .as_ref()
                .and_then(|slide| field_str(slide, "title"))
        })
        .map(str::to_string)
        .unwrap_or_else(|| format!("第 {} 张", item.index + 1))
}

/// 原版 tone 字符串 → 徽章色调。
fn tone_of(tone: &str) -> Tone2 {
    match tone {
        "done" => Tone2::Success,
        "active" => Tone2::Primary,
        "error" => Tone2::Error,
        _ => Tone2::Muted,
    }
}

/// 看板 / 详情面板的小标题栏（原版 `.gen-panel-head`）。
fn panel_head(
    icon: &'static str,
    label: &'static str,
    cx: &App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let c = cx.studio().colors;
    div()
        .id(SharedString::from(format!("s2-head-{icon}")))
        .flex_none()
        .px(px(12.0))
        .py(px(7.2))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.4))
        .bg(c.panel_2)
        .border_b_1()
        .border_color(c.border)
        .text_size(px(10.0))
        .font_weight(FontWeight::BOLD)
        .text_color(c.muted)
        .child(carbon(icon))
        .child(label)
}

/// 面板标题栏右侧的计数（原版 `.gen-panel-count`）。
fn count_chip(count: usize, cx: &App) -> AnyElement {
    let c = cx.studio().colors;
    div()
        .px(px(6.4))
        .rounded(px(999.0))
        .border_1()
        .border_color(c.border)
        .bg(c.surface)
        .text_size(px(9.6))
        .child(count.to_string())
        .into_any_element()
}

/// 头部的一个统计块（原版 `.gen-stat-chip`）。
fn stat_chip(value: String, label: &'static str, accent: bool, cx: &App) -> AnyElement {
    let c = cx.studio().colors;
    div()
        .min_w(px(54.0))
        .px(px(11.2))
        .py(px(6.4))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(1.6))
        .rounded(px(8.0))
        .border_1()
        .border_color(c.border)
        .bg(c.surface)
        .child(
            div()
                .text_size(px(18.0))
                .font_weight(FontWeight::BOLD)
                .text_color(if accent { c.primary } else { c.text })
                .child(value),
        )
        .child(div().text_size(px(9.0)).text_color(c.muted).child(label))
        .into_any_element()
}

/// 生成中的实时点（原版 `.gen-live-dot`：`background: var(--studio-error)` + 1.2s 闪烁）。
fn live_dot(cx: &App) -> AnyElement {
    let c = cx.studio().colors;
    div()
        .flex_none()
        .size(px(6.0))
        .rounded_full()
        .bg(c.error)
        .with_animation(
            "s2-live-dot",
            Animation::new(Duration::from_millis(1200)).repeat(),
            |dot, delta| dot.opacity(1.0 - 0.7 * (delta * std::f32::consts::PI).sin().abs()),
        )
        .into_any_element()
}

/// 看板 / 详情的空状态（原版 `.slides-empty` / `.sdp-empty`）。
fn empty_state(icon: &'static str, text: &'static str, cx: &App) -> AnyElement {
    let c = cx.studio().colors;
    div()
        .flex_1()
        .min_h(px(320.0))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(12.0))
        .child(div().text_size(px(32.0)).opacity(0.22).child(carbon(icon)))
        .child(div().text_size(px(12.8)).text_color(c.muted).child(text))
        .into_any_element()
}

/// 一张看板卡片（原版 `.slide-progress-card`）。
#[allow(clippy::too_many_arguments)]
fn board_card(
    card: &CardView,
    frame: FrameState,
    selected: bool,
    column_w: f32,
    card_h: f32,
    preview_w: f32,
    preview_h: f32,
    this: WeakEntity<Step2>,
    cx: &App,
) -> AnyElement {
    let c = cx.studio().colors;
    let index = card.index;
    let steps = card.steps;
    let on_pick = this.clone();
    let on_retry = this;

    // 阶段标签的三种样式（原版 `.spc-step.is-done / .is-active / 默认`）。
    let step_chip = move |state: &'static str, label: &'static str| {
        let (fg, bg, border) = match state {
            "done" => (c.success, c.success_bg, c.success),
            "active" => (c.primary, c.primary_bg, c.primary),
            _ => (c.muted, c.surface, c.border),
        };
        div()
            .flex_none()
            .px(px(8.0))
            .h(px(20.0))
            .flex()
            .items_center()
            .rounded(px(999.0))
            .border_1()
            .border_color(border)
            .bg(bg)
            .text_size(px(11.5))
            .text_color(fg)
            .child(label)
    };

    let preview: AnyElement = if card.has_blueprint {
        slide_frame(
            ("s2-thumb", index),
            frame,
            preview_w,
            preview_h,
            false,
            Some(Box::new(move |_window, cx| {
                let _ = on_retry.update(cx, |this, cx| {
                    this.thumbs.refresh();
                    cx.notify();
                });
            })),
            cx,
        )
        .into_any_element()
    } else {
        // 原版 `.spc-preview-placeholder`：有布局方案 → 「内容生成后将在这里出现预览」，
        // 否则「当前还没有可渲染内容」。
        let hint = if card.layout.is_some() {
            "内容生成后将在这里出现预览"
        } else {
            "当前还没有可渲染内容"
        };
        div()
            .flex_none()
            .w(px(preview_w))
            .h(px(preview_h))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .rounded(px(12.0))
            .border_1()
            .border_color(c.border)
            .bg(c.bg)
            .child(div().text_size(px(22.4)).opacity(0.5).child(carbon("task")))
            .child(div().text_size(px(12.8)).text_color(c.muted).child(hint))
            .into_any_element()
    };

    div()
        .id(("s2-card", index))
        .flex_none()
        .w(px(column_w))
        .h(px(card_h))
        .p(px(CARD_PAD))
        .flex()
        .flex_col()
        .gap(px(CARD_GAP))
        .rounded(px(16.0))
        .bg(c.panel)
        .border_1()
        .border_color(if selected { c.primary_border } else { c.border })
        .cursor_pointer()
        .on_click(move |_event, _window, cx| {
            let _ = on_pick.update(cx, |this, cx| this.select(index, cx));
        })
        .child(
            div()
                .h(px(CARD_TOP_H))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .flex_none()
                        .text_size(px(12.2))
                        .text_color(c.muted)
                        .child(format!("#{}", index + 1)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(15.2))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(card.title.clone()),
                )
                .child(widgets::chip(card.status.1.clone(), card.status.0, cx)),
        )
        .child(
            div()
                .h(px(CARD_STEPS_H))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .children(
                    STEP_LABELS
                        .iter()
                        .zip(steps)
                        .map(|(label, state)| step_chip(state, label)),
                ),
        )
        .child(
            div()
                .h(px(CARD_TAG_H))
                .flex()
                .flex_row()
                .items_center()
                .when_some(card.layout.clone(), |row, label| {
                    row.child(
                        div()
                            .px(px(10.4))
                            .py(px(3.8))
                            .truncate()
                            .rounded(px(999.0))
                            .border_1()
                            .border_color(c.primary_border)
                            .bg(c.primary_bg)
                            .text_size(px(12.0))
                            .text_color(c.primary)
                            .child(label),
                    )
                })
                .when(card.layout.is_none(), |row| {
                    row.child(
                        div()
                            .px(px(10.4))
                            .py(px(3.8))
                            .truncate()
                            .rounded(px(999.0))
                            .border_1()
                            .border_color(c.border)
                            .bg(c.surface)
                            .text_size(px(12.0))
                            .text_color(c.muted)
                            .child("等待布局排版结果"),
                    )
                }),
        )
        .child(preview)
        .into_any_element()
}

// ── 整体进度 ────────────────────────────────────────────────────────────────

/// 整体进度区（原版 `.gen-progress-overview`）。
fn progress_section(
    summary: &progress::ProgressSummary,
    open: bool,
    on_toggle: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let c = cx.studio().colors;
    let total = summary.total;
    let meta = |text: String, color: Hsla| div().text_size(px(12.5)).text_color(color).child(text);
    let fraction = if total > 0 {
        summary.done as f32 / total as f32
    } else {
        0.0
    };

    // 五个阶段卡片（原版 `.gpo-stages`：图标 + 名称 + 「进行中」徽章 + n/N + 分隔线）。
    let stages: Vec<AnyElement> = summary
        .stage_cards
        .iter()
        .enumerate()
        .map(|(position, card)| {
            let done = card.state == "done";
            let active = card.state == "active";
            let icon: AnyElement = if done {
                div()
                    .text_size(px(13.0))
                    .text_color(c.surface)
                    .child(carbon("checkmark"))
                    .into_any_element()
            } else if active {
                div()
                    .size(px(12.0))
                    .rounded_full()
                    .bg(c.primary)
                    .into_any_element()
            } else {
                div().into_any_element()
            };
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(9.6))
                .child(
                    div()
                        .flex_none()
                        .size(px(24.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_1()
                        .border_color(if done {
                            c.success
                        } else if active {
                            c.primary
                        } else {
                            c.border_hover
                        })
                        .bg(if done { c.success } else { c.surface })
                        .child(icon),
                )
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(6.4))
                                .text_size(px(12.8))
                                .text_color(if done || active { c.text } else { c.muted })
                                .child(card.label)
                                .when(active, |heading| {
                                    heading.child(
                                        div()
                                            .px(px(4.8))
                                            .rounded(px(5.0))
                                            .bg(c.primary_bg)
                                            .text_size(px(10.4))
                                            .text_color(c.primary)
                                            .child("进行中"),
                                    )
                                }),
                        )
                        // 原版只在有完成或进行中时显示 `completed/total`。
                        .when(card.completed > 0 || active, |body| {
                            body.child(
                                div()
                                    .mt(px(2.6))
                                    .text_size(px(11.5))
                                    .text_color(c.muted)
                                    .child(format!("{}/{}", card.completed, total)),
                            )
                        }),
                )
                .when(position + 1 < summary.stage_cards.len(), |stage| {
                    stage.child(div().flex_1().h(px(1.0)).ml(px(4.0)).bg(c.border))
                })
                .into_any_element()
        })
        .collect();

    // 「查看详情」展开后的四列表格（阶段 / 已完成 / 进行中 / 完成比例）。
    let table_row =
        |label: &str, completed: String, active: String, percent: String| -> AnyElement {
            div()
                .py(px(6.4))
                .flex()
                .flex_row()
                .gap(px(8.0))
                .text_size(px(12.0))
                .child(
                    div()
                        .flex_grow(1.5)
                        .flex_basis(px(0.0))
                        .child(label.to_string()),
                )
                .child(div().flex_grow(1.0).flex_basis(px(0.0)).child(completed))
                .child(div().flex_grow(1.0).flex_basis(px(0.0)).child(active))
                .child(div().flex_grow(1.0).flex_basis(px(0.0)).child(percent))
                .into_any_element()
        };
    let details = open.then(|| {
        div()
            .mt(px(13.6))
            .pt(px(9.6))
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(c.border)
            .text_size(px(12.0))
            .text_color(c.muted)
            .child(
                div()
                    .mt(px(4.8))
                    .mb(px(9.6))
                    .child("完成数仅统计已通过该阶段的页面；整体进度按全部阶段完成的页面计算。"),
            )
            .child(div().mb(px(9.6)).child(format!(
                "共 {} 页 · 已完成 {} · 进行中 {} · 等待 {} · 失败 {} · 已中断 {}",
                total,
                summary.done,
                summary.active,
                summary.pending,
                summary.error,
                summary.interrupted
            )))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .py(px(6.4))
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.text)
                            .child(div().flex_grow(1.5).flex_basis(px(0.0)).child("阶段"))
                            .child(div().flex_grow(1.0).flex_basis(px(0.0)).child("已完成"))
                            .child(div().flex_grow(1.0).flex_basis(px(0.0)).child("进行中"))
                            .child(div().flex_grow(1.0).flex_basis(px(0.0)).child("完成比例")),
                    )
                    .children(summary.stage_cards.iter().map(|card| {
                        table_row(
                            card.label,
                            format!("{}/{}", card.completed, total),
                            card.active.to_string(),
                            format!("{}%", card.percent),
                        )
                    })),
            )
    });

    div()
        .flex_none()
        .mb(px(16.0))
        .px(px(17.6))
        .py(px(14.4))
        .rounded(px(14.0))
        .border_1()
        .border_color(c.border)
        .bg(c.surface)
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap(px(16.0))
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(16.0))
                        .child(
                            div()
                                .flex_none()
                                .text_size(px(14.1))
                                .font_weight(FontWeight::BOLD)
                                .child("整体进度"),
                        )
                        .child(meta(format!("已完成 {}/{}", summary.done, total), c.muted))
                        .when(summary.active > 0, |main| {
                            main.child(meta(format!("进行中 {}", summary.active), c.muted))
                        })
                        .when(summary.error > 0, |main| {
                            main.child(meta(format!("失败 {}", summary.error), c.error))
                        })
                        .when(summary.interrupted > 0, |main| {
                            main.child(meta(format!("已中断 {}", summary.interrupted), c.muted))
                        }),
                )
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(16.0))
                        .child(
                            div()
                                .text_size(px(20.8))
                                .font_weight(FontWeight::BOLD)
                                .child(format!("{}%", summary.percent)),
                        )
                        .child(
                            div()
                                .id("s2-progress-toggle")
                                .pl(px(16.0))
                                .py(px(4.8))
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(6.4))
                                .border_l_1()
                                .border_color(c.border)
                                .text_size(px(12.5))
                                .text_color(c.muted)
                                .cursor_pointer()
                                .child(if open { "收起详情" } else { "查看详情" })
                                .child(carbon(if open { "chevron-up" } else { "chevron-down" }))
                                .on_click(on_toggle),
                        ),
                ),
        )
        .child(
            div()
                .mt(px(9.6))
                .h(px(4.0))
                .w_full()
                .rounded(px(999.0))
                .bg(c.border)
                .overflow_hidden()
                .child(div().h_full().w(relative(fraction)).bg(c.primary)),
        )
        .child(
            div()
                .mt(px(13.6))
                .flex()
                .flex_row()
                .gap(px(16.0))
                .children(stages),
        )
        .children(details)
        .into_any_element()
}

// ── 详情面板 ────────────────────────────────────────────────────────────────

/// 详情面板要用的一整套数据（原版 `selectedSlide` 相关的 computed）。
struct DetailArgs<'a> {
    item: Option<&'a SlideProgressItem>,
    status: Option<SlideStatus>,
    /// 原版 `props.pageStatuses.get(index)`（失败段的状态文字与判定要看它）。
    page_status: Option<&'a PageProgressStatus>,
    frame: Option<FrameState>,
    width: f32,
    stage_w: f32,
    aspect_ratio: f32,
    retrying: &'a std::collections::BTreeSet<i64>,
    running: bool,
    run_id: Option<i64>,
    recent_logs: &'a [GenerationLogEvent],
    debug_open: bool,
    logs_open: bool,
    code_font: SharedString,
    this: WeakEntity<Step2>,
}

/// 右侧「幻灯片详情」（原版 `.slide-detail-panel`）。
fn detail_panel(args: DetailArgs<'_>, cx: &App) -> AnyElement {
    let c = cx.studio().colors;
    let shell = |body: AnyElement| -> AnyElement {
        div()
            .flex_none()
            .w(px(args.width))
            .h_full()
            .flex()
            .flex_col()
            .rounded(px(16.0))
            .border_1()
            .border_color(c.border)
            .bg(c.surface)
            .overflow_hidden()
            .child(panel_head("view", "幻灯片详情", cx))
            .child(body)
            .into_any_element()
    };

    let Some(item) = args.item else {
        return shell(empty_state(
            "cursor-1",
            "生成开始后，选择任意一张幻灯片查看详情。",
            cx,
        ));
    };
    let index = item.index;
    let status = args.status.clone().unwrap_or(SlideStatus {
        tone: "pending".to_string(),
        label: String::new(),
    });
    let kind_label = progress::step2_kind_label(blueprint_kind(item).or_else(|| {
        item.layout_plan
            .as_ref()
            .and_then(|plan| field_str(plan, "kind"))
    }))
    .to_string();
    let title = card_title(item);
    let section_label = item
        .page_plan
        .as_ref()
        .and_then(|plan| field_str(plan, "section_title"))
        .map(str::to_string);

    // 大图（原版 `.sdp-preview-stage`）；没有蓝图时是「当前页尚未生成最终内容。」。
    let preview: AnyElement = if is_present(item.blueprint.as_ref()) {
        let on_retry = args.this.clone();
        slide_frame(
            ("s2-detail-frame", index.max(0) as usize),
            args.frame
                .clone()
                .unwrap_or(FrameState::Loading { show_label: false }),
            args.stage_w,
            args.stage_w * args.aspect_ratio,
            false,
            Some(Box::new(move |_window, cx| {
                let _ = on_retry.update(cx, |this, cx| {
                    this.detail.refresh();
                    cx.notify();
                });
            })),
            cx,
        )
        .into_any_element()
    } else {
        div()
            .w(px(args.stage_w))
            .h(px(args.stage_w * args.aspect_ratio))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .rounded(px(14.0))
            .border_1()
            .border_color(c.border)
            .bg(c.bg)
            .child(
                div()
                    .text_size(px(22.4))
                    .opacity(0.5)
                    .child(carbon("image-search")),
            )
            .child(
                div()
                    .text_size(px(12.8))
                    .text_color(c.muted)
                    .child("当前页尚未生成最终内容。"),
            )
            .into_any_element()
    };

    // 各段的共用部件（原版 `.sdp-section-title` / `.sdp-text` / `.sdp-note`）。
    let section_title = |text: &'static str| -> AnyElement {
        div()
            .text_size(px(12.2))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(c.muted)
            .mb(px(7.2))
            .child(text)
            .into_any_element()
    };
    let text = |value: String| -> AnyElement {
        div()
            .text_size(px(12.8))
            .text_color(c.muted)
            .child(value)
            .into_any_element()
    };
    let note = |value: String| -> AnyElement {
        div()
            .mt(px(8.0))
            .text_size(px(12.8))
            .text_color(c.muted)
            .child(value)
            .into_any_element()
    };
    let wrap = |children: Vec<AnyElement>| -> AnyElement {
        div()
            .p(px(12.0))
            .rounded(px(12.0))
            .border_1()
            .border_color(c.border)
            .bg(c.surface)
            .children(children)
            .into_any_element()
    };
    let mut body: Vec<AnyElement> = Vec::new();

    // 失败段（原版：该页 error，或正在（排队）重试）。
    let is_failed = args
        .page_status
        .map(|status| status.stage == "error")
        .unwrap_or(false);
    let is_retrying = args.retrying.contains(&item.index);
    if is_failed || is_retrying {
        let status_text = if is_retrying {
            if args.running {
                "已排队，其他页面生成结束后重试。".to_string()
            } else {
                "正在重新生成这一页…".to_string()
            }
        } else {
            args.page_status
                .and_then(|status| status.message.clone())
                .unwrap_or_default()
        };
        let this = args.this.clone();
        let run_id = args.run_id;
        let page = item.index.max(0) as usize;
        let mut children = vec![
            section_title("生成失败，可单独重试"),
            div()
                .text_size(px(12.8))
                .text_color(c.muted)
                .child(status_text)
                .into_any_element(),
            div()
                .mt(px(12.0))
                .flex()
                .flex_row()
                .child(
                    widgets::primary_button(
                        "s2-retry-page",
                        if is_retrying {
                            "重试处理中"
                        } else {
                            "重试这一页"
                        },
                    )
                    .icon(carbon("renew"))
                    // 原版禁用条件：本页在重试，或（不在生成中且已有页在重试）。
                    .disabled(is_retrying || (!args.running && !args.retrying.is_empty()))
                    .on_click(move |_event, window, cx| {
                        let _ = this.update(cx, |this, cx| {
                            this.show_retry_dialog(run_id, page, window, cx)
                        });
                    }),
                )
                .into_any_element(),
        ];
        if is_present(item.blueprint.as_ref()) {
            children.push(note(
                "当前保留简化内容，可继续编辑，也可重试生成。".to_string(),
            ));
        }
        body.push(wrap(children));
    }

    // 规划页面。
    let mut planned = vec![section_title("规划页面")];
    if let Some(objective) = item
        .page_plan
        .as_ref()
        .and_then(|plan| field_str(plan, "objective"))
    {
        planned.push(text(objective.to_string()));
    }
    let key_points: Vec<String> = item
        .page_plan
        .as_ref()
        .and_then(|plan| plan.get("key_points"))
        .and_then(Value::as_array)
        .map(|points| {
            points
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if !key_points.is_empty() {
        planned.push(
            div()
                .pl(px(4.0))
                .flex()
                .flex_col()
                .gap(px(5.6))
                .text_size(px(12.8))
                .text_color(c.muted)
                .children(
                    key_points
                        .into_iter()
                        .map(|point| div().flex().flex_row().gap(px(6.0)).child("·").child(point)),
                )
                .into_any_element(),
        );
    }
    if let Some(takeaway) = item
        .page_plan
        .as_ref()
        .and_then(|plan| field_str(plan, "takeaway"))
    {
        planned.push(note(format!("结论：{takeaway}")));
    }
    if item.page_plan.is_none() {
        planned.push(text("这一页的规划结果还没出来。".to_string()));
    }
    body.push(wrap(planned));

    // 布局排版。
    let mut layout = vec![section_title("布局排版")];
    if let Some(plan) = item.layout_plan.as_ref() {
        let adjusted = item.blueprint.is_some()
            && blueprint_kind(item) != plan.get("kind").and_then(Value::as_str);
        let suffix = if adjusted {
            " · 已调整布局".to_string()
        } else {
            field_str(plan, "reason")
                .map(|reason| format!(" · {reason}"))
                .unwrap_or_default()
        };
        layout.push(
            div()
                .flex()
                .flex_row()
                .text_size(px(12.8))
                .text_color(c.muted)
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .child(kind_label.clone()),
                )
                .child(suffix)
                .into_any_element(),
        );
        if let Some(subtitle) = field_str(plan, "subtitle") {
            layout.push(note(subtitle.to_string()));
        }
    } else {
        layout.push(text("这一页还没有布局方案。".to_string()));
    }
    body.push(wrap(layout));

    // 内容结果。
    let mut content = vec![section_title("内容结果")];
    if let Some(blueprint) = item.blueprint.as_ref().filter(|slide| !slide.is_null()) {
        let blueprint_title = field_str(blueprint, "title").unwrap_or_default();
        let subtitle = field_str(blueprint, "subtitle")
            .map(|subtitle| format!(" · {subtitle}"))
            .unwrap_or_default();
        content.push(text(format!("标题：{blueprint_title}{subtitle}")));
        if let Some(note_text) = field_str(blueprint, "note") {
            content.push(note(note_text.to_string()));
        }
    } else {
        content.push(text("内容尚未生成完成。".to_string()));
    }
    body.push(wrap(content));

    // 「查看当前页调试信息」（原版 `<details v-if="selectedSlide.contentLog">`）。
    if let Some(log) = item.content_log.as_ref() {
        let detail = log.detail.as_object();
        let mut blocks: Vec<AnyElement> = Vec::new();
        for (position, (label, key)) in [
            ("System Prompt", "system_prompt"),
            ("User Prompt", "user_prompt"),
            ("模型原始输出", "raw_output"),
            ("解析结果", "parsed_output"),
        ]
        .into_iter()
        .enumerate()
        {
            let Some(value) = detail.and_then(|detail| detail.get(key)) else {
                continue;
            };
            let value = detail_text(value);
            if value.is_empty() {
                continue;
            }
            blocks.push(
                div()
                    .mt(px(12.0))
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(12.2))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.muted)
                            .child(label),
                    )
                    .child(
                        div()
                            .id(("s2-debug", position))
                            .p(px(12.0))
                            .max_h(px(240.0))
                            .w_full()
                            .overflow_y_scroll()
                            .rounded(px(10.0))
                            .border_1()
                            .border_color(c.border)
                            .bg(c.bg)
                            .font_family(args.code_font.clone())
                            .text_size(px(11.5))
                            .text_color(c.text)
                            .child(value),
                    )
                    .into_any_element(),
            );
        }
        let open = args.debug_open;
        let this = args.this.clone();
        body.push(
            div()
                .p(px(12.0))
                .rounded(px(12.0))
                .border_1()
                .border_color(c.border)
                .bg(c.surface)
                .child(
                    div()
                        .id("s2-debug-toggle")
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .cursor_pointer()
                        .child(section_title("查看当前页调试信息"))
                        .child(carbon(if open { "chevron-up" } else { "chevron-down" }))
                        .on_click(move |_event, _window, cx| {
                            let _ = this.update(cx, |this, cx| {
                                this.debug_open = !this.debug_open;
                                cx.notify();
                            });
                        }),
                )
                .when(open, |panel| panel.children(blocks))
                .into_any_element(),
        );
    }

    // 「查看最近日志」（原版 `<details class="sdp-debug">`：最后 10 条倒序）。
    let open = args.logs_open;
    let this = args.this.clone();
    body.push(
        div()
            .p(px(12.0))
            .rounded(px(12.0))
            .border_1()
            .border_color(c.border)
            .bg(c.surface)
            .child(
                div()
                    .id("s2-logs-toggle")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .cursor_pointer()
                    .child(section_title("查看最近日志"))
                    .child(carbon(if open { "chevron-up" } else { "chevron-down" }))
                    .on_click(move |_event, _window, cx| {
                        let _ = this.update(cx, |this, cx| {
                            this.logs_open = !this.logs_open;
                            cx.notify();
                        });
                    }),
            )
            .when(open, |panel| {
                panel.child(div().mt(px(4.0)).flex().flex_col().gap(px(10.4)).children(
                    args.recent_logs.iter().map(|log| {
                        div()
                            .p(px(10.4))
                            .rounded(px(10.0))
                            .border_1()
                            .border_color(c.border)
                            .bg(c.surface)
                            .child(div().text_size(px(11.5)).text_color(c.muted).child(format!(
                                "{} · #{}",
                                progress::log_stage_label(&log.stage),
                                log.seq
                            )))
                            .child(div().mt(px(3.2)).text_size(px(12.8)).child(
                                if log.title.is_empty() {
                                    log.summary.clone()
                                } else {
                                    log.title.clone()
                                },
                            ))
                    }),
                ))
            })
            .into_any_element(),
    );

    shell(
        div()
            .id("s2-detail")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                div()
                    .p(px(PANEL_PAD))
                    .border_b_1()
                    .border_color(c.border)
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(c.muted)
                            .child(format!("第 {} 张", index + 1)),
                    )
                    .child(
                        div()
                            .mt(px(4.8))
                            .text_size(px(17.6))
                            .font_weight(FontWeight::BOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .mt(px(12.0))
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(px(8.0))
                            .child(widgets::chip(
                                status.label.clone(),
                                tone_of(&status.tone),
                                cx,
                            ))
                            .when(
                                item.layout_plan
                                    .as_ref()
                                    .and_then(|plan| field_str(plan, "kind"))
                                    .is_some(),
                                |meta| {
                                    // 原版 `.sdp-chip--accent`（绿）。
                                    meta.child(widgets::chip(
                                        kind_label.clone(),
                                        Tone2::Success,
                                        cx,
                                    ))
                                },
                            )
                            .when_some(section_label, |meta, label| {
                                meta.child(widgets::chip(label, Tone2::Muted, cx))
                            }),
                    ),
            )
            .child(
                div()
                    .p(px(PANEL_PAD))
                    .border_b_1()
                    .border_color(c.border)
                    .child(preview),
            )
            .child(
                div()
                    .p(px(PANEL_PAD))
                    .flex()
                    .flex_col()
                    .gap(px(14.4))
                    .children(body),
            )
            .into_any_element(),
    )
}

/// 原版 `detailText`：字符串原样，其余 `JSON.stringify(value, null, 2)`。
fn detail_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

/// 调试块的等宽字体（设置页的「代码」角色）。
fn code_font(handle: &ShellHandle) -> SharedString {
    let family = handle
        .shared()
        .lock()
        .ok()
        .map(|state| state.app_settings.fonts.code.clone())
        .unwrap_or_default();
    if family.is_empty() {
        SharedString::from("Consolas")
    } else {
        SharedString::from(family)
    }
}

// ── 选择重试布局（原版 `RetryLayoutDialog.vue`） ─────────────────────────────

/// 原版 `RetryLayoutDialog.vue`：选一个版式，让这一页按它重新生成。
struct RetryLayoutDialog {
    store: Entity<GenerationStore>,
    /// 原版 `requestVersion`：关掉对话框后迟到的结果作废。
    version: u64,
    page_index: usize,
    /// 候选版式：`label`（`editor_schema` 的中文名）与 kind 同序，下拉按 label 匹配回 kind。
    choices: Vec<(String, SlideKind)>,
    select: Option<Entity<SelectState<Vec<String>>>>,
    selected: Option<SlideKind>,
    loading: bool,
    error: String,
    subscriptions: Vec<Subscription>,
}

impl RetryLayoutDialog {
    fn new(store: Entity<GenerationStore>, _cx: &mut Context<Self>) -> Self {
        Self {
            store,
            version: 0,
            page_index: 0,
            choices: Vec::new(),
            select: None,
            selected: None,
            loading: false,
            error: String::new(),
            subscriptions: Vec::new(),
        }
    }

    /// 原版 `open(runId, index)`。
    fn load(
        &mut self,
        run_id: Option<i64>,
        page_index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.version += 1;
        let version = self.version;
        self.page_index = page_index;
        self.choices.clear();
        self.select = None;
        self.selected = None;
        self.loading = true;
        self.error.clear();
        cx.notify();

        // 原版 `if (!runId) throw new Error('缺少生成记录，请重新生成')`。
        let Some(run_id) = run_id else {
            self.loading = false;
            self.error = "缺少生成记录，请重新生成".to_string();
            cx.notify();
            return;
        };
        let shared = self.store.read(cx).shared().clone();
        run_bg(
            window,
            cx,
            move || {
                generation::get_retry_layouts(&shared, run_id, page_index)
                    .map_err(|error| error.to_string())
            },
            move |this: &mut Self, result, window, cx| this.apply(version, result, window, cx),
        );
    }

    /// 原版 `open` 的 `try` / `catch` / `finally`。
    fn apply(
        &mut self,
        version: u64,
        result: Result<generation::RetryLayoutOptions, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if version != self.version {
            return;
        }
        self.loading = false;
        match result {
            Ok(options) => {
                let schema = editor_schema::editor_schema();
                self.choices = options
                    .kinds
                    .iter()
                    .map(|kind| (kind_label(&schema, kind), kind.clone()))
                    .collect();
                // 原版：默认选中当前版式，不在候选里就选第一个。
                let index = self
                    .choices
                    .iter()
                    .position(|(_, kind)| *kind == options.current_kind)
                    .unwrap_or(0);
                self.selected = self.choices.get(index).map(|(_, kind)| kind.clone());
                if self.choices.is_empty() {
                    self.error = "这一页暂无可用布局".to_string();
                } else {
                    let items: Vec<String> = self
                        .choices
                        .iter()
                        .map(|(label, _)| label.clone())
                        .collect();
                    let select = cx
                        .new(|cx| SelectState::new(items, Some(IndexPath::new(index)), window, cx));
                    self.subscriptions = vec![cx.subscribe(
                        &select,
                        |this: &mut Self, _state, event: &SelectEvent<Vec<String>>, cx| {
                            if let SelectEvent::Confirm(Some(label)) = event {
                                this.selected = this
                                    .choices
                                    .iter()
                                    .find(|(name, _)| name == label)
                                    .map(|(_, kind)| kind.clone());
                                cx.notify();
                            }
                        },
                    )];
                    self.select = Some(select);
                }
            }
            Err(error) => self.error = error,
        }
        cx.notify();
    }

    /// 原版 `confirm()`：选的版式必须是候选里的一个，再交给 store 重试这一页。
    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(kind) = self
            .selected
            .clone()
            .filter(|kind| self.choices.iter().any(|(_, choice)| choice == kind))
        else {
            return;
        };
        if self.loading || !self.error.is_empty() {
            return;
        }
        let index = self.page_index as i64;
        self.version += 1;
        self.store
            .update(cx, |store, cx| store.retry_page(index, Some(kind), cx));
        window.close_dialog(cx);
    }

    /// 原版 `close()`：版本号 +1，迟到的结果作废。
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.version += 1;
        window.close_dialog(cx);
    }
}

impl Render for RetryLayoutDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        let description = self
            .selected
            .as_ref()
            .map(|kind| kind_desc(&editor_schema::editor_schema(), kind))
            .unwrap_or_default();
        let can_confirm = !self.loading
            && self.error.is_empty()
            && self
                .selected
                .as_ref()
                .map(|kind| self.choices.iter().any(|(_, choice)| choice == kind))
                .unwrap_or(false);

        // 外框（底色、边框、圆角、关闭钮）由 GPUI Kit 的 Dialog 画，这里只排内容。
        div()
            .w_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_size(px(20.8))
                    .font_weight(FontWeight::BOLD)
                    .mb(px(12.0))
                    .child("选择重试布局"),
            )
            .child(div().text_size(px(14.0)).child(format!(
                "第 {} 页将按所选布局重新生成。",
                self.page_index + 1
            )))
            .when(self.loading, |dialog| {
                dialog.child(
                    div()
                        .mt(px(12.0))
                        .text_size(px(14.0))
                        .child("正在读取可用布局…"),
                )
            })
            .when(!self.loading && !self.error.is_empty(), |dialog| {
                dialog.child(
                    div()
                        .mt(px(12.0))
                        .text_size(px(14.0))
                        .text_color(c.error)
                        .child(self.error.clone()),
                )
            })
            .when(!self.loading && self.error.is_empty(), |dialog| {
                dialog
                    .child(
                        div()
                            .mt(px(20.0))
                            .mb(px(8.0))
                            .text_size(px(14.0))
                            .child("页面布局"),
                    )
                    .when_some(self.select.clone(), |dialog, select| {
                        dialog.child(
                            div()
                                .w_full()
                                .child(Select::new(&select).w_full().placeholder("请选择布局")),
                        )
                    })
                    .child(
                        div()
                            .mt(px(12.0))
                            .min_h(px(42.0))
                            .text_size(px(14.0))
                            .text_color(c.muted)
                            .child(description),
                    )
            })
            .child(
                div()
                    .mt(px(24.0))
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap(px(12.0))
                    .child(
                        Button::new("s2-retry-cancel")
                            .ghost()
                            .label("取消")
                            .on_click(
                                cx.listener(|this, _event, window, cx| this.close(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("s2-retry-confirm")
                            .primary()
                            .label("按此布局重试")
                            .disabled(!can_confirm)
                            .on_click(
                                cx.listener(|this, _event, window, cx| this.confirm(window, cx)),
                            ),
                    ),
            )
    }
}

/// `editor_schema` 里这个版式的中文名（原版 `schemaStore.kindMeta[kind]?.label ?? kind`）。
fn kind_label(schema: &EditorSchema, kind: &SlideKind) -> String {
    let name = kind.as_str();
    schema
        .kinds
        .iter()
        .find(|entry| entry.kind == name)
        .map(|entry| entry.label.clone())
        .unwrap_or_else(|| name.to_string())
}

/// `editor_schema` 里这个版式的说明（原版 `schemaStore.kindMeta[kind]?.desc`）。
fn kind_desc(schema: &EditorSchema, kind: &SlideKind) -> String {
    let name = kind.as_str();
    schema
        .kinds
        .iter()
        .find(|entry| entry.kind == name)
        .map(|entry| entry.desc.clone())
        .unwrap_or_default()
}
