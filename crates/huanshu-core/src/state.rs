//! 应用状态：设置、当前牌组、SQLite 连接与存储根同步。
//!
//! 这份状态原来住在 Tauri 的 `lib.rs` 里，界面层（Tauri 命令、将来的 GPUI）都只是把它
//! 放在自己的互斥锁后面再取用：`Mutex<AppState>`。锁定与线程模型留给界面层，core 只管状态本身。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use rusqlite::Connection;

use crate::config::LlmSettings;
use crate::db;
use crate::types::{AppGlobalSettings, SlideBlueprint};

/// 界面层持有的状态句柄。Tauri 命令层与 GPUI 界面各用一个自己的互斥锁包住
/// [`AppState`]，服务函数只认 `&SharedState`。
pub type SharedState = std::sync::Mutex<AppState>;

/// 本机数据目录的默认值：`<LOCALAPPDATA>/auto-slidev-studio`。
pub fn default_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("auto-slidev-studio")
}

/// 指向数据目录等「开机前就要知道」的少量字段的文件。
pub fn bootstrap_path() -> PathBuf {
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
    #[serde(default)]
    initialized_embedding_model: String,
    #[serde(default)]
    fonts: crate::types::FontSettings,
}

pub fn load_bootstrap_data_dir() -> PathBuf {
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

pub fn load_bootstrap_settings() -> Option<AppGlobalSettings> {
    let boot_path = bootstrap_path();
    if boot_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&boot_path) {
            if let Ok(boot) = serde_json::from_str::<BootstrapData>(&content) {
                return Some(AppGlobalSettings {
                    data_dir: boot.data_dir,
                    media_dir: boot.media_dir,
                    llm_configured: boot.llm_configured,
                    embeddings_ready: boot.embeddings_ready,
                    initialized_embedding_model: boot.initialized_embedding_model,
                    fonts: boot.fonts,
                });
            }
        }
    }
    None
}

pub fn save_bootstrap_settings(settings: &AppGlobalSettings) -> Result<()> {
    let boot_path = bootstrap_path();
    if let Some(parent) = boot_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let boot = BootstrapData {
        data_dir: settings.data_dir.clone(),
        media_dir: settings.media_dir.clone(),
        llm_configured: settings.llm_configured,
        embeddings_ready: settings.embeddings_ready,
        initialized_embedding_model: settings.initialized_embedding_model.clone(),
        fonts: settings.fonts.clone(),
    };
    std::fs::write(&boot_path, serde_json::to_string_pretty(&boot)?)?;
    Ok(())
}

/// Global application state shared across the UI layer.
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
    /// Points the database, `settings.json` and the bootstrap file at the current
    /// data directory. Called whenever the user changes the storage root.
    pub fn sync_storage_root(&mut self) -> Result<()> {
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
        db::set_app_setting(
            &conn,
            "initialized_embedding_model",
            &self.app_settings.initialized_embedding_model,
        )?;
        db::set_app_setting(
            &conn,
            "fonts",
            &serde_json::to_string(&self.app_settings.fonts)?,
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
            let initialized_embedding_model =
                db::get_app_setting(&*conn, "initialized_embedding_model")
                    .ok()
                    .flatten()
                    .unwrap_or_default();
            AppGlobalSettings {
                data_dir: data_dir_str,
                media_dir,
                llm_configured,
                embeddings_ready,
                initialized_embedding_model,
                fonts: db::get_app_setting(&*conn, "fonts")
                    .ok()
                    .flatten()
                    .and_then(|value| serde_json::from_str(&value).ok())
                    .unwrap_or_default(),
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

    pub fn persist_settings(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.settings)?;
        std::fs::write(&self.settings_path, json)?;
        Ok(())
    }

    pub fn persist_app_settings(&mut self) -> Result<()> {
        self.sync_storage_root()
    }
}
