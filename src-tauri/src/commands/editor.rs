//! 编辑器模式表与图表校验的命令。实现在 `huanshu_core::editor_schema` 与
//! `huanshu_core::chart_check`——界面与生成流水线共用同一份规则。

use huanshu_core::chart_check::chart_problems_json;
use huanshu_core::editor_schema::{editor_schema, EditorSchema};

/// Every kind's metadata, field list and defaults, plus the form field definitions.
/// The editor fetches this once at startup.
#[tauri::command]
pub fn get_editor_schema() -> EditorSchema {
    editor_schema()
}

/// Structural problems of a hand-edited chart JSON, in Chinese. Empty means renderable.
#[tauri::command]
pub fn chart_problems(chart: serde_json::Value) -> Vec<String> {
    chart_problems_json(&chart)
        .iter()
        .map(huanshu_core::chart_check::ChartProblem::message_zh)
        .collect()
}
