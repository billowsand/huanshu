//! 幻述的核心库：与界面框架无关的业务逻辑。
//!
//! 这里包含数据库、生成流水线、导入导出（`.keynn`）、图表与版式校验、PDF 与缩略图渲染
//! 所依赖的全部逻辑。Tauri 命令层与将来的 GPUI 界面都只是这一层的薄封装，因此本 crate
//! **不依赖 `tauri` 或任何 `tauri-plugin-*`**（`cargo tree -p huanshu-core | grep -i tauri` 应无输出）。
//!
//! 异步约定：带 `async` 的函数（如 [`generator`] 的流水线、[`lmstudio`] 的请求）要求调用方
//! 提供 tokio 运行时；具体的线程模型（`spawn_blocking`、异步运行时选择）由调用方决定，
//! 本 crate 不引入 `tauri::async_runtime`。

pub mod assets;
pub mod chart_check;
pub mod config;
pub mod crypto;
pub mod db;
pub mod editor_schema;
pub mod embedded_icons;
pub mod events;
pub mod fonts;
pub mod generation_progress;
pub mod generator;
pub mod icon;
pub mod image_size;
pub mod input;
pub mod lmstudio;
pub mod markdown_tools;
pub mod network;
pub mod preview;
pub mod services;
pub mod slide_edit;
pub mod state;
pub mod themes;
pub mod types;
pub mod validate;

pub use state::AppState;
