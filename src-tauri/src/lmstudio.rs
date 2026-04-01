use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct LmStudioClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

impl LmStudioClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: None,
        }
    }

    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        let k = key.into();
        self.api_key = if k.trim().is_empty() { None } else { Some(k) };
        self
    }

    fn apply_auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(key) => req.bearer_auth(key),
            None => req,
        }
    }

    pub async fn list_models(&self) -> Result<Vec<String>> {
        let resp = self
            .apply_auth(self.client.get(format!("{}/v1/models", self.base_url)))
            .send()
            .await
            .context("failed to call LM Studio models endpoint")?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            bail!("LM Studio models endpoint failed: {status} {body}");
        }
        let parsed: ModelsResponse =
            serde_json::from_str(&body).context("failed to parse models response")?;
        Ok(parsed.data.into_iter().map(|m| m.id).collect())
    }

    pub async fn generate_text(&self, model: &str, system: &str, user: &str) -> Result<String> {
        let payload = json!({
            "model": model,
            "temperature": 0.2,
            "response_format": { "type": "text" },
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ]
        });
        let resp = self
            .apply_auth(self.client.post(format!("{}/v1/chat/completions", self.base_url)))
            .json(&payload)
            .send()
            .await
            .context("failed to call LM Studio chat completions")?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            bail!("LM Studio chat completions failed: {status} {body}");
        }
        let parsed: ChatCompletionResponse =
            serde_json::from_str(&body).context("failed to parse chat response")?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .ok_or_else(|| anyhow::anyhow!("chat response contained no content"))?;
        Ok(content)
    }

    pub async fn embed(&self, model: &str, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let payload = json!({
            "model": model,
            "input": inputs,
        });
        let resp = self
            .apply_auth(self.client.post(format!("{}/v1/embeddings", self.base_url)))
            .json(&payload)
            .send()
            .await
            .context("failed to call LM Studio embeddings")?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            bail!("LM Studio embeddings failed: {status} {body}");
        }
        let parsed: EmbeddingResponse =
            serde_json::from_str(&body).context("failed to parse embeddings response")?;
        Ok(parsed.data.into_iter().map(|item| item.embedding).collect())
    }

    pub async fn generate_image_caption(&self, model: &str, image_path: &str) -> Result<String> {
        let image_data = std::fs::read(image_path)?;
        let base64_image = base64_encode(&image_data);
        let mime_type = if image_path.ends_with(".png") {
            "image/png"
        } else if image_path.ends_with(".jpg") || image_path.ends_with(".jpeg") {
            "image/jpeg"
        } else if image_path.ends_with(".gif") {
            "image/gif"
        } else if image_path.ends_with(".webp") {
            "image/webp"
        } else {
            "image/jpeg"
        };
        let image_url = format!("data:{};base64,{}", mime_type, base64_image);

        let payload = json!({
            "model": model,
            "temperature": 0.3,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": "请描述这张图片的内容，用中文简短回答（50字以内）。只描述图像内容，不要有其他解释。"
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": image_url
                            }
                        }
                    ]
                }
            ]
        });

        let resp = self
            .apply_auth(self.client.post(format!("{}/v1/chat/completions", self.base_url)))
            .json(&payload)
            .send()
            .await
            .context("failed to call vision caption")?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            bail!("Vision caption failed: {status} {body}");
        }
        let parsed: ChatCompletionResponse =
            serde_json::from_str(&body).context("failed to parse caption response")?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .ok_or_else(|| anyhow::anyhow!("caption response contained no content"))?;
        Ok(content)
    }
}

fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
        result.push(ALPHABET[b0 >> 2] as char);
        result.push(ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)] as char);
        if chunk.len() > 1 {
            result.push(ALPHABET[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(ALPHABET[b2 & 0x3f] as char);
        } else {
            result.push('=');
        }
    }
    result
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingItem>,
}

#[derive(Debug, Deserialize, Serialize)]
struct EmbeddingItem {
    embedding: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelItem>,
}

#[derive(Debug, Deserialize)]
struct ModelItem {
    id: String,
}
