//! Studio 外壳的配色与字体。
//!
//! 原版（Tauri + Vue）的外壳颜色全部来自 `--studio-*` CSS 变量；这里把同一套 token
//! （[`tokens`]，从运行中的旧应用读出的计算值）放进一个 GPUI 全局量 [`Studio`]，
//! 页面一律经 `cx.studio()` 取色。**不要在页面里写颜色字面量**——六个主题都要能用。
//!
//! 同时把 token 写进 GPUI Kit 的 `Theme`，这样它自带的组件（按钮、输入框、对话框、
//! 滚动条、轻提示）也是同一套颜色，不会出现「组件一套、页面一套」。

pub mod tokens;

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{px, rgba, App, Global, Hsla, SharedString, Window};
use huanshu_core::themes::{self, AppTheme};

/// 当前主题的外壳配色。字段与 CSS 变量一一对应（`--studio-panel-2` → `panel_2`）。
#[derive(Debug, Clone, Copy)]
pub struct StudioColors {
    pub bg: Hsla,
    pub panel: Hsla,
    pub panel_2: Hsla,
    pub surface: Hsla,
    pub border: Hsla,
    pub border_hover: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub primary: Hsla,
    pub primary_light: Hsla,
    pub primary_contrast: Hsla,
    pub primary_bg: Hsla,
    pub primary_border: Hsla,
    pub success: Hsla,
    pub success_bg: Hsla,
    pub error: Hsla,
    pub error_bg: Hsla,
    pub error_border: Hsla,
    pub warning: Hsla,
    pub warning_bg: Hsla,
    pub warning_border: Hsla,
}

impl StudioColors {
    fn from_table(t: tokens::TokenTable) -> Self {
        let c = |value: u32| -> Hsla { rgba(value).into() };
        Self {
            bg: c(t.bg),
            panel: c(t.panel),
            panel_2: c(t.panel_2),
            surface: c(t.surface),
            border: c(t.border),
            border_hover: c(t.border_hover),
            text: c(t.text),
            muted: c(t.muted),
            primary: c(t.primary),
            primary_light: c(t.primary_light),
            primary_contrast: c(t.primary_contrast),
            primary_bg: c(t.primary_bg),
            primary_border: c(t.primary_border),
            success: c(t.success),
            success_bg: c(t.success_bg),
            error: c(t.error),
            error_bg: c(t.error_bg),
            error_border: c(t.error_border),
            warning: c(t.warning),
            warning_bg: c(t.warning_bg),
            warning_border: c(t.warning_border),
        }
    }
}

/// 当前主题：id、名称、明暗、配色与界面字体。全局唯一，经 [`apply`] 切换。
#[derive(Debug, Clone)]
pub struct Studio {
    pub theme: AppTheme,
    pub colors: StudioColors,
    /// 「设置 → 外观与字体」的「界面」字体（原版 `--font-interface`）。
    pub interface_font: SharedString,
}

impl Global for Studio {}

/// `cx.studio()`：页面取色的唯一入口。
pub trait StudioExt {
    fn studio(&self) -> &Studio;
}

impl StudioExt for App {
    fn studio(&self) -> &Studio {
        self.global::<Studio>()
    }
}

/// 原版 `--titlebar-height`。
pub const TITLEBAR_HEIGHT: f32 = 36.0;
/// 原版界面字体的默认值（`--font-interface`）。
pub const DEFAULT_INTERFACE_FONT: &str = "Microsoft YaHei";

/// 切换主题：写 [`Studio`] 全局量，并把 token 映射进 GPUI Kit 的 `Theme`。
///
/// 只改显示；**保存**设置由调用方决定（设置页是「点击预览、保存后生效」）。
pub fn apply(
    theme_id: &str,
    interface_font: Option<&str>,
    window: Option<&mut Window>,
    cx: &mut App,
) {
    let (theme, _) = themes::theme_or_default(Some(theme_id));
    let theme = *theme;
    let colors = StudioColors::from_table(tokens::table(theme.id));
    let font: SharedString = interface_font
        .filter(|font| !font.trim().is_empty())
        .unwrap_or(DEFAULT_INTERFACE_FONT)
        .to_string()
        .into();

    Theme::change(
        if theme.tone.is_dark() {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        window,
        cx,
    );
    let kit = Theme::global_mut(cx);
    kit.font_family = font.clone();
    kit.radius = px(8.0);
    let k = &mut kit.colors;
    k.background = colors.bg;
    k.foreground = colors.text;
    k.border = colors.border;
    k.input = colors.border;
    k.ring = colors.primary_border;
    k.caret = colors.primary;
    k.selection = colors.primary_bg;
    k.muted = colors.surface;
    k.muted_foreground = colors.muted;
    k.accent = colors.surface;
    k.accent_foreground = colors.text;
    k.primary = colors.primary;
    k.primary_hover = colors.primary_light;
    k.primary_active = colors.primary_light;
    k.primary_foreground = colors.primary_contrast;
    // GPUI Kit 的按钮读自己的一组 `button_*`，不读上面的 `primary*`。
    k.button_primary = colors.primary;
    k.button_primary_hover = colors.primary_light;
    k.button_primary_active = colors.primary_light;
    k.button_primary_foreground = colors.primary_contrast;
    k.button = colors.surface;
    k.button_hover = colors.panel_2;
    k.button_active = colors.panel_2;
    k.button_foreground = colors.text;
    k.button_danger = colors.error;
    k.button_danger_hover = colors.error;
    k.button_danger_active = colors.error;
    k.button_danger_foreground = colors.primary_contrast;
    k.secondary = colors.surface;
    k.secondary_hover = colors.panel_2;
    k.secondary_active = colors.panel_2;
    k.secondary_foreground = colors.text;
    k.danger = colors.error;
    k.danger_foreground = colors.primary_contrast;
    k.success = colors.success;
    k.warning = colors.warning;
    k.popover = colors.panel;
    k.popover_foreground = colors.text;
    k.title_bar = colors.panel;
    k.title_bar_border = colors.border;
    k.sidebar = colors.panel;
    k.sidebar_border = colors.border;
    k.sidebar_foreground = colors.text;
    k.list = colors.panel;
    k.list_hover = colors.surface;
    k.list_active = colors.primary_bg;
    k.list_active_border = colors.primary_border;
    k.scrollbar = colors.bg;
    k.scrollbar_thumb = colors.border_hover;
    k.progress_bar = colors.primary;
    k.tab_bar = colors.panel;
    k.tab_active = colors.surface;
    k.tab_active_foreground = colors.text;
    k.window_border = colors.border;
    k.overlay = gpui_kit::hsla(0.0, 0.0, 0.0, 0.45);
    // 组件实际读的是由颜色派生的 `tokens`（按钮、输入框的语义色），改完颜色要重新派生一次，
    // 否则主按钮还是 GPUI Kit 默认的白底。
    kit.tokens = gpui_kit::component::ThemeTokens::from(&kit.colors);

    cx.set_global(Studio {
        theme,
        colors,
        interface_font: font,
    });
}
