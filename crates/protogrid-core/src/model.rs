use serde::{Deserialize, Serialize};

use crate::subscription::SubUserInfo;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    #[default]
    Vless,
    Vmess,
    Trojan,
    Shadowsocks,
    Hysteria2,
    Tuic,
    Masque,
    OpenVpn,
}

impl Protocol {
    pub fn label(self) -> &'static str {
        match self {
            Protocol::Vless => "VLESS",
            Protocol::Vmess => "VMess",
            Protocol::Trojan => "Trojan",
            Protocol::Shadowsocks => "Shadowsocks",
            Protocol::Hysteria2 => "Hysteria2",
            Protocol::Tuic => "TUIC",
            Protocol::Masque => "MASQUE",
            Protocol::OpenVpn => "OpenVPN",
        }
    }

    pub fn default_port(self) -> u16 {
        match self {
            Protocol::Shadowsocks => 8388,
            Protocol::OpenVpn => 1194,
            _ => 443,
        }
    }

    pub fn supports(self, core: CoreKind) -> bool {
        match core {
            CoreKind::Singbox | CoreKind::Protogrid => true,
            CoreKind::ProtogridLite => self == Protocol::Vless,
            CoreKind::Xray => matches!(
                self,
                Protocol::Vless
                    | Protocol::Vmess
                    | Protocol::Trojan
                    | Protocol::Shadowsocks
                    | Protocol::Masque
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Security {
    #[default]
    None,
    Tls,
    Reality,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    #[default]
    Tcp,
    Ws,
    Grpc,
    Http,
    HttpUpgrade,
    Xhttp,
    Kcp,
    XDrive,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum CoreKind {
    #[serde(rename = "singbox")]
    Singbox,
    #[serde(rename = "xray")]
    Xray,
    /// ProtoGrid-core: a separate core based on sing-box with its own binary,
    /// path and capabilities (XHTTP, mKCP, XDRIVE, MASQUE).
    #[default]
    #[serde(rename = "protogrid")]
    Protogrid,
    /// ProtoGrid-core Lite: the same core cut down to VLESS/Reality over TCP.
    /// TUN and the mixed inbound are kept, so both VPN and system-proxy modes
    /// still work.
    #[serde(rename = "protogrid-lite")]
    ProtogridLite,
}

impl CoreKind {
    pub fn name(self) -> &'static str {
        match self {
            CoreKind::Singbox => "sing-box",
            CoreKind::Xray => "Xray",
            CoreKind::Protogrid => "ProtoGrid-core",
            CoreKind::ProtogridLite => "ProtoGrid-core Lite",
        }
    }

    pub fn binary_name(self) -> &'static str {
        match self {
            CoreKind::Singbox | CoreKind::Protogrid => "sing-box",
            CoreKind::Xray => "xray",
            CoreKind::ProtogridLite => "protogrid-core-lite",
        }
    }

    /// ProtoGrid-core speaks the sing-box config language. Everything that
    /// is true for sing-box (ports, TUN, config generation) is true for it too.
    pub fn is_singbox_like(self) -> bool {
        matches!(
            self,
            CoreKind::Singbox | CoreKind::Protogrid | CoreKind::ProtogridLite
        )
    }
}

/// Optional features exposed by a specific core build.
///
/// Stock sing-box does not implement XHTTP; ProtoGrid-core does. Keeping this
/// in a separate struct lets the config generator stay compatible with both.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CoreCapabilities {
    pub xhttp: bool,
    pub kcp: bool,
    pub xdrive: bool,
    /// REALITY ML-DSA-65 server verification (Xray 26 / ProtoGrid-core).
    /// Stock sing-box rejects the `mldsa65_verify` option.
    pub mldsa65: bool,
    /// MASQUE outbound (Xray / ProtoGrid-core). Stock sing-box has no such protocol.
    pub masque: bool,
    /// Xray-compatible UDP finalmask noise (ProtoGrid-core / Xray).
    pub noise: bool,
    /// OpenVPN client endpoint (`openvpn-client`). Requires sing-box 1.14+
    /// built with `with_openvpn` or ProtoGrid-core.
    pub openvpn: bool,
    /// ProtoGrid-core Lite: only VLESS over plain TCP is registered. Other
    /// protocols and transports are rejected before the config is written.
    pub vless_only: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    #[default]
    Proxy,
    Direct,
    Block,
}

impl RuleAction {
    pub fn label(self) -> &'static str {
        match self {
            RuleAction::Proxy => "Через прокси",
            RuleAction::Direct => "Напрямую",
            RuleAction::Block => "Блокировать",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    Domain,
    #[default]
    DomainSuffix,
    DomainKeyword,
    IpCidr,
    Geoip,
    Geosite,
    ProcessName,
}

impl RuleKind {
    pub fn label(self) -> &'static str {
        match self {
            RuleKind::Domain => "Домен",
            RuleKind::DomainSuffix => "Суффикс домена",
            RuleKind::DomainKeyword => "Ключевое слово",
            RuleKind::IpCidr => "IP / CIDR",
            RuleKind::Geoip => "GeoIP",
            RuleKind::Geosite => "GeoSite",
            RuleKind::ProcessName => "Процесс",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoutingPreset {
    Bypass,
    Global,
    Direct,
    /// Rules were changed by hand, so they no longer match a preset.
    Custom,
}

impl RoutingPreset {
    pub fn id(self) -> &'static str {
        match self {
            RoutingPreset::Bypass => "bypass",
            RoutingPreset::Global => "global",
            RoutingPreset::Direct => "direct",
            RoutingPreset::Custom => "custom",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RoutingPreset::Bypass => "Жизнь в РФ",
            RoutingPreset::Global => "Глобальный прокси",
            RoutingPreset::Direct => "Прямой",
            RoutingPreset::Custom => "Свой профиль",
        }
    }

    /// Built-in presets ship a fixed ruleset; only `Custom` can be edited.
    pub fn is_builtin(self) -> bool {
        !matches!(self, RoutingPreset::Custom)
    }

    pub fn from_id(id: &str) -> Option<Self> {
        [
            RoutingPreset::Bypass,
            RoutingPreset::Global,
            RoutingPreset::Direct,
            RoutingPreset::Custom,
        ]
        .into_iter()
        .find(|preset| preset.id() == id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Server {
    pub id: String,
    pub name: String,
    pub protocol: Protocol,
    pub address: String,
    /// ISO 3166-1 alpha-2 country code resolved from the address on add.
    pub country: String,
    pub port: u16,
    pub uuid: String,
    pub password: String,
    pub method: String,
    pub security: Security,
    pub sni: String,
    pub alpn: Vec<String>,
    pub fingerprint: String,
    pub allow_insecure: bool,
    pub flow: String,
    /// Xray 26 VLESS encryption string (empty or "none" for plain VLESS).
    pub encryption: String,
    pub transport: Transport,
    pub path: String,
    pub host: String,
    /// XHTTP uplink mode: auto, packet-up, stream-up or stream-one.
    pub xhttp_mode: String,
    pub kcp_mtu: u16,
    pub kcp_tti: u16,
    pub kcp_uplink: u32,
    pub kcp_downlink: u32,
    pub kcp_congestion: bool,
    pub kcp_seed: String,
    pub tcp_fast_open: bool,
    pub bind_interface: String,
    pub routing_mark: u32,
    pub fragment_enabled: bool,
    pub fragment_packets: String,
    pub fragment_length: String,
    pub fragment_delay: String,
    pub fragment_max_split: String,
    pub finalmask_tcp: String,
    pub finalmask_udp: String,
    pub noise_enabled: bool,
    pub noise_type: String,
    pub noise_packet: String,
    pub noise_rand: String,
    pub xdrive_service: String,
    pub xdrive_remote_folder: String,
    pub xdrive_secrets: String,
    pub xdrive_s3_endpoint: String,
    pub xdrive_s3_region: String,
    pub xdrive_s3_bucket: String,
    pub xdrive_s3_path_style: bool,
    pub masque_user: String,
    pub masque_pass: String,
    /// Normalized `.ovpn` profile kept for the sing-box `openvpn-client`
    /// endpoint (external files are inlined on import).
    pub openvpn_profile: String,
    pub openvpn_username: String,
    pub openvpn_password: String,
    pub pinned_peer_cert_sha256: String,
    pub masque_remote_dns: String,
    pub service_name: String,
    pub reality_public_key: String,
    pub reality_short_id: String,
    pub reality_spider_x: String,
    /// Optional ML-DSA-65 public key for Reality (Xray 26).
    pub reality_mldsa65_verify: String,
    pub ss_plugin: String,
    pub ss_plugin_opts: String,
    pub vmess_security: String,
    pub vmess_alter_id: u16,
    pub obfs: String,
    pub obfs_password: String,
    pub up_mbps: u32,
    pub down_mbps: u32,
    pub congestion_control: String,
    pub udp_relay_mode: String,
    pub subscription_id: Option<String>,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: String::new(),
            protocol: Protocol::Vless,
            address: String::new(),
            country: String::new(),
            port: Protocol::Vless.default_port(),
            uuid: String::new(),
            password: String::new(),
            method: "aes-256-gcm".into(),
            security: Security::None,
            sni: String::new(),
            alpn: Vec::new(),
            fingerprint: String::new(),
            allow_insecure: false,
            flow: String::new(),
            encryption: String::new(),
            transport: Transport::Tcp,
            path: String::new(),
            host: String::new(),
            xhttp_mode: "auto".into(),
            kcp_mtu: 1350,
            kcp_tti: 50,
            kcp_uplink: 5,
            kcp_downlink: 20,
            kcp_congestion: false,
            kcp_seed: String::new(),
            tcp_fast_open: false,
            bind_interface: String::new(),
            routing_mark: 0,
            fragment_enabled: false,
            fragment_packets: "tlshello".into(),
            fragment_length: "100-200".into(),
            fragment_delay: "10-20".into(),
            fragment_max_split: "3-6".into(),
            finalmask_tcp: String::new(),
            finalmask_udp: String::new(),
            noise_enabled: false,
            noise_type: "str".into(),
            noise_packet: String::new(),
            noise_rand: "1-3".into(),
            xdrive_service: "local".into(),
            xdrive_remote_folder: String::new(),
            xdrive_secrets: String::new(),
            xdrive_s3_endpoint: String::new(),
            xdrive_s3_region: String::new(),
            xdrive_s3_bucket: String::new(),
            xdrive_s3_path_style: false,
            masque_user: String::new(),
            masque_pass: String::new(),
            openvpn_profile: String::new(),
            openvpn_username: String::new(),
            openvpn_password: String::new(),
            pinned_peer_cert_sha256: String::new(),
            masque_remote_dns: String::new(),
            service_name: String::new(),
            reality_public_key: String::new(),
            reality_short_id: String::new(),
            reality_spider_x: String::new(),
            reality_mldsa65_verify: String::new(),
            ss_plugin: String::new(),
            ss_plugin_opts: String::new(),
            vmess_security: "auto".into(),
            vmess_alter_id: 0,
            obfs: String::new(),
            obfs_password: String::new(),
            up_mbps: 0,
            down_mbps: 0,
            congestion_control: "bbr".into(),
            udp_relay_mode: "native".into(),
            subscription_id: None,
        }
    }
}

impl Server {
    pub fn new(protocol: Protocol) -> Self {
        Self {
            protocol,
            port: protocol.default_port(),
            ..Default::default()
        }
    }

    pub fn target(&self) -> String {
        if self.address.contains(':') && !self.address.starts_with('[') {
            format!("[{}]:{}", self.address, self.port)
        } else {
            format!("{}:{}", self.address, self.port)
        }
    }

    pub fn display_name(&self) -> String {
        if self.name.trim().is_empty() {
            self.target()
        } else {
            self.name.clone()
        }
    }

    /// XHTTP mode normalized to one of the values understood by the cores.
    pub fn effective_xhttp_mode(&self) -> &str {
        match self.xhttp_mode.trim() {
            "packet-up" => "packet-up",
            "stream-up" => "stream-up",
            "stream-one" => "stream-one",
            _ => "auto",
        }
    }

    /// Server name used for TLS SNI when the user did not set one explicitly.
    pub fn effective_sni(&self) -> String {
        if !self.sni.trim().is_empty() {
            return self.sni.trim().to_string();
        }
        if !self.host.trim().is_empty() && self.host.parse::<std::net::IpAddr>().is_err() {
            return self.host.trim().to_string();
        }
        self.address.trim().to_string()
    }

    pub fn validate(&self, core: CoreKind) -> Result<(), String> {
        self.validate_with(core, CoreCapabilities::default())
    }

    pub fn validate_with(
        &self,
        core: CoreKind,
        capabilities: CoreCapabilities,
    ) -> Result<(), String> {
        if self.address.trim().is_empty() {
            return Err("не указан адрес сервера".into());
        }
        if self.port == 0 {
            return Err("не указан порт".into());
        }
        if !self.protocol.supports(core) {
            return Err(format!(
                "протокол {} не поддерживается ядром {}. Выберите sing-box.",
                self.protocol.label(),
                core.name()
            ));
        }
        match self.protocol {
            Protocol::Vless | Protocol::Vmess => {
                if self.uuid.trim().is_empty() {
                    return Err("не указан UUID".into());
                }
            }
            Protocol::Trojan | Protocol::Hysteria2 => {
                if self.password.is_empty() {
                    return Err("не указан пароль".into());
                }
            }
            Protocol::Shadowsocks => {
                if self.method.trim().is_empty() {
                    return Err("не указан метод шифрования".into());
                }
                if self.password.is_empty() {
                    return Err("не указан пароль".into());
                }
            }
            Protocol::Tuic => {
                if self.uuid.trim().is_empty() || self.password.is_empty() {
                    return Err("не указаны UUID и пароль".into());
                }
            }
            Protocol::Masque => {}
            Protocol::OpenVpn => {
                if self.openvpn_profile.trim().is_empty() {
                    return Err("не загружен профиль OpenVPN (.ovpn)".into());
                }
            }
        }
        if self.security == Security::Reality && self.reality_public_key.trim().is_empty() {
            return Err("для Reality не указан публичный ключ (pbk)".into());
        }
        if self.security == Security::Reality
            && !matches!(self.protocol, Protocol::Vless | Protocol::Vmess)
        {
            return Err("Reality поддерживается только для VLESS и VMess".into());
        }
        if capabilities.vless_only && self.protocol != Protocol::Vless {
            return Err("ProtoGrid-core Lite поддерживает только VLESS".into());
        }
        if capabilities.vless_only && self.transport != Transport::Tcp {
            return Err("ProtoGrid-core Lite поддерживает только транспорт TCP".into());
        }
        if self.transport == Transport::Xhttp && core.is_singbox_like() && !capabilities.xhttp {
            return Err(
                "транспорт XHTTP требует sing-box с поддержкой XHTTP (ProtoGrid-core)".into(),
            );
        }
        if self.transport == Transport::Kcp && core.is_singbox_like() && !capabilities.kcp {
            return Err(
                "транспорт mKCP требует sing-box с поддержкой mKCP (ProtoGrid-core)".into(),
            );
        }
        if self.transport == Transport::XDrive && core.is_singbox_like() && !capabilities.xdrive {
            return Err(
                "транспорт XDRIVE требует sing-box с поддержкой XDRIVE (ProtoGrid-core)".into(),
            );
        }
        if self.protocol == Protocol::Masque && core.is_singbox_like() && !capabilities.masque {
            return Err(
                "протокол MASQUE требует sing-box с поддержкой MASQUE (ProtoGrid-core) или Xray"
                    .into(),
            );
        }
        if self.protocol == Protocol::OpenVpn && core.is_singbox_like() && !capabilities.openvpn {
            return Err(
                "протокол OpenVPN требует sing-box 1.14+ с with_openvpn или ProtoGrid-core".into(),
            );
        }
        if self.noise_enabled && core.is_singbox_like() && !capabilities.noise {
            return Err(
                "UDP noise требует sing-box с поддержкой finalmask (ProtoGrid-core) или Xray"
                    .into(),
            );
        }
        if self.transport == Transport::Http && core == CoreKind::Xray {
            return Err(
                "Xray 26+ больше не поддерживает транспорт HTTP/2. Используйте XHTTP или sing-box."
                    .into(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub url: String,
    pub auto_update: bool,
    /// Unix timestamp (seconds) of the last successful refresh.
    pub last_updated: Option<i64>,
    pub userinfo: Option<SubUserInfo>,
    pub last_error: Option<String>,
}

impl Default for Subscription {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: String::new(),
            url: String::new(),
            auto_update: true,
            last_updated: None,
            userinfo: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RoutingRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub action: RuleAction,
    pub kind: RuleKind,
    pub value: String,
}

impl Default for RoutingRule {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: String::new(),
            enabled: true,
            action: RuleAction::Proxy,
            kind: RuleKind::DomainSuffix,
            value: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub core: CoreKind,
    pub tun_enabled: bool,
    pub system_proxy: bool,
    /// sing-box mixed inbound port (HTTP + SOCKS on the same port).
    pub mixed_port: u16,
    /// Xray SOCKS inbound port.
    pub socks_port: u16,
    /// Xray HTTP inbound port; also used for the system proxy.
    pub http_port: u16,
    pub allow_lan: bool,
    pub ipv6: bool,
    pub dns_remote: String,
    pub clash_api_port: u16,
    /// Random per-run password for the local Clash API. Never serialized:
    /// regenerated when the app starts, so other local processes cannot use
    /// the API without knowing it.
    #[serde(skip)]
    pub clash_api_secret: String,
    pub log_level: String,
    pub test_url: String,
    pub final_action: RuleAction,
    /// Routing preset the user picked last. `None` only appears in configs
    /// written before the field existed; `AppConfig::normalize` fills it in.
    pub routing_preset: Option<RoutingPreset>,
    /// Which entry of [`AppConfig::custom_profiles`] is active when
    /// `routing_preset` is `Custom`.
    pub custom_profile_id: Option<String>,
    pub close_to_tray: bool,
    pub auto_connect: bool,
    pub auto_update_subscriptions: bool,
    pub auto_select: bool,
    pub fake_dns: bool,
    pub singbox_path: String,
    pub xray_path: String,
    /// Path to ProtoGrid-core. Kept separate from `singbox_path`.
    pub protogrid_path: String,
    /// Path to ProtoGrid-core Lite. Kept separate from the full core.
    pub protogrid_lite_path: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            core: CoreKind::Protogrid,
            tun_enabled: false,
            system_proxy: true,
            mixed_port: 2080,
            socks_port: 10808,
            http_port: 10809,
            allow_lan: false,
            ipv6: false,
            dns_remote: "https://1.1.1.1/dns-query".into(),
            clash_api_port: 9090,
            clash_api_secret: String::new(),
            log_level: "info".into(),
            test_url: "https://www.gstatic.com/generate_204".into(),
            final_action: RuleAction::Proxy,
            routing_preset: None,
            custom_profile_id: None,
            close_to_tray: true,
            auto_connect: false,
            auto_update_subscriptions: true,
            auto_select: false,
            fake_dns: false,
            singbox_path: String::new(),
            xray_path: String::new(),
            protogrid_path: String::new(),
            protogrid_lite_path: String::new(),
        }
    }
}

impl Settings {
    /// HTTP port that clients (and the system proxy) should use.
    pub fn effective_http_port(&self) -> u16 {
        match self.core {
            CoreKind::Singbox | CoreKind::Protogrid | CoreKind::ProtogridLite => self.mixed_port,
            CoreKind::Xray => self.http_port,
        }
    }

    pub fn effective_socks_port(&self) -> u16 {
        match self.core {
            CoreKind::Singbox | CoreKind::Protogrid | CoreKind::ProtogridLite => self.mixed_port,
            CoreKind::Xray => self.socks_port,
        }
    }

    /// TUN is only usable with sing-box-like cores, and only when turned on.
    pub fn tun_active(&self) -> bool {
        self.tun_enabled && self.core.is_singbox_like()
    }

    pub fn core_path(&self) -> &str {
        match self.core {
            CoreKind::Singbox => &self.singbox_path,
            CoreKind::Xray => &self.xray_path,
            CoreKind::Protogrid => &self.protogrid_path,
            CoreKind::ProtogridLite => &self.protogrid_lite_path,
        }
    }

    pub fn sanitize(&mut self) {
        if self.mixed_port == 0 {
            self.mixed_port = 2080;
        }
        if self.socks_port == 0 {
            self.socks_port = 10808;
        }
        if self.http_port == 0 {
            self.http_port = 10809;
        }
        if self.clash_api_port == 0 {
            self.clash_api_port = 9090;
        }
        if self.dns_remote.trim().is_empty() {
            self.dns_remote = "https://1.1.1.1/dns-query".into();
        }
        if self.test_url.trim().is_empty() {
            self.test_url = "https://www.gstatic.com/generate_204".into();
        }
        let level = self.log_level.to_ascii_lowercase();
        if !matches!(
            level.as_str(),
            "trace" | "debug" | "info" | "warn" | "error"
        ) {
            self.log_level = "info".into();
        } else {
            self.log_level = level;
        }
        self.singbox_path = self.singbox_path.trim().to_string();
        self.xray_path = self.xray_path.trim().to_string();
        self.protogrid_path = self.protogrid_path.trim().to_string();
        self.protogrid_lite_path = self.protogrid_lite_path.trim().to_string();
    }

    /// Fields that require a core restart when changed.
    pub fn network_eq(&self, other: &Settings) -> bool {
        self.core == other.core
            && self.tun_enabled == other.tun_enabled
            && self.mixed_port == other.mixed_port
            && self.socks_port == other.socks_port
            && self.http_port == other.http_port
            && self.allow_lan == other.allow_lan
            && self.ipv6 == other.ipv6
            && self.dns_remote == other.dns_remote
            && self.clash_api_port == other.clash_api_port
            && self.log_level == other.log_level
            && self.singbox_path == other.singbox_path
            && self.xray_path == other.xray_path
            && self.protogrid_path == other.protogrid_path
            && self.protogrid_lite_path == other.protogrid_lite_path
            && self.final_action == other.final_action
            && self.auto_select == other.auto_select
            && self.fake_dns == other.fake_dns
    }
}

/// Rules of a user-created routing profile. Kept aside when a built-in preset
/// is applied so the user can switch back without losing anything.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CustomProfile {
    pub id: String,
    pub name: String,
    pub rules: Vec<RoutingRule>,
    pub final_action: RuleAction,
}

/// Name given to a profile created without an explicit one.
pub const DEFAULT_PROFILE_NAME: &str = "Свой профиль";

impl CustomProfile {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            rules: Vec::new(),
            final_action: RuleAction::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub version: u32,
    pub servers: Vec<Server>,
    pub subscriptions: Vec<Subscription>,
    pub rules: Vec<RoutingRule>,
    /// Single profile from configs written before multi-profile support.
    /// Migrated into [`AppConfig::custom_profiles`] on load.
    pub custom_profile: Option<CustomProfile>,
    /// Routing profiles created by the user.
    pub custom_profiles: Vec<CustomProfile>,
    pub settings: Settings,
    pub active_server_id: Option<String>,
}

/// Portable, credential-free settings snapshot used by "Экспорт настроек".
///
/// It intentionally holds no servers or subscriptions: those contain UUIDs,
/// passwords and subscription tokens. Import replaces only the local
/// preferences and routing profiles, never the user's saved servers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SettingsBackup {
    pub version: u32,
    pub settings: Settings,
    pub rules: Vec<RoutingRule>,
    pub custom_profile: Option<CustomProfile>,
    pub custom_profiles: Vec<CustomProfile>,
}

impl Default for SettingsBackup {
    fn default() -> Self {
        Self {
            version: 1,
            settings: Settings::default(),
            rules: default_rules(),
            custom_profile: None,
            custom_profiles: Vec::new(),
        }
    }
}

impl SettingsBackup {
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            version: 1,
            settings: config.settings.clone(),
            rules: config.rules.clone(),
            custom_profile: config.custom_profile.clone(),
            custom_profiles: config.custom_profiles.clone(),
        }
    }

    pub fn apply_to(&self, config: &mut AppConfig) {
        // The Clash API secret is per-run and must never come from a file.
        let secret = std::mem::take(&mut config.settings.clash_api_secret);
        config.settings = self.settings.clone();
        config.settings.clash_api_secret = secret;
        config.rules = self.rules.clone();
        config.custom_profile = self.custom_profile.clone();
        config.custom_profiles = self.custom_profiles.clone();
        config.normalize();
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            servers: Vec::new(),
            subscriptions: Vec::new(),
            rules: default_rules(),
            custom_profile: None,
            custom_profiles: Vec::new(),
            settings: Settings::default(),
            active_server_id: None,
        }
    }
}

impl AppConfig {
    pub fn normalize(&mut self) {
        self.version = 1;
        self.settings.sanitize();
        for server in &mut self.servers {
            if server.id.trim().is_empty() {
                server.id = uuid::Uuid::new_v4().to_string();
            }
            server.address = server.address.trim().to_string();
            server.country = server.country.trim().to_ascii_uppercase();
            if server.country.len() != 2
                || !server
                    .country
                    .bytes()
                    .all(|byte| byte.is_ascii_alphabetic())
            {
                server.country.clear();
            }
            server.sni = server.sni.trim().to_string();
            server.host = server.host.trim().to_string();
            server.path = server.path.trim().to_string();
            server.service_name = server.service_name.trim().to_string();
            server.flow = server.flow.trim().to_string();
            server.alpn = server
                .alpn
                .iter()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect();
            server.openvpn_username = server.openvpn_username.trim().to_string();
            server.openvpn_password = server.openvpn_password.trim().to_string();
            // Only the outer whitespace is stripped: inline PEM blocks inside
            // the profile must stay byte-for-byte intact.
            server.openvpn_profile = server.openvpn_profile.trim().to_string();
        }
        for sub in &mut self.subscriptions {
            if sub.id.trim().is_empty() {
                sub.id = uuid::Uuid::new_v4().to_string();
            }
            sub.url = sub.url.trim().to_string();
        }
        for rule in &mut self.rules {
            if rule.id.trim().is_empty() {
                rule.id = uuid::Uuid::new_v4().to_string();
            }
        }
        // Older builds shipped "Жизнь в РФ" (formerly "Обход РФ") without the process rules; upgrade the
        // stored copy so the preset keeps matching instead of becoming custom.
        if matches!(
            self.settings.routing_preset,
            None | Some(RoutingPreset::Bypass)
        ) && is_legacy_bypass(&self.rules)
        {
            self.rules = default_rules();
        }
        self.enforce_tun_process_rules();
        self.settings.routing_preset = Some(match self.settings.routing_preset {
            // Keep the explicit choice, but drop it once the rules diverge from
            // the preset (hand edits, imported configs, older builds).
            Some(RoutingPreset::Custom) => RoutingPreset::Custom,
            Some(preset) if preset_matches(preset, &self.rules, self.settings.final_action) => {
                preset
            }
            _ => detect_preset(&self.rules, self.settings.final_action),
        });
        // While a custom profile is active, it *is* the current ruleset.
        self.normalize_custom_profiles();
        self.sync_custom_profile();
        match &self.active_server_id {
            Some(id) if self.servers.iter().any(|s| &s.id == id) => {}
            _ => self.active_server_id = self.servers.first().map(|s| s.id.clone()),
        }
    }

    /// Serialize the portable settings snapshot (no servers, no secrets).
    pub fn export_settings_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&SettingsBackup::from_config(self))
            .map_err(|error| format!("не удалось сериализовать настройки: {error}"))
    }

    /// Parse a settings snapshot without applying it. The version check keeps
    /// future/foreign files from silently replacing the profile.
    pub fn parse_settings_backup(text: &str) -> Result<SettingsBackup, String> {
        let backup: SettingsBackup = serde_json::from_str(text)
            .map_err(|error| format!("это не файл настроек ProtoGrid: {error}"))?;
        if backup.version != 1 {
            return Err(format!(
                "неподдерживаемая версия файла настроек: {}",
                backup.version
            ));
        }
        Ok(backup)
    }

    /// Replace settings and routing profiles. Servers and subscriptions stay
    /// untouched, so importing a settings file cannot overwrite credentials.
    pub fn apply_settings_backup(&mut self, backup: SettingsBackup) {
        backup.apply_to(self);
    }

    pub fn server(&self, id: &str) -> Option<&Server> {
        self.servers.iter().find(|s| s.id == id)
    }

    /// Mirrors the active ruleset into the selected custom profile, so the
    /// profile the user returns to is always the latest saved state.
    pub fn sync_custom_profile(&mut self) {
        if self.settings.routing_preset != Some(RoutingPreset::Custom) {
            return;
        }
        if let Some(id) = self.settings.custom_profile_id.clone() {
            if let Some(profile) = self.custom_profiles.iter_mut().find(|p| p.id == id) {
                profile.rules = self.rules.clone();
                profile.final_action = self.settings.final_action;
                return;
            }
        }
        let mut profile = CustomProfile::new(DEFAULT_PROFILE_NAME);
        profile.rules = self.rules.clone();
        profile.final_action = self.settings.final_action;
        self.settings.custom_profile_id = Some(profile.id.clone());
        self.custom_profiles.push(profile);
    }

    /// Fills in ids and names for profiles loaded from older configs and
    /// migrates the single-profile field into the list.
    fn normalize_custom_profiles(&mut self) {
        if let Some(mut legacy) = self.custom_profile.take() {
            if legacy.id.trim().is_empty() {
                legacy.id = uuid::Uuid::new_v4().to_string();
            }
            if legacy.name.trim().is_empty() {
                legacy.name = DEFAULT_PROFILE_NAME.to_string();
            }
            if !self.custom_profiles.iter().any(|p| p.id == legacy.id) {
                self.custom_profiles.push(legacy);
            }
        }
        for (index, profile) in self.custom_profiles.iter_mut().enumerate() {
            if profile.id.trim().is_empty() {
                profile.id = uuid::Uuid::new_v4().to_string();
            }
            profile.name = profile.name.trim().to_string();
            if profile.name.is_empty() {
                profile.name = format!("{DEFAULT_PROFILE_NAME} {}", index + 1);
            }
            for rule in &mut profile.rules {
                if rule.id.trim().is_empty() {
                    rule.id = uuid::Uuid::new_v4().to_string();
                }
            }
        }
        if self.settings.routing_preset == Some(RoutingPreset::Custom) {
            let known = self
                .settings
                .custom_profile_id
                .as_deref()
                .map(|id| self.custom_profiles.iter().any(|p| p.id == id))
                .unwrap_or(false);
            if !known {
                self.settings.custom_profile_id =
                    self.custom_profiles.first().map(|p| p.id.clone());
            }
        }
    }

    pub fn active_custom_profile(&self) -> Option<&CustomProfile> {
        if self.settings.routing_preset != Some(RoutingPreset::Custom) {
            return None;
        }
        let id = self.settings.custom_profile_id.as_deref()?;
        self.custom_profiles.iter().find(|p| p.id == id)
    }

    /// Switches to a stored custom profile and makes its rules current.
    pub fn activate_custom_profile(&mut self, id: &str) -> bool {
        let Some(profile) = self.custom_profiles.iter().find(|p| p.id == id).cloned() else {
            return false;
        };
        self.rules = profile.rules;
        self.settings.final_action = profile.final_action;
        self.settings.routing_preset = Some(RoutingPreset::Custom);
        self.settings.custom_profile_id = Some(profile.id);
        self.enforce_tun_process_rules();
        self.sync_custom_profile();
        true
    }

    /// Process rules cannot work without TUN (Discord, Telegram Desktop and
    /// friends ignore the system proxy). They follow the TUN state: off when it
    /// is disabled or unsupported, on for the built-in "Жизнь в РФ" profile when
    /// TUN is available. A custom profile keeps whatever the user chose.
    pub fn enforce_tun_process_rules(&mut self) {
        let tun = self.settings.tun_active();
        let preset = self.settings.routing_preset == Some(RoutingPreset::Bypass);
        for rule in self.rules.iter_mut() {
            if rule.kind == RuleKind::ProcessName {
                rule.enabled = tun && (preset || rule.enabled);
            }
        }
        if !tun {
            for profile in &mut self.custom_profiles {
                disable_process_rules(&mut profile.rules);
            }
        }
    }

    pub fn active_server(&self) -> Option<&Server> {
        self.active_server_id
            .as_deref()
            .and_then(|id| self.server(id))
    }
}

/// Официальные диапазоны Cloudflare (https://www.cloudflare.com/ips/): IPv4 + IPv6.
/// Сайты за CF из РФ режутся после ~19 КБ (см. AGENTS.md §11.1), поэтому их адреса
/// уходят в прокси раньше geoip/geosite-правил.
const CLOUDFLARE_CIDRS: &str = "173.245.48.0/20, 103.21.244.0/22, 103.22.200.0/22, 103.31.4.0/22, 141.101.64.0/18, 108.162.192.0/18, 190.93.240.0/20, 188.114.96.0/20, 197.234.240.0/22, 198.41.128.0/17, 162.158.0.0/15, 104.16.0.0/13, 104.24.0.0/14, 172.64.0.0/13, 131.0.72.0/22, 2400:cb00::/32, 2606:4700::/32, 2803:f800::/32, 2405:b500::/32, 2405:8100::/32, 2a06:98c0::/29, 2c0f:f248::/32";
const CLOUDFLARE_RULE_ID: &str = "default-cloudflare";
const FORCED_PROXY_RULE_ID: &str = "default-forced-proxy";
/// Domains that must go through the proxy despite being `.ru` and matching the
/// RU GeoIP/GeoSite/direct rules. Kept first so the exception always wins.
const FORCED_PROXY_DOMAINS: &str = "web.telegram.ru, instagram.ru";

pub fn default_rules() -> Vec<RoutingRule> {
    vec![
        RoutingRule {
            id: FORCED_PROXY_RULE_ID.into(),
            name: "Принудительно через прокси".into(),
            enabled: true,
            action: RuleAction::Proxy,
            kind: RuleKind::Domain,
            value: FORCED_PROXY_DOMAINS.into(),
        },
        RoutingRule {
            id: "default-private-ip".into(),
            name: "Локальная сеть".into(),
            enabled: true,
            action: RuleAction::Direct,
            kind: RuleKind::IpCidr,
            value: "127.0.0.0/8, 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16, ::1/128, fc00::/7, fe80::/10".into(),
        },
        // Пользовательские сайты за Cloudflare из РФ часто рвутся после ~19 КБ
        // (отсечка CF в РФ), поэтому CF-адреса уводим в прокси до geoip/geosite.
        RoutingRule {
            id: CLOUDFLARE_RULE_ID.into(),
            name: "Cloudflare через прокси".into(),
            enabled: true,
            action: RuleAction::Proxy,
            kind: RuleKind::IpCidr,
            value: CLOUDFLARE_CIDRS.into(),
        },
        RoutingRule {
            id: "default-local-domains".into(),
            name: "Локальные домены".into(),
            enabled: true,
            action: RuleAction::Direct,
            kind: RuleKind::DomainSuffix,
            value: "local, localhost, lan, home.arpa".into(),
        },
        RoutingRule {
            id: "default-ads".into(),
            name: "Блокировка рекламы".into(),
            enabled: true,
            action: RuleAction::Block,
            kind: RuleKind::Geosite,
            value: "category-ads-all".into(),
        },
        RoutingRule {
            id: "default-ru-geoip".into(),
            name: "Жизнь в РФ (IP)".into(),
            enabled: true,
            action: RuleAction::Direct,
            kind: RuleKind::Geoip,
            value: "ru".into(),
        },
        RoutingRule {
            id: "default-ru-geosite".into(),
            name: "Жизнь в РФ (домены)".into(),
            enabled: true,
            action: RuleAction::Direct,
            kind: RuleKind::Geosite,
            value: "category-ru".into(),
        },
        // Process rules only take effect with TUN: Discord and Telegram Desktop
        // ignore the system proxy, so without TUN these connections never reach
        // the core. They stay disabled until the user turns TUN on.
        RoutingRule {
            id: "default-discord".into(),
            name: "Discord через прокси (нужен TUN)".into(),
            enabled: true,
            action: RuleAction::Proxy,
            kind: RuleKind::ProcessName,
            value: "Discord.exe".into(),
        },
        RoutingRule {
            id: "default-telegram".into(),
            name: "Telegram через прокси (нужен TUN)".into(),
            enabled: true,
            action: RuleAction::Proxy,
            kind: RuleKind::ProcessName,
            value: "Telegram.exe".into(),
        },
    ]
}

pub fn routing_preset(name: &str) -> Option<(Vec<RoutingRule>, RuleAction)> {
    match name {
        // The UI calls the smart-bypass preset "bypass"; older callers use "smart".
        "smart" | "bypass" => Some((default_rules(), RuleAction::Proxy)),
        "global" => Some((
            vec![RoutingRule {
                id: "preset-private-ip".into(),
                name: "Локальная сеть".into(),
                enabled: true,
                action: RuleAction::Direct,
                kind: RuleKind::IpCidr,
                value: "127.0.0.0/8, 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16, ::1/128, fc00::/7, fe80::/10".into(),
            }],
            RuleAction::Proxy,
        )),
        "direct" => Some((Vec::new(), RuleAction::Direct)),
        _ => None,
    }
}

/// Whether `rules` + `final_action` are exactly what `preset` would install.
pub fn preset_matches(
    preset: RoutingPreset,
    rules: &[RoutingRule],
    final_action: RuleAction,
) -> bool {
    let Some((canonical, action)) = routing_preset(preset.id()) else {
        return false;
    };
    action == final_action && same_rules(rules, &canonical)
}

/// Best guess for a config that does not record its preset (older profiles).
pub fn detect_preset(rules: &[RoutingRule], final_action: RuleAction) -> RoutingPreset {
    [
        RoutingPreset::Direct,
        RoutingPreset::Global,
        RoutingPreset::Bypass,
    ]
    .into_iter()
    .find(|preset| preset_matches(*preset, rules, final_action))
    .unwrap_or(RoutingPreset::Custom)
}

/// Forces every process rule off. Used when TUN is not active.
fn disable_process_rules(rules: &mut [RoutingRule]) {
    for rule in rules.iter_mut() {
        if rule.kind == RuleKind::ProcessName {
            rule.enabled = false;
        }
    }
}

/// The bypass ruleset as shipped by older builds. Besides the process-rule and
/// Cloudflare migrations, `web.telegram.ru`/`instagram.ru` were added later.
fn is_legacy_bypass(rules: &[RoutingRule]) -> bool {
    let canonical = default_rules();
    for drop_forced in [false, true] {
        for drop_cloudflare in [false, true] {
            for drop_process in [false, true] {
                if !drop_forced && !drop_cloudflare && !drop_process {
                    continue;
                }
                let variant: Vec<RoutingRule> = canonical
                    .iter()
                    .filter(|rule| {
                        !(drop_forced && rule.id == FORCED_PROXY_RULE_ID)
                            && !(drop_cloudflare && rule.id == CLOUDFLARE_RULE_ID)
                            && !(drop_process && rule.kind == RuleKind::ProcessName)
                    })
                    .cloned()
                    .collect();
                if same_rules(rules, &variant) {
                    return true;
                }
            }
        }
    }
    false
}

/// Ids and names are cosmetic, so only the parts that change routing count.
/// Process rules follow the TUN switch instead of being edited by hand, so
/// their `enabled` flag is not part of the identity.
fn same_rules(left: &[RoutingRule], right: &[RoutingRule]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(a, b)| {
            (a.enabled == b.enabled || a.kind == RuleKind::ProcessName)
                && a.action == b.action
                && a.kind == b.kind
                && a.value.trim() == b.value.trim()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing_preset_accepts_ui_ids() {
        for id in ["bypass", "smart", "global", "direct"] {
            assert!(routing_preset(id).is_some(), "preset {id} must resolve");
        }
        assert_eq!(routing_preset("bypass").unwrap().1, RuleAction::Proxy);
        assert_eq!(routing_preset("direct").unwrap().0.len(), 0);
        assert!(routing_preset("unknown").is_none());
        assert!(RoutingPreset::Bypass.is_builtin());
        assert!(!RoutingPreset::Custom.is_builtin());
        for id in ["bypass", "global", "direct", "custom"] {
            assert_eq!(RoutingPreset::from_id(id).map(RoutingPreset::id), Some(id));
        }
    }

    #[test]
    fn detect_preset_matches_installed_rules() {
        let rules = default_rules();
        assert_eq!(
            detect_preset(&rules, RuleAction::Proxy),
            RoutingPreset::Bypass
        );
        let (global, action) = routing_preset("global").unwrap();
        assert_eq!(detect_preset(&global, action), RoutingPreset::Global);
        assert_eq!(
            detect_preset(&[], RuleAction::Direct),
            RoutingPreset::Direct
        );
        assert_eq!(
            detect_preset(&rules, RuleAction::Direct),
            RoutingPreset::Custom
        );
    }

    #[test]
    fn edited_rules_drop_the_stored_preset() {
        let (global_rules, action) = routing_preset("global").unwrap();
        let mut config = AppConfig::default();
        config.rules = global_rules;
        config.settings.final_action = action;
        config.settings.routing_preset = Some(RoutingPreset::Global);
        config.normalize();
        assert_eq!(config.settings.routing_preset, Some(RoutingPreset::Global));

        config.rules.pop();
        config.normalize();
        assert_eq!(
            config.settings.routing_preset,
            Some(RoutingPreset::Custom),
            "rules that no longer match a preset must not keep its name"
        );

        config.settings.routing_preset = Some(RoutingPreset::Custom);
        config.normalize();
        assert_eq!(
            config.settings.routing_preset,
            Some(RoutingPreset::Custom),
            "an explicit custom choice must survive normalization"
        );
    }

    #[test]
    fn old_bypass_profile_is_upgraded() {
        let mut config = AppConfig::default();
        config
            .rules
            .retain(|rule| rule.kind != RuleKind::ProcessName && rule.id != CLOUDFLARE_RULE_ID);
        config.settings.routing_preset = Some(RoutingPreset::Bypass);
        config.normalize();
        assert_eq!(
            config.settings.routing_preset,
            Some(RoutingPreset::Bypass),
            "an old bypass config must not look like a custom profile"
        );
        assert_eq!(config.rules.len(), default_rules().len());
    }

    #[test]
    fn cloudflare_rule_sits_before_ru_rules() {
        let rules = default_rules();
        let cf = rules
            .iter()
            .position(|rule| rule.id == CLOUDFLARE_RULE_ID)
            .expect("в пресете должно быть правило Cloudflare");
        let ru = rules
            .iter()
            .position(|rule| rule.id == "default-ru-geoip")
            .expect("GeoIP ru");
        assert!(cf < ru, "Cloudflare должен проверяться раньше RU-правил");
        assert_eq!(rules[cf].action, RuleAction::Proxy);
        assert_eq!(rules[cf].kind, RuleKind::IpCidr);
        assert!(rules[cf].value.contains("104.16.0.0/13"));
        assert!(rules[cf].value.contains("2606:4700::/32"));
    }

    #[test]
    fn preset_rule_names_are_filled() {
        let mut presets = vec![default_rules()];
        presets.push(routing_preset("global").unwrap().0);
        for rules in presets {
            for rule in rules {
                assert!(
                    !rule.name.trim().is_empty(),
                    "у правила {} нет названия",
                    rule.id
                );
            }
        }
    }

    #[test]
    fn previous_bypass_profile_gains_forced_proxy_rule() {
        let mut config = AppConfig::default();
        config.rules.retain(|rule| rule.id != FORCED_PROXY_RULE_ID);
        config.settings.routing_preset = Some(RoutingPreset::Bypass);
        config.normalize();
        assert_eq!(
            config.settings.routing_preset,
            Some(RoutingPreset::Bypass),
            "пресет без новых принудительных доменов должен обновиться"
        );
        assert!(config.rules.iter().any(|rule| {
            rule.id == FORCED_PROXY_RULE_ID
                && rule.action == RuleAction::Proxy
                && rule.value.contains("web.telegram.ru")
                && rule.value.contains("instagram.ru")
        }));
    }

    #[test]
    fn previous_bypass_profile_gains_cloudflare_rule() {
        let mut config = AppConfig::default();
        config.rules.retain(|rule| rule.id != CLOUDFLARE_RULE_ID);
        config.settings.routing_preset = Some(RoutingPreset::Bypass);
        config.normalize();
        assert_eq!(
            config.settings.routing_preset,
            Some(RoutingPreset::Bypass),
            "прошлый пресет должен остаться пресетом, а не стать custom"
        );
        assert!(config
            .rules
            .iter()
            .any(|rule| rule.id == CLOUDFLARE_RULE_ID));
    }

    #[test]
    fn process_rules_require_tun() {
        let mut config = AppConfig::default();
        // TUN is off by default, so the shipped process rules are off.
        config.normalize();
        assert_eq!(config.settings.routing_preset, Some(RoutingPreset::Bypass));
        assert!(config
            .rules
            .iter()
            .filter(|rule| rule.kind == RuleKind::ProcessName)
            .all(|rule| !rule.enabled));

        // With TUN on (sing-box-like core) the built-in profile enables them.
        config.settings.tun_enabled = true;
        config.normalize();
        assert!(config
            .rules
            .iter()
            .filter(|rule| rule.kind == RuleKind::ProcessName)
            .all(|rule| rule.enabled));

        // And forces them off again when TUN is unavailable.
        config.settings.tun_enabled = false;
        config.normalize();
        assert!(config
            .rules
            .iter()
            .filter(|rule| rule.kind == RuleKind::ProcessName)
            .all(|rule| !rule.enabled));

        // A custom profile keeps the user's own choice.
        config.settings.tun_enabled = true;
        config.settings.routing_preset = Some(RoutingPreset::Custom);
        config.normalize();
        assert!(config
            .rules
            .iter()
            .filter(|rule| rule.kind == RuleKind::ProcessName)
            .all(|rule| !rule.enabled));
    }

    #[test]
    fn tun_needs_singbox_core() {
        let mut settings = Settings::default();
        settings.tun_enabled = true;
        assert!(settings.tun_active());
        settings.core = CoreKind::Xray;
        assert!(!settings.tun_active());
    }

    #[test]
    fn custom_profile_survives_serialization() {
        let mut config = AppConfig::default();
        let mut profile = CustomProfile::new("Работа");
        profile.rules = default_rules();
        profile.final_action = RuleAction::Proxy;
        config.custom_profiles.push(profile);
        config.settings.routing_preset = Some(RoutingPreset::Global);
        let json = serde_json::to_string(&config).unwrap();
        let restored: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(
            restored.settings.routing_preset,
            Some(RoutingPreset::Global)
        );
        assert_eq!(restored.custom_profiles.len(), 1);
        assert_eq!(restored.custom_profiles[0].name, "Работа");
        assert_eq!(
            restored.custom_profiles[0].rules.len(),
            default_rules().len()
        );
    }

    #[test]
    fn own_profile_survives_preset_switches() {
        // A legacy config whose rules no longer match a preset: the own profile
        // must be captured so the user can return to it.
        let mut config = AppConfig::default();
        config.rules.pop();
        config.normalize();
        assert_eq!(config.settings.routing_preset, Some(RoutingPreset::Custom));
        assert_eq!(config.custom_profiles.len(), 1);
        let saved = config.custom_profiles[0].clone();
        assert_eq!(saved.rules.len(), default_rules().len() - 1);

        // A built-in preset never overwrites the saved own profile.
        config.settings.routing_preset = Some(RoutingPreset::Global);
        config.rules = routing_preset("global").unwrap().0;
        config.normalize();
        assert_eq!(config.settings.routing_preset, Some(RoutingPreset::Global));
        assert_eq!(
            config.custom_profiles[0].rules.len(),
            default_rules().len() - 1
        );
    }

    #[test]
    fn multiple_profiles_switch_and_keep_their_rules() {
        let mut config = AppConfig::default();
        let mut work = CustomProfile::new("Работа");
        work.rules = routing_preset("global").unwrap().0;
        let mut home = CustomProfile::new("Дом");
        home.rules = Vec::new();
        home.final_action = RuleAction::Direct;
        let home_id = home.id.clone();
        config.custom_profiles = vec![work, home];
        config.settings.routing_preset = Some(RoutingPreset::Custom);
        config.settings.custom_profile_id = Some(home_id.clone());
        config.rules = config.custom_profiles[1].rules.clone();
        config.settings.final_action = RuleAction::Direct;
        config.normalize();
        assert!(config.rules.is_empty());

        assert!(config.activate_custom_profile(&home_id));
        assert!(config.rules.is_empty());
        assert_eq!(config.settings.final_action, RuleAction::Direct);
        let global = config.custom_profiles[0].id.clone();
        assert!(config.activate_custom_profile(&global));
        assert_eq!(config.rules.len(), 1);
        assert_eq!(config.settings.final_action, RuleAction::Proxy);
        // Both profiles are still stored.
        assert_eq!(config.custom_profiles.len(), 2);
    }

    #[test]
    fn legacy_single_profile_is_migrated_to_the_list() {
        let mut config = AppConfig::default();
        config.custom_profile = Some(CustomProfile {
            rules: default_rules(),
            final_action: RuleAction::Proxy,
            ..Default::default()
        });
        config.settings.routing_preset = Some(RoutingPreset::Custom);
        config.normalize();
        assert!(config.custom_profile.is_none());
        assert_eq!(config.custom_profiles.len(), 1);
        let profile = &config.custom_profiles[0];
        assert!(!profile.id.is_empty());
        assert_eq!(profile.name, DEFAULT_PROFILE_NAME);
        assert_eq!(
            config.settings.custom_profile_id.as_deref(),
            Some(profile.id.as_str())
        );
    }

    #[test]
    fn settings_backup_omits_secrets_and_keeps_servers() {
        let mut config = AppConfig::default();
        config.settings.mixed_port = 3456;
        config.settings.clash_api_secret = "per-run-secret".into();
        config.servers.push(Server {
            uuid: "credential-uuid".into(),
            password: "credential-password".into(),
            ..Default::default()
        });

        let json = config.export_settings_json().unwrap();
        assert!(!json.contains("per-run-secret"));
        assert!(!json.contains("credential-uuid"));
        assert!(!json.contains("credential-password"));

        let backup = AppConfig::parse_settings_backup(&json).unwrap();
        let mut restored = AppConfig {
            servers: config.servers.clone(),
            settings: Settings {
                clash_api_secret: "runtime-secret".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        restored.apply_settings_backup(backup);

        assert_eq!(restored.settings.mixed_port, 3456);
        assert_eq!(restored.settings.clash_api_secret, "runtime-secret");
        assert_eq!(restored.servers.len(), 1);
        assert_eq!(restored.servers[0].uuid, "credential-uuid");
    }

    #[test]
    fn settings_backup_rejects_other_versions() {
        let mut value = serde_json::to_value(SettingsBackup::default()).unwrap();
        value["version"] = serde_json::json!(2);
        let error = AppConfig::parse_settings_backup(&value.to_string()).unwrap_err();
        assert!(error.contains('2'), "unexpected error: {error}");
    }

    #[test]
    fn legacy_config_gets_preset_from_its_rules() {
        let mut config = AppConfig::default();
        config.rules.clear();
        config.settings.final_action = RuleAction::Direct;
        // What an old config.json looks like: no `routing_preset` field at all.
        let json = serde_json::to_value(&config).unwrap();
        let mut json = json;
        json["settings"]
            .as_object_mut()
            .unwrap()
            .remove("routing_preset");
        let mut restored: AppConfig = serde_json::from_value(json).unwrap();
        assert_eq!(restored.settings.routing_preset, None);
        restored.normalize();
        assert_eq!(
            restored.settings.routing_preset,
            Some(RoutingPreset::Direct)
        );
    }
}
