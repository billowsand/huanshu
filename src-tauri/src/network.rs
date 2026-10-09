//! Shared transport for all model APIs. SOCKS tunnels carry HTTP/HTTPS unchanged.
use anyhow::{bail, Context, Result};
use reqwest::{Client, Proxy, Url};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Socks5Settings {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    pub password: String,
}

impl Default for Socks5Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            url: "socks5://127.0.0.1:1080".into(),
            username: String::new(),
            password: String::new(),
        }
    }
}

impl Socks5Settings {
    pub fn validate(&self) -> Result<()> {
        self.proxy().map(|_| ())
    }

    fn proxy(&self) -> Result<Option<Proxy>> {
        if !self.enabled {
            return Ok(None);
        }
        // Do not include user input in errors: it may contain credentials.
        let mut url = Url::parse(self.url.trim())
            .map_err(|_| anyhow::anyhow!("SOCKS5 代理地址无效，请填写 socks5://主机:端口"))?;
        if !matches!(url.scheme(), "socks5" | "socks5h") {
            bail!("代理仅支持 socks5:// 或 socks5h:// 协议");
        }
        if url.host_str().is_none() || url.port().filter(|p| *p > 0).is_none() {
            bail!("SOCKS5 代理地址必须包含主机和有效端口（1–65535）");
        }
        if !url.username().is_empty() || url.password().is_some() {
            bail!("请在独立的代理用户名和密码字段中填写认证信息");
        }
        if !matches!(url.path(), "" | "/") || url.query().is_some() || url.fragment().is_some() {
            bail!("SOCKS5 代理地址不能包含路径、查询参数或片段");
        }
        if self.username.is_empty() != self.password.is_empty() {
            bail!("代理认证需要同时填写用户名和密码，或将两者留空");
        }
        if self.username.len() > 255 || self.password.len() > 255 {
            bail!("SOCKS5 用户名和密码各自不能超过 255 字节");
        }
        // Resolve model hostnames at the proxy, including when socks5:// is entered.
        url.set_scheme("socks5h")
            .map_err(|_| anyhow::anyhow!("SOCKS5 代理协议无效"))?;
        let mut proxy = Proxy::all(url).context("无法创建 SOCKS5 代理")?;
        if !self.username.is_empty() {
            proxy = proxy.basic_auth(&self.username, &self.password);
        }
        Ok(Some(proxy))
    }
}

pub fn build_client(settings: &Socks5Settings) -> Result<Client> {
    // Explicit routing avoids system proxies/NO_PROXY overriding the user's choice.
    let mut builder = Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(30));
    if let Some(proxy) = settings.proxy()? {
        builder = builder.proxy(proxy);
    }
    // No short response timeout: model generation can take several minutes.
    builder.build().context("无法创建模型网络客户端")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    fn read_bytes(stream: &mut TcpStream, len: usize) -> Vec<u8> {
        let mut bytes = vec![0; len];
        stream.read_exact(&mut bytes).unwrap();
        bytes
    }

    // A local SOCKS5 peer checks the real wire handshake and API payloads.
    // The destination deliberately has no DNS entry, so local resolution cannot pass.
    async fn exercise_model_transport(authenticated: bool) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let username = "user@中文";
        let password = "p@ss:/?#%中文";
        let peer = std::thread::spawn(move || {
            for endpoint in [
                "models",
                "chat/completions",
                "embeddings",
                "chat/completions",
            ] {
                let deadline = std::time::Instant::now() + Duration::from_secs(10);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                std::time::Instant::now() < deadline,
                                "SOCKS connection timed out"
                            );
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(e) => panic!("SOCKS accept failed: {e}"),
                    }
                };
                // Windows accepted sockets inherit the listener's nonblocking mode.
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let greeting = read_bytes(&mut stream, 2);
                assert_eq!(greeting[0], 5);
                let methods = read_bytes(&mut stream, greeting[1] as usize);
                let method = if authenticated { 2 } else { 0 };
                assert!(methods.contains(&method));
                stream.write_all(&[5, method]).unwrap();
                if authenticated {
                    let auth = read_bytes(&mut stream, 2);
                    assert_eq!(auth[0], 1);
                    assert_eq!(
                        read_bytes(&mut stream, auth[1] as usize),
                        username.as_bytes()
                    );
                    let password_len = read_bytes(&mut stream, 1)[0];
                    assert_eq!(
                        read_bytes(&mut stream, password_len as usize),
                        password.as_bytes()
                    );
                    stream.write_all(&[1, 0]).unwrap();
                }
                assert_eq!(read_bytes(&mut stream, 4), [5, 1, 0, 3]);
                let host_len = read_bytes(&mut stream, 1)[0];
                assert_eq!(read_bytes(&mut stream, host_len as usize), b"model.invalid");
                assert_eq!(read_bytes(&mut stream, 2), 8080u16.to_be_bytes());
                stream
                    .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 80])
                    .unwrap();
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    header.extend(read_bytes(&mut stream, 1));
                    assert!(header.len() < 16384);
                }
                let header = String::from_utf8(header).unwrap().to_lowercase();
                assert!(header.contains(&format!(" /v1/{endpoint} http/1.1")));
                assert!(header.contains("authorization: bearer model-api-key\r\n"));
                assert!(!header.contains("proxy-authorization"));
                let length = header
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .map(|value| value.parse::<usize>().unwrap())
                    .unwrap_or(0);
                let body = read_bytes(&mut stream, length);
                let response = match endpoint {
                    "models" => r#"{"data":[{"id":"test-model"}]}"#,
                    "embeddings" => {
                        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
                        assert_eq!(payload["input"][0], "test input");
                        r#"{"data":[{"embedding":[0.1,0.2]}]}"#
                    }
                    _ => {
                        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
                        assert_eq!(payload["model"], "test-model");
                        if payload["messages"][0]["content"].is_array() {
                            assert!(payload["messages"][0]["content"][1]["image_url"]["url"]
                                .as_str()
                                .unwrap()
                                .starts_with("data:image/png;base64,"));
                        } else {
                            assert_eq!(payload["messages"][1]["content"], "hello");
                        }
                        r#"{"choices":[{"message":{"content":"ok"}}]}"#
                    }
                };
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
            }
        });
        let mut service = crate::config::ModelServiceSettings::new(
            "http://model.invalid:8080",
            "model-api-key",
            "test-model",
        );
        service.proxy.enabled = true;
        service.proxy.url = format!("socks5://{address}");
        if authenticated {
            service.proxy.username = username.into();
            service.proxy.password = password.into();
        }
        let client = crate::lmstudio::LmStudioClient::from_service(&service).unwrap();
        assert_eq!(client.list_models().await.unwrap(), ["test-model"]);
        assert_eq!(
            client
                .generate_text("test-model", "system", "hello")
                .await
                .unwrap(),
            "ok"
        );
        assert_eq!(
            client
                .embed("test-model", &["test input".into()])
                .await
                .unwrap(),
            [vec![0.1, 0.2]]
        );
        let image =
            std::env::temp_dir().join(format!("huanshu-socks-{}.png", uuid::Uuid::new_v4()));
        std::fs::write(&image, b"test image bytes").unwrap();
        let caption = client
            .generate_image_caption("test-model", image.to_str().unwrap())
            .await;
        std::fs::remove_file(image).unwrap();
        assert_eq!(caption.unwrap(), "ok");
        peer.join().unwrap();
    }

    #[tokio::test]
    async fn model_apis_use_socks5_with_remote_dns() {
        exercise_model_transport(false).await;
    }

    #[tokio::test]
    async fn model_apis_use_socks5_username_password_auth() {
        exercise_model_transport(true).await;
    }

    #[tokio::test]
    async fn unavailable_proxy_does_not_fall_back_to_direct_connection() {
        let direct = TcpListener::bind("127.0.0.1:0").unwrap();
        direct.set_nonblocking(true).unwrap();
        let unused = TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy_address = unused.local_addr().unwrap();
        drop(unused);
        let mut service = crate::config::ModelServiceSettings::new(
            format!("http://{}", direct.local_addr().unwrap()),
            "",
            "test",
        );
        service.proxy.enabled = true;
        service.proxy.url = format!("socks5://{proxy_address}");
        let client = crate::lmstudio::LmStudioClient::from_service(&service).unwrap();
        assert!(client.list_models().await.is_err());
        assert_eq!(
            direct.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn validates_enabled_proxy_without_exposing_credentials() {
        let mut settings = Socks5Settings {
            enabled: true,
            ..Default::default()
        };
        assert!(settings.validate().is_ok());
        settings.url = "socks5h://[::1]:1080".into();
        assert!(settings.validate().is_ok());
        for url in [
            "http://localhost:1080",
            "socks5://localhost",
            "socks5://localhost:0",
            "socks5://localhost:99999",
            "socks5://localhost:1080/path",
            "socks5://localhost:1080?x=1",
            "socks5://user:secret@localhost:1080",
        ] {
            settings.url = url.into();
            let error = settings.validate().unwrap_err().to_string();
            assert!(!error.contains("secret"));
        }
        settings.url = "socks5://localhost:1080".into();
        settings.username = "user".into();
        assert!(settings.validate().is_err());
        settings.password = "p@ss:/?#%中文".into();
        assert!(settings.validate().is_ok());
        settings.password = "长".repeat(86);
        assert!(settings.validate().is_err());
        settings.enabled = false;
        settings.url = "invalid".into();
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn old_settings_load_with_direct_connections_and_proxy_roundtrips() {
        let old = r#"{"base_url":"http://localhost:1234","model":"chat"}"#;
        let mut settings: crate::config::LlmSettings = serde_json::from_str(old).unwrap();
        assert!(!settings.llm.proxy.enabled);
        assert!(!settings.embedding.proxy.enabled);
        assert!(!settings.multimodal.proxy.enabled);
        let service: crate::config::ModelServiceSettings =
            serde_json::from_str(r#"{"base_url":"https://example.com","model":"chat"}"#).unwrap();
        assert!(!service.proxy.enabled);
        settings.llm.proxy.enabled = true;
        settings.llm.proxy.password = "secret".into();
        settings.llm.proxy.username = "user".into();
        let serialized = serde_json::to_string(&settings).unwrap();
        let restored: crate::config::LlmSettings = serde_json::from_str(&serialized).unwrap();
        assert!(restored.llm.proxy.enabled);
        assert_eq!(restored.llm.proxy.password, "secret");
        assert!(!restored.embedding.proxy.enabled);
    }
}
