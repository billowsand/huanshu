//! D. The shell-capability page: one minimal working example per widget the Studio
//! shell needs, plus file dialogs, a modal confirmation, a password dialog with a
//! strength bar, toasts, and a hand-rolled drag-to-reorder list.

use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants as _, Toggle};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::dialog::{AlertDialog, Dialog, DialogAction, DialogClose, DialogFooter};
use gpui_kit::component::input::{Input, InputEvent, InputState, NumberInput};
use gpui_kit::component::label::Label;
use gpui_kit::component::notification::{Notification, NotificationType};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, WindowExt as _};
use gpui_kit::prelude::*;
use gpui_kit::{ClickEvent, Context, Entity, Render, SharedString, Subscription, Window, div, px};

/// Payload of a row drag; anything `Clone + 'static` works.
#[derive(Clone)]
struct RowDrag(usize);

impl Render for RowDrag {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Only the drop position matters here, so the drag image is a thin bar.
        div().w(px(60.)).h(px(3.)).bg(gpui_kit::rgb(0x4c8dff))
    }
}

pub struct Controls {
    switch_on: bool,
    checkbox: bool,
    radio: Option<usize>,
    toggle: bool,
    slider: Entity<SliderState>,
    slider_value: f32,
    amount: Entity<InputState>,
    amount_text: String,
    password: Entity<InputState>,
    password_text: String,
    rows: Vec<String>,
    drag_count: usize,
    note: String,
    /// Dropping a subscription cancels it, so they are kept for the view's lifetime.
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,
}

impl Controls {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let slider = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .step(5.0)
                .default_value(35.0)
        });
        let amount = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value("5")
                .step(1.0)
                .min(0.0)
                .max(100.0)
        });
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("密码（至少 8 位，含大小写与数字）")
        });
        let mut subscriptions = Vec::new();
        // The slider and the input expose no `on_change` builder: both are entities
        // that emit events, so the view subscribes to them.
        subscriptions.push(cx.subscribe(
            &slider,
            |this: &mut Controls, _state: Entity<SliderState>, event: &SliderEvent, cx| {
                if let SliderEvent::Change(value) = event {
                    this.slider_value = value.end();
                    cx.notify();
                }
            },
        ));
        subscriptions.push(cx.subscribe_in(
            &amount,
            window,
            |this: &mut Controls,
             state: &Entity<InputState>,
             event: &InputEvent,
             _window,
             cx| {
                if matches!(event, InputEvent::Change) {
                    this.amount_text = state.read(cx).value().to_string();
                    cx.notify();
                }
            },
        ));
        subscriptions.push(cx.subscribe(
            &password,
            |this: &mut Controls, state: Entity<InputState>, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.password_text = state.read(cx).value().to_string();
                    // The dialog body re-reads this state when it rebuilds, so a
                    // notify is what keeps the strength bar in step with typing.
                    cx.notify();
                }
            },
        ));
        Self {
            switch_on: true,
            checkbox: false,
            radio: Some(0),
            toggle: false,
            slider,
            slider_value: 35.0,
            amount,
            amount_text: "5".to_string(),
            password,
            password_text: String::new(),
            rows: vec![
                "封面 · cover".to_string(),
                "总览 · overview".to_string(),
                "四卡 · feature_grid".to_string(),
                "流程 · step_flow".to_string(),
                "对比 · compare".to_string(),
                "收尾 · closing".to_string(),
            ],
            drag_count: 0,
            note: String::new(),
            subscriptions,
        }
    }

    fn move_row(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if from >= self.rows.len() || to >= self.rows.len() || from == to {
            return;
        }
        let row = self.rows.remove(from);
        self.rows.insert(to, row);
        self.drag_count += 1;
        self.note = format!("拖动第 {} 行到第 {} 行（第 {} 次）", from + 1, to + 1, self.drag_count);
        cx.notify();
    }

    fn toast(&mut self, window: &mut Window, cx: &mut Context<Self>, kind: NotificationType) {
        let notification = match kind {
            NotificationType::Info => Notification::info("已保存当前牌组"),
            NotificationType::Success => Notification::success("导出完成：deck.pdf"),
            NotificationType::Warning => Notification::warning("LM Studio 未连接"),
            NotificationType::Error => Notification::error("Typst 编译失败：第 7 页溢出"),
        };
        window.push_notification(notification, cx);
        self.note = format!("已推送 {kind:?} 轻提示");
        cx.notify();
    }

    fn confirm_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.open_alert_dialog(cx, move |alert: AlertDialog, _window, _cx| {
            alert
                .title("删除这一页？")
                .description("删除之后可以用撤销恢复；缩略图会重新渲染。")
                .confirm()
                .ok_text("删除")
                .ok_variant(ButtonVariant::Danger)
                .cancel_text("保留")
                .on_ok(|_event, _window, _cx| true)
        });
        self.note = "已打开确认对话框".to_string();
        cx.notify();
    }

    pub fn open_password_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let password = self.password.clone();
        window.open_dialog(cx, move |dialog: Dialog, _window, cx| {
            let text = password.read(cx).value().to_string();
            dialog
                .title("导出加密牌组")
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(Input::new(&password))
                        .child(Label::new(format!(
                            "强度 {:.0} / 100{}",
                            password_strength(&text),
                            if password_strength(&text) < 60.0 {
                                " · 太弱"
                            } else {
                                ""
                            }
                        )))
                        .child(Progress::new("password-strength").value(password_strength(&text))),
                )
                .footer(
                    DialogFooter::new()
                        .child(DialogClose::new().trigger(|button| button.label("取消")))
                        .child(DialogAction::new().child(Button::new("ok").label("开始导出"))),
                )
        });
        self.note = "已打开密码对话框".to_string();
        cx.notify();
    }

    fn pick_file(&mut self, window: &mut Window, cx: &mut Context<Self>, save: bool) {
        cx.spawn_in(window, async move |this, window| {
            let dialog = rfd::AsyncFileDialog::new().add_filter("markdown", &["md"]);
            let chosen = if save {
                dialog.set_file_name("deck-notes.md").save_file().await
            } else {
                dialog.pick_file().await
            };
            let outcome = chosen
                .map(|file| file.path().display().to_string())
                .unwrap_or_else(|| "（取消）".to_string());
            let _ = this.update_in(window, |controls, _window, cx| {
                controls.note = if save {
                    format!("保存对话框：{outcome}")
                } else {
                    format!("打开对话框：{outcome}")
                };
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for Controls {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let slider_value = self.slider_value;
        let rows = self.rows.clone();
        let note = self.note.clone();
        let password_hint = format!("{:.0} / 100", password_strength(&self.password_text));

        let mut gallery = div().flex().flex_col().gap_4().p_4().w_full();

        gallery = gallery.child(section(
            "按钮 Button",
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(
                    Button::new("primary")
                        .primary()
                        .label("主要操作")
                        .tooltip("Button::new(id).primary()")
                        .on_click(cx.listener(|controls: &mut Controls, _: &ClickEvent, _, cx| {
                            controls.note = "主要按钮".to_string();
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("ghost")
                        .ghost()
                        .label("次要操作")
                        .on_click(cx.listener(|controls: &mut Controls, _: &ClickEvent, _, cx| {
                            controls.note = "次要按钮".to_string();
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("danger")
                        .label("删除")
                        .on_click(cx.listener(|controls: &mut Controls, _: &ClickEvent, window, cx| {
                            controls.confirm_delete(window, cx)
                        })),
                )
                .child(Button::new("disabled").label("不可用").disabled(true))
                .child(
                    Toggle::new("bold-toggle")
                        .label("Toggle")
                        .checked(self.toggle)
                        .on_click(cx.listener(|controls: &mut Controls, checked: &bool, _, cx| {
                            controls.toggle = *checked;
                            cx.notify();
                        })),
                ),
        ));

        gallery = gallery.child(section(
            "开关 / 勾选 / 单选",
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    Switch::new("demo-switch")
                        .label(if self.switch_on { "开关：开" } else { "开关：关" })
                        .checked(self.switch_on)
                        .on_change(cx.listener(|controls: &mut Controls, checked: &bool, _, cx| {
                            controls.switch_on = *checked;
                            cx.notify();
                        })),
                )
                .child(
                    Checkbox::new("demo-checkbox")
                        .label("复选框")
                        .checked(self.checkbox)
                        .on_change(cx.listener(|controls: &mut Controls, checked: &bool, _, cx| {
                            controls.checkbox = *checked;
                            cx.notify();
                        })),
                )
                .child(
                    RadioGroup::vertical("demo-radio")
                        .selected_index(self.radio)
                        .child(Radio::new("radio-16x9").label("16:9"))
                        .child(Radio::new("radio-32x9").label("32:9"))
                        .child(Radio::new("radio-48x9").label("48:9"))
                        .on_click(cx.listener(|controls: &mut Controls, index: &usize, _, cx| {
                            controls.radio = Some(*index);
                            cx.notify();
                        })),
                ),
        ));

        gallery = gallery.child(section(
            "滑块 / 数字输入",
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(Label::new(format!("Slider · {slider_value:.0}")))
                .child(Slider::new(&self.slider).horizontal().w(px(260.)))
                .child(Label::new(format!("NumberInput · {}", self.amount_text)))
                .child(NumberInput::new(&self.amount).w(px(140.))),
        ));

        gallery = gallery.child(section(
            "对话框 / 轻提示",
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(Button::new("alert").label("确认删除…").on_click(
                    cx.listener(|controls: &mut Controls, _: &ClickEvent, window, cx| {
                        controls.confirm_delete(window, cx)
                    }),
                ))
                .child(
                    Button::new("password-dialog")
                        .label("密码（带强度）…")
                        .on_click(cx.listener(
                            |controls: &mut Controls, _: &ClickEvent, window, cx| {
                                controls.open_password_dialog(window, cx)
                            },
                        )),
                )
                .child(Button::new("toast-info").label("Info").on_click(cx.listener(
                    |controls: &mut Controls, _: &ClickEvent, window, cx| {
                        controls.toast(window, cx, NotificationType::Info)
                    },
                )))
                .child(Button::new("toast-success").label("Success").on_click(
                    cx.listener(|controls: &mut Controls, _: &ClickEvent, window, cx| {
                        controls.toast(window, cx, NotificationType::Success)
                    }),
                ))
                .child(Button::new("toast-warning").label("Warning").on_click(
                    cx.listener(|controls: &mut Controls, _: &ClickEvent, window, cx| {
                        controls.toast(window, cx, NotificationType::Warning)
                    }),
                ))
                .child(Button::new("toast-error").label("Error").on_click(
                    cx.listener(|controls: &mut Controls, _: &ClickEvent, window, cx| {
                        controls.toast(window, cx, NotificationType::Error)
                    }),
                ))
                .child(Label::new(password_hint)),
        ));

        gallery = gallery.child(section(
            "文件对话框 / 浮层",
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    Button::new("rfd-open")
                        .label("打开文件…")
                        .on_click(cx.listener(|controls: &mut Controls, _: &ClickEvent, window, cx| {
                            controls.pick_file(window, cx, false)
                        })),
                )
                .child(
                    Button::new("rfd-save")
                        .label("保存文件…")
                        .on_click(cx.listener(|controls: &mut Controls, _: &ClickEvent, window, cx| {
                            controls.pick_file(window, cx, true)
                        })),
                )
                .child(
                    Popover::new("demo-popover")
                        .trigger(Button::new("popover-trigger").ghost().label("Popover"))
                        .content(|_state, _window, _cx| {
                            div().p_2().child("浮层内容：可以放筛选条件或图例。")
                        }),
                ),
        ));

        gallery = gallery.child(section(
            "拖拽排序（自行实现）",
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(rows.iter().enumerate().map(|(index, row)| {
                    let row = row.clone();
                    div()
                        .id(("drag-row", index))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py_1()
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(div().text_xs().child(SharedString::from(format!("{index}"))))
                        .child(SharedString::from(row))
                        .on_drag(RowDrag(index), |drag, _offset, _window, cx| {
                            cx.new(|_| drag.clone())
                        })
                        .drag_over::<RowDrag>(|style, _drag, _window, cx| {
                            style.bg(cx.theme().accent)
                        })
                        .on_drop(cx.listener(move |controls: &mut Controls, drag: &RowDrag, _, cx| {
                            controls.move_row(drag.0, index, cx)
                        }))
                })),
        ));

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                div()
                    .id("controls-gallery")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(gallery),
            )
            .child(
                div()
                    .px_3()
                    .py_1()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .child(SharedString::from(note)),
            )
    }
}

fn section(title: &str, body: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_sm().child(SharedString::from(title.to_string())))
        .child(body)
}

/// Same rule as the Tauri app's password dialog: length plus character classes.
fn password_strength(text: &str) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let mut score = (text.chars().count().min(16) as f32) * 3.0;
    for present in [
        text.chars().any(|c| c.is_ascii_lowercase()),
        text.chars().any(|c| c.is_ascii_uppercase()),
        text.chars().any(|c| c.is_ascii_digit()),
        text.chars().any(|c| !c.is_alphanumeric()),
    ] {
        if present {
            score += 13.0;
        }
    }
    score.min(100.0)
}
