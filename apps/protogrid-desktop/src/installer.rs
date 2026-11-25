use std::fs::File;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use protogrid_core::model::{CoreKind, Settings};
use protogrid_core::util::version_at_least;
use tokio::process::Command;

use crate::engine::{push_log, Inner};

#[derive(Debug, Clone)]
pub struct CoreInfo {
    pub found: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub compatible: bool,
    pub error: Option<String>,
}

impl CoreInfo {
    /// The ProtoGrid sing-box fork advertises XHTTP support in its version string,
    /// e.g. `1.14.4-protogrid.1`.
    pub fn supports_xhttp(&self) -> bool {
        self.version
            .as_deref()
            .is_some_and(|version| version.contains("-protogrid"))
    }

    pub fn supports_kcp(&self) -> bool {
        self.supports_xhttp()
    }

    pub fn supports_xdrive(&self) -> bool {
        self.supports_xhttp()
    }
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
    }
}

fn dir_name(kind: CoreKind) -> &'static str {
    match kind {
        CoreKind::Singbox => "sing-box",
        CoreKind::Xray => "xray",
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
    }
}

fn candidate_paths(kind: CoreKind, settings: &Settings, data_dir: &Path) -> Vec<PathBuf> {
    let binary = binary_name(kind);
    let dir = dir_name(kind);
    let mut candidates = Vec::new();
    let configured = configure_path(kind, settings);
    if !configured.trim().is_empty() {
        let mut path = expand_tilde(configured.trim());
        if path.is_dir() {
            path = path.join(binary);
        }
        if !path.is_absolute() {
            path = data_dir.join(path);
        }
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
        CoreKind::Singbox => {
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
                return CoreInfo {
                    found: true,
                    path: Some(path.to_string_lossy().to_string()),
                    version: Some(version),
                    compatible: error.is_none(),
                    error,
                };
            }
            Err(error) => run_failure = Some((path, error)),
        }
    }
    let binary = binary_name(kind);
    if let Some(path) = find_in_path(binary) {
        match run_version(&path).await {
            Ok(version) => {
                let error = compatibility(kind, &version).err();
                return CoreInfo {
                    found: true,
                    path: Some(path.to_string_lossy().to_string()),
                    version: Some(version),
                    compatible: error.is_none(),
                    error,
                };
            }
            Err(error) => run_failure = Some((path, error)),
        }
    }
    if let Some((path, error)) = run_failure {
        return CoreInfo {
            found: true,
            path: Some(path.to_string_lossy().to_string()),
            version: None,
            compatible: false,
            error: Some(format!("не удалось запустить ядро: {error}")),
        };
    }
    CoreInfo {
        found: false,
        path: None,
        version: None,
        compatible: false,
        error: Some(format!(
            "{} не найден. Скачайте его в настройках или укажите путь.",
            kind.name()
        )),
    }
}

fn asset_candidates(kind: CoreKind, tag: &str) -> Result<Vec<String>, String> {
    let version = tag.trim_start_matches('v');
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    match kind {
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
        CoreKind::Singbox => matches!(name, "sing-box" | "sing-box.exe" | "wintun.dll"),
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
    let info = CoreInfo {
        found: true,
        path: Some(binary.to_string_lossy().to_string()),
        version: Some(version),
        compatible: compatibility.is_ok(),
        error: compatibility.err(),
    };
    if !info.compatible {
        return Err(info
            .error
            .clone()
            .unwrap_or_else(|| "скачанное ядро несовместимо".into()));
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
        }
    }

    #[test]
    fn detects_xhttp_capable_fork() {
        assert!(core_with_version("1.14.4-protogrid.1").supports_xhttp());
        assert!(!core_with_version("1.14.4").supports_xhttp());
        assert!(!core_with_version("1.15.0").supports_xhttp());
    }
}
