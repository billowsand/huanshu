//! 模型服务设置：读取、保存与列出可用模型。
//!
//! 保存前会校验三个服务的代理设置；写盘失败时把内存里的设置还原，避免出现
//! 「界面显示已保存、磁盘上却是旧值」的状态。

use crate::config::{LlmSettings, ModelServiceSettings};
use crate::db;
use crate::lmstudio::LmStudioClient;
use crate::state::SharedState;

use super::Result;

pub fn get_settings(state: &SharedState) -> Result<LlmSettings> {
    let state = state.lock().unwrap();
    Ok(state.settings.clone())
}

pub fn save_settings(state: &SharedState, settings: LlmSettings) -> Result<()> {
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

/// Lists the models the service offers. `service` overrides the saved one, which is
/// how the settings page tests a connection before saving it.
pub async fn list_models(
    state: &SharedState,
    target: Option<String>,
    service: Option<ModelServiceSettings>,
) -> Result<Vec<String>> {
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

/// 当前主题的 id。没设置过时返回 [`crate::themes::DEFAULT_THEME`]。
///
/// 存在 `app_settings` 的 `app_theme` 键里（键值表，不改表结构）。注意 Tauri 版的主题存在
/// WebView 的 localStorage 里，两边暂不同步。
pub fn get_app_theme(state: &SharedState) -> Result<String> {
    let state = state.lock().unwrap();
    let db = state.db.lock().unwrap();
    let stored = db::get_app_setting(&*db, APP_THEME_KEY)
        .map_err(|e| e.to_string())?
        .filter(|value| crate::themes::is_known_theme(value));
    Ok(stored.unwrap_or_else(|| crate::themes::DEFAULT_THEME.to_string()))
}

/// 保存当前主题。未知 id 拒绝——否则导出时会静默回落成另一个主题，用户看不出来。
pub fn set_app_theme(state: &SharedState, theme: &str) -> Result<()> {
    if !crate::themes::is_known_theme(theme) {
        return Err(format!(
            "未知主题 {theme}；可用：{}",
            crate::themes::APP_THEMES
                .iter()
                .map(|t| t.id)
                .collect::<Vec<_>>()
                .join(" / ")
        ));
    }
    let state = state.lock().unwrap();
    let db = state.db.lock().unwrap();
    db::set_app_setting(&*db, APP_THEME_KEY, theme).map_err(|e| e.to_string())
}

/// `app_settings` 表里存主题的键。
const APP_THEME_KEY: &str = "app_theme";
