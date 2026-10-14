//! 幻述原生界面（只读查看器）：项目列表 → 看板 → 演示 → 导出 PDF。
//!
//! 业务逻辑全在 `huanshu_core`；本工程只做界面与「把 Typst 的像素画出来」这一件事。
//! 命令行开关见 `README.md` 与 `cli.rs`。

mod app;
mod board;
mod cli;
mod export;
mod images;
mod present;
mod shell;

use gpui_kit::prelude::*;
use gpui_kit::{px, size, App, Bounds, WindowBounds};

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
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            let (shared, current, data_dir) = app::load();
            let theme = shell::ThemeRef::from(current);
            if let Some(requested) = cli.theme.as_deref() {
                match huanshu_core::services::settings::set_app_theme(&shared, requested) {
                    Ok(()) => eprintln!("[native] 主题设为 {requested}"),
                    Err(error) => eprintln!("[native] --theme {requested}: {error}"),
                }
            }
            let boot = shell::Boot {
                open: cli.open,
                page: cli.page,
                present: cli.present,
            };

            // 预热渲染器：扫描系统字体约 0.5 秒，不该由第一张图来付。
            cx.background_spawn(async move {
                huanshu_core::services::export::renderer();
            })
            .detach();

            let mut options = gpui_kit::component::TitleBar::window_options();
            options.window_min_size = Some(size(px(1080.), px(680.)));
            options.window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1440.), px(900.)),
                cx,
            )));
            gpui_kit::open_window(options, cx, move |window, cx| {
                cx.new(move |cx| {
                    shell::Root::new(
                        shared,
                        theme,
                        data_dir,
                        Some(boot),
                        cli.quit_after,
                        cli.flip_soak,
                        window,
                        cx,
                    )
                })
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
    match app::export_pdf(&shared, project_id, path, theme) {
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
