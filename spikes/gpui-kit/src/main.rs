//! The spike application: a self-drawn (borderless) window with a title bar, a tab
//! strip, a dark/light theme switch and a presentation-mode window.
//!
//! Command line: `--page board|editor|controls`, `--theme dark|light`,
//! `--deck <path>`, `--typst-theme <id>`, `--present`, `--dialog`, `--bench`,
//! `--bench-edit`, `--soak`, `--quit-after <seconds>`. The switches exist so the
//! report's screenshots and numbers can be reproduced without clicking.

mod board;
mod controls;
mod deck;
mod editor;
mod image_bridge;
mod metrics;
mod present;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::button::Button;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme as _, Theme, ThemeMode, TitleBar};
use gpui_kit::prelude::*;
use gpui_kit::{
    App, Bounds, Context, Entity, SharedString, Subscription, Window, WindowBounds, div, px, size,
};
use slide_typst::SUPPORTED_THEMES;

use board::Board;
use controls::Controls;
use editor::EditorPage;
use present::Present;

const PAGES: [&str; 3] = ["幻灯片看板", "Markdown 编辑器", "外壳组件"];

/// Process start, so the first painted frame can be reported as a cold start.
static PROCESS_START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// Parsed command line; defaults open the board in the dark theme.
struct Cli {
    page: usize,
    dark: bool,
    deck: Option<PathBuf>,
    typst_theme: Option<String>,
    present: bool,
    dialog: bool,
    bench: bool,
    bench_edit: bool,
    soak: bool,
    paint_soak: bool,
    soak_seconds: u64,
    quit_after: Option<u64>,
}

impl Cli {
    fn parse() -> Self {
        let mut cli = Self {
            page: 0,
            dark: true,
            deck: None,
            typst_theme: None,
            present: false,
            dialog: false,
            bench: false,
            bench_edit: false,
            soak: false,
            paint_soak: false,
            soak_seconds: 120,
            quit_after: None,
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--page" => {
                    cli.page = match args.next().as_deref() {
                        Some("editor") => 1,
                        Some("controls") => 2,
                        _ => 0,
                    }
                }
                "--theme" => cli.dark = args.next().as_deref() != Some("light"),
                "--deck" => cli.deck = args.next().map(PathBuf::from),
                "--typst-theme" => cli.typst_theme = args.next(),
                "--present" => cli.present = true,
                "--dialog" => cli.dialog = true,
                "--bench" => cli.bench = true,
                "--bench-edit" => cli.bench_edit = true,
                "--soak" => cli.soak = true,
                "--paint-soak" => cli.paint_soak = true,
                "--soak-seconds" => {
                    cli.soak_seconds = args.next().and_then(|value| value.parse().ok()).unwrap_or(120);
                }
                "--quit-after" => {
                    cli.quit_after = args.next().and_then(|value| value.parse().ok());
                }
                other => eprintln!("[spike] 忽略未知参数 {other}"),
            }
        }
        cli
    }
}

struct AppRoot {
    page: usize,
    board: Entity<Board>,
    editor: Entity<EditorPage>,
    controls: Entity<Controls>,
    dark: bool,
    first_frame_logged: bool,
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,
}

impl AppRoot {
    fn new(window: &mut Window, cx: &mut Context<Self>, cli: Cli) -> Self {
        let board = cx.new(|cx| Board::new(cli.deck.clone(), window, cx));
        let editor = cx.new(|cx| EditorPage::new(window, cx));
        let controls = cx.new(|cx| Controls::new(window, cx));
        if let Some(theme) = cli.typst_theme.as_deref() {
            if let Some(index) = SUPPORTED_THEMES.iter().position(|it| *it == theme) {
                board.update(cx, |board, cx| board.set_theme(index, window, cx));
            } else {
                eprintln!("[spike] 未知 Typst 主题 {theme}，可用：{SUPPORTED_THEMES:?}");
            }
        }
        if !cli.dark {
            Theme::change(ThemeMode::Light, Some(window), cx);
        }
        if cli.bench {
            board.update(cx, |board, cx| board.start_transfer_bench(cx));
        }
        if cli.bench_edit {
            board.update(cx, |board, cx| board.start_edit_bench(window, cx));
        }
        if cli.soak {
            board.update(cx, |board, cx| {
                board.start_soak(window, cx, cli.soak_seconds)
            });
        }
        if cli.paint_soak {
            board.update(cx, |board, cx| {
                board.start_paint_soak(window, cx, cli.soak_seconds)
            });
        }
        let mut root = Self {
            page: cli.page,
            board,
            editor,
            controls,
            dark: cli.dark,
            first_frame_logged: false,
            subscriptions: Vec::new(),
        };
        if cli.present || cli.dialog {
            let open_present = cli.present;
            let open_dialog = cli.dialog;
            cx.defer_in(window, move |root: &mut AppRoot, window, cx| {
                if open_present {
                    root.open_presentation(window, cx);
                }
                if open_dialog {
                    root.controls.update(cx, |controls, cx| {
                        controls.open_password_dialog(window, cx)
                    });
                }
            });
        }
        if let Some(seconds) = cli.quit_after {
            cx.spawn(async move |_this, cx| {
                cx.background_executor()
                    .timer(Duration::from_secs(seconds))
                    .await;
                metrics::report("退出前");
                cx.update(|cx| cx.quit());
            })
            .detach();
        }
        root.subscriptions = Vec::new();
        root
    }

    fn set_dark(&mut self, dark: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.dark == dark {
            return;
        }
        self.dark = dark;
        Theme::change(
            if dark { ThemeMode::Dark } else { ThemeMode::Light },
            Some(window),
            cx,
        );
        eprintln!("[spike] theme -> {}", if dark { "dark" } else { "light" });
        cx.notify();
    }

    /// Opens the presentation window for the deck the board currently shows.
    fn open_presentation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (deck, theme, ratio) = {
            let board = self.board.read(cx);
            (
                Arc::new(board.deck().to_vec()),
                board.theme_name().to_string(),
                board.aspect_ratio().to_string(),
            )
        };
        eprintln!(
            "[spike] presentation: {} 页，主题 {theme}，比例 {ratio}",
            deck.len()
        );
        let options = Present::window_options(cx);
        let opened =
            cx.open_window(options, |window, cx| {
                cx.new(|cx| Present::new(deck, theme, ratio, window, cx))
            });
        if let Err(err) = opened {
            eprintln!("[spike] 无法打开演示窗口：{err}");
        }
        let _ = window;
    }
}

impl Render for AppRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.first_frame_logged {
            self.first_frame_logged = true;
            if let Some(start) = PROCESS_START.get() {
                // The first painted frame of the main window: GPUI's own startup plus
                // the initial view construction, measured from process entry. The
                // `[coldstart]` marker is ASCII so tools/coldstart.ps1 can parse it
                // (PowerShell 5.1 mis-reads non-ASCII in BOM-less scripts).
                eprintln!(
                    "[coldstart] first-frame {:.1} ms",
                    start.elapsed().as_secs_f64() * 1000.0
                );
            }
            metrics::report("首帧");
        }
        let page = self.page;
        let dark = self.dark;
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                TitleBar::new()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .child(div().text_sm().child("幻述 · GPUI Kit spike"))
                            .child(
                                Switch::new("theme-mode")
                                    .label(if dark { "深色" } else { "浅色" })
                                    .checked(dark)
                                    .on_change(cx.listener(
                                        |root, checked: &bool, window, cx| {
                                            root.set_dark(*checked, window, cx)
                                        },
                                    )),
                            )
                            .child(Button::new("present").label("演示模式").on_click(
                                cx.listener(|root, _event, window, cx| {
                                    root.open_presentation(window, cx)
                                }),
                            )),
                    )
                    .child(div().flex_1()),
            )
            .child(
                TabBar::new("pages")
                    .segmented()
                    .selected_index(page)
                    .children(PAGES.iter().map(|title| Tab::new().label(*title)))
                    .on_click(cx.listener(|root, index: &usize, _window, cx| {
                        root.page = *index;
                        cx.notify();
                    })),
            )
            .child(div().flex_1().min_h_0().child(match page {
                0 => self.board.clone().into_any_element(),
                1 => self.editor.clone().into_any_element(),
                _ => self.controls.clone().into_any_element(),
            }))
            .child(
                div()
                    .px_3()
                    .py_1()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .child(SharedString::from(
                        "GPUI Kit 0.7.3 · 幻灯片由 slide-typst 光栅化，界面只负责显示",
                    )),
            )
    }
}

fn main() {
    let _ = PROCESS_START.set(std::time::Instant::now());
    let cli = Cli::parse();
    metrics::report("启动前");
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            let mut options = TitleBar::window_options();
            options.window_min_size = Some(size(px(1080.), px(680.)));
            options.window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1440.), px(900.)),
                cx,
            )));
            gpui_kit::open_window(options, cx, |window, cx| {
                eprintln!(
                    "[spike] scale_factor={} bounds={}x{}",
                    window.scale_factor(),
                    window.bounds().size.width.as_f32(),
                    window.bounds().size.height.as_f32()
                );
                // The renderer's font scan happens on the first raster; warming it here
                // keeps the first paint from stalling on it.
                let warm = cx.background_spawn(async move {
                    let started = std::time::Instant::now();
                    let _ = deck::renderer();
                    metrics::time("渲染器就绪（字体扫描）", started.elapsed());
                });
                warm.detach();
                cx.new(move |cx| AppRoot::new(window, cx, cli))
            })
            .expect("open the spike window");
            metrics::report("窗口创建后");
        });
}
