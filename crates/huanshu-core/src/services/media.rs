//! 素材库：导入、删除、列举与更新图注。
//!
//! 文件落在 `<media_dir>/projects/<project_id>/{images,videos}/`，行记录在 `media_assets` 表。
//! 导入图片时从文件头读出像素尺寸（`image_size`），这样生成时的提示词能说出素材的形状。

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::db::{self, MediaAsset};
use crate::state::SharedState;

use super::Result;

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportMediaRequest {
    pub project_id: i64,
    pub slide_index: Option<i32>,
    pub filename: String,
    pub mime_type: Option<String>,
    pub data: Vec<u8>,
    pub media_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaAssetResponse {
    pub id: i64,
    pub project_id: i64,
    pub slide_index: Option<i32>,
    pub filename: String,
    pub original_name: String,
    pub media_type: String,
    pub mime_type: Option<String>,
    pub storage_path: String,
    pub caption: Option<String>,
    pub description: Option<String>,
    pub file_size: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub created_at: i64,
    pub url: String,
}

impl MediaAssetResponse {
    pub fn from_asset(a: MediaAsset, media_dir: &str) -> Self {
        let full_path = PathBuf::from(media_dir).join(&a.storage_path);
        Self {
            id: a.id,
            project_id: a.project_id,
            slide_index: a.slide_index,
            filename: a.filename.clone(),
            original_name: a.original_name,
            media_type: a.media_type,
            mime_type: a.mime_type,
            storage_path: a.storage_path.clone(),
            caption: a.caption,
            description: a.description,
            file_size: a.file_size,
            width: a.width,
            height: a.height,
            created_at: a.created_at,
            url: full_path.to_string_lossy().to_string(),
        }
    }
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn get_mime_ext(mime: &Option<String>, filename: &str) -> String {
    if let Some(m) = mime {
        let ext = match m.as_str() {
            "image/png" => "png",
            "image/jpeg" | "image/jpg" => "jpg",
            "image/gif" => "gif",
            "image/webp" => "webp",
            "image/svg+xml" => "svg",
            "video/mp4" => "mp4",
            "video/webm" => "webm",
            "video/quicktime" => "mov",
            _ => filename.split('.').last().unwrap_or("bin"),
        };
        ext.to_string()
    } else {
        filename.split('.').last().unwrap_or("bin").to_string()
    }
}

/// Stores the bytes, records the asset, and — when a multimodal model is configured —
/// asks it for a first caption. The placement pass later overwrites that caption with
/// one written in the context of the article.
pub async fn import_media_asset(
    state: &SharedState,
    req: ImportMediaRequest,
) -> Result<MediaAssetResponse> {
    let (asset, full_path, media_dir, llm_configured, service) = {
        let state = state.lock().unwrap();
        let media_dir = state.app_settings.media_dir.clone();
        let media_dir_path = PathBuf::from(&media_dir);
        let subdir = if req.media_type == "video" {
            "videos"
        } else {
            "images"
        };

        let project_media = media_dir_path
            .join("projects")
            .join(req.project_id.to_string())
            .join(subdir);
        fs::create_dir_all(&project_media).map_err(|e| e.to_string())?;

        let ext = get_mime_ext(&req.mime_type, &req.filename);
        let uuid = uuid::Uuid::new_v4().to_string();
        let stored_filename = format!("{}.{}", uuid, ext);
        let storage_path = format!("projects/{}/{}/{}", req.project_id, subdir, stored_filename);

        let full_path = media_dir_path.join(&storage_path);
        fs::write(&full_path, &req.data).map_err(|e| e.to_string())?;

        // Read the pixel size from the file header so the generator can reason about
        // aspect ratio. Unsupported formats simply stay unknown.
        let dimensions = if req.media_type == "image" {
            crate::image_size::image_dimensions(&req.data)
        } else {
            None
        };

        let mut asset = MediaAsset {
            id: 0,
            project_id: req.project_id,
            slide_index: req.slide_index,
            filename: stored_filename,
            original_name: req.filename.clone(),
            media_type: req.media_type.clone(),
            mime_type: req.mime_type.clone(),
            storage_path,
            caption: None,
            description: None,
            file_size: Some(req.data.len() as i64),
            width: dimensions.map(|(w, _)| w as i64),
            height: dimensions.map(|(_, h)| h as i64),
            created_at: now_ts(),
        };

        let db = state.db.lock().unwrap();
        let id = db::create_media_asset(&*db, &asset).map_err(|e| e.to_string())?;
        asset.id = id;

        let llm_configured = state.app_settings.llm_configured;
        let service = state.settings.multimodal.clone();

        (asset, full_path, media_dir, llm_configured, service)
    };

    let mut final_asset = asset;
    if llm_configured && final_asset.media_type == "image" {
        let caption = generate_caption_for_image(&service, &full_path).await;
        if let Ok(caption) = caption {
            final_asset.caption = Some(caption);
            let state = state.lock().unwrap();
            let db = state.db.lock().unwrap();
            let _ = db::update_media_asset_caption(
                &*db,
                final_asset.id,
                final_asset.caption.as_deref(),
                None,
            );
        }
    }

    Ok(MediaAssetResponse::from_asset(final_asset, &media_dir))
}

async fn generate_caption_for_image(
    service: &crate::config::ModelServiceSettings,
    image_path: &PathBuf,
) -> Result<String> {
    let client =
        crate::lmstudio::LmStudioClient::from_service(service).map_err(|e| e.to_string())?;

    client
        .generate_image_caption(&service.model, &image_path.to_string_lossy())
        .await
        .map_err(|e| e.to_string())
}

pub fn delete_media_asset(state: &SharedState, id: i64) -> Result<()> {
    let (storage_path, media_dir) = {
        let state = state.lock().unwrap();
        let db = state.db.lock().unwrap();
        let asset = db::get_media_asset_by_id(&*db, id).map_err(|e| e.to_string())?;
        match asset {
            Some(a) => (Some(a.storage_path), state.app_settings.media_dir.clone()),
            None => (None, String::new()),
        }
    };

    if let Some(path) = storage_path {
        let full_path = PathBuf::from(&media_dir).join(&path);
        let _ = fs::remove_file(full_path);
    }

    let state = state.lock().unwrap();
    let db = state.db.lock().unwrap();
    db::delete_media_asset(&*db, id).map_err(|e| e.to_string())
}

pub fn get_project_media(state: &SharedState, project_id: i64) -> Result<Vec<MediaAssetResponse>> {
    let state = state.lock().unwrap();
    let db = state.db.lock().unwrap();
    let media_dir = state.app_settings.media_dir.clone();
    let assets = db::get_media_assets_for_project(&*db, project_id).map_err(|e| e.to_string())?;
    Ok(assets
        .into_iter()
        .map(|a| MediaAssetResponse::from_asset(a, &media_dir))
        .collect())
}

pub fn update_media_caption(
    state: &SharedState,
    id: i64,
    caption: Option<String>,
    description: Option<String>,
) -> Result<()> {
    let state = state.lock().unwrap();
    let db = state.db.lock().unwrap();
    db::update_media_asset_caption(&*db, id, caption.as_deref(), description.as_deref())
        .map_err(|e| e.to_string())
}
