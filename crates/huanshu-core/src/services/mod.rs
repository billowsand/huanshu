//! 界面层调用的服务：命令层只负责取参数、锁状态、把错误转成 `String`。
//!
//! 每个模块按领域划分，`src-tauri` 的 `#[tauri::command]` 与将来的 GPUI 界面共用同一套实现。

/// 服务层统一以字符串报错：前端只显示它，GPUI 侧同样只显示它。
pub type Result<T, E = String> = std::result::Result<T, E>;

pub mod export;
pub mod generation;
pub mod media;
pub mod projects;
pub mod seed;
pub mod settings;
pub mod storage;
