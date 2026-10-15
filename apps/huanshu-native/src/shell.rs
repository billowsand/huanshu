//! 应用外壳（原版 `App.vue` + 路由）：标题栏 + 当前页面 + 演示覆盖层。
//!
//! 页面之间不直接持有彼此；它们拿一个 [`ShellHandle`]，经它换页、换主题、打开演示。
//! 换页时外壳先调用旧页面的 `release`，把它手上的位图交还 GPUI（图集不会自动回收）。

use gpui_kit::prelude::*;
use gpui_kit::{div, px, App, Context, Entity, Render, WeakEntity, Window};
use huanshu_core::services::{settings, storage};

use crate::app::AppShared;
use crate::generation::GenerationStore;
use crate::pages::home::HomePage;
use crate::pages::settings::SettingsPage;
use crate::pages::wizard::WizardPage;
use crate::pages::workflow::WorkflowPage;
use crate::present::{PresentArgs, Presentation};
use crate::router::Route;
use crate::theme::{self, StudioExt as _};
use crate::widgets;

/// 当前页面。每个变体都是一个实体；换页就是换掉它。
enum Page {
    Home(Entity<HomePage>),
    Settings(Entity<SettingsPage>),
    Wizard(Entity<WizardPage>),
    Workflow(Entity<WorkflowPage>),
}

pub struct Shell {
    shared: AppShared,
    route: Route,
    page: Page,
    presentation: Option<Entity<Presentation>>,
    /// 应用级生成状态（原版 pinia 的 `generation` store）：外壳创建、比任何页面活得久。
    generation: Entity<GenerationStore>,
    /// 进设置之前的那条路由（原版 `window.history.state.back`）。
    ///
    /// 设置页的「返回工作台」按原版 `goBack()` 走 `router.back()`：从工作流侧栏进来就回
    /// **那个项目的那个步骤**（`WorkflowPage::show_step` 会把步骤同步回路由），直接
    /// `--route settings` 启动这类没有来源的情况才回首页。
    settings_origin: Option<Route>,
    first_frame_logged: bool,
}

/// 页面手里的外壳句柄：可以随意克隆、存进页面里。
#[derive(Clone)]
pub struct ShellHandle {
    shell: WeakEntity<Shell>,
    shared: AppShared,
    generation: Entity<GenerationStore>,
}

impl ShellHandle {
    pub fn shared(&self) -> &AppShared {
        &self.shared
    }

    /// 应用级生成状态（原版 `useGenerationStore()`）；页面观察它即可，不要自己存一份。
    pub fn generation(&self) -> Entity<GenerationStore> {
        self.generation.clone()
    }

    /// 换页（原版 `router.push`）。
    pub fn navigate(&self, route: Route, window: &mut Window, cx: &mut App) {
        let _ = self
            .shell
            .update(cx, |shell, cx| shell.navigate(route, window, cx));
    }

    /// 只改路由、不重建页面（原版 `router.replace`）。
    ///
    /// 工作流第一步的自动保存把新建项目落库后用它对准路由：`navigate` 会重建
    /// `WorkflowPage`，Step1 的编辑器实体（连同撤销历史）会整个丢掉，所以不能用。
    /// 工作流换步骤也用它（`pages::workflow::WorkflowPage::show_step`），
    /// 这样「从工作流进设置再返回」能回到原来那一步。
    pub fn retarget_route(&self, route: Route, cx: &mut App) {
        let _ = self
            .shell
            .update(cx, |shell, cx| shell.retarget_route(route, cx));
    }

    /// 从设置页返回来源页（原版 `goBack()`：`history.state.back ? router.back() : push('/')`）。
    ///
    /// 来源是进设置之前的那条路由，可能是「某个项目的某一步」；没有来源（直接启动进设置）时回首页。
    pub fn leave_settings(&self, window: &mut Window, cx: &mut App) {
        let _ = self.shell.update(cx, |shell, cx| {
            let route = shell.settings_origin.take().unwrap_or(Route::Home);
            shell.navigate(route, window, cx);
        });
    }

    /// 切换主题。`save` 为真时写入设置（`app_settings.app_theme`）；设置页的「点击预览」传假。
    pub fn set_theme(&self, theme_id: &str, save: bool, window: &mut Window, cx: &mut App) {
        if save {
            if let Err(error) = settings::set_app_theme(&self.shared, theme_id) {
                eprintln!("[native] 保存主题失败：{error}");
            }
        }
        let font = interface_font(&self.shared);
        theme::apply(theme_id, font.as_deref(), Some(window), cx);
        cx.refresh_windows();
    }

    /// 打开演示覆盖层（原版 `PresentationOverlay.vue`）。
    pub fn present(&self, args: PresentArgs, window: &mut Window, cx: &mut App) {
        let _ = self
            .shell
            .update(cx, |shell, cx| shell.open_presentation(args, window, cx));
    }

    /// 关闭演示覆盖层。
    pub fn close_presentation(&self, window: &mut Window, cx: &mut App) {
        let _ = self
            .shell
            .update(cx, |shell, cx| shell.close_presentation(window, cx));
    }
}

/// 「设置 → 外观与字体」的界面字体。
pub fn interface_font(shared: &AppShared) -> Option<String> {
    let state = shared.lock().ok()?;
    let font = state.app_settings.fonts.interface.clone();
    (!font.trim().is_empty()).then_some(font)
}

impl Shell {
    pub fn new(
        shared: AppShared,
        start: Option<Route>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // 原版 App.vue：启动时 `is_first_run` 为真就进首次向导。
        let first_run = storage::is_first_run(&shared).unwrap_or(false);
        let route = match (first_run, start) {
            (true, _) => Route::Wizard,
            (false, Some(route)) => route,
            (false, None) => Route::Home,
        };
        // 应用级生成状态先建：它经 ShellHandle 发给页面，页面换页时不重建。
        let generation = {
            let shared = shared.clone();
            cx.new(|cx| GenerationStore::new(shared, cx))
        };
        let handle = ShellHandle {
            shell: cx.weak_entity(),
            shared: shared.clone(),
            generation: generation.clone(),
        };
        let page = build_page(&route, handle, window, cx);
        Self {
            shared,
            route,
            page,
            presentation: None,
            generation,
            // 直接以设置页启动（`--route settings`）时没有来源，返回走首页。
            settings_origin: None,
            first_frame_logged: false,
        }
    }

    pub fn handle(&self, cx: &Context<Self>) -> ShellHandle {
        ShellHandle {
            shell: cx.weak_entity(),
            shared: self.shared.clone(),
            generation: self.generation.clone(),
        }
    }

    pub fn navigate(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        // 进设置前先记住来源：设置页的「返回」要回这一条（原版 `router.back()`）。
        // 设置内部换分区（route 还是 Settings）不算新来源，别把它自己记成来源。
        if matches!(route, Route::Settings(_)) && !matches!(self.route, Route::Settings(_)) {
            self.settings_origin = Some(self.route.clone());
        }
        self.close_presentation(window, cx);
        self.release_page(cx);
        let handle = self.handle(cx);
        self.page = build_page(&route, handle, window, cx);
        self.route = route;
        cx.notify();
    }

    /// 只更新路由字段、不动当前页面（[`ShellHandle::retarget_route`] 的实现）。
    fn retarget_route(&mut self, route: Route, cx: &mut Context<Self>) {
        // 值没变就不要 notify：`show_step` 可能在 render 里被调到，无谓的通知会反复重绘。
        if self.route != route {
            self.route = route;
            cx.notify();
        }
    }

    fn release_page(&mut self, cx: &mut Context<Self>) {
        match &self.page {
            Page::Workflow(page) => page.update(cx, |page, cx| page.release(cx)),
            Page::Home(_) | Page::Settings(_) | Page::Wizard(_) => {}
        }
    }

    pub fn open_presentation(
        &mut self,
        args: PresentArgs,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_presentation(window, cx);
        let handle = self.handle(cx);
        let presentation = cx.new(|cx| Presentation::new(args, handle, window, cx));
        presentation.update(cx, |presentation, cx| presentation.focus(window, cx));
        self.presentation = Some(presentation);
        cx.notify();
    }

    pub fn close_presentation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(presentation) = self.presentation.take() {
            presentation.update(cx, |presentation, cx| presentation.release(cx));
            if window.is_fullscreen() {
                window.toggle_fullscreen();
            }
            cx.notify();
        }
    }
}

fn build_page(
    route: &Route,
    handle: ShellHandle,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> Page {
    match route {
        Route::Home => Page::Home(cx.new(|cx| HomePage::new(handle, window, cx))),
        Route::Settings(tab) => {
            let tab = *tab;
            Page::Settings(cx.new(|cx| SettingsPage::new(handle, tab, window, cx)))
        }
        Route::Wizard => Page::Wizard(cx.new(|cx| WizardPage::new(handle, window, cx))),
        Route::Project { id, step } => {
            let (id, step) = (*id, *step);
            Page::Workflow(cx.new(|cx| WorkflowPage::new(handle, id, step, window, cx)))
        }
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.first_frame_logged {
            self.first_frame_logged = true;
            crate::note_first_frame();
        }
        let studio = cx.studio().clone();
        let content = match &self.page {
            Page::Home(page) => page.clone().into_any_element(),
            Page::Settings(page) => page.clone().into_any_element(),
            Page::Wizard(page) => page.clone().into_any_element(),
            Page::Workflow(page) => page.clone().into_any_element(),
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(studio.colors.bg)
            .text_color(studio.colors.text)
            .font_family(studio.interface_font.clone())
            .text_size(px(14.0))
            // 全屏演示时没有标题栏：原版是演示元素自己进全屏（`requestFullscreen`），盖住整个屏幕。
            .when(
                !(self.presentation.is_some() && window.is_fullscreen()),
                |shell| {
                    // 打开文稿时标题栏正中是文稿名（点它改名），其余页面不显示。
                    let center = match &self.page {
                        Page::Workflow(page) => Some(WorkflowPage::title_element(page, cx)),
                        _ => None,
                    };
                    shell.child(widgets::title_bar(center, window, cx))
                },
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    // 便签主题：木纹桌面铺在所有页面底下，页面里不画底色的地方都露出它
                    // （原版 `.home-layout, .workflow-root { background: var(--notes-grain), … }`）。
                    .when_some(studio.paper, |area, paper| {
                        area.child(widgets::wood_grain(&paper))
                    })
                    .child(content)
                    .when_some(self.presentation.clone(), |element, presentation| {
                        element.child(div().absolute().inset_0().child(presentation))
                    }),
            )
    }
}
