//! 素材库的 Tauri 命令。实现见 `huanshu_core::services::media`。

use std::sync::{Arc, Mutex};

use tauri::State;

use huanshu_core::AppState;

use huanshu_core::services::media::{self as service, ImportMediaRequest, MediaAssetResponse};

/// Tauri 侧持有的状态句柄：core 的 `AppState` 外面套一层可跨线程共享的锁。
type Shared = Arc<Mutex<AppState>>;

#[tauri::command]
pub async fn import_media_asset(
    req: ImportMediaRequest,
    state: State<'_, Shared>,
) -> Result<MediaAssetResponse, String> {
    service::import_media_asset(&state, req).await
}

#[tauri::command]
pub fn delete_media_asset(id: i64, state: State<'_, Shared>) -> Result<(), String> {
    service::delete_media_asset(&state, id)
}

#[tauri::command]
pub fn get_project_media(
    project_id: i64,
    state: State<'_, Shared>,
) -> Result<Vec<MediaAssetResponse>, String> {
    service::get_project_media(&state, project_id)
}

#[tauri::command]
pub fn update_media_caption(
    id: i64,
    caption: Option<String>,
    description: Option<String>,
    state: State<'_, Shared>,
) -> Result<(), String> {
    service::update_media_caption(&state, id, caption, description)
}
