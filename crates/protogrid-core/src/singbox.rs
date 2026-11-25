use std::collections::BTreeMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};
use url::Url;

use crate::model::{
    CoreCapabilities, CoreKind, Protocol, RoutingRule, RuleAction, RuleKind, Security, Server,
    Settings, Transport,
};
use crate::ruleset::{self, sanitize_code};
use crate::util::{split_list, version_at_least};

/// Build a complete sing-box configuration (targets sing-box 1.12+).
///
/// All valid servers are included as outbounds, so the Clash API can measure
/// latency for every node without restarting the core. `core_version` lets us
/// emit only options understood by the installed version (the HTTP client and
/// TUN stack options changed in 1.14/1.15).
pub fn build_config(
    servers: &[Server],
    active_id: &str,
    settings: &Settings,
    rules: &[RoutingRule],
    core_version: Option<&str>,
    rule_set_dir: Option<&Path>,
) -> Result<Value, String> {
    build_config_with_capabilities(
        servers,
        active_id,
        settings,
        rules,
        core_version,
        rule_set_dir,
        CoreCapabilities::default(),
    )
}

/// Build a sing-box configuration with explicit core capabilities.
///
/// `capabilities` is used to decide whether transports missing from stock
/// sing-box (currently XHTTP) may be emitted.
#[allow(clippy::too_many_arguments)]
pub fn build_config_with_capabilities(
    servers: &[Server],
    active_id: &str,
    settings: &Settings,
    rules: &[RoutingRule],
    core_version: Option<&str>,
    rule_set_dir: Option<&Path>,
    capabilities: CoreCapabilities,
) -> Result<Value, String> {
    let modern_http_client = core_version
        .map(|version| version_at_least(version, "1.14.0"))
        .unwrap_or(true);
    let modern_tun = core_version
        .map(|version| version_at_least(version, "1.15.0"))
        .unwrap_or(false);
    if settings.mixed_port == 0 {
        return Err("не задан порт mixed inbound".into());
    }
    if settings.clash_api_port == 0 {
        return Err("не задан порт Clash API".into());
    }

    let active = servers
        .iter()
        .find(|server| server.id == active_id)
        .ok_or_else(|| "активный сервер не найден".to_string())?;
    active
        .validate_with(CoreKind::Singbox, capabilities)
        .map_err(|e| format!("Сервер «{}»: {e}", active.display_name()))?;

    let mut valid: Vec<&Server> = Vec::new();
    for server in servers {
        match server.validate_with(CoreKind::Singbox, capabilities) {
            Ok(()) => valid.push(server),
            Err(error) => {
                if server.id == active_id {
                    return Err(format!("Сервер «{}»: {error}", server.display_name()));
                }
            }
        }
    }
    if valid.is_empty() {
        return Err("нет ни одного корректного сервера".into());
    }

    let inbounds = build_inbounds(settings, modern_tun);
    let rule_sets = collect_rule_sets(rules, rule_set_dir);
    let use_http_client = modern_http_client && rule_set_dir.is_none() && !rule_sets.is_empty();
    let mut outbounds = Vec::new();
    let ids: Vec<String> = valid.iter().map(|s| s.id.clone()).collect();
    if settings.auto_select {
        outbounds.push(json!({
            "type": "urltest",
            "tag": "proxy",
            "outbounds": ids,
            "url": settings.test_url,
            "interval": "3m",
            "tolerance": 50,
        }));
    } else {
        outbounds.push(json!({
            "type": "selector",
            "tag": "proxy",
            "outbounds": ids,
            "default": active_id,
        }));
    }
    for server in &valid {
        outbounds.push(outbound(server, capabilities)?);
    }
    outbounds.push(json!({"type": "direct", "tag": "direct"}));
    if use_http_client {
        // A detour for the rule-set HTTP client must not be an "empty" direct
        // outbound, so give it an explicit domain resolver.
        outbounds.push(json!({
            "type": "direct",
            "tag": "direct-http-out",
            "domain_resolver": "dns-local",
        }));
    }
    outbounds.push(json!({"type": "block", "tag": "block"}));

    let mut route_rules = vec![
        json!({"action": "sniff"}),
        json!({"protocol": "dns", "action": "hijack-dns"}),
        json!({"ip_is_private": true, "outbound": "direct"}),
    ];
    for rule in rules.iter().filter(|rule| rule.enabled) {
        if let Some(value) = route_rule(rule, &rule_sets) {
            route_rules.push(value);
        }
    }

    let final_tag = match settings.final_action {
        RuleAction::Proxy => "proxy",
        RuleAction::Direct => "direct",
        RuleAction::Block => "block",
    };

    let mut route = json!({
        "rules": route_rules,
        "final": final_tag,
        "auto_detect_interface": true,
        "default_domain_resolver": "dns-local",
    });
    if use_http_client {
        route["default_http_client"] = json!("direct-http");
    }
    if !rule_sets.is_empty() {
        route["rule_set"] = Value::Array(
            rule_sets
                .values()
                .map(|rs| {
                    if let Some(path) = &rs.path {
                        json!({
                            "tag": rs.tag,
                            "type": "local",
                            "format": "binary",
                            "path": path,
                        })
                    } else if modern_http_client {
                        json!({
                            "tag": rs.tag,
                            "type": "remote",
                            "format": "binary",
                            "url": rs.url,
                            "update_interval": "7d",
                        })
                    } else {
                        json!({
                            "tag": rs.tag,
                            "type": "remote",
                            "format": "binary",
                            "url": rs.url,
                            "download_detour": "direct",
                            "update_interval": "7d",
                        })
                    }
                })
                .collect(),
        );
    }

    let strategy = if settings.ipv6 {
        "prefer_ipv4"
    } else {
        "ipv4_only"
    };

    let mut config = json!({
        "log": {
            "level": settings.log_level,
            "timestamp": true,
        },
        "dns": {
            "servers": [
                {"type": "local", "tag": "dns-local"},
                remote_dns(settings),
            ],
            "rules": [
                {"domain_suffix": [".local", "localhost"], "server": "dns-local"},
            ],
            "final": "dns-remote",
            "strategy": strategy,
        },
        "inbounds": inbounds,
        "outbounds": outbounds,
        "route": route,
        "experimental": {
            "clash_api": {
                "external_controller": format!("127.0.0.1:{}", settings.clash_api_port),
            },
        },
    });
    if settings.fake_dns {
        if let Some(servers) = config["dns"]["servers"].as_array_mut() {
            servers.push(json!({
                "type": "fakeip",
                "tag": "dns-fake",
                "inet4_range": "198.18.0.0/15",
                "inet6_range": "fc00::/18",
            }));
        }
        if let Some(rules) = config["dns"]["rules"].as_array_mut() {
            rules.push(json!({
                "query_type": ["A", "AAAA"],
                "server": "dns-fake",
            }));
        }
    }
    if use_http_client {
        config["http_clients"] = json!([
            {"tag": "direct-http", "detour": "direct-http-out"}
        ]);
    }
    Ok(config)
}

fn build_inbounds(settings: &Settings, modern_tun: bool) -> Vec<Value> {
    let listen = if settings.allow_lan {
        "0.0.0.0"
    } else {
        "127.0.0.1"
    };
    let mut inbounds = vec![json!({
        "type": "mixed",
        "tag": "mixed-in",
        "listen": listen,
        "listen_port": settings.mixed_port,
    })];

    if settings.tun_enabled {
        let mut addresses = vec!["172.19.0.1/30"];
        if settings.ipv6 {
            addresses.push("fdfe:dcba:9876::1/126");
        }
        let mut tun = json!({
            "type": "tun",
            "tag": "tun-in",
            "address": addresses,
            "mtu": 9000,
            "auto_route": true,
            "strict_route": true,
        });
        if !modern_tun {
            tun["stack"] = json!("mixed");
        }
        inbounds.push(tun);
    }
    inbounds
}

fn remote_dns(settings: &Settings) -> Value {
    let spec = settings.dns_remote.trim();
    if let Ok(url) = Url::parse(spec) {
        let scheme = url.scheme().to_ascii_lowercase();
        let kind = match scheme.as_str() {
            "https" => "https",
            "tls" => "tls",
            "quic" => "quic",
            "h3" => "h3",
            "tcp" => "tcp",
            "udp" => "udp",
            _ => "https",
        };
        let server = url.host_str().unwrap_or("1.1.1.1");
        let mut object = Map::new();
        object.insert("type".into(), json!(kind));
        object.insert("tag".into(), json!("dns-remote"));
        object.insert("server".into(), json!(server));
        object.insert("detour".into(), json!("proxy"));
        if let Some(port) = url.port() {
            object.insert("server_port".into(), json!(port));
        }
        if kind == "https" {
            let path = url.path();
            if !path.is_empty() && path != "/" {
                object.insert("path".into(), json!(path));
            }
        }
        return Value::Object(object);
    }

    json!({
        "type": "udp",
        "tag": "dns-remote",
        "server": if spec.is_empty() { "1.1.1.1" } else { spec },
        "detour": "proxy",
    })
}

fn tls_object(server: &Server) -> Option<Value> {
    let forced = matches!(server.protocol, Protocol::Hysteria2 | Protocol::Tuic);
    if server.security == Security::None && !forced {
        return None;
    }

    let mut tls = Map::new();
    tls.insert("enabled".into(), json!(true));

    let sni = server.effective_sni();
    if !sni.is_empty() && sni.parse::<IpAddr>().is_err() {
        tls.insert("server_name".into(), json!(sni));
    }
    if server.allow_insecure {
        tls.insert("insecure".into(), json!(true));
    }
    if !server.alpn.is_empty() {
        tls.insert("alpn".into(), json!(server.alpn));
    }
    if server.fragment_enabled {
        tls.insert("fragment".into(), json!(true));
        tls.insert("record_fragment".into(), json!(true));
    }

    let fingerprint = if server.fingerprint.trim().is_empty() {
        if server.security == Security::Reality {
            "chrome"
        } else {
            ""
        }
    } else {
        server.fingerprint.trim()
    };
    if !fingerprint.is_empty() {
        tls.insert(
            "utls".into(),
            json!({"enabled": true, "fingerprint": fingerprint}),
        );
    }

    if server.security == Security::Reality {
        let mut reality = Map::new();
        reality.insert("enabled".into(), json!(true));
        reality.insert("public_key".into(), json!(server.reality_public_key.trim()));
        if !server.reality_short_id.trim().is_empty() {
            reality.insert("short_id".into(), json!(server.reality_short_id.trim()));
        }
        if !server.reality_mldsa65_verify.trim().is_empty() {
            reality.insert(
                "mldsa65_verify".into(),
                json!(server.reality_mldsa65_verify.trim()),
            );
        }
        tls.insert("reality".into(), Value::Object(reality));
    }

    Some(Value::Object(tls))
}

fn transport_object(
    server: &Server,
    capabilities: CoreCapabilities,
) -> Result<Option<Value>, String> {
    if matches!(server.protocol, Protocol::Hysteria2 | Protocol::Tuic) {
        return Ok(None);
    }
    let path = if server.path.trim().is_empty() {
        "/"
    } else {
        server.path.trim()
    };
    let object = match server.transport {
        Transport::Tcp => return Ok(None),
        Transport::Ws => {
            let mut value = json!({"type": "ws", "path": path});
            if !server.host.trim().is_empty() {
                value["headers"] = json!({"Host": server.host.trim()});
            }
            value
        }
        Transport::Grpc => {
            json!({"type": "grpc", "service_name": server.service_name.trim()})
        }
        Transport::Http => {
            let mut value = json!({"type": "http", "path": path});
            if !server.host.trim().is_empty() {
                value["host"] = json!([server.host.trim()]);
            }
            value
        }
        Transport::HttpUpgrade => {
            let mut value = json!({"type": "httpupgrade", "path": path});
            if !server.host.trim().is_empty() {
                value["host"] = json!(server.host.trim());
            }
            value
        }
        Transport::Xhttp => {
            if !capabilities.xhttp {
                return Err(
                    "sing-box без поддержки XHTTP. Установите форк ProtoGrid или выберите Xray."
                        .into(),
                );
            }
            let mut value = json!({
                "type": "xhttp",
                "path": path,
                "mode": server.effective_xhttp_mode(),
            });
            if !server.host.trim().is_empty() {
                value["host"] = json!(server.host.trim());
            }
            value
        }
        Transport::Kcp => {
            if !capabilities.kcp {
                return Err(
                    "sing-box без поддержки mKCP. Установите форк ProtoGrid или выберите Xray."
                        .into(),
                );
            }
            json!({
                "type": "kcp",
                "mtu": server.kcp_mtu.max(21),
                "tti": server.kcp_tti.clamp(10, 1000),
                "uplink_capacity": server.kcp_uplink.max(1),
                "downlink_capacity": server.kcp_downlink.max(1),
                "congestion": server.kcp_congestion,
            })
        }
        Transport::XDrive => {
            if !capabilities.xdrive {
                return Err("sing-box без поддержки XDRIVE. Установите форк ProtoGrid.".into());
            }
            let mut value = json!({
                "type": "xdrive",
                "service": if server.xdrive_service.trim().is_empty() {
                    "local"
                } else {
                    server.xdrive_service.trim()
                },
            });
            if !server.xdrive_remote_folder.trim().is_empty() {
                value["remote_folder"] = json!(server.xdrive_remote_folder.trim());
            }
            let secrets: Vec<&str> = server
                .xdrive_secrets
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .collect();
            if !secrets.is_empty() {
                value["secrets"] = json!(secrets);
            }
            if !server.xdrive_s3_endpoint.trim().is_empty() {
                value["s3"] = json!({
                    "endpoint": server.xdrive_s3_endpoint.trim(),
                    "region": server.xdrive_s3_region.trim(),
                    "bucket": server.xdrive_s3_bucket.trim(),
                    "path_style": server.xdrive_s3_path_style,
                });
            }
            value
        }
    };
    Ok(Some(object))
}

fn outbound(server: &Server, capabilities: CoreCapabilities) -> Result<Value, String> {
    let tag = &server.id;
    let address = server.address.trim();
    let port = server.port;
    let tls = tls_object(server);
    let transport = transport_object(server, capabilities)?;

    let mut object = match server.protocol {
        Protocol::Vless => {
            let mut value = json!({
                "type": "vless",
                "tag": tag,
                "server": address,
                "server_port": port,
                "uuid": server.uuid.trim(),
            });
            if !server.encryption.trim().is_empty() {
                value["encryption"] = json!(server.encryption.trim());
            }
            value
        }
        Protocol::Vmess => json!({
            "type": "vmess",
            "tag": tag,
            "server": address,
            "server_port": port,
            "uuid": server.uuid.trim(),
            "security": if server.vmess_security.is_empty() { "auto" } else { server.vmess_security.as_str() },
            "alter_id": server.vmess_alter_id,
        }),
        Protocol::Trojan => json!({
            "type": "trojan",
            "tag": tag,
            "server": address,
            "server_port": port,
            "password": server.password,
        }),
        Protocol::Shadowsocks => {
            let mut value = json!({
                "type": "shadowsocks",
                "tag": tag,
                "server": address,
                "server_port": port,
                "method": server.method.trim(),
                "password": server.password,
            });
            if !server.ss_plugin.trim().is_empty() {
                value["plugin"] = json!(server.ss_plugin.trim());
                value["plugin_opts"] = json!(server.ss_plugin_opts);
            }
            value
        }
        Protocol::Hysteria2 => {
            let mut value = json!({
                "type": "hysteria2",
                "tag": tag,
                "server": address,
                "server_port": port,
                "password": server.password,
            });
            if !server.obfs.trim().is_empty() {
                value["obfs"] = json!({
                    "type": server.obfs.trim(),
                    "password": server.obfs_password,
                });
            }
            if server.up_mbps > 0 {
                value["up_mbps"] = json!(server.up_mbps);
            }
            if server.down_mbps > 0 {
                value["down_mbps"] = json!(server.down_mbps);
            }
            value
        }
        Protocol::Tuic => json!({
            "type": "tuic",
            "tag": tag,
            "server": address,
            "server_port": port,
            "uuid": server.uuid.trim(),
            "password": server.password,
            "congestion_control": if server.congestion_control.is_empty() { "bbr" } else { server.congestion_control.as_str() },
            "udp_relay_mode": if server.udp_relay_mode.is_empty() { "native" } else { server.udp_relay_mode.as_str() },
        }),
        Protocol::Masque => {
            let mut value = json!({
                "type": "masque",
                "tag": tag,
                "server": address,
                "server_port": port,
                "tls": {
                    "enabled": true,
                    "server_name": server.effective_sni(),
                },
            });
            if !server.host.trim().is_empty() {
                value["host"] = json!(server.host.trim());
            }
            if !server.path.trim().is_empty() {
                value["path"] = json!(server.path.trim());
            }
            if !server.masque_user.trim().is_empty() {
                value["user"] = json!(server.masque_user.trim());
                value["pass"] = json!(server.masque_pass);
            }
            if !server.pinned_peer_cert_sha256.trim().is_empty() {
                value["pinned_peer_cert_sha256"] = json!(server.pinned_peer_cert_sha256.trim());
            }
            let remote_dns: Vec<&str> = server
                .masque_remote_dns
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .collect();
            if !remote_dns.is_empty() {
                value["remote_dns"] = json!(remote_dns);
            }
            value
        }
    };

    if server.protocol == Protocol::Vless && !server.flow.trim().is_empty() {
        object["flow"] = json!(server.flow.trim());
    }
    if server.protocol == Protocol::Vless {
        object["packet_encoding"] = json!("xudp");
    }
    if server.tcp_fast_open {
        object["tcp_fast_open"] = json!(true);
    }
    if !server.bind_interface.trim().is_empty() {
        object["bind_interface"] = json!(server.bind_interface.trim());
    }
    if server.routing_mark > 0 {
        object["routing_mark"] = json!(server.routing_mark);
    }
    if let Some(tls) = tls {
        object["tls"] = tls;
    }
    if let Some(transport) = transport {
        object["transport"] = transport;
    }
    Ok(object)
}

#[derive(Debug, Clone)]
struct RuleSetDef {
    tag: String,
    url: String,
    path: Option<PathBuf>,
}

fn collect_rule_sets(rules: &[RoutingRule], dir: Option<&Path>) -> BTreeMap<String, RuleSetDef> {
    let mut sets = BTreeMap::new();
    for source in ruleset::sources(rules) {
        if let Some(dir) = dir {
            let path = ruleset::local_path(dir, &source.tag);
            if path.is_file() {
                sets.insert(
                    source.tag.clone(),
                    RuleSetDef {
                        tag: source.tag,
                        url: source.url,
                        path: Some(path),
                    },
                );
            }
        } else {
            sets.insert(
                source.tag.clone(),
                RuleSetDef {
                    tag: source.tag,
                    url: source.url,
                    path: None,
                },
            );
        }
    }
    sets
}

fn route_rule(rule: &RoutingRule, rule_sets: &BTreeMap<String, RuleSetDef>) -> Option<Value> {
    let values = split_list(&rule.value);
    if values.is_empty() {
        return None;
    }

    let mut object = Map::new();
    match rule.kind {
        RuleKind::Domain => {
            object.insert("domain".into(), json!(values));
        }
        RuleKind::DomainSuffix => {
            object.insert("domain_suffix".into(), json!(values));
        }
        RuleKind::DomainKeyword => {
            object.insert("domain_keyword".into(), json!(values));
        }
        RuleKind::IpCidr => {
            object.insert("ip_cidr".into(), json!(values));
        }
        RuleKind::ProcessName => {
            object.insert("process_name".into(), json!(values));
        }
        RuleKind::Geoip | RuleKind::Geosite => {
            let prefix = match rule.kind {
                RuleKind::Geoip => "geoip",
                _ => "geosite",
            };
            let tags: Vec<String> = values
                .iter()
                .map(|code| format!("{prefix}-{}", sanitize_code(code)))
                .filter(|tag| rule_sets.contains_key(tag))
                .collect();
            if tags.is_empty() {
                return None;
            }
            object.insert("rule_set".into(), json!(tags));
        }
    }

    match rule.action {
        RuleAction::Block => {
            object.insert("action".into(), json!("reject"));
        }
        RuleAction::Proxy => {
            object.insert("outbound".into(), json!("proxy"));
        }
        RuleAction::Direct => {
            object.insert("outbound".into(), json!("direct"));
        }
    }
    Some(Value::Object(object))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{default_rules, Protocol, Security};

    fn sample_server() -> Server {
        let mut server = Server::new(Protocol::Vless);
        server.id = "srv-1".into();
        server.name = "Test".into();
        server.address = "example.com".into();
        server.port = 443;
        server.uuid = "11111111-1111-1111-1111-111111111111".into();
        server.security = Security::Reality;
        server.reality_public_key = "jNXHt1yRo0vDuchQlIP6Z0ZvjT3KtzVI-T4E7RoLJS0".into();
        server.fingerprint = "chrome".into();
        server
    }

    #[test]
    fn builds_config() {
        let server = sample_server();
        let settings = Settings::default();
        let config = build_config(
            &[server],
            "srv-1",
            &settings,
            &default_rules(),
            Some("1.14.4"),
            None,
        )
        .unwrap();
        assert_eq!(config["inbounds"][0]["type"], "mixed");
        assert!(config["http_clients"].is_array());
        assert_eq!(config["http_clients"][0]["detour"], "direct-http-out");
        assert!(config["route"]["default_http_client"].is_string());
        assert!(config["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|outbound| outbound["tag"] == "direct-http-out"));
        assert_eq!(config["outbounds"][0]["tag"], "proxy");
        assert_eq!(config["route"]["final"], "proxy");
        assert_eq!(config["dns"]["servers"][0]["type"], "local");
        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("dns-remote"));
    }

    #[test]
    fn tun_inbound_added() {
        let server = sample_server();
        let settings = Settings {
            tun_enabled: true,
            ..Default::default()
        };
        let config =
            build_config(&[server], "srv-1", &settings, &[], Some("1.14.4"), None).unwrap();
        assert!(config["inbounds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["type"] == "tun"));
    }

    #[test]
    fn older_singbox_uses_legacy_rule_set_detour() {
        let server = sample_server();
        let config = build_config(
            &[server],
            "srv-1",
            &Settings::default(),
            &default_rules(),
            Some("1.13.0"),
            None,
        )
        .unwrap();
        assert!(config.get("http_clients").is_none());
        assert_eq!(config["route"]["rule_set"][0]["download_detour"], "direct");
    }

    #[test]
    fn local_rule_sets_are_used_when_available() {
        let server = sample_server();
        let dir = std::env::temp_dir().join(format!("protogrid-rules-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("geoip-ru.srs"), b"not-a-real-srs").unwrap();
        let config = build_config(
            &[server],
            "srv-1",
            &Settings::default(),
            &default_rules(),
            Some("1.14.4"),
            Some(&dir),
        )
        .unwrap();
        assert!(config.get("http_clients").is_none());
        let sets = config["route"]["rule_set"].as_array().unwrap();
        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0]["type"], "local");
        assert!(sets[0]["path"].as_str().unwrap().ends_with("geoip-ru.srs"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn xhttp_requires_core_capability() {
        let mut server = sample_server();
        server.transport = Transport::Xhttp;
        server.xhttp_mode = "packet-up".into();
        server.security = Security::Tls;
        server.reality_public_key.clear();
        let settings = Settings::default();

        let stock = build_config(
            &[server.clone()],
            "srv-1",
            &settings,
            &[],
            Some("1.14.4"),
            None,
        );
        assert!(stock.is_err());

        let fork = build_config_with_capabilities(
            &[server],
            "srv-1",
            &settings,
            &[],
            Some("1.14.4-protogrid.1"),
            None,
            CoreCapabilities {
                xhttp: true,
                kcp: true,
                xdrive: true,
            },
        )
        .unwrap();
        let outbound = fork["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|outbound| outbound["tag"] == "srv-1")
            .unwrap();
        assert_eq!(outbound["transport"]["type"], "xhttp");
        assert_eq!(outbound["transport"]["mode"], "packet-up");
    }

    #[test]
    fn fake_dns_and_auto_select() {
        let server = sample_server();
        let settings = Settings {
            fake_dns: true,
            auto_select: true,
            ..Default::default()
        };
        let config =
            build_config(&[server], "srv-1", &settings, &[], Some("1.14.4"), None).unwrap();
        assert!(config["dns"]["servers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|server| server["type"] == "fakeip"));
        assert!(config["dns"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .any(|rule| rule["server"] == "dns-fake"));
        assert_eq!(config["outbounds"][0]["type"], "urltest");
        assert_eq!(config["outbounds"][0]["tag"], "proxy");
    }

    #[test]
    fn tls_fragment_emitted() {
        let mut server = sample_server();
        server.security = Security::Tls;
        server.reality_public_key.clear();
        server.fragment_enabled = true;
        let config =
            build_config(&[server], "srv-1", &Settings::default(), &[], None, None).unwrap();
        let outbound = config["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|outbound| outbound["tag"] == "srv-1")
            .unwrap();
        assert_eq!(outbound["tls"]["fragment"], true);
        assert_eq!(outbound["tls"]["record_fragment"], true);
    }

    #[test]
    fn masque_singbox_outbound() {
        let mut server = sample_server();
        server.protocol = Protocol::Masque;
        server.security = Security::Tls;
        server.reality_public_key.clear();
        server.masque_user = "user".into();
        server.masque_pass = "pass".into();
        server.pinned_peer_cert_sha256 = "abcd".into();
        let config =
            build_config(&[server], "srv-1", &Settings::default(), &[], None, None).unwrap();
        let outbound = config["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|outbound| outbound["tag"] == "srv-1")
            .unwrap();
        assert_eq!(outbound["type"], "masque");
        assert_eq!(outbound["user"], "user");
        assert_eq!(outbound["pinned_peer_cert_sha256"], "abcd");
    }

    #[test]
    fn kcp_requires_core_capability() {
        let mut server = sample_server();
        server.transport = Transport::Kcp;
        server.security = Security::None;
        server.reality_public_key.clear();
        server.kcp_mtu = 1350;
        server.kcp_tti = 50;
        let settings = Settings::default();

        let stock = build_config(
            &[server.clone()],
            "srv-1",
            &settings,
            &[],
            Some("1.14.4"),
            None,
        );
        assert!(stock.is_err());

        let fork = build_config_with_capabilities(
            &[server],
            "srv-1",
            &settings,
            &[],
            Some("1.14.4-protogrid.1"),
            None,
            CoreCapabilities {
                xhttp: true,
                kcp: true,
                xdrive: true,
            },
        )
        .unwrap();
        let outbound = fork["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|outbound| outbound["tag"] == "srv-1")
            .unwrap();
        assert_eq!(outbound["transport"]["type"], "kcp");
        assert_eq!(outbound["transport"]["mtu"], 1350);
    }

    #[test]
    fn xdrive_requires_core_capability() {
        let mut server = sample_server();
        server.transport = Transport::XDrive;
        server.security = Security::None;
        server.reality_public_key.clear();
        server.xdrive_service = "local".into();
        server.xdrive_remote_folder = "/tmp/xdrive".into();
        let settings = Settings::default();

        let stock = build_config(
            &[server.clone()],
            "srv-1",
            &settings,
            &[],
            Some("1.14.4"),
            None,
        );
        assert!(stock.is_err());

        let fork = build_config_with_capabilities(
            &[server],
            "srv-1",
            &settings,
            &[],
            Some("1.14.4-protogrid.1"),
            None,
            CoreCapabilities {
                xhttp: true,
                kcp: true,
                xdrive: true,
            },
        )
        .unwrap();
        let outbound = fork["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|outbound| outbound["tag"] == "srv-1")
            .unwrap();
        assert_eq!(outbound["transport"]["type"], "xdrive");
        assert_eq!(outbound["transport"]["service"], "local");
    }

    #[test]
    fn xdrive_s3_config() {
        let mut server = sample_server();
        server.transport = Transport::XDrive;
        server.security = Security::None;
        server.reality_public_key.clear();
        server.xdrive_service = "s3".into();
        server.xdrive_remote_folder = "test".into();
        server.xdrive_secrets = "minioadmin,minioadmin".into();
        server.xdrive_s3_endpoint = "http://127.0.0.1:9000".into();
        server.xdrive_s3_region = "us-east-1".into();
        server.xdrive_s3_bucket = "xdrive".into();
        server.xdrive_s3_path_style = true;
        let config = build_config_with_capabilities(
            &[server],
            "srv-1",
            &Settings::default(),
            &[],
            Some("1.14.4-protogrid.1"),
            None,
            CoreCapabilities {
                xhttp: true,
                kcp: true,
                xdrive: true,
            },
        )
        .unwrap();
        let outbound = config["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|outbound| outbound["tag"] == "srv-1")
            .unwrap();
        assert_eq!(outbound["transport"]["service"], "s3");
        assert_eq!(outbound["transport"]["s3"]["bucket"], "xdrive");
        assert_eq!(outbound["transport"]["secrets"][0], "minioadmin");
    }

    #[test]
    fn skips_broken_inactive_server() {
        let server = sample_server();
        let mut broken = sample_server();
        broken.id = "broken".into();
        broken.uuid.clear();
        let config = build_config(
            &[server, broken],
            "srv-1",
            &Settings::default(),
            &[],
            Some("1.14.4"),
            None,
        )
        .unwrap();
        let tags: Vec<&str> = config["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|o| o["tag"].as_str())
            .collect();
        assert!(tags.contains(&"srv-1"));
        assert!(!tags.contains(&"broken"));
    }
}
