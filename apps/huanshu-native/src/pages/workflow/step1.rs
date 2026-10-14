//! 第一步「准备素材」（原版 `steps/Step1Prepare.vue` + `MarkdownWorkbench.vue`）——**骨架**。
//!
//! 已实现：顶栏（名称输入、字数、AI 优化 / 上传文件按钮位）、格式工具行（按钮位）、
//! Markdown 编辑器（GPUI Kit `Editor`，tree-sitter 高亮）与实时预览（`TextView::markdown`）、
//! 右栏「幻灯片比例」三选一（已有项目时写回）、底栏「开始 AI 生成」与预计页数。
//! 待实现见各处黄色虚线框与 `TODO(parity)`。

use gpui_kit::component::input::{Editor, EditorState, Input, InputEvent, InputState};
use gpui_kit::component::text::TextView;
use gpui_kit::component::Disableable as _;
use gpui_kit::prelude::*;
use gpui_kit::{div, px, Context, Entity, Render, SharedString, Subscription, Window};
use huanshu_core::services::projects;
use huanshu_core::types::AspectRatio;

use super::ProjectModel;
use crate::icons::carbon;
use crate::shell::ShellHandle;
use crate::theme::StudioExt as _;
use crate::widgets::{self, Tone2};

pub struct Step1 {
    model: Entity<ProjectModel>,
    handle: ShellHandle,
    name: Entity<InputState>,
    editor: Entity<EditorState>,
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,
}

impl Step1 {
    pub fn new(
        model: Entity<ProjectModel>,
        handle: ShellHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (name_text, md) = {
            let model = model.read(cx);
            (model.name.clone(), model.md_content.clone())
        };
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("请填写演示名称")
                .default_value(name_text)
        });
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .placeholder("粘贴 Markdown 内容，或拖拽 .md 文件到此处...")
                .default_value(md)
        });
        let subscriptions = vec![
            cx.subscribe(&name, |this: &mut Self, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let value = state.read(cx).value().to_string();
                    this.model.update(cx, |model, cx| {
                        model.name = value;
                        cx.notify();
                    });
                    // TODO(parity): 原版草稿自动保存（`draftSaveState`，防抖后 update_project_content）。
                }
            }),
            cx.observe(&editor, |this: &mut Self, editor, cx| {
                let value = editor.read(cx).value().to_string();
                this.model.update(cx, |model, cx| {
                    if model.md_content != value {
                        model.md_content = value;
                        cx.notify();
                    }
                });
                cx.notify();
            }),
        ];
        Self {
            model,
            handle,
            name,
            editor,
            subscriptions,
        }
    }

    fn set_aspect(&mut self, aspect: AspectRatio, cx: &mut Context<Self>) {
        let id = self.model.read(cx).id;
        self.model.update(cx, |model, cx| {
            model.aspect = aspect;
            cx.notify();
        });
        if let Some(id) = id {
            if let Err(error) =
                projects::update_project_aspect_ratio(self.handle.shared(), id, aspect)
            {
                eprintln!("[native] 保存比例失败：{error}");
            }
        }
        cx.notify();
    }
}

/// 粗略的页数估计：二级标题数、三级标题数。
/// TODO(parity): 原版的分页粒度规则（`Step1Prepare.vue` 的 `granularityOptions`、
/// `services::generation::detect_granularity` 的自动推荐）还没搬过来。
fn heading_counts(md: &str) -> (usize, usize) {
    let h2 = md.lines().filter(|line| line.starts_with("## ")).count();
    let h3 = md.lines().filter(|line| line.starts_with("### ")).count();
    (h2, h3)
}

impl Render for Step1 {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        let model = self.model.read(cx);
        let md = model.md_content.clone();
        let chars = md.chars().count();
        let aspect = model.aspect;
        let has_name = !model.name.trim().is_empty();
        let (h2, h3) = heading_counts(&md);

        let top_bar = div()
            .flex_none()
            .h(px(64.0))
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
                    .text_size(px(14.0))
                    .text_color(c.muted)
                    .child("准备素材"),
            )
            .child(div().w(px(380.0)).child(Input::new(&self.name)))
            .child(div().flex_1())
            .child(widgets::chip(format!("{chars} 字"), Tone2::Primary, cx))
            // TODO(parity): AI 优化（services::generation::optimize_markdown_headings，async → tasks::run_async）。
            .child(
                widgets::ghost_button("ai-optimize", "AI 优化")
                    .icon(carbon("magic-wand"))
                    .disabled(true),
            )
            // TODO(parity): 上传文件（rfd 选 .md/.txt，读入编辑器）。
            .child(
                widgets::ghost_button("upload", "上传文件")
                    .icon(carbon("upload"))
                    .disabled(true),
            );

        let tools = div()
            .flex_none()
            .h(px(44.0))
            .px(px(18.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .border_b_1()
            .border_color(c.border)
            .child(
                div()
                    .pr(px(10.0))
                    .text_size(px(13.0))
                    .text_color(c.muted)
                    .child("格式工具"),
            )
            // TODO(parity): 六个格式工具（MarkdownWorkbench.vue 的 formatTools），改写编辑器内容。
            .children(
                [
                    "自动标题",
                    "清除标题加粗",
                    "删除序号",
                    "`##` 一、二、",
                    "`###` （一）（二）",
                    "`####` 1. 2.",
                ]
                .iter()
                .enumerate()
                .map(|(i, label)| {
                    widgets::small(widgets::ghost_button(("fmt", i), *label)).disabled(true)
                }),
            );

        let pane_header = |icon: &'static str, label: &'static str| {
            div()
                .flex_none()
                .h(px(32.0))
                .px(px(14.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .text_size(px(12.5))
                .text_color(c.muted)
                .border_b_1()
                .border_color(c.border)
                .child(carbon(icon))
                .child(label)
        };
        let workbench = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_row()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .border_r_1()
                    .border_color(c.border)
                    .child(pane_header("edit", "编辑"))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .child(Editor::new(&self.editor).size_full()),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(pane_header("view", "预览"))
                    // TODO(parity): 原版按分页粒度把预览切成 P1/P2… 卡片，```chart 围栏显示图表校验结果。
                    .child(
                        div()
                            .id("md-preview")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .p(px(20.0))
                            .child(if md.trim().is_empty() {
                                div()
                                    .text_color(c.muted)
                                    .child("预览区将实时显示 Markdown 渲染效果")
                                    .into_any_element()
                            } else {
                                TextView::markdown("md-preview-text", md.clone()).into_any_element()
                            }),
                    ),
            );

        let aspect_card = |ratio: AspectRatio,
                           title: &'static str,
                           desc: &'static str,
                           cx: &mut Context<Self>| {
            let selected = aspect == ratio;
            div()
                .id(SharedString::from(format!("aspect-{title}")))
                .flex_1()
                .py(px(14.0))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(4.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(if selected { c.primary_border } else { c.border })
                .bg(if selected { c.primary_bg } else { c.surface })
                .cursor_pointer()
                .child(div().text_size(px(16.0)).child(title))
                .child(div().text_size(px(12.0)).text_color(c.muted).child(desc))
                .on_click(cx.listener(move |this, _event, _window, cx| this.set_aspect(ratio, cx)))
        };
        let sidebar = div()
            .id("step1-sidebar")
            .flex_none()
            .w(px(270.0))
            .h_full()
            .overflow_y_scroll()
            .p(px(16.0))
            .flex()
            .flex_col()
            .gap(px(22.0))
            .bg(c.panel)
            .border_l_1()
            .border_color(c.border)
            .child(widgets::todo_block(
                "分页粒度",
                "Step1Prepare.vue「分页粒度」（自动推荐 / 按二级标题 / 按三级标题，各显示页数）",
                &[
                    &format!("当前粗估：二级标题 {h2} 个，三级标题 {h3} 个"),
                    "自动推荐用 services::generation::detect_granularity",
                ],
                cx,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(widgets::section_label("幻灯片比例", cx))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .child(aspect_card(AspectRatio::Ratio16x9, "16:9", "标准宽屏", cx))
                            .child(aspect_card(AspectRatio::Ratio32x9, "32:9", "超宽屏幕", cx))
                            .child(aspect_card(AspectRatio::Ratio48x9, "48:9", "全景屏幕", cx)),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(c.muted)
                            .child("32:9 / 48:9 在 16:9 屏幕上播放时会横向压缩"),
                    ),
            )
            .child(widgets::todo_block(
                "素材库",
                "Step1Prepare.vue「素材库」+ composables/useMediaLibrary.ts",
                &[
                    "文件数、添加素材（rfd 多选图片/视频）、拖拽到此处",
                    "缩略图列表、删除、图注编辑（services::media）",
                    "「素材生成时自动参与图文匹配。」",
                ],
                cx,
            ));

        let bottom = div()
            .flex_none()
            .h(px(60.0))
            .px(px(18.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.0))
            .bg(c.panel)
            .border_t_1()
            .border_color(c.border)
            // TODO(parity): 开始生成 → 建项目（新建时）、保存文稿、切到第 2 步并启动流水线（见 step2）。
            .child(
                widgets::primary_button("generate", "开始 AI 生成")
                    .icon(carbon("magic-wand"))
                    .disabled(true),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(c.muted)
                    .child(if has_name {
                        format!("当前预计生成 {} 张幻灯片", if h3 > 0 { h3 } else { h2 })
                    } else {
                        "请填写演示名称".to_string()
                    }),
            );

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(top_bar)
            .child(tools)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .child(workbench)
                    .child(sidebar),
            )
            .child(bottom)
    }
}
