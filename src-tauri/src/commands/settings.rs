use crate::config::{LlmSettings, ModelServiceSettings};
use crate::lmstudio::LmStudioClient;
use crate::AppState;
use std::sync::Mutex;
use tauri::State;

#[tauri::command]
pub async fn get_settings(state: State<'_, Mutex<AppState>>) -> Result<LlmSettings, String> {
    let state = state.lock().unwrap();
    Ok(state.settings.clone())
}

#[tauri::command]
pub async fn save_settings(
    settings: LlmSettings,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let mut state = state.lock().unwrap();
    state.settings = settings;
    state.persist_settings().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_models(
    target: Option<String>,
    service: Option<ModelServiceSettings>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<String>, String> {
    let service = if let Some(service) = service {
        service
    } else {
        let state = state.lock().unwrap();
        match target.as_deref().unwrap_or("llm") {
            "llm" => state.settings.llm.clone(),
            "embedding" => state.settings.embedding.clone(),
            "multimodal" => state.settings.multimodal.clone(),
            other => return Err(format!("unknown model target: {other}")),
        }
    };
    let client = LmStudioClient::new(&service.base_url).with_api_key(&service.api_key);
    client.list_models().await.map_err(|e| e.to_string())
}
