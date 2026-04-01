use crate::config::LlmSettings;
use crate::lmstudio::LmStudioClient;
use tauri::State;
use crate::AppState;
use std::sync::Mutex;

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
pub async fn list_models(state: State<'_, Mutex<AppState>>) -> Result<Vec<String>, String> {
    let (base_url, api_key) = {
        let state = state.lock().unwrap();
        (state.settings.base_url.clone(), state.settings.api_key.clone())
    };
    let client = LmStudioClient::new(&base_url).with_api_key(&api_key);
    client.list_models().await.map_err(|e| e.to_string())
}
