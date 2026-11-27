//! OpenVPN `.ovpn` profile support for the sing-box `openvpn-client` endpoint.
//!
//! sing-box 1.14 ships an OpenVPN client as an *endpoint*, so ProtoGrid keeps
//! the profile as normalized text on [`Server::openvpn_profile`] and renders it
//! into the top-level `endpoints` array at config-build time. External files
//! (`ca`, `cert`, `key`, `tls-auth`, ...) are inlined on import so the stored
//! profile no longer depends on the filesystem.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::model::{Protocol, Security, Server};

const DEFAULT_PORT: u16 = 1194;

/// Heuristic: does this text look like a raw OpenVPN profile?
pub fn looks_like_profile(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let mut score = 0usize;
    for line in trimmed.lines().take(400) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if inline_tag_open(line).is_some() {
            score += 1;
        } else {
            let directive = line
                .split(|c: char| c.is_whitespace())
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            if matches!(
                directive.as_str(),
                "client"
                    | "pull"
                    | "dev"
                    | "proto"
                    | "remote"
                    | "remote-random"
                    | "nobind"
                    | "resolv-retry"
                    | "persist-key"
                    | "persist-tun"
                    | "verb"
                    | "auth-user-pass"
                    | "auth-retry"
                    | "ca"
                    | "cert"
                    | "key"
                    | "tls-auth"
                    | "tls-crypt"
                    | "tls-crypt-v2"
                    | "key-direction"
                    | "remote-cert-tls"
                    | "secret"
                    | "cipher"
                    | "data-ciphers"
                    | "auth"
                    | "comp-lzo"
                    | "route"
                    | "redirect-gateway"
                    | "tun-mtu"
            ) {
                score += 1;
            }
        }
        if score >= 2 {
            return true;
        }
    }
    false
}

/// Parse a profile and build a ready-to-store [`Server`].
///
/// `base_dir` is the directory the profile was loaded from: referenced files
/// are read and inlined. Without it a profile that points at external files is
/// rejected with a hint to import the `.ovpn` file through the app.
pub fn import_profile(text: &str, base_dir: Option<&Path>) -> Result<Server, String> {
    if text.trim().is_empty() {
        return Err("пустой профиль OpenVPN".into());
    }
    let profile = parse_profile(text, base_dir, true)?;
    let (host, port) = profile.first_remote().ok_or_else(|| {
        "в профиле OpenVPN нет директивы remote — укажите адрес сервера вручную".to_string()
    })?;

    let mut server = Server::new(Protocol::OpenVpn);
    server.address = host;
    server.port = port;
    server.name = format!("OpenVPN {}", server.address);
    server.security = if profile.static_key_mode {
        Security::None
    } else {
        Security::Tls
    };
    server.openvpn_username = profile.username.trim().to_string();
    server.openvpn_password = profile.password.trim().to_string();
    server.openvpn_profile = profile.normalized;
    Ok(server)
}

/// Build the `openvpn-client` endpoint JSON for a server.
pub fn endpoint(server: &Server) -> Result<Value, String> {
    let profile = parse_profile(&server.openvpn_profile, None, false)?;
    let static_key = profile.static_key_mode;

    let mut remotes = profile.remotes.clone();
    if remotes.is_empty() {
        if server.address.trim().is_empty() {
            return Err("в профиле OpenVPN нет директивы remote".into());
        }
        remotes.push(Remote {
            host: server.address.trim().to_string(),
            port: None,
            network: None,
        });
    }
    // Manual edits of the server address/port win over the first `remote`.
    if !server.address.trim().is_empty() {
        remotes[0].host = server.address.trim().to_string();
    }
    if server.port != 0 {
        remotes[0].port = Some(server.port);
    }

    let mut object = Map::new();
    object.insert("type".into(), json!("openvpn-client"));
    object.insert("tag".into(), json!(server.id));
    object.insert("system".into(), json!(false));
    if static_key {
        object.insert("mode".into(), json!("static_key"));
    }

    if remotes.len() == 1 {
        let remote = &remotes[0];
        object.insert("server".into(), json!(remote.host));
        object.insert(
            "server_port".into(),
            json!(remote.port.or(profile.default_port).unwrap_or(DEFAULT_PORT)),
        );
        if let Some(network) = remote.network.clone().or_else(|| profile.network.clone()) {
            object.insert("network".into(), json!(network));
        }
    } else {
        let mut servers = Vec::with_capacity(remotes.len());
        for remote in &remotes {
            let mut item = Map::new();
            item.insert("server".into(), json!(remote.host));
            item.insert(
                "server_port".into(),
                json!(remote.port.or(profile.default_port).unwrap_or(DEFAULT_PORT)),
            );
            if let Some(network) = remote.network.clone().or_else(|| profile.network.clone()) {
                item.insert("network".into(), json!(network));
            }
            servers.push(Value::Object(item));
        }
        object.insert("servers".into(), Value::Array(servers));
    }

    insert_bool(&mut object, "remote_random", profile.remote_random);

    if !static_key {
        let username = if server.openvpn_username.trim().is_empty() {
            profile.username.clone()
        } else {
            server.openvpn_username.trim().to_string()
        };
        insert_str(&mut object, "username", &username);
        let password = if server.openvpn_password.is_empty() {
            profile.password.clone()
        } else {
            server.openvpn_password.clone()
        };
        insert_str(&mut object, "password", &password);
        insert_str(&mut object, "auth_retry", &profile.auth_retry);
        insert_str(&mut object, "static_challenge", &profile.static_challenge);
        insert_bool(
            &mut object,
            "static_challenge_echo",
            profile.static_challenge_echo,
        );
    }

    if static_key {
        insert_str(&mut object, "key_direction", &profile.key_direction);
        insert_str(&mut object, "static_key", &profile.static_key);
    }

    if !profile.address.is_empty() {
        object.insert("address".into(), json!(profile.address));
    }
    insert_str(&mut object, "peer_address", &profile.peer_address);
    insert_str(&mut object, "peer_address_ipv6", &profile.peer_address_ipv6);
    insert_str(&mut object, "topology", &profile.topology);

    if static_key {
        insert_str(&mut object, "cipher", &profile.cipher);
    } else {
        // sing-box rejects `cipher` in TLS mode; an old OpenVPN `cipher` line is
        // exactly the "peers without cipher negotiation" fallback.
        if profile.data_ciphers_fallback.is_empty() {
            insert_str(&mut object, "data_ciphers_fallback", &profile.cipher);
        } else {
            insert_str(
                &mut object,
                "data_ciphers_fallback",
                &profile.data_ciphers_fallback,
            );
        }
        if !profile.data_ciphers.is_empty() {
            object.insert("data_ciphers".into(), json!(profile.data_ciphers));
        }
    }
    insert_str(&mut object, "auth", &profile.auth);
    if let Some(value) = profile.mss_fix.filter(|value| *value > 0) {
        object.insert("mss_fix".into(), json!(value));
    }
    insert_bool(&mut object, "mss_fix_disabled", profile.mss_fix_disabled);
    insert_str(&mut object, "mss_fix_mode", &profile.mss_fix_mode);
    if let Some(value) = profile.fragment.filter(|value| *value >= 68) {
        object.insert("fragment".into(), json!(value));
    }
    if let Some(value) = profile.replay_window.filter(|value| *value > 0) {
        object.insert("replay_window".into(), json!(value));
    }
    insert_duration(
        &mut object,
        "replay_window_time",
        profile.replay_window_time,
    );
    insert_str(&mut object, "compression", &profile.compression);
    insert_str(&mut object, "compression_lzo", &profile.compression_lzo);
    insert_str(&mut object, "allow_compression", &profile.allow_compression);

    if !static_key {
        insert_bool(&mut object, "route_no_pull", profile.route_no_pull);
        if !profile.pull_filters.is_empty() {
            let filters: Vec<Value> = profile
                .pull_filters
                .iter()
                .map(|(action, text)| json!({"action": action, "text": text}))
                .collect();
            object.insert("pull_filters".into(), Value::Array(filters));
        }
    }

    if !profile.routes.is_empty() {
        object.insert("routes".into(), json!(profile.routes));
    }
    insert_str(&mut object, "route_gateway", &profile.route_gateway);
    if let Some(metric) = profile.route_metric.filter(|value| *value != 0) {
        object.insert("route_metric".into(), json!(metric));
    }
    insert_bool(&mut object, "redirect_gateway", profile.redirect_gateway);
    if !profile.redirect_gateway_flags.is_empty() {
        object.insert(
            "redirect_gateway_flags".into(),
            json!(profile.redirect_gateway_flags),
        );
    }
    insert_bool(&mut object, "redirect_private", profile.redirect_private);
    insert_bool(&mut object, "block_ipv6", profile.block_ipv6);
    insert_duration(&mut object, "ping_interval", profile.ping_interval);
    insert_duration(&mut object, "ping_restart", profile.ping_restart);

    if !static_key {
        insert_duration(
            &mut object,
            "renegotiate_interval",
            profile.renegotiate_interval,
        );
        insert_bool(
            &mut object,
            "renegotiate_disabled",
            profile.renegotiate_disabled,
        );
        insert_duration(&mut object, "tls_timeout", profile.tls_timeout);
        insert_duration(&mut object, "handshake_window", profile.handshake_window);
        let tls = tls_value(&profile.tls).unwrap_or_else(|| Value::Object(Map::new()));
        object.insert("tls".into(), tls);
    }

    if let Some(value) = profile.explicit_exit_notify.filter(|value| *value > 0) {
        object.insert("explicit_exit_notify".into(), json!(value));
    }
    if let Some(value) = profile.mtu.filter(|value| *value > 0) {
        object.insert("mtu".into(), json!(value));
    }

    insert_bool(&mut object, "tcp_fast_open", server.tcp_fast_open);
    insert_str(&mut object, "bind_interface", &server.bind_interface);
    if server.routing_mark > 0 {
        object.insert("routing_mark".into(), json!(server.routing_mark));
    }

    Ok(Value::Object(object))
}

fn tls_value(tls: &TlsProfile) -> Option<Value> {
    let mut object = Map::new();
    insert_str(&mut object, "certificate", &tls.certificate);
    insert_str(&mut object, "client_certificate", &tls.client_certificate);
    insert_str(&mut object, "client_key", &tls.client_key);
    insert_str(&mut object, "server_name", &tls.server_name);
    if !tls.server_name.is_empty() {
        let kind = if tls.server_name_type.is_empty() {
            "name"
        } else {
            tls.server_name_type.as_str()
        };
        insert_str(&mut object, "server_name_type", kind);
    }
    if tls.remote_certificate_eku.is_empty() {
        insert_str(
            &mut object,
            "remote_certificate_tls",
            &tls.remote_certificate_tls,
        );
    } else {
        // sing-box treats EKU and the purpose check as mutually exclusive.
        insert_str(
            &mut object,
            "remote_certificate_eku",
            &tls.remote_certificate_eku,
        );
    }
    if !tls.remote_certificate_ku.is_empty() {
        object.insert(
            "remote_certificate_ku".into(),
            json!(tls.remote_certificate_ku),
        );
    }
    insert_str(&mut object, "ns_certificate_type", &tls.ns_certificate_type);
    insert_str(&mut object, "version_min", &tls.version_min);
    insert_str(&mut object, "version_max", &tls.version_max);
    insert_str(&mut object, "cipher", &tls.cipher);
    insert_str(&mut object, "groups", &tls.groups);
    if !tls.control_wrap_type.is_empty() {
        let mut wrap = Map::new();
        wrap.insert("type".into(), json!(tls.control_wrap_type));
        insert_str(&mut wrap, "key", &tls.control_wrap_key);
        if tls.control_wrap_type == "tls_auth" {
            insert_str(&mut wrap, "direction", &tls.control_wrap_direction);
        }
        object.insert("control_wrap".into(), Value::Object(wrap));
    }
    if object.is_empty() {
        None
    } else {
        Some(Value::Object(object))
    }
}

fn insert_str(object: &mut Map<String, Value>, key: &str, value: &str) {
    let value = value.trim();
    if !value.is_empty() {
        object.insert(key.to_string(), json!(value));
    }
}

fn insert_bool(object: &mut Map<String, Value>, key: &str, value: bool) {
    if value {
        object.insert(key.to_string(), json!(true));
    }
}

fn insert_duration(object: &mut Map<String, Value>, key: &str, seconds: Option<u32>) {
    if let Some(seconds) = seconds.filter(|value| *value > 0) {
        object.insert(key.to_string(), json!(format!("{seconds}s")));
    }
}

#[derive(Debug, Clone, Default)]
struct Remote {
    host: String,
    port: Option<u16>,
    network: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct TlsProfile {
    certificate: String,
    client_certificate: String,
    client_key: String,
    server_name: String,
    server_name_type: String,
    remote_certificate_tls: String,
    remote_certificate_ku: Vec<String>,
    remote_certificate_eku: String,
    ns_certificate_type: String,
    version_min: String,
    version_max: String,
    cipher: String,
    groups: String,
    control_wrap_type: String,
    control_wrap_key: String,
    control_wrap_direction: String,
}

#[derive(Debug, Clone, Default)]
struct Profile {
    static_key_mode: bool,
    network: Option<String>,
    remotes: Vec<Remote>,
    default_port: Option<u16>,
    remote_random: bool,
    address: Vec<String>,
    peer_address: String,
    peer_address_ipv6: String,
    topology: String,
    username: String,
    password: String,
    auth_retry: String,
    static_challenge: String,
    static_challenge_echo: bool,
    key_direction: String,
    tls: TlsProfile,
    cipher: String,
    data_ciphers: Vec<String>,
    data_ciphers_fallback: String,
    auth: String,
    mss_fix: Option<u32>,
    mss_fix_disabled: bool,
    mss_fix_mode: String,
    fragment: Option<u32>,
    replay_window: Option<u32>,
    replay_window_time: Option<u32>,
    compression: String,
    compression_lzo: String,
    allow_compression: String,
    route_no_pull: bool,
    pull_filters: Vec<(String, String)>,
    routes: Vec<String>,
    route_gateway: String,
    route_metric: Option<i64>,
    redirect_gateway: bool,
    redirect_gateway_flags: Vec<String>,
    redirect_private: bool,
    block_ipv6: bool,
    ping_interval: Option<u32>,
    ping_restart: Option<u32>,
    renegotiate_interval: Option<u32>,
    renegotiate_disabled: bool,
    tls_timeout: Option<u32>,
    handshake_window: Option<u32>,
    explicit_exit_notify: Option<u32>,
    mtu: Option<u32>,
    static_key: String,
    normalized: String,
}

impl Profile {
    fn first_remote(&self) -> Option<(String, u16)> {
        let remote = self.remotes.first()?;
        Some((
            remote.host.clone(),
            remote.port.or(self.default_port).unwrap_or(DEFAULT_PORT),
        ))
    }
}

fn parse_profile(text: &str, base_dir: Option<&Path>, strict: bool) -> Result<Profile, String> {
    let mut profile = Profile::default();
    let mut output: Vec<String> = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut index = 0;

    while index < lines.len() {
        let raw = lines[index];
        index += 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if let Some(tag) = inline_tag_open(line) {
            let closing = format!("</{tag}>");
            let mut content = String::new();
            let mut closed = false;
            while index < lines.len() {
                let inner = lines[index];
                index += 1;
                if inner.trim().eq_ignore_ascii_case(&closing) {
                    closed = true;
                    break;
                }
                content.push_str(inner);
                content.push('\n');
            }
            if !closed {
                return Err(format!("незакрытый inline-блок <{tag}> в профиле OpenVPN"));
            }
            let content = content.trim_end_matches('\n').to_string();
            apply_inline(&mut profile, &tag, &content);
            output.push(format!("<{tag}>\n{content}\n</{tag}>"));
            continue;
        }

        let tokens = tokenize(line);
        if tokens.is_empty() {
            continue;
        }
        let directive = tokens[0].to_ascii_lowercase();
        let args = &tokens[1..];
        let mut keep = true;

        match directive.as_str() {
            "client" | "pull" => {}
            "dev" => {
                let dev = first_arg(args).unwrap_or("").to_ascii_lowercase();
                if dev.starts_with("tap") {
                    return Err("TAP/L2 (dev tap) не поддерживается: нужен tun-режим".into());
                }
            }
            "proto" => {
                if let Some(value) = first_arg(args) {
                    let lower = value.to_ascii_lowercase();
                    if lower == "tcp-server" {
                        return Err(
                            "proto tcp-server не поддерживается клиентским профилем OpenVPN".into(),
                        );
                    }
                    if let Some(network) = network_of(&lower) {
                        profile.network = Some(network);
                    }
                }
            }
            "remote" => {
                if let Some(host) = first_arg(args) {
                    let mut remote = Remote {
                        host: host.to_string(),
                        port: None,
                        network: None,
                    };
                    for value in args.iter().skip(1) {
                        if remote.port.is_none() {
                            if let Ok(port) = value.parse::<u16>() {
                                if port > 0 {
                                    remote.port = Some(port);
                                    continue;
                                }
                            }
                        }
                        if let Some(network) = network_of(value) {
                            remote.network = Some(network);
                        }
                    }
                    profile.remotes.push(remote);
                }
            }
            "remote-random" => profile.remote_random = true,
            "port" => {
                if let Some(port) = first_arg(args).and_then(|value| value.parse::<u16>().ok()) {
                    if port > 0 {
                        profile.default_port = Some(port);
                    }
                }
            }
            "nobind" => {}
            "ca" | "cert" | "key" => {
                if let Some(content) = read_inline(&directive, first_arg(args), base_dir, strict)? {
                    apply_inline(&mut profile, &directive, &content);
                    output.push(format!("<{directive}>\n{content}\n</{directive}>"));
                    keep = false;
                }
            }
            "tls-auth" | "tls-crypt" | "tls-crypt-v2" => {
                if let Some(content) = read_inline(&directive, first_arg(args), base_dir, strict)? {
                    apply_inline(&mut profile, &directive, &content);
                    output.push(format!("<{directive}>\n{content}\n</{directive}>"));
                    if directive == "tls-auth" {
                        if let Some(direction) = args.get(1).and_then(|v| direction_of(v)) {
                            // The inline block cannot carry the direction, so
                            // keep it as a directive in the normalized text.
                            output.push(format!("key-direction {direction}"));
                            profile.tls.control_wrap_direction = direction;
                        }
                    }
                    keep = false;
                }
            }
            "secret" => {
                if let Some(content) = read_inline("secret", first_arg(args), base_dir, strict)? {
                    apply_inline(&mut profile, "secret", &content);
                    output.push(format!("<secret>\n{content}\n</secret>"));
                    if let Some(direction) = args.get(1).and_then(|v| direction_of(v)) {
                        output.push(format!("key-direction {direction}"));
                        profile.key_direction = direction;
                    }
                    keep = false;
                }
            }
            "key-direction" => {
                if let Some(direction) = first_arg(args).and_then(direction_of) {
                    profile.key_direction = direction.clone();
                    profile.tls.control_wrap_direction = direction;
                }
            }
            "verify-x509-name" => {
                if let Some(name) = first_arg(args) {
                    profile.tls.server_name = name.to_string();
                    profile.tls.server_name_type = match args.get(1) {
                        Some(kind) => {
                            let kind = kind.to_ascii_lowercase();
                            if matches!(kind.as_str(), "subject" | "name" | "name-prefix") {
                                kind
                            } else {
                                "name".into()
                            }
                        }
                        None => "name".into(),
                    };
                }
            }
            "remote-cert-tls" => {
                if let Some(value) = first_arg(args) {
                    let lower = value.to_ascii_lowercase();
                    if matches!(lower.as_str(), "server" | "client") {
                        profile.tls.remote_certificate_tls = lower;
                    }
                }
            }
            "remote-cert-ku" => {
                profile.tls.remote_certificate_ku = args
                    .iter()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .collect();
            }
            "remote-cert-eku" => {
                profile.tls.remote_certificate_eku = args_joined(args, 0);
            }
            "ns-cert-type" => {
                if let Some(value) = first_arg(args) {
                    let lower = value.to_ascii_lowercase();
                    if matches!(lower.as_str(), "server" | "client") {
                        profile.tls.ns_certificate_type = lower;
                    }
                }
            }
            "tls-version-min" => {
                if let Some(value) = first_arg(args) {
                    profile.tls.version_min = value.to_string();
                }
            }
            "tls-version-max" => {
                if let Some(value) = first_arg(args) {
                    profile.tls.version_max = value.to_string();
                }
            }
            "tls-cipher" => profile.tls.cipher = args_joined(args, 0),
            "tls-groups" => profile.tls.groups = args_joined(args, 0),
            "auth-user-pass" => {
                if let Some(path) = first_arg(args) {
                    if let Some(content) = read_credentials(path, base_dir, strict)? {
                        apply_auth_user_pass(&mut profile, &content);
                    }
                }
                // The path is intentionally dropped: credentials live in the
                // server fields, so the stored profile stays self-contained.
                keep = false;
                output.push("auth-user-pass".to_string());
            }
            "auth-retry" => {
                if let Some(value) = first_arg(args) {
                    profile.auth_retry = value.to_ascii_lowercase();
                }
            }
            "static-challenge" => {
                if let Some(text) = first_arg(args) {
                    profile.static_challenge = text.to_string();
                }
                profile.static_challenge_echo = args
                    .get(1)
                    .map(|value| matches!(value.as_str(), "1" | "true" | "yes"))
                    .unwrap_or(false);
            }
            "cipher" => {
                if let Some(value) = first_arg(args) {
                    profile.cipher = value.to_string();
                }
            }
            "data-ciphers" => {
                profile.data_ciphers = args_joined(args, 0)
                    .split(':')
                    // OpenVPN allows `?` (optional) and `!` (disabled) prefixes.
                    .map(|value| value.trim().trim_start_matches(['?', '!']).trim())
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            "data-ciphers-fallback" => {
                if let Some(value) = first_arg(args) {
                    profile.data_ciphers_fallback = value.to_string();
                }
            }
            "auth" => {
                if let Some(value) = first_arg(args) {
                    profile.auth = value.to_string();
                }
            }
            "comp-lzo" => {
                // sing-openvpn treats a bare `comp-lzo` (and `adaptive`) as
                // adaptive LZO; a missing value must not mean plain "yes".
                profile.compression_lzo = first_arg(args)
                    .map(|value| value.to_ascii_lowercase())
                    .unwrap_or_else(|| "adaptive".into());
            }
            "compress" => match first_arg(args).map(|value| value.to_ascii_lowercase()) {
                // A bare `compress` is the deprecated LZO stub framing (v1).
                None => profile.compression = "stub".into(),
                Some(value) if value.is_empty() => profile.compression = "stub".into(),
                Some(value) => match value.as_str() {
                    "lzo" => profile.compression_lzo = "yes".into(),
                    "lz4" | "lz4-v2" | "stub" | "stub-v2" => profile.compression = value,
                    "no" | "none" | "off" | "disabled" | "migrate" => {
                        profile.compression.clear();
                        profile.compression_lzo.clear();
                    }
                    other => profile.compression_lzo = other.to_string(),
                },
            },
            "allow-compression" => {
                if let Some(value) = first_arg(args) {
                    profile.allow_compression = value.to_ascii_lowercase();
                }
            }
            "fragment" => {
                if let Some(value) = first_arg(args).and_then(|value| value.parse::<u32>().ok()) {
                    if value > 0 {
                        profile.fragment = Some(value);
                    }
                }
            }
            "mssfix" => {
                let mut value: Option<u32> = None;
                let mut disabled = false;
                for arg in args {
                    let lower = arg.to_ascii_lowercase();
                    if matches!(lower.as_str(), "mtu" | "fixed") {
                        profile.mss_fix_mode = lower;
                    } else if let Ok(number) = arg.parse::<u32>() {
                        if number == 0 {
                            // `mssfix 0` disables clamping.
                            disabled = true;
                        } else {
                            value = Some(number);
                        }
                    }
                }
                if disabled {
                    profile.mss_fix_disabled = true;
                    profile.mss_fix = None;
                    profile.mss_fix_mode.clear();
                } else {
                    profile.mss_fix = Some(value.unwrap_or(1492));
                }
            }
            "tun-mtu" => {
                if let Some(value) = first_arg(args).and_then(|value| value.parse::<u32>().ok()) {
                    if value > 0 {
                        profile.mtu = Some(value);
                    }
                }
            }
            "replay-window" => {
                if let Some(value) = args.first().and_then(|value| value.parse::<u32>().ok()) {
                    if value > 0 {
                        profile.replay_window = Some(value);
                    }
                }
                if let Some(value) = args.get(1).and_then(|value| value.parse::<u32>().ok()) {
                    if value > 0 {
                        profile.replay_window_time = Some(value);
                    }
                }
            }
            "ping" => {
                if let Some(value) = first_arg(args).and_then(|value| value.parse::<u32>().ok()) {
                    if value > 0 {
                        profile.ping_interval = Some(value);
                    }
                }
            }
            "ping-restart" => {
                if let Some(value) = first_arg(args).and_then(|value| value.parse::<u32>().ok()) {
                    if value > 0 {
                        profile.ping_restart = Some(value);
                    }
                }
            }
            "reneg-sec" => match first_arg(args).and_then(|value| value.parse::<u32>().ok()) {
                Some(0) => profile.renegotiate_disabled = true,
                Some(value) => profile.renegotiate_interval = Some(value),
                None => {}
            },
            "tls-timeout" => {
                if let Some(value) = first_arg(args).and_then(|value| value.parse::<u32>().ok()) {
                    if value > 0 {
                        profile.tls_timeout = Some(value);
                    }
                }
            }
            "hand-window" => {
                if let Some(value) = first_arg(args).and_then(|value| value.parse::<u32>().ok()) {
                    if value > 0 {
                        profile.handshake_window = Some(value);
                    }
                }
            }
            "explicit-exit-notify" => {
                profile.explicit_exit_notify = Some(
                    first_arg(args)
                        .and_then(|value| value.parse::<u32>().ok())
                        .filter(|value| *value > 0)
                        .unwrap_or(1),
                );
            }
            "route" | "route-ipv6" => {
                if let Some(cidr) = route_to_cidr(args) {
                    profile.routes.push(cidr);
                }
            }
            "route-gateway" => {
                if let Some(value) = first_arg(args) {
                    // Values such as `dhcp` are meaningless for sing-box.
                    if value.parse::<std::net::IpAddr>().is_ok() {
                        profile.route_gateway = value.to_string();
                    }
                }
            }
            "route-metric" => {
                if let Some(value) = first_arg(args).and_then(|value| value.parse::<i64>().ok()) {
                    profile.route_metric = Some(value);
                }
            }
            "redirect-gateway" => {
                profile.redirect_gateway = true;
                profile.redirect_gateway_flags = args
                    .iter()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .collect();
            }
            "redirect-private" => {
                profile.redirect_private = true;
                for value in args {
                    let value = value.trim();
                    if !value.is_empty() {
                        profile.redirect_gateway_flags.push(value.to_string());
                    }
                }
            }
            "block-ipv6" => profile.block_ipv6 = true,
            "route-nopull" => profile.route_no_pull = true,
            "pull-filter" => {
                if let Some(action) = first_arg(args) {
                    let action = action.to_ascii_lowercase();
                    if matches!(action.as_str(), "accept" | "ignore" | "reject") {
                        let text = args_joined(args, 1);
                        profile.pull_filters.push((action, text));
                    }
                }
            }
            "ifconfig" => {
                if let Some(local) = first_arg(args) {
                    if let Some((address, explicit_prefix)) = split_local_address(local) {
                        let peer = args.get(1).map(String::as_str).filter(|v| !v.is_empty());
                        // With `topology subnet` the second argument is a
                        // netmask, not a peer address. Detecting the mask is
                        // order-independent: `topology` may follow `ifconfig`.
                        let mask_prefix = if address.is_ipv4() {
                            peer.and_then(mask_to_prefix)
                        } else {
                            peer.and_then(ipv6_mask_prefix)
                        };
                        let prefix = explicit_prefix
                            .or(mask_prefix)
                            .unwrap_or(if address.is_ipv4() { 32 } else { 128 });
                        profile.address.push(format!("{address}/{prefix}"));
                        if mask_prefix.is_none() {
                            if let Some(peer) = peer {
                                if peer.parse::<std::net::IpAddr>().is_ok() {
                                    profile.peer_address = peer.to_string();
                                }
                            }
                        }
                    }
                }
            }
            "ifconfig-ipv6" => {
                if let Some(local) = first_arg(args) {
                    if let Some((address, explicit_prefix)) = split_local_address(local) {
                        let peer = args.get(1).map(String::as_str).filter(|v| !v.is_empty());
                        let mask_prefix = peer.and_then(ipv6_mask_prefix);
                        let prefix = explicit_prefix
                            .or(mask_prefix)
                            .unwrap_or(if address.is_ipv4() { 32 } else { 128 });
                        profile.address.push(format!("{address}/{prefix}"));
                        if mask_prefix.is_none() {
                            if let Some(peer) = peer {
                                if peer.parse::<std::net::IpAddr>().is_ok() {
                                    profile.peer_address_ipv6 = peer.to_string();
                                }
                            }
                        }
                    }
                }
            }
            "topology" => {
                if let Some(value) = first_arg(args) {
                    let lower = value.to_ascii_lowercase();
                    if matches!(lower.as_str(), "net30" | "p2p" | "subnet") {
                        profile.topology = lower;
                    }
                }
            }
            _ => {}
        }

        if keep {
            let line = strip_comment(line).trim();
            if !line.is_empty() {
                output.push(line.to_string());
            }
        }
    }

    profile.normalized = output.join("\n");
    Ok(profile)
}

fn apply_inline(profile: &mut Profile, tag: &str, content: &str) {
    match tag {
        "ca" => profile.tls.certificate = content.to_string(),
        "cert" => profile.tls.client_certificate = content.to_string(),
        "key" => profile.tls.client_key = content.to_string(),
        "tls-auth" => {
            profile.tls.control_wrap_type = "tls_auth".into();
            profile.tls.control_wrap_key = content.to_string();
        }
        "tls-crypt" => {
            profile.tls.control_wrap_type = "tls_crypt".into();
            profile.tls.control_wrap_key = content.to_string();
        }
        "tls-crypt-v2" => {
            profile.tls.control_wrap_type = "tls_crypt_v2".into();
            profile.tls.control_wrap_key = content.to_string();
        }
        "secret" => {
            profile.static_key_mode = true;
            profile.static_key = content.to_string();
        }
        "auth-user-pass" => apply_auth_user_pass(profile, content),
        _ => {}
    }
}

fn apply_auth_user_pass(profile: &mut Profile, content: &str) {
    let mut lines = content.lines();
    if let Some(username) = lines.next() {
        profile.username = username.trim().to_string();
    }
    if let Some(password) = lines.next() {
        profile.password = password.trim().to_string();
    }
}

fn read_inline(
    directive: &str,
    path: Option<&str>,
    base_dir: Option<&Path>,
    strict: bool,
) -> Result<Option<String>, String> {
    let Some(path) = path.map(str::trim).filter(|path| !path.is_empty()) else {
        return Ok(None);
    };
    let Some(base_dir) = base_dir else {
        if strict {
            return Err(format!(
                "профиль OpenVPN ссылается на внешний файл «{path}» ({directive}). Загрузите .ovpn файл через приложение, чтобы встроить его содержимое"
            ));
        }
        return Ok(None);
    };
    let candidate = Path::new(path);
    let full: PathBuf = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        base_dir.join(candidate)
    };
    match std::fs::read_to_string(&full) {
        Ok(content) => Ok(Some(content.trim_end().to_string())),
        Err(error) => {
            if strict {
                Err(format!(
                    "не удалось прочитать файл «{path}» ({directive}): {error}"
                ))
            } else {
                Ok(None)
            }
        }
    }
}

fn read_credentials(
    path: &str,
    base_dir: Option<&Path>,
    strict: bool,
) -> Result<Option<String>, String> {
    let Some(base_dir) = base_dir else {
        if strict {
            return Err(format!(
                "профиль OpenVPN ссылается на внешний файл «{path}» (auth-user-pass). Загрузите .ovpn файл через приложение, чтобы встроить его содержимое"
            ));
        }
        return Ok(None);
    };
    let candidate = Path::new(path);
    let full: PathBuf = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        base_dir.join(candidate)
    };
    match std::fs::read_to_string(&full) {
        Ok(content) => Ok(Some(content)),
        Err(error) => Err(format!(
            "не удалось прочитать файл логина/пароля «{path}»: {error}"
        )),
    }
}

fn route_to_cidr(args: &[String]) -> Option<String> {
    let first = args.first()?.trim();
    if first.is_empty() {
        return None;
    }
    if first.contains('/') {
        return Some(first.to_string());
    }
    let address: std::net::IpAddr = first.parse().ok()?;
    if let Some(prefix) = args.get(1).and_then(|mask| mask_to_prefix(mask)) {
        return Some(format!("{first}/{prefix}"));
    }
    Some(format!(
        "{first}/{}",
        if address.is_ipv4() { 32 } else { 128 }
    ))
}

fn mask_to_prefix(mask: &str) -> Option<u8> {
    let address: std::net::Ipv4Addr = mask.trim().parse().ok()?;
    let bits = u32::from(address);
    let prefix = bits.leading_ones() as u8;
    let expected = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    (bits == expected).then_some(prefix)
}

/// Split `addr` or `addr/bits` into an address and an optional prefix length.
fn split_local_address(value: &str) -> Option<(std::net::IpAddr, Option<u8>)> {
    let value = value.trim();
    let (address, prefix) = match value.split_once('/') {
        Some((address, prefix)) => (address, prefix.trim().parse::<u8>().ok()),
        None => (value, None),
    };
    let address = address.parse::<std::net::IpAddr>().ok()?;
    let max = if address.is_ipv4() { 32 } else { 128 };
    Some((address, prefix.filter(|prefix| *prefix <= max)))
}

/// IPv6 has no netmask, but a second `ifconfig-ipv6` argument may still be a
/// prefix written as `64` or `::/64`.
fn ipv6_mask_prefix(value: &str) -> Option<u8> {
    let value = value.trim();
    if let Ok(bits) = value.parse::<u8>() {
        return (bits <= 128).then_some(bits);
    }
    let (_, prefix) = value.split_once('/')?;
    let bits = prefix.trim().parse::<u8>().ok()?;
    (bits <= 128).then_some(bits)
}

fn network_of(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "udp" => Some("udp".into()),
        "udp4" => Some("udp4".into()),
        "udp6" => Some("udp6".into()),
        "tcp" | "tcp-client" => Some("tcp".into()),
        "tcp4" => Some("tcp4".into()),
        "tcp6" => Some("tcp6".into()),
        _ => None,
    }
}

fn direction_of(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "server" => Some("server".into()),
        "0" | "client" => Some("client".into()),
        _ => None,
    }
}

fn first_arg(args: &[String]) -> Option<&str> {
    args.first()
        .map(String::as_str)
        .filter(|arg| !arg.is_empty())
}

fn args_joined(args: &[String], skip: usize) -> String {
    args.iter()
        .skip(skip)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn inline_tag_open(line: &str) -> Option<String> {
    let rest = line.strip_prefix('<')?;
    let tag = rest.strip_suffix('>')?;
    if tag.is_empty() || tag.starts_with('/') {
        return None;
    }
    if !tag
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return None;
    }
    Some(tag.to_ascii_lowercase())
}

fn strip_comment(line: &str) -> &str {
    let mut in_token = false;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (index, c) in line.char_indices() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' && q == '"' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => {
                quote = Some(c);
                in_token = true;
            }
            '#' | ';' if !in_token => return &line[..index],
            c if c.is_whitespace() => in_token = false,
            _ => in_token = true,
        }
    }
    line
}

fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut quote: Option<char> = None;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if q == '"' && c == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else if c == q {
                    quote = None;
                } else {
                    current.push(c);
                }
            }
            None => {
                if (c == '#' || c == ';') && !in_token {
                    break;
                }
                if c == '"' || c == '\'' {
                    quote = Some(c);
                    in_token = true;
                } else if c.is_whitespace() {
                    if in_token {
                        tokens.push(std::mem::take(&mut current));
                        in_token = false;
                    }
                } else {
                    current.push(c);
                    in_token = true;
                }
            }
        }
    }
    if in_token || !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link::parse_link;
    use crate::model::{CoreCapabilities, Settings};
    use crate::singbox::build_config_with_capabilities;

    fn b64(text: &str) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(text)
    }

    const MINIMAL: &str = "client\ndev tun\nproto udp\nremote vpn.example.com 1194\n";

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("protogrid-ovpn-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn minimal_profile_becomes_endpoint() {
        assert!(looks_like_profile(MINIMAL));
        let server = import_profile(MINIMAL, None).unwrap();
        assert_eq!(server.protocol, Protocol::OpenVpn);
        assert_eq!(server.address, "vpn.example.com");
        assert_eq!(server.port, 1194);
        assert_eq!(server.security, Security::Tls);
        assert_eq!(server.name, "OpenVPN vpn.example.com");

        let endpoint = endpoint(&server).unwrap();
        assert_eq!(endpoint["type"], "openvpn-client");
        assert_eq!(endpoint["tag"], server.id);
        assert_eq!(endpoint["system"], false);
        assert_eq!(endpoint["server"], "vpn.example.com");
        assert_eq!(endpoint["server_port"], 1194);
        assert_eq!(endpoint["network"], "udp");
        assert!(endpoint.get("servers").is_none());
        assert!(endpoint.get("mode").is_none());
        assert!(endpoint.get("username").is_none());
    }

    #[test]
    fn inline_tls_blocks_are_inlined() {
        let profile = "client\ndev tun\nproto udp\nremote vpn.example.com 1194\n\
<ca>\n-----BEGIN CERTIFICATE-----\nCA\n-----END CERTIFICATE-----\n</ca>\n\
<cert>\nCERT\n</cert>\n\
<key>\nKEY\n</key>\n\
<tls-crypt>\nCRYPT\n</tls-crypt>\n\
remote-cert-tls server\n\
verify-x509-name vpn.example.com name\n";
        let server = import_profile(profile, None).unwrap();
        let endpoint = endpoint(&server).unwrap();
        assert_eq!(
            endpoint["tls"]["certificate"],
            "-----BEGIN CERTIFICATE-----\nCA\n-----END CERTIFICATE-----"
        );
        assert_eq!(endpoint["tls"]["client_certificate"], "CERT");
        assert_eq!(endpoint["tls"]["client_key"], "KEY");
        assert_eq!(endpoint["tls"]["control_wrap"]["type"], "tls_crypt");
        assert_eq!(endpoint["tls"]["control_wrap"]["key"], "CRYPT");
        assert!(endpoint["tls"]["control_wrap"].get("direction").is_none());
        assert_eq!(endpoint["tls"]["server_name"], "vpn.example.com");
        assert_eq!(endpoint["tls"]["server_name_type"], "name");
        assert_eq!(endpoint["tls"]["remote_certificate_tls"], "server");
    }

    #[test]
    fn data_channel_options_are_mapped() {
        let profile = "client\ndev tun\nproto udp\nremote h 1194\n\
data-ciphers AES-256-GCM:CHACHA20-POLY1305\n\
auth SHA256\n\
cipher AES-256-CBC\n\
mssfix 1450 mtu\n\
fragment 1300\n\
compress\n";
        let server = import_profile(profile, None).unwrap();
        let endpoint = endpoint(&server).unwrap();
        assert_eq!(
            endpoint["data_ciphers"],
            json!(["AES-256-GCM", "CHACHA20-POLY1305"])
        );
        assert_eq!(endpoint["auth"], "SHA256");
        // `cipher` is illegal in TLS mode; it becomes the negotiation fallback.
        assert!(endpoint.get("cipher").is_none());
        assert_eq!(endpoint["data_ciphers_fallback"], "AES-256-CBC");
        assert_eq!(endpoint["mss_fix"], 1450);
        assert_eq!(endpoint["mss_fix_mode"], "mtu");
        assert_eq!(endpoint["fragment"], 1300);
        // A bare `compress` is LZO stub framing (v1), not LZO compression.
        assert_eq!(endpoint["compression"], "stub");
        assert!(endpoint.get("compression_lzo").is_none());
    }

    #[test]
    fn compression_directives_follow_sing_openvpn() {
        let server = import_profile(
            "client\ndev tun\nproto udp\nremote h 1194\ncomp-lzo\n",
            None,
        )
        .unwrap();
        let rendered = endpoint(&server).unwrap();
        assert_eq!(rendered["compression_lzo"], "adaptive");
        assert!(rendered.get("compression").is_none());

        for (variant, expected) in [("yes", "yes"), ("adaptive", "adaptive"), ("asym", "asym")] {
            let profile =
                format!("client\ndev tun\nproto udp\nremote h 1194\ncomp-lzo {variant}\n");
            let rendered = endpoint(&import_profile(&profile, None).unwrap()).unwrap();
            assert_eq!(rendered["compression_lzo"], expected);
        }

        for variant in ["stub", "stub-v2", "lz4", "lz4-v2"] {
            let profile =
                format!("client\ndev tun\nproto udp\nremote h 1194\ncompress {variant}\n");
            let rendered = endpoint(&import_profile(&profile, None).unwrap()).unwrap();
            assert_eq!(rendered["compression"], variant);
            assert!(rendered.get("compression_lzo").is_none());
        }

        let rendered = endpoint(
            &import_profile(
                "client\ndev tun\nproto udp\nremote h 1194\ncompress lzo\n",
                None,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(rendered["compression_lzo"], "yes");
        assert!(rendered.get("compression").is_none());

        for variant in ["no", "none", "off", "disabled", "migrate"] {
            let profile = format!(
                "client\ndev tun\nproto udp\nremote h 1194\ncomp-lzo yes\ncompress {variant}\n"
            );
            let rendered = endpoint(&import_profile(&profile, None).unwrap()).unwrap();
            assert!(rendered.get("compression").is_none(), "{variant}");
            assert!(rendered.get("compression_lzo").is_none(), "{variant}");
        }
    }

    #[test]
    fn data_cipher_prefixes_are_stripped() {
        let profile = "client\ndev tun\nproto udp\nremote h 1194\n\
data-ciphers ?AES-256-GCM:!CHACHA20-POLY1305:?:!AES-128-GCM\n";
        let server = import_profile(profile, None).unwrap();
        let endpoint = endpoint(&server).unwrap();
        assert_eq!(
            endpoint["data_ciphers"],
            json!(["AES-256-GCM", "CHACHA20-POLY1305", "AES-128-GCM"])
        );
    }

    #[test]
    fn mssfix_zero_disables_clamping() {
        let server = import_profile(
            "client\ndev tun\nproto udp\nremote h 1194\nmssfix 0 mtu\n",
            None,
        )
        .unwrap();
        let rendered = endpoint(&server).unwrap();
        assert_eq!(rendered["mss_fix_disabled"], true);
        assert!(rendered.get("mss_fix").is_none());
        assert!(rendered.get("mss_fix_mode").is_none());

        let server =
            import_profile("client\ndev tun\nproto udp\nremote h 1194\nmssfix\n", None).unwrap();
        let rendered = endpoint(&server).unwrap();
        assert_eq!(rendered["mss_fix"], 1492);
        assert!(rendered.get("mss_fix_disabled").is_none());
    }

    #[test]
    fn routes_and_redirect_gateway() {
        let profile = "client\ndev tun\nproto udp\nremote h 1194\n\
route 10.0.0.0 255.255.255.0\n\
route 192.168.5.5\n\
route 2001:db8::/32\n\
route-metric 50\n\
redirect-gateway def1\n";
        let server = import_profile(profile, None).unwrap();
        let endpoint = endpoint(&server).unwrap();
        assert_eq!(
            endpoint["routes"],
            json!(["10.0.0.0/24", "192.168.5.5/32", "2001:db8::/32"])
        );
        assert_eq!(endpoint["route_metric"], 50);
        assert_eq!(endpoint["redirect_gateway"], true);
        assert_eq!(endpoint["redirect_gateway_flags"], json!(["def1"]));
    }

    #[test]
    fn auth_user_pass_file_is_embedded() {
        let dir = temp_dir("auth");
        std::fs::write(dir.join("auth.txt"), "alice\ns3cret\n").unwrap();
        let profile = "client\ndev tun\nproto udp\nremote h 1194\nauth-user-pass auth.txt\n";
        let server = import_profile(profile, Some(&dir)).unwrap();
        assert_eq!(server.openvpn_username, "alice");
        assert_eq!(server.openvpn_password, "s3cret");
        assert!(!server.openvpn_profile.contains("auth.txt"));
        assert_eq!(
            server.openvpn_profile.lines().last(),
            Some("auth-user-pass")
        );
        let endpoint = endpoint(&server).unwrap();
        assert_eq!(endpoint["username"], "alice");
        assert_eq!(endpoint["password"], "s3cret");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn auth_user_pass_file_without_base_dir_is_rejected() {
        let profile = "client\ndev tun\nproto udp\nremote h 1194\nauth-user-pass auth.txt\n";
        let error = import_profile(profile, None).unwrap_err();
        assert!(error.contains("auth.txt"), "unexpected error: {error}");
        assert!(error.contains(".ovpn"), "unexpected error: {error}");

        // endpoint() re-parses leniently and must not fail on the stale path.
        let mut server = Server::new(Protocol::OpenVpn);
        server.address = "h".into();
        server.port = 1194;
        server.openvpn_profile = profile.to_string();
        let endpoint_value = endpoint(&server).unwrap();
        assert_eq!(endpoint_value["server"], "h");
        assert!(endpoint_value.get("username").is_none());
        assert!(endpoint_value.get("password").is_none());
    }

    #[test]
    fn import_fills_server_fields() {
        let dir = temp_dir("files");
        std::fs::write(dir.join("ca.crt"), "CA\n").unwrap();
        let profile = "client\ndev tun\nproto tcp-client\nremote vpn.example.org 443\nauth-user-pass\nca ca.crt\n";
        let server = import_profile(profile, Some(&dir)).unwrap();
        assert_eq!(server.address, "vpn.example.org");
        assert_eq!(server.port, 443);
        assert_eq!(server.name, "OpenVPN vpn.example.org");
        assert_eq!(server.security, Security::Tls);
        assert!(server.openvpn_profile.contains("<ca>\nCA\n</ca>"));
        assert!(!server.openvpn_profile.contains("ca.crt"));
        let endpoint = endpoint(&server).unwrap();
        assert_eq!(endpoint["network"], "tcp");
        assert_eq!(endpoint["tls"]["certificate"], "CA");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn external_files_without_base_dir_are_rejected() {
        let profile = "client\ndev tun\nproto udp\nremote h 1194\nca ca.crt\n";
        let error = import_profile(profile, None).unwrap_err();
        assert!(error.contains("ca.crt"), "unexpected error: {error}");
    }

    #[test]
    fn endpoint_handles_multiple_remotes_and_overrides() {
        let profile = "client\ndev tun\nremote a.example.com 1194\nremote b.example.com 443 udp\n";
        let mut server = import_profile(profile, None).unwrap();
        let endpoint_value = endpoint(&server).unwrap();
        let servers = endpoint_value["servers"].as_array().unwrap();
        assert_eq!(servers.len(), 2);
        assert!(endpoint_value.get("server").is_none());
        assert_eq!(servers[0]["server"], "a.example.com");
        assert_eq!(servers[0]["server_port"], 1194);
        assert_eq!(servers[1]["server"], "b.example.com");
        assert_eq!(servers[1]["server_port"], 443);
        assert_eq!(servers[1]["network"], "udp");

        server.address = "manual.example.com".into();
        server.port = 8443;
        let endpoint_value = endpoint(&server).unwrap();
        let servers = endpoint_value["servers"].as_array().unwrap();
        assert_eq!(servers[0]["server"], "manual.example.com");
        assert_eq!(servers[0]["server_port"], 8443);
        assert_eq!(servers[1]["server"], "b.example.com");
    }

    #[test]
    fn endpoint_omits_empty_fields() {
        let server = import_profile(MINIMAL, None).unwrap();
        let endpoint_value = endpoint(&server).unwrap();
        let object = endpoint_value.as_object().unwrap();
        for key in [
            "username",
            "password",
            "mode",
            "routes",
            "pull_filters",
            "compression",
            "mss_fix",
            "fragment",
            "mtu",
            "servers",
            "key_direction",
        ] {
            assert!(!object.contains_key(key), "unexpected key {key}");
        }
    }

    #[test]
    fn tls_auth_direction_mapping() {
        let profile =
            "client\ndev tun\nproto udp\nremote h 1194\ntls-auth ta.key 1\nkey-direction 0\n";
        let server = import_profile_from_blocks(profile, "ta.key", "[static key]\n");
        let endpoint = endpoint(&server).unwrap();
        assert_eq!(endpoint["tls"]["control_wrap"]["type"], "tls_auth");
        assert_eq!(endpoint["tls"]["control_wrap"]["key"], "[static key]");
        // `key-direction 0` follows `tls-auth ... 1` and wins.
        assert_eq!(endpoint["tls"]["control_wrap"]["direction"], "client");
    }

    fn import_profile_from_blocks(profile: &str, file: &str, content: &str) -> Server {
        let dir = temp_dir("block");
        std::fs::write(dir.join(file), content).unwrap();
        let result = import_profile(profile, Some(&dir)).unwrap();
        let _ = std::fs::remove_dir_all(dir);
        result
    }

    #[test]
    fn static_key_mode_is_detected() {
        let profile = "client\ndev tun\nproto udp\nremote h 1194\nsecret sk.key 1\nifconfig 10.8.0.2 10.8.0.1\n";
        let server = import_profile_from_blocks(profile, "sk.key", "STATIC\n");
        assert_eq!(server.security, Security::None);
        let endpoint = endpoint(&server).unwrap();
        assert_eq!(endpoint["mode"], "static_key");
        assert_eq!(endpoint["static_key"], "STATIC");
        assert_eq!(endpoint["key_direction"], "server");
        assert_eq!(endpoint["address"], json!(["10.8.0.2/32"]));
        assert_eq!(endpoint["peer_address"], "10.8.0.1");
        assert!(endpoint.get("tls").is_none());
    }

    #[test]
    fn rejects_tap_and_tcp_server() {
        assert!(import_profile("client\ndev tap\nremote h\n", None).is_err());
        assert!(import_profile("client\ndev tun\nproto tcp-server\nremote h\n", None).is_err());
    }

    #[test]
    fn config_puts_openvpn_into_endpoints() {
        let server = import_profile(MINIMAL, None).unwrap();
        let capabilities = CoreCapabilities {
            openvpn: true,
            vless_only: false,
            ..Default::default()
        };
        let config = build_config_with_capabilities(
            std::slice::from_ref(&server),
            &server.id,
            &Settings::default(),
            &[],
            Some("1.14.4"),
            None,
            capabilities,
        )
        .unwrap();
        let endpoints = config["endpoints"].as_array().unwrap();
        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0]["type"], "openvpn-client");
        assert_eq!(endpoints[0]["tag"], server.id);
        let outbound_tags: Vec<&str> = config["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|outbound| outbound["tag"].as_str())
            .collect();
        assert!(!outbound_tags.contains(&server.id.as_str()));
        assert!(config["outbounds"][0]["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tag| tag == &json!(server.id)));
        assert_eq!(config["route"]["final"], "proxy");

        let stock = build_config_with_capabilities(
            std::slice::from_ref(&server),
            &server.id,
            &Settings::default(),
            &[],
            Some("1.14.4"),
            None,
            CoreCapabilities::default(),
        );
        assert!(stock.is_err());
    }

    #[test]
    fn config_without_openvpn_has_no_endpoints_key() {
        let mut server = Server::new(Protocol::Vless);
        server.id = "srv-1".into();
        server.address = "example.com".into();
        server.uuid = "11111111-1111-1111-1111-111111111111".into();
        let config = build_config_with_capabilities(
            &[server],
            "srv-1",
            &Settings::default(),
            &[],
            Some("1.14.4"),
            None,
            CoreCapabilities::default(),
        )
        .unwrap();
        assert!(config.get("endpoints").is_none());
    }

    #[test]
    fn parse_links_accepts_raw_profile() {
        let outcome = crate::link::parse_links(MINIMAL);
        assert_eq!(outcome.errors, Vec::<String>::new());
        assert_eq!(outcome.servers.len(), 1);
        assert_eq!(outcome.servers[0].protocol, Protocol::OpenVpn);
        assert_eq!(outcome.servers[0].address, "vpn.example.com");
    }

    #[test]
    fn parse_links_accepts_ovpn_link() {
        let encoded = b64(MINIMAL);
        for scheme in ["ovpn", "openvpn"] {
            let server = parse_link(&format!("{scheme}://{encoded}")).unwrap();
            assert_eq!(server.protocol, Protocol::OpenVpn);
            assert_eq!(server.port, 1194);
        }
        let outcome = crate::link::parse_links(&format!("ovpn://{encoded}"));
        assert_eq!(outcome.servers.len(), 1);
        assert!(outcome.errors.is_empty());
    }

    #[test]
    fn urls_in_comments_do_not_break_profile_detection() {
        let profile = "# https://provider.example/config\n\
; mirror: https://mirror.example/ovpn\n\
client\ndev tun\nproto udp\nremote vpn.example.com 1194\n";
        assert!(looks_like_profile(profile));
        assert!(!looks_like_profile("https://example.com/subscription"));
        assert!(!looks_like_profile(
            "vless://id@example.com:443?type=tcp#Node"
        ));

        let outcome = crate::link::parse_links(profile);
        assert!(outcome.errors.is_empty(), "{:?}", outcome.errors);
        assert_eq!(outcome.servers.len(), 1);
        assert_eq!(outcome.servers[0].address, "vpn.example.com");
    }

    #[test]
    fn ifconfig_subnet_mask_becomes_prefix() {
        // `topology` before `ifconfig`.
        let profile = "client\ndev tun\nproto udp\nremote h 1194\n\
topology subnet\nifconfig 10.8.0.2 255.255.255.0\n";
        let rendered = endpoint(&import_profile(profile, None).unwrap()).unwrap();
        assert_eq!(rendered["address"], json!(["10.8.0.2/24"]));
        assert_eq!(rendered["topology"], "subnet");
        assert!(rendered.get("peer_address").is_none());

        // `topology` after `ifconfig` — the result must be the same.
        let profile = "client\ndev tun\nproto udp\nremote h 1194\n\
ifconfig 10.8.0.2 255.255.255.0\ntopology subnet\n";
        let rendered = endpoint(&import_profile(profile, None).unwrap()).unwrap();
        assert_eq!(rendered["address"], json!(["10.8.0.2/24"]));
        assert_eq!(rendered["topology"], "subnet");
        assert!(rendered.get("peer_address").is_none());
    }

    #[test]
    fn ifconfig_p2p_keeps_peer_address() {
        let profile = "client\ndev tun\nproto udp\nremote h 1194\n\
topology p2p\nifconfig 10.8.0.2 10.8.0.1\n";
        let rendered = endpoint(&import_profile(profile, None).unwrap()).unwrap();
        assert_eq!(rendered["address"], json!(["10.8.0.2/32"]));
        assert_eq!(rendered["peer_address"], "10.8.0.1");
    }

    #[test]
    fn ifconfig_ipv6_handles_prefix_and_peer() {
        let profile = "client\ndev tun\nproto udp\nremote h 1194\n\
ifconfig-ipv6 fd00::2/64 fd00::1\n";
        let rendered = endpoint(&import_profile(profile, None).unwrap()).unwrap();
        assert_eq!(rendered["address"], json!(["fd00::2/64"]));
        assert_eq!(rendered["peer_address_ipv6"], "fd00::1");

        let profile = "client\ndev tun\nproto udp\nremote h 1194\n\
ifconfig-ipv6 fd00::2 ::/64\n";
        let rendered = endpoint(&import_profile(profile, None).unwrap()).unwrap();
        assert_eq!(rendered["address"], json!(["fd00::2/64"]));
        assert!(rendered.get("peer_address_ipv6").is_none());
    }
}
