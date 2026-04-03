use crate::input::HeadingLevel;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// LLM settings persisted to disk via tauri-plugin-store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSettings {
    pub base_url: String,
    /// Optional API key sent as `Authorization: Bearer <key>`.
    /// Leave empty for local providers (LM Studio, Ollama) that don't require auth.
    #[serde(default)]
    pub api_key: String,
    pub model: String,
    pub embedding_model: String,
    pub repair_rounds: usize,
    pub concurrency: usize,
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:1234".into(),
            api_key: String::new(),
            model: "qwen/qwen3.5-9b".into(),
            embedding_model: "text-embedding-bge-m3".into(),
            repair_rounds: 4,
            concurrency: 4,
        }
    }
}

/// Runtime configuration passed to generator functions.
/// Replaces the old CLI Args struct.
#[derive(Debug, Clone)]
pub struct GenerationConfig {
    pub lmstudio_base_url: String,
    pub api_key: String,
    pub model: String,
    pub embedding_model: String,
    pub repair_rounds: usize,
    pub concurrency: usize,
    pub _start_page: usize,
    pub _limit_pages: Option<usize>,
    pub frontmatter_title: Option<String>,
    pub _skip_build_check: bool,
    /// Working directory containing components/, public/, etc.
    pub project_dir: PathBuf,
    /// Debug output directory
    pub debug_dir: PathBuf,
    /// Heading level used as the granularity boundary for slide splitting
    pub granularity: HeadingLevel,
    /// Target aspect ratio for the generated slides
    pub aspect_ratio: crate::types::AspectRatio,
}

impl GenerationConfig {
    pub fn from_settings(
        settings: &LlmSettings,
        project_dir: PathBuf,
        aspect_ratio: crate::types::AspectRatio,
    ) -> Self {
        let debug_dir = project_dir.join(".slidev-gen-debug");
        Self {
            lmstudio_base_url: settings.base_url.clone(),
            api_key: settings.api_key.clone(),
            model: settings.model.clone(),
            embedding_model: settings.embedding_model.clone(),
            repair_rounds: settings.repair_rounds,
            concurrency: settings.concurrency,
            _start_page: 1,
            _limit_pages: None,
            frontmatter_title: None,
            _skip_build_check: true,
            project_dir,
            debug_dir,
            granularity: HeadingLevel::H3,
            aspect_ratio,
        }
    }
}
