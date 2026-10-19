use anyhow::{bail, Context, Result};
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
    pub fn from_service(service: &crate::config::ModelServiceSettings) -> Result<Self> {
        Ok(Self {
            client: crate::network::build_client(&service.proxy)?,
            base_url: service
                .base_url
                .trim_end_matches('/')
                .trim_end_matches("/v1")
                .to_string(),
            api_key: None,
        }
        .with_api_key(&service.api_key))
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
        let mut payload = json!({
            "model": model,
            "temperature": 0.2,
            "stream": false,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ]
        });
        self.disable_thinking(model, &mut payload);
        let _audit = RequestAudit::start(model, &payload);
        let resp = self
            .apply_auth(
                self.client
                    .post(format!("{}/v1/chat/completions", self.base_url)),
            )
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
        ensure_direct_response(&parsed)?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .ok_or_else(|| anyhow::anyhow!("chat response contained no content"))?;
        Ok(content)
    }

    pub async fn embed(&self, model: &str, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut output = Vec::with_capacity(inputs.len());
        // Cloud providers cap array length; the icon catalog alone exceeds 2,000.
        for batch in inputs.chunks(128) {
            output.extend(self.embed_batch(model, batch).await?);
        }
        if let Some(first) = output.first() {
            if first.is_empty()
                || output
                    .iter()
                    .any(|row| row.len() != first.len() || row.iter().any(|v| !v.is_finite()))
            {
                bail!("embedding response has invalid or inconsistent dimensions");
            }
        }
        Ok(output)
    }

    async fn embed_batch(&self, model: &str, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
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
        let mut parsed: EmbeddingResponse =
            serde_json::from_str(&body).context("failed to parse embeddings response")?;
        if parsed.data.len() != inputs.len() {
            bail!(
                "embedding count mismatch: sent {}, got {}",
                inputs.len(),
                parsed.data.len()
            );
        }
        if parsed.data.iter().any(|item| item.index.is_some()) {
            parsed.data.sort_by_key(|item| item.index);
            if parsed
                .data
                .iter()
                .enumerate()
                .any(|(i, item)| item.index != Some(i))
            {
                bail!("embedding response has missing or duplicate indices");
            }
        }
        Ok(parsed.data.into_iter().map(|item| item.embedding).collect())
    }

    /// Send one image plus a text prompt to the multimodal model and return its answer.
    pub async fn generate_vision(
        &self,
        model: &str,
        image_path: &str,
        prompt: &str,
    ) -> Result<String> {
        let image_data = std::fs::read(image_path)?;
        let base64_image = base64_encode(&image_data);
        let image_url = format!(
            "data:{};base64,{}",
            image_mime_type(image_path),
            base64_image
        );

        let mut payload = json!({
            "model": model,
            "temperature": 0.3,
            "stream": false,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": prompt
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
        self.disable_thinking(model, &mut payload);
        let _audit = RequestAudit::start(model, &payload);

        let resp = self
            .apply_auth(
                self.client
                    .post(format!("{}/v1/chat/completions", self.base_url)),
            )
            .json(&payload)
            .send()
            .await
            .context("failed to call the vision model")?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            bail!("Vision request failed: {status} {body}");
        }
        let parsed: ChatCompletionResponse =
            serde_json::from_str(&body).context("failed to parse the vision response")?;
        ensure_direct_response(&parsed)?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .ok_or_else(|| anyhow::anyhow!("vision response contained no content"))?;
        Ok(content)
    }

    /// One-off description of a single image, used when an asset is imported.
    pub async fn generate_image_caption(&self, model: &str, image_path: &str) -> Result<String> {
        self.generate_vision(
            model,
            image_path,
            "请描述这张图片的内容，用中文简短回答（50字以内）。只描述图像内容，不要有其他解释。",
        )
        .await
    }

    fn disable_thinking(&self, model: &str, payload: &mut serde_json::Value) {
        // Qwen-compatible cloud HTTP APIs require this at the top level, not
        // inside an SDK's extra_body or LM Studio's template options.
        let host = reqwest::Url::parse(&self.base_url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .unwrap_or_default();
        let local = matches!(host.as_str(), "127.0.0.1" | "localhost" | "::1");
        if model.to_ascii_lowercase().contains("qwen")
            || host.ends_with("dmxapi.cn")
            || host.contains("dashscope")
        {
            payload["enable_thinking"] = json!(false);
        }
        if local {
            payload["chat_template_kwargs"] = json!({"enable_thinking":false});
        }
    }
}

fn ensure_direct_response(response: &ChatCompletionResponse) -> Result<()> {
    let reasoning_tokens = response
        .usage
        .pointer("/completion_tokens_details/reasoning_tokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let reasoning = response.choices.iter().any(|choice| {
        let message = &choice.message;
        message.reasoning_content.as_ref().is_some_and(nonempty)
            || message.reasoning.as_ref().is_some_and(nonempty)
            || message
                .content
                .as_ref()
                .is_some_and(|s| s.contains("<think>") || s.contains("<thinking>"))
    });
    audit_record(
        json!({"event":"response","reasoning_present":reasoning,"reasoning_tokens":reasoning_tokens}),
    );
    if reasoning || reasoning_tokens > 0 {
        bail!("model returned thinking despite direct-response mode; check provider/model compatibility");
    }
    Ok(())
}

fn nonempty(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::String(s) => !s.trim().is_empty(),
        serde_json::Value::Array(a) => !a.is_empty(),
        serde_json::Value::Object(o) => !o.is_empty(),
        _ => true,
    }
}

// Opt-in diagnostics contain request metadata only, never credentials or prompts.
fn audit_record(mut value: serde_json::Value) {
    use std::io::Write;
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let Some(path) = std::env::var_os("HUANSHU_MODEL_AUDIT") else {
        return;
    };
    let Ok(_guard) = LOCK.lock() else {
        return;
    };
    value["timestamp_ms"] = json!(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis());
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{value}");
    }
}
struct RequestAudit(usize);
impl RequestAudit {
    fn start(model: &str, payload: &serde_json::Value) -> Self {
        static ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        audit_record(
            json!({"event":"start","id":id,"model":model,"stream":payload["stream"],"enable_thinking":payload["enable_thinking"],"chat_template_kwargs":payload["chat_template_kwargs"]}),
        );
        Self(id)
    }
}
impl Drop for RequestAudit {
    fn drop(&mut self) {
        audit_record(json!({"event":"end","id":self.0}));
    }
}

/// MIME type from the file extension, for the data URL handed to the vision model.
fn image_mime_type(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else {
        "image/jpeg"
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
    #[serde(default)]
    usage: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<serde_json::Value>,
    #[serde(default)]
    reasoning: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingItem>,
}

#[derive(Debug, Deserialize, Serialize)]
struct EmbeddingItem {
    embedding: Vec<f32>,
    #[serde(default)]
    index: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelItem>,
}

#[derive(Debug, Deserialize)]
struct ModelItem {
    id: String,
}

#[cfg(test)]
mod direct_response_tests {
    use super::*;
    #[test]
    fn cloud_qwen_uses_top_level_switch_and_accepts_v1_base_url() {
        let service = crate::config::ModelServiceSettings::new(
            "https://www.dmxapi.cn/v1/",
            "test-key",
            "qwen3.8-flash",
        );
        let client = LmStudioClient::from_service(&service).unwrap();
        assert_eq!(client.base_url, "https://www.dmxapi.cn");
        let mut payload = json!({"model":"qwen3.8-flash","stream":false});
        client.disable_thinking("qwen3.8-flash", &mut payload);
        assert_eq!(payload["enable_thinking"], false);
        assert!(payload.get("chat_template_kwargs").is_none());
        assert!(payload.get("reasoning_effort").is_none());
    }
    #[test]
    fn local_qwen_disables_both_template_and_server_thinking() {
        let client = LmStudioClient::from_service(&crate::config::ModelServiceSettings::new(
            "http://127.0.0.1:12345",
            "",
            "qwen/local",
        ))
        .unwrap();
        let mut payload = json!({});
        client.disable_thinking("qwen/local", &mut payload);
        assert_eq!(payload["chat_template_kwargs"]["enable_thinking"], false);
        assert_eq!(payload["enable_thinking"], false);
    }
    #[test]
    fn reasoning_is_rejected_instead_of_silently_discarded() {
        for message in [
            json!({"content":"ok","reasoning_content":"reasoning"}),
            json!({"content":"ok","reasoning":[{"text":"reasoning"}]}),
            json!({"content":"<think>reasoning</think>ok"}),
        ] {
            let response: ChatCompletionResponse =
                serde_json::from_value(json!({"choices":[{"message":message}]})).unwrap();
            assert!(ensure_direct_response(&response).is_err());
        }
        let response:ChatCompletionResponse=serde_json::from_value(json!({"choices":[{"message":{"content":"ok"}}],"usage":{"completion_tokens_details":{"reasoning_tokens":12}}})).unwrap();
        assert!(ensure_direct_response(&response).is_err());
    }
    #[test]
    fn empty_reasoning_fields_are_valid_for_non_thinking_responses() {
        let response:ChatCompletionResponse=serde_json::from_value(json!({"choices":[{"message":{"content":"ok","reasoning_content":"","reasoning":null}}],"usage":{"completion_tokens_details":{"reasoning_tokens":0}}})).unwrap();
        ensure_direct_response(&response).unwrap();
    }
}
