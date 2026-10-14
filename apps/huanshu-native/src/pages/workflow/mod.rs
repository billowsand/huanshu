//! 项目工作流（原版 `WorkflowView.vue`）：左侧边栏 + 三步内容。
//!
//! 数据放在一个共享实体 [`ProjectModel`] 里，三步页面都持有它、`cx.observe` 它：
//! Step1 改文稿、Step2 生成、Step3 改蓝图，都改这一个模型，侧栏与其他步骤自动更新
//! （原版是 Pinia 的 `generation` / `projects` store 加一堆 props）。
//!
//! 已实现：加载 / 加载失败、侧栏（返回、项目名、三步状态与说明、主题圆点、设置）、
//! 按原版规则选初始步骤与判断能否点击、步骤切换时交还位图。
//! 待实现：项目改名、新建项目时的草稿保存、生成状态（见 step2）。

pub mod step1;
pub mod step2;
pub mod step3;

use gpui_kit::prelude::*;
use gpui_kit::{div, px, Context, Entity, Render, SharedString, Subscription, Window};
use huanshu_core::db::GenerationRun;
use huanshu_core::markdown_tools::{self, Granularity};
use huanshu_core::services::media::MediaAssetResponse;
use huanshu_core::services::{generation, projects};
use huanshu_core::types::AspectRatio;

use crate::icons::carbon;
use crate::router::{Route, SettingsTab, Step};
use crate::shell::ShellHandle;
use crate::tasks::run_bg;
use crate::theme::StudioExt as _;
use crate::widgets;

/// 一个项目在工作流里的全部数据。三步页面共享它。
pub struct ProjectModel {
    /// `None` = 新建、尚未落库（原版 `/new`）。
    pub id: Option<i64>,
    pub name: String,
    pub md_content: String,
    /// 上传的文稿文件名（原版 `step1State.mdFilename`，顶栏标签与侧栏说明会显示）。
    pub md_filename: String,
    /// 分页粒度（原版 `step1State.granularity`）。
    pub granularity: Granularity,
    /// 素材库（原版 `mediaLib.mediaItems`，`services::media` 的记录）。
    pub media: Vec<MediaAssetResponse>,
    /// 草稿自动保存状态（原版 `draftSaveState`；防抖保存逻辑在 C 节，这里只存状态与展示）。
    pub draft_save_state: DraftSaveState,
    /// 草稿保存失败原因（原版 `draftSaveError`，格式工具行内错误会显示）。
    pub draft_save_error: String,
    /// 牌组按**原始 JSON** 保存（与 core 的导出、预览一致；强类型往返会挡掉能渲染的牌组）。
    pub deck: Vec<serde_json::Value>,
    pub aspect: AspectRatio,
    pub latest_run: Option<GenerationRun>,
    /// 生成是否在进行（原版 `gen.running`）。待 step2 接上生成流水线后由事件驱动。
    pub generating: bool,
    /// 最近一次生成失败（原版 `gen.stage === 'error'`）。
    pub generation_failed: bool,
}

/// 草稿自动保存状态（原版 `'idle' | 'saving' | 'saved' | 'error'`）。
/// Saving / Saved / Error 由 C 节的自动保存逻辑置位，本轮只有展示。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum DraftSaveState {
    #[default]
    Idle,
    Saving,
    Saved,
    Error,
}

impl DraftSaveState {
    /// 原版 `draftStatusText`。
    pub fn text(self) -> &'static str {
        match self {
            Self::Idle => "",
            Self::Saving => "草稿保存中...",
            Self::Saved => "草稿已保存",
            Self::Error => "草稿保存失败",
        }
    }
}

impl ProjectModel {
    /// 页面高宽比（16:9 → 0.5625）。
    pub fn aspect_ratio(&self) -> f32 {
        self.aspect.height() as f32 / self.aspect.width() as f32
    }

    /// 文稿长度（原版 `mdContent.length`，JS 的 UTF-16 码元数；中文每字 1，与原版一致）。
    pub fn md_len(&self) -> usize {
        self.md_content.encode_utf16().count()
    }

    /// 原版 `detectedGranularityLocal`：自动推荐解析成 H2 / H3。
    pub fn detected_granularity(&self) -> Granularity {
        markdown_tools::detect_granularity(&self.md_content)
    }

    /// 原版 `estimatedSlideCount`。
    pub fn estimated_slides(&self) -> usize {
        markdown_tools::estimate_slides(&self.md_content, self.granularity)
    }

    /// 「开始 AI 生成」本地可用条件（原版 `canGenerate` 的名称、文稿部分；
    /// `!gen.running && retryingPages.size === 0` 待 C 节接上生成状态后补充）。
    pub fn can_generate_local(&self) -> bool {
        !self.md_content.trim().is_empty() && !self.name.trim().is_empty()
    }
}

enum Load {
    Loading,
    Failed(String),
    Ready,
}

enum StepView {
    Prepare(Entity<step1::Step1>),
    Generate(Entity<step2::Step2>),
    Edit(Entity<step3::Step3>),
}

pub struct WorkflowPage {
    handle: ShellHandle,
    model: Entity<ProjectModel>,
    load: Load,
    step: Step,
    view: Option<StepView>,
    requested_step: Option<Step>,
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,
}

impl WorkflowPage {
    pub fn new(
        handle: ShellHandle,
        id: Option<i64>,
        step: Option<Step>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let model = cx.new(|_| ProjectModel {
            id,
            name: String::new(),
            md_content: String::new(),
            md_filename: String::new(),
            granularity: Granularity::Auto,
            media: Vec::new(),
            draft_save_state: DraftSaveState::Idle,
            draft_save_error: String::new(),
            deck: Vec::new(),
            aspect: AspectRatio::Ratio16x9,
            latest_run: None,
            generating: false,
            generation_failed: false,
        });
        let subscriptions = vec![cx.observe(&model, |_page, _model, cx| cx.notify())];
        let mut page = Self {
            handle,
            model,
            load: Load::Loading,
            step: Step::Prepare,
            view: None,
            requested_step: step,
            subscriptions,
        };
        match id {
            Some(id) => page.load_project(id, window, cx),
            None => {
                page.load = Load::Ready;
                page.show_step(Step::Prepare, window, cx);
            }
        }
        page
    }

    fn load_project(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let shared = self.handle.shared().clone();
        run_bg(
            window,
            cx,
            move || {
                let project = projects::open_project(&shared, id)?;
                let run = generation::get_latest_generation_run(&shared, id)?;
                let media = huanshu_core::services::media::get_project_media(&shared, id)?;
                Ok::<_, String>((project, run, media))
            },
            |page: &mut Self, result, window, cx| {
                match result {
                    Err(error) => page.load = Load::Failed(error),
                    Ok((project, run, media)) => {
                        let deck: Vec<serde_json::Value> =
                            if project.blueprints_json.trim().is_empty() {
                                Vec::new()
                            } else {
                                serde_json::from_str(&project.blueprints_json).unwrap_or_default()
                            };
                        let generating = run.as_ref().is_some_and(|run| run.status == "running");
                        let failed = run.as_ref().is_some_and(|run| run.status == "error");
                        page.model.update(cx, |model, cx| {
                            model.name = project.name;
                            model.md_content = project.md_content;
                            model.aspect = project.aspect_ratio.unwrap_or(AspectRatio::Ratio16x9);
                            model.media = media;
                            model.deck = deck;
                            model.latest_run = run;
                            model.generating = generating;
                            model.generation_failed = failed;
                            cx.notify();
                        });
                        // TODO(parity): C 节从「项目附加信息」读回 mdFilename / granularity
                        // （原版 localStorage 的 `persistStep1Meta` / `loadStep1Meta`）。
                        page.load = Load::Ready;
                        // 原版：生成进行中 → 2，有蓝图 → 3，否则 → 1。
                        let model = page.model.read(cx);
                        let auto = if model.generating {
                            Step::Generate
                        } else if !model.deck.is_empty() {
                            Step::Edit
                        } else {
                            Step::Prepare
                        };
                        let step = page.requested_step.take().unwrap_or(auto);
                        page.show_step(step, window, cx);
                    }
                }
                cx.notify();
            },
        );
    }

    /// 侧栏一步能否点（原版 `steps[].clickable`）。
    fn clickable(&self, step: Step, cx: &Context<Self>) -> bool {
        let model = self.model.read(cx);
        match step {
            Step::Prepare => true,
            // 原版：只有已经走到第 2 步及以后才能点回第 2 步。
            Step::Generate => self.step >= Step::Generate,
            // TODO(parity): 原版还要求没有正在重试的页（`retryingPages.size === 0`）。
            Step::Edit => !model.deck.is_empty(),
        }
    }

    pub fn show_step(&mut self, step: Step, window: &mut Window, cx: &mut Context<Self>) {
        self.release(cx);
        let (model, handle) = (self.model.clone(), self.handle.clone());
        let page = cx.weak_entity();
        self.view = Some(match step {
            Step::Prepare => {
                StepView::Prepare(cx.new(|cx| step1::Step1::new(model, handle, window, cx)))
            }
            Step::Generate => {
                StepView::Generate(cx.new(|cx| step2::Step2::new(model, handle, page, window, cx)))
            }
            Step::Edit => StepView::Edit(cx.new(|cx| step3::Step3::new(model, handle, window, cx))),
        });
        self.step = step;
        cx.notify();
    }

    /// 离开工作流或换步骤：交还当前步骤手上的位图。
    pub fn release(&mut self, cx: &mut Context<Self>) {
        match self.view.take() {
            Some(StepView::Generate(view)) => view.update(cx, |view, cx| view.release(cx)),
            Some(StepView::Edit(view)) => view.update(cx, |view, cx| view.release(cx)),
            Some(StepView::Prepare(_)) | None => {}
        }
    }
}

impl Render for WorkflowPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        match &self.load {
            Load::Loading => {
                return div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(c.muted)
                    .child("加载项目中...")
                    .into_any_element();
            }
            Load::Failed(error) => {
                let handle = self.handle.clone();
                return div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .items_center()
                    .justify_center()
                    .child(div().text_color(c.error).child(error.clone()))
                    .child(widgets::ghost_button("home", "返回首页").on_click(
                        move |_event, window, cx| handle.navigate(Route::Home, window, cx),
                    ))
                    .into_any_element();
            }
            Load::Ready => {}
        }

        let model = self.model.read(cx);
        let slide_count = model.deck.len();
        let name = if model.name.trim().is_empty() {
            "未命名项目".to_string()
        } else {
            model.name.clone()
        };
        // 原版 `steps` 的说明文案。
        let descs = [
            if !model.md_filename.is_empty() {
                model.md_filename.clone()
            } else if !model.md_content.is_empty() {
                format!("{} 字符", model.md_len())
            } else {
                "上传或粘贴文稿".to_string()
            },
            if model.generation_failed {
                "生成出错".to_string()
            } else if model.generating {
                "生成中…".to_string()
            } else if slide_count > 0 {
                format!("已生成 {slide_count} 张")
            } else {
                "等待开始".to_string()
            },
            if slide_count > 0 {
                format!("{slide_count} 张幻灯片")
            } else {
                "生成后可编辑".to_string()
            },
        ];
        let generation_failed = model.generation_failed;
        let theme_id = cx.studio().theme.id;

        let steps = [
            (Step::Prepare, "准备素材"),
            (Step::Generate, "AI 生成"),
            (Step::Edit, "编辑完善"),
        ];
        let nav = div().flex().flex_col().px(px(14.0)).pt(px(24.0)).children(
            steps.iter().enumerate().map(|(i, (step, label))| {
                let step = *step;
                let status_error = step == Step::Generate && generation_failed;
                let done = step < self.step;
                let active = step == self.step;
                let clickable = self.clickable(step, cx);
                let circle = div()
                    .size(px(40.0))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_2()
                    .border_color(if status_error {
                        c.error
                    } else if active || done {
                        c.primary
                    } else {
                        c.border
                    })
                    .bg(if active { c.primary_bg } else { c.panel })
                    .text_color(if status_error {
                        c.error
                    } else if active || done {
                        c.primary
                    } else {
                        c.muted
                    })
                    .text_size(px(14.0))
                    .child(if status_error {
                        carbon("warning").into_any_element()
                    } else if done {
                        carbon("checkmark").into_any_element()
                    } else {
                        step.number().to_string().into_any_element()
                    });
                div()
                    .id(SharedString::from(format!("step-{}", step.number())))
                    .flex()
                    .flex_row()
                    .gap(px(14.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded(px(8.0))
                    .when(active, |item| item.bg(c.surface))
                    .when(!clickable, |item| item.opacity(0.5))
                    .when(clickable, |item| {
                        item.cursor_pointer().on_click(cx.listener(
                            move |page, _event, window, cx| {
                                if page.step != step {
                                    page.show_step(step, window, cx)
                                }
                            },
                        ))
                    })
                    .child(div().flex().flex_col().items_center().child(circle).when(
                        i < 2,
                        |column| {
                            column.child(div().w(px(2.0)).h(px(40.0)).bg(if done {
                                c.primary
                            } else {
                                c.border
                            }))
                        },
                    ))
                    .child(
                        div()
                            .pt(px(8.0))
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(15.0))
                                    .text_color(if active { c.text } else { c.muted })
                                    .child(*label),
                            )
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .text_color(c.muted)
                                    .child(descs[i].clone()),
                            ),
                    )
            }),
        );

        let (back, settings, theme) = (
            self.handle.clone(),
            self.handle.clone(),
            self.handle.clone(),
        );
        let sidebar = div()
            .flex_none()
            .w(px(204.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(c.panel)
            .border_r_1()
            .border_color(c.border)
            .child(
                div()
                    .h(px(64.0))
                    .px(px(14.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .border_b_1()
                    .border_color(c.border)
                    .child(
                        widgets::icon_button("back", "arrow-left", "返回首页").on_click(
                            move |_event, window, cx| back.navigate(Route::Home, window, cx),
                        ),
                    )
                    // TODO(parity): 点击项目名进入改名（回车保存、Esc 取消、失焦保存）。
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(15.0))
                            .child(name),
                    ),
            )
            .child(nav)
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .p(px(14.0))
                    .border_t_1()
                    .border_color(c.border)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(widgets::section_label("主题", cx))
                            .child(widgets::theme_dots(
                                "workflow-theme",
                                theme_id,
                                move |id, window, cx| theme.set_theme(id, true, window, cx),
                                cx,
                            )),
                    )
                    .child(
                        widgets::ghost_button("settings", "设置")
                            .icon(carbon("settings"))
                            .on_click(move |_event, window, cx| {
                                settings.navigate(
                                    Route::Settings(SettingsTab::default()),
                                    window,
                                    cx,
                                )
                            }),
                    ),
            );

        let content = match &self.view {
            Some(StepView::Prepare(view)) => view.clone().into_any_element(),
            Some(StepView::Generate(view)) => view.clone().into_any_element(),
            Some(StepView::Edit(view)) => view.clone().into_any_element(),
            None => div().into_any_element(),
        };
        div()
            .size_full()
            .flex()
            .flex_row()
            .child(sidebar)
            .child(div().flex_1().min_w_0().h_full().child(content))
            .into_any_element()
    }
}
