use serde_yaml::{Mapping, Value};

use crate::model::{Protocol, Security, Server, Transport};
use crate::util::split_list;

/// Parse the `proxies:` section of a Clash / Clash.Meta YAML subscription.
pub fn parse_clash_yaml(text: &str) -> Result<Vec<Server>, String> {
    let root: Value =
        serde_yaml::from_str(text).map_err(|e| format!("не удалось разобрать Clash YAML: {e}"))?;
    let proxies = root
        .get("proxies")
        .and_then(Value::as_sequence)
        .ok_or_else(|| "в YAML нет списка proxies".to_string())?;

    let mut servers = Vec::new();
    let mut errors = Vec::new();
    for (index, item) in proxies.iter().enumerate() {
        let Some(map) = item.as_mapping() else {
            continue;
        };
        match from_clash_proxy(map) {
            Ok(Some(server)) => servers.push(server),
            Ok(None) => {}
            Err(error) => {
                let name = str_at(map, "name").unwrap_or_else(|| format!("#{}", index + 1));
                errors.push(format!("{name}: {error}"));
            }
        }
    }

    if servers.is_empty() {
        let reason = if errors.is_empty() {
            "в YAML не найдено поддерживаемых прокси".to_string()
        } else {
            errors.join("; ")
        };
        return Err(reason);
    }
    Ok(servers)
}

fn str_at(map: &Mapping, key: &str) -> Option<String> {
    map.get(Value::String(key.to_string()))
        .and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(b.to_string()),
            _ => None,
        })
}

fn value_at<'a>(map: &'a Mapping, key: &str) -> Option<&'a Value> {
    map.get(Value::String(key.to_string()))
}

fn map_at<'a>(map: &'a Mapping, key: &str) -> Option<&'a Mapping> {
    value_at(map, key).and_then(Value::as_mapping)
}

fn bool_at(map: &Mapping, key: &str) -> bool {
    match value_at(map, key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => matches!(
            s.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0) != 0,
        _ => false,
    }
}

fn u16_at(map: &Mapping, key: &str) -> u16 {
    match value_at(map, key) {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0).min(u16::MAX as u64) as u16,
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
        _ => 0,
    }
}

fn u32_at(map: &Mapping, key: &str) -> u32 {
    match value_at(map, key) {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0).min(u32::MAX as u64) as u32,
        Some(Value::String(s)) => s
            .trim()
            .trim_end_matches(|c: char| !c.is_ascii_digit())
            .parse()
            .unwrap_or(0),
        _ => 0,
    }
}

fn list_at(map: &Mapping, key: &str) -> Vec<String> {
    match value_at(map, key) {
        Some(Value::Sequence(seq)) => seq
            .iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(s.trim().to_string()),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .filter(|s| !s.is_empty())
            .collect(),
        Some(Value::String(s)) => split_list(s),
        _ => Vec::new(),
    }
}

fn parse_transport(name: &str) -> Transport {
    match name.to_ascii_lowercase().as_str() {
        "ws" | "websocket" => Transport::Ws,
        "grpc" | "gun" => Transport::Grpc,
        "h2" | "http" => Transport::Http,
        "httpupgrade" => Transport::HttpUpgrade,
        "xhttp" | "splithttp" => Transport::Xhttp,
        "kcp" | "mkcp" => Transport::Kcp,
        "xdrive" => Transport::XDrive,
        _ => Transport::Tcp,
    }
}

fn apply_network(server: &mut Server, map: &Mapping) {
    let network = str_at(map, "network").unwrap_or_default();
    server.transport = parse_transport(&network);

    if let Some(ws) = map_at(map, "ws-opts") {
        server.path = str_at(ws, "path").unwrap_or_default();
        if let Some(headers) = map_at(ws, "headers") {
            server.host = str_at(headers, "Host")
                .or_else(|| str_at(headers, "host"))
                .unwrap_or_default();
        }
        if server.path.is_empty() {
            server.path = "/".into();
        }
    }
    if let Some(grpc) = map_at(map, "grpc-opts") {
        server.service_name = str_at(grpc, "grpc-service-name")
            .or_else(|| str_at(grpc, "serviceName"))
            .unwrap_or_default();
    }
    if let Some(h2) = map_at(map, "h2-opts").or_else(|| map_at(map, "http-opts")) {
        server.path = str_at(h2, "path").unwrap_or_default();
        let hosts = list_at(h2, "host");
        server.host = hosts.first().cloned().unwrap_or_default();
    }
    if let Some(xhttp) = map_at(map, "xhttp-opts") {
        server.path = str_at(xhttp, "path").unwrap_or_default();
        server.host = str_at(xhttp, "host").unwrap_or_default();
        server.xhttp_mode = str_at(xhttp, "mode").unwrap_or_else(|| "auto".into());
    }
    if let Some(kcp) = map_at(map, "kcp-opts") {
        server.kcp_mtu = str_at(kcp, "mtu")
            .and_then(|value| value.parse().ok())
            .unwrap_or(1350);
        server.kcp_tti = str_at(kcp, "tti")
            .and_then(|value| value.parse().ok())
            .unwrap_or(50);
        server.kcp_uplink = str_at(kcp, "uplink-capacity")
            .or_else(|| str_at(kcp, "uplink_capacity"))
            .and_then(|value| value.parse().ok())
            .unwrap_or(5);
        server.kcp_downlink = str_at(kcp, "downlink-capacity")
            .or_else(|| str_at(kcp, "downlink_capacity"))
            .and_then(|value| value.parse().ok())
            .unwrap_or(20);
        server.kcp_seed = str_at(kcp, "seed").unwrap_or_default();
    }
    if let Some(xdrive) = map_at(map, "xdrive-opts") {
        server.xdrive_service = str_at(xdrive, "service").unwrap_or_else(|| "local".into());
        server.xdrive_remote_folder = str_at(xdrive, "remote-folder")
            .or_else(|| str_at(xdrive, "remote_folder"))
            .unwrap_or_default();
        server.xdrive_secrets = list_at(xdrive, "secrets").join(",");
        if let Some(s3) = map_at(xdrive, "s3") {
            server.xdrive_s3_endpoint = str_at(s3, "endpoint").unwrap_or_default();
            server.xdrive_s3_region = str_at(s3, "region").unwrap_or_default();
            server.xdrive_s3_bucket = str_at(s3, "bucket").unwrap_or_default();
            server.xdrive_s3_path_style = bool_at(s3, "path-style") || bool_at(s3, "path_style");
        }
    }
}

fn apply_tls(server: &mut Server, map: &Mapping, default_tls: bool) {
    let tls_value = value_at(map, "tls");
    let tls_enabled = match tls_value {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !matches!(
            s.trim().to_ascii_lowercase().as_str(),
            "" | "none" | "false"
        ),
        Some(_) => true,
        None => default_tls,
    };

    let reality = map_at(map, "reality-opts");
    server.allow_insecure = bool_at(map, "skip-cert-verify");
    server.sni = str_at(map, "servername")
        .or_else(|| str_at(map, "sni"))
        .or_else(|| str_at(map, "peer"))
        .unwrap_or_default();
    server.alpn = list_at(map, "alpn");
    server.encryption = str_at(map, "encryption").unwrap_or_default();
    if server.encryption == "none" {
        server.encryption.clear();
    }
    server.tcp_fast_open = bool_at(map, "tfo") || bool_at(map, "tcp-fast-open");
    server.bind_interface = str_at(map, "interface-name")
        .or_else(|| str_at(map, "interface"))
        .unwrap_or_default();
    server.routing_mark = str_at(map, "routing-mark")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    server.fingerprint = str_at(map, "client-fingerprint")
        .or_else(|| str_at(map, "fingerprint"))
        .unwrap_or_default();

    if let Some(reality) = reality {
        server.security = Security::Reality;
        server.reality_public_key = str_at(reality, "public-key")
            .or_else(|| str_at(reality, "public_key"))
            .unwrap_or_default();
        server.reality_short_id = str_at(reality, "short-id")
            .or_else(|| str_at(reality, "short_id"))
            .unwrap_or_default();
        server.reality_mldsa65_verify = str_at(reality, "mldsa65-verify")
            .or_else(|| str_at(reality, "mldsa65_verify"))
            .unwrap_or_default();
        if server.fingerprint.is_empty() {
            server.fingerprint = "chrome".into();
        }
    } else if tls_enabled {
        server.security = Security::Tls;
        if server.fingerprint.is_empty() {
            server.fingerprint = "chrome".into();
        }
    } else {
        server.security = Security::None;
    }
}

fn apply_shadowsocks_plugin(server: &mut Server, map: &Mapping) {
    let plugin = str_at(map, "plugin").unwrap_or_default();
    if plugin.is_empty() {
        return;
    }
    server.ss_plugin = match plugin.as_str() {
        "obfs" | "simple-obfs" | "obfs-local" => "obfs-local".into(),
        other => other.to_string(),
    };
    let mut opts: Vec<String> = Vec::new();
    if let Some(options) = map_at(map, "plugin-opts") {
        if server.ss_plugin == "obfs-local" {
            let mode = str_at(options, "mode").unwrap_or_else(|| "http".into());
            opts.push(format!("obfs={mode}"));
            if let Some(host) = str_at(options, "host") {
                opts.push(format!("obfs-host={host}"));
            }
        } else {
            if let Some(mode) = str_at(options, "mode") {
                opts.push(format!("mode={mode}"));
            }
            if let Some(host) = str_at(options, "host") {
                opts.push(format!("host={host}"));
            }
            if let Some(path) = str_at(options, "path") {
                opts.push(format!("path={path}"));
            }
            if bool_at(options, "tls") {
                opts.push("tls".into());
            }
        }
    }
    server.ss_plugin_opts = opts.join(";");
}

fn from_clash_proxy(map: &Mapping) -> Result<Option<Server>, String> {
    let kind = str_at(map, "type").unwrap_or_default().to_ascii_lowercase();
    let protocol = match kind.as_str() {
        "ss" | "shadowsocks" => Protocol::Shadowsocks,
        "vmess" => Protocol::Vmess,
        "vless" => Protocol::Vless,
        "trojan" => Protocol::Trojan,
        "hysteria2" | "hy2" => Protocol::Hysteria2,
        "tuic" => Protocol::Tuic,
        "masque" => Protocol::Masque,
        _ => return Ok(None),
    };

    let mut server = Server::new(protocol);
    server.name = str_at(map, "name").unwrap_or_default();
    server.address = str_at(map, "server").unwrap_or_default();
    server.port = u16_at(map, "port");
    if server.address.is_empty() {
        return Err("не указан адрес".into());
    }
    if server.port == 0 {
        server.port = protocol.default_port();
    }
    server.subscription_id = None;

    match protocol {
        Protocol::Shadowsocks => {
            server.method = str_at(map, "cipher")
                .or_else(|| str_at(map, "method"))
                .unwrap_or_else(|| "aes-256-gcm".into());
            server.password = str_at(map, "password").unwrap_or_default();
            apply_shadowsocks_plugin(&mut server, map);
        }
        Protocol::Vmess => {
            server.uuid = str_at(map, "uuid").unwrap_or_default();
            server.vmess_alter_id = u16_at(map, "alterId");
            server.vmess_security = str_at(map, "cipher").unwrap_or_else(|| "auto".into());
            apply_tls(&mut server, map, false);
            apply_network(&mut server, map);
        }
        Protocol::Vless => {
            server.uuid = str_at(map, "uuid").unwrap_or_default();
            server.flow = str_at(map, "flow").unwrap_or_default();
            apply_tls(&mut server, map, bool_at(map, "tls"));
            apply_network(&mut server, map);
        }
        Protocol::Trojan => {
            server.password = str_at(map, "password").unwrap_or_default();
            apply_tls(&mut server, map, true);
            apply_network(&mut server, map);
        }
        Protocol::Hysteria2 => {
            server.password = str_at(map, "password").unwrap_or_default();
            server.security = Security::Tls;
            server.sni = str_at(map, "sni")
                .or_else(|| str_at(map, "servername"))
                .unwrap_or_default();
            server.allow_insecure = bool_at(map, "skip-cert-verify");
            server.alpn = list_at(map, "alpn");
            server.obfs = str_at(map, "obfs").unwrap_or_default();
            server.obfs_password = str_at(map, "obfs-password").unwrap_or_default();
            server.up_mbps = u32_at(map, "up");
            server.down_mbps = u32_at(map, "down");
        }
        Protocol::Tuic => {
            server.uuid = str_at(map, "uuid").unwrap_or_default();
            server.password = str_at(map, "password").unwrap_or_default();
            server.security = Security::Tls;
            server.sni = str_at(map, "sni")
                .or_else(|| str_at(map, "servername"))
                .unwrap_or_default();
            server.allow_insecure = bool_at(map, "skip-cert-verify");
            server.alpn = list_at(map, "alpn");
            server.congestion_control = str_at(map, "congestion-controller")
                .or_else(|| str_at(map, "congestion_control"))
                .unwrap_or_else(|| "bbr".into());
            server.udp_relay_mode = str_at(map, "udp-relay-mode")
                .or_else(|| str_at(map, "udp_relay_mode"))
                .unwrap_or_else(|| "native".into());
        }
        Protocol::Masque => {
            server.security = Security::Tls;
            server.masque_user = str_at(map, "username")
                .or_else(|| str_at(map, "user"))
                .unwrap_or_default();
            server.masque_pass = str_at(map, "password").unwrap_or_default();
            server.sni = str_at(map, "sni")
                .or_else(|| str_at(map, "servername"))
                .unwrap_or_default();
            server.host = str_at(map, "host").unwrap_or_default();
            server.path = str_at(map, "path").unwrap_or_default();
            server.pinned_peer_cert_sha256 = str_at(map, "pinned-peer-cert-sha256")
                .or_else(|| str_at(map, "pinned_peer_cert_sha256"))
                .unwrap_or_default();
            server.masque_remote_dns = str_at(map, "remote-dns")
                .or_else(|| str_at(map, "remote_dns"))
                .unwrap_or_default();
        }
        // Clash has no OpenVPN proxy type; it never produces this protocol.
        Protocol::OpenVpn => {}
    }

    if server.name.is_empty() {
        server.name = server.target();
    }
    Ok(Some(server))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clash_subscription() {
        let yaml = r#"
proxies:
  - name: "VLESS Reality"
    type: vless
    server: example.com
    port: 443
    uuid: 11111111-1111-1111-1111-111111111111
    network: grpc
    tls: true
    servername: cdn.example.com
    client-fingerprint: chrome
    grpc-opts:
      grpc-service-name: svc
    reality-opts:
      public-key: jNXHt1yRo0vDuchQlIP6Z0ZvjT3KtzVI-T4E7RoLJS0
      short-id: "0123"
  - name: "SS"
    type: ss
    server: ss.example.com
    port: 8388
    cipher: aes-256-gcm
    password: secret
  - name: unsupported
    type: socks5
    server: localhost
    port: 1080
"#;
        let servers = parse_clash_yaml(yaml).unwrap();
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0].protocol, Protocol::Vless);
        assert_eq!(servers[0].security, Security::Reality);
        assert_eq!(servers[0].transport, Transport::Grpc);
        assert_eq!(servers[0].service_name, "svc");
        assert_eq!(servers[1].password, "secret");
    }
}
