use serde_json::{json, Map, Value};

use crate::model::{
    CoreKind, Protocol, RoutingRule, RuleAction, RuleKind, Security, Server, Settings, Transport,
};
use crate::util::split_list;

/// Build an Xray-core configuration. Only the active server is used as the
/// proxy outbound; Xray has no equivalent of a sing-box selector.
pub fn build_config(
    servers: &[Server],
    active_id: &str,
    settings: &Settings,
    rules: &[RoutingRule],
) -> Result<Value, String> {
    if settings.tun_enabled {
        return Err("TUN-режим поддерживается только ядром sing-box".into());
    }
    if settings.socks_port == settings.http_port {
        return Err("SOCKS и HTTP порты Xray должны различаться".into());
    }

    let active = servers
        .iter()
        .find(|server| server.id == active_id)
        .ok_or_else(|| "активный сервер не найден".to_string())?;
    active
        .validate(CoreKind::Xray)
        .map_err(|e| format!("Сервер «{}»: {e}", active.display_name()))?;
    if matches!(active.protocol, Protocol::Hysteria2 | Protocol::Tuic) {
        return Err(format!(
            "{} не поддерживается Xray. Выберите sing-box.",
            active.protocol.label()
        ));
    }

    let listen = if settings.allow_lan {
        "0.0.0.0"
    } else {
        "127.0.0.1"
    };
    let sniffing = json!({
        "enabled": true,
        "destOverride": ["http", "tls", "quic"],
        "routeOnly": false,
    });
    let inbounds = vec![
        json!({
            "tag": "socks-in",
            "listen": listen,
            "port": settings.socks_port,
            "protocol": "socks",
            "settings": {"auth": "noauth", "udp": true},
            "sniffing": sniffing,
        }),
        json!({
            "tag": "http-in",
            "listen": listen,
            "port": settings.http_port,
            "protocol": "http",
            "settings": {},
            "sniffing": sniffing,
        }),
    ];

    let mut outbounds = Vec::new();
    if settings.auto_select {
        let mut valid_servers = Vec::new();
        for server in servers {
            if server.validate(CoreKind::Xray).is_ok()
                && !matches!(server.protocol, Protocol::Hysteria2 | Protocol::Tuic)
            {
                valid_servers.push(server);
            }
        }
        if valid_servers.is_empty() {
            return Err("нет ни одного корректного сервера для Xray".into());
        }
        for (index, server) in valid_servers.iter().enumerate() {
            outbounds.push(xray_outbound(server, &format!("srv-{index}"))?);
        }
    } else {
        outbounds.push(xray_outbound(active, "proxy")?);
    }
    outbounds.push(json!({"tag": "direct", "protocol": "freedom"}));
    outbounds.push(json!({"tag": "block", "protocol": "blackhole"}));

    let mut route_rules = vec![json!({
        "type": "field",
        "ip": ["geoip:private"],
        "outboundTag": "direct",
    })];
    for rule in rules.iter().filter(|rule| rule.enabled) {
        if let Some(value) = route_rule(rule) {
            route_rules.push(value);
        }
    }
    match settings.final_action {
        RuleAction::Proxy => {}
        RuleAction::Direct => route_rules.push(json!({
            "type": "field",
            "network": "tcp,udp",
            "outboundTag": "direct",
        })),
        RuleAction::Block => route_rules.push(json!({
            "type": "field",
            "network": "tcp,udp",
            "outboundTag": "block",
        })),
    }
    if settings.auto_select {
        route_rules.push(json!({
            "type": "field",
            "network": "tcp,udp",
            "balancerTag": "proxy",
        }));
    }

    let query_strategy = if settings.ipv6 { "UseIP" } else { "UseIPv4" };
    let dns_remote = if settings.dns_remote.trim().is_empty() {
        "1.1.1.1".to_string()
    } else {
        settings.dns_remote.trim().to_string()
    };

    let mut config = json!({
        "log": {
            "loglevel": xray_log_level(&settings.log_level),
        },
        "dns": {
            "servers": [dns_remote, "localhost"],
            "queryStrategy": query_strategy,
        },
        "inbounds": inbounds,
        "outbounds": outbounds,
        "routing": {
            "domainStrategy": "IPIfNonMatch",
            "rules": route_rules,
        },
    });
    if settings.auto_select {
        config["observatory"] = json!({
            "subjectSelector": ["srv-"],
            "probeURL": settings.test_url,
            "probeInterval": "1m",
            "enableConcurrency": true,
        });
        config["routing"]["balancers"] = json!([{
            "tag": "proxy",
            "selector": ["srv-"],
            "strategy": {"type": "leastLoad"},
            "fallbackTag": "direct",
        }]);
    }
    Ok(config)
}

fn xray_masque_outbound(server: &Server) -> Result<Value, String> {
    if server.security != Security::Tls {
        return Err("MASQUE требует TLS".into());
    }
    let mut stream = Map::new();
    stream.insert("network".into(), json!("masque"));
    stream.insert("security".into(), json!("tls"));
    let mut tls = Map::new();
    let sni = server.effective_sni();
    if !sni.is_empty() {
        tls.insert("serverName".into(), json!(sni));
    }
    if !server.pinned_peer_cert_sha256.trim().is_empty() {
        tls.insert(
            "pinnedPeerCertSha256".into(),
            json!(server.pinned_peer_cert_sha256.trim()),
        );
    }
    stream.insert("tlsSettings".into(), Value::Object(tls));
    let mut masque = Map::new();
    if !server.host.trim().is_empty() {
        masque.insert("host".into(), json!(server.host.trim()));
    }
    if !server.path.trim().is_empty() {
        masque.insert("path".into(), json!(server.path.trim()));
    }
    if !server.masque_user.trim().is_empty() {
        masque.insert("user".into(), json!(server.masque_user.trim()));
        masque.insert("pass".into(), json!(server.masque_pass));
    }
    stream.insert("masqueSettings".into(), Value::Object(masque));

    let mut settings = Map::new();
    settings.insert("address".into(), json!(server.address.trim()));
    settings.insert("port".into(), json!(server.port));
    let remote_dns: Vec<&str> = server
        .masque_remote_dns
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect();
    if !remote_dns.is_empty() {
        settings.insert("remoteDNS".into(), json!(remote_dns));
    }

    Ok(json!({
        "tag": "proxy",
        "protocol": "masque",
        "settings": Value::Object(settings),
        "streamSettings": Value::Object(stream),
    }))
}

fn xray_log_level(level: &str) -> &'static str {
    match level {
        "trace" => "trace",
        "debug" => "debug",
        "info" => "info",
        "warn" | "warning" => "warning",
        "error" => "error",
        _ => "warning",
    }
}

fn xray_outbound(server: &Server, tag: &str) -> Result<Value, String> {
    if server.protocol == Protocol::Masque {
        return xray_masque_outbound(server);
    }
    let address = server.address.trim();
    let port = server.port;
    let stream = stream_settings(server)?;

    let value = match server.protocol {
        Protocol::Vless => {
            let mut user = Map::new();
            user.insert("id".into(), json!(server.uuid.trim()));
            user.insert(
                "encryption".into(),
                json!(if server.encryption.trim().is_empty() {
                    "none"
                } else {
                    server.encryption.trim()
                }),
            );
            if !server.flow.trim().is_empty() {
                user.insert("flow".into(), json!(server.flow.trim()));
            }
            let mut object = json!({
                "tag": tag,
                "protocol": "vless",
                "settings": {
                    "vnext": [{
                        "address": address,
                        "port": port,
                        "users": [Value::Object(user)],
                    }],
                },
            });
            if let Some(stream) = stream {
                object["streamSettings"] = stream;
            }
            object
        }
        Protocol::Vmess => {
            let mut user = Map::new();
            user.insert("id".into(), json!(server.uuid.trim()));
            user.insert("alterId".into(), json!(server.vmess_alter_id));
            user.insert(
                "security".into(),
                json!(if server.vmess_security.is_empty() {
                    "auto"
                } else {
                    server.vmess_security.as_str()
                }),
            );
            let mut object = json!({
                "tag": tag,
                "protocol": "vmess",
                "settings": {
                    "vnext": [{
                        "address": address,
                        "port": port,
                        "users": [Value::Object(user)],
                    }],
                },
            });
            if let Some(stream) = stream {
                object["streamSettings"] = stream;
            }
            object
        }
        Protocol::Trojan => {
            let mut server_object = Map::new();
            server_object.insert("address".into(), json!(address));
            server_object.insert("port".into(), json!(port));
            server_object.insert("password".into(), json!(server.password));
            let mut object = json!({
                "tag": tag,
                "protocol": "trojan",
                "settings": {
                    "servers": [Value::Object(server_object)],
                },
            });
            if let Some(stream) = stream {
                object["streamSettings"] = stream;
            }
            object
        }
        Protocol::Shadowsocks => {
            let mut server_object = Map::new();
            server_object.insert("address".into(), json!(address));
            server_object.insert("port".into(), json!(port));
            server_object.insert("method".into(), json!(server.method.trim()));
            server_object.insert("password".into(), json!(server.password));
            if !server.ss_plugin.trim().is_empty() {
                server_object.insert("plugin".into(), json!(server.ss_plugin.trim()));
                server_object.insert("pluginOpts".into(), json!(server.ss_plugin_opts));
            }
            json!({
                "tag": tag,
                "protocol": "shadowsocks",
                "settings": {"servers": [Value::Object(server_object)]},
            })
        }
        Protocol::Hysteria2 => {
            return Err("Hysteria2 не поддерживается Xray. Выберите sing-box.".into())
        }
        Protocol::Tuic => return Err("TUIC не поддерживается Xray. Выберите sing-box.".into()),
        Protocol::OpenVpn => {
            return Err("OpenVPN не поддерживается Xray. Выберите sing-box.".into())
        }
        Protocol::Masque => unreachable!("handled above"),
    };
    let mut value = value;
    if server.transport == Transport::XDrive {
        // Xray requires mux for the XDRIVE transport.
        value["mux"] = json!({"enabled": true, "concurrency": 8});
    }
    Ok(value)
}

fn stream_settings(server: &Server) -> Result<Option<Value>, String> {
    if server.protocol == Protocol::Shadowsocks {
        return Ok(None);
    }
    let mut stream = Map::new();

    let (network, settings) = match server.transport {
        Transport::Tcp => ("tcp", None),
        Transport::Ws => {
            let mut ws = Map::new();
            ws.insert(
                "path".into(),
                json!(if server.path.trim().is_empty() {
                    "/"
                } else {
                    server.path.trim()
                }),
            );
            if !server.host.trim().is_empty() {
                ws.insert("host".into(), json!(server.host.trim()));
            }
            ("ws", Some(("wsSettings", Value::Object(ws))))
        }
        Transport::Grpc => {
            let mut grpc = Map::new();
            grpc.insert("serviceName".into(), json!(server.service_name.trim()));
            ("grpc", Some(("grpcSettings", Value::Object(grpc))))
        }
        Transport::Http => {
            return Err(
                "Xray 26+ не поддерживает HTTP/2 транспорт. Выберите XHTTP или sing-box.".into(),
            );
        }
        Transport::HttpUpgrade => {
            let mut http = Map::new();
            http.insert(
                "path".into(),
                json!(if server.path.trim().is_empty() {
                    "/"
                } else {
                    server.path.trim()
                }),
            );
            if !server.host.trim().is_empty() {
                http.insert("host".into(), json!(server.host.trim()));
            }
            (
                "httpupgrade",
                Some(("httpupgradeSettings", Value::Object(http))),
            )
        }
        Transport::Xhttp => {
            let mut xhttp = Map::new();
            xhttp.insert(
                "path".into(),
                json!(if server.path.trim().is_empty() {
                    "/"
                } else {
                    server.path.trim()
                }),
            );
            if !server.host.trim().is_empty() {
                xhttp.insert("host".into(), json!(server.host.trim()));
            }
            xhttp.insert("mode".into(), json!(server.effective_xhttp_mode()));
            ("xhttp", Some(("xhttpSettings", Value::Object(xhttp))))
        }
        Transport::Kcp => {
            let mut kcp = Map::new();
            kcp.insert("mtu".into(), json!(server.kcp_mtu.max(21)));
            kcp.insert("tti".into(), json!(server.kcp_tti.clamp(10, 1000)));
            kcp.insert("uplinkCapacity".into(), json!(server.kcp_uplink.max(1)));
            kcp.insert("downlinkCapacity".into(), json!(server.kcp_downlink.max(1)));
            kcp.insert("congestion".into(), json!(server.kcp_congestion));
            if !server.kcp_seed.trim().is_empty() {
                kcp.insert("seed".into(), json!(server.kcp_seed.trim()));
            }
            ("kcp", Some(("kcpSettings", Value::Object(kcp))))
        }
        Transport::XDrive => {
            let mut xdrive = Map::new();
            xdrive.insert(
                "service".into(),
                json!(if server.xdrive_service.trim().is_empty() {
                    "local"
                } else {
                    server.xdrive_service.trim()
                }),
            );
            if !server.xdrive_remote_folder.trim().is_empty() {
                xdrive.insert(
                    "remoteFolder".into(),
                    json!(server.xdrive_remote_folder.trim()),
                );
            }
            let secrets: Vec<&str> = server
                .xdrive_secrets
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .collect();
            if !secrets.is_empty() {
                xdrive.insert("secrets".into(), json!(secrets));
            }
            if !server.xdrive_s3_endpoint.trim().is_empty() {
                xdrive.insert(
                    "s3".into(),
                    json!({
                        "endpoint": server.xdrive_s3_endpoint.trim(),
                        "region": server.xdrive_s3_region.trim(),
                        "bucket": server.xdrive_s3_bucket.trim(),
                        "pathStyle": server.xdrive_s3_path_style,
                    }),
                );
            }
            ("xdrive", Some(("xdriveSettings", Value::Object(xdrive))))
        }
    };
    stream.insert("network".into(), json!(network));
    if let Some((key, value)) = settings {
        stream.insert(key.into(), value);
    }

    match server.security {
        Security::None => {}
        Security::Tls => {
            let mut tls = Map::new();
            let sni = server.effective_sni();
            if !sni.is_empty() {
                tls.insert("serverName".into(), json!(sni));
            }
            if server.allow_insecure {
                // Xray 26 removed `allowInsecure`. `verifyPeerCertByName`
                // skips CA validation but still checks that the certificate
                // belongs to the requested host.
                let name = server.effective_sni();
                let name = if name.is_empty() {
                    server.address.trim().to_string()
                } else {
                    name
                };
                if !name.is_empty() {
                    tls.insert("verifyPeerCertByName".into(), json!(name));
                }
            }
            if !server.alpn.is_empty() {
                tls.insert("alpn".into(), json!(server.alpn));
            }
            if !server.fingerprint.trim().is_empty() {
                tls.insert("fingerprint".into(), json!(server.fingerprint.trim()));
            }
            stream.insert("security".into(), json!("tls"));
            stream.insert("tlsSettings".into(), Value::Object(tls));
        }
        Security::Reality => {
            let mut reality = Map::new();
            reality.insert("serverName".into(), json!(server.effective_sni()));
            reality.insert(
                "fingerprint".into(),
                json!(if server.fingerprint.trim().is_empty() {
                    "chrome"
                } else {
                    server.fingerprint.trim()
                }),
            );
            reality.insert("publicKey".into(), json!(server.reality_public_key.trim()));
            if !server.reality_short_id.trim().is_empty() {
                reality.insert("shortId".into(), json!(server.reality_short_id.trim()));
            }
            if !server.reality_mldsa65_verify.trim().is_empty() {
                reality.insert(
                    "mldsa65Verify".into(),
                    json!(server.reality_mldsa65_verify.trim()),
                );
            }
            reality.insert(
                "spiderX".into(),
                json!(if server.reality_spider_x.trim().is_empty() {
                    "/"
                } else {
                    server.reality_spider_x.trim()
                }),
            );
            stream.insert("security".into(), json!("reality"));
            stream.insert("realitySettings".into(), Value::Object(reality));
        }
    }

    let mut sockopt = Map::new();
    if server.tcp_fast_open {
        sockopt.insert("tcpFastOpen".into(), json!(true));
    }
    if !server.bind_interface.trim().is_empty() {
        sockopt.insert("interface".into(), json!(server.bind_interface.trim()));
    }
    if server.routing_mark > 0 {
        sockopt.insert("mark".into(), json!(server.routing_mark));
    }
    if !sockopt.is_empty() {
        stream.insert("sockopt".into(), Value::Object(sockopt));
    }

    let mut finalmask = Map::new();
    let mut tcp_masks = Vec::new();
    if server.fragment_enabled {
        tcp_masks.push(json!({
            "type": "fragment",
            "settings": {
                "packets": if server.fragment_packets.trim().is_empty() {
                    "tlshello"
                } else {
                    server.fragment_packets.trim()
                },
                "length": server.fragment_length.trim(),
                "delay": server.fragment_delay.trim(),
                "maxSplit": server.fragment_max_split.trim(),
            }
        }));
    }
    let mut udp_masks = Vec::new();
    if server.noise_enabled {
        let noise_rand = if server.noise_packet.trim().is_empty() {
            server.noise_rand.trim()
        } else {
            "0-0"
        };
        udp_masks.push(json!({
            "type": "noise",
            "settings": {
                "noise": [{
                    "type": if server.noise_type.trim().is_empty() {
                        "str"
                    } else {
                        server.noise_type.trim()
                    },
                    "packet": server.noise_packet,
                    "rand": noise_rand,
                }]
            }
        }));
    }
    if !server.finalmask_tcp.trim().is_empty() {
        let masks: Vec<Value> = serde_json::from_str(server.finalmask_tcp.trim())
            .map_err(|error| format!("finalmask_tcp: {error}"))?;
        tcp_masks.extend(masks);
    }
    if !server.finalmask_udp.trim().is_empty() {
        let masks: Vec<Value> = serde_json::from_str(server.finalmask_udp.trim())
            .map_err(|error| format!("finalmask_udp: {error}"))?;
        udp_masks.extend(masks);
    }
    if !tcp_masks.is_empty() {
        finalmask.insert("tcp".into(), Value::Array(tcp_masks));
    }
    if !udp_masks.is_empty() {
        finalmask.insert("udp".into(), Value::Array(udp_masks));
    }
    if !finalmask.is_empty() {
        stream.insert("finalmask".into(), Value::Object(finalmask));
    }

    Ok(Some(Value::Object(stream)))
}

fn escape_regex(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 2);
    for c in value.chars() {
        if matches!(
            c,
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn route_rule(rule: &RoutingRule) -> Option<Value> {
    let values = split_list(&rule.value);
    if values.is_empty() {
        return None;
    }
    let outbound = match rule.action {
        RuleAction::Proxy => "proxy",
        RuleAction::Direct => "direct",
        RuleAction::Block => "block",
    };

    let mut object = Map::new();
    object.insert("type".into(), json!("field"));
    object.insert("outboundTag".into(), json!(outbound));
    match rule.kind {
        RuleKind::Domain => {
            let domains: Vec<String> = values.iter().map(|v| format!("full:{v}")).collect();
            object.insert("domain".into(), json!(domains));
        }
        RuleKind::DomainSuffix => {
            let domains: Vec<String> = values
                .iter()
                .map(|v| format!("domain:{}", v.trim_start_matches('.')))
                .collect();
            object.insert("domain".into(), json!(domains));
        }
        RuleKind::DomainKeyword => {
            let domains: Vec<String> = values
                .iter()
                .map(|v| format!("regexp:.*{}.*", escape_regex(v)))
                .collect();
            object.insert("domain".into(), json!(domains));
        }
        RuleKind::IpCidr => {
            object.insert("ip".into(), json!(values));
        }
        RuleKind::Geoip => {
            let ips: Vec<String> = values
                .iter()
                .map(|code| format!("geoip:{}", code.to_ascii_lowercase()))
                .collect();
            object.insert("ip".into(), json!(ips));
        }
        RuleKind::Geosite => {
            let domains: Vec<String> = values
                .iter()
                .map(|code| format!("geosite:{}", code.to_ascii_lowercase()))
                .collect();
            object.insert("domain".into(), json!(domains));
        }
        RuleKind::ProcessName => {
            object.insert("process".into(), json!(values));
        }
    }
    Some(Value::Object(object))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::default_rules;

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
        let config = build_config(
            &[sample_server()],
            "srv-1",
            &Settings::default(),
            &default_rules(),
        )
        .unwrap();
        assert_eq!(config["outbounds"][0]["tag"], "proxy");
        assert_eq!(config["outbounds"][0]["protocol"], "vless");
        assert_eq!(config["inbounds"][0]["port"], 10808);
        assert_eq!(config["routing"]["rules"][0]["outboundTag"], "direct");
        let text = serde_json::to_string(&config).unwrap();
        assert!(text.contains("geosite:category-ads-all"));
    }

    #[test]
    fn tun_rejected() {
        let settings = Settings {
            tun_enabled: true,
            ..Default::default()
        };
        assert!(build_config(&[sample_server()], "srv-1", &settings, &[]).is_err());
    }

    #[test]
    fn xhttp_supported() {
        let mut server = sample_server();
        server.transport = Transport::Xhttp;
        server.path = "/x".into();
        let config = build_config(&[server], "srv-1", &Settings::default(), &[]).unwrap();
        assert_eq!(config["outbounds"][0]["streamSettings"]["network"], "xhttp");
    }

    #[test]
    fn xdrive_enables_mux() {
        let mut server = sample_server();
        server.transport = Transport::XDrive;
        server.security = Security::Tls;
        server.reality_public_key.clear();
        server.encryption = "mlkem768x25519plus.native.0rtt.abc".into();
        let config = build_config(&[server], "srv-1", &Settings::default(), &[]).unwrap();
        let outbound = &config["outbounds"][0];
        assert_eq!(outbound["streamSettings"]["network"], "xdrive");
        assert_eq!(outbound["mux"]["enabled"], true);
    }

    #[test]
    fn vless_encryption_emitted() {
        let mut server = sample_server();
        server.security = Security::Tls;
        server.reality_public_key.clear();
        server.encryption = "mlkem768x25519plus.native.0rtt.abc".into();
        let config = build_config(&[server], "srv-1", &Settings::default(), &[]).unwrap();
        assert_eq!(
            config["outbounds"][0]["settings"]["vnext"][0]["users"][0]["encryption"],
            "mlkem768x25519plus.native.0rtt.abc"
        );
    }

    #[test]
    fn sockopt_emitted() {
        let mut server = sample_server();
        server.security = Security::Tls;
        server.reality_public_key.clear();
        server.tcp_fast_open = true;
        server.bind_interface = "eth0".into();
        server.routing_mark = 123;
        let config = build_config(&[server], "srv-1", &Settings::default(), &[]).unwrap();
        let sockopt = &config["outbounds"][0]["streamSettings"]["sockopt"];
        assert_eq!(sockopt["tcpFastOpen"], true);
        assert_eq!(sockopt["interface"], "eth0");
        assert_eq!(sockopt["mark"], 123);
    }

    #[test]
    fn finalmask_fragment_and_noise() {
        let mut server = sample_server();
        server.security = Security::Tls;
        server.reality_public_key.clear();
        server.fragment_enabled = true;
        server.fragment_packets = "tlshello".into();
        server.fragment_length = "100-200".into();
        server.noise_enabled = true;
        server.noise_packet = "abcd".into();
        let config = build_config(&[server], "srv-1", &Settings::default(), &[]).unwrap();
        let finalmask = &config["outbounds"][0]["streamSettings"]["finalmask"];
        assert_eq!(finalmask["tcp"][0]["type"], "fragment");
        assert_eq!(finalmask["tcp"][0]["settings"]["length"], "100-200");
        assert_eq!(finalmask["udp"][0]["type"], "noise");
        assert_eq!(
            finalmask["udp"][0]["settings"]["noise"][0]["packet"],
            "abcd"
        );
    }

    #[test]
    fn auto_select_uses_least_load_balancer() {
        let mut second = sample_server();
        second.id = "srv-2".into();
        second.address = "second.example.com".into();
        let settings = Settings {
            auto_select: true,
            ..Default::default()
        };
        let config = build_config(&[sample_server(), second], "srv-1", &settings, &[]).unwrap();
        assert!(config.get("observatory").is_some());
        assert_eq!(
            config["routing"]["balancers"][0]["strategy"]["type"],
            "leastLoad"
        );
        assert!(config["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|outbound| outbound["tag"] == "srv-1"));
        assert!(config["routing"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .any(|rule| rule["balancerTag"] == "proxy"));
    }

    #[test]
    fn custom_finalmask_masks_passthrough() {
        let mut server = sample_server();
        server.security = Security::Tls;
        server.reality_public_key.clear();
        server.finalmask_tcp = r#"[{"type":"filter","settings":{"blockedSourceIPs":["192.0.2.0/24"]}},{"type":"header-custom","settings":{"clients":[]}}]"#.into();
        server.finalmask_udp =
            r#"[{"type":"filter","settings":{"allowedSourceIPs":["192.0.2.0/24"]}}]"#.into();
        let config = build_config(&[server], "srv-1", &Settings::default(), &[]).unwrap();
        let finalmask = &config["outbounds"][0]["streamSettings"]["finalmask"];
        assert_eq!(finalmask["tcp"].as_array().unwrap().len(), 2);
        assert_eq!(finalmask["tcp"][0]["type"], "filter");
        assert_eq!(finalmask["tcp"][1]["type"], "header-custom");
        assert_eq!(finalmask["udp"][0]["type"], "filter");
    }

    #[test]
    fn masque_outbound() {
        let mut server = sample_server();
        server.protocol = Protocol::Masque;
        server.security = Security::Tls;
        server.reality_public_key.clear();
        server.masque_user = "user".into();
        server.masque_pass = "pass".into();
        server.pinned_peer_cert_sha256 = "abcd".into();
        let config = build_config(&[server], "srv-1", &Settings::default(), &[]).unwrap();
        assert_eq!(config["outbounds"][0]["protocol"], "masque");
        assert_eq!(
            config["outbounds"][0]["streamSettings"]["network"],
            "masque"
        );
        assert_eq!(
            config["outbounds"][0]["streamSettings"]["masqueSettings"]["user"],
            "user"
        );
    }

    #[test]
    fn insecure_tls_uses_verify_peer_name() {
        let mut server = sample_server();
        server.security = Security::Tls;
        server.sni = "example.com".into();
        server.allow_insecure = true;
        let config = build_config(&[server], "srv-1", &Settings::default(), &[]).unwrap();
        let tls = &config["outbounds"][0]["streamSettings"]["tlsSettings"];
        assert_eq!(tls["verifyPeerCertByName"], "example.com");
        assert!(tls.get("allowInsecure").is_none());
    }
}
