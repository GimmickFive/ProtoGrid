use std::fs;

use protogrid_core::model::{
    default_rules, CoreCapabilities, Protocol, Security, Server, Settings, Transport,
};
use protogrid_core::{singbox, xray};

fn server(protocol: Protocol, name: &str, address: &str, port: u16) -> Server {
    let mut s = Server::new(protocol);
    s.id = format!("srv-{name}");
    s.name = name.into();
    s.address = address.into();
    s.port = port;
    s
}

fn main() {
    let mut vless = server(Protocol::Vless, "vless-reality", "example.com", 443);
    vless.uuid = "11111111-1111-1111-1111-111111111111".into();
    vless.security = Security::Reality;
    vless.fingerprint = "chrome".into();
    vless.reality_public_key = "jNXHt1yRo0vDuchQlIP6Z0ZvjT3KtzVI-T4E7RoLJS0".into();
    vless.reality_short_id = "0123".into();
    vless.transport = Transport::Grpc;
    vless.service_name = "svc".into();
    vless.fragment_enabled = true;
    vless.noise_enabled = true;
    vless.noise_packet = "abcd".into();
    vless.finalmask_tcp = r#"[{"type":"filter","settings":{"blockedSourceIPs":["192.0.2.0/24"]}},{"type":"header-custom","settings":{"clients":[]}}]"#.into();
    vless.finalmask_udp =
        r#"[{"type":"filter","settings":{"allowedSourceIPs":["192.0.2.0/24"]}}]"#.into();

    let mut vmess = server(Protocol::Vmess, "vmess-ws", "vmess.example.com", 443);
    vmess.uuid = "22222222-2222-2222-2222-222222222222".into();
    vmess.security = Security::Tls;
    vmess.transport = Transport::Ws;
    vmess.path = "/ws".into();
    vmess.host = "vmess.example.com".into();
    vmess.sni = "vmess.example.com".into();
    vmess.fingerprint = "chrome".into();
    vmess.alpn = vec!["h2".into(), "http/1.1".into()];

    let mut trojan = server(Protocol::Trojan, "trojan", "trojan.example.com", 443);
    trojan.password = "secret".into();
    trojan.security = Security::Tls;
    trojan.sni = "trojan.example.com".into();

    let mut ss = server(Protocol::Shadowsocks, "ss", "ss.example.com", 8388);
    ss.method = "aes-256-gcm".into();
    ss.password = "secret".into();

    let mut hy = server(Protocol::Hysteria2, "hy2", "hy.example.com", 443);
    hy.password = "secret".into();
    hy.security = Security::Tls;
    hy.sni = "hy.example.com".into();
    hy.obfs = "salamander".into();
    hy.obfs_password = "obfs".into();

    let mut tuic = server(Protocol::Tuic, "tuic", "tuic.example.com", 443);
    tuic.uuid = "33333333-3333-3333-3333-333333333333".into();
    tuic.password = "secret".into();
    tuic.security = Security::Tls;
    tuic.sni = "tuic.example.com".into();
    tuic.alpn = vec!["h3".into()];

    let mut masque = server(Protocol::Masque, "masque", "masque.example.com", 443);
    masque.security = Security::Tls;
    masque.sni = "masque.example.com".into();
    masque.masque_user = "user".into();
    masque.masque_pass = "pass".into();
    masque.host = "masque.example.com".into();
    masque.path = "/masque".into();
    masque.pinned_peer_cert_sha256 = "00".repeat(32);

    let mut http = server(Protocol::Vless, "vless-http", "http.example.com", 443);
    http.uuid = uuid::Uuid::new_v4().to_string();
    http.security = Security::Tls;
    http.sni = "http.example.com".into();
    http.transport = Transport::Http;
    http.path = "/h2".into();
    http.host = "http.example.com".into();

    let mut upgrade = server(Protocol::Vless, "vless-upgrade", "up.example.com", 443);
    upgrade.uuid = uuid::Uuid::new_v4().to_string();
    upgrade.security = Security::Tls;
    upgrade.transport = Transport::HttpUpgrade;
    upgrade.path = "/up".into();
    upgrade.host = "up.example.com".into();

    let mut xhttp = server(Protocol::Vless, "vless-xhttp", "xhttp.example.com", 443);
    xhttp.uuid = uuid::Uuid::new_v4().to_string();
    xhttp.security = Security::Tls;
    xhttp.transport = Transport::Xhttp;
    xhttp.path = "/x".into();
    xhttp.host = "xhttp.example.com".into();

    let servers = vec![
        vless.clone(),
        vmess,
        trojan,
        ss,
        hy,
        tuic,
        masque,
        http,
        upgrade,
        xhttp,
    ];

    let settings = Settings {
        tun_enabled: true,
        fake_dns: true,
        auto_select: true,
        ..Default::default()
    };
    let sb = singbox::build_config_with_capabilities(
        &servers,
        &vless.id,
        &settings,
        &default_rules(),
        Some("1.14.4-protogrid.1"),
        None,
        CoreCapabilities {
            xhttp: true,
            kcp: true,
            xdrive: true,
            mldsa65: true,
            masque: true,
            noise: true,
            openvpn: true,
        },
    )
    .unwrap();
    fs::write(
        "/tmp/cores/generated-sb.json",
        serde_json::to_string_pretty(&sb).unwrap(),
    )
    .unwrap();

    let settings = Settings {
        auto_select: true,
        ..Default::default()
    };
    let xr = xray::build_config(&servers, &vless.id, &settings, &default_rules()).unwrap();
    fs::write(
        "/tmp/cores/generated-xr.json",
        serde_json::to_string_pretty(&xr).unwrap(),
    )
    .unwrap();

    for s in &servers {
        if s.validate(protogrid_core::model::CoreKind::Xray).is_err() {
            continue;
        }
        let config =
            xray::build_config(std::slice::from_ref(s), &s.id, &settings, &default_rules())
                .unwrap();
        fs::write(
            format!("/tmp/cores/generated-xr-{}.json", s.id),
            serde_json::to_string_pretty(&config).unwrap(),
        )
        .unwrap();
    }
    println!("written");
}
