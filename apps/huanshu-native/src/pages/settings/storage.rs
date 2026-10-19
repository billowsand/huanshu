//! 「存储与缓存」分区（原版 `SettingsView.vue` 477–507 行，文案逐字）：
//! 文件位置（只读）+ 图标向量缓存（状态、初始化）。

use gpui_kit::component::progress::Progress;
use gpui_kit::component::{Disableable as _, WindowExt as _};
use gpui_kit::prelude::*;
use gpui_kit::{div, px, Context, Render, Window};
use huanshu_core::services::{generation, storage};

use super::service_form::ModelTarget;
use super::SettingsPage;
use crate::tasks::run_async;
use crate::theme::StudioExt as _;
use crate::widgets;

/// A blocking dialog keeps the settings page alive until initialization finishes.
struct EmbeddingInitDialog {
    running: bool,
    failed: bool,
    message: String,
}

impl Render for EmbeddingInitDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        div()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(
                Progress::new("embedding-init-progress")
                    .loading(self.running)
                    .value(if self.running || self.failed {
                        0.0
                    } else {
                        100.0
                    })
                    .color(if self.failed { c.error } else { c.primary })
                    .accessibility_label("图标向量缓存初始化进度"),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(if self.failed { c.error } else { c.text })
                    .child(self.message.clone()),
            )
            .when(self.running, |body| {
                body.child(
                    div().text_size(px(12.0)).text_color(c.muted).child(
                        "首次初始化可能需要几分钟，请保持应用打开。完成前暂时无法操作设置。",
                    ),
                )
            })
            .when(!self.running, |body| {
                body.child(
                    div().flex().justify_end().child(
                        widgets::ghost_button("close-embedding-init", "关闭")
                            .on_click(|_event, window, cx| window.close_dialog(cx)),
                    ),
                )
            })
    }
}

impl SettingsPage {
    /// 向量缓存「已初始化」：embeddings_ready 且草稿里的向量模型与已初始化的模型一致
    /// （原版 `embeddingReady`）。
    fn embedding_ready(&self, cx: &gpui_kit::App) -> bool {
        let model = self
            .form(ModelTarget::Embedding)
            .read(cx)
            .draft
            .model
            .clone();
        let model = model.trim();
        self.app_settings.embeddings_ready
            && !model.is_empty()
            && model == self.app_settings.initialized_embedding_model.trim()
    }

    /// 原版 `embeddingStatus`：已初始化 / 模型已变更 / 未初始化。
    fn embedding_status(&self, cx: &gpui_kit::App) -> &'static str {
        if self.embedding_ready(cx) {
            "已初始化"
        } else if self.app_settings.embeddings_ready {
            "模型已变更"
        } else {
            "未初始化"
        }
    }

    /// 原版 `initEmbeddings()`：`ensure_icon_embeddings`（async → `tasks::run_async`）。
    fn init_embeddings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.initializing_embeddings || self.saving || self.config_dirty(cx) || !self.ready() {
            return;
        }
        self.initializing_embeddings = true;
        self.embedding_message = "正在初始化图标向量缓存…".to_string();
        let progress = cx.new(|_cx| EmbeddingInitDialog {
            running: true,
            failed: false,
            message: self.embedding_message.clone(),
        });
        let content = progress.clone();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title("初始化图标向量缓存")
                .w(px(440.0))
                .overlay(true)
                .overlay_closable(false)
                .keyboard(false)
                .close_button(false)
                .on_cancel(|_event, _window, _cx| false)
                .on_ok(|_event, _window, _cx| false)
                .child(content.clone())
        });
        cx.notify();
        let shared = self.handle.shared().clone();
        run_async(
            window,
            cx,
            async move {
                // Convert task panics into a failure too, so the modal never stays locked.
                crate::tasks::tokio()
                    .spawn(async move { generation::ensure_icon_embeddings(&shared).await })
                    .await
                    .unwrap_or_else(|error| Err(format!("初始化任务异常，请重试：{error}")))
            },
            move |page: &mut Self, result, _window, cx| {
                page.initializing_embeddings = false;
                let failed = result.is_err();
                match result {
                    Ok(()) => {
                        // 成功后重新读 app_settings 刷新状态（原版 appSettings.load()）。
                        if let Ok(settings) = storage::get_app_settings(page.handle.shared()) {
                            page.app_settings = settings;
                        }
                        page.embedding_message = "向量缓存初始化完成。".to_string();
                    }
                    Err(error) => page.embedding_message = format!("初始化失败：{error}"),
                }
                progress.update(cx, |dialog, cx| {
                    dialog.running = false;
                    dialog.failed = failed;
                    dialog.message = page.embedding_message.clone();
                    cx.notify();
                });
                cx.notify();
            },
        );
    }

    pub(super) fn storage(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        let data_dir = self.app_settings.data_dir.clone();
        let media_dir = self.app_settings.media_dir.clone();
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
                        .rounded(widgets::radius(cx, 6.0))
                        .bg(c.surface)
                        .border_1()
                        .border_color(c.border)
                        .text_size(px(13.0))
                        .child(value),
                )
                .child(div().text_size(px(12.0)).text_color(c.muted).child(hint))
        };

        let ready = self.embedding_ready(cx);
        let status = self.embedding_status(cx);
        let config_dirty = self.config_dirty(cx);
        let initializing = self.initializing_embeddings;
        let embedding_model = self
            .form(ModelTarget::Embedding)
            .read(cx)
            .draft
            .model
            .trim()
            .to_string();
        let cached_model = self
            .app_settings
            .initialized_embedding_model
            .trim()
            .to_string();

        // 原版 494 行用 `is-ok` / `is-untested`，而 settings.css 只给 `is-ok` 换了颜色，
        // `is-untested` 回落到 `.prefs-status` 的默认弱化底。
        let (chip_fg, chip_bg, chip_border) = if ready {
            (c.success, c.success_bg, c.success)
        } else {
            (c.muted, c.surface, c.border)
        };
        let status_badge = div()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(5.0))
            .px(px(8.0))
            .py(px(2.0))
            .rounded(widgets::radius(cx, 999.0))
            .border_1()
            .border_color(chip_border)
            .bg(chip_bg)
            .text_size(px(11.5))
            .text_color(chip_fg)
            .child(div().size(px(6.0)).rounded_full().bg(chip_fg))
            .child(status);
        let button_label = if initializing {
            "初始化中…"
        } else if ready {
            "缓存已就绪"
        } else {
            "初始化缓存"
        };
        let button_help = if config_dirty {
            "请先保存模型配置，再初始化缓存。"
        } else {
            "使用已保存的模型服务配置。"
        };

        div()
            .flex()
            .flex_col()
            .gap(px(18.0))
            .child(
                // 原版 `.prefs-simple-panel { max-width: 880px }`（settings.css 220）。
                widgets::panel(cx)
                    .max_w(px(880.0))
                    .p(px(18.0))
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .child(div().text_size(px(16.0)).child("文件位置"))
                    .child(row("数据目录", data_dir, "存放数据库和项目配置文件。"))
                    .child(row("媒体目录", media_dir, "存放上传的图片和视频。")),
            )
            .child(
                widgets::panel(cx)
                    .max_w(px(880.0))
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
                            .child(div().text_size(px(16.0)).child("图标向量缓存"))
                            .child(status_badge),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(c.muted)
                            .child("为图标建立语义索引，提高内容与图标的匹配效果。"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .text_size(px(13.0))
                                    .child(div().text_color(c.muted).child("当前向量模型"))
                                    .child(div().child(if embedding_model.is_empty() {
                                        "未配置".to_string()
                                    } else {
                                        embedding_model
                                    })),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    // 原版 `.prefs-summary > div + div`：虚线分隔（css 208）。
                                    .border_t_1()
                                    .border_dashed()
                                    .border_color(c.border)
                                    .justify_between()
                                    .text_size(px(13.0))
                                    .child(div().text_color(c.muted).child("缓存对应模型"))
                                    .child(div().child(if cached_model.is_empty() {
                                        "—".to_string()
                                    } else {
                                        cached_model
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(10.0))
                            .child(
                                widgets::ghost_button("init-embeddings", button_label)
                                    .disabled(initializing || self.saving || ready || config_dirty)
                                    .on_click(cx.listener(|page, _event, window, cx| {
                                        page.init_embeddings(window, cx)
                                    })),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(c.muted)
                                    .child(button_help),
                            ),
                    )
                    .when(!self.embedding_message.is_empty(), |card| {
                        card.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(c.muted)
                                .child(self.embedding_message.clone()),
                        )
                    }),
            )
    }
}
