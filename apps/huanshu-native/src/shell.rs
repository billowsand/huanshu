//! 外壳：无边框窗口 + 自绘标题栏 + 首页，并在首页与看板之间切换。
//!
//! 标题栏显示数据目录——隔离目录（`HUANSHU_HOME`）时一眼能看出来自己没在动真实项目。

use gpui_kit::component::button::Button;
use gpui_kit::component::{ActiveTheme as _, Theme, ThemeMode};
use gpui_kit::prelude::*;
use gpui_kit::{div, Context, Entity, Render, SharedString, Window};
use huanshu_core::services::{projects as project_service, settings};
use huanshu_core::themes::{self, AppTheme};

use crate::app::{self, AppShared};
use crate::board::{Board, BoardArgs};

/// 主题的小快照，方便放进实体里比较。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeRef {
    pub id: &'static str,
    pub name: &'static str,
    pub dark: bool,
}

impl From<AppTheme> for ThemeRef {
    fn from(theme: AppTheme) -> Self {
        Self {
            id: theme.id,
            name: theme.name,
            dark: theme.tone.is_dark(),
        }
    }
}

fn theme_ref(id: &str) -> ThemeRef {
    let (theme, _) = themes::theme_or_default(Some(id));
    ThemeRef {
        id: theme.id,
        name: theme.name,
        dark: theme.tone.is_dark(),
    }
}

/// 根实体：标题栏 + 页面。
pub struct Root {
    pub shared: AppShared,
    pub data_dir: String,
    pub theme: ThemeRef,
    /// `None` = 首页。
    board: Option<Entity<Board>>,
    /// 首页缓存的项目行，回到首页时刷新。
    projects: Vec<ProjectRow>,
    /// 命令行带来的「首次要做什么」；首帧之后跑一次。
    boot: Option<Boot>,
    /// `--flip-soak`：看板上每 100ms 换一页，循环这么久。
    flip_soak: Option<u64>,
    first_frame_logged: bool,
    #[allow(dead_code)]
    subscriptions: Vec<gpui_kit::Subscription>,
}

/// 命令行要求的一串启动动作。
#[derive(Debug, Clone, Default)]
pub struct Boot {
    pub open: Option<i64>,
    pub page: Option<usize>,
    pub present: bool,
}

/// 首页列表的一行（与服务层解耦，渲染时不再碰数据库）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRow {
    pub id: i64,
    pub name: String,
    pub slides: usize,
    pub ratio: String,
    pub updated: String,
}

/// 秒级时间戳 → `YYYY-MM-DD HH:MM`（本地时区）。只显示到分钟，够用。
fn format_time(seconds: i64) -> String {
    if seconds <= 0 {
        return "—".to_string();
    }
    let days = seconds / 86_400;
    let time = seconds % 86_400;
    // 用 `chrono` 之外的办法：Hinnant 的 civil_from_days。
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        time / 3_600,
        (time % 3_600) / 60
    )
}

impl Root {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        shared: AppShared,
        theme: ThemeRef,
        data_dir: String,
        boot: Option<Boot>,
        quit_after: Option<u64>,
        flip_soak: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // The shell follows the theme's tone.
        Theme::change(
            if theme.dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            Some(window),
            cx,
        );
        let projects = load_projects(&shared);
        if let Some(seconds) = quit_after {
            cx.spawn(async move |_this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(seconds))
                    .await;
                cx.update(|cx| cx.quit());
            })
            .detach();
        }
        Self {
            shared,
            data_dir,
            theme,
            board: None,
            projects,
            boot,
            flip_soak,
            first_frame_logged: false,
            subscriptions: Vec::new(),
        }
    }

    /// `--flip-soak`：每 100ms 换一页，跑完停；期间每 30 秒打一次工作集内存。
    fn start_flip_soak(&mut self, cx: &mut Context<Self>) {
        let seconds = self.flip_soak.take().unwrap_or(0);
        cx.spawn(async move |this, cx| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(seconds);
            let mut next_report = std::time::Instant::now();
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                let now = std::time::Instant::now();
                if now >= deadline {
                    eprintln!("[mem] {:.1} MiB (soak 结束)", app::working_set_mib());
                    break;
                }
                if now >= next_report {
                    eprintln!("[mem] {:.1} MiB", app::working_set_mib());
                    next_report = now + std::time::Duration::from_secs(30);
                }
                let _ = this.update(cx, |root, cx| root.cycle_page(cx));
            }
        })
        .detach();
    }

    /// soak 专用：往下翻一页，末尾回首页。
    fn cycle_page(&mut self, cx: &mut Context<Self>) {
        let Some(board) = self.board.clone() else {
            self.goto_home(cx);
            return;
        };
        board.update(cx, |board, cx| {
            if board.page_count() > 0 {
                let next = (board.selected + 1) % board.page_count();
                board.select_index(next);
                cx.notify();
            }
        });
    }

    fn open(&mut self, id: i64, page: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let args = BoardArgs {
            project_id: id,
            start_page: page.unwrap_or(0),
            theme: self.theme.id,
            shared: self.shared.clone(),
            root: cx.entity(),
        };
        let board = cx.new(|cx| Board::new(args, window, cx));
        self.board = Some(board);
        cx.notify();
    }

    /// 回到首页并刷新项目列表（能看到别的界面刚改过的更新时间）。
    ///
    /// 看板实体被丢弃前必须释放它的位图：GPUI 不会替我们回收图集 tile。
    pub fn goto_home(&mut self, cx: &mut Context<Self>) {
        if let Some(board) = self.board.take() {
            board.update(cx, |board, cx| board.release_all(cx));
        }
        self.projects = load_projects(&self.shared);
        cx.notify();
    }

    /// 命令行的启动动作，在首帧之后跑。
    pub fn run_boot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(boot) = self.boot.take() else {
            return;
        };
        if let Some(id) = boot.open {
            self.open(id, boot.page, window, cx);
            if boot.present {
                self.present(cx);
            }
        }
    }

    pub fn present(&mut self, cx: &mut Context<Self>) {
        let Some(board) = self.board.clone() else {
            return;
        };
        let (project_id, slides, theme, shared) = board.read(cx).deck_for_present();
        crate::present::open(project_id, slides, theme, shared, cx);
    }

    fn set_theme(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let theme = theme_ref(id);
        if theme.id == self.theme.id {
            return;
        }
        self.theme = theme;
        if let Err(error) = settings::set_app_theme(&self.shared, id) {
            eprintln!("[native] 保存主题失败：{error}");
        }
        Theme::change(
            if theme.dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            Some(window),
            cx,
        );
        if let Some(board) = self.board.clone() {
            board.update(cx, |board, cx| board.set_theme(theme.id, cx));
        }
        cx.notify();
    }
}

fn load_projects(shared: &AppShared) -> Vec<ProjectRow> {
    project_service::list_projects(shared)
        .unwrap_or_else(|err| {
            eprintln!("[native] 读取项目列表失败：{err}");
            Vec::new()
        })
        .into_iter()
        .map(|project| ProjectRow {
            id: project.id,
            name: project.name,
            slides: project.slide_count,
            ratio: project
                .aspect_ratio
                .map(|ratio| ratio.label().split(' ').next().unwrap_or("").to_string())
                .unwrap_or_else(|| "—".to_string()),
            updated: format_time(project.updated_at),
        })
        .collect()
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.first_frame_logged {
            self.first_frame_logged = true;
            crate::note_first_frame();
            if self.flip_soak.is_some() {
                self.start_flip_soak(cx);
            }
        }
        self.run_boot(window, cx);
        let theme_index = themes::APP_THEMES
            .iter()
            .position(|theme| theme.id == self.theme.id)
            .unwrap_or(0);
        let next_theme = themes::APP_THEMES[(theme_index + 1) % themes::APP_THEMES.len()].id;
        let board = self.board.clone();
        let on_board = board.is_some();

        let content: gpui_kit::AnyElement = match board {
            Some(board) => board.into_any_element(),
            None => self.home(cx).into_any_element(),
        };

        // 整个应用是一列：标题栏在上面，页面在下面——窗口尺寸由此决定。
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                gpui_kit::component::TitleBar::new().child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .child(div().text_sm().child(SharedString::from(if on_board {
                            "幻述 · 看板"
                        } else {
                            "幻述 · 原生界面"
                        })))
                        // The data directory is on screen on purpose: an isolated sandbox must be
                        // obvious at a glance.
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(SharedString::from(self.data_dir.clone())),
                        )
                        .child(div().flex_1())
                        .child(
                            Button::new("theme")
                                .label(format!("主题：{}", self.theme.name))
                                // Cycle through the catalogue; a real dropdown comes later.
                                .on_click(cx.listener(move |root, _event, window, cx| {
                                    root.set_theme(next_theme, window, cx)
                                })),
                        )
                        .child(
                            Button::new("home").label("项目").on_click(
                                cx.listener(|root, _event, _window, cx| root.goto_home(cx)),
                            ),
                        ),
                ),
            )
            .child(div().flex_1().min_h_0().child(content))
    }
}

impl Root {
    fn home(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.projects.clone();
        let empty = rows.is_empty();
        div()
            .id("home-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(
        div()
            .flex()
            .flex_col()
            // 滚动容器里没有确定高度，不能用 `size_full`（会把内容压成 0 高）。
            .w_full()
            .p_4()
            .gap_2()
            .when(empty, |element| {
                element.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(SharedString::from(
                            "还没有项目。用 cargo run -p huanshu-core --example seed_demo 播种演示数据。",
                        )),
                )
            })
            .children(rows.into_iter().map(|row| {
                let id = row.id;
                div()
                    .id(SharedString::from(format!("project-{id}")))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .w_full()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .cursor_pointer()
                    .child(
                        div()
                            .w_full()
                            .text_lg()
                            .child(SharedString::from(row.name)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_4()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(SharedString::from(format!("{} 张", row.slides)))
                            .child(SharedString::from(row.ratio))
                            .child(SharedString::from(row.updated)),
                    )
                    .on_click(cx.listener(move |root, _event, window, cx| {
                        root.open(id, None, window, cx);
                    }))
            }))
            )
    }
}
