/// Tauri 管理的状态句柄：core 的 `AppState` 外面套一层可跨线程共享的锁
/// （`Arc` 让 `spawn_blocking` 的闭包能带走它）。
pub type Shared = std::sync::Arc<std::sync::Mutex<huanshu_core::AppState>>;

pub mod editor;
pub mod generate;
pub mod media;
pub mod pdf;
pub mod projects;
pub mod settings;
pub mod storage;
pub mod thumbnails;
