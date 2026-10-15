//! 设置（原版 `SettingsView.vue`）。
//!
//! 已实现：左侧导航（五个分区）、界面主题卡片（点击即预览，保存后生效）、字体角色
//! （可搜索系统字体选择、统一所有字体、恢复默认、双标签预览）、模型服务（三类服务
//! 卡片、服务表单、连接状态卡、SOCKS5 代理）、生成参数、存储与缓存（图标向量缓存）、
//! 关于与帮助、底部统一保存条（撤销 / 保存 / Ctrl+S）、离开前的未保存确认。
//!
//! 子模块：`appearance` 字体与预览、`models` 服务卡片与连接状态、`service_form`
//! 可复用的服务表单（首次向导也用它）、`generation` 生成参数、`storage` 存储与缓存。

mod appearance;
mod generation;
mod models;
pub mod service_form;
mod storage;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::{Dialog, DialogFooter};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{Disableable as _, WindowExt as _};
use gpui_kit::prelude::*;
use gpui_kit::{
    div, px, Context, Entity, FocusHandle, KeyDownEvent, Render, SharedString, Subscription, Window,
};
use huanshu_core::config::LlmSettings;
use huanshu_core::fonts::SystemFont;
use huanshu_core::services::{settings, storage as storage_service};
use huanshu_core::types::{AppGlobalSettings, FontSettings};

use crate::icons::carbon;
use crate::router::{Route, SettingsTab};
use crate::shell::ShellHandle;
use crate::tasks::run_bg;
use crate::theme::{self, StudioExt as _};
use crate::widgets;

use service_form::{ModelTarget, ServiceForm};

pub use appearance::FontRole;

/// 预览卡片的两个标签页（原版 `previewTab`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewTab {
    Slides,
    Interface,
}

/// 原版分区说明（`sections[].desc`，逐字）。
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

    /// 上次保存的模型/生成参数（原版 `baseline`）。
    baseline: LlmSettings,
    /// 生成参数草稿（数字输入框解析后的值）。
    repair_rounds: usize,
    concurrency: usize,
    /// 数字输入框里有解析不了的文本（保存时按校验失败处理）。
    gen_invalid: bool,
    repair_input: Entity<InputState>,
    concurrency_input: Entity<InputState>,

    /// 三类服务的表单组件（原版三类卡片共用一份草稿区）。
    forms: [Entity<ServiceForm>; 3],
    active_service: ModelTarget,

    /// 字体草稿与上次保存值（原版 `fontDraft` / `fontBaseline`）。
    fonts: FontSettings,
    fonts_baseline: FontSettings,
    /// 「统一所有字体」开关。
    unified: bool,
    /// 后台枚举的系统字体（`None` = 还在读，照原版显示「系统字体」）。
    fonts_list: Option<Vec<SystemFont>>,
    /// 读取失败或读到空列表时的提示（原版 `stores/systemFonts.ts` 的 `error`）。
    fonts_error: Option<String>,
    preview_tab: PreviewTab,
    preview_role: FontRole,

    /// `app_settings` 快照（向量缓存状态、目录展示用）。
    app_settings: AppGlobalSettings,
    initializing_embeddings: bool,
    embedding_message: String,

    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl SettingsPage {
    pub fn new(
        handle: ShellHandle,
        tab: SettingsTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let shared = handle.shared();
        let saved =
            settings::get_app_theme(shared).unwrap_or_else(|_| cx.studio().theme.id.to_string());
        let llm = settings::get_settings(shared).unwrap_or_default();
        let app_settings = storage_service::get_app_settings(shared).unwrap_or_default();
        let fonts = appearance::normalize_fonts(&app_settings.fonts);

        let repair_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("0–10")
                .default_value(llm.repair_rounds.to_string())
        });
        let concurrency_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("1–8")
                .default_value(llm.concurrency.to_string())
        });
        let forms = [
            cx.new(|cx| {
                ServiceForm::new(
                    ModelTarget::Llm,
                    llm.llm.clone(),
                    shared.clone(),
                    window,
                    cx,
                )
            }),
            cx.new(|cx| {
                ServiceForm::new(
                    ModelTarget::Embedding,
                    llm.embedding.clone(),
                    shared.clone(),
                    window,
                    cx,
                )
            }),
            cx.new(|cx| {
                ServiceForm::new(
                    ModelTarget::Multimodal,
                    llm.multimodal.clone(),
                    shared.clone(),
                    window,
                    cx,
                )
            }),
        ];
        let subscriptions = vec![
            cx.subscribe(
                &repair_input,
                |this: &mut Self, state, _: &InputEvent, cx| {
                    let text = state.read(cx).value().to_string();
                    match text.trim().parse::<usize>() {
                        Ok(value) => {
                            this.gen_invalid = false;
                            if this.repair_rounds != value {
                                this.repair_rounds = value;
                                this.just_saved = false;
                            }
                        }
                        Err(_) => this.gen_invalid = true,
                    }
                    cx.notify();
                },
            ),
            cx.subscribe(
                &concurrency_input,
                |this: &mut Self, state, _: &InputEvent, cx| {
                    let text = state.read(cx).value().to_string();
                    match text.trim().parse::<usize>() {
                        Ok(value) => {
                            this.gen_invalid = false;
                            if this.concurrency != value {
                                this.concurrency = value;
                                this.just_saved = false;
                            }
                        }
                        Err(_) => this.gen_invalid = true,
                    }
                    cx.notify();
                },
            ),
        ];
        let focus = cx.focus_handle();

        let mut page = Self {
            handle,
            tab,
            draft_theme: saved.clone(),
            saved_theme: saved,
            saving: false,
            save_error: None,
            just_saved: false,
            baseline: llm.clone(),
            repair_rounds: llm.repair_rounds,
            concurrency: llm.concurrency,
            gen_invalid: false,
            repair_input,
            concurrency_input,
            forms,
            active_service: ModelTarget::Llm,
            fonts,
            fonts_baseline: app_settings.fonts.clone(),
            unified: false,
            fonts_list: None,
            fonts_error: None,
            preview_tab: PreviewTab::Slides,
            preview_role: FontRole::Body,
            app_settings,
            initializing_embeddings: false,
            embedding_message: String::new(),
            focus,
            _subscriptions: subscriptions,
        };
        page.load_fonts(window, cx);
        page
    }

    /// 字体枚举（DirectWrite，同步重活）走后台；照原版显示「已安装 N 种字体」。
    /// 失败与空列表都算失败（原版 `stores/systemFonts.ts`：`!result.length` 抛错），
    /// 设置页与字体弹层据此显示提示。
    fn load_fonts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        run_bg(
            window,
            cx,
            huanshu_core::fonts::enumerate_system_fonts,
            |page: &mut Self, result, _window, cx| {
                match result {
                    Ok(fonts) if !fonts.is_empty() => {
                        page.fonts_list = Some(fonts);
                        page.fonts_error = None;
                    }
                    Ok(_) => {
                        page.fonts_list = None;
                        page.fonts_error = Some("没有读取到可用字体，请刷新重试。".to_string());
                    }
                    Err(error) => {
                        page.fonts_list = None;
                        page.fonts_error = Some(error);
                    }
                }
                cx.notify();
            },
        );
    }

    /// 组装当前草稿（原版 `draft`：三类服务 + 修复轮数 + 并发数）。
    fn llm_draft(&self, cx: &gpui_kit::App) -> LlmSettings {
        LlmSettings {
            llm: self.forms[ModelTarget::Llm.index()].read(cx).draft.clone(),
            embedding: self.forms[ModelTarget::Embedding.index()]
                .read(cx)
                .draft
                .clone(),
            multimodal: self.forms[ModelTarget::Multimodal.index()]
                .read(cx)
                .draft
                .clone(),
            repair_rounds: self.repair_rounds,
            concurrency: self.concurrency,
        }
    }

    fn config_dirty(&self, cx: &gpui_kit::App) -> bool {
        serde_json::to_string(&self.llm_draft(cx)).ok()
            != serde_json::to_string(&self.baseline).ok()
    }

    fn fonts_dirty(&self) -> bool {
        self.fonts != self.fonts_baseline
    }

    fn theme_dirty(&self) -> bool {
        self.draft_theme != self.saved_theme
    }

    fn dirty(&self, cx: &gpui_kit::App) -> bool {
        self.config_dirty(cx) || self.fonts_dirty() || self.theme_dirty() || self.gen_invalid
    }

    fn preview_theme(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.draft_theme = id.to_string();
        self.just_saved = false;
        self.handle.set_theme(id, false, window, cx);
        // 原版的主题预览（`data-theme`）与字体预览（`--font-interface`）是两条独立的实时通道；
        // `set_theme` 用的是**已保存**的界面字体，这里把草稿补回去，否则换主题会丢掉字体预览。
        let interface = self.fonts.interface.clone();
        theme::apply(id, Some(&interface), Some(window), cx);
        cx.notify();
    }

    /// 字体草稿变化：立即把「界面」字体应用到主题（原版 `applyFonts` 的 watch），
    /// 其余角色体现在右侧预览。
    fn fonts_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.just_saved = false;
        let theme_id = cx.studio().theme.id.to_string();
        let interface = self.fonts.interface.clone();
        theme::apply(&theme_id, Some(&interface), Some(window), cx);
        cx.notify();
    }

    /// 字体选择弹层「应用字体 / 应用到全部」的回调。
    pub fn apply_font(
        &mut self,
        role: FontRole,
        value: &str,
        apply_all: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        appearance::set_font(&mut self.fonts, role, value, apply_all || self.unified);
        self.preview_role = role;
        self.fonts_changed(window, cx);
    }

    fn reset_fonts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.fonts = FontSettings::default();
        self.unified = false;
        self.fonts_changed(window, cx);
    }

    fn toggle_unified(&mut self, unified: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.unified = unified;
        if unified {
            // 原版 toggleUnified：开启时用「正文」铺满全部角色。
            let body = self.fonts.body.clone();
            appearance::apply_font_to_all(&mut self.fonts, &body);
        }
        self.fonts_changed(window, cx);
    }

    /// 点「统一所有字体」文字/开关时的取反入口（appearance 子模块用）。
    pub(super) fn toggle_unified_fonts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let unified = !self.unified;
        self.toggle_unified(unified, window, cx);
    }

    /// 开关控件的 on_change 入口（appearance 子模块用）。
    pub(super) fn set_unified(
        &mut self,
        unified: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if unified != self.unified {
            self.toggle_unified(unified, window, cx);
        }
    }

    fn revert(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let baseline = self.baseline.clone();
        for (form, draft) in [
            (&self.forms[0], &baseline.llm),
            (&self.forms[1], &baseline.embedding),
            (&self.forms[2], &baseline.multimodal),
        ] {
            form.update(cx, |form, cx| form.set_draft(draft.clone(), window, cx));
        }
        self.repair_rounds = baseline.repair_rounds;
        self.concurrency = baseline.concurrency;
        self.gen_invalid = false;
        self.repair_input.update(cx, |state, cx| {
            state.set_value(self.repair_rounds.to_string(), window, cx)
        });
        self.concurrency_input.update(cx, |state, cx| {
            state.set_value(self.concurrency.to_string(), window, cx)
        });
        self.fonts = self.fonts_baseline.clone();
        self.unified = false;
        self.save_error = None;
        let saved = self.saved_theme.clone();
        self.preview_theme(&saved, window, cx);
        cx.notify();
    }

    /// 原版 `save()`：校验生成参数 → 模型配置 → 字体 → 主题。同步 core 调用（写文件，量级小）。
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving || !self.dirty(cx) {
            return;
        }
        // 原版保存前的校验：repair 0–10 整数、concurrency 1–8 整数，失败跳到生成设置分区。
        if !generation::generation_valid(self.repair_rounds, self.concurrency, self.gen_invalid) {
            self.save_error = Some("修复轮数需为 0–10 的整数，并发数需为 1–8 的整数。".to_string());
            self.tab = SettingsTab::Generation;
            cx.notify();
            return;
        }
        self.saving = true;
        self.save_error = None;
        cx.notify();

        let shared = self.handle.shared().clone();
        let draft = self.llm_draft(cx);
        if self.config_dirty(cx) {
            match settings::save_settings(&shared, draft.clone()) {
                Ok(()) => self.baseline = draft.clone(),
                Err(error) => {
                    // 失败保留草稿（core 自己回滚持久层），可重试。
                    self.saving = false;
                    self.save_error =
                        Some(format!("保存未完成：{error}。未保存的修改仍保留，可重试。"));
                    cx.notify();
                    return;
                }
            }
        }
        if self.fonts_dirty() {
            let globals = storage_service::get_app_settings(&shared);
            match globals {
                Ok(mut globals) => {
                    globals.fonts = self.fonts.clone();
                    match storage_service::save_app_settings(&shared, globals) {
                        Ok(()) => self.fonts_baseline = self.fonts.clone(),
                        Err(error) => {
                            self.saving = false;
                            self.save_error =
                                Some(format!("保存未完成：{error}。未保存的修改仍保留，可重试。"));
                            cx.notify();
                            return;
                        }
                    }
                }
                Err(error) => {
                    self.saving = false;
                    self.save_error =
                        Some(format!("保存未完成：{error}。未保存的修改仍保留，可重试。"));
                    cx.notify();
                    return;
                }
            }
        }
        // 主题：持久化（services::settings::set_app_theme）并立即应用，
        // 界面字体用刚保存的值。
        self.handle.set_theme(&self.draft_theme, true, window, cx);
        self.saved_theme = self.draft_theme.clone();
        self.saving = false;
        self.just_saved = true;
        // 「更改已保存」显示 3 秒后回到「所有更改已保存」；页面已销毁时静默丢弃。
        cx.spawn_in(window, async move |this, window| {
            window
                .background_executor()
                .timer(std::time::Duration::from_secs(3))
                .await;
            let _ = this.update_in(window, |page, _window, cx| {
                page.just_saved = false;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// 「返回工作台」：有未保存更改先弹确认框（原版 `onBeforeRouteLeave` + `leavePrompt`）。
    fn leave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dirty(cx) {
            self.handle.navigate(Route::Home, window, cx);
            return;
        }
        let page = cx.weak_entity();
        window.open_dialog(cx, move |dialog: Dialog, _window, _cx| {
            let discard = page.clone();
            let save_leave = page.clone();
            let footer = DialogFooter::new().child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(Button::new("discard").danger().label("放弃更改").on_click(
                        move |_event, window, cx| {
                            window.close_dialog(cx);
                            let _ = discard.update(cx, |page, cx| {
                                page.revert(window, cx);
                                page.handle.navigate(Route::Home, window, cx);
                            });
                        },
                    ))
                    .child(Button::new("stay").ghost().label("继续编辑").on_click(
                        move |_event, window, cx| {
                            window.close_dialog(cx);
                        },
                    ))
                    .child(
                        Button::new("save-leave")
                            .primary()
                            .label("保存并离开")
                            .on_click(move |_event, window, cx| {
                                window.close_dialog(cx);
                                let _ = save_leave.update(cx, |page, cx| {
                                    page.save(window, cx);
                                    // 保存成功（不再 dirty）才离开；失败留在设置页看错误。
                                    if !page.dirty(cx) {
                                        page.handle.navigate(Route::Home, window, cx);
                                    }
                                });
                            }),
                    ),
            );
            dialog
                .title("有未保存的更改")
                .child(
                    div()
                        .text_size(px(13.0))
                        .child("离开前要保存吗？放弃后，主题与字体会恢复为上次保存的样式。"),
                )
                .footer(footer)
        });
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if keystroke.modifiers.control && keystroke.key.eq_ignore_ascii_case("s") {
            self.save(window, cx);
        }
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
            SettingsTab::Models => self.models(cx).into_any_element(),
            SettingsTab::Generation => self.generation(cx).into_any_element(),
            SettingsTab::Storage => self.storage(cx).into_any_element(),
            SettingsTab::About => self.about(cx).into_any_element(),
        };

        let status = if self.saving {
            "正在保存…"
        } else if self.just_saved {
            "更改已保存"
        } else if self.dirty(cx) {
            "有未保存的更改"
        } else {
            "所有更改已保存"
        };
        // 有未保存的更改时补一个 `Ctrl + S` 提示（原版 `SettingsView.vue` 534 行的 `kbd`）。
        let show_shortcut = self.dirty(cx) && !self.saving;
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
            // 原版 `.prefs-state-dot`：平时成功色，脏时用主色（settings.css 251–252）。
            .child(div().size(px(8.0)).rounded_full().bg(if self.dirty(cx) {
                c.primary
            } else {
                c.success
            }))
            .child(div().text_size(px(13.0)).text_color(c.muted).child(status))
            .when(show_shortcut, |bar| {
                bar.child(
                    div()
                        .px(px(6.0))
                        .py(px(1.0))
                        .rounded(px(4.0))
                        .border_1()
                        .border_color(c.border)
                        .bg(c.surface)
                        .text_size(px(11.0))
                        .text_color(c.muted)
                        .child("Ctrl + S"),
                )
            })
            .when_some(self.save_error.clone(), |bar, error| {
                bar.child(div().text_size(px(12.0)).text_color(c.error).child(error))
            })
            .child(div().flex_1())
            .child(
                widgets::ghost_button("revert", "撤销更改")
                    .disabled(!self.dirty(cx) || self.saving)
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
                .disabled(!self.dirty(cx) || self.saving)
                .on_click(cx.listener(|page, _event, window, cx| page.save(window, cx))),
            );

        div()
            .id("settings")
            .track_focus(&self.focus)
            .on_key_down(
                cx.listener(|page, event: &KeyDownEvent, window, cx| {
                    page.on_key(event, window, cx)
                }),
            )
            .size_full()
            .flex()
            .flex_row()
            .child(nav)
            .child(
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
                                    // 原版 `.prefs-heading h1 { font-size: 24px }`（settings.css 62）。
                                    .child(div().text_size(px(24.0)).child(self.tab.label()))
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
    /// 原版的排列：浅色 终端·纸 / 报刊·钴蓝 / 便签·稿纸，深色 暗·琥珀 / 暗·星图 / 终端·墨。
    fn theme_group(
        &mut self,
        tone: huanshu_core::themes::Tone,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let order: &[&'static str] = match tone {
            huanshu_core::themes::Tone::Light => {
                &["light-terminal", "light-editorial", "light-notes"]
            }
            huanshu_core::themes::Tone::Dark => &["dark-yellow", "dark-blue", "dark-terminal"],
        };
        let cards: Vec<_> = order
            .iter()
            .map(|id| {
                let theme = huanshu_core::themes::APP_THEMES
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
                if tone == huanshu_core::themes::Tone::Light {
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
        let t = crate::theme::tokens::table(id);
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
                    .relative()
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
                    )
                    // 原版 `.prefs-theme-check`（277–278 行 / settings.css 108）：选中卡片的勾选徽章。
                    .when(selected, |preview| {
                        preview.child(
                            div()
                                .absolute()
                                .top(px(6.0))
                                .right(px(6.0))
                                .size(px(18.0))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(c.primary)
                                .text_color(c.primary_contrast)
                                .text_size(px(12.0))
                                .child(carbon("checkmark")),
                        )
                    }),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(if selected { c.text } else { c.muted })
                    .child(name),
            )
    }

    /// 「关于与帮助」（原版 about 分区，已实现部分保持原样）。
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
            .max_w(px(880.0))
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
                            .child(div().text_size(px(18.0)).child("幻述 Studio"))
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
            // 原版 `.prefs-about-divider`（517 行 / settings.css 240）。
            .child(div().h(px(1.0)).w_full().bg(c.border))
            .child(div().text_size(px(15.0)).child("开始使用"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use huanshu_core::config::ModelServiceSettings;
    use huanshu_core::network::Socks5Settings;

    fn settings() -> LlmSettings {
        LlmSettings::default()
    }

    #[test]
    fn config_dirty_tracks_any_component_change() {
        let base = settings();
        assert_eq!(
            serde_json::to_string(&base).ok(),
            serde_json::to_string(&base).ok()
        );
        // 每个分量单独变化都要让序列化结果不同。
        let mut changed = base.clone();
        changed.llm.model = "other".into();
        assert_ne!(
            serde_json::to_string(&changed).ok(),
            serde_json::to_string(&base).ok()
        );
        let mut changed = base.clone();
        changed.embedding.base_url = "http://elsewhere".into();
        assert_ne!(
            serde_json::to_string(&changed).ok(),
            serde_json::to_string(&base).ok()
        );
        let mut changed = base.clone();
        changed.multimodal.proxy.enabled = true;
        assert_ne!(
            serde_json::to_string(&changed).ok(),
            serde_json::to_string(&base).ok()
        );
        let mut changed = base.clone();
        changed.repair_rounds = base.repair_rounds + 1;
        assert_ne!(
            serde_json::to_string(&changed).ok(),
            serde_json::to_string(&base).ok()
        );
        let mut changed = base.clone();
        changed.concurrency = base.concurrency + 1;
        assert_ne!(
            serde_json::to_string(&changed).ok(),
            serde_json::to_string(&base).ok()
        );
    }

    #[test]
    fn llm_settings_roundtrips_through_json_signature() {
        let mut value = settings();
        value.llm.api_key = "sk-test".into();
        value.embedding.proxy = Socks5Settings {
            enabled: true,
            url: "socks5://127.0.0.1:1080".into(),
            username: "u".into(),
            password: "p".into(),
        };
        let text = serde_json::to_string(&value).unwrap();
        let service: ModelServiceSettings =
            serde_json::from_str(&serde_json::to_string(&value.llm).unwrap()).unwrap();
        assert_eq!(service.api_key, "sk-test");
        assert!(text.contains("repair_rounds"));
    }
}
