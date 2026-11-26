use std::time::{Duration, Instant};

use tokio::net::TcpStream;

use crate::model::Server;
use crate::util::host_for_socket;

/// Measure a plain TCP handshake to the server endpoint.
pub async fn tcp_ping(server: &Server, timeout_ms: u64) -> Result<u32, String> {
    if server.address.trim().is_empty() {
        return Err("пустой адрес".into());
    }
    let target = format!("{}:{}", host_for_socket(server.address.trim()), server.port);
    let started = Instant::now();
    match tokio::time::timeout(
        Duration::from_millis(timeout_ms),
        TcpStream::connect(&target),
    )
    .await
    {
        Ok(Ok(stream)) => {
            drop(stream);
            Ok(started.elapsed().as_millis().min(u32::MAX as u128) as u32)
        }
        Ok(Err(error)) => Err(format!("{error}")),
        Err(_) => Err("таймаут".into()),
    }
}

/// URI latency test through the local HTTP proxy (sing-box mixed / Xray HTTP inbound).
pub async fn http_proxy_test(http_port: u16, url: &str, timeout_ms: u64) -> Result<u32, String> {
    let proxy = format!("http://127.0.0.1:{http_port}");
    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(&proxy).map_err(|e| format!("прокси: {e}"))?)
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|e| format!("клиент: {e}"))?;
    let started = Instant::now();
    let response = client
        .get(url)
        .header("Cache-Control", "no-store")
        .send()
        .await
        .map_err(|e| format!("{e}"))?;
    if response.status().as_u16() >= 500 {
        return Err(format!("HTTP {}", response.status()));
    }
    Ok(started.elapsed().as_millis().min(u32::MAX as u128) as u32)
}

/// Real per-outbound delay through the sing-box Clash API. The outbound tag is
/// the server id because `singbox::build_config` uses ids as tags.
pub async fn clash_delay(
    api_port: u16,
    secret: &str,
    tag: &str,
    url: &str,
    timeout_ms: u64,
) -> Result<u32, String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(timeout_ms.max(1000) + 1500))
        .build()
        .map_err(|e| format!("клиент: {e}"))?;
    let endpoint = format!("http://127.0.0.1:{api_port}/proxies/{tag}/delay");
    let mut request = client
        .get(endpoint)
        .query(&[("url", url), ("timeout", &timeout_ms.to_string())]);
    if !secret.is_empty() {
        request = request.bearer_auth(secret);
    }
    let response = request.send().await.map_err(|e| format!("{e}"))?;
    let status = response.status();
    let value: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("ответ Clash API: {e}"))?;
    if !status.is_success() {
        let message = value
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("ошибка Clash API");
        return Err(message.to_string());
    }
    value
        .get("delay")
        .and_then(|v| v.as_u64())
        .map(|delay| delay.min(u32::MAX as u64) as u32)
        .ok_or_else(|| "Clash API не вернул задержку".into())
}
