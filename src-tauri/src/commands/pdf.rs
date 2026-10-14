//! PDF 导出的 Tauri 命令。实现见 `huanshu_core::services::export`。
//!
//! 渲染是同步的重活，命令层把它放到阻塞线程池上跑，避免卡住 Tauri 的异步运行时。

use std::path::Path;

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
    // Core works on the stored JSON; the editor sends typed blueprints, so convert once here.
    let slides = slides.map(|slides| {
        slides
            .iter()
            .filter_map(|slide| serde_json::to_value(slide).ok())
            .collect::<Vec<serde_json::Value>>()
    });
    tauri::async_runtime::spawn_blocking(move || {
        service::export_pdf(&state, project_id, slides, path, theme)
    })
    .await
    .map_err(|e| format!("导出任务异常结束：{e}"))?
}

/// Selects the exported file in the system file manager. The shell-out lives in
/// `huanshu_core::services::export::reveal_in_file_manager` so the native UI does the
/// same thing without duplicating it.
#[tauri::command]
pub fn reveal_exported_file(path: String) -> Result<(), String> {
    service::reveal_in_file_manager(Path::new(&path))
}
