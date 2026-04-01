use crate::db;
use crate::embedded_icons;
use crate::generator::utils::cosine_similarity;
use crate::lmstudio::LmStudioClient;
use anyhow::{Context, Result};
use rusqlite::Connection;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct IconRecord {
    pub full_name: String,
    pub search_text: String,
}

#[derive(Debug, Clone)]
pub struct IconIndex {
    icons: Vec<IconRecord>,
    names: HashSet<String>,
    /// Pre-computed embedding vectors for each icon, aligned with `icons`.
    /// None means not yet embedded.
    icon_embeddings: Option<Vec<Vec<f32>>>,
    embedding_model: Option<String>,
    package_version: Option<String>,
}

impl IconIndex {
    /// Load icon index without embeddings (legacy behavior).
    pub fn load(project_root: &Path) -> Result<Self> {
        Self::load_impl(project_root, false, "", None)
    }

    /// Load icon index with embeddings from DB cache if available.
    pub fn load_with_cache(
        project_root: &Path,
        embedding_model: &str,
        db: &Connection,
    ) -> Result<Self> {
        Self::load_impl(project_root, true, embedding_model, Some(db))
    }

    fn load_impl(
        project_root: &Path,
        with_cache: bool,
        embedding_model: &str,
        db: Option<&Connection>,
    ) -> Result<Self> {
        let (icons, package_version) = match load_icons_from_embedded() {
            Ok(result) => result,
            Err(_) => {
                if let Ok(collections) = discover_icon_collections(project_root) {
                    load_icons_from_filesystem(collections)?
                } else {
                    return Ok(Self {
                        icons: Vec::new(),
                        names: HashSet::new(),
                        icon_embeddings: None,
                        embedding_model: None,
                        package_version: None,
                    });
                }
            }
        };

        // Build names HashSet from icons
        let mut names = HashSet::new();
        for icon in &icons {
            names.insert(icon.full_name.clone());
        }

        // Try to load embeddings from DB
        let icon_embeddings = if with_cache {
            if let Some(conn) = db {
                if let Some(loaded) =
                    db::load_icon_embeddings(conn, embedding_model, &package_version, icons.len())?
                {
                    println!("[icon-index] cache hit: {} icons loaded", loaded.len());
                    Some(loaded)
                } else {
                    println!("[icon-index] cache miss, will embed {} icons", icons.len());
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        Ok(Self {
            icons,
            names,
            icon_embeddings,
            embedding_model: if with_cache {
                Some(embedding_model.to_string())
            } else {
                None
            },
            package_version: Some(package_version),
        })
    }

    /// Embed all icons using the given client and store results in DB.
    /// Returns the number of icons embedded.
    pub async fn embed_all(
        &mut self,
        client: &LmStudioClient,
        embedding_model: &str,
        db: &std::sync::Arc<std::sync::Mutex<Connection>>,
    ) -> Result<usize> {
        if self.icons.is_empty() {
            self.icon_embeddings = Some(Vec::new());
            self.embedding_model = Some(embedding_model.to_string());
            return Ok(0);
        }

        let texts: Vec<String> = self.icons.iter().map(|i| i.search_text.clone()).collect();
        let embeddings = client.embed(embedding_model, &texts).await?;
        if embeddings.len() != texts.len() {
            anyhow::bail!(
                "embed count mismatch: sent {}, got {}",
                texts.len(),
                embeddings.len()
            );
        }

        let icon_tuples: Vec<(String, String)> = self
            .icons
            .iter()
            .map(|i| (i.full_name.clone(), i.search_text.clone()))
            .collect();

        let conn = db.lock().unwrap();
        db::store_icon_embeddings(
            &*conn,
            embedding_model,
            self.package_version.as_deref().unwrap_or(""),
            &icon_tuples,
            &embeddings,
        )?;
        drop(conn);

        self.icon_embeddings = Some(embeddings);
        self.embedding_model = Some(embedding_model.to_string());

        println!("[icon-index] stored {} icon embeddings in DB cache", self.icons.len());
        Ok(self.icons.len())
    }

    pub fn empty() -> Self {
        Self {
            icons: Vec::new(),
            names: HashSet::new(),
            icon_embeddings: None,
            embedding_model: None,
            package_version: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.icons.is_empty()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.names.contains(name)
    }

    pub fn is_embedded(&self) -> bool {
        self.icon_embeddings.is_some()
    }

    /// Perform semantic search given a query embedding.
    /// Returns up to `top_k` icon records sorted by cosine similarity descending.
    pub fn semantic_search_with_emb(
        &self,
        query_emb: &[f32],
        top_k: usize,
    ) -> Vec<(f32, IconRecord)> {
        let Some(embs) = &self.icon_embeddings else {
            return Vec::new();
        };
        if embs.is_empty() {
            return Vec::new();
        }

        let mut scored: Vec<(f32, &IconRecord)> = self
            .icons
            .iter()
            .zip(embs.iter())
            .map(|(icon, emb)| (cosine_similarity(query_emb, emb), icon))
            .collect();

        scored.sort_by(|a, b| match b.0.total_cmp(&a.0) {
            Ordering::Equal => a.1.full_name.cmp(&b.1.full_name),
            other => other,
        });

        scored
            .into_iter()
            .take(top_k)
            .map(|(score, icon)| (score, icon.clone()))
            .collect()
    }

    /// Legacy lexical search — kept for pre-filtering / fallback.
    pub fn top_candidates(&self, query: &str, limit: usize) -> Vec<IconRecord> {
        let query_tokens = tokenize(query);
        let mut scored = self
            .icons
            .iter()
            .map(|icon| {
                let score = lexical_score(&query_tokens, &icon.search_text);
                (score, icon)
            })
            .collect::<Vec<_>>();
        scored.sort_by(|a, b| match b.0.total_cmp(&a.0) {
            Ordering::Equal => a.1.full_name.cmp(&b.1.full_name),
            other => other,
        });
        scored
            .into_iter()
            .filter(|(score, _)| *score > 0.0)
            .take(limit)
            .map(|(_, icon)| icon.clone())
            .collect()
    }
}

/// Load icons from embedded data (baked into binary at compile time).
/// Returns (icons, version) on success.
fn load_icons_from_embedded() -> Result<(Vec<IconRecord>, String)> {
    let json: Value = serde_json::from_slice(embedded_icons::CARBON_ICONS_JSON)
        .context("failed to parse embedded icons JSON")?;

    let prefix = json
        .get("prefix")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let map = json
        .get("icons")
        .and_then(Value::as_object)
        .context("missing 'icons' object in embedded JSON")?;

    let mut icons = Vec::new();
    for key in map.keys() {
        icons.push(IconRecord {
            full_name: format!("i-{prefix}:{key}"),
            search_text: icon_key_to_search_text(&prefix, key),
        });
    }

    icons.sort_by(|a, b| a.full_name.cmp(&b.full_name));

    Ok((icons, embedded_icons::CARBON_VERSION.to_string()))
}

/// Load icons from filesystem (fallback for development).
fn load_icons_from_filesystem(collections: Vec<(PathBuf, String)>) -> Result<(Vec<IconRecord>, String)> {
    let mut icons = Vec::new();
    let mut package_versions: Vec<String> = Vec::new();

    for (path, version) in collections {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read icon json: {}", path.display()))?;
        let json: Value = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse icon json: {}", path.display()))?;
        let prefix = json
            .get("prefix")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let Some(map) = json.get("icons").and_then(Value::as_object) else {
            continue;
        };
        for key in map.keys() {
            icons.push(IconRecord {
                full_name: format!("i-{prefix}:{key}"),
                search_text: icon_key_to_search_text(&prefix, key),
            });
        }
        package_versions.push(version);
    }

    icons.sort_by(|a, b| a.full_name.cmp(&b.full_name));

    Ok((icons, package_versions.join("+")))
}

/// Returns (path, version) for each discovered icon collection.
fn discover_icon_collections(project_root: &Path) -> Result<Vec<(PathBuf, String)>> {
    let mut packages: HashMap<String, String> = HashMap::new();

    // Scan root package.json and known sub-packages for @iconify-json/* deps
    let candidates = [
        project_root.join("package.json"),
        project_root.join("studio").join("package.json"),
    ];
    for pkg_path in &candidates {
        let Ok(raw) = fs::read_to_string(pkg_path) else { continue };
        let Ok(json) = serde_json::from_str::<Value>(&raw) else { continue };
        for key in ["dependencies", "devDependencies"] {
            if let Some(obj) = json.get(key).and_then(Value::as_object) {
                for (pkg, ver) in obj.iter() {
                    if pkg.starts_with("@iconify-json/") {
                        // Store version string for cache key
                        packages.entry(pkg.clone()).or_insert_with(|| {
                            ver.as_str().unwrap_or("unknown").to_string()
                        });
                    }
                }
            }
        }
    }

    if packages.is_empty() {
        anyhow::bail!("no @iconify-json/* packages found in any package.json");
    }

    let mut results: Vec<(PathBuf, String)> = packages
        .iter()
        .flat_map(|(pkg, ver)| {
            let _pkg_basename = pkg.strip_prefix("@iconify-json/").unwrap_or(pkg);
            let search_paths = [
                project_root.join("node_modules").join(pkg),
                project_root.join("studio").join("node_modules").join(pkg),
            ];
            search_paths
                .iter()
                .map(|p| p.join("icons.json"))
                .find(|p| p.is_file())
                .map(|path| (path, ver.clone()))
        })
        .collect();

    results.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(results)
}

fn icon_key_to_search_text(prefix: &str, key: &str) -> String {
    format!(
        "{} {}",
        prefix,
        key.replace('-', " ")
            .replace(':', " ")
            .replace('_', " ")
            .trim()
    )
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .flat_map(|part| part.split(['-', '_']))
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn lexical_score(query_tokens: &[String], candidate: &str) -> f32 {
    if query_tokens.is_empty() {
        return 0.0;
    }
    let candidate_tokens = tokenize(candidate);
    let mut matched = 0.0;
    for token in query_tokens {
        if candidate_tokens.iter().any(|cand| cand == token) {
            matched += 2.0;
        } else if candidate_tokens
            .iter()
            .any(|cand| cand.contains(token) || token.contains(cand))
        {
            matched += 1.0;
        }
    }
    matched / query_tokens.len() as f32
}
