//! C1. The Markdown editing page.
//!
//! Both multi-line controls of the component library are wired to the same document:
//! `EditorState` (the tree-sitter code editor, with Markdown highlighting turned on)
//! and `TextareaState` (the plain multi-line input), switched at runtime so the two
//! can be compared with the same text. The status bar shows the character count and
//! the caret's line/column.

use std::path::PathBuf;

use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Editor, EditorState, Textarea, TextareaState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::prelude::*;
use gpui_kit::{Context, Entity, SharedString, Subscription, Window, div};

use crate::metrics;

pub struct EditorPage {
    path: PathBuf,
    editor: Entity<EditorState>,
    textarea: Entity<TextareaState>,
    /// Which of the two controls is shown.
    code_editor: bool,
    note: String,
    /// Dropping a subscription cancels it, so they are kept for the view's lifetime.
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,
}

impl EditorPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let path = sample_path();
        let text = std::fs::read_to_string(&path).unwrap_or_else(|err| {
            format!("# 读不到 {}\n\n{err}\n", path.display())
        });
        eprintln!(
            "[spike] sample.md: {} chars, {} lines",
            text.chars().count(),
            text.lines().count()
        );
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .default_value(text.clone())
        });
        let textarea = cx.new(|cx| TextareaState::new(window, cx).default_value(text));
        let subscriptions = vec![
            cx.observe(&editor, |_this, _state, cx| cx.notify()),
            cx.observe(&textarea, |_this, _state, cx| cx.notify()),
        ];
        metrics::report("编辑器载入后");
        Self {
            path,
            editor,
            textarea,
            code_editor: true,
            note: String::new(),
            subscriptions,
        }
    }

    /// Moves the document across when the control is switched, so the text is never
    /// silently lost.
    fn set_code_editor(&mut self, code_editor: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.code_editor == code_editor {
            return;
        }
        let text = if self.code_editor {
            self.editor.read(cx).value().to_string()
        } else {
            self.textarea.read(cx).value().to_string()
        };
        self.code_editor = code_editor;
        if code_editor {
            self.editor
                .update(cx, |state, cx| state.set_value(text, window, cx));
        } else {
            self.textarea
                .update(cx, |state, cx| state.set_value(text, window, cx));
        }
        cx.notify();
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, window| {
            let picked = rfd::AsyncFileDialog::new()
                .add_filter("markdown", &["md", "markdown", "txt"])
                .pick_file()
                .await;
            let Some(file) = picked else {
                return;
            };
            let path = file.path().to_path_buf();
            let wanted = path.clone();
            let text = window
                .background_spawn(async move { std::fs::read_to_string(&wanted) })
                .await;
            let _ = this.update_in(window, |page, window, cx| match text {
                Ok(text) => {
                    page.path = path;
                    page.note = format!("已载入 {} 字符", text.chars().count());
                    page.editor
                        .update(cx, |state, cx| state.set_value(text, window, cx));
                    cx.notify();
                }
                Err(err) => {
                    page.note = format!("读取失败：{err}");
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn save_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = if self.code_editor {
            self.editor.read(cx).value().to_string()
        } else {
            self.textarea.read(cx).value().to_string()
        };
        let suggested = self
            .path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "sample.md".to_string());
        cx.spawn_in(window, async move |this, window| {
            let target = rfd::AsyncFileDialog::new()
                .set_file_name(suggested)
                .add_filter("markdown", &["md"])
                .save_file()
                .await;
            let Some(file) = target else {
                return;
            };
            let path = file.path().to_path_buf();
            let written = window
                .background_spawn(async move {
                    std::fs::write(&path, text).map_err(|err| err.to_string())?;
                    Ok::<PathBuf, String>(path)
                })
                .await;
            let _ = this.update_in(window, |page, _window, cx| {
                page.note = match written {
                    Ok(path) => format!("已写入 {}", path.display()),
                    Err(err) => format!("写入失败：{err}"),
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn reload_sample(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => {
                self.note = format!("已重置为 {} 字符", text.chars().count());
                self.editor
                    .update(cx, |state, cx| state.set_value(text, window, cx));
            }
            Err(err) => self.note = format!("读取失败：{err}"),
        }
        cx.notify();
    }
}

impl Render for EditorPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (chars, line, column) = if self.code_editor {
            let state = self.editor.read(cx);
            let position = state.cursor_position();
            (
                state.text().len_chars(),
                position.line + 1,
                position.character + 1,
            )
        } else {
            let state = self.textarea.read(cx);
            let position = state.cursor_position();
            (
                state.text().len_chars(),
                position.line + 1,
                position.character + 1,
            )
        };
        let code_editor = self.code_editor;
        let note = self.note.clone();
        let path = self.path.file_name().map(|name| name.to_string_lossy().to_string());

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .p_2()
                    .child(
                        Switch::new("markdown-highlight")
                            .label("Markdown 高亮（Editor）")
                            .checked(code_editor)
                            .on_change(cx.listener(|page, checked: &bool, window, cx| {
                                page.set_code_editor(*checked, window, cx)
                            })),
                    )
                    .child(
                        Button::new("open-md")
                            .label("打开 Markdown…")
                            .on_click(cx.listener(|page, _event, window, cx| page.open(window, cx))),
                    )
                    .child(Button::new("save-md").label("另存为…").on_click(
                        cx.listener(|page, _event, window, cx| page.save_as(window, cx)),
                    ))
                    .child(Button::new("reset-md").label("恢复示例文档").on_click(
                        cx.listener(|page, _event, window, cx| page.reload_sample(window, cx)),
                    ))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(SharedString::from(format!(
                                "{} · {}",
                                path.unwrap_or_else(|| "(未命名)".to_string()),
                                note
                            ))),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(if code_editor {
                        Editor::new(&self.editor).size_full().into_any_element()
                    } else {
                        Textarea::new(&self.textarea).size_full().into_any_element()
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_4()
                    .px_2()
                    .py_1()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .child(SharedString::from(format!("{chars} 字符")))
                    .child(SharedString::from(format!("第 {line} 行，第 {column} 列")))
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("输入法（微软拼音 / 搜狗）请在真机上按 README 的清单人工验收"),
                    ),
            )
    }
}

/// `spikes/gpui-kit/assets/sample.md`, resolved at build time so the binary can be
/// started from any working directory.
fn sample_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/sample.md")
}
