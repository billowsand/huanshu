//! 设置（原版 `SettingsView.vue`）——**骨架**。
//!
//! 已实现：左侧导航（五个分区）、分区标题与说明、「界面主题」卡片（点击即预览，保存后生效）、
//! 「存储与缓存」的目录展示、「关于与帮助」、底部保存条（撤销 / 保存）。
//! 待实现（各分区里的黄色虚线框逐条列出）：字体角色选择与预览、模型服务三联表单与测试连接、
//! 生成参数、图标向量缓存初始化、离开前的未保存确认。

use gpui_kit::component::Disableable as _;
use gpui_kit::prelude::*;
use gpui_kit::{div, px, Context, Render, SharedString, Window};
use huanshu_core::services::settings;
use huanshu_core::themes::{self, Tone};

use crate::icons::carbon;
use crate::router::{Route, SettingsTab};
use crate::shell::ShellHandle;
use crate::theme::{tokens, StudioExt as _};
use crate::widgets;

/// 原版分区说明（`sections[].desc`）。
fn tab_description(tab: SettingsTab) -> &'static str {
    match tab {
        SettingsTab::Appearance => "自定义工作环境与演示文稿的文字风格。",
        SettingsTab::Models => "为生成、检索与图片理解配置独立服务。",
        SettingsTab::Generation => "控制生成效率与自动修复行为。",
        SettingsTab::Storage => "查看文件位置，管理图标检索的向量缓存。",
        SettingsTab::About => "从 Markdown 到演示文稿，让想法清晰呈现。",
    }
}

pub struct SettingsPage {
    handle: ShellHandle,
    tab: SettingsTab,
    /// 已保存的主题与正在预览的主题（原版「点击即可预览，保存后生效」）。
    saved_theme: String,
    draft_theme: String,
    saving: bool,
    save_error: Option<String>,
    just_saved: bool,
}

impl SettingsPage {
    pub fn new(
        handle: ShellHandle,
        tab: SettingsTab,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let saved = settings::get_app_theme(handle.shared())
            .unwrap_or_else(|_| cx.studio().theme.id.to_string());
        Self {
            handle,
            tab,
            draft_theme: saved.clone(),
            saved_theme: saved,
            saving: false,
            save_error: None,
            just_saved: false,
        }
    }

    fn dirty(&self) -> bool {
        self.draft_theme != self.saved_theme
    }

    fn preview_theme(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.draft_theme = id.to_string();
        self.just_saved = false;
        self.handle.set_theme(id, false, window, cx);
        cx.notify();
    }

    fn revert(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let saved = self.saved_theme.clone();
        self.preview_theme(&saved, window, cx);
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.saving = true;
        self.handle.set_theme(&self.draft_theme, true, window, cx);
        self.saved_theme = self.draft_theme.clone();
        self.saving = false;
        self.just_saved = true;
        self.save_error = None;
        cx.notify();
    }

    fn leave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // TODO(parity): 有未保存更改时原版弹「有未保存的更改」确认框（保存并离开 / 放弃更改 / 继续编辑）。
        // 骨架阶段先按「放弃」处理：恢复已保存的主题再离开。
        if self.dirty() {
            self.revert(window, cx);
        }
        self.handle.navigate(Route::Home, window, cx);
    }
}

impl Render for SettingsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;

        let nav = div()
            .flex_none()
            .w(px(204.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(c.panel)
            .border_r_1()
            .border_color(c.border)
            .p(px(12.0))
            .child(
                widgets::ghost_button("back", "返回工作台")
                    .icon(carbon("arrow-left"))
                    .on_click(cx.listener(|page, _event, window, cx| page.leave(window, cx))),
            )
            .child(
                div()
                    .px(px(10.0))
                    .pt(px(18.0))
                    .pb(px(10.0))
                    .text_size(px(20.0))
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .child("设置"),
            )
            .children(SettingsTab::ALL.iter().map(|tab| {
                let tab = *tab;
                let selected = tab == self.tab;
                div()
                    .id(SharedString::from(format!("tab-{}", tab.label())))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .px(px(12.0))
                    .py(px(9.0))
                    .rounded(px(8.0))
                    .text_size(px(14.0))
                    .cursor_pointer()
                    .when(selected, |item| {
                        item.bg(c.primary_bg)
                            .text_color(c.primary)
                            .border_l_2()
                            .border_color(c.primary)
                    })
                    .when(!selected, |item| {
                        item.text_color(c.muted).hover(|style| style.bg(c.surface))
                    })
                    .child(carbon(tab.icon()))
                    .child(tab.label())
                    .on_click(cx.listener(move |page, _event, _window, cx| {
                        page.tab = tab;
                        cx.notify();
                    }))
            }))
            .child(div().flex_1())
            .child(
                div()
                    .px(px(10.0))
                    .text_size(px(12.0))
                    .text_color(c.muted)
                    .child(format!("v{}", env!("CARGO_PKG_VERSION"))),
            );

        let section = match self.tab {
            SettingsTab::Appearance => self.appearance(cx).into_any_element(),
            SettingsTab::Models => widgets::todo_block(
                "模型服务",
                "SettingsView.vue「models」分区（第 344–450 行）",
                &[
                    "左侧三类服务卡片：大语言模型 / 向量模型 / 多模态模型，显示连接状态",
                    "服务地址、API Key、模型（下拉，可切换「手动输入模型 ID」）",
                    "网络代理开关 + socks5 地址、用户名、密码",
                    "「连接状态」卡片：服务类型、模型、响应耗时、测试连接（services::settings::list_models）",
                    "「这个服务用于」说明列表",
                    "保存经 services::settings::save_settings（LlmSettings）",
                ],
                cx,
            )
            .into_any_element(),
            SettingsTab::Generation => widgets::todo_block(
                "生成设置",
                "SettingsView.vue「generation」分区（第 453–475 行）",
                &[
                    "自动修复轮数（建议 2–4 轮）",
                    "生成并发数（本地模型建议 1，最多 8）与负载提示",
                ],
                cx,
            )
            .into_any_element(),
            SettingsTab::Storage => self.storage(cx).into_any_element(),
            SettingsTab::About => self.about(cx).into_any_element(),
        };

        let status = if self.saving {
            "正在保存…"
        } else if self.just_saved {
            "更改已保存"
        } else if self.dirty() {
            "有未保存的更改"
        } else {
            "所有更改已保存"
        };
        let save_bar = div()
            .flex_none()
            .h(px(56.0))
            .px(px(28.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .bg(c.panel)
            .border_t_1()
            .border_color(c.border)
            .child(div().size(px(8.0)).rounded_full().bg(if self.dirty() {
                c.warning
            } else {
                c.success
            }))
            .child(div().text_size(px(13.0)).text_color(c.muted).child(status))
            .when_some(self.save_error.clone(), |bar, error| {
                bar.child(div().text_size(px(12.0)).text_color(c.error).child(error))
            })
            .child(div().flex_1())
            .child(
                widgets::ghost_button("revert", "撤销更改")
                    .disabled(!self.dirty())
                    .on_click(cx.listener(|page, _event, window, cx| page.revert(window, cx))),
            )
            .child(
                widgets::primary_button(
                    "save",
                    if self.saving {
                        "保存中…"
                    } else {
                        "保存更改"
                    },
                )
                .disabled(!self.dirty() || self.saving)
                .on_click(cx.listener(|page, _event, window, cx| page.save(window, cx))),
            );

        div().size_full().flex().flex_row().child(nav).child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .id("settings-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .px(px(30.0))
                        .py(px(28.0))
                        .flex()
                        .flex_col()
                        .gap(px(22.0))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .text_size(px(26.0))
                                        .font_weight(gpui_kit::FontWeight::BOLD)
                                        .child(self.tab.label()),
                                )
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .text_color(c.muted)
                                        .child(tab_description(self.tab)),
                                ),
                        )
                        .child(section),
                )
                .child(save_bar),
        )
    }
}

impl SettingsPage {
    /// 「外观与字体」：界面主题卡片（浅色一组、深色一组）+ 字体（待实现）。
    fn appearance(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        let light = self.theme_group(Tone::Light, cx);
        let dark = self.theme_group(Tone::Dark, cx);
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
                            .child(div().text_size(px(17.0)).font_weight(gpui_kit::FontWeight::BOLD).child("界面主题"))
                            .child(div().text_size(px(13.0)).text_color(c.muted).child("点击即可预览，保存后生效")),
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
            .child(widgets::todo_block(
                "字体",
                "SettingsView.vue「appearance」字体区（第 289–340 行）与 SystemFontPicker.vue",
                &[
                    "七个角色：界面 / 标题 / 副标题 / 正文 / 注释与标签 / 数字与序号 / 代码与编辑器",
                    "每个角色一个可搜索的系统字体下拉（fonts::enumerate_system_fonts，已安装 N 种字体）",
                    "「统一所有字体」开关、「恢复默认」",
                    "右侧「演示预览 / 界面预览」两个标签页的实时预览",
                    "保存经 services::storage::save_app_settings（AppGlobalSettings.fonts）；界面字体保存后调 theme::apply",
                ],
                cx,
            ))
    }

    /// 原版的排列：浅色 终端·纸 / 报刊·钴蓝 / 便签·稿纸，深色 暗·琥珀 / 暗·星图 / 终端·墨。
    fn theme_group(&mut self, tone: Tone, cx: &mut Context<Self>) -> impl IntoElement {
        let order: &[&'static str] = match tone {
            Tone::Light => &["light-terminal", "light-editorial", "light-notes"],
            Tone::Dark => &["dark-yellow", "dark-blue", "dark-terminal"],
        };
        let cards: Vec<_> = order
            .iter()
            .map(|id| {
                let theme = themes::APP_THEMES
                    .iter()
                    .find(|theme| theme.id == *id)
                    .expect("catalogue ids");
                self.theme_card(theme.id, theme.name, cx).into_any_element()
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(widgets::section_label(
                if tone == Tone::Light {
                    "浅色"
                } else {
                    "深色"
                },
                cx,
            ))
            .child(div().flex().flex_row().gap(px(12.0)).children(cards))
    }

    /// 一张主题预览卡：缩小的「侧栏 + 内容 + 主按钮」示意图。
    fn theme_card(
        &mut self,
        id: &'static str,
        name: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let c = cx.studio().colors;
        let t = tokens::table(id);
        let color = |value: u32| -> gpui_kit::Hsla { gpui_kit::rgba(value).into() };
        let selected = self.draft_theme == id;
        div()
            .id(SharedString::from(format!("theme-card-{id}")))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.0))
            .cursor_pointer()
            .on_click(
                cx.listener(move |page, _event, window, cx| page.preview_theme(id, window, cx)),
            )
            .child(
                div()
                    .w(px(124.0))
                    .h(px(78.0))
                    .rounded(px(8.0))
                    .border_2()
                    .border_color(if selected { c.primary } else { c.border })
                    .bg(color(t.bg))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(10.0))
                            .px(px(5.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(2.0))
                            .bg(color(t.panel))
                            .children(
                                (0..3)
                                    .map(|_| div().size(px(3.0)).rounded_full().bg(color(t.muted))),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .child(
                                div()
                                    .w(px(30.0))
                                    .h_full()
                                    .bg(color(t.panel))
                                    .p(px(5.0))
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.0))
                                    .child(div().h(px(3.0)).w(px(16.0)).bg(color(t.primary)))
                                    .child(div().h(px(3.0)).w(px(16.0)).bg(color(t.border_hover)))
                                    .child(div().h(px(3.0)).w(px(16.0)).bg(color(t.border_hover))),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .p(px(6.0))
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.0))
                                    .child(div().h(px(3.0)).w(px(56.0)).bg(color(t.text)))
                                    .child(div().h(px(3.0)).w(px(44.0)).bg(color(t.muted)))
                                    .child(div().h(px(3.0)).w(px(50.0)).bg(color(t.muted)))
                                    .child(div().flex_1())
                                    .child(
                                        div().flex().flex_row().justify_end().child(
                                            div()
                                                .h(px(9.0))
                                                .w(px(24.0))
                                                .rounded(px(2.0))
                                                .bg(color(t.primary)),
                                        ),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(if selected { c.text } else { c.muted })
                    .child(name),
            )
    }

    fn storage(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        let (data_dir, media_dir) = self
            .handle
            .shared()
            .lock()
            .map(|state| {
                (
                    state.app_settings.data_dir.clone(),
                    state.app_settings.media_dir.clone(),
                )
            })
            .unwrap_or_default();
        let row = |label: &'static str, value: String, hint: &'static str| {
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(div().text_size(px(13.0)).child(label))
                .child(
                    div()
                        .px(px(10.0))
                        .py(px(7.0))
                        .rounded(px(6.0))
                        .bg(c.surface)
                        .border_1()
                        .border_color(c.border)
                        .text_size(px(13.0))
                        .child(value),
                )
                .child(div().text_size(px(12.0)).text_color(c.muted).child(hint))
        };
        div()
            .flex()
            .flex_col()
            .gap(px(18.0))
            .child(
                widgets::panel(cx)
                    .p(px(18.0))
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .child(div().text_size(px(16.0)).font_weight(gpui_kit::FontWeight::BOLD).child("文件位置"))
                    .child(row("数据目录", data_dir, "存放数据库和项目配置文件。"))
                    .child(row("媒体目录", media_dir, "存放上传的图片和视频。")),
            )
            .child(widgets::todo_block(
                "图标向量缓存",
                "SettingsView.vue「storage」分区（第 493–510 行）",
                &[
                    "状态（缓存已就绪 / 未初始化）、当前向量模型、缓存对应模型",
                    "「初始化缓存」按钮：services::generation::ensure_icon_embeddings（async，走 tasks::run_async）",
                    "模型配置未保存时禁用并提示「请先保存模型配置，再初始化缓存。」",
                ],
                cx,
            ))
    }

    fn about(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        let step = |n: usize, text: &'static str| {
            div()
                .flex()
                .flex_row()
                .gap(px(10.0))
                .child(
                    div()
                        .size(px(22.0))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(c.primary_bg)
                        .text_color(c.primary)
                        .text_size(px(12.0))
                        .child(n.to_string()),
                )
                .child(div().text_size(px(14.0)).child(text))
        };
        widgets::panel(cx)
            .p(px(22.0))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(14.0))
                    .child(widgets::brand_mark(44.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(18.0))
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .child("幻述 Studio"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(c.muted)
                                    .child(format!("版本 {}", env!("CARGO_PKG_VERSION"))),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .child("将 Markdown 文档自动转化为精美的演示文稿。"),
            )
            .child(
                div()
                    .text_size(px(15.0))
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .child("开始使用"),
            )
            .child(step(
                1,
                "在「模型服务」中配置文本生成、向量检索和图片理解服务。",
            ))
            .child(step(
                2,
                "测试连接并保存设置，需要图标检索时初始化向量缓存。",
            ))
            .child(step(
                3,
                "返回工作台，上传 Markdown 文件，开始生成演示文稿。",
            ))
    }
}
