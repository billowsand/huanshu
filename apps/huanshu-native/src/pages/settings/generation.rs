//! 「生成设置」分区（原版 `SettingsView.vue` 453–475 行，文案逐字）：
//! 自动修复轮数（0–10）、生成并发数（1–8）与负载提示。

use gpui_kit::component::input::Input;
use gpui_kit::prelude::*;
use gpui_kit::{div, px, Context};

use super::SettingsPage;
use crate::icons::carbon;
use crate::theme::StudioExt as _;
use crate::widgets;

/// 保存前的校验（原版 `save()` 里的规则）：repair 0–10 整数、concurrency 1–8 整数。
/// `invalid` 表示输入框里有解析不了的文本。
pub fn generation_valid(repair_rounds: usize, concurrency: usize, invalid: bool) -> bool {
    !invalid && repair_rounds <= 10 && (1..=8).contains(&concurrency)
}

impl SettingsPage {
    pub(super) fn generation(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;

        let setting_row = |label: &'static str,
                           desc: &'static str,
                           small: &'static str,
                           input: gpui_kit::AnyElement| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap(px(24.0))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .child(label),
                        )
                        .child(div().text_size(px(13.0)).text_color(c.muted).child(desc))
                        .child(div().text_size(px(12.0)).text_color(c.muted).child(small)),
                )
                .child(input)
                .into_any_element()
        };
        let number_input = |input: gpui_kit::AnyElement| {
            div()
                .w(px(96.0))
                .flex_none()
                .child(input)
                .into_any_element()
        };

        widgets::panel(cx)
            .p(px(22.0))
            .flex()
            .flex_col()
            .gap(px(20.0))
            .child(
                div()
                    .text_size(px(16.0))
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .child("生成参数"),
            )
            .child(setting_row(
                "自动修复轮数",
                "生成后遇到格式校验错误时，自动修复的最大次数。",
                "建议 2–4 轮",
                number_input(
                    div()
                        .child(Input::new(&self.repair_input))
                        .into_any_element(),
                ),
            ))
            .child(setting_row(
                "生成并发数",
                "同时生成的幻灯片数量，按模型服务的处理能力调整。",
                "本地模型建议 1，最多 8",
                number_input(
                    div()
                        .child(Input::new(&self.concurrency_input))
                        .into_any_element(),
                ),
            ))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .text_size(px(12.0))
                    .text_color(c.muted)
                    .child(carbon("information"))
                    .child(
                        "并发越高，对模型服务的负载越大。遇到超时或资源不足时，可以降低并发数。",
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_ranges_match_the_original_rules() {
        assert!(generation_valid(0, 1, false));
        assert!(generation_valid(10, 8, false));
        assert!(generation_valid(4, 4, false));
        assert!(!generation_valid(11, 4, false));
        assert!(!generation_valid(4, 0, false));
        assert!(!generation_valid(4, 9, false));
        // 输入框里有解析不了的文本时不允许保存。
        assert!(!generation_valid(4, 4, true));
    }
}
