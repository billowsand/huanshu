//! 模型服务设置的 Tauri 命令。实现见 `huanshu_core::services::settings`。

use tauri::State;

use super::Shared;

use huanshu_core::config::{LlmSettings, ModelServiceSettings};

use huanshu_core::services::settings as service;

#[tauri::command]
pub async fn get_settings(state: State<'_, Shared>) -> Result<LlmSettings, String> {
    service::get_settings(&state)
}

#[tauri::command]
pub async fn save_settings(settings: LlmSettings, state: State<'_, Shared>) -> Result<(), String> {
    service::save_settings(&state, settings)
}

#[tauri::command]
pub async fn list_models(
    target: Option<String>,
    service: Option<ModelServiceSettings>,
    state: State<'_, Shared>,
) -> Result<Vec<String>, String> {
    huanshu_core::services::settings::list_models(&state, target, service).await
}
