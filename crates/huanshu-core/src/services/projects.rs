//! 项目的增删改查与「当前打开哪个项目」。
//!
//! `open_project` 会把牌组读进 [`AppState`](crate::state::AppState)，这样查看器与编辑器
//! 不必再自己读一次数据库。

use std::fs;
use std::path::PathBuf;

use crate::db::{self, Project, ProjectSummary};
use crate::state::SharedState;
use crate::types::{AspectRatio, SlideBlueprint};

use super::Result;

pub fn list_projects(state: &SharedState) -> Result<Vec<ProjectSummary>> {
    let st = state.lock().unwrap();
    let db = st.db.lock().unwrap();
    db::list_projects(&*db).map_err(|e| e.to_string())
}

pub fn get_project(state: &SharedState, id: i64) -> Result<Project> {
    let st = state.lock().unwrap();
    let db = st.db.lock().unwrap();
    db::get_project(&*db, id).map_err(|e| e.to_string())
}

pub fn create_project(state: &SharedState, name: String, md_content: String) -> Result<i64> {
    let st = state.lock().unwrap();
    let db = st.db.lock().unwrap();
    db::create_project(&*db, &name, &md_content).map_err(|e| e.to_string())
}

pub fn update_project_content(
    state: &SharedState,
    id: i64,
    name: String,
    md_content: String,
) -> Result<()> {
    let st = state.lock().unwrap();
    let db = st.db.lock().unwrap();
    db::update_project_content(&*db, id, &name, &md_content).map_err(|e| e.to_string())
}

pub fn update_project_blueprints(
    state: &SharedState,
    id: i64,
    blueprints_json: String,
) -> Result<()> {
    let st = state.lock().unwrap();
    let db = st.db.lock().unwrap();
    db::update_project_blueprints(&*db, id, &blueprints_json).map_err(|e| e.to_string())
}

/// Deletes the project and the media files it owned. Files that are already gone
/// are not an error: the row is what matters.
pub fn delete_project(state: &SharedState, id: i64) -> Result<()> {
    let (media_dir, storage_paths) = {
        let st = state.lock().unwrap();
        let db = st.db.lock().unwrap();
        let media_dir = st.app_settings.media_dir.clone();
        let assets = db::delete_media_assets_for_project(&*db, id).map_err(|e| e.to_string())?;
        (
            media_dir,
            assets
                .into_iter()
                .map(|a| a.storage_path)
                .collect::<Vec<_>>(),
        )
    };

    for path in &storage_paths {
        let full_path = PathBuf::from(&media_dir).join(path);
        let _ = fs::remove_file(full_path);
    }

    {
        let st = state.lock().unwrap();
        let db = st.db.lock().unwrap();
        db::delete_project(&*db, id).map_err(|e| e.to_string())?;
    }

    let project_media_dir = PathBuf::from(&media_dir)
        .join("projects")
        .join(id.to_string());
    let _ = fs::remove_dir_all(project_media_dir);

    Ok(())
}

/// Load a project's blueprints into AppState so the viewer can access them.
/// Returns the project so the frontend can display it.
pub fn open_project(state: &SharedState, id: i64) -> Result<Project> {
    let mut st = state.lock().unwrap();
    let project = {
        let db = st.db.lock().unwrap();
        db::get_project(&*db, id).map_err(|e| e.to_string())?
    };
    let blueprints: Vec<SlideBlueprint> =
        serde_json::from_str(&project.blueprints_json).unwrap_or_default();
    st.blueprints = blueprints;
    st.active_project_id = Some(id);
    Ok(project)
}

/// Set the active project ID (called after creating a new project before generation).
pub fn set_active_project(state: &SharedState, id: i64) -> Result<()> {
    let mut st = state.lock().unwrap();
    st.active_project_id = Some(id);
    Ok(())
}

pub fn update_project_aspect_ratio(
    state: &SharedState,
    id: i64,
    aspect_ratio: AspectRatio,
) -> Result<()> {
    let st = state.lock().unwrap();
    let db = st.db.lock().unwrap();
    db::update_project_aspect_ratio(&*db, id, aspect_ratio).map_err(|e| e.to_string())
}

pub fn get_project_aspect_ratio(state: &SharedState, id: i64) -> Result<Option<AspectRatio>> {
    let st = state.lock().unwrap();
    let db = st.db.lock().unwrap();
    let project = db::get_project(&*db, id).map_err(|e| e.to_string())?;
    Ok(project.aspect_ratio)
}
