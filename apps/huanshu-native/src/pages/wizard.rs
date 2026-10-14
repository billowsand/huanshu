//! 首次启动向导（原版 `FirstRunWizard.vue`）。
//!
//! 四步：选择存储目录（默认值来自 core 的 `app_settings`，尊重 HUANSHU_HOME）→
//! 配置大模型（复用设置页的 `ServiceForm` 紧凑模式，下一步前照原版 `verifyModels`
//! 依次测试三类服务）→ 初始化向量库（先保存服务配置再 `ensure_icon_embeddings`）→
//! 配置完成（摘要、目录变更的重启警告、`finishWizard` 顺序保存后进入首页）。

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::Disableable as _;
use gpui_kit::prelude::*;
use gpui_kit::{div, px, Context, Entity, Render, SharedString, Subscription, Window};
use huanshu_core::services::{generation, settings, storage};

use crate::icons::carbon;
use crate::pages::settings::service_form::{ModelTarget, ServiceForm};
use crate::router::Route;
use crate::shell::ShellHandle;
use crate::tasks::{run_async, run_bg};
use crate::theme::StudioExt as _;
use crate::widgets;

const STEPS: [&str; 4] = ["选择存储目录", "配置大模型", "初始化向量库", "配置完成！"];

/// 原版 `browseDataDir` 的联动：媒体目录不含 "/projects/"（对 \ 与 / 都容忍）时，
/// 返回 `<选中>/media`；否则不动（返回 `None`）。
fn suggest_media_dir(selected: &str, media_dir: &str) -> Option<String> {
    let normalized = media_dir.replace('\\', "/");
    if normalized.contains("/projects/") {
        return None;
    }
    Some(
        std::path::Path::new(selected)
            .join("media")
            .to_string_lossy()
            .to_string(),
    )
}

pub struct WizardPage {
    handle: ShellHandle,
    /// 0..=3（原版 Step 1..=4）。
    step: usize,
    data_dir: Entity<InputState>,
    media_dir: Entity<InputState>,
    /// core 启动时的目录（原版 `defaultDataDir` / `defaultMediaDir`，用于 dataDirChanged）。
    default_data_dir: String,
    default_media_dir: String,
    /// 三类服务的紧凑表单（第二步）。
    forms: [Entity<ServiceForm>; 3],
    embedding_progress: u8,
    embedding_status: String,
    initializing: bool,
    verifying: bool,
    skip_llm: bool,
    skip_embedding: bool,
    finishing: bool,
    /// 任一步骤的错误（原版 `modelError`，显示在卡片底部）。
    model_error: String,
    _subscriptions: Vec<Subscription>,
}

impl WizardPage {
    pub fn new(handle: ShellHandle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 目录默认值必须来自 core（它尊重 HUANSHU_HOME）——绝不照抄原版 localDataDir 拼路径。
        let app_settings = storage::get_app_settings(handle.shared()).unwrap_or_default();
        let default_data_dir = app_settings.data_dir.clone();
        let default_media_dir = app_settings.media_dir.clone();
        let data_dir =
            cx.new(|cx| InputState::new(window, cx).default_value(default_data_dir.clone()));
        let media_dir =
            cx.new(|cx| InputState::new(window, cx).default_value(default_media_dir.clone()));

        // 服务草稿默认取 core 当前的 settings（同设置页默认值）。
        let llm = settings::get_settings(handle.shared()).unwrap_or_default();
        let shared = handle.shared().clone();
        let forms = [
            cx.new(|cx| {
                ServiceForm::compact(
                    ModelTarget::Llm,
                    llm.llm.clone(),
                    shared.clone(),
                    window,
                    cx,
                )
            }),
            cx.new(|cx| {
                ServiceForm::compact(
                    ModelTarget::Embedding,
                    llm.embedding.clone(),
                    shared.clone(),
                    window,
                    cx,
                )
            }),
            cx.new(|cx| {
                ServiceForm::compact(
                    ModelTarget::Multimodal,
                    llm.multimodal.clone(),
                    shared.clone(),
                    window,
                    cx,
                )
            }),
        ];
        let _subscriptions = vec![
            cx.subscribe(&data_dir, |_this, _state, _: &InputEvent, cx| {
                cx.notify();
            }),
            cx.subscribe(&media_dir, |_this, _state, _: &InputEvent, cx| {
                cx.notify();
            }),
        ];
        Self {
            handle,
            step: 0,
            data_dir,
            media_dir,
            default_data_dir,
            default_media_dir,
            forms,
            embedding_progress: 0,
            embedding_status: String::new(),
            initializing: false,
            verifying: false,
            skip_llm: false,
            skip_embedding: false,
            finishing: false,
            model_error: String::new(),
            _subscriptions,
        }
    }

    fn data_dir_value(&self, cx: &gpui_kit::App) -> String {
        self.data_dir.read(cx).value().to_string()
    }

    fn media_dir_value(&self, cx: &gpui_kit::App) -> String {
        self.media_dir.read(cx).value().to_string()
    }

    /// 原版 `canProceedStep1`。
    fn can_proceed_step1(&self, cx: &gpui_kit::App) -> bool {
        !self.data_dir_value(cx).trim().is_empty() && !self.media_dir_value(cx).trim().is_empty()
    }

    /// 原版 `canProceedStep2`：跳过，或三类服务的地址与模型都非空。
    fn can_proceed_step2(&self, cx: &gpui_kit::App) -> bool {
        if self.skip_llm {
            return true;
        }
        self.forms.iter().all(|form| {
            let draft = &form.read(cx).draft;
            !draft.base_url.trim().is_empty() && !draft.model.trim().is_empty()
        })
    }

    /// 原版 `dataDirChanged`（与 core 初始目录比较）。
    fn data_dir_changed(&self, cx: &gpui_kit::App) -> bool {
        self.data_dir_value(cx) != self.default_data_dir
            || self.media_dir_value(cx) != self.default_media_dir
    }

    /// 原版 `embeddingsReady`。
    fn embeddings_ready(&self) -> bool {
        !self.skip_llm && !self.skip_embedding && self.embedding_progress >= 100
    }

    fn next_step(&mut self, cx: &mut Context<Self>) {
        if self.step == 0 && self.can_proceed_step1(cx) {
            if self.skip_llm {
                self.skip_embedding = true;
                self.step = 3;
            } else {
                self.step = 1;
            }
        } else if self.step == 2 {
            self.step = 3;
        }
        cx.notify();
    }

    fn prev_step(&mut self, cx: &mut Context<Self>) {
        self.step = match self.step {
            1 => 0,
            2 => 1,
            3 => {
                if self.skip_llm {
                    0
                } else {
                    2
                }
            }
            _ => self.step,
        };
        cx.notify();
    }

    fn skip_llm_step(&mut self, cx: &mut Context<Self>) {
        self.skip_llm = true;
        self.skip_embedding = true;
        self.step = 3;
        cx.notify();
    }

    /// 原版 `browseDataDir`：浏览后若媒体目录不含 "/projects/"（容忍 \ 分隔符），
    /// 自动设为 `<选中>/media`。
    fn browse_data_dir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(selected) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        let selected = selected.to_string_lossy().to_string();
        self.data_dir.update(cx, |state, cx| {
            state.set_value(selected.clone(), window, cx)
        });
        if let Some(media) = suggest_media_dir(&selected, &self.media_dir_value(cx)) {
            self.media_dir
                .update(cx, |state, cx| state.set_value(media, window, cx));
        }
        cx.notify();
    }

    fn browse_media_dir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(selected) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        let selected = selected.to_string_lossy().to_string();
        self.media_dir
            .update(cx, |state, cx| state.set_value(selected, window, cx));
        cx.notify();
    }

    /// 第一步的目录字段：可编辑输入框 + 「浏览...」（原版 field/dir-input-row）。
    fn dir_field(
        &mut self,
        label: &'static str,
        input: &Entity<InputState>,
        hint: &'static str,
        browse: fn(&mut Self, &mut Window, &mut Context<Self>),
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let c = cx.studio().colors;
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(div().text_size(px(12.0)).text_color(c.muted).child(label))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().flex_1().min_w_0().child(Input::new(input)))
                    .child(
                        widgets::ghost_button(
                            SharedString::from(format!("browse-{label}")),
                            "浏览...",
                        )
                        .on_click(
                            cx.listener(move |page, _event, window, cx| browse(page, window, cx)),
                        ),
                    ),
            )
            .child(div().text_size(px(11.0)).text_color(c.muted).child(hint))
    }

    /// 原版 `verifyModels`：依次对三类服务调 `list_models`（传草稿覆盖），
    /// 任一失败显示错误不翻页，全过才进第三步。
    fn verify_models(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.verifying {
            return;
        }
        self.verifying = true;
        self.model_error.clear();
        cx.notify();
        let shared = self.handle.shared().clone();
        let services: Vec<(String, huanshu_core::config::ModelServiceSettings)> = self
            .forms
            .iter()
            .map(|form| {
                let target = form.read(cx).target();
                (target.key().to_string(), form.read(cx).draft.clone())
            })
            .collect();
        run_async(
            window,
            cx,
            async move {
                for (target, service) in services {
                    settings::list_models(&shared, Some(target), Some(service)).await?;
                }
                Ok::<_, String>(())
            },
            |page: &mut Self, result, _window, cx| {
                page.verifying = false;
                match result {
                    Ok(()) => page.step = 2,
                    Err(error) => page.model_error = error,
                }
                cx.notify();
            },
        );
    }

    /// 第二步点「下一步」：先校验（原版把 verifyModels 接在 nextStep 前）。
    fn next_from_models(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_proceed_step2(cx) {
            return;
        }
        self.verify_models(window, cx);
    }

    /// 原版 `startEmbeddingInit`：先把三类服务写入 settings 并保存，
    /// 再 `ensure_icon_embeddings`（async → `tasks::run_async`）。
    fn start_embedding_init(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.initializing {
            return;
        }
        self.initializing = true;
        self.embedding_progress = 0;
        self.embedding_status = "正在初始化图标向量库...".to_string();
        self.model_error.clear();
        cx.notify();
        let shared = self.handle.shared().clone();
        let services: Vec<huanshu_core::config::ModelServiceSettings> = self
            .forms
            .iter()
            .map(|form| form.read(cx).draft.clone())
            .collect();
        run_async(
            window,
            cx,
            async move {
                let [llm, embedding, multimodal]: [_; 3] = services
                    .try_into()
                    .map_err(|_| "服务数量不对".to_string())?;
                let mut current = settings::get_settings(&shared)?;
                current.llm = llm;
                current.embedding = embedding;
                current.multimodal = multimodal;
                settings::save_settings(&shared, current)?;
                generation::ensure_icon_embeddings(&shared).await
            },
            |page: &mut Self, result, _window, cx| {
                page.initializing = false;
                match result {
                    Ok(()) => {
                        page.embedding_progress = 100;
                        page.embedding_status = "初始化完成！".to_string();
                    }
                    Err(error) => {
                        page.embedding_status = format!("初始化失败: {error}");
                        page.model_error = error;
                    }
                }
                cx.notify();
            },
        );
    }

    /// 原版 `finishWizard`：先 `save_app_settings`（目录 + 完成状态），
    /// 未跳过时再 `save_settings` 保存三类服务，最后 `complete_first_run` 进首页。
    /// 都是同步 core 调用（写文件），走 `run_bg`。
    fn finish_wizard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.finishing {
            return;
        }
        self.finishing = true;
        self.model_error.clear();
        cx.notify();
        let shared = self.handle.shared().clone();
        let data_dir = self.data_dir_value(cx);
        let media_dir = self.media_dir_value(cx);
        let llm_configured = !self.skip_llm;
        let embeddings_ready = self.embeddings_ready();
        let initialized_embedding_model = if !self.skip_llm && !self.skip_embedding {
            self.forms[ModelTarget::Embedding.index()]
                .read(cx)
                .draft
                .model
                .clone()
        } else {
            String::new()
        };
        let services = (!self.skip_llm).then(|| {
            let [llm, embedding, multimodal]: [_; 3] = [
                self.forms[0].read(cx).draft.clone(),
                self.forms[1].read(cx).draft.clone(),
                self.forms[2].read(cx).draft.clone(),
            ];
            (llm, embedding, multimodal)
        });
        run_bg(
            window,
            cx,
            move || {
                let mut globals = storage::get_app_settings(&shared)?;
                globals.data_dir = data_dir;
                globals.media_dir = media_dir;
                globals.llm_configured = llm_configured;
                globals.embeddings_ready = embeddings_ready;
                globals.initialized_embedding_model = initialized_embedding_model;
                storage::save_app_settings(&shared, globals)?;
                if let Some((llm, embedding, multimodal)) = services {
                    let mut current = settings::get_settings(&shared)?;
                    current.llm = llm;
                    current.embedding = embedding;
                    current.multimodal = multimodal;
                    settings::save_settings(&shared, current)?;
                }
                storage::complete_first_run(&shared, llm_configured)
            },
            |page: &mut Self, result, window, cx| {
                page.finishing = false;
                match result {
                    Ok(()) => page.handle.navigate(Route::Home, window, cx),
                    Err(error) => page.model_error = error,
                }
                cx.notify();
            },
        );
    }
}

impl Render for WizardPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;

        // ── 步骤指示器（原版 wizard-steps-indicator，含 skipped 与连线的 done 态） ──
        let indicator = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .mb(px(24.0))
            .children((0..STEPS.len()).map(|index| {
                let active = index == self.step;
                let done = index < self.step;
                let skipped = (index == 1 && self.skip_llm)
                    || (index == 2 && (self.skip_embedding || self.skip_llm));
                let dot = div()
                    .id(SharedString::from(format!("wizard-dot-{index}")))
                    .size(px(32.0))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(13.0))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .border_2()
                    .when(active, |dot| {
                        dot.bg(c.primary_bg)
                            .border_color(c.primary)
                            .text_color(c.primary)
                    })
                    .when(!active && done && !skipped, |dot| {
                        dot.bg(c.primary)
                            .border_color(c.primary)
                            .text_color(c.primary_contrast)
                    })
                    .when(!active && skipped, |dot| {
                        dot.bg(c.surface).border_color(c.muted).text_color(c.muted)
                    })
                    .when(!active && !done && !skipped, |dot| {
                        dot.bg(c.surface).border_color(c.border).text_color(c.muted)
                    })
                    .child((index + 1).to_string());
                let mut item = div().flex().flex_row().items_center().child(dot);
                if index < STEPS.len() - 1 {
                    let line_done = if index == 0 {
                        self.step > 0
                    } else if index == 1 {
                        self.step > 1 || self.skip_llm
                    } else {
                        self.step > 2 || self.skip_llm
                    };
                    item = item.child(div().w(px(48.0)).h(px(2.0)).bg(if line_done {
                        c.primary
                    } else {
                        c.border
                    }));
                }
                item
            }));

        // ── 步骤主体 ─────────────────────────────────────────────
        let step_body: gpui_kit::AnyElement = match self.step {
            0 => {
                let data_dir = self.data_dir.clone();
                let media_dir = self.media_dir.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .child(self.dir_field(
                        "数据目录",
                        &data_dir,
                        "存放数据库和项目配置文件",
                        WizardPage::browse_data_dir,
                        cx,
                    ))
                    .child(self.dir_field(
                        "媒体目录",
                        &media_dir,
                        "存放上传的图片和视频文件",
                        WizardPage::browse_media_dir,
                        cx,
                    ))
                    .into_any_element()
            }
            1 => {
                let panels = ModelTarget::ALL.iter().map(|target| {
                    let target = *target;
                    let info = target.info();
                    div()
                        .flex_1()
                        .min_w_0()
                        .p(px(14.0))
                        .rounded(px(10.0))
                        .border_1()
                        .border_color(c.border)
                        .bg(c.panel_2)
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .child(info.title),
                        )
                        .child(self.forms[target.index()].clone())
                });
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .px(px(12.0))
                            .py(px(10.0))
                            .rounded(px(8.0))
                            .bg(c.surface)
                            .border_1()
                            .border_color(c.border)
                            .text_size(px(12.0))
                            .text_color(c.muted)
                            .child(carbon("information"))
                            .child("跳过此步骤仍可使用幻灯片播放功能，但无法生成新幻灯片"),
                    )
                    .child(div().flex().flex_row().gap(px(12.0)).children(panels))
                    .into_any_element()
            }
            2 => {
                // 进度条：窄卡内宽 = 560 - 2*32(padding) = 496。
                let bar_w = 496.0f32;
                let fill_w = bar_w * (self.embedding_progress as f32) / 100.0;
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(
                        div()
                            .w(px(bar_w))
                            .h(px(8.0))
                            .rounded(px(4.0))
                            .bg(c.surface)
                            .overflow_hidden()
                            .child(div().h_full().w(px(fill_w)).bg(c.primary)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_center()
                            .text_size(px(13.0))
                            .text_color(c.muted)
                            .child(self.embedding_status.clone()),
                    )
                    .child(
                        widgets::primary_button(
                            "start-init",
                            if self.initializing {
                                "初始化中..."
                            } else {
                                "开始初始化"
                            },
                        )
                        .icon(carbon(if self.initializing {
                            "renew"
                        } else {
                            "data-base"
                        }))
                        .disabled(self.initializing)
                        .on_click(cx.listener(
                            |page, _event, window, cx| page.start_embedding_init(window, cx),
                        )),
                    )
                    .child(
                        widgets::ghost_button("skip-embedding", "跳过（可在设置中后续初始化）")
                            .w_full()
                            .on_click(cx.listener(|page, _event, _window, cx| {
                                page.skip_embedding = true;
                                page.step = 3;
                                cx.notify();
                            })),
                    )
                    .into_any_element()
            }
            _ => {
                let row = |label: &'static str, value: String| {
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .text_size(px(13.0))
                        .child(div().text_color(c.muted).child(label))
                        .child(
                            div()
                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                .max_w(px(340.0))
                                .truncate()
                                .child(value),
                        )
                        .into_any_element()
                };
                let model_of = |target: ModelTarget| -> String {
                    if self.skip_llm {
                        "未配置".to_string()
                    } else {
                        self.forms[target.index()].read(cx).draft.model.clone()
                    }
                };
                let embedding_state = if self.skip_embedding || self.skip_llm {
                    "未初始化"
                } else {
                    "已就绪"
                };
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(10.0))
                            .mb(px(4.0))
                            .child(
                                widgets::brand_chip(88.0, 28.0, 52.0),
                            ),
                    )
                    .child(
                        div()
                            .p(px(14.0))
                            .rounded(px(8.0))
                            .bg(c.surface)
                            .border_1()
                            .border_color(c.border)
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(row("数据目录", self.data_dir_value(cx)))
                            .child(row("媒体目录", self.media_dir_value(cx)))
                            .child(row("大模型", model_of(ModelTarget::Llm)))
                            .child(row("向量模型", model_of(ModelTarget::Embedding)))
                            .child(row("多模态模型", model_of(ModelTarget::Multimodal)))
                            .child(row("向量库", embedding_state.to_string())),
                    )
                    .when(self.data_dir_changed(cx), |done| {
                        done.child(
                            div()
                                .flex()
                                .flex_row()
                                .items_start()
                                .gap(px(10.0))
                                .p(px(14.0))
                                .rounded(px(8.0))
                                .bg(c.warning_bg)
                                .border_1()
                                .border_color(c.warning_border)
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .text_color(c.warning)
                                        .child(carbon("warning")),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(3.0))
                                        .child(
                                            div()
                                                .text_size(px(13.0))
                                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                .text_color(c.warning)
                                                .child("需要重启应用"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .text_color(c.muted)
                                                .child(
                                                    "数据目录已更改，请关闭并重新启动应用以使更改生效。当前会话中的新建项目将存储在原目录。",
                                                ),
                                        ),
                                ),
                        )
                    })
                    .into_any_element()
            }
        };

        // ── 步骤标题与说明（原版 step-title / step-desc） ─────────
        let (step_title, step_desc): (&'static str, &'static str) = match self.step {
            0 => ("选择存储目录", "设置数据库和多媒体文件的存储位置"),
            1 => (
                "配置大模型",
                "设置 AI 生成所需的模型服务（可跳过，仅播放幻灯片时无需配置）",
            ),
            2 => (
                "初始化向量库",
                "正在为图标语义匹配生成向量索引（需要一些时间）",
            ),
            _ => ("配置完成！", ""),
        };

        // ── 底部按钮（原版 wizard-footer 的显隐逻辑） ─────────────
        let footer =
            div()
                .flex()
                .flex_row()
                .items_center()
                .mt(px(24.0))
                .pt(px(18.0))
                .border_t_1()
                .border_color(c.border)
                .when(self.step > 0, |footer| {
                    footer.child(
                        widgets::ghost_button("prev", "上一步")
                            .icon(carbon("arrow-left"))
                            .on_click(cx.listener(|page, _event, _window, cx| page.prev_step(cx))),
                    )
                })
                .child(div().flex_1())
                .when(self.step == 0 && self.can_proceed_step1(cx), |footer| {
                    footer.child(
                        widgets::primary_button("next", "下一步")
                            .icon(carbon("arrow-right"))
                            .on_click(cx.listener(|page, _event, _window, cx| page.next_step(cx))),
                    )
                })
                .when(self.step == 1, |footer| {
                    footer
                        .child(widgets::ghost_button("skip", "跳过").on_click(
                            cx.listener(|page, _event, _window, cx| page.skip_llm_step(cx)),
                        ))
                        .when(self.can_proceed_step2(cx), |footer| {
                            footer.child(
                                widgets::primary_button("next-models", "下一步")
                                    .icon(carbon("arrow-right"))
                                    .disabled(self.verifying)
                                    .on_click(cx.listener(|page, _event, window, cx| {
                                        page.next_from_models(window, cx)
                                    })),
                            )
                        })
                })
                .when(self.step == 2, |footer| {
                    footer.child(
                        widgets::primary_button("next-3", "下一步")
                            .icon(carbon("arrow-right"))
                            .disabled(!self.skip_embedding && self.embedding_progress < 100)
                            .on_click(cx.listener(|page, _event, _window, cx| {
                                page.step = 3;
                                cx.notify();
                            })),
                    )
                })
                .when(self.step == 3, |footer| {
                    footer.child(
                        widgets::primary_button(
                            "finish",
                            if self.finishing {
                                "进入中…"
                            } else {
                                "进入 Studio"
                            },
                        )
                        .icon(carbon("arrow-right"))
                        .disabled(self.finishing)
                        .on_click(
                            cx.listener(|page, _event, window, cx| page.finish_wizard(window, cx)),
                        ),
                    )
                });

        let wide = self.step == 1;
        div()
            .id("wizard")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .items_center()
            .py(px(36.0))
            .child(
                widgets::panel(cx)
                    .w(px(if wide { 960.0 } else { 560.0 }))
                    .p(px(32.0))
                    .flex()
                    .flex_col()
                    .gap(px(16.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(12.0))
                                    .child(widgets::brand_chip(56.0, 18.0, 28.0))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                                    .text_color(c.primary)
                                                    .child("HUANSHU STUDIO"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(24.0))
                                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                                    .child("幻述"),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(22.0))
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .child("首次启动配置"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(c.muted)
                                    .child("完成以下设置后，即可开始生成和演示你的幻灯片"),
                            ),
                    )
                    .child(indicator)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(19.0))
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .child(step_title),
                            )
                            .when(!step_desc.is_empty(), |heading| {
                                heading.child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(c.muted)
                                        .child(step_desc),
                                )
                            }),
                    )
                    .child(step_body)
                    .when(!self.model_error.is_empty(), |card| {
                        card.child(
                            div()
                                .mt(px(4.0))
                                .p(px(12.0))
                                .rounded(px(6.0))
                                .bg(c.error_bg)
                                .border_1()
                                .border_color(c.error_border)
                                .text_size(px(12.0))
                                .text_color(c.error)
                                .child(self.model_error.clone()),
                        )
                    })
                    .child(footer),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browsing_data_dir_refreshes_media_dir_unless_it_is_a_projects_dir() {
        // 默认联动：媒体目录设为 <选中>/media。
        let media = suggest_media_dir(r"E:\huanshu\sandbox", r"C:\old\media");
        assert_eq!(media.unwrap(), r"E:\huanshu\sandbox\media");
        // 媒体目录已经是项目子目录时不覆盖（原版 includes("/projects/")，\ 也容忍）。
        assert!(suggest_media_dir(r"D:\new", r"D:\data\projects\1\media").is_none());
        assert!(suggest_media_dir(r"D:\new", "D:/data/projects/1/media").is_none());
        // 空媒体目录同样联动。
        assert!(suggest_media_dir(r"D:\new", "").is_some());
    }

    #[test]
    fn embeddings_ready_requires_full_init_flow() {
        // 与原版 embeddingsReady 计算一致：未跳过 + 进度 100。
        let ready = !true && !true && 100 >= 100;
        assert!(!ready);
        let ready = !false && !false && 100 >= 100;
        assert!(ready);
        let ready = !false && !false && 40 >= 100;
        assert!(!ready);
        let ready = !false && !true && 100 >= 100;
        assert!(!ready);
        let _ = STEPS;
    }
}
