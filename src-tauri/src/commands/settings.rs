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
    for (name, service) in [
        ("大语言模型", &settings.llm),
        ("向量模型", &settings.embedding),
        ("多模态模型", &settings.multimodal),
    ] {
        service
            .proxy
            .validate()
            .map_err(|e| format!("{name}: {e}"))?;
    }
    let mut state = state.lock().unwrap();
    let previous = std::mem::replace(&mut state.settings, settings);
    if let Err(error) = state.persist_settings() {
        state.settings = previous;
        return Err(error.to_string());
    }
    Ok(())
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
    let client = LmStudioClient::from_service(&service).map_err(|e| e.to_string())?;
    client.list_models().await.map_err(|e| e.to_string())
}
