//! 应用状态与「渲染一批页」这一件事。
//!
//! 状态就是 core 的 `AppState`，外面套一层和 Tauri 版同样的锁（`Arc<Mutex<AppState>>`），
//! 数据一律经 `huanshu_core::services` 读写——本工程不碰 SQLite。

use std::sync::{Arc, Mutex};

use gpui_kit::RenderImage;
use huanshu_core::preview::{PreviewResult, RenderedSlide};
use huanshu_core::services::{export, projects, settings};
use huanshu_core::themes::{self, AppTheme};
use huanshu_core::types::SlideBlueprint;

use crate::images::to_render_image;

/// 与 Tauri 侧 `commands::Shared` 同一个形状：core 的状态 + 一层可跨线程的锁。
pub type AppShared = Arc<Mutex<huanshu_core::AppState>>;

/// 启动时加载状态与当前主题。`HUANSHU_HOME` 由 core 自己读，这里不参与。
pub fn load() -> (AppShared, AppTheme, String) {
    let shared: AppShared = Arc::new(Mutex::new(huanshu_core::AppState::load()));
    let data_dir = shared.lock().unwrap().app_settings.data_dir.clone();
    let theme_id = settings::get_app_theme(&shared).unwrap_or_else(|err| {
        eprintln!("[native] 读取主题失败，用默认：{err}");
        themes::DEFAULT_THEME.to_string()
    });
    let (theme, fallback) = themes::theme_or_default(Some(&theme_id));
    if let Some(requested) = fallback {
        eprintln!("[native] 未知主题 {requested}，回落到 {}", theme.id);
    }
    (shared, *theme, data_dir)
}

/// 读一份项目的牌组。空牌组也让调用方继续（看板上就是空列表）。
pub fn load_deck(shared: &AppShared, project_id: i64) -> Result<Vec<SlideBlueprint>, String> {
    let project = projects::get_project(shared, project_id)?;
    if project.blueprints_json.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&project.blueprints_json).map_err(|err| format!("读取幻灯片失败：{err}"))
}

/// 一批渲染好的页。
pub type Pages = PreviewResult<Arc<RenderImage>>;

/// 渲染 `only` 这些页（子集位置）。**只能在后台线程上调用。**
pub fn render_pages(
    shared: AppShared,
    project_id: Option<i64>,
    slides: Vec<SlideBlueprint>,
    theme: Option<String>,
    width_px: u32,
    only: Vec<usize>,
) -> Result<Pages, String> {
    let batch =
        export::render_slide_pixels(&shared, project_id, slides, theme, width_px, Some(only))?;
    Ok(PreviewResult {
        pages: batch
            .pages
            .into_iter()
            .map(|page| {
                let index = page.index;
                let overflow = batch.overflow.contains(&index);
                RenderedSlide {
                    index,
                    image: to_render_image(page),
                    // Typst covers every kind the catalogue lists, so a placeholder here
                    // would mean the deck references a kind with no layout.
                    placeholder: false,
                    overflow,
                }
            })
            .collect(),
        theme_fallback: batch.theme_fallback,
    })
}

/// 导出 PDF，返回页数等报告要点。**只能在后台线程上调用。**
pub fn export_pdf(
    shared: &AppShared,
    project_id: i64,
    path: std::path::PathBuf,
    theme: String,
) -> Result<export::PdfExportReport, String> {
    export::export_pdf(
        shared,
        Some(project_id),
        None,
        path.to_string_lossy().into(),
        Some(theme),
    )
}

/// 工作集内存（MiB），`--flip-soak` 用来证明内存收敛。
pub fn working_set_mib() -> f64 {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "[math]::Round((Get-Process -Id $PID).WorkingSet64 / 1MB, 1)",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        if let Ok(output) = output {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if let Ok(value) = text.trim().parse::<f64>() {
                    return value;
                }
            }
        }
        0.0
    }
    #[cfg(not(windows))]
    {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `HUANSHU_HOME` 把状态指到隔离目录，因此 `AppState::load()` 不会碰真实数据。
    #[test]
    fn state_load_follows_the_sandbox() {
        let home = std::env::temp_dir().join("huanshu-native-test-home");
        let data_dir = home.join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        unsafe { std::env::set_var("HUANSHU_HOME", &home) };
        let (shared, _, reported) = load();
        assert!(reported.contains("huanshu-native-test-home"), "{reported}");
        assert!(!reported.contains("auto-slidev-studio"));
        // 这次写的是沙箱：清掉，不留垃圾。
        drop(shared);
        std::fs::remove_dir_all(&home).ok();
        unsafe { std::env::remove_var("HUANSHU_HOME") };
    }
}
