//! 第二步「AI 生成」（原版 `steps/Step2Generate.vue` + `stores/generation.ts`）——**骨架**。
//!
//! 已实现：顶部状态与统计、「幻灯片生成看板」两列卡片（序号、标题、版式、Typst 缩略图）、
//! 右侧「幻灯片详情」（大图、标题、版式）、底栏「前往编辑」。
//! 待实现：生成流水线本身（`services::generation::generate_slides` + `tasks::listen_events`
//! 接 `CoreEvent`）、整体进度与五个阶段、逐页状态与阶段标签、详情里的规划/日志/调试段、
//! 单页重试与换版式（`RetryLayoutDialog.vue`）。

use gpui_kit::component::Disableable as _;
use gpui_kit::prelude::*;
use gpui_kit::{div, px, Context, Entity, Render, Subscription, WeakEntity, Window};
use huanshu_core::editor_schema;
use huanshu_core::preview::PreviewOptions;

use super::{ProjectModel, WorkflowPage};
use crate::generation::GenerationStore;
use crate::icons::carbon;
use crate::router::Step;
use crate::shell::ShellHandle;
use crate::slides::{slide_frame, SlideImages};
use crate::theme::StudioExt as _;
use crate::widgets::{self, Tone2};

/// 看板卡片里缩略图的宽度与详情大图的宽度（逻辑像素，原版实测）。
const CARD_W: f32 = 300.0;
const DETAIL_W: f32 = 380.0;

pub struct Step2 {
    model: Entity<ProjectModel>,
    /// 应用级生成状态（原版 `stores/generation.ts`）：页面只读它，状态不在这里。
    store: Entity<GenerationStore>,
    page: WeakEntity<WorkflowPage>,
    selected: usize,
    thumbs: SlideImages,
    detail: SlideImages,
    kind_labels: Vec<(String, String)>,
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,
}

impl Step2 {
    pub fn new(
        model: Entity<ProjectModel>,
        handle: ShellHandle,
        page: WeakEntity<WorkflowPage>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let id = model.read(cx).id;
        let store = handle.generation();
        let shared = handle.shared().clone();
        let subscriptions = vec![
            cx.observe(&model, |_this, _model, cx| cx.notify()),
            cx.observe(&store, |_this, _store, cx| cx.notify()),
        ];
        Self {
            model,
            store,
            page,
            selected: 0,
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
            kind_labels: editor_schema::editor_schema()
                .kinds
                .into_iter()
                .map(|entry| (entry.kind, entry.label))
                .collect(),
            subscriptions,
        }
    }

    pub fn release(&mut self, cx: &mut Context<Self>) {
        self.thumbs.release(cx);
        self.detail.release(cx);
    }

    fn kind_label(&self, slide: &serde_json::Value) -> String {
        let kind = slide
            .get("kind")
            .and_then(|kind| kind.as_str())
            .unwrap_or("");
        self.kind_labels
            .iter()
            .find(|(name, _)| name == kind)
            .map(|(_, label)| label.clone())
            .unwrap_or_else(|| kind.to_string())
    }
}

impl Render for Step2 {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        let (deck, aspect) = {
            let model = self.model.read(cx);
            (model.deck.clone(), model.aspect_ratio())
        };
        // 生成状态一律来自应用级 store（原版 props 都来自 `useGenerationStore()`）。
        let state = {
            let model = self.model.read(cx);
            self.store
                .read(cx)
                .state_for(model.id, model.latest_run.as_ref(), &model.deck)
        };
        let count = deck.len();
        self.selected = self.selected.min(count.saturating_sub(1));

        // 顶部标题（原版 `Step2Generate.vue` 的 `.gen-hd-title`）：
        // 生成中 → 「{stageLabel}...」、完成 → 「生成完成 · N 张幻灯片」（N = 看板条目数）、
        // 出错 → 「生成遇到问题」、否则「准备开始」；第二行是 `message`。
        let (status_title, status_sub, status_color) = if state.running {
            (
                format!("{}...", state.stage_label()),
                state.message.clone(),
                c.primary,
            )
        } else if state.stage == "done" {
            (
                format!("生成完成 · {} 张幻灯片", state.board_items),
                state.message.clone(),
                c.success,
            )
        } else if state.stage == "error" {
            ("生成遇到问题".to_string(), state.message.clone(), c.error)
        } else {
            ("准备开始".to_string(), state.message.clone(), c.muted)
        };

        let slots: Vec<Option<serde_json::Value>> = deck.iter().cloned().map(Some).collect();
        self.thumbs.set_deck(slots.clone());
        self.thumbs.set_width(CARD_W, window);
        // TODO(parity): 只请求可见卡片（虚拟列表 + wanted），现在整副都请求。
        self.thumbs
            .pump(None, |this: &mut Self| &mut this.thumbs, window, cx);
        self.detail.set_deck(slots);
        self.detail.set_width(DETAIL_W, window);
        self.detail.set_wanted(&[self.selected]);
        self.detail.pump(
            Some(self.selected),
            |this: &mut Self| &mut this.detail,
            window,
            cx,
        );

        let stat = |value: String, label: &'static str| {
            div()
                .w(px(64.0))
                .py(px(6.0))
                .flex()
                .flex_col()
                .items_center()
                .rounded(px(8.0))
                .border_1()
                .border_color(c.border)
                .bg(c.surface)
                .child(div().text_size(px(18.0)).child(value))
                .child(div().text_size(px(10.5)).text_color(c.muted).child(label))
        };
        let header = div()
            .flex_none()
            .h(px(78.0))
            .px(px(22.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(14.0))
            .bg(c.panel)
            .border_b_1()
            .border_color(c.border)
            .child(div().size(px(10.0)).rounded_full().bg(status_color))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(div().text_size(px(16.0)).child(status_title))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(c.muted)
                            .child(status_sub),
                    ),
            )
            .child(stat(state.board_items.to_string(), "幻灯片"))
            // TODO(parity): 「关键事件」「生成记录」来自 get_generation_logs。
            .child(stat("—".to_string(), "关键事件"))
            .child(stat("—".to_string(), "生成记录"));

        let progress = div().flex_none().p(px(16.0)).child(widgets::todo_block(
            "整体进度",
            "Step2Generate.vue「整体进度」+ utils/generationProgress.ts",
            &[
                "百分比与进度条、「查看详情」折叠",
                "五个阶段：页面规划 / 布局排版 / 内容生成 / 规范修复 / 校验修复，各自 n/N",
                "数据来自 CoreEvent::Progress / PageStatus（tasks::listen_events）",
            ],
            cx,
        ));

        let selected = self.selected;
        let cards = div().flex().flex_row().flex_wrap().gap(px(16.0)).children(
            deck.iter().enumerate().map(|(index, slide)| {
                let title = slide
                    .get("title")
                    .and_then(|title| title.as_str())
                    .unwrap_or("")
                    .to_string();
                let active = index == selected;
                div()
                    .id(("card", index))
                    .w(px(CARD_W + 28.0))
                    .p(px(14.0))
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .rounded(px(10.0))
                    .bg(c.panel)
                    .border_1()
                    .border_color(if active { c.primary } else { c.border })
                    .cursor_pointer()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(c.muted)
                                    .child(format!("#{}", index + 1)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(15.0))
                                    .child(title),
                            )
                            .child(widgets::chip("已完成", Tone2::Success, cx)),
                    )
                    // TODO(parity): 五个阶段标签（规划/布局/内容/规范/校验）按 PageStatus 着色。
                    .child(div().flex().flex_row().child(widgets::chip(
                        self.kind_label(slide),
                        Tone2::Primary,
                        cx,
                    )))
                    .child(slide_frame(
                        ("card-frame", index),
                        self.thumbs.frame_state(index),
                        CARD_W,
                        CARD_W * aspect,
                        false,
                        None,
                        cx,
                    ))
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.selected = index;
                        cx.notify();
                    }))
            }),
        );
        let board = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .h(px(36.0))
                    .px(px(16.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .text_size(px(12.5))
                    .text_color(c.muted)
                    .child(carbon("list-boxes"))
                    .child("幻灯片生成看板")
                    .child(div().flex_1())
                    .child(widgets::chip(
                        state.board_items.to_string(),
                        Tone2::Muted,
                        cx,
                    )),
            )
            .child(
                div()
                    .id("board")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(16.0))
                    .child(cards),
            );

        let detail_slide = deck.get(selected).cloned();
        let detail = div()
            .id("detail")
            .flex_none()
            .w(px(DETAIL_W + 36.0))
            .h_full()
            .overflow_y_scroll()
            .p(px(18.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .bg(c.panel)
            .border_l_1()
            .border_color(c.border)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(6.0))
                    .text_size(px(12.5))
                    .text_color(c.muted)
                    .child(carbon("view"))
                    .child("幻灯片详情"),
            )
            .when_some(detail_slide, |panel, slide| {
                let title = slide
                    .get("title")
                    .and_then(|title| title.as_str())
                    .unwrap_or("")
                    .to_string();
                panel
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(c.muted)
                            .child(format!("第 {} 张", selected + 1)),
                    )
                    .child(div().text_size(px(18.0)).child(title))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(6.0))
                            .child(widgets::chip("已完成", Tone2::Success, cx))
                            .child(widgets::chip(self.kind_label(&slide), Tone2::Primary, cx)),
                    )
                    .child(slide_frame(
                        "detail-frame",
                        self.detail.frame_state(selected),
                        DETAIL_W,
                        DETAIL_W * aspect,
                        false,
                        None,
                        cx,
                    ))
                    .child(widgets::todo_block(
                        "详情分段",
                        "Step2Generate.vue「slide-detail-panel」",
                        &[
                            "规划页面（page_plan 原文与要点）",
                            "生成日志（逐条，可展开 detail JSON）",
                            "重试本页 / 换版式重试",
                        ],
                        cx,
                    ))
            });

        let page = self.page.clone();
        let bottom = div()
            .flex_none()
            .h(px(60.0))
            .px(px(18.0))
            .flex()
            .flex_row()
            .items_center()
            .bg(c.panel)
            .border_t_1()
            .border_color(c.border)
            .child(
                widgets::primary_button("to-edit", "前往编辑")
                    .icon(carbon("edit"))
                    .disabled(count == 0)
                    .on_click(move |_event, window, cx| {
                        let _ = page.update(cx, |page, cx| page.show_step(Step::Edit, window, cx));
                    }),
            );

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(header)
            .child(progress)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .child(board)
                    .child(detail),
            )
            .child(bottom)
    }
}
