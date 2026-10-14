//! 密码对话框（原版 `PasswordDialog.vue`）：encrypt（设置密码 + 强度条 + 确认密码）与
//! decrypt（输入密码）两种模式。供首页加密导出 / 加密导入复用。
//!
//! 交互照原版：眼睛切换显示、确认密码不一致红边 + 错误文案、encrypt 提交前的强度门槛
//! （`/(?=.*[a-z])(?=.*[A-Z])(?=.*\d).{8,}/`）、`externalError` 由调用方更新（密码错误
//! 重试时不关对话框）。提交/取消的后续动作由调用方回调决定（对话框自己不关）。

use std::rc::Rc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Disableable as _, WindowExt as _};
use gpui_kit::prelude::*;
use gpui_kit::{div, px, App, Context, Entity, Hsla, Render, Subscription, Window};

use crate::icons::carbon;
use crate::theme::StudioExt as _;
use crate::widgets;

/// 对话框模式（原版 `mode: 'encrypt' | 'decrypt'`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordMode {
    Encrypt,
    Decrypt,
}

/// 原版强度条的等级文案与颜色（CSS 字面量，不随主题变）。
const LEVELS: [(&str, u32); 5] = [
    ("太弱", 0xef4444),
    ("弱", 0xf97316),
    ("一般", 0xeab308),
    ("良好", 0x22c55e),
    ("强", 0x10b981),
];

/// 原版 `strength` 的评分：len==0 → 0；≥8 +1；≥12 +1；含小写+大写 +1；含数字 +1；
/// 含非字母数字 +1。长度用字符数（BMP 字符与原版 JS `length` 一致）。
pub fn strength_score(password: &str) -> u8 {
    let mut score = 0u8;
    let len = password.chars().count();
    if len >= 8 {
        score += 1;
    }
    if len >= 12 {
        score += 1;
    }
    if password.chars().any(|c| c.is_ascii_lowercase())
        && password.chars().any(|c| c.is_ascii_uppercase())
    {
        score += 1;
    }
    if password.chars().any(|c| c.is_ascii_digit()) {
        score += 1;
    }
    if password.chars().any(|c| !c.is_ascii_alphanumeric()) {
        score += 1;
    }
    score
}

/// 等级标签与颜色：score clamp 到 0–4 取标签（原版 `levels[Math.min(score, 4)]`）。
pub fn strength_level(score: u8) -> (&'static str, Hsla) {
    let (label, color) = LEVELS[usize::from(score).min(4)];
    (label, gpui_kit::rgba(color).into())
}

/// 原版 encrypt 提交前的强度门槛：`/(?=.*[a-z])(?=.*[A-Z])(?=.*\d).{8,}/`。
/// 与 core `crypto::validate_password_strength` 同一组规则（两边用 ASCII 判定；
/// core 用 byte len，原版用 JS string length，ASCII 场景一致）。
pub fn meets_encrypt_requirement(password: &str) -> bool {
    password.len() >= 8
        && password.chars().any(|c| c.is_ascii_lowercase())
        && password.chars().any(|c| c.is_ascii_uppercase())
        && password.chars().any(|c| c.is_ascii_digit())
}

type SubmitCallback = Rc<dyn Fn(String, &mut Window, &mut App)>;
type CancelCallback = Rc<dyn Fn(&mut Window, &mut App)>;

/// 密码对话框实体（内容渲染在 `window.open_dialog` 里）。
pub struct PasswordDialog {
    mode: PasswordMode,
    project_name: Option<String>,
    /// 调用方传入的错误（原版 `externalError`，密码错误重试时由调用方更新）。
    external_error: String,
    /// 对话框自己的校验错误（原版 `internalError`）。
    internal_error: String,
    password: Entity<InputState>,
    confirm: Entity<InputState>,
    show_password: bool,
    on_submit: SubmitCallback,
    on_cancel: CancelCallback,
    _subscriptions: Vec<Subscription>,
}

impl PasswordDialog {
    pub fn new(
        mode: PasswordMode,
        project_name: Option<String>,
        on_submit: impl Fn(String, &mut Window, &mut App) + 'static,
        on_cancel: impl Fn(&mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(if mode == PasswordMode::Encrypt {
                    "设置密码"
                } else {
                    "输入密码"
                })
                .masked(true)
        });
        let confirm = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("再次输入密码")
                .masked(true)
        });
        let _subscriptions = vec![
            cx.subscribe(&password, |this: &mut Self, _state, _: &InputEvent, cx| {
                // 密码变化后清掉门槛错误（原版 internalError 只由提交设置，但重新输入
                // 后旧错误继续显示会误导；原版 watch 只在 visible 时重置）。
                this.internal_error.clear();
                cx.notify();
            }),
            cx.subscribe(&confirm, |_this, _state, _: &InputEvent, cx| {
                cx.notify();
            }),
        ];
        Self {
            mode,
            project_name,
            external_error: String::new(),
            internal_error: String::new(),
            password,
            confirm,
            show_password: false,
            on_submit: Rc::new(on_submit),
            on_cancel: Rc::new(on_cancel),
            _subscriptions,
        }
    }

    /// 调用方更新外部错误（原版 `externalError` prop；密码错误重试场景）。
    pub fn set_external_error(&mut self, error: String, cx: &mut Context<Self>) {
        self.external_error = error;
        cx.notify();
    }

    fn password_value(&self, cx: &App) -> String {
        self.password.read(cx).value().to_string()
    }

    fn confirm_value(&self, cx: &App) -> String {
        self.confirm.read(cx).value().to_string()
    }

    /// 原版 `canSubmit`。
    fn can_submit(&self, cx: &App) -> bool {
        let password = self.password_value(cx);
        match self.mode {
            PasswordMode::Decrypt => !password.is_empty(),
            PasswordMode::Encrypt => {
                password.chars().count() >= 8 && password == self.confirm_value(cx)
            }
        }
    }

    /// 原版 `handleSubmit()`：encrypt 先过强度门槛，再交回调用方。
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_submit(cx) {
            return;
        }
        let password = self.password_value(cx);
        if self.mode == PasswordMode::Encrypt && !meets_encrypt_requirement(&password) {
            self.internal_error = "密码必须至少8字符，包含大小写字母和数字".to_string();
            cx.notify();
            return;
        }
        (self.on_submit)(password, window, cx);
    }

    /// 打开密码对话框（宽 400，照原版 `.password-modal`）。
    pub fn open(
        window: &mut Window,
        cx: &mut App,
        build: impl Fn(&mut Window, &mut Context<Self>) -> Self + 'static,
    ) -> Entity<Self> {
        let dialog = cx.new(|cx| build(window, cx));
        let entity = dialog.clone();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog.overlay(true).w(px(400.0)).child(entity.clone())
        });
        dialog
    }
}

impl Render for PasswordDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        let password = self.password_value(cx);
        let confirm = self.confirm_value(cx);
        let mismatch = !confirm.is_empty() && password != confirm;
        let error = if self.internal_error.is_empty() {
            self.external_error.clone()
        } else {
            self.internal_error.clone()
        };
        let score = strength_score(&password);
        let (level_label, level_color) = strength_level(score);
        let submitting = match self.mode {
            PasswordMode::Encrypt => "加密导出",
            PasswordMode::Decrypt => "解密并导入",
        };

        // 输入框外壳：自绘边框，确认密码不一致时红边（原版 `.form-input.error`）。
        let shell = |error: bool, input: gpui_kit::AnyElement| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .pl(px(2.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(if error { c.error } else { c.border })
                .bg(c.bg)
                .child(input)
                .into_any_element()
        };
        let eye = widgets::icon_button(
            "toggle-password",
            if self.show_password {
                "view-off"
            } else {
                "view"
            },
            if self.show_password {
                "隐藏密码"
            } else {
                "显示密码"
            },
        )
        .on_click(cx.listener(|this, _event, window, cx| {
            this.show_password = !this.show_password;
            this.password.update(cx, |state, cx| {
                state.set_masked(!this.show_password, window, cx)
            });
            this.confirm.update(cx, |state, cx| {
                state.set_masked(!this.show_password, window, cx)
            });
            cx.notify();
        }));

        let password_group = div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(div().text_size(px(13.0)).child("密码"))
            .child(shell(
                false,
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(Input::new(&self.password).appearance(false).flex_1())
                    .child(eye)
                    .into_any_element(),
            ))
            .when(
                self.mode == PasswordMode::Encrypt && !password.is_empty(),
                |group| {
                    group.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .mt(px(2.0))
                            .child(
                                div()
                                    .flex_1()
                                    .h(px(4.0))
                                    .rounded(px(2.0))
                                    .bg(c.border)
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .h_full()
                                            .w(px(280.0 * f32::from(score) / 5.0))
                                            .bg(level_color),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                                    .text_color(level_color)
                                    .child(level_label),
                            ),
                    )
                },
            );

        let confirm_group = (self.mode == PasswordMode::Encrypt).then(|| {
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(div().text_size(px(13.0)).child("确认密码"))
                .child(shell(
                    mismatch,
                    div()
                        .child(Input::new(&self.confirm).appearance(false))
                        .into_any_element(),
                ))
                .when(mismatch, |group| {
                    group.child(
                        div()
                            .text_size(px(12.0))
                            .text_color(c.error)
                            .child("两次输入的密码不一致"),
                    )
                })
        });

        let desc: Option<String> = match self.mode {
            PasswordMode::Encrypt => self
                .project_name
                .clone()
                .map(|name| format!("为「{name}」设置导出密码")),
            PasswordMode::Decrypt => Some("请输入密码以解密文件".to_string()),
        };

        let close_cancel = self.on_cancel.clone();
        let cancel_cancel = self.on_cancel.clone();
        div()
            .w(px(352.0))
            .flex()
            .flex_col()
            .items_center()
            .bg(c.panel)
            .p(px(24.0))
            .child(div().flex().flex_row().justify_end().w_full().child(
                widgets::icon_button("close-password", "close", "关闭").on_click(cx.listener(
                    move |_this, _event, window, cx| {
                        window.close_dialog(cx);
                        close_cancel(window, cx);
                    },
                )),
            ))
            .child(
                div()
                    .size(px(48.0))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .mb(px(16.0))
                    .bg(if self.mode == PasswordMode::Encrypt {
                        gpui_kit::rgba(0x22c55e26)
                    } else {
                        gpui_kit::rgba(0x3b82f626)
                    })
                    .text_color(if self.mode == PasswordMode::Encrypt {
                        gpui_kit::rgb(0x22c55e)
                    } else {
                        gpui_kit::rgb(0x3b82f6)
                    })
                    .child(carbon("locked")),
            )
            .child(
                div()
                    .text_size(px(18.0))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .mb(px(8.0))
                    .child(match self.mode {
                        PasswordMode::Encrypt => "设置密码",
                        PasswordMode::Decrypt => "输入密码",
                    }),
            )
            .when_some(desc, |dialog, desc| {
                dialog.child(
                    div()
                        .text_size(px(14.0))
                        .text_color(c.muted)
                        .mb(px(20.0))
                        .child(desc),
                )
            })
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(16.0))
                    .child(password_group)
                    .when_some(confirm_group, |form, group| form.child(group)),
            )
            .when(!error.is_empty(), |dialog| {
                dialog.child(
                    div()
                        .w_full()
                        .mt(px(4.0))
                        .p(px(8.0))
                        .pl(px(12.0))
                        .rounded(px(6.0))
                        .bg(gpui_kit::rgba(0xef44441a))
                        .text_size(px(13.0))
                        .text_color(c.error)
                        .child(error),
                )
            })
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .gap(px(12.0))
                    .justify_end()
                    .mt(px(20.0))
                    .child(
                        Button::new("cancel-password")
                            .ghost()
                            .label("取消")
                            .on_click(cx.listener(move |_this, _event, window, cx| {
                                window.close_dialog(cx);
                                cancel_cancel(window, cx);
                            })),
                    )
                    .child(
                        Button::new("submit-password")
                            .primary()
                            .label(submitting)
                            .disabled(!self.can_submit(cx))
                            .on_click(
                                cx.listener(move |this, _event, window, cx| {
                                    this.submit(window, cx)
                                }),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use huanshu_core::crypto::validate_password_strength;

    #[test]
    fn strength_scores_follow_the_original_rules() {
        assert_eq!(strength_score(""), 0);
        assert_eq!(strength_score("short"), 0);
        assert_eq!(strength_score("abcdefgh"), 1);
        assert_eq!(strength_score("abcdefghijkl"), 2);
        assert_eq!(strength_score("abcdEFGH"), 2);
        assert_eq!(strength_score("abcd1234"), 2);
        assert_eq!(strength_score("abcd1234!"), 3);
        assert_eq!(strength_score("abcdEFGH12"), 3);
        assert_eq!(strength_score("abcdEFGH12!"), 4);
        assert_eq!(strength_score("abcdEFGH12!long"), 5);
    }

    #[test]
    fn strength_levels_clamp_to_four() {
        assert_eq!(strength_level(0).0, "太弱");
        assert_eq!(strength_level(1).0, "弱");
        assert_eq!(strength_level(2).0, "一般");
        assert_eq!(strength_level(3).0, "良好");
        assert_eq!(strength_level(4).0, "强");
        assert_eq!(strength_level(5).0, "强");
    }

    #[test]
    fn encrypt_gate_matches_the_original_regex() {
        assert!(meets_encrypt_requirement("Passw0rd"));
        assert!(meets_encrypt_requirement("TestPassword123"));
        assert!(!meets_encrypt_requirement("password"));
        assert!(!meets_encrypt_requirement("PASSWORD123"));
        assert!(!meets_encrypt_requirement("Password"));
        assert!(!meets_encrypt_requirement("Pass1"));
    }

    #[test]
    fn encrypt_gate_agrees_with_core_password_validation() {
        // 同一组 ASCII 密码：界面门槛（原版正则）与 core validate_password_strength 同过同不过。
        // core 用 byte len，原版用 JS string length，ASCII 场景两者一致。
        for password in [
            "Passw0rd",
            "TestPassword123",
            "abc123ABC!@#",
            "password",
            "PASSWORD123",
            "Password",
            "Pass1",
            "12345678",
            "p@ssw0rd",
            "ABCdef12",
        ] {
            assert_eq!(
                meets_encrypt_requirement(password),
                validate_password_strength(password).is_ok(),
                "门槛不一致：{password}"
            );
        }
    }
}
