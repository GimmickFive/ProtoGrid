use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::link::{parse_links, ParseOutcome};
use crate::model::Server;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SubUserInfo {
    /// Bytes uploaded according to the subscription provider.
    pub upload: u64,
    /// Bytes downloaded according to the subscription provider.
    pub download: u64,
    /// Traffic limit in bytes. Zero means "not reported".
    pub total: u64,
    /// Unix timestamp (seconds) when the subscription expires. Zero means "not reported".
    pub expire: i64,
}

impl SubUserInfo {
    pub fn used(&self) -> u64 {
        self.upload.saturating_add(self.download)
    }
}

#[derive(Debug)]
pub struct FetchedSubscription {
    pub servers: Vec<Server>,
    pub errors: Vec<String>,
    pub userinfo: Option<SubUserInfo>,
}

/// Parse the `subscription-userinfo` header used by most providers.
pub fn parse_userinfo(header: &str) -> SubUserInfo {
    let mut info = SubUserInfo::default();
    for part in header.split(';') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let value = value.trim().parse::<i64>().unwrap_or(0);
        match key.trim().to_ascii_lowercase().as_str() {
            "upload" => info.upload = value.max(0) as u64,
            "download" => info.download = value.max(0) as u64,
            "total" => info.total = value.max(0) as u64,
            "expire" => info.expire = value,
            _ => {}
        }
    }
    info
}

pub fn parse_subscription_body(text: &str) -> ParseOutcome {
    parse_links(text)
}

/// Download a subscription without going through the proxy (to avoid loops).
pub async fn fetch_subscription(url: &str) -> Result<FetchedSubscription, String> {
    let url = url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("адрес подписки должен начинаться с http:// или https://".into());
    }

    let client = reqwest::Client::builder()
        .user_agent(format!("ProtoGrid/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(30))
        .no_proxy()
        .build()
        .map_err(|e| format!("не удалось создать HTTP-клиент: {e}"))?;

    let response = client
        .get(url)
        .header("Accept", "*/*")
        .send()
        .await
        .map_err(|e| format!("не удалось загрузить подписку: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("сервер подписки ответил {}", response.status()));
    }

    if let Some(length) = response.content_length() {
        if length > 16 * 1024 * 1024 {
            return Err("подписка слишком большая (больше 16 МБ)".into());
        }
    }

    let userinfo = response
        .headers()
        .get("subscription-userinfo")
        .and_then(|value| value.to_str().ok())
        .map(parse_userinfo);

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("не удалось прочитать ответ подписки: {e}"))?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("подписка слишком большая (больше 16 МБ)".into());
    }

    let mut text = String::from_utf8(bytes.to_vec())
        .map_err(|_| "ответ подписки не является текстом (UTF-8)".to_string())?;
    if let Some(stripped) = text.strip_prefix('\u{feff}') {
        text = stripped.to_string();
    }

    let outcome = parse_subscription_body(&text);
    if outcome.servers.is_empty() {
        let reason = if outcome.errors.is_empty() {
            "в подписке не найдено серверов".to_string()
        } else {
            crate::link::display_error_list(&outcome.errors)
        };
        return Err(reason);
    }

    Ok(FetchedSubscription {
        servers: outcome.servers,
        errors: outcome.errors,
        userinfo,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn userinfo_header() {
        let info = parse_userinfo("upload=100; download=200; total=10000; expire=1700000000");
        assert_eq!(info.used(), 300);
        assert_eq!(info.total, 10000);
        assert_eq!(info.expire, 1_700_000_000);
    }

    #[test]
    fn base64_subscription() {
        let raw = "vless://11111111-1111-1111-1111-111111111111@a.example:443#A\nss://YWVzLTI1Ni1nY206cGFzcw==@b.example:8388#B";
        let encoded = base64::engine::general_purpose::STANDARD.encode(raw);
        let outcome = parse_subscription_body(&encoded);
        assert_eq!(outcome.servers.len(), 2, "{:?}", outcome.errors);
        assert_eq!(outcome.errors.len(), 0);
    }

    #[test]
    fn clash_yaml_body() {
        let yaml = r#"
proxies:
  - {name: A, type: ss, server: a.example, port: 8388, cipher: aes-256-gcm, password: pass}
"#;
        let outcome = parse_subscription_body(yaml);
        assert_eq!(outcome.servers.len(), 1);
    }
}
