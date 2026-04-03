use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::types::AspectRatio;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: i64,
    pub name: String,
    pub slide_count: usize,
    pub aspect_ratio: Option<AspectRatio>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub md_content: String,
    pub blueprints_json: String,
    pub media_root: Option<String>,
    pub aspect_ratio: Option<AspectRatio>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRun {
    pub id: i64,
    pub project_id: Option<i64>,
    pub status: String,
    pub current_stage: Option<String>,
    pub source_markdown: String,
    pub frontmatter_title: Option<String>,
    pub debug_dir: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationLogEntry {
    pub id: i64,
    pub run_id: i64,
    pub seq: i64,
    pub slide_index: Option<i32>,
    pub stage: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub detail: serde_json::Value,
    pub important: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaAsset {
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
}

pub fn open(data_dir: &Path) -> Result<Connection> {
    std::fs::create_dir_all(data_dir)?;
    let db_path = data_dir.join("projects.db");
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS projects (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            md_content  TEXT NOT NULL DEFAULT '',
            blueprints  TEXT NOT NULL DEFAULT '[]',
            media_root  TEXT,
            created_at  INTEGER NOT NULL,
            updated_at  INTEGER NOT NULL
        );",
    )?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS icon_embeddings (
            icon_name         TEXT NOT NULL,
            embedding_model   TEXT NOT NULL,
            package_version   TEXT NOT NULL,
            embedding         BLOB NOT NULL,
            PRIMARY KEY (icon_name, embedding_model, package_version)
        );",
    )?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS generation_runs (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id        INTEGER,
            status            TEXT NOT NULL,
            current_stage     TEXT,
            source_markdown   TEXT NOT NULL DEFAULT '',
            frontmatter_title TEXT,
            debug_dir         TEXT NOT NULL DEFAULT '',
            created_at        INTEGER NOT NULL,
            updated_at        INTEGER NOT NULL,
            finished_at       INTEGER,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE SET NULL
        );",
    )?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS generation_logs (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id     INTEGER NOT NULL,
            seq        INTEGER NOT NULL,
            slide_index INTEGER,
            stage      TEXT NOT NULL,
            kind       TEXT NOT NULL,
            title      TEXT NOT NULL DEFAULT '',
            summary    TEXT NOT NULL DEFAULT '',
            detail_json TEXT NOT NULL DEFAULT '{}',
            important  INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (run_id) REFERENCES generation_runs(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_generation_runs_project_created
            ON generation_runs(project_id, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_generation_logs_run_seq
            ON generation_logs(run_id, seq ASC);",
    )?;

    let _ = conn.execute("ALTER TABLE projects ADD COLUMN media_root TEXT", []);
    let _ = conn.execute(
        "ALTER TABLE generation_logs ADD COLUMN slide_index INTEGER",
        [],
    );
    let _ = conn.execute("ALTER TABLE projects ADD COLUMN aspect_ratio TEXT", []);
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS media_assets (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id     INTEGER NOT NULL,
            slide_index    INTEGER,
            filename       TEXT NOT NULL,
            original_name  TEXT NOT NULL,
            media_type     TEXT NOT NULL,
            mime_type      TEXT,
            storage_path   TEXT NOT NULL,
            caption        TEXT,
            description    TEXT,
            file_size      INTEGER,
            created_at     INTEGER NOT NULL,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        );",
    )?;
    Ok(())
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub fn list_projects(conn: &Connection) -> Result<Vec<ProjectSummary>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, blueprints, aspect_ratio, created_at, updated_at
         FROM projects ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        let blueprints_json: String = row.get(2)?;
        let slide_count = serde_json::from_str::<serde_json::Value>(&blueprints_json)
            .ok()
            .and_then(|v| v.as_array().map(|a| a.len()))
            .unwrap_or(0);
        let aspect_ratio_str: Option<String> = row.get(3)?;
        let aspect_ratio =
            aspect_ratio_str.and_then(|s| serde_json::from_str::<AspectRatio>(&s).ok());
        Ok(ProjectSummary {
            id: row.get(0)?,
            name: row.get(1)?,
            slide_count,
            aspect_ratio,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get_project(conn: &Connection, id: i64) -> Result<Project> {
    let p = conn.query_row(
        "SELECT id, name, md_content, blueprints, media_root, aspect_ratio, created_at, updated_at
         FROM projects WHERE id = ?1",
        params![id],
        |row| {
            let aspect_ratio_str: Option<String> = row.get(5)?;
            let aspect_ratio =
                aspect_ratio_str.and_then(|s| serde_json::from_str::<AspectRatio>(&s).ok());
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                md_content: row.get(2)?,
                blueprints_json: row.get(3)?,
                media_root: row.get(4)?,
                aspect_ratio,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        },
    )?;
    Ok(p)
}

pub fn create_project(conn: &Connection, name: &str, md_content: &str) -> Result<i64> {
    let ts = now_ts();
    conn.execute(
        "INSERT INTO projects (name, md_content, blueprints, created_at, updated_at)
         VALUES (?1, ?2, '[]', ?3, ?3)",
        params![name, md_content, ts],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_project_content(
    conn: &Connection,
    id: i64,
    name: &str,
    md_content: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE projects SET name=?1, md_content=?2, updated_at=?3 WHERE id=?4",
        params![name, md_content, now_ts(), id],
    )?;
    Ok(())
}

pub fn update_project_blueprints(conn: &Connection, id: i64, blueprints_json: &str) -> Result<()> {
    conn.execute(
        "UPDATE projects SET blueprints=?1, updated_at=?2 WHERE id=?3",
        params![blueprints_json, now_ts(), id],
    )?;
    Ok(())
}

pub fn update_project_media_root(conn: &Connection, id: i64, media_root: &str) -> Result<()> {
    conn.execute(
        "UPDATE projects SET media_root=?1, updated_at=?2 WHERE id=?3",
        params![media_root, now_ts(), id],
    )?;
    Ok(())
}

pub fn update_project_aspect_ratio(
    conn: &Connection,
    id: i64,
    aspect_ratio: AspectRatio,
) -> Result<()> {
    let ratio_json = serde_json::to_string(&aspect_ratio).map_err(|e| anyhow::anyhow!("{e}"))?;
    conn.execute(
        "UPDATE projects SET aspect_ratio=?1, updated_at=?2 WHERE id=?3",
        params![ratio_json, now_ts(), id],
    )?;
    Ok(())
}

pub fn get_project_by_name(conn: &Connection, name: &str) -> Result<Option<Project>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, md_content, blueprints, media_root, aspect_ratio, created_at, updated_at
         FROM projects WHERE name = ?1 LIMIT 1",
    )?;
    let mut rows = stmt.query(params![name])?;
    if let Some(row) = rows.next()? {
        let aspect_ratio_str: Option<String> = row.get(5)?;
        let aspect_ratio =
            aspect_ratio_str.and_then(|s| serde_json::from_str::<AspectRatio>(&s).ok());
        Ok(Some(Project {
            id: row.get(0)?,
            name: row.get(1)?,
            md_content: row.get(2)?,
            blueprints_json: row.get(3)?,
            media_root: row.get(4)?,
            aspect_ratio,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn delete_project(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM projects WHERE id=?1", params![id])?;
    Ok(())
}

fn map_generation_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<GenerationRun> {
    Ok(GenerationRun {
        id: row.get(0)?,
        project_id: row.get(1)?,
        status: row.get(2)?,
        current_stage: row.get(3)?,
        source_markdown: row.get(4)?,
        frontmatter_title: row.get(5)?,
        debug_dir: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        finished_at: row.get(9)?,
    })
}

pub fn create_generation_run(
    conn: &Connection,
    project_id: Option<i64>,
    source_markdown: &str,
    frontmatter_title: Option<&str>,
    debug_dir: &str,
) -> Result<i64> {
    let ts = now_ts();
    conn.execute(
        "INSERT INTO generation_runs (
            project_id, status, current_stage, source_markdown, frontmatter_title,
            debug_dir, created_at, updated_at
        ) VALUES (?1, 'running', 'start', ?2, ?3, ?4, ?5, ?5)",
        params![
            project_id,
            source_markdown,
            frontmatter_title,
            debug_dir,
            ts
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_generation_run_stage(
    conn: &Connection,
    run_id: i64,
    status: &str,
    current_stage: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE generation_runs
         SET status=?1, current_stage=?2, updated_at=?3
         WHERE id=?4",
        params![status, current_stage, now_ts(), run_id],
    )?;
    Ok(())
}

pub fn finish_generation_run(
    conn: &Connection,
    run_id: i64,
    status: &str,
    current_stage: Option<&str>,
) -> Result<()> {
    let ts = now_ts();
    conn.execute(
        "UPDATE generation_runs
         SET status=?1, current_stage=?2, updated_at=?3, finished_at=?3
         WHERE id=?4",
        params![status, current_stage, ts, run_id],
    )?;
    Ok(())
}

pub fn append_generation_log(
    conn: &Connection,
    run_id: i64,
    slide_index: Option<i32>,
    stage: &str,
    kind: &str,
    title: &str,
    summary: &str,
    detail: &serde_json::Value,
    important: bool,
) -> Result<GenerationLogEntry> {
    let detail_json = serde_json::to_string(detail)?;
    let seq = conn.query_row(
        "SELECT COALESCE(MAX(seq), 0) + 1 FROM generation_logs WHERE run_id=?1",
        params![run_id],
        |row| row.get::<_, i64>(0),
    )?;
    let ts = now_ts();
    conn.execute(
        "INSERT INTO generation_logs (
            run_id, seq, slide_index, stage, kind, title, summary, detail_json, important, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            run_id,
            seq,
            slide_index,
            stage,
            kind,
            title,
            summary,
            detail_json,
            if important { 1 } else { 0 },
            ts
        ],
    )?;
    Ok(GenerationLogEntry {
        id: conn.last_insert_rowid(),
        run_id,
        seq,
        slide_index,
        stage: stage.to_string(),
        kind: kind.to_string(),
        title: title.to_string(),
        summary: summary.to_string(),
        detail: detail.clone(),
        important,
        created_at: ts,
    })
}

pub fn get_latest_generation_run(
    conn: &Connection,
    project_id: i64,
) -> Result<Option<GenerationRun>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, status, current_stage, source_markdown, frontmatter_title,
                debug_dir, created_at, updated_at, finished_at
         FROM generation_runs
         WHERE project_id = ?1
         ORDER BY created_at DESC
         LIMIT 1",
    )?;
    let mut rows = stmt.query(params![project_id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_generation_run(row)?))
    } else {
        Ok(None)
    }
}

pub fn get_generation_logs(conn: &Connection, run_id: i64) -> Result<Vec<GenerationLogEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, run_id, seq, slide_index, stage, kind, title, summary, detail_json, important, created_at
         FROM generation_logs
         WHERE run_id = ?1
         ORDER BY seq ASC",
    )?;
    let rows = stmt.query_map(params![run_id], |row| {
        let detail_json: String = row.get(8)?;
        Ok(GenerationLogEntry {
            id: row.get(0)?,
            run_id: row.get(1)?,
            seq: row.get(2)?,
            slide_index: row.get(3)?,
            stage: row.get(4)?,
            kind: row.get(5)?,
            title: row.get(6)?,
            summary: row.get(7)?,
            detail: serde_json::from_str(&detail_json).unwrap_or(serde_json::Value::Null),
            important: row.get::<_, i64>(9)? != 0,
            created_at: row.get(10)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

// ---------------------------------------------------------------------------
// App settings (key-value store)
// ---------------------------------------------------------------------------

pub fn get_app_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM app_settings WHERE key = ?1")?;
    let mut rows = stmt.query(params![key])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row.get(0)?))
    } else {
        Ok(None)
    }
}

pub fn set_app_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

pub fn is_first_run(conn: &Connection) -> Result<bool> {
    let configured = get_app_setting(conn, "storage_configured")?;
    Ok(configured.is_none())
}

pub fn mark_first_run_done(conn: &Connection) -> Result<()> {
    set_app_setting(conn, "storage_configured", "true")
}

// ---------------------------------------------------------------------------
// Media assets
// ---------------------------------------------------------------------------

pub fn create_media_asset(conn: &Connection, asset: &MediaAsset) -> Result<i64> {
    conn.execute(
        "INSERT INTO media_assets (project_id, slide_index, filename, original_name, media_type,
         mime_type, storage_path, caption, description, file_size, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            asset.project_id,
            asset.slide_index,
            asset.filename,
            asset.original_name,
            asset.media_type,
            asset.mime_type,
            asset.storage_path,
            asset.caption,
            asset.description,
            asset.file_size,
            asset.created_at,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_media_assets_for_project(conn: &Connection, project_id: i64) -> Result<Vec<MediaAsset>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, slide_index, filename, original_name, media_type,
         mime_type, storage_path, caption, description, file_size, created_at
         FROM media_assets WHERE project_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map(params![project_id], |row| {
        Ok(MediaAsset {
            id: row.get(0)?,
            project_id: row.get(1)?,
            slide_index: row.get(2)?,
            filename: row.get(3)?,
            original_name: row.get(4)?,
            media_type: row.get(5)?,
            mime_type: row.get(6)?,
            storage_path: row.get(7)?,
            caption: row.get(8)?,
            description: row.get(9)?,
            file_size: row.get(10)?,
            created_at: row.get(11)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn update_media_asset_caption(
    conn: &Connection,
    id: i64,
    caption: Option<&str>,
    description: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE media_assets SET caption = ?1, description = ?2 WHERE id = ?3",
        params![caption, description, id],
    )?;
    Ok(())
}

pub fn delete_media_asset(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM media_assets WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn get_media_asset_by_id(conn: &Connection, id: i64) -> Result<Option<MediaAsset>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, slide_index, filename, original_name, media_type,
         mime_type, storage_path, caption, description, file_size, created_at
         FROM media_assets WHERE id = ?1 LIMIT 1",
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(MediaAsset {
            id: row.get(0)?,
            project_id: row.get(1)?,
            slide_index: row.get(2)?,
            filename: row.get(3)?,
            original_name: row.get(4)?,
            media_type: row.get(5)?,
            mime_type: row.get(6)?,
            storage_path: row.get(7)?,
            caption: row.get(8)?,
            description: row.get(9)?,
            file_size: row.get(10)?,
            created_at: row.get(11)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn delete_media_assets_for_project(
    conn: &Connection,
    project_id: i64,
) -> Result<Vec<MediaAsset>> {
    let assets = get_media_assets_for_project(conn, project_id)?;
    conn.execute(
        "DELETE FROM media_assets WHERE project_id = ?1",
        params![project_id],
    )?;
    Ok(assets)
}

// ---------------------------------------------------------------------------
// Icon embedding cache
// ---------------------------------------------------------------------------

/// Load icon embeddings from SQLite cache. Returns None if cache miss.
pub fn load_icon_embeddings(
    conn: &Connection,
    embedding_model: &str,
    package_version: &str,
    icon_count: usize,
) -> Result<Option<Vec<Vec<f32>>>> {
    let mut stmt = conn.prepare(
        "SELECT icon_name, embedding FROM icon_embeddings
         WHERE embedding_model=?1 AND package_version=?2
         ORDER BY icon_name",
    )?;
    let mut rows = stmt.query(params![embedding_model, package_version])?;

    // Build a name->embedding map in the same order icons were loaded
    let mut loaded = Vec::new();
    while let Some(row) = rows.next()? {
        let icon_name: String = row.get(0)?;
        let blob: Vec<u8> = row.get(1)?;
        let embedding: Vec<f32> = bincode::deserialize(&blob)
            .map_err(|e| anyhow::anyhow!("bincode deserialize embedding for {icon_name}: {e}"))?;
        loaded.push((icon_name, embedding));
    }

    if loaded.is_empty() || loaded.len() != icon_count {
        return Ok(None);
    }

    // Sort by icon_name to match IconIndex order, then extract vectors
    loaded.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(Some(loaded.into_iter().map(|(_, v)| v).collect()))
}

/// Store icon embeddings into SQLite cache.
pub fn store_icon_embeddings(
    conn: &Connection,
    embedding_model: &str,
    package_version: &str,
    icons: &[(String, String)], // (full_name, search_text) — ordered by IconIndex
    embeddings: &[Vec<f32>],
) -> Result<()> {
    // Clear old entries for this model+version first
    conn.execute(
        "DELETE FROM icon_embeddings WHERE embedding_model=?1 AND package_version=?2",
        params![embedding_model, package_version],
    )?;

    let mut stmt = conn.prepare(
        "INSERT INTO icon_embeddings (icon_name, embedding_model, package_version, embedding)
         VALUES (?1, ?2, ?3, ?4)",
    )?;

    for ((icon_name, _), embedding) in icons.iter().zip(embeddings.iter()) {
        let blob = bincode::serialize(embedding)
            .map_err(|e| anyhow::anyhow!("bincode serialize embedding for {icon_name}: {e}"))?;
        stmt.execute(params![icon_name, embedding_model, package_version, blob])?;
    }
    Ok(())
}

/// Delete icon embedding cache for a specific model (useful when model changes).
#[allow(dead_code)]
pub fn delete_icon_embeddings(conn: &Connection, embedding_model: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM icon_embeddings WHERE embedding_model=?1",
        params![embedding_model],
    )?;
    Ok(())
}
