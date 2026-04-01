use crate::db::{self, Project, ProjectSummary};
use crate::types::SlideBlueprint;
use crate::AppState;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

#[tauri::command]
pub fn list_projects(state: State<'_, Mutex<AppState>>) -> Result<Vec<ProjectSummary>, String> {
    let st = state.lock().unwrap();
    let db = st.db.lock().unwrap();
    db::list_projects(&*db).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_project(state: State<'_, Mutex<AppState>>, id: i64) -> Result<Project, String> {
    let st = state.lock().unwrap();
    let db = st.db.lock().unwrap();
    db::get_project(&*db, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_project(
    state: State<'_, Mutex<AppState>>,
    name: String,
    md_content: String,
) -> Result<i64, String> {
    let st = state.lock().unwrap();
    let db = st.db.lock().unwrap();
    db::create_project(&*db, &name, &md_content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_project_content(
    state: State<'_, Mutex<AppState>>,
    id: i64,
    name: String,
    md_content: String,
) -> Result<(), String> {
    let st = state.lock().unwrap();
    let db = st.db.lock().unwrap();
    db::update_project_content(&*db, id, &name, &md_content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_project_blueprints(
    state: State<'_, Mutex<AppState>>,
    id: i64,
    blueprints_json: String,
) -> Result<(), String> {
    let st = state.lock().unwrap();
    let db = st.db.lock().unwrap();
    db::update_project_blueprints(&*db, id, &blueprints_json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_project(state: State<'_, Mutex<AppState>>, id: i64) -> Result<(), String> {
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
#[tauri::command]
pub fn open_project(state: State<'_, Mutex<AppState>>, id: i64) -> Result<Project, String> {
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
#[tauri::command]
pub fn set_active_project(state: State<'_, Mutex<AppState>>, id: i64) -> Result<(), String> {
    let mut st = state.lock().unwrap();
    st.active_project_id = Some(id);
    Ok(())
}
