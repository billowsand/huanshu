mod commands;
mod config;
mod db;
mod embedded_icons;
mod generator;
mod icon;
mod input;
mod lmstudio;
mod render;
mod types;
mod validate;

use config::LlmSettings;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use types::{AppGlobalSettings, SlideBlueprint};

fn default_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("auto-slidev-studio")
}

fn bootstrap_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("auto-slidev-studio")
        .join("bootstrap.json")
}

#[derive(serde::Serialize, serde::Deserialize)]
struct BootstrapData {
    data_dir: String,
    media_dir: String,
    llm_configured: bool,
    embeddings_ready: bool,
}

fn load_bootstrap_data_dir() -> PathBuf {
    let boot_path = bootstrap_path();
    if boot_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&boot_path) {
            if let Ok(boot) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(data_dir) = boot.get("data_dir").and_then(|v| v.as_str()) {
                    return PathBuf::from(data_dir);
                }
            }
        }
    }
    default_data_dir()
}

fn load_bootstrap_settings() -> Option<AppGlobalSettings> {
    let boot_path = bootstrap_path();
    if boot_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&boot_path) {
            if let Ok(boot) = serde_json::from_str::<BootstrapData>(&content) {
                return Some(AppGlobalSettings {
                    data_dir: boot.data_dir,
                    media_dir: boot.media_dir,
                    llm_configured: boot.llm_configured,
                    embeddings_ready: boot.embeddings_ready,
                });
            }
        }
    }
    None
}

fn save_bootstrap_settings(settings: &AppGlobalSettings) -> anyhow::Result<()> {
    let boot_path = bootstrap_path();
    if let Some(parent) = boot_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let boot = BootstrapData {
        data_dir: settings.data_dir.clone(),
        media_dir: settings.media_dir.clone(),
        llm_configured: settings.llm_configured,
        embeddings_ready: settings.embeddings_ready,
    };
    std::fs::write(&boot_path, serde_json::to_string_pretty(&boot)?)?;
    Ok(())
}

/// Global application state shared across Tauri commands.
pub struct AppState {
    pub settings: LlmSettings,
    pub app_settings: AppGlobalSettings,
    pub blueprints: Vec<SlideBlueprint>,
    pub generation_running: bool,
    pub last_error: Option<String>,
    pub active_project_id: Option<i64>,
    pub project_dir: PathBuf,
    pub db: Arc<Mutex<Connection>>,
    settings_path: PathBuf,
}

impl AppState {
    fn sync_storage_root(&mut self) -> anyhow::Result<()> {
        let data_dir = PathBuf::from(&self.app_settings.data_dir);
        std::fs::create_dir_all(&data_dir)?;

        let settings_path = data_dir.join("settings.json");
        let settings_json = serde_json::to_string_pretty(&self.settings)?;
        std::fs::write(&settings_path, settings_json)?;

        let conn = db::open(&data_dir)?;
        db::set_app_setting(&conn, "data_dir", &self.app_settings.data_dir)?;
        db::set_app_setting(&conn, "media_dir", &self.app_settings.media_dir)?;
        db::set_app_setting(
            &conn,
            "llm_configured",
            if self.app_settings.llm_configured {
                "true"
            } else {
                "false"
            },
        )?;
        db::set_app_setting(
            &conn,
            "embeddings_ready",
            if self.app_settings.embeddings_ready {
                "true"
            } else {
                "false"
            },
        )?;

        self.db = Arc::new(Mutex::new(conn));
        self.settings_path = settings_path;
        save_bootstrap_settings(&self.app_settings)?;
        Ok(())
    }

    pub fn load() -> Self {
        let bootstrap_settings = load_bootstrap_settings();
        let data_dir = bootstrap_settings
            .as_ref()
            .map(|s| PathBuf::from(&s.data_dir))
            .unwrap_or_else(load_bootstrap_data_dir);
        std::fs::create_dir_all(&data_dir).ok();

        let settings_path = data_dir.join("settings.json");
        let settings = std::fs::read_to_string(&settings_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let db = Arc::new(Mutex::new(
            db::open(&data_dir).expect("failed to open SQLite database"),
        ));

        let app_settings = bootstrap_settings.unwrap_or_else(|| {
            let conn = db.lock().unwrap();
            let data_dir_str = db::get_app_setting(&*conn, "data_dir")
                .ok()
                .flatten()
                .unwrap_or_else(|| data_dir.to_string_lossy().to_string());
            let media_dir = db::get_app_setting(&*conn, "media_dir")
                .ok()
                .flatten()
                .unwrap_or_else(|| data_dir.join("media").to_string_lossy().to_string());
            let llm_configured = db::get_app_setting(&*conn, "llm_configured")
                .ok()
                .flatten()
                .map(|v| v == "true")
                .unwrap_or(false);
            let embeddings_ready = db::get_app_setting(&*conn, "embeddings_ready")
                .ok()
                .flatten()
                .map(|v| v == "true")
                .unwrap_or(false);
            AppGlobalSettings {
                data_dir: data_dir_str,
                media_dir,
                llm_configured,
                embeddings_ready,
            }
        });

        let project_dir = {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            if cwd.file_name().and_then(|n| n.to_str()) == Some("src-tauri") {
                cwd.parent().map(|p| p.to_path_buf()).unwrap_or(cwd)
            } else {
                cwd
            }
        };

        Self {
            settings,
            app_settings,
            blueprints: Vec::new(),
            generation_running: false,
            last_error: None,
            active_project_id: None,
            project_dir,
            db,
            settings_path,
        }
    }

    pub fn persist_settings(&self) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(&self.settings)?;
        std::fs::write(&self.settings_path, json)?;
        Ok(())
    }

    pub fn persist_app_settings(&mut self) -> anyhow::Result<()> {
        self.sync_storage_root()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = std::sync::Mutex::new(AppState::load());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::generate::generate_slides,
            commands::generate::detect_granularity,
            commands::generate::optimize_markdown_headings,
            commands::generate::get_blueprints,
            commands::generate::get_generation_status,
            commands::generate::get_latest_generation_run,
            commands::generate::get_generation_logs,
            commands::generate::repair_slide,
            commands::generate::ensure_icon_embeddings,
            commands::generate::recommend_icons_for_query,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::list_models,
            commands::projects::list_projects,
            commands::projects::get_project,
            commands::projects::create_project,
            commands::projects::update_project_content,
            commands::projects::update_project_blueprints,
            commands::projects::delete_project,
            commands::projects::open_project,
            commands::projects::set_active_project,
            commands::storage::get_app_settings,
            commands::storage::save_app_settings,
            commands::storage::is_first_run,
            commands::storage::complete_first_run,
            commands::storage::import_media_asset,
            commands::storage::delete_media_asset,
            commands::storage::get_project_media,
            commands::storage::update_media_caption,
            commands::storage::export_project,
            commands::storage::import_project,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
