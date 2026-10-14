//! 首次启动向导（原版 `FirstRunWizard.vue`）——**骨架**。
//!
//! 已实现：品牌头、四步步骤条、翻页（上一步 / 下一步 / 跳过）、第一步的目录展示、
//! 最后一步「进入 Studio」（`complete_first_run` 后进入首页）。
//! 待实现见各步的黄色虚线框。
//!
//! **注意**：目录默认值必须来自 core（`AppState` 当前的 `data_dir`，它尊重 `HUANSHU_HOME`）。
//! 原版在前端自己拼 `localDataDir()/auto-slidev-studio`，沙箱里会指回真实目录——不要照抄。

use gpui_kit::component::Disableable as _;
use gpui_kit::prelude::*;
use gpui_kit::{div, px, Context, Render, SharedString, Window};
use huanshu_core::services::storage;

use crate::router::Route;
use crate::shell::ShellHandle;
use crate::tasks::run_bg;
use crate::theme::StudioExt as _;
use crate::widgets;

const STEPS: [&str; 4] = ["选择存储目录", "配置大模型", "初始化向量库", "配置完成！"];

pub struct WizardPage {
    handle: ShellHandle,
    /// 0..=3。
    step: usize,
    /// 第二步选择了「跳过」（之后跳过向量库一步，原版同样如此）。
    skip_llm: bool,
    finishing: bool,
    error: Option<String>,
}

impl WizardPage {
    pub fn new(handle: ShellHandle, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            handle,
            step: 0,
            skip_llm: false,
            finishing: false,
            error: None,
        }
    }

    fn next(&mut self, cx: &mut Context<Self>) {
        self.step = (self.step + 1).min(STEPS.len() - 1);
        cx.notify();
    }

    fn prev(&mut self, cx: &mut Context<Self>) {
        // 跳过了大模型的话，从完成页退回大模型那一步（原版行为）。
        self.step = if self.step == 3 && self.skip_llm {
            1
        } else {
            self.step.saturating_sub(1)
        };
        cx.notify();
    }

    fn skip(&mut self, cx: &mut Context<Self>) {
        self.skip_llm = true;
        self.step = 3;
        cx.notify();
    }

    fn finish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // TODO(parity): 原版先 `save_app_settings`（用户改过的目录），再 `complete_first_run`；
        // 目录改了要提示「需要重启应用」。骨架只做后一步。
        self.finishing = true;
        cx.notify();
        let shared = self.handle.shared().clone();
        let llm_configured = !self.skip_llm;
        run_bg(
            window,
            cx,
            move || storage::complete_first_run(&shared, llm_configured),
            |page: &mut Self, result, window, cx| {
                page.finishing = false;
                match result {
                    Ok(()) => page.handle.navigate(Route::Home, window, cx),
                    Err(error) => page.error = Some(error),
                }
                cx.notify();
            },
        );
    }
}

impl Render for WizardPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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

        let indicator = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(10.0))
            .children((0..STEPS.len()).map(|index| {
                let done = index < self.step;
                let active = index == self.step;
                div()
                    .size(px(26.0))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.0))
                    .border_1()
                    .border_color(if active || done { c.primary } else { c.border })
                    .bg(if active {
                        c.primary
                    } else if done {
                        c.primary_bg
                    } else {
                        c.surface
                    })
                    .text_color(if active {
                        c.primary_contrast
                    } else if done {
                        c.primary
                    } else {
                        c.muted
                    })
                    .child((index + 1).to_string())
            }));

        let field = |label: &'static str, value: String, hint: &'static str| {
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(div().text_size(px(13.0)).child(label))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(8.0))
                        .child(
                            div()
                                .flex_1()
                                .px(px(10.0))
                                .py(px(8.0))
                                .rounded(px(6.0))
                                .bg(c.surface)
                                .border_1()
                                .border_color(c.border)
                                .text_size(px(13.0))
                                .child(value),
                        )
                        .child(
                            widgets::ghost_button(
                                SharedString::from(format!("browse-{label}")),
                                "浏览...",
                            )
                            .disabled(true),
                        ),
                )
                .child(div().text_size(px(12.0)).text_color(c.muted).child(hint))
        };

        let body: gpui_kit::AnyElement = match self.step {
            0 => div()
                .flex()
                .flex_col()
                .gap(px(16.0))
                .child(field("数据目录", data_dir.clone(), "存放数据库和项目配置文件"))
                .child(field("媒体目录", media_dir.clone(), "存放上传的图片和视频文件"))
                .child(widgets::todo_block(
                    "选择存储目录",
                    "FirstRunWizard.vue 第一步",
                    &["「浏览...」选目录（rfd::FileDialog::pick_folder）", "改了目录后完成时保存并提示需要重启"],
                    cx,
                ))
                .into_any_element(),
            1 => div()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(c.muted)
                        .child("跳过此步骤仍可使用幻灯片播放功能，但无法生成新幻灯片"),
                )
                .child(widgets::todo_block(
                    "配置大模型",
                    "FirstRunWizard.vue 第二步",
                    &[
                        "大语言模型 / 向量模型 / 多模态模型，各自 API 地址、API Key（本地留空）、模型",
                        "与设置页「模型服务」共用一套表单组件（先做设置页，再在这里复用）",
                        "下一步前校验并保存（services::settings::save_settings），错误显示在表单下方",
                    ],
                    cx,
                ))
                .into_any_element(),
            2 => widgets::todo_block(
                "初始化向量库",
                "FirstRunWizard.vue 第三步",
                &["状态文案 + 「开始初始化」（services::generation::ensure_icon_embeddings，async）"],
                cx,
            )
            .into_any_element(),
            _ => {
                let row = |label: &'static str, value: String| {
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .text_size(px(13.0))
                        .child(div().text_color(c.muted).child(label))
                        .child(div().child(value))
                };
                let llm = if self.skip_llm { "未配置" } else { "已配置" };
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .p(px(16.0))
                    .rounded(px(8.0))
                    .bg(c.surface)
                    .child(row("数据目录", data_dir))
                    .child(row("媒体目录", media_dir))
                    .child(row("大模型", llm.to_string()))
                    .child(row("向量模型", llm.to_string()))
                    .child(row("多模态模型", llm.to_string()))
                    .child(row("向量库", "未初始化".to_string()))
                    .into_any_element()
            }
        };

        let actions = div()
            .flex()
            .flex_row()
            .gap(px(10.0))
            .when(self.step > 0, |row| {
                row.child(
                    widgets::ghost_button("prev", "上一步")
                        .on_click(cx.listener(|page, _event, _window, cx| page.prev(cx))),
                )
            })
            .child(div().flex_1())
            .when(self.step == 1, |row| {
                row.child(
                    widgets::ghost_button("skip", "跳过")
                        .on_click(cx.listener(|page, _event, _window, cx| page.skip(cx))),
                )
            })
            .child(if self.step < STEPS.len() - 1 {
                widgets::primary_button("next", "下一步")
                    .on_click(cx.listener(|page, _event, _window, cx| page.next(cx)))
            } else {
                widgets::primary_button(
                    "finish",
                    if self.finishing {
                        "进入中…"
                    } else {
                        "进入 Studio"
                    },
                )
                .disabled(self.finishing)
                .on_click(cx.listener(|page, _event, window, cx| page.finish(window, cx)))
            });

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
                    .w(px(620.0))
                    .p(px(32.0))
                    .flex()
                    .flex_col()
                    .gap(px(22.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(8.0))
                            .child(widgets::brand_mark(52.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(c.primary)
                                    .child("HUANSHU STUDIO"),
                            )
                            .child(
                                div()
                                    .text_size(px(22.0))
                                    .font_weight(gpui_kit::FontWeight::BOLD)
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
                        div().flex().flex_col().gap(px(4.0)).child(
                            div()
                                .text_size(px(17.0))
                                .font_weight(gpui_kit::FontWeight::BOLD)
                                .child(STEPS[self.step]),
                        ),
                    )
                    .child(body)
                    .when_some(self.error.clone(), |card, error| {
                        card.child(div().text_size(px(12.0)).text_color(c.error).child(error))
                    })
                    .child(actions),
            )
    }
}
