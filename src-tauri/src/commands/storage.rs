//! 应用设置、`.keynn` 导入导出与加密检测的 Tauri 命令。
//! 实现见 `huanshu_core::services::storage`，素材库命令在 [`super::media`]。

use tauri::State;

use super::Shared;

use huanshu_core::types::AppGlobalSettings;

use huanshu_core::services::storage as service;

#[tauri::command]
pub fn get_app_settings(state: State<'_, Shared>) -> Result<AppGlobalSettings, String> {
    service::get_app_settings(&state)
}

#[tauri::command]
pub fn save_app_settings(
    settings: AppGlobalSettings,
    state: State<'_, Shared>,
) -> Result<(), String> {
    service::save_app_settings(&state, settings)
}

#[tauri::command]
pub fn is_first_run(state: State<'_, Shared>) -> Result<bool, String> {
    service::is_first_run(&state)
}

#[tauri::command]
pub fn complete_first_run(llm_configured: bool, state: State<'_, Shared>) -> Result<(), String> {
    service::complete_first_run(&state, llm_configured)
}

#[tauri::command]
pub fn export_project(
    project_id: i64,
    state: State<'_, Shared>,
    encrypted: Option<bool>,
    password: Option<String>,
) -> Result<Vec<u8>, String> {
    service::export_project(&state, project_id, encrypted, password)
}

#[tauri::command]
pub fn import_project(
    zip_data: Vec<u8>,
    state: State<'_, Shared>,
    password: Option<String>,
) -> Result<i64, String> {
    service::import_project(&state, zip_data, password)
}

#[tauri::command]
pub fn is_encrypted_file(data: Vec<u8>) -> bool {
    service::is_encrypted_file(&data)
}
