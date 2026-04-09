use crate::input::HeadingLevel;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelServiceSettings {
    pub base_url: String,
    /// Optional API key sent as `Authorization: Bearer <key>`.
    /// Leave empty for local providers (LM Studio, Ollama) that don't require auth.
    #[serde(default)]
    pub api_key: String,
    pub model: String,
}

impl ModelServiceSettings {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }
}

/// LLM settings persisted to disk via tauri-plugin-store
#[derive(Debug, Clone, Serialize)]
pub struct LlmSettings {
    pub llm: ModelServiceSettings,
    pub embedding: ModelServiceSettings,
    pub multimodal: ModelServiceSettings,
    pub repair_rounds: usize,
    pub concurrency: usize,
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            llm: ModelServiceSettings::new(
                "http://127.0.0.1:1234",
                "",
                "qwen/qwen3.5-9b",
            ),
            embedding: ModelServiceSettings::new(
                "http://127.0.0.1:1234",
                "",
                "text-embedding-bge-m3",
            ),
            multimodal: ModelServiceSettings::new(
                "http://127.0.0.1:1234",
                "",
                "qwen/qwen2.5-vl-7b-instruct",
            ),
            repair_rounds: 4,
            concurrency: 4,
        }
    }
}

impl<'de> Deserialize<'de> for LlmSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct LegacySettings {
            #[serde(default)]
            base_url: Option<String>,
            #[serde(default)]
            api_key: Option<String>,
            #[serde(default)]
            model: Option<String>,
            #[serde(default)]
            embedding_model: Option<String>,
            #[serde(default)]
            llm: Option<ModelServiceSettings>,
            #[serde(default)]
            embedding: Option<ModelServiceSettings>,
            #[serde(default)]
            multimodal: Option<ModelServiceSettings>,
            #[serde(default)]
            repair_rounds: Option<usize>,
            #[serde(default)]
            concurrency: Option<usize>,
        }

        let raw = LegacySettings::deserialize(deserializer)?;
        let mut settings = LlmSettings::default();

        if let Some(llm) = raw.llm {
            settings.llm = llm;
        }
        if let Some(embedding) = raw.embedding {
            settings.embedding = embedding;
        }
        if let Some(multimodal) = raw.multimodal {
            settings.multimodal = multimodal;
        }

        if let Some(base_url) = raw.base_url {
            settings.llm.base_url = base_url.clone();
            settings.embedding.base_url = base_url.clone();
            settings.multimodal.base_url = base_url;
        }
        if let Some(api_key) = raw.api_key {
            settings.llm.api_key = api_key.clone();
            settings.embedding.api_key = api_key.clone();
            settings.multimodal.api_key = api_key;
        }
        if let Some(model) = raw.model {
            settings.llm.model = model.clone();
            if settings.multimodal.model == LlmSettings::default().multimodal.model {
                settings.multimodal.model = model;
            }
        }
        if let Some(embedding_model) = raw.embedding_model {
            settings.embedding.model = embedding_model;
        }
        if let Some(repair_rounds) = raw.repair_rounds {
            settings.repair_rounds = repair_rounds;
        }
        if let Some(concurrency) = raw.concurrency {
            settings.concurrency = concurrency;
        }

        Ok(settings)
    }
}

/// Runtime configuration passed to generator functions.
/// Replaces the old CLI Args struct.
#[derive(Debug, Clone)]
pub struct GenerationConfig {
    pub llm: ModelServiceSettings,
    pub embedding: ModelServiceSettings,
    pub multimodal: ModelServiceSettings,
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
            llm: settings.llm.clone(),
            embedding: settings.embedding.clone(),
            multimodal: settings.multimodal.clone(),
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

    pub fn llm_client(&self) -> crate::lmstudio::LmStudioClient {
        crate::lmstudio::LmStudioClient::new(&self.llm.base_url).with_api_key(&self.llm.api_key)
    }

    pub fn embedding_client(&self) -> crate::lmstudio::LmStudioClient {
        crate::lmstudio::LmStudioClient::new(&self.embedding.base_url)
            .with_api_key(&self.embedding.api_key)
    }

    pub fn multimodal_client(&self) -> crate::lmstudio::LmStudioClient {
        crate::lmstudio::LmStudioClient::new(&self.multimodal.base_url)
            .with_api_key(&self.multimodal.api_key)
    }
}
