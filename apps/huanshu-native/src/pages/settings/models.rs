//! 「模型服务」分区（原版 `SettingsView.vue` 344–450 行）：三类服务卡片 + 表单 +
//! 右侧「连接状态」卡 + 「这个服务用于」。

use gpui_kit::prelude::*;
use gpui_kit::{div, px, Context, SharedString};

use super::service_form::{service_location, ModelTarget, ServiceForm, ServiceState};
use super::SettingsPage;
use crate::icons::carbon;
use crate::theme::StudioExt as _;
use crate::widgets::{self, Tone2};
use gpui_kit::component::Disableable as _;

/// 状态徽章（原版 `.prefs-status`，带圆点）。
pub(super) fn status_chip(state: ServiceState, cx: &gpui_kit::App) -> impl IntoElement {
    let c = cx.studio().colors;
    let (fg, bg, border) = match state.tone() {
        Tone2::Primary => (c.primary, c.primary_bg, c.primary_border),
        Tone2::Success => (c.success, c.success_bg, c.success),
        Tone2::Warning => (c.warning, c.warning_bg, c.warning_border),
        Tone2::Error => (c.error, c.error_bg, c.error_border),
        Tone2::Muted => (c.muted, c.surface, c.border),
    };
    // 原版 `.prefs-status.is-testing i { background: var(--prefs-accent) }`：只有圆点换主色。
    let dot = if state == ServiceState::Testing {
        c.primary
    } else {
        fg
    };
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(5.0))
        .px(px(8.0))
        .py(px(2.0))
        .rounded(px(999.0))
        .border_1()
        .border_color(border)
        .bg(bg)
        .text_size(px(11.5))
        .text_color(fg)
        .child(div().size(px(6.0)).rounded_full().bg(dot))
        .child(state.label())
}

impl SettingsPage {
    pub(super) fn form(&self, target: ModelTarget) -> &gpui_kit::Entity<ServiceForm> {
        &self.forms[target.index()]
    }

    /// 「模型服务」分区。
    pub(super) fn models(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;

        let tiles = div()
            .flex()
            .flex_row()
            .gap(px(12.0))
            .children(ModelTarget::ALL.iter().map(|target| {
                let target = *target;
                let info = target.info();
                let selected = target == self.active_service;
                let state = self.form(target).read(cx).state();
                div()
                    .id(SharedString::from(format!("service-tile-{}", target.key())))
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    .px(px(16.0))
                    .py(px(14.0))
                    .rounded(px(10.0))
                    .cursor_pointer()
                    .border_1()
                    .when(selected, |tile| {
                        tile.border_color(c.primary).bg(c.primary_bg)
                    })
                    .when(!selected, |tile| {
                        tile.border_color(c.border)
                            .bg(c.panel)
                            .hover(|style| style.border_color(c.primary_border))
                    })
                    .child(
                        div()
                            .size(px(34.0))
                            .rounded_full()
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(c.surface)
                            .text_color(c.primary)
                            .child(carbon(info.icon)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(div().text_size(px(14.0)).child(info.title))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(c.muted)
                                    .child(info.subtitle),
                            ),
                    )
                    .child(status_chip(state, cx))
                    .on_click(cx.listener(move |page, _event, _window, cx| {
                        page.active_service = target;
                        cx.notify();
                    }))
            }));

        let active = self.active_service;
        let info = active.info();
        let form = self.form(active).clone();
        let draft = form.read(cx).draft.clone();
        let state = form.read(cx).state();
        let active_result = form.read(cx).active_result().cloned();
        let testing = form.read(cx).testing;
        let can_test = !testing && !draft.base_url.trim().is_empty();
        let model_pill = draft
            .model
            .split('/')
            .next_back()
            .filter(|name| !name.is_empty())
            .map(|name| name.to_string());

        let summary_row = |label: &'static str, value: String| {
            div()
                .flex()
                .flex_row()
                .justify_between()
                .text_size(px(13.0))
                .child(div().text_color(c.muted).child(label))
                .child(div().child(value))
        };

        let connection = widgets::panel(cx)
            .w(px(300.0))
            .flex_none()
            .p(px(18.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(div().text_size(px(16.0)).child("连接状态"))
                    .child(status_chip(state, cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(summary_row(
                        "服务类型",
                        service_location(&draft.base_url).to_string(),
                    ))
                    // 原版 `.prefs-summary > div + div { border-top: 1px dashed }`（css 208）。
                    .child(
                        summary_row(
                            "模型",
                            if draft.model.trim().is_empty() {
                                "未选择".to_string()
                            } else {
                                draft.model.clone()
                            },
                        )
                        .border_t_1()
                        .border_dashed()
                        .border_color(c.border),
                    )
                    .child(
                        summary_row(
                            "响应耗时",
                            match &active_result {
                                Some(result) if result.ok => format!("{} ms", result.elapsed),
                                _ => "—".to_string(),
                            },
                        )
                        .border_t_1()
                        .border_dashed()
                        .border_color(c.border),
                    ),
            )
            .child(
                widgets::ghost_button(
                    "test-connection",
                    if testing {
                        "正在测试…"
                    } else {
                        "测试连接"
                    },
                )
                .icon(carbon("connection-signal"))
                .disabled(!can_test)
                .on_click(cx.listener(move |page, _event, window, cx| {
                    let form = page.form(active).clone();
                    let _ = form.update(cx, |form, cx| form.test_connection(window, cx));
                })),
            )
            .when_some(active_result.filter(|result| !result.ok), |card, result| {
                card.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(c.error)
                        .child(result.error),
                )
            })
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(c.muted)
                    .child("通过获取模型列表检查连接，模型能力以服务实际支持为准。"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(div().text_size(px(14.0)).child("这个服务用于"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .children(info.uses.iter().map(|usage| {
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.0))
                                    .text_size(px(13.0))
                                    .child(div().size(px(4.0)).rounded_full().bg(c.muted))
                                    .child(*usage)
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(c.muted)
                            .child("各类服务独立配置，互不覆盖。"),
                    ),
            );

        let form_panel = widgets::panel(cx)
            .flex_1()
            .min_w_0()
            .p(px(18.0))
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .child(div().text_size(px(16.0)).child(info.title))
                    .when_some(model_pill, |heading, pill| {
                        heading.child(widgets::chip(pill, Tone2::Muted, cx))
                    }),
            )
            .child(form);

        div().flex().flex_col().gap(px(16.0)).child(tiles).child(
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(16.0))
                .child(form_panel)
                .child(connection),
        )
    }
}
