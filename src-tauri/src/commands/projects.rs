//! 项目相关的 Tauri 命令：只取参数、锁状态、转错误。实现见 `huanshu_core::services::projects`。

use tauri::State;

use super::Shared;

use huanshu_core::db::{Project, ProjectSummary};
use huanshu_core::types::AspectRatio;

use huanshu_core::services::projects as service;

#[tauri::command]
pub fn list_projects(state: State<'_, Shared>) -> Result<Vec<ProjectSummary>, String> {
    service::list_projects(&state)
}

#[tauri::command]
pub fn get_project(state: State<'_, Shared>, id: i64) -> Result<Project, String> {
    service::get_project(&state, id)
}

#[tauri::command]
pub fn create_project(
    state: State<'_, Shared>,
    name: String,
    md_content: String,
) -> Result<i64, String> {
    service::create_project(&state, name, md_content)
}

#[tauri::command]
pub fn update_project_content(
    state: State<'_, Shared>,
    id: i64,
    name: String,
    md_content: String,
) -> Result<(), String> {
    service::update_project_content(&state, id, name, md_content)
}

#[tauri::command]
pub fn update_project_blueprints(
    state: State<'_, Shared>,
    id: i64,
    blueprints_json: String,
) -> Result<(), String> {
    service::update_project_blueprints(&state, id, blueprints_json)
}

#[tauri::command]
pub fn delete_project(state: State<'_, Shared>, id: i64) -> Result<(), String> {
    service::delete_project(&state, id)
}

#[tauri::command]
pub fn open_project(state: State<'_, Shared>, id: i64) -> Result<Project, String> {
    service::open_project(&state, id)
}

#[tauri::command]
pub fn set_active_project(state: State<'_, Shared>, id: i64) -> Result<(), String> {
    service::set_active_project(&state, id)
}

#[tauri::command]
pub fn update_project_aspect_ratio(
    state: State<'_, Shared>,
    id: i64,
    aspect_ratio: AspectRatio,
) -> Result<(), String> {
    service::update_project_aspect_ratio(&state, id, aspect_ratio)
}

#[tauri::command]
pub fn get_project_aspect_ratio(
    state: State<'_, Shared>,
    id: i64,
) -> Result<Option<AspectRatio>, String> {
    service::get_project_aspect_ratio(&state, id)
}
