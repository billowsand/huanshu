//! 页面共用的小部件：对应原版的 `.btn`、`.badge`、`.card`、`TitleBar.vue`、`ThemeToggle.vue`、
//! `BrandMark.vue` 等。**新页面先找这里**，没有再加，不要在页面里各写一份样式。
//!
//! 颜色一律取自 `cx.studio().colors`（见 `theme`），尺寸按原版 CSS 的逻辑像素抄。
//! 按钮直接用 GPUI Kit 的 `Button`（它的颜色已经由 `theme::apply` 映射成 studio token）。

use std::path::PathBuf;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, IconName, Sizable as _};
use gpui_kit::prelude::*;
use gpui_kit::{
    div, img, linear_color_stop, linear_gradient, point, px, AnyElement, App, Background,
    BoxShadow, ClickEvent, Div, ElementId, Hsla, SharedString, Stateful, Window, WindowControlArea,
};
use huanshu_core::themes::{self, Tone};

use crate::icons::carbon;
use crate::theme::{tokens, PaperLook, StudioExt as _};

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

/// 窗口按钮的宽度（Windows 标准标题栏按钮约 46px）。居中标题按它左右对称留白。
const WINDOW_BUTTON_W: f32 = 46.0;

/// 自绘标题栏（原版 `TitleBar.vue`）：品牌图标 + 「幻述」，中间是当前文稿的标题（`center`），
/// 右侧最小化 / 最大化 / 关闭。
///
/// 不用 GPUI Kit 的 `TitleBar`：它的窗口按钮固定取 `Theme.foreground` 着色，便签主题的深棕
/// 标题栏上会看不见。这里一样只声明 `window_control_area`（拖动、贴靠、双击最大化、
/// 按钮点击都交给 Windows 的命中测试），颜色自己定。
///
/// `center` 里要接收点击的元素必须 `.occlude()`：整条标题栏是拖动区（`HTCAPTION`），
/// 不挡住它，点击会被系统当成拖窗口。
pub fn title_bar(center: Option<AnyElement>, window: &Window, cx: &App) -> impl IntoElement {
    let colors = cx.studio().colors;
    let paper = cx.studio().paper;
    // 便签主题：锤子便签的深棕导航条；其余主题沿用 GPUI Kit 标题栏的面板色渐变。
    let (background, border, text, hover): (Background, Hsla, Hsla, Hsla) = match paper {
        Some(p) => (
            linear_gradient(
                180.0,
                linear_color_stop(p.chrome_top, 0.0),
                linear_color_stop(p.chrome_bottom, 1.0),
            ),
            p.chrome_border,
            p.chrome_text,
            p.chrome_hover,
        ),
        None => (
            linear_gradient(
                180.0,
                linear_color_stop(colors.panel.blend(colors.bg.opacity(0.45)), 0.0),
                linear_color_stop(colors.panel, 1.0),
            ),
            colors.border,
            colors.text,
            colors.text.opacity(0.08),
        ),
    };
    let button = |id: &'static str, icon: IconName, area: WindowControlArea, close: bool| {
        div()
            .id(id)
            .flex_none()
            .w(px(WINDOW_BUTTON_W))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(text)
            .hover(move |style| {
                if close {
                    style.bg(colors.error).text_color(gpui_kit::white())
                } else {
                    style.bg(hover)
                }
            })
            .window_control_area(area)
            .child(Icon::new(icon).small())
    };
    div()
        .id("title-bar")
        .flex_none()
        .h(px(crate::theme::TITLEBAR_HEIGHT))
        .flex()
        .flex_row()
        .bg(background)
        .border_b_1()
        .border_color(border)
        .child(
            div()
                .id("title-bar-drag")
                .relative()
                .flex_1()
                .min_w_0()
                .h_full()
                .pl(px(12.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .window_control_area(WindowControlArea::Drag)
                .child(brand_chip(22.0, 7.0, 16.0))
                .child(div().text_size(px(12.3)).text_color(text).child("幻述"))
                .when_some(center, |bar, center| {
                    // 右边让出三个窗口按钮的宽度，标题落在整扇窗口的正中。
                    bar.child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .pl(px(WINDOW_BUTTON_W * 3.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(text)
                            .child(center),
                    )
                }),
        )
        .child(button(
            "window-min",
            IconName::WindowMinimize,
            WindowControlArea::Min,
            false,
        ))
        .child(button(
            "window-max",
            if window.is_maximized() {
                IconName::WindowRestore
            } else {
                IconName::WindowMaximize
            },
            WindowControlArea::Max,
            false,
        ))
        .child(button(
            "window-close",
            IconName::WindowClose,
            WindowControlArea::Close,
            true,
        ))
}

/// 标题栏正中的文稿标题：超过 [`TITLE_MAX_CHARS`] 个字就截断加省略号（再窄也会按像素省略）。
pub const TITLE_MAX_CHARS: usize = 28;

pub fn clip_title(name: &str) -> SharedString {
    let name = name.trim();
    if name.chars().count() > TITLE_MAX_CHARS {
        let head: String = name.chars().take(TITLE_MAX_CHARS).collect();
        format!("{head}…").into()
    } else {
        SharedString::from(name.to_string())
    }
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

/// 便签主题的木纹桌面（原版 `--notes-grain, --notes-backing`）：铺满父元素（父元素要 `relative`），
/// 放在最底层，上面的内容自己透明。
///
/// 纹理按原尺寸 512×1024 平铺（原版 CSS 背景就是平铺），不拉伸：铺满整扇窗口时拉伸会把纤维放粗两三倍。
/// 固定铺 8 × 3 块（4096 × 3072 逻辑像素），多出的被裁掉；同一张 SVG 只光栅化一次。
pub fn wood_grain(paper: &PaperLook) -> Div {
    const TILE_W: f32 = 512.0;
    const TILE_H: f32 = 1024.0;
    const COLS: usize = 8;
    const ROWS: usize = 3;
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .overflow_hidden()
        .bg(paper.backing)
        .child(
            div()
                .w(px(TILE_W * COLS as f32))
                .flex()
                .flex_row()
                .flex_wrap()
                .children((0..COLS * ROWS).map(|_| {
                    img("notes/wood.svg")
                        .flex_none()
                        .w(px(TILE_W))
                        .h(px(TILE_H))
                })),
        )
}

/// 便签卡片（锤子便签列表里的一张）：卡片纸、细纸边、小圆角、纸张阴影。非便签主题原样返回
/// [`panel`]。
pub fn paper_card(cx: &App) -> Div {
    match cx.studio().paper {
        Some(p) => panel(cx)
            .bg(p.card)
            .border_color(p.edge)
            .rounded(px(3.0))
            .shadow(paper_shadow(&p)),
        None => panel(cx),
    }
}

/// 纸张阴影（原版 `--notes-paper-shadow: 0 1px 2px …, 0 4px 8px -5px …`）。
pub fn paper_shadow(paper: &PaperLook) -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: paper.shadow_near,
            offset: point(px(0.0), px(1.0)),
            blur_radius: px(2.0),
            spread_radius: px(0.0),
            inset: false,
        },
        BoxShadow {
            color: paper.shadow_far,
            offset: point(px(0.0), px(4.0)),
            blur_radius: px(8.0),
            spread_radius: px(-5.0),
            inset: false,
        },
    ]
}

/// 稿纸上的表头条（原版便签主题 `.prepare-tools` / `.prepare-topbar` 的纸色渐变）。
pub fn paper_bar(paper: &PaperLook) -> Background {
    linear_gradient(
        180.0,
        linear_color_stop(paper.bar_top, 0.0),
        linear_color_stop(paper.bar_bottom, 1.0),
    )
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

/// 素材小卡（原版 `.ej-media-strip` 里的 `.ej-media-item`：缩略图 + 名字 + 操作按钮，
/// 卡宽以放下完整的「设为主图」按钮为准）。
///
/// 缩略图画法同第一步 `media_thumb`（`img` 铺满一个 52×36 的圆角框）。操作按钮由调用方给
/// （第三步是「设为主图」/「加入图集」），所以它们带着自己的点击逻辑一起传进来。
pub fn media_tile(
    id: impl Into<ElementId>,
    url: &str,
    name: &str,
    actions: Vec<AnyElement>,
    cx: &App,
) -> Stateful<Div> {
    let c = cx.studio().colors;
    div()
        .id(id)
        .w(px(64.0))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(2.0))
        .p(px(3.0))
        .rounded(px(5.0))
        .child(
            div()
                .w(px(52.0))
                .h(px(36.0))
                .rounded(px(4.0))
                .border_1()
                .border_color(c.border)
                .bg(c.panel)
                .overflow_hidden()
                .child(img(PathBuf::from(url)).size_full()),
        )
        .child(
            div()
                .max_w(px(58.0))
                .truncate()
                .text_size(px(8.8))
                .text_color(c.muted)
                .child(SharedString::from(name.to_string())),
        )
        .children(actions)
        .tooltip_text(SharedString::from(name.to_string()))
}

/// 「代码」角色的等宽字体（设置 → 外观与字体；第二步的调试块用的是同一套）。
pub fn code_font(handle: &crate::shell::ShellHandle) -> SharedString {
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
