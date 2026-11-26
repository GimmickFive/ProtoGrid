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

/// Subscription URLs are credentials: they usually contain a token. Only
/// HTTPS is accepted so the token and the server list are never sent in clear
/// text. Redirects to plain HTTP are stopped for the same reason.
fn validate_subscription_url(raw: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(raw.trim()).map_err(|_| "некорректный адрес подписки".to_string())?;
    if url.scheme() != "https" {
        return Err("адрес подписки должен начинаться с https://".into());
    }
    if url.host_str().unwrap_or_default().is_empty() {
        return Err("в адресе подписки нет хоста".into());
    }
    Ok(url)
}

/// URL for display in logs and UI: the query string is almost always the
/// subscription token, so it is replaced with an ellipsis.
pub fn display_url(raw: &str) -> String {
    let raw = raw.trim();
    let Ok(mut url) = url::Url::parse(raw) else {
        return raw.split(['?', '#']).next().unwrap_or(raw).to_string();
    };
    let _ = url.set_username("");
    let _ = url.set_password(None);
    if url.query().is_some() {
        url.set_query(None);
    }
    url.to_string()
}

/// Download a subscription without going through the proxy (to avoid loops).
pub async fn fetch_subscription(url: &str) -> Result<FetchedSubscription, String> {
    let url = validate_subscription_url(url)?;

    let client = reqwest::Client::builder()
        .user_agent(format!("ProtoGrid/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(30))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.url().scheme() == "https" {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()
        .map_err(|e| format!("не удалось создать HTTP-клиент: {e}"))?;

    let response = client
        .get(url)
        .header("Accept", "*/*")
        .send()
        .await
        // `without_url` keeps the subscription token out of the message that
        // is stored in config.json and written to the journal.
        .map_err(|e| format!("не удалось загрузить подписку: {}", e.without_url()))?;

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
        .map_err(|e| format!("не удалось прочитать ответ подписки: {}", e.without_url()))?;
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

    #[test]
    fn display_url_hides_token_and_credentials() {
        let shown = display_url("https://user:pass@example.com/sub?token=secret#x");
        assert!(!shown.contains("secret"));
        assert!(!shown.contains("pass"));
        assert!(shown.starts_with("https://example.com/sub"));
    }

    #[test]
    fn subscription_url_requires_https() {
        assert!(validate_subscription_url("https://example.com/sub?token=x").is_ok());
        assert!(validate_subscription_url("http://example.com/sub?token=x").is_err());
        assert!(validate_subscription_url("ftp://example.com/sub").is_err());
        assert!(validate_subscription_url("https://").is_err());
    }
}
