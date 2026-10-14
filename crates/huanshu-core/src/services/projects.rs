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
        // 第一步的附加信息（文件名、粒度、比例）随项目一起删。
        db::delete_app_setting(&*db, &step1_meta_key(id)).map_err(|e| e.to_string())?;
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

/// 第一步的「项目附加信息」（原版 localStorage `auto-slidev:step1-meta:<id>` 的那份 JSON）：
/// 上传的文稿文件名、分页粒度、幻灯片比例。原生版存 core 的 `app_settings` 键值表，
/// 键是 `project_meta:<id>`，值为 JSON，不改表结构。
///
/// 粒度 / 比例存字符串，读回时 [`Step1ProjectMeta::resolve`] 校验（照原版 `loadStep1Meta`：
/// 非法值回落默认，不整份丢弃）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Step1ProjectMeta {
    pub md_filename: String,
    pub granularity: String,
    pub aspect_ratio: String,
}

impl Step1ProjectMeta {
    /// 读回校验（原版 `loadStep1Meta` 的三个白名单）：粒度只认 `auto`/`h2`/`h3`，
    /// 比例只认三种 `ratio_*`，非法值分别回落到调用方给的默认；`md_filename` 只要是字符串就保留。
    pub fn resolve(&self, default_granularity: &str, default_aspect_ratio: &str) -> Self {
        let granularity = match self.granularity.as_str() {
            "auto" | "h2" | "h3" => self.granularity.clone(),
            _ => default_granularity.to_string(),
        };
        let aspect_ratio = match self.aspect_ratio.as_str() {
            "ratio_16x9" | "ratio_32x9" | "ratio_48x9" => self.aspect_ratio.clone(),
            _ => default_aspect_ratio.to_string(),
        };
        Self {
            md_filename: self.md_filename.clone(),
            granularity,
            aspect_ratio,
        }
    }
}

fn step1_meta_key(id: i64) -> String {
    format!("project_meta:{id}")
}

/// 读第一步附加信息。键不存在返回 `None`；JSON 损坏按「没有」处理
/// （原版 `loadStep1Meta` 的 `catch` 后 `removeItem`）。
pub fn get_step1_project_meta(state: &SharedState, id: i64) -> Result<Option<Step1ProjectMeta>> {
    let st = state.lock().unwrap();
    let db = st.db.lock().unwrap();
    let Some(raw) = db::get_app_setting(&*db, &step1_meta_key(id)).map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    match serde_json::from_str(&raw) {
        Ok(meta) => Ok(Some(meta)),
        Err(_) => Ok(None),
    }
}

/// 写第一步附加信息（`app_settings` 键值表，`INSERT OR REPLACE`）。
pub fn set_step1_project_meta(state: &SharedState, id: i64, meta: &Step1ProjectMeta) -> Result<()> {
    let raw = serde_json::to_string(meta).map_err(|e| e.to_string())?;
    let st = state.lock().unwrap();
    let db = st.db.lock().unwrap();
    db::set_app_setting(&*db, &step1_meta_key(id), &raw).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    /// 临时目录 + `db::open`（建好 schema）+ 最小状态，不碰真实数据目录。
    fn test_state() -> crate::state::SharedState {
        let dir =
            std::env::temp_dir().join(format!("huanshu-projects-test-{}", uuid::Uuid::new_v4()));
        let conn = db::open(&dir).expect("open test database");
        std::fs::remove_dir_all(&dir).ok();
        std::sync::Mutex::new(AppState::for_tests(conn))
    }

    #[test]
    fn step1_meta_roundtrips_through_app_settings() {
        let state = test_state();
        let meta = Step1ProjectMeta {
            md_filename: "季度汇报.md".to_string(),
            granularity: "h3".to_string(),
            aspect_ratio: "ratio_32x9".to_string(),
        };
        set_step1_project_meta(&state, 7, &meta).unwrap();
        assert_eq!(get_step1_project_meta(&state, 7).unwrap(), Some(meta));
        // 覆盖写。
        let next = Step1ProjectMeta {
            md_filename: String::new(),
            granularity: "auto".to_string(),
            aspect_ratio: "ratio_16x9".to_string(),
        };
        set_step1_project_meta(&state, 7, &next).unwrap();
        assert_eq!(get_step1_project_meta(&state, 7).unwrap(), Some(next));
    }

    #[test]
    fn deleting_a_project_drops_its_step1_meta() {
        let state = test_state();
        let id = create_project(&state, "临时".to_string(), "# 临时".to_string()).unwrap();
        let meta = Step1ProjectMeta {
            md_filename: "a.md".to_string(),
            granularity: "h2".to_string(),
            aspect_ratio: "ratio_16x9".to_string(),
        };
        set_step1_project_meta(&state, id, &meta).unwrap();
        delete_project(&state, id).unwrap();
        assert_eq!(get_step1_project_meta(&state, id).unwrap(), None);
    }

    #[test]
    fn step1_meta_is_none_for_unknown_project() {
        let state = test_state();
        assert_eq!(get_step1_project_meta(&state, 4242).unwrap(), None);
    }

    #[test]
    fn step1_meta_corrupt_json_reads_as_none() {
        let state = test_state();
        {
            let st = state.lock().unwrap();
            let db = st.db.lock().unwrap();
            db::set_app_setting(&*db, "project_meta:9", "{not json").unwrap();
        }
        assert_eq!(get_step1_project_meta(&state, 9).unwrap(), None);
    }

    #[test]
    fn step1_meta_resolve_falls_back_on_illegal_values() {
        let meta = Step1ProjectMeta {
            md_filename: "a.md".to_string(),
            granularity: "h4".to_string(),
            aspect_ratio: "ratio_21x9".to_string(),
        };
        let resolved = meta.resolve("auto", "ratio_48x9");
        assert_eq!(resolved.md_filename, "a.md");
        assert_eq!(resolved.granularity, "auto");
        assert_eq!(resolved.aspect_ratio, "ratio_48x9");

        let legal = Step1ProjectMeta {
            md_filename: "b.md".to_string(),
            granularity: "h2".to_string(),
            aspect_ratio: "ratio_16x9".to_string(),
        };
        let resolved = legal.resolve("auto", "ratio_48x9");
        assert_eq!(resolved.granularity, "h2");
        assert_eq!(resolved.aspect_ratio, "ratio_16x9");
    }
}
