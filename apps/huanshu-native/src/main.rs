//! 幻述原生界面（GPUI Kit）：逐页复刻 Tauri + Vue 版。
//!
//! 结构见 `README.md`「框架」一节与 `docs/native/parity.md`（逐项功能对照）。
//! 业务逻辑全在 `huanshu_core`；本工程只做界面。

// 发布版不弹控制台窗口：换回 Windows 子系统。
// 命令行模式（`--quit-after`、`--export-pdf`）不受影响——父进程重定向的 stdout/stderr
// 句柄照旧可写，`[coldstart]`、`[export]` 这些行仍在。debug 构建保留控制台，日志直接可见。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cli;
mod export;
mod formulas;
mod generation;
mod icons;
mod images;
mod pages;
mod password_dialog;
mod platform;
mod present;
mod router;
mod shell;
mod slides;
mod tasks;
mod theme;
mod widgets;

use gpui_kit::prelude::*;
use gpui_kit::{point, px, size, App, Bounds, TitlebarOptions, WindowBounds};

/// 进程入口时刻，用来报冷启动到首帧。
static PROCESS_START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

fn main() {
    let _ = PROCESS_START.set(std::time::Instant::now());
    let cli = cli::Cli::parse();

    // `--export-pdf` 是纯脚本路径：不建窗口，导完按退出码结束。
    if let (Some(project_id), Some(path)) = (cli.open, cli.export_pdf.clone()) {
        std::process::exit(export_only(project_id, path, cli.theme.as_deref()));
    }

    gpui_kit::application()
        .with_assets(icons::NativeAssets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            // 退出、菜单栏、⌘Q（macOS）；关掉最后一个窗口即退出（全平台）。
            platform::init(cx);
            theme::init_typography(cx);
            let (shared, current, _data_dir) = app::load();
            let theme_id = match cli.theme.as_deref() {
                Some(requested) => {
                    match huanshu_core::services::settings::set_app_theme(&shared, requested) {
                        Ok(()) => requested.to_string(),
                        Err(error) => {
                            eprintln!("[native] --theme {requested}: {error}");
                            current.id.to_string()
                        }
                    }
                }
                None => current.id.to_string(),
            };
            let font = shell::font_settings(&shared);
            theme::apply(&theme_id, Some(&font), None, cx);

            // 预热渲染器：扫描系统字体约 0.5 秒，不该由第一张图来付。
            cx.background_spawn(async move {
                huanshu_core::services::export::renderer();
            })
            .detach();

            if let Some(seconds) = cli.quit_after {
                cx.spawn(async move |cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(seconds))
                        .await;
                    cx.update(|cx| cx.quit());
                })
                .detach();
            }

            let mut options = gpui_kit::component::TitleBar::window_options();
            // 自绘标题栏不画系统标题，但 Win32 窗口文本 / 任务栏悬停仍要读它；
            // 这里与 Tauri 版 `app.windows[0].title` 对齐（同时也是安装版验收看的标题）。
            options.titlebar = Some(TitlebarOptions {
                title: Some("幻述".into()),
                // 红绿灯位置（仅 macOS）：自绘标题栏高 36px，(12, 12) 让系统按钮纵向居中、
                // 左边距与正文对齐；gpui-component 默认的 (9, 9) 是按它 34px 的栏高定的。
                // `widgets::title_bar` 在红绿灯右边留出 TRAFFIC_LIGHTS_WIDTH。
                traffic_light_position: Some(point(px(12.0), px(12.0))),
                ..gpui_kit::component::TitleBar::title_bar_options()
            });
            options.window_min_size = Some(size(px(1080.), px(680.)));
            options.window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1440.), px(900.)),
                cx,
            )));
            let route = cli.route.clone();
            gpui_kit::open_window(options, cx, move |window, cx| {
                cx.new(move |cx| shell::Shell::new(shared, route, window, cx))
            })
            .expect("open the native window");
        });
}

/// 冷启动到首页出现的耗时，release 构建下由 `tools/coldstart.ps1` 解析这一行。
pub fn note_first_frame() {
    if let Some(start) = PROCESS_START.get() {
        eprintln!(
            "[coldstart] first-frame {:.1} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
}

/// 没有窗口的导出：只打印一行摘要，退出码 0/1。
fn export_only(project_id: i64, path: std::path::PathBuf, theme: Option<&str>) -> i32 {
    let (shared, current, _) = app::load();
    let theme = theme.unwrap_or(current.id).to_string();
    match app::export_pdf(&shared, project_id, None, path, theme) {
        Ok(report) => {
            eprintln!("[export] {}", export::summarize(&report));
            0
        }
        Err(error) => {
            eprintln!("[export] 失败：{error}");
            1
        }
    }
}
