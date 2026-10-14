//! PDF 导出的 Tauri 命令。实现见 `huanshu_core::services::export`。
//!
//! 渲染是同步的重活，命令层把它放到阻塞线程池上跑，避免卡住 Tauri 的异步运行时。

use std::path::PathBuf;

use tauri::State;

use super::Shared;

use huanshu_core::services::export::{self as service, PdfExportReport};
use huanshu_core::types::SlideBlueprint;

/// Exports `slides` (the editor's current deck) or, when absent, the project's saved
/// blueprints to a PDF at `path`.
#[tauri::command]
pub async fn export_pdf(
    project_id: Option<i64>,
    slides: Option<Vec<SlideBlueprint>>,
    path: String,
    theme: Option<String>,
    state: State<'_, Shared>,
) -> Result<PdfExportReport, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service::export_pdf(&state, project_id, slides, path, theme)
    })
    .await
    .map_err(|e| format!("导出任务异常结束：{e}"))?
}

/// Selects the exported file in the system file manager. This one really is a shell-out,
/// so it stays in the command layer instead of the service.
#[tauri::command]
pub fn reveal_exported_file(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    if !path.is_file() {
        return Err("文件不存在".to_string());
    }
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("explorer")
        .arg("/select,")
        .arg(&path)
        .spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open")
        .arg("-R")
        .arg(&path)
        .spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open")
        .arg(path.parent().unwrap_or(std::path::Path::new(".")))
        .spawn();
    result.map(|_| ()).map_err(|e| e.to_string())
}
