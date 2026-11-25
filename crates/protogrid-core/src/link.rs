use std::collections::HashMap;

use serde_json::Value;
use url::Url;

use crate::model::{Protocol, Security, Server, Transport};
use crate::util::{
    b64_decode_string, decode_uri_component, parse_bool, split_host_port, split_list,
};

#[derive(Debug, Default)]
pub struct ParseOutcome {
    pub servers: Vec<Server>,
    pub errors: Vec<String>,
}

fn qmap(url: &Url) -> HashMap<String, String> {
    url.query_pairs()
        .map(|(k, v)| (k.to_ascii_lowercase(), v.into_owned()))
        .collect()
}

fn q<'a>(map: &'a HashMap<String, String>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| map.get(*key).map(String::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn q_owned(map: &HashMap<String, String>, keys: &[&str]) -> String {
    q(map, keys).unwrap_or_default().to_string()
}

fn fragment_name(url: &Url) -> String {
    url.fragment()
        .map(decode_uri_component)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn parse_security(map: &HashMap<String, String>, default: Security) -> Result<Security, String> {
    match q(map, &["security"]) {
        Some("tls") | Some("xtls") => Ok(Security::Tls),
        Some("reality") => Ok(Security::Reality),
        Some("none") | Some("") => Ok(Security::None),
        Some(other) => Err(format!("неизвестная защита: {other}")),
        None => Ok(default),
    }
}

fn parse_transport(map: &HashMap<String, String>) -> Result<Transport, String> {
    match q(map, &["type", "network"]) {
        None | Some("tcp") | Some("raw") => Ok(Transport::Tcp),
        Some("ws") | Some("websocket") => Ok(Transport::Ws),
        Some("grpc") | Some("gun") => Ok(Transport::Grpc),
        Some("http") | Some("h2") => Ok(Transport::Http),
        Some("httpupgrade") => Ok(Transport::HttpUpgrade),
        Some("xhttp") | Some("splithttp") => Ok(Transport::Xhttp),
        Some("kcp") | Some("mkcp") => Ok(Transport::Kcp),
        Some("xdrive") => Ok(Transport::XDrive),
        Some(other) => Err(format!("транспорт «{other}» не поддерживается")),
    }
}

fn apply_common_transport(server: &mut Server, map: &HashMap<String, String>) {
    server.host = q_owned(map, &["host"]);
    server.path = q_owned(map, &["path"]);
    if server.transport == Transport::Xhttp {
        server.xhttp_mode = q_owned(map, &["mode", "xhttp-mode", "xhttp_mode"]);
        if server.xhttp_mode.is_empty() {
            server.xhttp_mode = "auto".into();
        }
    }
    if server.transport == Transport::Kcp {
        server.kcp_mtu = q_owned(map, &["mtu"]).parse().unwrap_or(1350);
        server.kcp_tti = q_owned(map, &["tti"]).parse().unwrap_or(50);
        server.kcp_uplink = q_owned(map, &["uplinkcapacity", "uplink_capacity", "up"])
            .parse()
            .unwrap_or(5);
        server.kcp_downlink = q_owned(map, &["downlinkcapacity", "downlink_capacity", "down"])
            .parse()
            .unwrap_or(20);
        server.kcp_congestion = q(map, &["congestion"])
            .map(|value| parse_bool(Some(&value.to_string())))
            .unwrap_or(false);
        server.kcp_seed = q_owned(map, &["seed"]);
    }
    if server.transport == Transport::XDrive {
        server.xdrive_service = q_owned(map, &["service"]);
        if server.xdrive_service.is_empty() {
            server.xdrive_service = "local".into();
        }
        server.xdrive_remote_folder =
            q_owned(map, &["remotefolder", "remote_folder", "remote-folder"]);
        server.xdrive_secrets = q_owned(map, &["secrets", "secret"]);
        server.xdrive_s3_endpoint = q_owned(map, &["s3_endpoint", "s3-endpoint"]);
        server.xdrive_s3_region = q_owned(map, &["s3_region", "s3-region"]);
        server.xdrive_s3_bucket = q_owned(map, &["s3_bucket", "s3-bucket"]);
        server.xdrive_s3_path_style = q(map, &["s3_path_style", "s3-path-style"])
            .map(|value| parse_bool(Some(&value.to_string())))
            .unwrap_or(false);
    }
    server.service_name = q_owned(map, &["servicename", "service_name"]);
    server.alpn = q(map, &["alpn"]).map(split_list).unwrap_or_default();
    server.fingerprint = q_owned(map, &["fp", "fingerprint"]);
    server.tcp_fast_open = q(map, &["tcpfastopen", "tcp_fast_open", "tfo"])
        .map(|value| parse_bool(Some(&value.to_string())))
        .unwrap_or(false);
    server.bind_interface = q_owned(map, &["interface", "bind_interface", "bind-interface"]);
    server.routing_mark = q_owned(map, &["mark", "routing_mark", "routing-mark"])
        .parse()
        .unwrap_or(0);
    server.allow_insecure = q(map, &["allowinsecure", "insecure", "skip-cert-verify"])
        .map(|value| parse_bool(Some(&value.to_string())))
        .unwrap_or(false);
}

pub fn parse_link(input: &str) -> Result<Server, String> {
    let link = input.trim().trim_matches('"');
    let scheme = link
        .split_once("://")
        .map(|(scheme, _)| scheme.to_ascii_lowercase())
        .ok_or_else(|| "это не похоже на share-ссылку".to_string())?;

    match scheme.as_str() {
        "vless" => parse_vless(link),
        "vmess" => parse_vmess(link),
        "trojan" => parse_trojan(link),
        "ss" => parse_ss(link),
        "hysteria2" | "hy2" | "hysteria" => parse_hysteria2(link),
        "tuic" => parse_tuic(link),
        "masque" => parse_masque(link),
        "ssr" => Err("ShadowsocksR (ssr://) не поддерживается".into()),
        other => Err(format!("схема «{other}» не поддерживается")),
    }
}

fn parse_vless(link: &str) -> Result<Server, String> {
    let url = Url::parse(link).map_err(|e| format!("некорректная ссылка VLESS: {e}"))?;
    let map = qmap(&url);
    let address = url
        .host_str()
        .ok_or_else(|| "в ссылке VLESS нет адреса".to_string())?
        .to_string();
    let uuid = decode_uri_component(url.username());
    if uuid.is_empty() {
        return Err("в ссылке VLESS нет UUID".into());
    }
    let mut server = Server::new(Protocol::Vless);
    server.uuid = uuid;
    server.address = address;
    server.port = url.port().unwrap_or(443);
    server.security = parse_security(&map, Security::None)?;
    server.transport = parse_transport(&map)?;
    apply_common_transport(&mut server, &map);
    server.flow = q_owned(&map, &["flow"]);
    server.encryption = q_owned(&map, &["encryption", "enc"]);
    if server.encryption == "none" {
        server.encryption.clear();
    }
    server.sni = q_owned(&map, &["sni", "peer", "servername"]);
    server.reality_public_key = q_owned(&map, &["pbk", "publickey", "public_key"]);
    server.reality_short_id = q_owned(&map, &["sid", "shortid", "short_id"]);
    server.reality_spider_x = q_owned(&map, &["spx", "spiderx", "spider_x"]);
    server.reality_mldsa65_verify = q_owned(
        &map,
        &[
            "mldsa65verify",
            "mldsa65_verify",
            "mldsa65-verify",
            "mldsa65",
        ],
    );
    if server.security == Security::Reality && server.fingerprint.is_empty() {
        server.fingerprint = "chrome".into();
    }
    if server.security == Security::Tls && server.sni.is_empty() {
        server.sni = server.effective_sni();
    }
    server.name = fragment_name(&url);
    if server.name.is_empty() {
        server.name = server.target();
    }
    Ok(server)
}

fn json_string(value: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| value.get(*key))
        .map(|v| match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => String::new(),
        })
        .unwrap_or_default()
}

fn json_port(value: &Value) -> u16 {
    json_string(value, &["port"])
        .parse()
        .ok()
        .filter(|p: &u16| *p > 0)
        .unwrap_or(443)
}

fn parse_vmess(link: &str) -> Result<Server, String> {
    let body = link.trim_start_matches(|c: char| c != ':');
    let body = body.trim_start_matches("://");
    let (payload, external_tag) = match body.split_once('#') {
        Some((payload, tag)) => (payload, decode_uri_component(tag)),
        None => (body, String::new()),
    };

    if let Some(decoded) = b64_decode_string(payload) {
        if let Ok(value) = serde_json::from_str::<Value>(decoded.trim()) {
            return vmess_from_json(&value, &external_tag);
        }
    }

    parse_vmess_uri(link)
}

fn vmess_from_json(value: &Value, external_tag: &str) -> Result<Server, String> {
    let mut server = Server::new(Protocol::Vmess);
    server.address = json_string(value, &["add", "address"]);
    if server.address.is_empty() {
        return Err("в ссылке VMess нет адреса".into());
    }
    server.port = json_port(value);
    server.uuid = json_string(value, &["id", "uuid"]);
    if server.uuid.is_empty() {
        return Err("в ссылке VMess нет UUID".into());
    }
    server.vmess_security = {
        let scy = json_string(value, &["scy", "security"]);
        if scy.is_empty() {
            "auto".into()
        } else {
            scy
        }
    };
    server.vmess_alter_id = json_string(value, &["aid", "alterId"]).parse().unwrap_or(0);
    server.name = {
        let ps = json_string(value, &["ps", "name"]);
        if ps.is_empty() {
            external_tag.to_string()
        } else {
            ps
        }
    };

    let tls = json_string(value, &["tls"]).to_ascii_lowercase();
    server.security = match tls.as_str() {
        "tls" | "xtls" => Security::Tls,
        "reality" => Security::Reality,
        _ => Security::None,
    };
    let network = json_string(value, &["net", "network"]).to_ascii_lowercase();
    server.transport = match network.as_str() {
        "" | "tcp" | "raw" => Transport::Tcp,
        "ws" | "websocket" => Transport::Ws,
        "grpc" | "gun" => Transport::Grpc,
        "http" | "h2" => Transport::Http,
        "httpupgrade" => Transport::HttpUpgrade,
        "xhttp" | "splithttp" => Transport::Xhttp,
        "kcp" | "mkcp" => Transport::Kcp,
        "xdrive" => Transport::XDrive,
        other => return Err(format!("транспорт «{other}» не поддерживается")),
    };
    server.host = json_string(value, &["host"]);
    server.path = json_string(value, &["path"]);
    server.service_name = json_string(value, &["serviceName", "service_name"]);
    server.sni = {
        let sni = json_string(value, &["sni", "serverName"]);
        if sni.is_empty() {
            json_string(value, &["host"])
        } else {
            sni
        }
    };
    server.alpn = split_list(&json_string(value, &["alpn"]));
    server.fingerprint = json_string(value, &["fp", "fingerprint"]);
    server.allow_insecure = json_string(value, &["allowInsecure", "skip-cert-verify"])
        .parse::<u8>()
        .map(|v| v == 1)
        .unwrap_or(false);
    server.reality_public_key = json_string(value, &["pbk", "publicKey"]);
    server.reality_short_id = json_string(value, &["sid", "shortId"]);
    server.reality_spider_x = json_string(value, &["spx", "spiderX"]);
    if server.name.is_empty() {
        server.name = server.target();
    }
    Ok(server)
}

fn parse_vmess_uri(link: &str) -> Result<Server, String> {
    let url = Url::parse(link).map_err(|e| format!("некорректная ссылка VMess: {e}"))?;
    let map = qmap(&url);
    let mut server = Server::new(Protocol::Vmess);
    server.uuid = decode_uri_component(url.username());
    server.address = url
        .host_str()
        .ok_or_else(|| "в ссылке VMess нет адреса".to_string())?
        .to_string();
    server.port = url.port().unwrap_or(443);
    server.security = parse_security(&map, Security::None)?;
    server.transport = parse_transport(&map)?;
    apply_common_transport(&mut server, &map);
    server.sni = q_owned(&map, &["sni", "peer", "servername"]);
    server.name = fragment_name(&url);
    if server.name.is_empty() {
        server.name = server.target();
    }
    if server.uuid.is_empty() {
        return Err("в ссылке VMess нет UUID".into());
    }
    Ok(server)
}

fn parse_trojan(link: &str) -> Result<Server, String> {
    let url = Url::parse(link).map_err(|e| format!("некорректная ссылка Trojan: {e}"))?;
    let map = qmap(&url);
    let mut server = Server::new(Protocol::Trojan);
    server.password = decode_uri_component(url.username());
    if server.password.is_empty() {
        return Err("в ссылке Trojan нет пароля".into());
    }
    server.address = url
        .host_str()
        .ok_or_else(|| "в ссылке Trojan нет адреса".to_string())?
        .to_string();
    server.port = url.port().unwrap_or(443);
    server.security = parse_security(&map, Security::Tls)?;
    server.transport = parse_transport(&map)?;
    apply_common_transport(&mut server, &map);
    server.sni = q_owned(&map, &["sni", "peer", "servername"]);
    server.flow = q_owned(&map, &["flow"]);
    server.name = fragment_name(&url);
    if server.name.is_empty() {
        server.name = server.target();
    }
    Ok(server)
}

fn parse_ss(link: &str) -> Result<Server, String> {
    let prefix = link.get(..5).unwrap_or_default();
    if !prefix.eq_ignore_ascii_case("ss://") {
        return Err("некорректная ссылка Shadowsocks".into());
    }
    let body = &link[5..];
    let (body, raw_tag) = match body.split_once('#') {
        Some((body, tag)) => (body, decode_uri_component(tag)),
        None => (body, String::new()),
    };
    let (main, query) = match body.split_once('?') {
        Some((main, query)) => (main, Some(query)),
        None => (body, None),
    };

    let (method, password, hostport) = if let Some((userinfo, hostport)) = main.rsplit_once('@') {
        let decoded = if userinfo.contains(':') {
            decode_uri_component(userinfo)
        } else {
            b64_decode_string(userinfo)
                .ok_or_else(|| "не удалось декодировать данные Shadowsocks".to_string())?
        };
        let (method, password) = decoded
            .split_once(':')
            .ok_or_else(|| "в ссылке Shadowsocks нет метода/пароля".to_string())?;
        (
            method.to_string(),
            password.to_string(),
            hostport.to_string(),
        )
    } else {
        let decoded = b64_decode_string(main)
            .ok_or_else(|| "не удалось декодировать ссылку Shadowsocks".to_string())?;
        let (userinfo, hostport) = decoded
            .rsplit_once('@')
            .ok_or_else(|| "в ссылке Shadowsocks нет адреса".to_string())?;
        let (method, password) = userinfo
            .split_once(':')
            .ok_or_else(|| "в ссылке Shadowsocks нет метода/пароля".to_string())?;
        (
            method.to_string(),
            password.to_string(),
            hostport.to_string(),
        )
    };

    let (address, port) = split_host_port(&hostport)
        .ok_or_else(|| "в ссылке Shadowsocks некорректный адрес".to_string())?;

    let mut server = Server::new(Protocol::Shadowsocks);
    server.address = address;
    server.port = port;
    server.method = method.to_ascii_lowercase();
    server.password = password;
    server.name = raw_tag;

    if let Some(query) = query {
        let params: HashMap<String, String> = url::form_urlencoded::parse(query.as_bytes())
            .map(|(k, v)| (k.to_ascii_lowercase(), v.into_owned()))
            .collect();
        if let Some(plugin) = params.get("plugin") {
            let mut parts = plugin.split(';');
            let raw_name = parts.next().unwrap_or("").trim();
            let opts: Vec<&str> = parts.collect();
            server.ss_plugin = match raw_name {
                "simple-obfs" | "obfs-local" => "obfs-local".into(),
                other => other.to_string(),
            };
            server.ss_plugin_opts = opts.join(";");
        }
    }

    if server.name.is_empty() {
        server.name = server.target();
    }
    Ok(server)
}

fn parse_hysteria2(link: &str) -> Result<Server, String> {
    let url = Url::parse(link).map_err(|e| format!("некорректная ссылка Hysteria2: {e}"))?;
    let map = qmap(&url);
    let mut server = Server::new(Protocol::Hysteria2);
    let username = decode_uri_component(url.username());
    let url_password = url.password().map(decode_uri_component).unwrap_or_default();
    server.password = q_owned(&map, &["auth", "password"]);
    if server.password.is_empty() {
        server.password = if !url_password.is_empty() {
            if username.is_empty() {
                url_password
            } else {
                format!("{username}:{url_password}")
            }
        } else {
            username
        };
    }
    if server.password.is_empty() {
        return Err("в ссылке Hysteria2 нет пароля".into());
    }
    server.address = url
        .host_str()
        .ok_or_else(|| "в ссылке Hysteria2 нет адреса".to_string())?
        .to_string();
    server.port = url.port().unwrap_or(443);
    server.security = Security::Tls;
    server.sni = q_owned(&map, &["sni", "peer"]);
    server.alpn = q(&map, &["alpn"]).map(split_list).unwrap_or_default();
    server.allow_insecure = q(&map, &["insecure", "allowinsecure", "skip-cert-verify"])
        .map(|v| parse_bool(Some(&v.to_string())))
        .unwrap_or(false);
    server.obfs = q_owned(&map, &["obfs"]);
    server.obfs_password = q_owned(&map, &["obfs-password", "obfs_password", "obfspassword"]);
    server.up_mbps = q_owned(&map, &["up", "upmbps", "up_mbps"])
        .trim_end_matches(|c: char| !c.is_ascii_digit())
        .parse()
        .unwrap_or(0);
    server.down_mbps = q_owned(&map, &["down", "downmbps", "down_mbps"])
        .trim_end_matches(|c: char| !c.is_ascii_digit())
        .parse()
        .unwrap_or(0);
    server.name = fragment_name(&url);
    if server.name.is_empty() {
        server.name = server.target();
    }
    Ok(server)
}

fn parse_masque(link: &str) -> Result<Server, String> {
    let url = Url::parse(link).map_err(|e| format!("некорректная ссылка MASQUE: {e}"))?;
    let map = qmap(&url);
    let mut server = Server::new(Protocol::Masque);
    server.security = Security::Tls;
    server.address = url
        .host_str()
        .ok_or_else(|| "в ссылке MASQUE нет адреса".to_string())?
        .to_string();
    server.port = url.port().unwrap_or(443);
    server.masque_user = decode_uri_component(url.username());
    server.masque_pass = url.password().map(decode_uri_component).unwrap_or_default();
    server.sni = q_owned(&map, &["sni", "servername"]);
    server.host = q_owned(&map, &["host"]);
    server.path = q_owned(&map, &["path"]);
    server.pinned_peer_cert_sha256 = q_owned(&map, &["pinned_peer_cert_sha256", "pin", "pcs"]);
    server.masque_remote_dns = q_owned(&map, &["remote_dns", "remotedns"]);
    server.name = fragment_name(&url);
    if server.name.is_empty() {
        server.name = server.target();
    }
    Ok(server)
}

fn parse_tuic(link: &str) -> Result<Server, String> {
    let url = Url::parse(link).map_err(|e| format!("некорректная ссылка TUIC: {e}"))?;
    let map = qmap(&url);
    let mut server = Server::new(Protocol::Tuic);
    server.uuid = decode_uri_component(url.username());
    server.password = url.password().map(decode_uri_component).unwrap_or_default();
    if server.uuid.is_empty() {
        server.uuid = q_owned(&map, &["uuid"]);
    }
    if server.password.is_empty() {
        server.password = q_owned(&map, &["password"]);
    }
    if server.uuid.is_empty() || server.password.is_empty() {
        return Err("в ссылке TUIC нет UUID или пароля".into());
    }
    server.address = url
        .host_str()
        .ok_or_else(|| "в ссылке TUIC нет адреса".to_string())?
        .to_string();
    server.port = url.port().unwrap_or(443);
    server.security = Security::Tls;
    server.sni = q_owned(&map, &["sni", "peer"]);
    server.alpn = q(&map, &["alpn"]).map(split_list).unwrap_or_default();
    server.allow_insecure = q(&map, &["allow_insecure", "insecure", "skip-cert-verify"])
        .map(|v| parse_bool(Some(&v.to_string())))
        .unwrap_or(false);
    server.congestion_control =
        q_owned(&map, &["congestion_control", "congestion-controller", "cc"]);
    if server.congestion_control.is_empty() {
        server.congestion_control = "bbr".into();
    }
    server.udp_relay_mode = q_owned(&map, &["udp_relay_mode", "udp-relay-mode"]);
    if server.udp_relay_mode.is_empty() {
        server.udp_relay_mode = "native".into();
    }
    if q(&map, &["disable_sni"]).map(|v| parse_bool(Some(&v.to_string()))) == Some(true) {
        server.sni.clear();
    }
    server.name = fragment_name(&url);
    if server.name.is_empty() {
        server.name = server.target();
    }
    Ok(server)
}

/// Parse a text blob that contains one or more share links.
pub fn parse_links(text: &str) -> ParseOutcome {
    let mut outcome = ParseOutcome::default();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        outcome.errors.push("пустой ввод".into());
        return outcome;
    }

    if trimmed.contains("proxies:") {
        match crate::clash::parse_clash_yaml(trimmed) {
            Ok(mut servers) => {
                for (index, server) in servers.iter_mut().enumerate() {
                    if server.name.trim().is_empty() {
                        server.name = format!("Сервер {}", index + 1);
                    }
                }
                outcome.servers = servers;
            }
            Err(error) => outcome.errors.push(error),
        }
        return outcome;
    }

    let mut source = trimmed.to_string();
    if !trimmed.contains("://") {
        if let Some(decoded) = b64_decode_string(trimmed) {
            if decoded.contains("://") || decoded.contains("proxies:") {
                source = decoded;
            }
        }
    }
    if source.contains("proxies:") {
        return match crate::clash::parse_clash_yaml(&source) {
            Ok(servers) => ParseOutcome {
                servers,
                errors: Vec::new(),
            },
            Err(error) => ParseOutcome {
                servers: Vec::new(),
                errors: vec![error],
            },
        };
    }

    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        match parse_link(line) {
            Ok(server) => outcome.servers.push(server),
            Err(error) => {
                let preview: String = line.chars().take(80).collect();
                outcome.errors.push(format!("{preview} — {error}"));
            }
        }
    }

    if outcome.servers.is_empty() && outcome.errors.is_empty() {
        outcome
            .errors
            .push("не найдено ни одной поддерживаемой ссылки".into());
    }
    outcome
}

pub fn display_error_list(errors: &[String]) -> String {
    const LIMIT: usize = 5;
    let mut text = errors
        .iter()
        .take(LIMIT)
        .cloned()
        .collect::<Vec<_>>()
        .join("; ");
    if errors.len() > LIMIT {
        text.push_str(&format!(" и ещё {} ошибок", errors.len() - LIMIT));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RuleAction;

    #[test]
    fn vless_reality() {
        let link = "vless://11111111-1111-1111-1111-111111111111@example.com:443?security=reality&sni=cdn.example.com&fp=chrome&pbk=jNXHt1yRo0vDuchQlIP6Z0ZvjT3KtzVI-T4E7RoLJS0&sid=0123&type=grpc&serviceName=svc#%D0%A2%D0%B5%D1%81%D1%82";
        let server = parse_link(link).unwrap();
        assert_eq!(server.protocol, Protocol::Vless);
        assert_eq!(server.address, "example.com");
        assert_eq!(server.port, 443);
        assert_eq!(server.security, Security::Reality);
        assert_eq!(server.transport, Transport::Grpc);
        assert_eq!(server.service_name, "svc");
        assert_eq!(server.name, "Тест");
        assert_eq!(server.reality_short_id, "0123");
    }

    #[test]
    fn vmess_json() {
        let payload = serde_json::json!({
            "v": "2", "ps": "Тест VMess", "add": "example.org", "port": "8443",
            "id": "22222222-2222-2222-2222-222222222222", "aid": "0", "scy": "auto",
            "net": "ws", "host": "example.org", "path": "/ws", "tls": "tls", "sni": "example.org"
        });
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(payload.to_string());
        let server = parse_link(&format!("vmess://{encoded}")).unwrap();
        assert_eq!(server.protocol, Protocol::Vmess);
        assert_eq!(server.port, 8443);
        assert_eq!(server.transport, Transport::Ws);
        assert_eq!(server.security, Security::Tls);
        assert_eq!(server.path, "/ws");
    }

    #[test]
    fn shadowsocks_sip002() {
        use base64::Engine;
        let userinfo =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("aes-256-gcm:pass word");
        let link = format!("ss://{userinfo}@example.net:8388#Node");
        let server = parse_link(&link).unwrap();
        assert_eq!(server.protocol, Protocol::Shadowsocks);
        assert_eq!(server.method, "aes-256-gcm");
        assert_eq!(server.password, "pass word");
        assert_eq!(server.address, "example.net");
    }

    #[test]
    fn shadowsocks_legacy() {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD
            .encode("chacha20-ietf-poly1305:secret@example.net:8388");
        let server = parse_link(&format!("ss://{encoded}#Legacy")).unwrap();
        assert_eq!(server.password, "secret");
        assert_eq!(server.port, 8388);
        assert_eq!(server.name, "Legacy");
    }

    #[test]
    fn hysteria2_link() {
        let server = parse_link(
            "hysteria2://secret@example.com:8443?sni=example.com&insecure=1&obfs=salamander&obfs-password=x#HY",
        )
        .unwrap();
        assert_eq!(server.protocol, Protocol::Hysteria2);
        assert_eq!(server.password, "secret");
        assert_eq!(server.obfs, "salamander");
        assert!(server.allow_insecure);
    }

    #[test]
    fn tuic_link() {
        let server = parse_link(
            "tuic://11111111-1111-1111-1111-111111111111:pass@example.com:443?congestion_control=bbr&udp_relay_mode=native&alpn=h3#TUIC",
        )
        .unwrap();
        assert_eq!(server.protocol, Protocol::Tuic);
        assert_eq!(server.uuid, "11111111-1111-1111-1111-111111111111");
        assert_eq!(server.password, "pass");
        assert_eq!(server.alpn, vec!["h3".to_string()]);
    }

    #[test]
    fn trojan_link() {
        let server =
            parse_link("trojan://p%40ss@example.com:443?sni=example.com&type=ws&path=%2Fws#T")
                .unwrap();
        assert_eq!(server.protocol, Protocol::Trojan);
        assert_eq!(server.password, "p@ss");
        assert_eq!(server.transport, Transport::Ws);
        assert_eq!(server.path, "/ws");
        assert_eq!(server.security, Security::Tls);
    }

    #[test]
    fn vless_xhttp_mode() {
        let server = parse_link(
            "vless://11111111-1111-1111-1111-111111111111@example.com:443?type=xhttp&mode=packet-up&path=%2Fx#X",
        )
        .unwrap();
        assert_eq!(server.transport, Transport::Xhttp);
        assert_eq!(server.effective_xhttp_mode(), "packet-up");
    }

    #[test]
    fn vless_kcp_link() {
        let server = parse_link(
            "vless://11111111-1111-1111-1111-111111111111@example.com:443?type=kcp&mtu=1400&tti=40#K",
        )
        .unwrap();
        assert_eq!(server.transport, Transport::Kcp);
        assert_eq!(server.kcp_mtu, 1400);
        assert_eq!(server.kcp_tti, 40);
    }

    #[test]
    fn vless_encryption_link() {
        let server = parse_link(
            "vless://11111111-1111-1111-1111-111111111111@example.com:443?encryption=mlkem768x25519plus.native.0rtt.abc#E",
        )
        .unwrap();
        assert_eq!(server.encryption, "mlkem768x25519plus.native.0rtt.abc");
    }

    #[test]
    fn vless_xdrive_link() {
        let server = parse_link(
            "vless://11111111-1111-1111-1111-111111111111@example.com:443?type=xdrive&service=local&remote_folder=%2Ftmp%2Fx#X",
        )
        .unwrap();
        assert_eq!(server.transport, Transport::XDrive);
        assert_eq!(server.xdrive_service, "local");
        assert_eq!(server.xdrive_remote_folder, "/tmp/x");
    }

    #[test]
    fn masque_link() {
        let server = parse_link(
            "masque://user:pass@example.com:443?host=cdn.example.com&path=%2Fmasque&pinned_peer_cert_sha256=abcd#M",
        )
        .unwrap();
        assert_eq!(server.protocol, Protocol::Masque);
        assert_eq!(server.masque_user, "user");
        assert_eq!(server.masque_pass, "pass");
        assert_eq!(server.host, "cdn.example.com");
        assert_eq!(server.path, "/masque");
        assert_eq!(server.pinned_peer_cert_sha256, "abcd");
    }

    #[test]
    fn parse_multiple_with_errors() {
        let outcome = parse_links("vless://id@example.com:443#A\nssr://bad");
        assert_eq!(outcome.servers.len(), 1);
        assert_eq!(outcome.errors.len(), 1);
    }

    #[test]
    fn default_rule_actions_are_sane() {
        assert_eq!(RuleAction::Proxy.label(), "Через прокси");
    }
}
