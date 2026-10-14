//! 缩略图光栅化的 Tauri 命令。实现见 `huanshu_core::services::export`——与 PDF 导出
//! 共用同一套主题、比例、字体与素材解析。

use tauri::State;

use super::Shared;

use huanshu_core::services::export::{self as service, ThumbnailBatch};
use huanshu_core::types::SlideBlueprint;

/// Rasterizes `slides` (the deck currently on screen) into PNG thumbnails.
///
/// `only` limits the rasterized pages; positions refer to `slides`.
#[tauri::command]
pub async fn render_slide_thumbnails(
    project_id: Option<i64>,
    slides: Vec<SlideBlueprint>,
    theme: Option<String>,
    width_px: u32,
    only: Option<Vec<usize>>,
    state: State<'_, Shared>,
) -> Result<ThumbnailBatch, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service::render_slide_thumbnails(&state, project_id, slides, theme, width_px, only)
    })
    .await
    .map_err(|e| format!("缩略图渲染任务异常结束：{e}"))?
}
