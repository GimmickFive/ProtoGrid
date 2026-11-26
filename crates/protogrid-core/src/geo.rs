//! Automatic country detection for servers.
//!
//! The server list shows a flag next to every connection. The country comes
//! from a public IP geolocation service when the server is added and is stored
//! in [`Server::country`], so the lookup happens only once per address.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::OnceLock;
use std::time::Duration;

use serde_json::Value;

const LOOKUP_TIMEOUT: Duration = Duration::from_secs(4);

/// Resolves the ISO 3166-1 alpha-2 country code for a server address.
///
/// Accepts an IP literal or a host name. Private and local addresses are
/// skipped because they cannot be geolocated. Errors are swallowed: a missing
/// country only means the badge falls back to the name initials.
pub async fn lookup_country(host: &str) -> Option<String> {
    let host = host.trim().trim_start_matches('[').trim_end_matches(']');
    if host.is_empty() {
        return None;
    }
    let ip = resolve(host).await?;
    if !is_public(ip) {
        return None;
    }
    let client = client()?;
    for url in endpoints(ip) {
        if let Some(code) = fetch(&url, client).await {
            return Some(code);
        }
    }
    None
}

fn client() -> Option<&'static reqwest::Client> {
    static CLIENT: OnceLock<Option<reqwest::Client>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                // Go straight to the provider: the system proxy often points at
                // our own core, which may be stopped while the UI is running.
                .no_proxy()
                .user_agent(concat!("ProtoGrid/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(Duration::from_secs(3))
                .timeout(LOOKUP_TIMEOUT)
                .build()
                .ok()
        })
        .as_ref()
}

async fn resolve(host: &str) -> Option<IpAddr> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Some(ip);
    }
    let resolved = tokio::time::timeout(
        Duration::from_secs(4),
        tokio::net::lookup_host((host, 0_u16)),
    )
    .await
    .ok()?
    .ok()?;
    let mut fallback = None;
    for address in resolved {
        let ip = address.ip();
        if !is_public(ip) {
            continue;
        }
        if ip.is_ipv4() {
            return Some(ip);
        }
        fallback.get_or_insert(ip);
    }
    fallback
}

fn endpoints(ip: IpAddr) -> Vec<String> {
    vec![
        // ipwho.is follows the RIR registration more closely than the other
        // free services, which often geolocate hosting ranges to the wrong
        // country.
        format!("https://ipwho.is/{ip}?fields=success,country_code"),
        format!("https://api.country.is/{ip}"),
        format!("http://ip-api.com/json/{ip}?fields=status,countryCode"),
    ]
}

async fn fetch(url: &str, client: &reqwest::Client) -> Option<String> {
    let value: Value = client
        .get(url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()?;
    // The providers use different field names for the same code.
    for field in ["country", "country_code", "countryCode"] {
        if let Some(code) = value.get(field).and_then(Value::as_str) {
            if let Some(code) = normalize_code(code) {
                return Some(code);
            }
        }
    }
    None
}

fn normalize_code(raw: &str) -> Option<String> {
    let code = raw.trim();
    if code.len() == 2 && code.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        Some(code.to_ascii_uppercase())
    } else {
        None
    }
}

fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => is_public_v6(v6),
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0
        || a == 10
        || a == 127
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 168)
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 224
        || ip == Ipv4Addr::BROADCAST)
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_public_v4(v4);
    }
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || ip.is_unique_local()
        || ip.is_unicast_link_local())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_codes() {
        assert_eq!(normalize_code(" us "), Some("US".into()));
        assert_eq!(normalize_code("de"), Some("DE".into()));
        assert_eq!(normalize_code("USA"), None);
        assert_eq!(normalize_code("1a"), None);
        assert_eq!(normalize_code(""), None);
    }

    #[test]
    fn skips_local_addresses() {
        assert!(!is_public("127.0.0.1".parse().unwrap()));
        assert!(!is_public("192.168.1.10".parse().unwrap()));
        assert!(!is_public("10.2.3.4".parse().unwrap()));
        assert!(!is_public("100.64.0.1".parse().unwrap()));
        assert!(!is_public("::1".parse().unwrap()));
        assert!(!is_public("fd00::1".parse().unwrap()));
        assert!(is_public("8.8.8.8".parse().unwrap()));
        assert!(is_public("2a00:1450:4001:80f::200e".parse().unwrap()));
    }
}
