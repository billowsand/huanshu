//! 「外观与字体」的字体区（原版 `SettingsView.vue` 286–340 行 + `SystemFontPicker.vue`
//! + `typography.ts`）：七个角色各一个可搜索的系统字体选择（弹层对话框）、
//! 「统一所有字体」开关、「恢复默认」，右侧「演示预览 / 界面预览」实时预览。

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Disableable as _, Sizable as _, WindowExt as _};
use gpui_kit::prelude::*;
use gpui_kit::{
    div, px, Context, Entity, KeyDownEvent, Render, SharedString, Subscription, Window,
};
use huanshu_core::fonts::SystemFont;
use huanshu_core::types::FontSettings;

use super::{PreviewTab, SettingsPage};
use crate::icons::carbon;
use crate::tasks::run_bg;
use crate::theme::StudioExt as _;
use crate::widgets::{self};

/// 原版 `DEFAULT_FONT`。
pub const DEFAULT_FONT: &str = "Microsoft YaHei";

/// 七个字体角色（原版 `FontRole`，顺序照 `fontRoles`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontRole {
    Interface,
    Title,
    Subtitle,
    Body,
    Caption,
    Number,
    Code,
}

impl FontRole {
    pub const ALL: [FontRole; 7] = [
        FontRole::Interface,
        FontRole::Title,
        FontRole::Subtitle,
        FontRole::Body,
        FontRole::Caption,
        FontRole::Number,
        FontRole::Code,
    ];

    /// 存储键（`FontSettings` 字段名）。
    pub fn key(self) -> &'static str {
        match self {
            FontRole::Interface => "interface",
            FontRole::Title => "title",
            FontRole::Subtitle => "subtitle",
            FontRole::Body => "body",
            FontRole::Caption => "caption",
            FontRole::Number => "number",
            FontRole::Code => "code",
        }
    }

    /// 原版 `roleLabels`（逐字）。
    pub fn label(self) -> &'static str {
        match self {
            FontRole::Interface => "界面",
            FontRole::Title => "标题",
            FontRole::Subtitle => "副标题",
            FontRole::Body => "正文",
            FontRole::Caption => "注释与标签",
            FontRole::Number => "数字与序号",
            FontRole::Code => "代码与编辑器",
        }
    }

    /// 原版 `roleDescriptions`（逐字）。
    pub fn desc(self) -> &'static str {
        match self {
            FontRole::Interface => "菜单、按钮与表单",
            FontRole::Title => "封面与章节标题",
            FontRole::Subtitle => "副标题与章节说明",
            FontRole::Body => "演示正文与内容卡片",
            FontRole::Caption => "脚注、标签与辅助信息",
            FontRole::Number => "编号、序号与时间轴",
            FontRole::Code => "Markdown 编辑器与代码块",
        }
    }

    /// 原版 `fontRoles[].label`（弹层标题「选择{label}」，逐字）。
    pub fn picker_label(self) -> &'static str {
        match self {
            FontRole::Interface => "界面字体",
            FontRole::Title => "标题字体",
            FontRole::Subtitle => "副标题字体",
            FontRole::Body => "正文字体",
            FontRole::Caption => "注释与标签字体",
            FontRole::Number => "数字与序号字体",
            FontRole::Code => "代码与编辑器字体",
        }
    }

    /// 原版 `fontRoles[].sample`（弹层效果预览文案，逐字）。
    pub fn sample(self) -> &'static str {
        match self {
            FontRole::Interface => "幻述 Studio · 保存设置",
            FontRole::Title => "让每一个想法清晰呈现",
            FontRole::Subtitle => "从灵感到表达 · Ideas into Stories",
            FontRole::Body => "将 Markdown 文档转化为精美演示文稿。",
            FontRole::Caption => "资料来源 · 重点提示 · 更新于今日",
            FontRole::Number => "01 02 03 · 2026 · 1234567890",
            FontRole::Code => "## Hello 幻述 · const value = 42",
        }
    }

    pub fn get<'a>(self, fonts: &'a FontSettings) -> &'a str {
        match self {
            FontRole::Interface => &fonts.interface,
            FontRole::Title => &fonts.title,
            FontRole::Subtitle => &fonts.subtitle,
            FontRole::Body => &fonts.body,
            FontRole::Caption => &fonts.caption,
            FontRole::Number => &fonts.number,
            FontRole::Code => &fonts.code,
        }
    }

    pub fn set(self, fonts: &mut FontSettings, value: impl Into<String>) {
        let value = value.into();
        match self {
            FontRole::Interface => fonts.interface = value,
            FontRole::Title => fonts.title = value,
            FontRole::Subtitle => fonts.subtitle = value,
            FontRole::Body => fonts.body = value,
            FontRole::Caption => fonts.caption = value,
            FontRole::Number => fonts.number = value,
            FontRole::Code => fonts.code = value,
        }
    }
}

/// 原版 `searchKey`：去首尾空白、小写、去所有空白。
fn search_key(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

/// 原版 `matchesFont`：name / label / aliases 任一匹配（子串）。
pub fn matches_font(font: &SystemFont, query: &str) -> bool {
    let key = search_key(query);
    std::iter::once(&font.name)
        .chain(std::iter::once(&font.label))
        .chain(font.aliases.iter())
        .any(|name| search_key(name).contains(&key))
}

/// 原版 `findSystemFont`：name / label / aliases 任一完全相等。
pub fn find_system_font<'a>(fonts: &'a [SystemFont], name: &str) -> Option<&'a SystemFont> {
    let key = search_key(name);
    if key.is_empty() {
        return None;
    }
    fonts.iter().find(|font| {
        std::iter::once(&font.name)
            .chain(std::iter::once(&font.label))
            .chain(font.aliases.iter())
            .any(|alias| search_key(alias) == key)
    })
}

/// 原版 `commonNames`：无搜索词时常用字体优先（code 角色一份单独的名单）。
pub fn common_names(role: FontRole) -> &'static [&'static str] {
    if role == FontRole::Code {
        &["Consolas", "Cascadia Code", "JetBrains Mono", DEFAULT_FONT]
    } else {
        &[
            DEFAULT_FONT,
            "Microsoft YaHei UI",
            "SimSun",
            "SimHei",
            "KaiTi",
            "DengXian",
            "MiSans",
            "Arial",
            "Georgia",
        ]
    }
}

/// 原版 `filtered`：搜索词过滤全部；无搜索词时常用优先、其余按原名序跟上。
/// 返回 `fonts` 里的下标。
pub fn filtered_font_indices(fonts: &[SystemFont], query: &str, role: FontRole) -> Vec<usize> {
    let matched: Vec<usize> = fonts
        .iter()
        .enumerate()
        .filter(|(_, font)| matches_font(font, query))
        .map(|(index, _)| index)
        .collect();
    if !query.trim().is_empty() {
        return matched;
    }
    let mut priority: Vec<String> = Vec::new();
    let mut ordered = Vec::new();
    for name in common_names(role) {
        if let Some(font) = find_system_font(fonts, name) {
            priority.push(font.name.to_lowercase());
        }
    }
    for index in &matched {
        let name = fonts[*index].name.to_lowercase();
        if priority.contains(&name) {
            continue;
        }
        ordered.push(*index);
    }
    // 常用字体按名单顺序排最前。
    let mut common = Vec::new();
    for name in common_names(role) {
        if let Some(index) = matched
            .iter()
            .copied()
            .find(|&index| search_key(&fonts[index].name) == search_key(name))
        {
            common.push(index);
        }
    }
    common.into_iter().chain(ordered).collect()
}

/// 原版 `normalizeFonts`：空白归一为 `DEFAULT_FONT`（core `FontSettings::normalize` 同款规则）。
pub fn normalize_fonts(fonts: &FontSettings) -> FontSettings {
    let mut normalized = fonts.clone();
    normalized.normalize();
    normalized
}

/// 原版 `applyFontToAll`：全部角色设为同一字体。
pub fn apply_font_to_all(fonts: &mut FontSettings, value: &str) {
    for role in FontRole::ALL {
        role.set(fonts, value);
    }
}

/// 原版 `setFont`：统一开启时铺满全部，否则只改当前角色。
pub fn set_font(fonts: &mut FontSettings, role: FontRole, value: &str, unified: bool) {
    if unified {
        apply_font_to_all(fonts, value);
    } else {
        role.set(fonts, value);
    }
}

/// 触发按钮上显示的字体名（原版 `currentLabel`）。
fn trigger_label(fonts: &[SystemFont], value: &str, loaded: bool) -> String {
    match find_system_font(fonts, value) {
        Some(font) => font.label.clone(),
        None if !loaded => value.to_string(),
        None if value == DEFAULT_FONT => "微软雅黑".to_string(),
        None => value.to_string(),
    }
}

impl SettingsPage {
    /// 「外观与字体」：界面主题卡片 + 字体区 + 预览卡。
    pub(super) fn appearance(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        let light = self.theme_group(huanshu_core::themes::Tone::Light, cx);
        let dark = self.theme_group(huanshu_core::themes::Tone::Dark, cx);

        let fonts = self.fonts_list.clone().unwrap_or_default();
        let loaded = self.fonts_list.is_some();
        let font_meta = if loaded {
            format!("已安装 {} 种字体", fonts.len())
        } else {
            "系统字体".to_string()
        };

        let font_rows = FontRole::ALL.iter().map(|role| {
            let role = *role;
            let value = role.get(&self.fonts).to_string();
            let highlighted = self.preview_role == role;
            let label = trigger_label(&fonts, &value, loaded);
            let missing = loaded && find_system_font(&fonts, &value).is_none();
            let trigger = div()
                .id(SharedString::from(format!("font-trigger-{}", role.key())))
                .w(px(250.0))
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.0))
                .px(px(12.0))
                .py(px(8.0))
                .rounded(px(8.0))
                .bg(c.panel)
                .border_1()
                .border_color(c.border)
                .cursor_pointer()
                .hover(|style| style.border_color(c.primary_border))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(13.0))
                        .truncate()
                        .child(label),
                )
                .when(value == DEFAULT_FONT, |trigger| {
                    trigger.child(div().text_size(px(11.0)).text_color(c.muted).child("默认"))
                })
                .child(carbon("chevron-down"))
                .on_click(cx.listener(move |page, _event, window, cx| {
                    page.open_font_picker(role, window, cx)
                }));
            div()
                .id(SharedString::from(format!("font-row-{}", role.key())))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(16.0))
                .px(px(12.0))
                .py(px(10.0))
                .rounded(px(8.0))
                .cursor_pointer()
                .when(highlighted, |row| row.bg(c.primary_bg))
                .when(!highlighted, |row| row.hover(|style| style.bg(c.surface)))
                .on_click(cx.listener(move |page, _event, _window, cx| {
                    // 原版点行即换 previewRole。
                    page.preview_role = role;
                    cx.notify();
                }))
                .child(
                    div()
                        .w(px(108.0))
                        .flex_none()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(div().text_size(px(13.0)).child(role.label()))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(c.muted)
                                .child(role.desc()),
                        ),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_end()
                        .gap(px(4.0))
                        .child(trigger)
                        .when(missing, |col| {
                            col.child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(c.primary)
                                    .child("此字体未安装，已回退到微软雅黑。"),
                            )
                        }),
                )
        });

        let unify = div()
            .id("unify-fonts-row")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .cursor_pointer()
            .on_click(cx.listener(|page, _event, window, cx| page.toggle_unified_fonts(window, cx)))
            .child(
                gpui_kit::component::switch::Switch::new("unify-fonts")
                    .checked(self.unified)
                    .on_change(cx.listener(|page, checked: &bool, window, cx| {
                        page.set_unified(*checked, window, cx);
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(1.0))
                    .child(div().text_size(px(13.0)).child("统一所有字体"))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(c.muted)
                            .child("开启后，修改任一用途会同步到全部用途"),
                    ),
            );

        let font_settings =
            widgets::panel(cx)
                .flex_1()
                .min_w_0()
                .p(px(18.0))
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(12.0))
                        // 原版 `.prefs-section-title { font-size: 15px }`（settings.css 66）。
                        .child(div().text_size(px(15.0)).child("字体"))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(c.muted)
                                .child(font_meta),
                        )
                        .child(div().flex_1())
                        .child(
                            widgets::ghost_button("reset-fonts", "恢复默认")
                                .icon(carbon("reset"))
                                .on_click(cx.listener(|page, _event, window, cx| {
                                    page.reset_fonts(window, cx)
                                })),
                        ),
                )
                .child(div().flex().flex_col().children(font_rows))
                .child(unify);

        let preview = self.font_preview_card(cx);

        div()
            .flex()
            .flex_col()
            .gap(px(24.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_baseline()
                            .gap(px(12.0))
                            .child(div().text_size(px(15.0)).child("界面主题"))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(c.muted)
                                    .child("点击即可预览，保存后生效"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(40.0))
                            .child(light)
                            .child(dark),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(24.0))
                    .child(font_settings)
                    .child(preview),
            )
    }

    /// 打开一个角色的字体选择弹层（原版 `SystemFontPicker` 的弹层对话框）。
    fn open_font_picker(&mut self, role: FontRole, window: &mut Window, cx: &mut Context<Self>) {
        self.preview_role = role;
        let page = cx.weak_entity();
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索中文或英文名称"));
        let picker = cx.new(|cx| {
            FontPicker::new(
                role,
                role.get(&self.fonts).to_string(),
                self.fonts_list.clone().unwrap_or_default(),
                self.fonts_list.is_some(),
                self.fonts_error.clone(),
                page,
                search.clone(),
                cx,
            )
        });
        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog.overlay(true).w(px(760.0)).child(picker.clone())
        });
        // 原版打开弹层后把焦点放进搜索框（`SystemFontPicker.vue` 49 行的 `searchInput.focus()`）。
        search.update(cx, |state, cx| state.focus(window, cx));
        cx.notify();
    }

    /// 右侧预览卡：「演示预览 / 界面预览」两个标签页 + specimen 行 + 代码 specimen。
    fn font_preview_card(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        let fonts = &self.fonts;
        let role_font = |role: FontRole| -> SharedString { role.get(fonts).to_string().into() };
        let tab_button = |_tab: PreviewTab, label: &'static str, selected: bool| {
            div()
                .id(SharedString::from(format!("preview-tab-{label}")))
                .px(px(12.0))
                .py(px(7.0))
                .rounded(px(8.0))
                .text_size(px(13.0))
                .cursor_pointer()
                .when(selected, |button| {
                    button.bg(c.primary_bg).text_color(c.primary)
                })
                .when(!selected, |button| {
                    button
                        .text_color(c.muted)
                        .hover(|style| style.bg(c.surface))
                })
                .child(label)
        };

        let slides = div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(c.muted)
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .child(div().font_family(role_font(FontRole::Number)).child("01"))
                    .child(div().child(" / IDEAS")),
            )
            .child(
                div()
                    .text_size(px(24.0))
                    .font_family(role_font(FontRole::Title))
                    .child("让想法清晰呈现"),
            )
            .child(
                div()
                    .text_size(px(15.0))
                    .font_family(role_font(FontRole::Subtitle))
                    .child("从灵感到表达"),
            )
            .child(div().h(px(1.0)).w_full().bg(c.border))
            .child(
                div()
                    .text_size(px(13.0))
                    .font_family(role_font(FontRole::Body))
                    .child("将 Markdown 文档转化为精美演示文稿。")
                    .child(div().child("清晰的层级，让内容更容易阅读。")),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(c.muted)
                    .font_family(role_font(FontRole::Caption))
                    .child("幻述 Studio · 2026"),
            );

        let interface = div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .font_family(role_font(FontRole::Interface))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(widgets::brand_mark(18.0))
                    .child(div().text_size(px(13.0)).child("幻述 Studio")),
            )
            .child(div().text_size(px(18.0)).child("新的演示文稿"))
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(c.muted)
                    .child("将灵感写下来，让内容自然成形。"),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(c.muted)
                    .child("项目名称"),
            )
            .child(
                div()
                    .px(px(10.0))
                    .py(px(7.0))
                    .rounded(px(6.0))
                    .bg(c.surface)
                    .border_1()
                    .border_color(c.border)
                    .text_size(px(13.0))
                    .child("我的第一份演示文稿"),
            )
            .child(
                div()
                    .px(px(12.0))
                    .py(px(7.0))
                    .rounded(px(6.0))
                    .bg(c.primary)
                    .text_color(c.primary_contrast)
                    .text_size(px(13.0))
                    .child("创建项目"),
            );

        let specimen_font = role_font(self.preview_role);
        let specimen = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(24.0))
            .px(px(4.0))
            .py(px(12.0))
            .border_t_1()
            .border_color(c.border)
            .font_family(specimen_font)
            // 原版 `.prefs-specimen`（css 144–147）：标签用主色、12px、界面字体；
            // 「Aa Bb」20px 常规字色；两个 span 13px 弱化色。
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(c.primary)
                    .child(self.preview_role.label()),
            )
            .child(div().text_size(px(20.0)).child("Aa Bb"))
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(c.muted)
                    .child("0123456789"),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(c.muted)
                    .child("中文字体示例"),
            );

        let code_specimen = div()
            .px(px(12.0))
            .py(px(10.0))
            .rounded(px(8.0))
            .bg(c.surface)
            .border_1()
            .border_color(c.border)
            .text_size(px(13.0))
            .font_family(role_font(FontRole::Code))
            .child("const ideas = 'Hello, 幻述';");

        let preview_body = match self.preview_tab {
            PreviewTab::Slides => slides.into_any_element(),
            PreviewTab::Interface => interface.into_any_element(),
        };

        widgets::panel(cx)
            .w(px(400.0))
            .flex_none()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .border_b_1()
                    .border_color(c.border)
                    .p(px(8.0))
                    .child(
                        tab_button(
                            PreviewTab::Slides,
                            "演示预览",
                            self.preview_tab == PreviewTab::Slides,
                        )
                        .on_click(cx.listener(
                            |page, _event, _window, cx| {
                                page.preview_tab = PreviewTab::Slides;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        tab_button(
                            PreviewTab::Interface,
                            "界面预览",
                            self.preview_tab == PreviewTab::Interface,
                        )
                        .on_click(cx.listener(
                            |page, _event, _window, cx| {
                                page.preview_tab = PreviewTab::Interface;
                                cx.notify();
                            },
                        )),
                    ),
            )
            .child(
                div()
                    .p(px(20.0))
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(preview_body)
                    .child(specimen)
                    .child(code_specimen),
            )
    }
}

/// 系统字体选择弹层（原版 `SystemFontPicker.vue` 的 `font-dialog`）。
pub struct FontPicker {
    role: FontRole,
    /// 打开时该角色的当前值（`modelValue`）。
    current: String,
    fonts: Vec<SystemFont>,
    loaded: bool,
    refreshing: bool,
    /// 读取失败 / 空列表的提示（原版 `systemFonts.error`，显示在搜索框下面）。
    error: Option<String>,
    search: Entity<InputState>,
    /// `filtered` 里的选中项下标（原版 `activeIndex`）。
    active: usize,
    /// 「所有用途使用此字体」（原版 `applyAll`）。
    apply_all: bool,
    page: gpui_kit::WeakEntity<SettingsPage>,
    _subscription: Subscription,
}

impl FontPicker {
    fn new(
        role: FontRole,
        current: String,
        fonts: Vec<SystemFont>,
        loaded: bool,
        error: Option<String>,
        page: gpui_kit::WeakEntity<SettingsPage>,
        search: Entity<InputState>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.subscribe(&search, |this: &mut Self, state, _: &InputEvent, cx| {
            let query = state.read(cx).value().to_string();
            let filtered = filtered_font_indices(&this.fonts, &query, this.role);
            // 原版 resetActive：搜索变化后选中当前字体，找不到回 0。
            this.active = filtered
                .iter()
                .copied()
                .position(|index| {
                    find_system_font(std::slice::from_ref(&this.fonts[index]), &this.current)
                        .is_some()
                })
                .unwrap_or(0);
            cx.notify();
        });
        let mut this = Self {
            role,
            current,
            fonts,
            loaded,
            refreshing: false,
            error,
            search,
            active: 0,
            apply_all: false,
            page,
            _subscription: subscription,
        };
        this.active = this
            .initial_filtered()
            .iter()
            .copied()
            .position(|index| {
                find_system_font(std::slice::from_ref(&this.fonts[index]), &this.current).is_some()
            })
            .unwrap_or(0);
        this
    }

    fn initial_filtered(&self) -> Vec<usize> {
        filtered_font_indices(&self.fonts, "", self.role)
    }

    /// 刷新系统字体（原版「刷新」按钮，重新枚举）。
    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        // 原版 `load(true)`：重试前先清掉上一次的错误。
        self.error = None;
        cx.notify();
        let page = self.page.clone();
        run_bg(
            window,
            cx,
            huanshu_core::fonts::enumerate_system_fonts,
            move |this: &mut Self, result, _window, cx| {
                this.refreshing = false;
                match result {
                    Ok(fonts) if !fonts.is_empty() => {
                        this.fonts = fonts;
                        this.loaded = true;
                        this.error = None;
                        this.active = 0;
                        // 同步回设置页（页头显示「已安装 N 种字体」）。
                        if let Some(page) = page.upgrade() {
                            let fonts = this.fonts.clone();
                            let _ = page.update(cx, |page, cx| {
                                page.fonts_list = Some(fonts);
                                page.fonts_error = None;
                                cx.notify();
                            });
                        }
                    }
                    // 原版 `stores/systemFonts.ts`：空列表也算失败。
                    Ok(_) => {
                        this.error = Some("没有读取到可用字体，请刷新重试。".to_string());
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            },
        );
    }

    /// 键盘：Enter 提交（原版 `onSearchKeydown` 里的 Enter → `select()`）。
    ///
    /// 原版还有 ↑/↓/Home/End 在候选里移动，但 GPUI 的单行输入框把它们当作光标移动动作
    /// 消费掉了（`gpui-base` 的 `MoveUp`/`MoveDown`/`MoveHome`/`MoveEnd` 不 `propagate()`），
    /// 弹层收不到这几个键；见 `docs/native/audit-settings.md`。
    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key.as_str() != "enter" {
            return;
        }
        let query = self.search.read(cx).value().to_string();
        let filtered = filtered_font_indices(&self.fonts, &query, self.role);
        let Some(&index) = filtered.get(self.active) else {
            return;
        };
        let font = self.fonts[index].clone();
        let role = self.role;
        let apply_all = self.apply_all;
        if let Some(page) = self.page.upgrade() {
            let _ = page.update(cx, |page, cx| {
                page.apply_font(role, &font.name, apply_all, window, cx)
            });
        }
        window.close_dialog(cx);
    }
}

impl Render for FontPicker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        let query = self.search.read(cx).value().to_string();
        let filtered = filtered_font_indices(&self.fonts, &query, self.role);
        let candidate = filtered
            .get(self.active)
            .map(|&index| self.fonts[index].clone());

        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px(px(24.0))
            .py(px(20.0))
            .border_b_1()
            .border_color(c.border)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(17.0))
                            .child(format!("选择{}", self.role.picker_label())),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(c.muted)
                            .child("先预览，再应用到设置"),
                    ),
            )
            .child(
                widgets::icon_button("close-picker", "close", "关闭字体选择").on_click(
                    cx.listener(|_this, _event, window, cx| {
                        window.close_dialog(cx);
                    }),
                ),
            );

        let meta_label = if query.trim().is_empty() {
            "常用优先 · 全部字体"
        } else {
            "搜索结果"
        };
        // 原版 `SystemFontPicker.vue` 120 行：只有「还在读」时才是「正在读取系统字体…」；
        // 读失败（error 有值）落到下面的空态，显示「未读取到字体，请刷新重试。」。
        let list_content: gpui_kit::AnyElement =
            if !self.loaded && self.error.is_none() && !self.refreshing {
                div()
                    .p(px(24.0))
                    .text_size(px(13.0))
                    .text_color(c.muted)
                    .child("正在读取系统字体…")
                    .into_any_element()
            } else if filtered.is_empty() {
                div()
                    .p(px(24.0))
                    .text_size(px(13.0))
                    .text_color(c.muted)
                    .child(if self.loaded {
                        "没有找到字体，试试其他名称。"
                    } else {
                        "未读取到字体，请刷新重试。"
                    })
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .children(filtered.iter().enumerate().map(|(row, &index)| {
                        let font = self.fonts[index].clone();
                        let active = row == self.active;
                        let is_current =
                            find_system_font(std::slice::from_ref(&font), &self.current).is_some();
                        div()
                            .id(SharedString::from(format!("font-option-{row}")))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .px(px(10.0))
                            .py(px(8.0))
                            .rounded(px(6.0))
                            .cursor_pointer()
                            .when(active, |option| {
                                option.bg(c.primary_bg).text_color(c.primary)
                            })
                            .when(!active, |option| option.hover(|style| style.bg(c.surface)))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(13.0))
                                    .truncate()
                                    .child(font.label.clone()),
                            )
                            .when(is_current, |option| {
                                option.child(
                                    div().text_size(px(11.0)).text_color(c.muted).child("当前"),
                                )
                            })
                            .child(
                                div()
                                    .w(px(14.0))
                                    .flex_none()
                                    .text_size(px(12.0))
                                    .text_color(c.primary)
                                    .child(if active { "✓" } else { "" }),
                            )
                            .on_click(cx.listener(move |this, _event, window, cx| {
                                this.active = row;
                                // 原版 `@click="activeIndex = index; searchInput?.focus()"`。
                                let search = this.search.clone();
                                search.update(cx, |state, cx| state.focus(window, cx));
                                cx.notify();
                            }))
                    }))
                    .into_any_element()
            };

        let catalog = div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .min_w_0()
            .p(px(16.0))
            .border_r_1()
            .border_color(c.border)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(10.0))
                    .rounded(px(8.0))
                    .bg(c.panel_2)
                    .border_1()
                    .border_color(c.border)
                    .child(carbon("search"))
                    .child(div().flex_1().child(Input::new(&self.search)))
                    .when(!query.is_empty(), |search| {
                        search.child(
                            widgets::icon_button("clear-search", "close", "清空搜索").on_click(
                                cx.listener(|this, _event, window, cx| {
                                    let search = this.search.clone();
                                    search.update(cx, |state, cx| state.set_value("", window, cx));
                                    cx.notify();
                                }),
                            ),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .px(px(4.0))
                    .text_size(px(11.0))
                    .text_color(c.muted)
                    .child(meta_label)
                    .child(filtered.len().to_string()),
            )
            // 原版 `SystemFontPicker.vue` 118 行的 `font-warning`（读失败时显示在列表上方）。
            .when_some(self.error.clone(), |catalog, error| {
                catalog.child(
                    div()
                        .px(px(4.0))
                        .text_size(px(12.0))
                        .text_color(c.primary)
                        .child(error),
                )
            })
            .child(
                div()
                    .id("font-picker-list")
                    .h(px(300.0))
                    .overflow_y_scroll()
                    .child(list_content),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(4.0))
                    .text_size(px(11.0))
                    .text_color(c.muted)
                    .child("系统字体")
                    .child(
                        Button::new("refresh-fonts")
                            .ghost()
                            .small()
                            .label(if self.refreshing {
                                "读取中…"
                            } else {
                                "刷新"
                            })
                            .icon(carbon("renew"))
                            .disabled(self.refreshing)
                            .on_click(
                                cx.listener(|this, _event, window, cx| this.refresh(window, cx)),
                            ),
                    ),
            );

        let preview_name = candidate
            .as_ref()
            .map(|font| font.name.clone())
            .unwrap_or_else(|| self.current.clone());
        let preview_label = candidate
            .as_ref()
            .map(|font| font.label.clone())
            .unwrap_or_else(|| trigger_label(&self.fonts, &self.current, self.loaded));
        let preview_font: SharedString = preview_name.into();
        let preview = div()
            .flex()
            .flex_col()
            .min_w_0()
            .p(px(28.0))
            .bg(c.panel_2)
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(c.muted)
                    .child("效果预览"),
            )
            .child(div().mt(px(12.0)).text_size(px(15.0)).child(preview_label))
            .when_some(candidate.clone(), |col, font| {
                col.when(font.name != font.label, |col| {
                    col.child(
                        div()
                            .text_size(px(11.0))
                            .text_color(c.muted)
                            .child(font.name),
                    )
                })
            })
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap(px(24.0))
                    .py(px(28.0))
                    .font_family(preview_font)
                    .child(div().text_size(px(26.0)).child(self.role.sample()))
                    .child(
                        div()
                            .text_size(px(22.0))
                            .child("Aa Bb Cc")
                            .child(div().child("0123456789")),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(c.muted)
                    .child("预览当前用途的文字、中英文与数字"),
            );

        let apply_label = if self.apply_all {
            "应用到全部"
        } else {
            "应用字体"
        };
        let footer = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(16.0))
            .px(px(24.0))
            .py(px(16.0))
            .border_t_1()
            .border_color(c.border)
            .child(
                Checkbox::new("apply-all")
                    .label("所有用途使用此字体")
                    .checked(self.apply_all)
                    .on_change(cx.listener(|this, checked: &bool, _window, cx| {
                        this.apply_all = *checked;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .child(Button::new("cancel-picker").ghost().label("取消").on_click(
                        cx.listener(|_this, _event, window, cx| {
                            window.close_dialog(cx);
                        }),
                    ))
                    .child(
                        Button::new("apply-font")
                            .primary()
                            .label(apply_label)
                            .disabled(candidate.is_none())
                            .on_click(cx.listener(|this, _event, window, cx| {
                                let query = this.search.read(cx).value().to_string();
                                let filtered =
                                    filtered_font_indices(&this.fonts, &query, this.role);
                                let Some(&index) = filtered.get(this.active) else {
                                    return;
                                };
                                let font = this.fonts[index].clone();
                                let role = this.role;
                                let apply_all = this.apply_all;
                                if let Some(page) = this.page.upgrade() {
                                    let _ = page.update(cx, |page, cx| {
                                        page.apply_font(role, &font.name, apply_all, window, cx)
                                    });
                                }
                                window.close_dialog(cx);
                            })),
                    ),
            );

        div()
            .id("font-picker")
            .flex()
            .flex_col()
            .bg(c.panel)
            // Enter 从搜索框冒泡上来（单行输入框对 Enter 会 `cx.propagate()`）。
            .on_key_down(
                cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    this.on_key(event, window, cx)
                }),
            )
            .child(header)
            .child(div().flex().flex_row().child(catalog).child(preview))
            .child(footer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font(name: &str, label: &str, aliases: &[&str]) -> SystemFont {
        SystemFont {
            name: name.into(),
            label: label.into(),
            aliases: aliases.iter().map(|alias| alias.to_string()).collect(),
        }
    }

    fn fonts() -> Vec<SystemFont> {
        vec![
            font("Arial", "Arial", &[]),
            font("Consolas", "Consolas", &[]),
            font("KaiTi", "楷体", &["KaiTi"]),
            font(
                "Microsoft YaHei",
                "微软雅黑",
                &["Microsoft YaHei UI", "微软雅黑"],
            ),
            font("SimSun", "宋体", &["SimSun"]),
        ]
    }

    #[test]
    fn set_font_under_unified_fills_every_role() {
        let mut settings = FontSettings::default();
        set_font(&mut settings, FontRole::Title, "KaiTi", true);
        for role in FontRole::ALL {
            assert_eq!(role.get(&settings), "KaiTi", "{role:?} 应同步");
        }
    }

    #[test]
    fn set_font_without_unified_touches_only_that_role() {
        let mut settings = FontSettings::default();
        set_font(&mut settings, FontRole::Code, "Consolas", false);
        assert_eq!(settings.code, "Consolas");
        assert_eq!(settings.body, DEFAULT_FONT);
        assert_eq!(settings.interface, DEFAULT_FONT);
    }

    #[test]
    fn toggling_unified_on_floods_body_value() {
        let mut settings = FontSettings::default();
        settings.body = "KaiTi".into();
        settings.title = "SimSun".into();
        // 原版 toggleUnified：开启时用「正文」值铺满。
        let body = settings.body.clone();
        apply_font_to_all(&mut settings, &body);
        for role in FontRole::ALL {
            assert_eq!(role.get(&settings), "KaiTi");
        }
    }

    #[test]
    fn reset_fonts_returns_to_defaults() {
        // 原版 resetFonts：全部角色归 Microsoft YaHei（core FontSettings::default）。
        let settings = FontSettings {
            interface: "Arial".into(),
            title: "KaiTi".into(),
            ..FontSettings::default()
        };
        assert_ne!(settings, FontSettings::default());
        let reset = FontSettings::default();
        assert_eq!(reset, normalize_fonts(&reset));
        assert_eq!(reset.interface, DEFAULT_FONT);
        assert_eq!(reset.title, DEFAULT_FONT);
    }

    #[test]
    fn normalize_fonts_replaces_blank_with_default() {
        let settings = FontSettings {
            title: "  KaiTi  ".into(),
            body: " ".into(),
            ..FontSettings::default()
        };
        let normalized = normalize_fonts(&settings);
        assert_eq!(normalized.title, "KaiTi");
        assert_eq!(normalized.body, DEFAULT_FONT);
    }

    #[test]
    fn matches_font_ignores_case_and_whitespace_and_uses_aliases() {
        let fonts = fonts();
        let yahei = fonts.iter().find(|f| f.name == "Microsoft YaHei").unwrap();
        assert!(matches_font(yahei, "microsoftyahei"));
        assert!(matches_font(yahei, "微软雅黑"));
        assert!(matches_font(yahei, "Microsoft YaHei UI"));
        assert!(!matches_font(yahei, "雅黑黑"));
    }

    #[test]
    fn find_system_font_matches_aliases_exactly() {
        let fonts = fonts();
        let found = find_system_font(&fonts, "microsoft yahei ui").unwrap();
        assert_eq!(found.name, "Microsoft YaHei");
        assert!(find_system_font(&fonts, "黑体").is_none());
        assert!(find_system_font(&fonts, "").is_none());
    }

    #[test]
    fn filtered_prioritizes_common_fonts_without_query() {
        let fonts = fonts();
        let filtered = filtered_font_indices(&fonts, "", FontRole::Body);
        // 常用名单里的 Microsoft YaHei / KaiTi / SimSun 排最前（按名单顺序）。
        let first_names: Vec<&str> = filtered
            .iter()
            .map(|&index| fonts[index].name.as_str())
            .collect();
        assert_eq!(first_names[0], "Microsoft YaHei");
        assert_eq!(first_names[1], "SimSun");
        assert_eq!(first_names[2], "KaiTi");
        assert_eq!(filtered.len(), fonts.len());
        // code 角色：Consolas 最前。
        let filtered = filtered_font_indices(&fonts, "", FontRole::Code);
        assert_eq!(fonts[filtered[0]].name, "Consolas");
    }

    #[test]
    fn filtered_with_query_returns_only_matches() {
        let fonts = fonts();
        let filtered = filtered_font_indices(&fonts, "楷体", FontRole::Body);
        assert_eq!(filtered.len(), 1);
        assert_eq!(fonts[filtered[0]].name, "KaiTi");
        let filtered = filtered_font_indices(&fonts, "  KaiTi ", FontRole::Body);
        assert_eq!(filtered.len(), 1);
    }
}
