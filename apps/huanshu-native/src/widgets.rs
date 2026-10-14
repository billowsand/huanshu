//! 页面共用的小部件：对应原版的 `.btn`、`.badge`、`.card`、`TitleBar.vue`、`ThemeToggle.vue`、
//! `BrandMark.vue` 等。**新页面先找这里**，没有再加，不要在页面里各写一份样式。
//!
//! 颜色一律取自 `cx.studio().colors`（见 `theme`），尺寸按原版 CSS 的逻辑像素抄。
//! 按钮直接用 GPUI Kit 的 `Button`（它的颜色已经由 `theme::apply` 映射成 studio token）。

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Sizable as _, TitleBar};
use gpui_kit::prelude::*;
use gpui_kit::{
    div, img, px, App, ClickEvent, Div, ElementId, Hsla, SharedString, Stateful, Window,
};
use huanshu_core::themes::{self, Tone};

use crate::icons::carbon;
use crate::theme::{tokens, StudioExt as _};

/// 原版 `BrandMark.vue`（同一份 SVG，见 `assets/brand-mark.svg`）。
pub fn brand_mark(size: f32) -> impl IntoElement {
    img("brand/mark.svg").size(px(size)).flex_none()
}

/// 品牌图标的深色底座（原版 `.titlebar-brand-chip` / 向导的 logo 块）：固定的深色底，不随主题变。
pub fn brand_chip(size: f32, radius: f32, mark: f32) -> Div {
    div()
        .flex_none()
        .size(px(size))
        .rounded(px(radius))
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui_kit::rgb(0x111a28))
        .border_1()
        .border_color(gpui_kit::rgba(0x78aadc29))
        .child(brand_mark(mark))
}

/// 自绘标题栏（原版 `TitleBar.vue`）：品牌图标 + 「幻述」，最小化/最大化/关闭由 GPUI Kit
/// 的 `TitleBar` 画（Windows 上靠系统的命中测试实现拖动、贴靠、双击最大化）。
pub fn title_bar(cx: &App) -> impl IntoElement {
    let colors = cx.studio().colors;
    TitleBar::new().child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(brand_chip(22.0, 7.0, 16.0))
            .child(
                div()
                    .text_size(px(12.3))
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .text_color(colors.text)
                    .child("幻述"),
            ),
    )
}

/// 主按钮（原版 `.btn.btn-primary`）。
pub fn primary_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).primary().label(label)
}

/// 幽灵按钮（原版 `.btn.btn-ghost`）。
pub fn ghost_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).ghost().label(label)
}

/// 只有图标的幽灵按钮（原版 `.btn.btn-ghost.icon-btn`），`icon` 写 Carbon 名。
pub fn icon_button(
    id: impl Into<ElementId>,
    icon: &str,
    tooltip: impl Into<SharedString>,
) -> Button {
    Button::new(id).ghost().icon(carbon(icon)).tooltip(tooltip)
}

/// 徽章的色调（原版 `.badge` 与各处 `--studio-*-bg/border` 组合）。
#[allow(dead_code)] // 完整色板，Error 留给失败状态的徽章
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone2 {
    Primary,
    Success,
    Warning,
    Error,
    Muted,
}

/// 小徽章（原版 `.badge`：圆角胶囊、浅底、细边）。
pub fn chip(text: impl Into<SharedString>, tone: Tone2, cx: &App) -> Div {
    let c = cx.studio().colors;
    let (fg, bg, border) = match tone {
        Tone2::Primary => (c.primary, c.primary_bg, c.primary_border),
        Tone2::Success => (c.success, c.success_bg, c.success),
        Tone2::Warning => (c.warning, c.warning_bg, c.warning_border),
        Tone2::Error => (c.error, c.error_bg, c.error_border),
        Tone2::Muted => (c.muted, c.surface, c.border),
    };
    div()
        .flex_none()
        .px(px(8.0))
        .py(px(1.0))
        .rounded(px(999.0))
        .border_1()
        .border_color(border)
        .bg(bg)
        .text_size(px(11.5))
        .text_color(fg)
        .child(text.into())
}

/// 带图标的徽章（原版 `.filename-tag`：document 图标 + 文件名）。
pub fn chip_icon(icon: &'static str, text: impl Into<SharedString>, tone: Tone2, cx: &App) -> Div {
    let c = cx.studio().colors;
    let (fg, bg, border) = match tone {
        Tone2::Primary => (c.primary, c.primary_bg, c.primary_border),
        Tone2::Success => (c.success, c.success_bg, c.success),
        Tone2::Warning => (c.warning, c.warning_bg, c.warning_border),
        Tone2::Error => (c.error, c.error_bg, c.error_border),
        Tone2::Muted => (c.muted, c.surface, c.border),
    };
    div()
        .flex_none()
        .px(px(8.0))
        .py(px(1.0))
        .rounded(px(999.0))
        .border_1()
        .border_color(border)
        .bg(bg)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(5.0))
        .text_size(px(11.5))
        .text_color(fg)
        .child(carbon(icon))
        .child(text.into())
}

/// 面板（原版 `--studio-panel` 底 + `--studio-border` 细边 + 圆角）。
pub fn panel(cx: &App) -> Div {
    let c = cx.studio().colors;
    div()
        .bg(c.panel)
        .border_1()
        .border_color(c.border)
        .rounded(px(10.0))
}

/// 小节标题（原版各处的 `.section-label` / `.sdp-label`：小号、弱化色）。
pub fn section_label(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(12.0))
        .text_color(cx.studio().colors.muted)
        .child(text.into())
}

/// 竖线分隔（原版 `.bar-divider`）。
pub fn bar_divider(cx: &App) -> Div {
    div().w(px(1.0)).h(px(18.0)).bg(cx.studio().colors.border)
}

/// **骨架占位**：框架里还没实现的区域。标题、对照的 Vue 源码位置、要实现的功能逐条列出，
/// 后续实现时整块替换掉。颜色用警示色虚线框，截图里一眼能看出哪里还没做。
pub fn todo_block(title: &str, vue_ref: &str, items: &[&str], cx: &App) -> Div {
    let c = cx.studio().colors;
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .p(px(12.0))
        .rounded(px(8.0))
        .border_1()
        .border_dashed()
        .border_color(c.warning_border)
        .bg(c.warning_bg)
        .child(
            div()
                .text_size(px(12.5))
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(c.warning)
                .child(format!("待实现 · {title}")),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(c.muted)
                .child(format!("对照 {vue_ref}")),
        )
        .children(items.iter().map(|item| {
            div()
                .text_size(px(11.5))
                .text_color(c.text)
                .child(format!("· {item}"))
        }))
}

/// 主题切换圆点（原版 `ThemeToggle.vue`：六个圆点，当前主题带一圈主色）。
///
/// `on_pick` 收到主题 id；保存与否由调用方决定。
pub fn theme_dots(
    id: impl Into<ElementId>,
    current: &str,
    on_pick: impl Fn(&'static str, &mut Window, &mut App) + Clone + 'static,
    cx: &App,
) -> Stateful<Div> {
    let ring = cx.studio().colors.primary;
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .children(themes::APP_THEMES.iter().map(|theme| {
            let table = tokens::table(theme.id);
            let swatch: Hsla = gpui_kit::rgba(table.panel).into();
            let accent: Hsla = gpui_kit::rgba(table.primary).into();
            let selected = theme.id == current;
            let on_pick = on_pick.clone();
            let theme_id = theme.id;
            div()
                .id(SharedString::from(format!("theme-dot-{}", theme.id)))
                .size(px(20.0))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(swatch)
                .border_2()
                .border_color(if selected {
                    ring
                } else if theme.tone == Tone::Dark {
                    gpui_kit::rgba(0xffffff26).into()
                } else {
                    gpui_kit::rgba(0x0000001f).into()
                })
                .cursor_pointer()
                .child(div().size(px(8.0)).rounded_full().bg(accent))
                .tooltip_text(theme.name)
                .on_click(move |_event: &ClickEvent, window, cx| on_pick(theme_id, window, cx))
        }))
}

/// 统一的小号按钮尺寸（原版工具栏按钮比默认小一号）。
pub fn small(button: Button) -> Button {
    button.small()
}

/// 行内错误小字（原版 `.prepare-inline-error`：红色、小号，跟在格式工具行右侧）。
pub fn inline_error(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(12.0))
        .text_color(cx.studio().colors.error)
        .child(text.into())
}

/// 给 `Stateful<Div>` 一个只显示文字的悬停提示（GPUI Kit `Tooltip`）。
pub trait TooltipText {
    fn tooltip_text(self, text: impl Into<SharedString>) -> Self;
}

impl TooltipText for Stateful<Div> {
    fn tooltip_text(self, text: impl Into<SharedString>) -> Self {
        let text = text.into();
        self.tooltip(move |window, cx| {
            gpui_kit::component::tooltip::Tooltip::new(text.clone()).build(window, cx)
        })
    }
}
