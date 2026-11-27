use std::fs::File;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use protogrid_core::model::{
    CoreCapabilities, CoreKind, Protocol, Security, Server, Settings, Transport,
};
use protogrid_core::util::version_at_least;
use protogrid_core::{singbox, xray};
use tokio::process::Command;

use crate::engine::{push_log, Inner};

/// Features of a concrete core binary, verified by running `check` on tiny
/// probe configs. Nothing here is guessed from the core's name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CoreFeatures {
    pub xhttp: bool,
    pub kcp: bool,
    pub xdrive: bool,
    pub masque: bool,
    pub mldsa65: bool,
    pub vless_encryption: bool,
    pub tun: bool,
    pub fakedns: bool,
    pub clash_api: bool,
    pub noise: bool,
    pub openvpn: bool,
}

impl CoreFeatures {
    /// Fallback used before (or instead of) probing: stock sing-box vs fork
    /// is distinguishable by the version string, Xray by its kind.
    pub fn heuristic(kind: CoreKind, version: &str) -> Self {
        let fork = version.contains("-protogrid");
        match kind {
            CoreKind::Xray => Self {
                xhttp: true,
                kcp: true,
                xdrive: true,
                masque: true,
                mldsa65: true,
                vless_encryption: true,
                tun: false,
                fakedns: false,
                clash_api: false,
                noise: true,
                openvpn: false,
            },
            CoreKind::ProtogridLite => Self {
                xhttp: false,
                kcp: false,
                xdrive: false,
                masque: false,
                mldsa65: false,
                vless_encryption: false,
                tun: true,
                fakedns: true,
                clash_api: true,
                noise: false,
                openvpn: false,
            },
            CoreKind::Singbox | CoreKind::Protogrid => Self {
                xhttp: fork,
                kcp: fork,
                xdrive: fork,
                masque: fork,
                mldsa65: fork,
                vless_encryption: fork,
                tun: true,
                fakedns: true,
                clash_api: true,
                noise: fork,
                openvpn: version_at_least(version, "1.14.0"),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct CoreInfo {
    pub found: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub compatible: bool,
    pub error: Option<String>,
    pub features: CoreFeatures,
}

impl CoreInfo {
    pub fn supports_xhttp(&self) -> bool {
        self.features.xhttp
    }

    pub fn supports_kcp(&self) -> bool {
        self.features.kcp
    }

    pub fn supports_xdrive(&self) -> bool {
        self.features.xdrive
    }

    /// REALITY ML-DSA-65 verification is only emitted by the ProtoGrid fork.
    pub fn supports_mldsa65(&self) -> bool {
        self.features.mldsa65
    }

    /// MASQUE is implemented by the ProtoGrid fork (and Xray).
    pub fn supports_masque(&self) -> bool {
        self.features.masque
    }

    /// UDP finalmask noise is implemented by the ProtoGrid fork (and Xray).
    pub fn supports_noise(&self) -> bool {
        self.features.noise
    }

    /// OpenVPN client endpoint: sing-box 1.14+ with `with_openvpn` (the
    /// ProtoGrid fork always includes it).
    pub fn supports_openvpn(&self) -> bool {
        self.features.openvpn
    }
}

#[derive(Debug, Clone, Copy)]
enum Probe {
    Xhttp,
    Kcp,
    XDrive,
    Masque,
    Mldsa65,
    VlessEncryption,
    Noise,
    OpenVpn,
}

const PROBES: [Probe; 8] = [
    Probe::Xhttp,
    Probe::Kcp,
    Probe::XDrive,
    Probe::Masque,
    Probe::Mldsa65,
    Probe::VlessEncryption,
    Probe::Noise,
    Probe::OpenVpn,
];

/// Minimal TLS OpenVPN profile that passes `sing-box check`. The CA is a
/// throwaway self-signed certificate used only to validate the probe config;
/// nothing is ever connected to.
const OPENVPN_PROBE_PROFILE: &str = r#"client
dev tun
proto udp
remote example.com 1194
remote-cert-tls server
verify-x509-name example.com name
<ca>
-----BEGIN CERTIFICATE-----
MIIDGzCCAgOgAwIBAgIUfHZfjhg3fZaCE2ZIIyuUqi7jTR4wDQYJKoZIhvcNAQEL
BQAwHTEbMBkGA1UEAwwSUHJvdG9HcmlkIFByb2JlIENBMB4XDTI2MTEyNjAyNTEx
N1oXDTM2MTEyMzAyNTExN1owHTEbMBkGA1UEAwwSUHJvdG9HcmlkIFByb2JlIENB
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAzyBPZnKSQgXrpggPpHdP
nsfRUaQbz7Lk1oXViZYEiGA+bLqR6a5qpk/QkIz7EfmwV40H6IX49fW/8sGnqFQI
rYLyXPlLzsKb2R5QRECzuT4chNuUCCpNHKbOwKyMt5FCmwRCzAJWZTTdysAMaWdS
ppB0mYcYRLnd9Xo4epAlcExePKIwIyZSWKe4Wi0rMGhLqDwYofj4AB/rzEs1r/R1
EZTQaPxsXrdC2eSY7Ufwx2HW8f7zfle+OSqxM13CdO5u/3MkkVaAo0d1BeJwbtSd
mCSyD5+AhsjBJP162WqAj6PK6dDHDb+bmEVYwDcxSI1+1hlt9D8w2RZTOttbhzPB
jQIDAQABo1MwUTAdBgNVHQ4EFgQUAY3NO4C+yeURWBfkrR/XDAMCJAcwHwYDVR0j
BBgwFoAUAY3NO4C+yeURWBfkrR/XDAMCJAcwDwYDVR0TAQH/BAUwAwEB/zANBgkq
hkiG9w0BAQsFAAOCAQEAVmBbtSb82FjEtPixyWJfA9ERo5lM7IpOLnsH6f6dJP/P
6lkO7FXdku4tbEWeAFF2N2nmT2+2F+eMjrBwbWZkWZH9z51V/P5haE0cT1HDnS2q
myp039ugoFC9CJwUS2y5JiYtGWqj1msvMa5CWANAfZMcdYQJeyYnWV/DLQ9A9QF0
PnXDlr/hnjjiflgoZxVHWghEJqYy1teVFFM8T4RXU6R502rGO/c0Inn3i8rWsOq4
9TJ+WLFW6ew1sSWwcodG4OZ31LZqdjjjgf/yybbNKiUUzqRgINojJuOjw74snexG
KAH2g66dmzml0toPPGfCRl0RVjdmfr08uQ1HN0MwPg==
-----END CERTIFICATE-----
</ca>
"#;

fn probe_server(protocol: Protocol) -> Server {
    let mut server = Server::new(protocol);
    server.id = "probe".into();
    server.name = "probe".into();
    server.address = "example.com".into();
    server.port = 443;
    server.uuid = "11111111-1111-1111-1111-111111111111".into();
    server
}

fn probe_config(kind: CoreKind, probe: Probe) -> Option<serde_json::Value> {
    let mut settings = Settings::default();
    settings.auto_select = false;
    let mut capabilities = CoreCapabilities::default();
    let mut server = probe_server(Protocol::Vless);
    match probe {
        Probe::Xhttp => {
            server.transport = Transport::Xhttp;
            server.security = Security::Tls;
            capabilities.xhttp = true;
        }
        Probe::Kcp => {
            server.transport = Transport::Kcp;
            // Xray 26 refuses plain VLESS without TLS or VLESS encryption.
            server.security = Security::Tls;
            capabilities.kcp = true;
        }
        Probe::XDrive => {
            server.transport = Transport::XDrive;
            server.security = Security::Tls;
            server.encryption = format!("mlkem768x25519plus.native.0rtt.{}", "A".repeat(43));
            capabilities.xdrive = true;
        }
        Probe::Masque => {
            server = probe_server(Protocol::Masque);
            server.security = Security::Tls;
            capabilities.masque = true;
        }
        Probe::Mldsa65 => {
            server.security = Security::Reality;
            server.reality_public_key = "jNXHt1yRo0vDuchQlIP6Z0ZvjT3KtzVI-T4E7RoLJS0".into();
            server.reality_short_id = "0123".into();
            // 1952-byte ML-DSA-65 public key, raw-url base64.
            server.reality_mldsa65_verify = "A".repeat(2603);
            capabilities.mldsa65 = true;
        }
        Probe::VlessEncryption => {
            server.encryption = format!("mlkem768x25519plus.native.0rtt.{}", "A".repeat(43));
        }
        Probe::Noise => {
            server.security = Security::Tls;
            server.noise_enabled = true;
            server.noise_packet = "abcd".into();
            capabilities.noise = true;
        }
        Probe::OpenVpn => {
            server = probe_server(Protocol::OpenVpn);
            server.openvpn_profile = OPENVPN_PROBE_PROFILE.into();
            server.security = Security::Tls;
            capabilities.openvpn = true;
        }
    }
    if kind == CoreKind::Xray {
        xray::build_config(&[server], "probe", &settings, &[]).ok()
    } else {
        singbox::build_config_with_capabilities(
            &[server],
            "probe",
            &settings,
            &[],
            None,
            None,
            capabilities,
        )
        .ok()
    }
}

async fn run_check(kind: CoreKind, path: &Path, config: &Path) -> bool {
    let mut command = Command::new(path);
    match kind {
        CoreKind::Xray => {
            command.arg("run").arg("-test").arg("-c").arg(config);
            if let Some(parent) = path.parent() {
                command.env("XRAY_LOCATION_ASSET", parent);
            }
        }
        _ => {
            command.arg("check").arg("-c").arg(config);
        }
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.as_std_mut().creation_flags(0x08000000);
    }
    matches!(
        tokio::time::timeout(Duration::from_secs(6), command.status()).await,
        Ok(Ok(status)) if status.success()
    )
}

/// Ask the actual binary which optional features it understands. Every probe
/// is a complete config validated by the core itself, so a fork rebuild or a
/// newer upstream release changes the badges without code changes.
async fn probe_features(kind: CoreKind, path: &Path, version: &str) -> CoreFeatures {
    let mut features = CoreFeatures::heuristic(kind, version);
    // The Lite build is a fixed feature set; probing its absent features only
    // costs eight extra process launches on every core detection.
    if kind == CoreKind::ProtogridLite {
        return features;
    }
    let dir = std::env::temp_dir().join(format!("protogrid-probe-{}", std::process::id()));
    if std::fs::create_dir_all(&dir).is_err() {
        return features;
    }
    let baseline = dir.join("baseline.json");
    let mut plain = probe_server(Protocol::Vless);
    plain.security = Security::Tls;
    let settings = Settings::default();
    let baseline_value = if kind == CoreKind::Xray {
        xray::build_config(&[plain], "probe", &settings, &[]).ok()
    } else {
        singbox::build_config(&[plain], "probe", &settings, &[], None, None).ok()
    };
    let baseline_ok = match baseline_value {
        Some(value) => match serde_json::to_vec(&value) {
            Ok(bytes) => {
                std::fs::write(&baseline, bytes).is_ok() && run_check(kind, path, &baseline).await
            }
            Err(_) => false,
        },
        None => false,
    };
    if !baseline_ok {
        let _ = std::fs::remove_dir_all(&dir);
        return features;
    }
    for probe in PROBES {
        let Some(value) = probe_config(kind, probe) else {
            continue;
        };
        let Ok(bytes) = serde_json::to_vec(&value) else {
            continue;
        };
        let name = match probe {
            Probe::Xhttp => "xhttp",
            Probe::Kcp => "kcp",
            Probe::XDrive => "xdrive",
            Probe::Masque => "masque",
            Probe::Mldsa65 => "mldsa65",
            Probe::VlessEncryption => "vless-encryption",
            Probe::Noise => "noise",
            Probe::OpenVpn => "openvpn",
        };
        let file = dir.join(format!("{name}.json"));
        if std::fs::write(&file, bytes).is_err() {
            continue;
        }
        let supported = run_check(kind, path, &file).await;
        match probe {
            Probe::Xhttp => features.xhttp = supported,
            Probe::Kcp => features.kcp = supported,
            Probe::XDrive => features.xdrive = supported,
            Probe::Masque => features.masque = supported,
            Probe::Mldsa65 => features.mldsa65 = supported,
            Probe::VlessEncryption => {
                // Stock sing-box may accept the field without implementing it.
                features.vless_encryption =
                    supported && (kind == CoreKind::Xray || version.contains("-protogrid"));
            }
            Probe::Noise => features.noise = supported,
            Probe::OpenVpn => features.openvpn = supported,
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    features
}

fn binary_name(kind: CoreKind) -> &'static str {
    match kind {
        CoreKind::Singbox => {
            if cfg!(windows) {
                "sing-box.exe"
            } else {
                "sing-box"
            }
        }
        CoreKind::Xray => {
            if cfg!(windows) {
                "xray.exe"
            } else {
                "xray"
            }
        }
        // The fork ships next to the launcher under its own name so it cannot
        // be mistaken for a stock sing-box lookup.
        CoreKind::Protogrid => {
            if cfg!(windows) {
                "protogrid-core.exe"
            } else {
                "protogrid-core"
            }
        }
        CoreKind::ProtogridLite => {
            if cfg!(windows) {
                "protogrid-core-lite.exe"
            } else {
                "protogrid-core-lite"
            }
        }
    }
}

fn dir_name(kind: CoreKind) -> &'static str {
    match kind {
        CoreKind::Singbox => "sing-box",
        CoreKind::Xray => "xray",
        CoreKind::Protogrid => "protogrid",
        CoreKind::ProtogridLite => "protogrid-lite",
    }
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn configure_path(kind: CoreKind, settings: &Settings) -> String {
    match kind {
        CoreKind::Singbox => settings.singbox_path.clone(),
        CoreKind::Xray => settings.xray_path.clone(),
        CoreKind::Protogrid => settings.protogrid_path.clone(),
        CoreKind::ProtogridLite => settings.protogrid_lite_path.clone(),
    }
}

fn legacy_protogrid_name() -> &'static str {
    if cfg!(windows) {
        "sing-box.exe"
    } else {
        "sing-box"
    }
}

fn candidate_paths(kind: CoreKind, settings: &Settings, data_dir: &Path) -> Vec<PathBuf> {
    let binary = binary_name(kind);
    let dir = dir_name(kind);
    let mut candidates = Vec::new();
    let configured = {
        let raw = configure_path(kind, settings);
        if raw.trim().is_empty() {
            None
        } else {
            let mut path = expand_tilde(raw.trim());
            if path.is_dir() {
                path = path.join(binary);
            }
            if !path.is_absolute() {
                path = data_dir.join(path);
            }
            Some(path)
        }
    };

    if matches!(kind, CoreKind::Protogrid | CoreKind::ProtogridLite) {
        let managed_binary = data_dir.join("cores").join(dir).join(binary);
        let managed_legacy = data_dir
            .join("cores")
            .join(dir)
            .join(legacy_protogrid_name());
        if let Some(path) = configured {
            // Older releases copied the fork into the profile and saved that
            // path. Ignore it so the fresh core next to the launcher wins.
            if path != managed_binary && path != managed_legacy {
                candidates.push(path);
            }
        }
        // Bundled cores ship next to the launcher rather than inside it.
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                candidates.push(parent.join(binary));
                candidates.push(parent.join("cores").join(dir).join(binary));
                if kind == CoreKind::Protogrid {
                    candidates.push(parent.join("cores").join(dir).join(legacy_protogrid_name()));
                }
            }
        }
        candidates.push(managed_binary);
        if kind == CoreKind::Protogrid {
            candidates.push(managed_legacy);
        }
        return candidates;
    }

    if let Some(path) = configured {
        candidates.push(path);
    }
    candidates.push(data_dir.join("cores").join(dir).join(binary));
    candidates.push(data_dir.join("cores").join(binary));
    candidates.push(data_dir.join("bin").join(binary));
    candidates.push(data_dir.join(binary));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join("cores").join(dir).join(binary));
            candidates.push(parent.join("cores").join(binary));
            candidates.push(parent.join("bin").join(binary));
            candidates.push(parent.join(binary));
        }
    }
    candidates
}

async fn run_version(path: &Path) -> Result<String, String> {
    let mut last_error = String::new();
    for arg in ["version", "--version"] {
        let mut command = Command::new(path);
        command.arg(arg).kill_on_drop(true);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.as_std_mut().creation_flags(0x08000000);
        }
        match tokio::time::timeout(Duration::from_secs(4), command.output()).await {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                if let Some(version) = parse_version(&stdout, &stderr) {
                    return Ok(version);
                }
                if !output.status.success() {
                    last_error = if stderr.trim().is_empty() {
                        format!("ядро завершилось с кодом {:?}", output.status.code())
                    } else {
                        stderr.trim().to_string()
                    };
                } else {
                    last_error = "не удалось определить версию ядра".into();
                }
            }
            Ok(Err(error)) => last_error = error.to_string(),
            Err(_) => last_error = "таймаут при запуске ядра".into(),
        }
    }
    Err(last_error)
}

fn parse_version(stdout: &str, stderr: &str) -> Option<String> {
    let all = format!("{stdout}\n{stderr}");
    for line in all.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("sing-box version ") {
            return rest.split_whitespace().next().map(str::to_string);
        }
        if let Some(rest) = line.strip_prefix("Xray ") {
            return rest.split_whitespace().next().map(str::to_string);
        }
    }
    None
}

fn compatibility(kind: CoreKind, version: &str) -> Result<(), String> {
    match kind {
        CoreKind::Singbox | CoreKind::Protogrid | CoreKind::ProtogridLite => {
            if version_at_least(version, "1.12.0") {
                Ok(())
            } else {
                Err(format!(
                    "sing-box {version} устарел. Нужна версия 1.12 или новее."
                ))
            }
        }
        CoreKind::Xray => Ok(()),
    }
}

pub async fn detect(kind: CoreKind, settings: &Settings, data_dir: &Path) -> CoreInfo {
    let mut run_failure: Option<(PathBuf, String)> = None;
    for path in candidate_paths(kind, settings, data_dir) {
        if !path.is_file() {
            continue;
        }
        match run_version(&path).await {
            Ok(version) => {
                let error = compatibility(kind, &version).err();
                let features = probe_features(kind, &path, &version).await;
                return CoreInfo {
                    found: true,
                    path: Some(path.to_string_lossy().to_string()),
                    version: Some(version),
                    compatible: error.is_none(),
                    error,
                    features,
                };
            }
            Err(error) => run_failure = Some((path, error)),
        }
    }
    let binary = binary_name(kind);
    if !matches!(kind, CoreKind::Protogrid | CoreKind::ProtogridLite) {
        if let Some(path) = find_in_path(binary) {
            match run_version(&path).await {
                Ok(version) => {
                    let error = compatibility(kind, &version).err();
                    let features = probe_features(kind, &path, &version).await;
                    return CoreInfo {
                        found: true,
                        path: Some(path.to_string_lossy().to_string()),
                        version: Some(version),
                        compatible: error.is_none(),
                        error,
                        features,
                    };
                }
                Err(error) => run_failure = Some((path, error)),
            }
        }
    }
    if let Some((path, error)) = run_failure {
        return CoreInfo {
            found: true,
            path: Some(path.to_string_lossy().to_string()),
            version: None,
            compatible: false,
            error: Some(format!("не удалось запустить ядро: {error}")),
            features: CoreFeatures::default(),
        };
    }
    let hint = if matches!(kind, CoreKind::Protogrid | CoreKind::ProtogridLite) {
        format!(
            "{} не найден. Положите {} рядом с ProtoGrid.exe или укажите путь в настройках.",
            kind.name(),
            binary
        )
    } else {
        format!(
            "{} не найден. Скачайте его в настройках или укажите путь.",
            kind.name()
        )
    };
    CoreInfo {
        found: false,
        path: None,
        version: None,
        compatible: false,
        error: Some(hint),
        features: CoreFeatures::default(),
    }
}

fn asset_candidates(kind: CoreKind, tag: &str) -> Result<Vec<String>, String> {
    let version = tag.trim_start_matches('v');
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    match kind {
        CoreKind::Protogrid => Err(
            "Ядро ProtoGrid поставляется вместе с приложением и не скачивается с GitHub.".into(),
        ),
        CoreKind::ProtogridLite => Err(
            "Ядро ProtoGrid-core Lite поставляется вместе с приложением и не скачивается с GitHub."
                .into(),
        ),
        CoreKind::Singbox => {
            let os_name = match os {
                "macos" => "darwin",
                "windows" => "windows",
                "linux" => "linux",
                other => return Err(format!("неизвестная ОС: {other}")),
            };
            let arch_name = match arch {
                "x86_64" => "amd64",
                "aarch64" => "arm64",
                other => return Err(format!("неизвестная архитектура: {other}")),
            };
            let ext = if os == "windows" { "zip" } else { "tar.gz" };
            Ok(vec![format!(
                "sing-box-{version}-{os_name}-{arch_name}.{ext}"
            )])
        }
        CoreKind::Xray => {
            let os_name = match os {
                "macos" => "macos",
                "windows" => "windows",
                "linux" => "linux",
                other => return Err(format!("неизвестная ОС: {other}")),
            };
            let arch_name = match arch {
                "x86_64" => "64",
                "aarch64" => "arm64-v8a",
                other => return Err(format!("неизвестная архитектура: {other}")),
            };
            Ok(vec![format!("Xray-{os_name}-{arch_name}.zip")])
        }
    }
}

async fn fetch_release(repo: &str) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder()
        .user_agent(format!("ProtoGrid/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| format!("HTTP-клиент: {error}"))?;
    let response = client
        .get(format!(
            "https://api.github.com/repos/{repo}/releases/latest"
        ))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| format!("не удалось получить релиз: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "GitHub API вернул {}. Укажите путь к ядру вручную.",
            response.status()
        ));
    }
    response
        .json::<serde_json::Value>()
        .await
        .map_err(|error| format!("некорректный ответ GitHub API: {error}"))
}

fn is_wanted(kind: CoreKind, name: &str) -> bool {
    match kind {
        CoreKind::Singbox | CoreKind::Protogrid | CoreKind::ProtogridLite => {
            matches!(name, "sing-box" | "sing-box.exe" | "wintun.dll")
        }
        CoreKind::Xray => matches!(name, "xray" | "xray.exe" | "geoip.dat" | "geosite.dat"),
    }
}

fn set_mode(path: &Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
}

fn extract_zip(kind: CoreKind, bytes: &[u8], dest: &Path) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("ZIP: {e}"))?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|e| format!("ZIP: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = Path::new(entry.name())
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        if !is_wanted(kind, &name) {
            continue;
        }
        let out_path = dest.join(&name);
        let mut out = File::create(&out_path)
            .map_err(|e| format!("не удалось создать {}: {e}", out_path.display()))?;
        std::io::copy(&mut entry, &mut out)
            .map_err(|e| format!("не удалось распаковать {}: {e}", out_path.display()))?;
        if name.ends_with(".exe") || name == "sing-box" || name == "xray" {
            set_mode(&out_path, 0o755);
        }
    }
    Ok(())
}

fn extract_tar_gz(kind: CoreKind, bytes: &[u8], dest: &Path) -> Result<(), String> {
    let decoder = flate2::read::GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    let entries = archive.entries().map_err(|e| format!("TAR: {e}"))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| format!("TAR: {e}"))?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().map_err(|e| format!("TAR: {e}"))?.to_path_buf();
        let Some(name) = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        if !is_wanted(kind, &name) {
            continue;
        }
        let out_path = dest.join(&name);
        let mut out = File::create(&out_path)
            .map_err(|e| format!("не удалось создать {}: {e}", out_path.display()))?;
        std::io::copy(&mut entry, &mut out)
            .map_err(|e| format!("не удалось распаковать {}: {e}", out_path.display()))?;
        if name == "sing-box" || name == "xray" {
            set_mode(&out_path, 0o755);
        }
    }
    Ok(())
}

pub async fn install(
    kind: CoreKind,
    inner: &Arc<Mutex<Inner>>,
    ctx: &egui::Context,
) -> Result<CoreInfo, String> {
    if matches!(kind, CoreKind::Protogrid | CoreKind::ProtogridLite) {
        return install_bundled(kind, inner, ctx).await;
    }
    let data_dir = inner
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .data_dir
        .clone();
    push_log(
        inner,
        ctx,
        "app",
        Some("info"),
        format!("Поиск последней версии {}…", kind.name()),
    );
    let repo = match kind {
        CoreKind::Singbox => "SagerNet/sing-box",
        CoreKind::Xray => "XTLS/Xray-core",
        CoreKind::Protogrid | CoreKind::ProtogridLite => {
            unreachable!("handled by install_bundled")
        }
    };
    let release = fetch_release(repo).await?;
    let tag = release
        .get("tag_name")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "в релизе нет tag_name".to_string())?;
    let candidates = asset_candidates(kind, tag)?;
    let assets = release
        .get("assets")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "в релизе нет списка файлов".to_string())?;
    let mut selected = None;
    for candidate in &candidates {
        for asset in assets {
            let name = asset
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            if name.eq_ignore_ascii_case(candidate) {
                let url = asset
                    .get("browser_download_url")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                selected = Some((name.to_string(), url.to_string()));
                break;
            }
        }
        if selected.is_some() {
            break;
        }
    }
    let (asset_name, asset_url) = selected.ok_or_else(|| {
        format!(
            "в релизе {tag} не найден подходящий файл ({})",
            candidates.join(" / ")
        )
    })?;
    push_log(
        inner,
        ctx,
        "app",
        Some("info"),
        format!("Загрузка {asset_name}…"),
    );
    let client = reqwest::Client::builder()
        .user_agent(format!("ProtoGrid/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|error| format!("HTTP-клиент: {error}"))?;
    let response = client
        .get(&asset_url)
        .send()
        .await
        .map_err(|error| format!("не удалось скачать ядро: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("скачивание ядра: HTTP {}", response.status()));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("не удалось прочитать ядро: {error}"))?;
    let dest = data_dir.join("cores").join(dir_name(kind));
    std::fs::create_dir_all(&dest)
        .map_err(|error| format!("не удалось создать {}: {error}", dest.display()))?;
    if asset_name.ends_with(".zip") {
        extract_zip(kind, &bytes, &dest)?;
    } else if asset_name.ends_with(".tar.gz") {
        extract_tar_gz(kind, &bytes, &dest)?;
    } else {
        return Err(format!("неизвестный архив: {asset_name}"));
    }
    let binary = dest.join(binary_name(kind));
    if !binary.is_file() {
        return Err("в архиве не найден исполняемый файл".into());
    }
    set_mode(&binary, 0o755);
    let version = run_version(&binary)
        .await
        .map_err(|error| format!("скачанное ядро не запускается: {error}"))?;
    let compatibility = compatibility(kind, &version);
    let features = probe_features(kind, &binary, &version).await;
    let info = CoreInfo {
        found: true,
        path: Some(binary.to_string_lossy().to_string()),
        version: Some(version),
        compatible: compatibility.is_ok(),
        error: compatibility.err(),
        features,
    };
    if !info.compatible {
        return Err(info
            .error
            .clone()
            .unwrap_or_else(|| "скачанное ядро несовместимо".into()));
    }
    Ok(info)
}

/// Bundled cores are shipped next to the launcher. Install means: locate and
/// validate that binary without copying it into the profile.
async fn install_bundled(
    kind: CoreKind,
    inner: &Arc<Mutex<Inner>>,
    ctx: &egui::Context,
) -> Result<CoreInfo, String> {
    let (data_dir, settings) = {
        let guard = inner.lock().unwrap_or_else(|e| e.into_inner());
        (guard.data_dir.clone(), guard.config.settings.clone())
    };
    push_log(
        inner,
        ctx,
        "app",
        Some("info"),
        format!("Поиск ядра {} рядом с приложением…", kind.name()),
    );
    let info = detect(kind, &settings, &data_dir).await;
    if !info.found {
        return Err(format!(
            "Ядро {} не найдено. Положите {} рядом с ProtoGrid.exe.",
            kind.name(),
            binary_name(kind)
        ));
    }
    if let Some(error) = info.error.clone() {
        return Err(error);
    }
    if let Some(path) = &info.path {
        push_log(
            inner,
            ctx,
            "app",
            Some("info"),
            format!("Ядро {} готово: {path}", kind.name()),
        );
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core_with_version(version: &str) -> CoreInfo {
        CoreInfo {
            found: true,
            path: None,
            version: Some(version.into()),
            compatible: true,
            error: None,
            features: CoreFeatures::heuristic(CoreKind::Singbox, version),
        }
    }

    #[test]
    fn detects_xhttp_capable_fork() {
        assert!(core_with_version("1.14.4-protogrid.1").supports_xhttp());
        assert!(!core_with_version("1.14.4").supports_xhttp());
        assert!(!core_with_version("1.15.0").supports_xhttp());
    }

    #[test]
    fn detects_mldsa65_capable_fork() {
        assert!(core_with_version("1.14.4-protogrid.1").supports_mldsa65());
        assert!(!core_with_version("1.14.4").supports_mldsa65());
    }

    #[test]
    fn detects_masque_capable_fork() {
        assert!(core_with_version("1.14.4-protogrid.1").supports_masque());
        assert!(!core_with_version("1.14.4").supports_masque());
    }

    #[test]
    fn detects_noise_capable_fork() {
        assert!(core_with_version("1.14.4-protogrid.2").supports_noise());
        assert!(!core_with_version("1.14.4").supports_noise());
        let xray = CoreInfo {
            found: true,
            path: None,
            version: Some("26.10.21".into()),
            compatible: true,
            error: None,
            features: CoreFeatures::heuristic(CoreKind::Xray, "26.10.21"),
        };
        assert!(xray.supports_noise());
    }

    #[test]
    fn detects_openvpn_capable_core() {
        assert!(core_with_version("1.14.0").supports_openvpn());
        assert!(core_with_version("1.14.4").supports_openvpn());
        assert!(core_with_version("1.14.4-protogrid.3").supports_openvpn());
        assert!(!core_with_version("1.13.0").supports_openvpn());
        let xray = CoreInfo {
            found: true,
            path: None,
            version: Some("26.10.21".into()),
            compatible: true,
            error: None,
            features: CoreFeatures::heuristic(CoreKind::Xray, "26.10.21"),
        };
        assert!(!xray.supports_openvpn());
    }
}
