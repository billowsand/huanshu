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

/// 本机的存储位置。
///
/// 设了 `HUANSHU_HOME` 时全部落在它下面（隔离沙箱：真机验证、演示数据、与真实项目
/// 互不干扰）；没设时与从前逐字相同——`<LOCALAPPDATA>/auto-slidev-studio` 放数据、
/// `<APPDATA>/auto-slidev-studio/bootstrap.json` 记数据目录。素材目录仍跟随数据目录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoragePaths {
    /// SQLite、settings.json 与默认素材目录的根。
    pub data_dir: PathBuf,
    /// 开机前就要知道数据目录的那几个字段。
    pub bootstrap: PathBuf,
}

/// 算出存储位置。`home` 是 `HUANSHU_HOME` 的值；`None` 或空串按「未设置」处理。
///
/// 纯函数：单测直接传 `Some(path)`，不去改进程环境变量（并发测试会互相干扰）。
pub fn storage_paths(home: Option<&std::path::Path>) -> StoragePaths {
    // An empty `HUANSHU_HOME` is a mistake, not a request to store data in the cwd.
    match home.filter(|home| !home.as_os_str().is_empty()) {
        Some(home) => StoragePaths {
            data_dir: home.join("data"),
            bootstrap: home.join("bootstrap.json"),
        },
        None => StoragePaths {
            data_dir: system_data_dir(),
            bootstrap: system_bootstrap_path(),
        },
    }
}

/// `HUANSHU_HOME` 的值；未设置或为空串时是 `None`。
pub fn sandbox_home() -> Option<PathBuf> {
    match std::env::var_os("HUANSHU_HOME") {
        Some(value) if !value.is_empty() => Some(PathBuf::from(value)),
        _ => None,
    }
}

/// 本机数据目录的默认值：`<LOCALAPPDATA>/auto-slidev-studio`。
fn system_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("auto-slidev-studio")
}

/// bootstrap 文件的默认位置：`<APPDATA>/auto-slidev-studio/bootstrap.json`。
fn system_bootstrap_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("auto-slidev-studio")
        .join("bootstrap.json")
}

/// 本机数据目录：设了 `HUANSHU_HOME` 就在沙箱里，否则用系统目录。
pub fn default_data_dir() -> PathBuf {
    storage_paths(sandbox_home().as_deref()).data_dir
}

/// bootstrap 文件：设了 `HUANSHU_HOME` 就在沙箱里，否则用系统目录。
pub fn bootstrap_path() -> PathBuf {
    storage_paths(sandbox_home().as_deref()).bootstrap
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

/// 读 `settings.json`。返回读到的设置与失败原因（`None` = 读成功）。
///
/// 文件不存在是「第一次运行」，不算失败；文件存在但读不了或解析不了才是失败——
/// 那时把设置当默认值用，但**记住原因**（[`AppState::settings_load_error`]）：
/// 界面必须显示失败、不允许保存，否则用户会把默认值写回、覆盖掉他原来的配置。
fn read_settings_file(path: &std::path::Path) -> (LlmSettings, Option<String>) {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (LlmSettings::default(), None)
        }
        Err(error) => {
            return (
                LlmSettings::default(),
                Some(format!("读取 {} 失败：{error}", path.display())),
            )
        }
    };
    match serde_json::from_str::<LlmSettings>(&text) {
        Ok(settings) => (settings, None),
        Err(error) => (
            LlmSettings::default(),
            Some(format!("{} 不是有效的设置文件：{error}", path.display())),
        ),
    }
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
    /// 上次读 `settings.json` 失败的原因（`None` = 正常）。设置页据此显示失败态并禁止保存。
    settings_load_error: Option<String>,
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
        let (settings, settings_load_error) = read_settings_file(&settings_path);

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
            settings_load_error,
        }
    }

    /// 上次读 `settings.json` 失败的原因；`None` 表示设置是可信的。
    ///
    /// 界面在它为 `Some` 时**不能**把内存里的设置当成用户的配置：那是默认值。
    pub fn settings_load_error(&self) -> Option<&str> {
        self.settings_load_error.as_deref()
    }

    /// `settings.json` 现在还能正常读吗？只读磁盘，不改内存。
    ///
    /// 应用跑着的时候文件也可能被改坏（手改、别的进程）；那时内存里那份还是好的，
    /// 但界面上的设置与磁盘已经对不上，保存回去会把磁盘那份盖掉——所以打开设置页时
    /// 也要再确认一次。
    pub fn settings_file_error(&self) -> Option<String> {
        read_settings_file(&self.settings_path).1
    }

    /// 重新从磁盘读 `settings.json`（设置页的「重新加载」）。
    ///
    /// 失败时不改动内存里的设置（可能还有别处在用），只更新失败原因。
    pub fn reload_settings(&mut self) -> Result<LlmSettings> {
        match read_settings_file(&self.settings_path) {
            (_, Some(error)) => {
                self.settings_load_error = Some(error.clone());
                Err(anyhow::anyhow!(error))
            }
            (settings, None) => {
                self.settings = settings;
                self.settings_load_error = None;
                Ok(self.settings.clone())
            }
        }
    }

    pub fn persist_settings(&mut self) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.settings)?;
        std::fs::write(&self.settings_path, json)?;
        // 刚落盘的就是内存里这份设置，从前的读取失败已经翻篇（写失败时不会走到这里）。
        self.settings_load_error = None;
        Ok(())
    }

    pub fn persist_app_settings(&mut self) -> Result<()> {
        self.sync_storage_root()
    }
}

#[cfg(test)]
impl AppState {
    /// 单测用的最小状态：数据库用调用方给的连接（`db::open(临时目录)` 已建好 schema），
    /// 其余字段取默认，不碰真实数据目录。`settings_path` 是私有字段，构造器只能放这里。
    pub fn for_tests(conn: Connection) -> Self {
        Self {
            settings: Default::default(),
            app_settings: Default::default(),
            blueprints: Vec::new(),
            generation_running: false,
            last_error: None,
            active_project_id: None,
            project_dir: PathBuf::from("."),
            db: Arc::new(Mutex::new(conn)),
            settings_path: PathBuf::from("settings.json"),
            settings_load_error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// 沙箱把两处路径都收进 `HUANSHU_HOME` 下，真机验证因此不会碰到用户的真实项目。
    #[test]
    fn a_sandbox_home_redirects_both_paths_under_itself() {
        let paths = storage_paths(Some(Path::new("E:/huanshu/target/sandbox-home")));
        assert_eq!(
            paths.data_dir,
            PathBuf::from("E:/huanshu/target/sandbox-home/data")
        );
        assert_eq!(
            paths.bootstrap,
            PathBuf::from("E:/huanshu/target/sandbox-home/bootstrap.json")
        );
    }

    /// 没设 `HUANSHU_HOME` 时行为必须与从前逐字相同，否则会悄悄改掉用户的数据位置。
    #[test]
    fn without_a_sandbox_the_system_directories_are_used() {
        let paths = storage_paths(None);
        assert!(paths.data_dir.ends_with("auto-slidev-studio"), "{paths:?}");
        assert_eq!(
            paths.bootstrap.file_name().unwrap().to_str().unwrap(),
            "bootstrap.json"
        );
        // 沙箱的 data/ 目录名不能漏进系统路径。
        assert_ne!(
            paths.data_dir.file_name().unwrap().to_str().unwrap(),
            "data"
        );
    }

    #[test]
    fn an_empty_home_falls_back_to_the_system_directories() {
        assert_eq!(
            storage_paths(Some(Path::new(""))),
            storage_paths(None),
            "an empty HUANSHU_HOME is treated as unset"
        );
    }

    /// 文件不存在 = 第一次运行（不算失败）；文件在但坏了 = 失败，且必须留下原因：
    /// 界面靠它判定「设置不可信」，否则会把默认值当成用户的配置写回去。
    #[test]
    fn a_corrupt_settings_file_is_a_failure_but_a_missing_one_is_not() {
        let dir = std::env::temp_dir().join("huanshu-core-settings-read");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");

        let (settings, error) = read_settings_file(&path);
        assert_eq!(settings.repair_rounds, LlmSettings::default().repair_rounds);
        assert!(error.is_none(), "{error:?}");

        std::fs::write(&path, "{ this is not json").unwrap();
        let (_, error) = read_settings_file(&path);
        let error = error.expect("损坏的 settings.json 必须报错");
        assert!(error.contains("settings.json"), "{error}");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 「重新加载」：坏文件报错并记住原因，修好后能读回真实设置并清掉失败原因。
    #[test]
    fn reloading_settings_recovers_after_the_file_is_fixed() {
        let dir = std::env::temp_dir().join("huanshu-core-settings-reload");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let conn = db::open(&dir).unwrap();
        let mut state = AppState::for_tests(conn);
        state.settings_path = path.clone();

        std::fs::write(&path, "{ broken").unwrap();
        // 没重读过也要能看出磁盘上那份已经坏了：设置页打开时就靠这个判定失败态。
        assert!(state.settings_file_error().is_some());
        assert!(state.reload_settings().is_err());
        assert!(state.settings_load_error().is_some());

        let mut fixed = LlmSettings::default();
        fixed.repair_rounds = 7;
        std::fs::write(&path, serde_json::to_string(&fixed).unwrap()).unwrap();
        assert!(state.settings_file_error().is_none());
        let loaded = state.reload_settings().unwrap();
        assert_eq!(loaded.repair_rounds, 7);
        assert_eq!(state.settings.repair_rounds, 7);
        assert!(state.settings_load_error().is_none());

        // 保存成功也算翻篇：刚落盘的就是内存里这份设置。
        state.settings_load_error = Some("stale".to_string());
        state.persist_settings().unwrap();
        assert!(state.settings_load_error().is_none());

        drop(state);
        std::fs::remove_dir_all(&dir).ok();
    }
}
