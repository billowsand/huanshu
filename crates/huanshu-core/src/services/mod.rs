//! 界面层调用的服务：调用方只负责取参数、锁状态、把错误转成 `String`。
//!
//! 每个模块按领域划分；界面层（`apps/huanshu-native`）共用同一套实现。

/// 服务层统一以字符串报错：界面只显示它。
pub type Result<T, E = String> = std::result::Result<T, E>;

pub mod export;
pub mod generation;
pub mod media;
pub mod projects;
pub mod seed;
pub mod settings;
pub mod storage;
