//! 第三步「编辑完善」（原版 `steps/Step3Editor.vue` + `composables/useSlideEditor.ts`）——**骨架**。
//!
//! 已实现：工具栏（幻灯片数、页码、导出 PDF、演示）、左侧幻灯片列表（序号、标题、版式）、
//! ↑/↓ 换页（当前页是 [`ProjectModel::active_slide`]）、中间 Typst 预览、右侧「幻灯片类型」
//! 卡片与按编辑器模式表列出的字段（只读）。
//! 待实现：字段表单（编辑器模式表驱动的输入框、列表区、对象区、图标推荐、素材插图）、
//! 图表 JSON 框与 `chart_problems` 校验、高级 JSON、换版式（KindPicker）、新建/删除/移动页、
//! 应用更改（Ctrl+S，走 `WorkflowPage::commit_deck`——牌组唯一的写入路径）、未保存标记、裁切提示。

use gpui_kit::component::Disableable as _;
use gpui_kit::prelude::*;
use gpui_kit::{
    div, px, Context, Entity, FocusHandle, KeyDownEvent, Render, Subscription, WeakEntity, Window,
};
use huanshu_core::editor_schema::{self, EditorSchema};
use huanshu_core::preview::PreviewOptions;

use super::{ProjectModel, WorkflowPage};
use crate::icons::carbon;
use crate::present::PresentArgs;
use crate::shell::ShellHandle;
use crate::slides::{slide_frame, SlideImages};
use crate::tasks::run_bg;
use crate::theme::StudioExt as _;
use crate::widgets::{self, Tone2};

/// 左侧列表与右侧编辑栏的宽度（原版实测，逻辑像素）。
const LIST_W: f32 = 200.0;
const PANEL_W: f32 = 360.0;

pub struct Step3 {
    model: Entity<ProjectModel>,
    /// 工作流页面：牌组的编辑走它的 [`WorkflowPage::commit_deck`]（唯一写入路径）、
    /// 生成中只读看 [`WorkflowPage::editing_locked`]。
    #[allow(dead_code)]
    page: WeakEntity<WorkflowPage>,
    handle: ShellHandle,
    schema: EditorSchema,
    preview: SlideImages,
    exporting: bool,
    focus: FocusHandle,
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
        let subscriptions = vec![cx.observe(&model, |_this, _model, cx| cx.notify())];
        Self {
            model,
            page,
            handle,
            schema: editor_schema::editor_schema(),
            preview,
            exporting: false,
            focus,
            subscriptions,
        }
    }

    pub fn release(&mut self, cx: &mut Context<Self>) {
        self.preview.release(cx);
    }

    /// 当前页（原版 `activeSlide`）：它是 [`ProjectModel`] 的字段，切步骤回来还在。
    fn active(&self, cx: &Context<Self>) -> usize {
        let model = self.model.read(cx);
        model.active_slide.min(model.deck.len().saturating_sub(1))
    }

    /// 换当前页（原版 `v-model:active-slide`）。换页本身不改牌组，写的是模型的字段。
    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.model.update(cx, |model, cx| {
            if index < model.deck.len() && model.active_slide != index {
                model.active_slide = index;
                cx.notify();
            }
        });
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let active = self.active(cx);
        match event.keystroke.key.as_str() {
            "up" => self.select(active.saturating_sub(1), cx),
            "down" => self.select(active + 1, cx),
            _ => {}
        }
    }

    fn present(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let start = self.active(cx);
        let model = self.model.read(cx);
        let args = PresentArgs {
            project_id: model.id,
            deck: model.deck.clone(),
            start,
            aspect: model.aspect_ratio(),
        };
        self.handle.present(args, window, cx);
    }

    /// 原版「导出 PDF」：导出的是编辑器里**已应用**的牌组（这里就是模型里的牌组）。
    fn export_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (id, name) = {
            let model = self.model.read(cx);
            (model.id, model.name.clone())
        };
        let Some(id) = id else { return };
        let Some(path) = crate::export::ask_save_path(&format!("{name}.pdf"), "PDF", "pdf") else {
            return;
        };
        self.exporting = true;
        cx.notify();
        let shared = self.handle.shared().clone();
        let theme = cx.studio().theme.id.to_string();
        run_bg(
            window,
            cx,
            move || crate::app::export_pdf(&shared, id, path, theme),
            |this: &mut Self, result, window, cx| {
                this.exporting = false;
                crate::export::show_report(result, window, cx);
                cx.notify();
            },
        );
    }
}

impl Render for Step3 {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        let (deck, aspect) = {
            let model = self.model.read(cx);
            (model.deck.clone(), model.aspect_ratio())
        };
        let count = deck.len();
        // 渲染里不改模型（那会在渲染途中触发观察者）；越界只在这一帧按最后一页显示，
        // 真正的收敛在 `on_generation_changed` 与 `commit_deck` 里。
        let selected = self.active(cx);

        // 舞台：在列表与右栏之间取最大的、保持比例的框。
        let viewport = window.viewport_size();
        let avail_w = (viewport.width.as_f32() - 204.0 - LIST_W - PANEL_W - 48.0).max(200.0);
        let avail_h =
            (viewport.height.as_f32() - crate::theme::TITLEBAR_HEIGHT - 56.0 - 48.0).max(120.0);
        let (mut stage_w, mut stage_h) = (avail_w, avail_w * aspect);
        if stage_h > avail_h {
            stage_h = avail_h;
            stage_w = stage_h / aspect;
        }
        // TODO(parity): 编辑时原版只发当前这一页（`slides = [editorPreviewSlide]`，单页编译最快）。
        self.preview
            .set_deck(deck.iter().cloned().map(Some).collect());
        self.preview.set_width(stage_w, window);
        self.preview.set_wanted(&[selected]);
        self.preview.pump(
            Some(selected),
            |this: &mut Self| &mut this.preview,
            window,
            cx,
        );

        let kind_of = |slide: &serde_json::Value| {
            slide
                .get("kind")
                .and_then(|kind| kind.as_str())
                .unwrap_or("")
                .to_string()
        };
        let toolbar = div()
            .flex_none()
            .h(px(56.0))
            .px(px(18.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(14.0))
            .bg(c.panel)
            .border_b_1()
            .border_color(c.border)
            .child(
                div()
                    .w(px(LIST_W - 36.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(carbon("layers"))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(c.muted)
                            .child("幻灯片"),
                    )
                    .child(widgets::chip(count.to_string(), Tone2::Muted, cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .text_size(px(14.0))
                    .text_color(c.muted)
                    .child(carbon("view"))
                    .child("实时预览"),
            )
            .child(div().flex_1())
            .child(div().text_size(px(13.0)).text_color(c.muted).child(format!(
                "{} / {}",
                selected + 1,
                count
            )))
            .child(div().flex_1())
            .child(
                widgets::ghost_button(
                    "export-pdf",
                    if self.exporting {
                        "导出中…"
                    } else {
                        "导出 PDF"
                    },
                )
                .icon(carbon("document-pdf"))
                .disabled(self.exporting)
                .on_click(cx.listener(|this, _event, window, cx| this.export_pdf(window, cx))),
            )
            .child(
                widgets::primary_button("present", "演示")
                    .icon(carbon("presentation-file"))
                    .on_click(cx.listener(|this, _event, window, cx| this.present(window, cx))),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(c.muted)
                    .child(format!("编辑 第 {} 张", selected + 1)),
            );

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
                div()
                    .id("slide-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(8.0))
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .children(deck.iter().enumerate().map(|(index, slide)| {
                        let active = index == selected;
                        let title = slide
                            .get("title")
                            .and_then(|title| title.as_str())
                            .unwrap_or("")
                            .to_string();
                        let label = self
                            .schema
                            .kind(&kind_of(slide))
                            .map(|entry| entry.label.clone())
                            .unwrap_or_else(|| kind_of(slide));
                        div()
                            .id(("slide-row", index))
                            .flex()
                            .flex_row()
                            .gap(px(12.0))
                            .px(px(12.0))
                            .py(px(10.0))
                            .rounded(px(8.0))
                            .border_1()
                            .border_color(if active {
                                c.primary
                            } else {
                                gpui_kit::transparent_black()
                            })
                            .when(active, |row| row.bg(c.primary_bg))
                            .cursor_pointer()
                            .child(
                                div()
                                    .w(px(18.0))
                                    .text_size(px(12.0))
                                    .text_color(c.muted)
                                    .child((index + 1).to_string()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(div().truncate().text_size(px(14.0)).child(title))
                                    .child(
                                        div().text_size(px(12.0)).text_color(c.muted).child(label),
                                    ),
                            )
                            .on_click(
                                cx.listener(move |this, _event, _window, cx| {
                                    this.select(index, cx)
                                }),
                            )
                    })),
            )
            // TODO(parity): 新建幻灯片 → KindPicker（add 模式），插在当前页之后，继承章节号。
            .child(
                div()
                    .flex_none()
                    .p(px(10.0))
                    .border_t_1()
                    .border_color(c.border)
                    .child(
                        widgets::ghost_button("new-slide", "新建幻灯片")
                            .icon(carbon("add"))
                            .disabled(true),
                    ),
            );

        let clipped = self.preview.overflow(selected);
        let stage = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .child(slide_frame(
                "editor-stage",
                self.preview.frame_state(selected),
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
            ))
            // 原版：内容缩到最小仍放不下时，预览下方提示。
            .when(clipped, |stage| {
                stage.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(c.warning)
                        .child("内容过多，已缩到最小字号仍放不下，超出部分被裁切"),
                )
            });

        let slide = deck.get(selected).cloned().unwrap_or_default();
        let kind = kind_of(&slide);
        let entry = self.schema.kind(&kind).cloned();
        let fields: Vec<String> = entry
            .as_ref()
            .map(|entry| entry.fields.clone())
            .unwrap_or_default();
        let value_text = |key: &str| -> String {
            match slide.get(key) {
                None | Some(serde_json::Value::Null) => "—".to_string(),
                Some(serde_json::Value::String(text)) => text.clone(),
                Some(serde_json::Value::Array(items)) => format!("{} 项", items.len()),
                Some(serde_json::Value::Object(_)) => "（对象）".to_string(),
                Some(other) => other.to_string(),
            }
        };
        let simple: Vec<_> = self
            .schema
            .simple_fields
            .iter()
            .filter(|field| fields.contains(&field.key))
            .cloned()
            .collect();
        let panel = div()
            .id("editor-panel")
            .flex_none()
            .w(px(PANEL_W))
            .h_full()
            .overflow_y_scroll()
            .p(px(16.0))
            .flex()
            .flex_col()
            .gap(px(14.0))
            .bg(c.panel)
            .border_l_1()
            .border_color(c.border)
            .child(widgets::section_label("幻灯片类型", cx))
            .child(
                widgets::panel(cx)
                    .px(px(14.0))
                    .py(px(12.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .when_some(entry.clone(), |card, entry| {
                        card.child(crate::icons::carbon(&entry.icon))
                            .child(div().text_size(px(15.0)).child(entry.label))
                    })
                    .child(div().text_size(px(12.0)).text_color(c.muted).child(kind.clone()))
                    .child(div().flex_1())
                    // TODO(parity): 「更换」打开 KindPicker（change 模式，useSlideEditor::onKindPicked）。
                    .child(widgets::small(widgets::ghost_button("change-kind", "更换")).disabled(true)),
            )
            .child(widgets::section_label("基础信息", cx))
            .children(simple.into_iter().map(|field| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .px(px(12.0))
                    .py(px(8.0))
                    .rounded(px(8.0))
                    .bg(c.surface)
                    .border_1()
                    .border_color(c.border)
                    .child(div().text_size(px(12.0)).text_color(c.muted).child(field.label.clone()))
                    .child(div().text_size(px(14.0)).child(value_text(&field.key)))
            }))
            .child(widgets::todo_block(
                "字段编辑",
                "Step3Editor.vue（表单）+ huanshu_core::editor_schema",
                &[
                    "上面的只读行换成输入框 / 多行 / 下拉 / 开关（EditorField.field_type）",
                    "列表区（list_sections：增删、上移下移、new_item 模板、嵌套列表、图标推荐）",
                    "对象区（object_sections：对比左右、SWOT 四象限）",
                    "图表 JSON 框：150ms 防抖调 chart_check::chart_problems_json，通过才写回；失焦立即校验",
                    "高级 JSON 模式、应用更改（Ctrl+S）、未保存标记、「超出部分被裁切」提示",
                ],
                cx,
            ));

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
            .flex_col()
            .child(toolbar)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .child(list)
                    .child(stage)
                    .child(panel),
            )
    }
}
