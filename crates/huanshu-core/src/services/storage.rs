//! 应用设置、首次运行标记与 `.keynn` 包的导入导出（含加密）。
//!
//! 包格式见「加密导出/导入功能」：`manifest.json` + `content/*`，勾选加密时外面再包一层
//! AES-256-GCM（密码用 Argon2id 派生）。素材库操作在 [`super::media`]。

use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::crypto::{validate_password_strength, CryptoService};
use crate::db;
use crate::state::SharedState;
use crate::types::AppGlobalSettings;

use super::Result;

#[derive(Debug, Serialize, Deserialize)]
struct KeynnManifest {
    version: String,
    project_name: String,
    exported_at: i64,
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub fn get_app_settings(state: &SharedState) -> Result<AppGlobalSettings> {
    let state = state.lock().unwrap();
    Ok(state.app_settings.clone())
}

pub fn save_app_settings(state: &SharedState, mut settings: AppGlobalSettings) -> Result<()> {
    let mut state = state.lock().unwrap();
    settings.fonts.normalize();
    let previous = std::mem::replace(&mut state.app_settings, settings);
    if let Err(error) = state.persist_app_settings() {
        state.app_settings = previous;
        return Err(error.to_string());
    }
    Ok(())
}

pub fn is_first_run(state: &SharedState) -> Result<bool> {
    let state = state.lock().unwrap();
    let db = state.db.lock().unwrap();
    db::is_first_run(&*db).map_err(|e| e.to_string())
}

pub fn complete_first_run(state: &SharedState, llm_configured: bool) -> Result<()> {
    let mut state = state.lock().unwrap();
    state.app_settings.llm_configured = llm_configured;
    state.persist_app_settings().map_err(|e| e.to_string())?;
    let db = state.db.lock().unwrap();
    db::mark_first_run_done(&*db).map_err(|e| e.to_string())
}

/// Rewrites every `media:<id>` reference to the id the imported project received.
fn remap_media_refs_in_json(
    blueprints_json: &str,
    media_id_map: &std::collections::HashMap<i64, i64>,
) -> Result<String> {
    if blueprints_json.trim().is_empty() || media_id_map.is_empty() {
        return Ok(blueprints_json.to_string());
    }

    fn rewrite_value(
        value: &mut serde_json::Value,
        media_id_map: &std::collections::HashMap<i64, i64>,
    ) {
        match value {
            serde_json::Value::String(text) => {
                if let Some(old_id) = text
                    .strip_prefix("media:")
                    .and_then(|id| id.parse::<i64>().ok())
                {
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

    let mut value: serde_json::Value =
        serde_json::from_str(blueprints_json).map_err(|e| e.to_string())?;
    rewrite_value(&mut value, media_id_map);
    serde_json::to_string(&value).map_err(|e| e.to_string())
}

/// Packs a project, its blueprints and its media into a `.keynn` package, optionally
/// encrypted. Returns the file bytes; the caller picks the path.
pub fn export_project(
    state: &SharedState,
    project_id: i64,
    encrypted: Option<bool>,
    password: Option<String>,
) -> Result<Vec<u8>> {
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

    let media_metadata: Vec<serde_json::Value> = assets
        .iter()
        .map(|a| {
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
                "width": a.width,
                "height": a.height,
            })
        })
        .collect();
    zip.start_file("content/media.json", SimpleFileOptions::default())
        .map_err(|e| e.to_string())?;
    zip.write_all(
        serde_json::to_string_pretty(&serde_json::json!(media_metadata))
            .unwrap()
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;

    for asset in &assets {
        let full_path = media_dir.join(&asset.storage_path);
        if full_path.exists() {
            let subdir = if asset.media_type == "video" {
                "videos"
            } else {
                "images"
            };
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

/// Restores a `.keynn` package as a new project. An existing name gets a ` (副本)` suffix.
/// Returns the new project's id.
pub fn import_project(
    state: &SharedState,
    zip_data: Vec<u8>,
    password: Option<String>,
) -> Result<i64> {
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
                fs::write(&target_path, &buffer).map_err(|e| e.to_string())?;

                let storage_path = format!("projects/{}/{}/{}", project_id, subdir, filename);

                let meta = media_metadata.get(filename);
                let old_media_id = meta.and_then(|m| m.get("id").and_then(|v| v.as_i64()));
                let mime_type = meta.and_then(|m| {
                    m.get("mime_type")
                        .and_then(|v| v.as_str().map(String::from))
                });
                let caption =
                    meta.and_then(|m| m.get("caption").and_then(|v| v.as_str().map(String::from)));
                let description = meta.and_then(|m| {
                    m.get("description")
                        .and_then(|v| v.as_str().map(String::from))
                });
                let slide_index = meta.and_then(|m| {
                    m.get("slide_index")
                        .and_then(|v| v.as_i64().map(|n| n as i32))
                });
                // Prefer the packaged metadata, but fall back to reading the restored
                // bytes so packages exported before dimensions existed still get sizes.
                let dimensions = crate::image_size::image_dimensions(&buffer);
                let width = meta
                    .and_then(|m| m.get("width").and_then(|v| v.as_i64()))
                    .or_else(|| dimensions.map(|(w, _)| w as i64));
                let height = meta
                    .and_then(|m| m.get("height").and_then(|v| v.as_i64()))
                    .or_else(|| dimensions.map(|(_, h)| h as i64));
                let original_name = meta
                    .and_then(|m| {
                        m.get("original_name")
                            .and_then(|v| v.as_str().map(String::from))
                    })
                    .unwrap_or_else(|| filename.to_string());

                let asset = db::MediaAsset {
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
                    width,
                    height,
                    created_at: ts,
                };
                let new_media_id =
                    db::create_media_asset(&*db, &asset).map_err(|e| e.to_string())?;
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

pub fn is_encrypted_file(data: &[u8]) -> bool {
    CryptoService::is_encrypted(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_references_follow_the_renumbered_ids() {
        let json = r#"{"kind":"spotlight","images":["media:3","figure/a.png"],"nested":[{"image":"media:9"}]}"#;
        let mut map = std::collections::HashMap::new();
        map.insert(3, 41);
        map.insert(9, 42);
        let rewritten = remap_media_refs_in_json(json, &map).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rewritten).unwrap(),
            serde_json::json!({
                "kind": "spotlight",
                "images": ["media:41", "figure/a.png"],
                "nested": [{ "image": "media:42" }],
            }),
        );
    }

    #[test]
    fn nothing_to_remap_leaves_the_payload_untouched() {
        assert_eq!(
            remap_media_refs_in_json("", &Default::default()).unwrap(),
            ""
        );
        assert_eq!(
            remap_media_refs_in_json("{\"image\":\"media:1\"}", &Default::default()).unwrap(),
            "{\"image\":\"media:1\"}"
        );
    }
}
