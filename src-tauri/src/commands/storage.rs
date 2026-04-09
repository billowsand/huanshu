use crate::crypto::{CryptoService, validate_password_strength};
use crate::db::{self, MediaAsset};
use crate::types::AppGlobalSettings;
use crate::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

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

fn remap_media_refs_in_json(
    blueprints_json: &str,
    media_id_map: &std::collections::HashMap<i64, i64>,
) -> Result<String, String> {
    if blueprints_json.trim().is_empty() || media_id_map.is_empty() {
        return Ok(blueprints_json.to_string());
    }

    fn rewrite_value(value: &mut serde_json::Value, media_id_map: &std::collections::HashMap<i64, i64>) {
        match value {
            serde_json::Value::String(text) => {
                if let Some(old_id) = text.strip_prefix("media:").and_then(|id| id.parse::<i64>().ok()) {
                    if let Some(new_id) = media_id_map.get(&old_id) {
                        *text = format!("media:{new_id}");
                    }
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    rewrite_value(item, media_id_map);
                }
            }
            serde_json::Value::Object(map) => {
                for value in map.values_mut() {
                    rewrite_value(value, media_id_map);
                }
            }
            _ => {}
        }
    }

    let mut value: serde_json::Value = serde_json::from_str(blueprints_json).map_err(|e| e.to_string())?;
    rewrite_value(&mut value, media_id_map);
    serde_json::to_string(&value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_app_settings(state: State<'_, Mutex<AppState>>) -> Result<AppGlobalSettings, String> {
    let state = state.lock().unwrap();
    Ok(state.app_settings.clone())
}

#[tauri::command]
pub fn save_app_settings(
    settings: AppGlobalSettings,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let mut state = state.lock().unwrap();
    state.app_settings = settings;
    state.persist_app_settings().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn is_first_run(state: State<'_, Mutex<AppState>>) -> Result<bool, String> {
    let state = state.lock().unwrap();
    let db = state.db.lock().unwrap();
    db::is_first_run(&*db).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn complete_first_run(
    llm_configured: bool,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let mut state = state.lock().unwrap();
    state.app_settings.llm_configured = llm_configured;
    state.persist_app_settings().map_err(|e| e.to_string())?;
    let db = state.db.lock().unwrap();
    db::mark_first_run_done(&*db).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_media_asset(
    req: ImportMediaRequest,
    state: State<'_, Mutex<AppState>>,
) -> Result<MediaAssetResponse, String> {
    let (asset, full_path, media_dir, llm_configured, service) = {
        let state = state.lock().unwrap();
        let media_dir = state.app_settings.media_dir.clone();
        let media_dir_path = PathBuf::from(&media_dir);
        let subdir = if req.media_type == "video" { "videos" } else { "images" };

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
        let caption = generate_caption_for_image_internal(&service, &full_path).await;
        if let Ok(caption) = caption {
            final_asset.caption = Some(caption);
            let state = state.lock().unwrap();
            let db = state.db.lock().unwrap();
            let _ = db::update_media_asset_caption(&*db, final_asset.id, final_asset.caption.as_deref(), None);
        }
    }

    Ok(MediaAssetResponse::from_asset(final_asset, &media_dir))
}

async fn generate_caption_for_image_internal(
    service: &crate::config::ModelServiceSettings,
    image_path: &PathBuf,
) -> Result<String, String> {
    let client = crate::lmstudio::LmStudioClient::new(&service.base_url)
        .with_api_key(&service.api_key);

    client
        .generate_image_caption(&service.model, &image_path.to_string_lossy())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_media_asset(id: i64, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
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

#[tauri::command]
pub fn get_project_media(
    project_id: i64,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<MediaAssetResponse>, String> {
    let state = state.lock().unwrap();
    let db = state.db.lock().unwrap();
    let media_dir = state.app_settings.media_dir.clone();
    let assets = db::get_media_assets_for_project(&*db, project_id).map_err(|e| e.to_string())?;
    Ok(assets.into_iter().map(|a| MediaAssetResponse::from_asset(a, &media_dir)).collect())
}

#[tauri::command]
pub fn update_media_caption(
    id: i64,
    caption: Option<String>,
    description: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let state = state.lock().unwrap();
    let db = state.db.lock().unwrap();
    db::update_media_asset_caption(&*db, id, caption.as_deref(), description.as_deref())
        .map_err(|e| e.to_string())
}

#[derive(Debug, Serialize, Deserialize)]
struct KeynnManifest {
    version: String,
    project_name: String,
    exported_at: i64,
}

#[tauri::command]
pub fn export_project(
    project_id: i64,
    state: State<'_, Mutex<AppState>>,
    encrypted: Option<bool>,
    password: Option<String>,
) -> Result<Vec<u8>, String> {
    let do_encrypt = encrypted.unwrap_or(false);
    
    let pwd = if do_encrypt {
        let p = password.ok_or("Password required for encrypted export")?;
        validate_password_strength(&p)?;
        p
    } else {
        String::new()
    };

    let state = state.lock().unwrap();
    let db = state.db.lock().unwrap();

    let project = db::get_project(&*db, project_id).map_err(|e| e.to_string())?;
    let assets = db::get_media_assets_for_project(&*db, project_id).map_err(|e| e.to_string())?;

    let media_dir = PathBuf::from(&state.app_settings.media_dir);

    let manifest = KeynnManifest {
        version: "1.0".to_string(),
        project_name: project.name.clone(),
        exported_at: now_ts(),
    };

    let mut zip_data: Vec<u8> = Vec::new();
    let mut zip = ZipWriter::new(std::io::Cursor::new(&mut zip_data));

    zip.start_file("manifest.json", SimpleFileOptions::default())
        .map_err(|e| e.to_string())?;
    zip.write_all(serde_json::to_string_pretty(&manifest).unwrap().as_bytes())
        .map_err(|e| e.to_string())?;

    zip.start_file("content/project.json", SimpleFileOptions::default())
        .map_err(|e| e.to_string())?;
    let project_json = serde_json::to_string_pretty(&project).map_err(|e| e.to_string())?;
    zip.write_all(project_json.as_bytes())
        .map_err(|e| e.to_string())?;

    zip.start_file("content/blueprints.json", SimpleFileOptions::default())
        .map_err(|e| e.to_string())?;
    zip.write_all(project.blueprints_json.as_bytes())
        .map_err(|e| e.to_string())?;

    let media_metadata: Vec<serde_json::Value> = assets.iter().map(|a| {
        serde_json::json!({
            "id": a.id,
            "filename": a.filename,
            "original_name": a.original_name,
            "media_type": a.media_type,
            "mime_type": a.mime_type,
            "storage_path": a.storage_path,
            "caption": a.caption,
            "description": a.description,
            "slide_index": a.slide_index,
            "file_size": a.file_size,
        })
    }).collect();
    zip.start_file("content/media.json", SimpleFileOptions::default())
        .map_err(|e| e.to_string())?;
    zip.write_all(serde_json::to_string_pretty(&serde_json::json!(media_metadata)).unwrap().as_bytes())
        .map_err(|e| e.to_string())?;

    for asset in &assets {
        let full_path = media_dir.join(&asset.storage_path);
        if full_path.exists() {
            let subdir = if asset.media_type == "video" { "videos" } else { "images" };
            let entry_path = format!("content/media/{}/{}", subdir, asset.filename);
            zip.start_file(&entry_path, SimpleFileOptions::default())
                .map_err(|e| e.to_string())?;
            let mut f = fs::File::open(&full_path).map_err(|e| e.to_string())?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer).map_err(|e| e.to_string())?;
            zip.write_all(&buffer).map_err(|e| e.to_string())?;
        }
    }

    zip.finish().map_err(|e| e.to_string())?;

    if do_encrypt {
        let original_name = format!("{}.keynn", project.name.replace(" ", "_"));
        CryptoService::encrypt(&zip_data, &pwd, &original_name)
    } else {
        Ok(zip_data)
    }
}

#[tauri::command]
pub fn import_project(
    zip_data: Vec<u8>,
    state: State<'_, Mutex<AppState>>,
    password: Option<String>,
) -> Result<i64, String> {
    let data = if CryptoService::is_encrypted(&zip_data) {
        let pwd = password.ok_or("Password required for encrypted file")?;
        CryptoService::decrypt(&zip_data, &pwd)?
    } else {
        zip_data
    };

    let state = state.lock().unwrap();
    let db = state.db.lock().unwrap();

    let cursor = std::io::Cursor::new(data);
    let mut archive = ZipArchive::new(cursor).map_err(|e| e.to_string())?;

    let _manifest: KeynnManifest = {
        let mut manifest_str = String::new();
        archive
            .by_name("manifest.json")
            .map_err(|e| e.to_string())?
            .read_to_string(&mut manifest_str)
            .map_err(|e| e.to_string())?;
        serde_json::from_str(&manifest_str).map_err(|e| e.to_string())?
    };

    let mut project_json_str = String::new();
    archive
        .by_name("content/project.json")
        .map_err(|e| e.to_string())?
        .read_to_string(&mut project_json_str)
        .map_err(|e| e.to_string())?;
    let project_json: serde_json::Value =
        serde_json::from_str(&project_json_str).map_err(|e| e.to_string())?;

    let name = project_json["name"]
        .as_str()
        .ok_or("invalid project name")?
        .to_string();

    let final_name = if db::get_project_by_name(&*db, &name)
        .map_err(|e| e.to_string())?
        .is_some()
    {
        format!("{} (副本)", name)
    } else {
        name
    };

    let md_content = project_json["md_content"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let ts = now_ts();

    db.execute(
        "INSERT INTO projects (name, md_content, blueprints, media_root, created_at, updated_at)
         VALUES (?1, ?2, '[]', '', ?3, ?3)",
        params![final_name, md_content, ts],
    )
    .map_err(|e| e.to_string())?;
    let project_id = db.last_insert_rowid();

    let media_dir = PathBuf::from(&state.app_settings.media_dir);
    let project_media_dir = media_dir.join("projects").join(project_id.to_string());
    fs::create_dir_all(&project_media_dir).map_err(|e| e.to_string())?;

    let mut blueprints_json_str = String::new();
    if let Ok(mut blueprints_file) = archive.by_name("content/blueprints.json") {
        blueprints_file
            .read_to_string(&mut blueprints_json_str)
            .map_err(|e| e.to_string())?;
    }

    let mut media_id_map = std::collections::HashMap::new();

    let media_metadata: std::collections::HashMap<String, serde_json::Value> = {
        let mut media_json_str = String::new();
        if let Ok(mut media_file) = archive.by_name("content/media.json") {
            let _ = media_file.read_to_string(&mut media_json_str);
        }
        let mut map = std::collections::HashMap::new();
        if let Ok(metadata) = serde_json::from_str::<Vec<serde_json::Value>>(&media_json_str) {
            for m in metadata {
                if let Some(fname) = m.get("filename").and_then(|v| v.as_str()) {
                    map.insert(fname.to_string(), m);
                }
            }
        }
        map
    };

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().to_string();

        if name.starts_with("content/media/") && !name.ends_with('/') {
            let relative = name.strip_prefix("content/media/").unwrap();
            let parts: Vec<&str> = relative.split('/').collect();
            if parts.len() >= 2 {
                let subdir = parts[0];
                let filename = parts[1];
                let target_dir = project_media_dir.join(subdir);
                fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
                let target_path = target_dir.join(filename);
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer).map_err(|e| e.to_string())?;
                fs::write(&target_path, buffer).map_err(|e| e.to_string())?;

                let storage_path = format!("projects/{}/{}/{}", project_id, subdir, filename);

                let meta = media_metadata.get(filename);
                let old_media_id = meta.and_then(|m| m.get("id").and_then(|v| v.as_i64()));
                let mime_type = meta.and_then(|m| m.get("mime_type").and_then(|v| v.as_str().map(String::from)));
                let caption = meta.and_then(|m| m.get("caption").and_then(|v| v.as_str().map(String::from)));
                let description = meta.and_then(|m| m.get("description").and_then(|v| v.as_str().map(String::from)));
                let slide_index = meta.and_then(|m| m.get("slide_index").and_then(|v| v.as_i64().map(|n| n as i32)));
                let original_name = meta.and_then(|m| m.get("original_name").and_then(|v| v.as_str().map(String::from)))
                    .unwrap_or_else(|| filename.to_string());

                let asset = MediaAsset {
                    id: 0,
                    project_id,
                    slide_index,
                    filename: filename.to_string(),
                    original_name,
                    media_type: if subdir == "images" {
                        "image".to_string()
                    } else {
                        "video".to_string()
                    },
                    mime_type,
                    storage_path,
                    caption,
                    description,
                    file_size: Some(file.size() as i64),
                    created_at: ts,
                };
                let new_media_id = db::create_media_asset(&*db, &asset).map_err(|e| e.to_string())?;
                if let Some(old_media_id) = old_media_id {
                    media_id_map.insert(old_media_id, new_media_id);
                }
            }
        }
    }

    let blueprints_json_str = remap_media_refs_in_json(&blueprints_json_str, &media_id_map)?;

    db::update_project_blueprints(&*db, project_id, &blueprints_json_str)
        .map_err(|e| e.to_string())?;

    Ok(project_id)
}

#[tauri::command]
pub fn is_encrypted_file(data: Vec<u8>) -> bool {
    CryptoService::is_encrypted(&data)
}
