//! 页面共用的小部件：对应原版的 `.btn`、`.badge`、`.card`、`TitleBar.vue`、`ThemeToggle.vue`、
//! `BrandMark.vue` 等。**新页面先找这里**，没有再加，不要在页面里各写一份样式。
//!
//! 颜色一律取自 `cx.studio().colors`（见 `theme`），尺寸按原版 CSS 的逻辑像素抄。
//! 按钮直接用 GPUI Kit 的 `Button`（它的颜色已经由 `theme::apply` 映射成 studio token）。

use std::path::PathBuf;

use gpui_kit::base::InteractiveElementExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, IconName, Sizable as _};
use gpui_kit::prelude::*;
use gpui_kit::{
    div, img, linear_color_stop, linear_gradient, point, px, AnyElement, App, Background,
    BoxShadow, ClickEvent, Div, ElementId, Hsla, MouseButton, SharedString, Stateful, Window,
    WindowControlArea,
};
use huanshu_core::themes::{self, Tone};

use crate::icons::carbon;
use crate::theme::{tokens, PaperLook, StudioExt as _};

/// Shared geometry for terminal panes, badges, menus and inspectors.
pub fn radius(cx: &App, ordinary: f32) -> gpui_kit::Pixels {
    px(if cx.studio().is_terminal() {
        0.0
    } else {
        ordinary
    })
}

/// A real status strip: callers supply the current state instead of decorative metrics.
pub fn terminal_status(
    left: impl Into<SharedString>,
    right: impl Into<SharedString>,
    cx: &App,
) -> Div {
    let c = cx.studio().colors;
    div()
        .flex_none()
        .h(px(28.0))
        .px(px(12.0))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .bg(c.panel)
        .border_t_1()
        .border_color(c.border)
        .font_family(code_font(cx))
        .text_size(px(11.0))
        .text_color(c.muted)
        .child(div().min_w_0().truncate().child(left.into()))
        .child(div().flex_none().child(right.into()))
}

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

/// macOS 标题栏左侧让给系统红绿灯按钮的宽度：红绿灯放在 (12, 12)（见 `main.rs` 的
/// `traffic_light_position`），三个按钮占到 x≈64，留 80px 内容不压它们。
const TRAFFIC_LIGHTS_WIDTH: f32 = 80.0;

/// 标题栏的拖窗状态（macOS / Linux 手动拖窗，见 [`title_bar`]）：按下置位，随后的移动
/// 交给系统拖窗；抬起或按下落在栏外即复位。
struct TitleBarState {
    should_move: bool,
}

/// 自绘标题栏（原版 `TitleBar.vue`）：品牌图标 + 「幻述」，中间是当前文稿的标题（`center`），
/// 右侧最小化 / 最大化 / 关闭（**仅 Windows / Linux**；macOS 用系统红绿灯，不画自绘按钮）。
///
/// 不用 GPUI Kit 的 `TitleBar`：它的窗口按钮固定取 `Theme.foreground` 着色，便签主题的深棕
/// 标题栏上会看不见。颜色自己定，窗口行为按平台分（拖窗、双击与 GPUI Kit `TitleBar` 同一套
/// 做法）：
///
/// - **Windows**：整条栏声明 `window_control_area`（拖动、贴靠、双击最大化、按钮点击都
///   交给系统的命中测试）。`center` 里要接收点击的元素必须 `.occlude()`：整条标题栏是
///   拖动区（`HTCAPTION`），不挡住它，点击会被系统当成拖窗口。
/// - **macOS**：窗口选项带 `app_owns_titlebar_drag`（`TitleBar::window_options()`），
///   AppKit 不接管标题栏拖动，由这里 `TitleBarState` + `start_window_move` 手动拖；
///   双击走 `titlebar_double_click`（跟随系统「连按标题栏」设置）。红绿灯由系统画在
///   左上角，左侧留 [`TRAFFIC_LIGHTS_WIDTH`] 不遮它。
pub fn title_bar(
    center: Option<AnyElement>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let colors = cx.studio().colors;
    let paper = cx.studio().paper;
    let is_macos = cfg!(target_os = "macos");
    let maximized = window.is_maximized();
    let fullscreen = window.is_fullscreen();
    let state = window.use_state(cx, |_, _| TitleBarState { should_move: false });
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
        None if cx.studio().is_terminal() => (
            colors.panel.into(),
            colors.border,
            colors.text,
            colors.surface,
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
        // 双击标题栏：macOS 跟随系统设置（放大 / 最小化），Linux 直接切换最大化；
        // Windows 不用处理——整条栏是 HTCAPTION，系统自己响应双击。
        .when(is_macos, |bar| {
            bar.on_double_click(|_, window, _| window.titlebar_double_click())
        })
        .when(cfg!(target_os = "linux"), |bar| {
            bar.on_double_click(|_, window, _| window.zoom_window())
        })
        // 手动拖窗（Windows 上这条链不会触发：拖动区是非客户区，收不到客户区鼠标事件）。
        // 子元素先收到按下事件，不阻断冒泡则视为「按住标题栏」——正中标题等可点元素
        // 要自己 `stop_propagation`（见 `WorkflowPage::title_element` 的改名框）。
        .on_mouse_down_out(window.listener_for(&state, |state, _, _, _| {
            state.should_move = false;
        }))
        .on_mouse_down(
            MouseButton::Left,
            window.listener_for(&state, |state, _, _, _| {
                state.should_move = true;
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            window.listener_for(&state, |state, _, _, _| {
                state.should_move = false;
            }),
        )
        .on_mouse_move(window.listener_for(&state, |state, _, window, _| {
            if state.should_move {
                state.should_move = false;
                window.start_window_move();
            }
        }))
        .child(
            div()
                .id("title-bar-drag")
                .relative()
                .flex_1()
                .min_w_0()
                .h_full()
                // macOS：内容从系统红绿灯右边开始；全屏时红绿灯自动隐藏，恢复普通边距。
                .pl(px(if is_macos && !fullscreen {
                    TRAFFIC_LIGHTS_WIDTH
                } else {
                    12.0
                }))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .window_control_area(WindowControlArea::Drag)
                .child(if cx.studio().is_terminal() {
                    div()
                        .size(px(22.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .font_family(code_font(cx))
                        .text_color(colors.primary)
                        .child(">_")
                } else {
                    brand_chip(22.0, 7.0, 16.0)
                })
                .child(div().text_size(px(12.3)).text_color(text).child("幻述"))
                .when_some(center, |bar, center| {
                    // 标题落在整扇窗口的正中：Windows 右边让出三个窗口按钮的宽度；
                    // macOS 左右对称让出红绿灯的宽度（右边没有自绘按钮）。
                    bar.child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .when(is_macos, |d| d.px(px(TRAFFIC_LIGHTS_WIDTH)))
                            .when(!is_macos, |d| d.pl(px(WINDOW_BUTTON_W * 3.0)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(text)
                            .child(center),
                    )
                }),
        )
        // 自绘窗口按钮不进 macOS：红绿灯是系统画的，再画一套既重复又抢点击。
        .when(!is_macos, |bar| {
            bar.child(button(
                "window-min",
                IconName::WindowMinimize,
                WindowControlArea::Min,
                false,
            ))
            .child(button(
                "window-max",
                if maximized {
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
        })
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
        .rounded(px(
            if cx.studio().theme.id == "light-editorial" || cx.studio().is_terminal() {
                0.0
            } else {
                999.0
            },
        ))
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
        .rounded(px(
            if cx.studio().theme.id == "light-editorial" || cx.studio().is_terminal() {
                0.0
            } else {
                999.0
            },
        ))
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
        .rounded(px(
            if cx.studio().theme.id == "light-editorial" || cx.studio().is_terminal() {
                0.0
            } else {
                10.0
            },
        ))
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
            .rounded(radius(cx, 3.0))
            .shadow(paper_shadow(&p)),
        None => panel(cx),
    }
}

/// Home sheets have a pale upper face, a warmer foot and a fine raised rim.
pub fn note_sheet(cx: &App) -> Div {
    paper_card(cx).when_some(cx.studio().paper, |sheet, p| {
        sheet
            .bg(linear_gradient(
                180.0,
                linear_color_stop(p.card.blend(gpui_kit::white().opacity(0.48)), 0.0),
                linear_color_stop(p.bar_top, 1.0),
            ))
            .rounded(radius(cx, 2.0))
            .shadow(vec![
                BoxShadow {
                    color: p.ink.opacity(0.13),
                    offset: point(px(0.0), px(1.0)),
                    blur_radius: px(2.0),
                    spread_radius: px(0.0),
                    inset: false,
                },
                BoxShadow {
                    color: p.ink.opacity(0.16),
                    offset: point(px(0.0), px(3.0)),
                    blur_radius: px(5.0),
                    spread_radius: px(-1.0),
                    inset: false,
                },
                BoxShadow {
                    color: gpui_kit::white().opacity(0.7),
                    offset: point(px(0.0), px(1.0)),
                    blur_radius: px(0.0),
                    spread_radius: px(0.0),
                    inset: true,
                },
            ])
    })
}

pub fn chrome_button(button: Button, cx: &App) -> Button {
    button.when_some(cx.studio().paper, |button, p| {
        button.custom(
            gpui_kit::component::button::ButtonCustomVariant::new(cx)
                .foreground(p.chrome_text)
                .hover(p.chrome_hover)
                .active(p.chrome_hover),
        )
    })
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
            offset: point(px(0.0), px(5.0)),
            blur_radius: px(10.0),
            spread_radius: px(-3.0),
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

/// Recessed scroll viewport: paper slips beneath a soft lip at either edge.
/// Read the handle during paint, after the scroller has settled its offset.
/// A canvas has no hitbox, so the shading never intercepts clicks or wheels.
pub fn scroll_edges(content: impl IntoElement, scroll: &gpui_kit::ScrollHandle, cx: &App) -> Div {
    let scroll = scroll.clone();
    div()
        .relative()
        .flex_1()
        .min_h_0()
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(content)
        .when_some(cx.studio().paper, |viewport, paper| {
            viewport.child(
                gpui_kit::canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _cx| {
                        let offset = -scroll.offset().y.as_f32();
                        let remaining = scroll.max_offset().y.as_f32() - offset;
                        let depth = bounds.size.height.min(px(16.0));
                        for (top, distance) in [(true, offset), (false, remaining)] {
                            if distance <= 0.5 {
                                continue;
                            }
                            let strength = (distance / 12.0).clamp(0.0, 1.0);
                            let shade = paper.ink.opacity(0.13 * strength);
                            let clear = paper.ink.opacity(0.0);
                            let (start, end) = if top { (shade, clear) } else { (clear, shade) };
                            window.paint_quad(gpui_kit::fill(
                                gpui_kit::Bounds::new(
                                    point(
                                        bounds.left(),
                                        if top {
                                            bounds.top()
                                        } else {
                                            bounds.bottom() - depth
                                        },
                                    ),
                                    gpui_kit::size(bounds.size.width, depth),
                                ),
                                linear_gradient(
                                    180.0,
                                    linear_color_stop(start, 0.0),
                                    linear_color_stop(end, 1.0),
                                ),
                            ));
                        }
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
        })
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
        .rounded(radius(cx, 5.0))
        .child(
            div()
                .w(px(52.0))
                .h(px(36.0))
                .rounded(radius(cx, 4.0))
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
pub fn code_font(cx: &App) -> SharedString {
    gpui_kit::component::Theme::global(cx)
        .mono_font_family
        .clone()
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
