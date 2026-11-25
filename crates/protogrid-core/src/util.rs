use base64::Engine;
use percent_encoding::percent_decode_str;

/// Decode a percent-encoded component the way share links usually expect.
pub fn decode_uri_component(input: &str) -> String {
    percent_decode_str(input).decode_utf8_lossy().to_string()
}

/// Decode base64 in all flavours used by proxy subscriptions (standard,
/// URL-safe, padded or unpadded). Percent-encoded input is handled as well.
pub fn b64_decode(input: &str) -> Option<Vec<u8>> {
    let decoded = decode_uri_component(input);
    let cleaned: String = decoded.chars().filter(|c| !c.is_whitespace()).collect();
    let trimmed = cleaned.trim_end_matches('=');
    if trimmed.is_empty() {
        return None;
    }
    use base64::engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE_NO_PAD};
    URL_SAFE_NO_PAD
        .decode(trimmed)
        .ok()
        .or_else(|| STANDARD_NO_PAD.decode(trimmed).ok())
}

pub fn b64_decode_string(input: &str) -> Option<String> {
    b64_decode(input).and_then(|bytes| String::from_utf8(bytes).ok())
}

pub fn parse_bool(value: Option<&String>) -> bool {
    match value {
        Some(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        None => false,
    }
}

pub fn split_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .flat_map(|part| part.split(';'))
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

/// Parse `host:port`, including bracketed IPv6 addresses.
pub fn split_host_port(input: &str) -> Option<(String, u16)> {
    let input = input.trim().trim_end_matches('/');
    if let Some(rest) = input.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        let port = after.strip_prefix(':')?.trim();
        return Some((host.to_string(), port.parse().ok()?));
    }
    let (host, port) = input.rsplit_once(':')?;
    if host.is_empty() || host.contains(':') {
        return None;
    }
    Some((host.to_string(), port.trim().parse().ok()?))
}

/// Wrap a raw IPv6 address so it can be placed inside `host:port` strings.
pub fn host_for_socket(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}

pub fn looks_like_base64_payload(input: &str) -> bool {
    let trimmed = input.trim();
    !trimmed.is_empty()
        && !trimmed.contains("://")
        && !trimmed.contains("proxies:")
        && !trimmed.contains('\n')
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '-' | '_' | '='))
}

/// Parse a sing-box version string such as "1.14.4" and compare it to a
/// minimum supported version.
pub fn version_at_least(version: &str, minimum: &str) -> bool {
    fn parts(v: &str) -> Vec<u64> {
        v.split(|c: char| !c.is_ascii_digit())
            .filter_map(|p| p.parse::<u64>().ok())
            .collect()
    }
    let a = parts(version);
    let b = parts(minimum);
    for i in 0..3 {
        let x = *a.get(i).unwrap_or(&0);
        let y = *b.get(i).unwrap_or(&0);
        if x != y {
            return x > y;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_variants() {
        assert_eq!(b64_decode_string("aGVsbG8=").unwrap(), "hello");
        assert_eq!(b64_decode_string("aGVsbG8").unwrap(), "hello");
        assert!(b64_decode_string("%%%").is_none());
    }

    #[test]
    fn host_port() {
        assert_eq!(
            split_host_port("example.com:443").unwrap(),
            ("example.com".into(), 443)
        );
        assert_eq!(
            split_host_port("[2001:db8::1]:8443").unwrap(),
            ("2001:db8::1".into(), 8443)
        );
    }

    #[test]
    fn versions() {
        assert!(version_at_least("1.14.4", "1.12.0"));
        assert!(version_at_least("sing-box version 1.12.0", "1.12.0"));
        assert!(!version_at_least("1.11.9", "1.12.0"));
    }
}
