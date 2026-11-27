use std::net::SocketAddr;
use std::time::{Duration, Instant};

use tokio::net::TcpStream;

use crate::model::{Protocol, Server, Transport};

/// Number of probes whose median is shown. A single probe is too noisy: one
/// TCP retransmit or TLS handshake spike can double the displayed value.
const SAMPLES: usize = 3;

fn median(mut samples: Vec<u32>) -> Option<u32> {
    if samples.is_empty() {
        return None;
    }
    samples.sort_unstable();
    let middle = samples.len() / 2;
    if samples.len() % 2 == 0 {
        Some(samples[middle - 1].saturating_add(samples[middle]) / 2)
    } else {
        Some(samples[middle])
    }
}

/// Direct TCP RTT to the server endpoint (median of [`SAMPLES`] handshakes).
///
/// DNS resolution happens once and stays outside the timer, so the value is a
/// real SYN → SYN/ACK round trip. There is no ICMP here: raw sockets need
/// privileges and are often filtered. The number is still the RTT to the
/// address from the config, which for CDN/anycast fronting is the edge, not the
/// machine behind it. UDP-only transports have no TCP listener, so they return
/// an error.
pub async fn tcp_rtt(server: &Server, timeout_ms: u64) -> Result<u32, String> {
    if server.address.trim().is_empty() {
        return Err("пустой адрес".into());
    }
    if matches!(
        server.protocol,
        Protocol::Hysteria2 | Protocol::Tuic | Protocol::Masque
    ) || server.transport == Transport::Kcp
    {
        return Err("протокол работает по UDP, TCP RTT недоступен".into());
    }
    let host = server.address.trim();
    let host = host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(host);
    let addresses: Vec<SocketAddr> = tokio::time::timeout(
        Duration::from_millis(timeout_ms.max(1000)),
        tokio::net::lookup_host((host, server.port)),
    )
    .await
    .map_err(|_| "таймаут DNS".to_string())?
    .map_err(|error| format!("DNS: {error}"))?
    .collect();
    if addresses.is_empty() {
        return Err("DNS не вернул адреса".into());
    }

    let mut samples = Vec::with_capacity(SAMPLES);
    let mut last_error = None;
    for _ in 0..SAMPLES {
        let mut connected = false;
        for address in &addresses {
            let started = Instant::now();
            match tokio::time::timeout(
                Duration::from_millis(timeout_ms),
                TcpStream::connect(*address),
            )
            .await
            {
                Ok(Ok(stream)) => {
                    drop(stream);
                    samples.push(started.elapsed().as_millis().min(u32::MAX as u128) as u32);
                    connected = true;
                    break;
                }
                Ok(Err(error)) => last_error = Some(error.to_string()),
                Err(_) => last_error = Some("таймаут".into()),
            }
        }
        if !connected {
            break;
        }
    }

    median(samples).ok_or_else(|| last_error.unwrap_or_else(|| "нет ответа".into()))
}

/// HTTP TTFB (not RTT) through the local HTTP proxy (sing-box mixed / Xray HTTP
/// inbound), median of [`SAMPLES`] requests. Every sample builds a fresh client,
/// so each one pays for its own TCP and TLS handshake instead of reusing a
/// pooled connection and reporting only the last leg.
pub async fn http_proxy_ttfb(http_port: u16, url: &str, timeout_ms: u64) -> Result<u32, String> {
    let mut samples = Vec::with_capacity(SAMPLES);
    let mut last_error = None;
    for _ in 0..SAMPLES {
        match http_proxy_once(http_port, url, timeout_ms).await {
            Ok(value) => samples.push(value),
            Err(error) => last_error = Some(error),
        }
    }
    median(samples).ok_or_else(|| last_error.unwrap_or_else(|| "нет ответа".into()))
}

async fn http_proxy_once(http_port: u16, url: &str, timeout_ms: u64) -> Result<u32, String> {
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

/// HTTP TTFB (not RTT) through the sing-box Clash API, median of [`SAMPLES`]
/// URL tests. The outbound tag is the server id because `singbox::build_config`
/// uses ids as tags.
pub async fn clash_delay(
    api_port: u16,
    secret: &str,
    tag: &str,
    url: &str,
    timeout_ms: u64,
) -> Result<u32, String> {
    let mut samples = Vec::with_capacity(SAMPLES);
    let mut last_error = None;
    for _ in 0..SAMPLES {
        match clash_delay_once(api_port, secret, tag, url, timeout_ms).await {
            Ok(value) => samples.push(value),
            Err(error) => last_error = Some(error),
        }
    }
    median(samples).ok_or_else(|| last_error.unwrap_or_else(|| "нет ответа".into()))
}

async fn clash_delay_once(
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

#[cfg(test)]
mod tests {
    use super::{median, tcp_rtt};
    use crate::model::{Protocol, Server};

    #[test]
    fn median_of_odd_samples() {
        assert_eq!(median(vec![30, 10, 20]), Some(20));
    }

    #[test]
    fn median_of_even_samples() {
        assert_eq!(median(vec![10, 20, 30, 40]), Some(25));
    }

    #[test]
    fn median_ignores_order() {
        assert_eq!(median(vec![120, 40, 80]), Some(80));
    }

    #[test]
    fn median_of_empty_is_none() {
        assert_eq!(median(Vec::new()), None);
    }

    #[tokio::test]
    async fn udp_protocols_skip_tcp_rtt() {
        let mut server = Server::new(Protocol::Hysteria2);
        server.address = "127.0.0.1".into();
        server.port = 9;
        assert!(tcp_rtt(&server, 50).await.is_err());
    }
}
