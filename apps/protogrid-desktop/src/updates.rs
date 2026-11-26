//! Update checks against the public GitHub releases.
//!
//! Everything here is deliberately read-only: a release is fetched, compared
//! with the running build, and the matching asset is downloaded only after an
//! explicit click. Cores (sing-box and Xray) are checked the same way; the
//! ProtoGrid core ships with the application and is updated with it.

use std::path::{Path, PathBuf};
use std::time::Duration;

pub const PROTOGRID_REPO: &str = "GimmickFive/ProtoGrid";
pub const SINGBOX_REPO: &str = "SagerNet/sing-box";
pub const XRAY_REPO: &str = "XTLS/Xray-core";

const RELEASE_LIMIT: usize = 20;

#[derive(Debug, Clone, Default)]
pub struct Release {
    pub tag: String,
    pub notes: String,
    pub page_url: String,
    pub published_at: String,
    pub prerelease: bool,
    pub assets: Vec<Asset>,
}

impl Release {
    /// `2026-11-26T09:53:56Z` -> `2026-11-26`.
    pub fn published_date(&self) -> Option<&str> {
        self.published_at
            .split('T')
            .next()
            .filter(|date| !date.is_empty())
    }
}

#[derive(Debug, Clone, Default)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub digest: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AppUpdate {
    pub version: String,
    pub notes: String,
    pub page_url: String,
    pub published: Option<String>,
    pub prerelease: bool,
    pub asset_name: String,
    pub asset_url: String,
    pub asset_size: u64,
    pub asset_digest: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CoreUpdate {
    pub latest: Option<String>,
    pub available: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateState {
    pub current: String,
    pub checking: bool,
    pub checked: bool,
    pub error: Option<String>,
    pub available: Option<AppUpdate>,
    pub downloading: bool,
    pub downloaded: Option<PathBuf>,
    pub last_checked: Option<i64>,
}

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            current: env!("CARGO_PKG_VERSION").to_string(),
            checking: false,
            checked: false,
            error: None,
            available: None,
            downloading: false,
            downloaded: None,
            last_checked: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CoreUpdates {
    pub singbox: CoreUpdate,
    pub xray: CoreUpdate,
}

/// Strip the tag decoration used by the release workflows.
pub fn version_of(tag: &str) -> String {
    tag.trim()
        .trim_start_matches("android-v")
        .trim_start_matches('v')
        .to_string()
}

/// Numeric key of the main version part. Prerelease/build suffixes are
/// ignored, so `0.2.1` and `0.2.1-beta.1` compare as the same release.
pub fn version_key(version: &str) -> Vec<u64> {
    let cleaned = version_of(version);
    let main = cleaned.split(['-', '+']).next().unwrap_or(cleaned.as_str());
    main.split('.')
        .map(|part| part.trim().parse::<u64>().unwrap_or(0))
        .collect()
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    let a = version_key(latest);
    let b = version_key(current);
    if a.is_empty() || b.is_empty() {
        return false;
    }
    for index in 0..a.len().max(b.len()) {
        let left = *a.get(index).unwrap_or(&0);
        let right = *b.get(index).unwrap_or(&0);
        if left != right {
            return left > right;
        }
    }
    false
}

fn client(timeout: Duration) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(format!("ProtoGrid/{}", env!("CARGO_PKG_VERSION")))
        .timeout(timeout)
        .build()
        .map_err(|error| format!("HTTP-клиент: {error}"))
}

async fn fetch_json(url: &str) -> Result<serde_json::Value, String> {
    let client = client(Duration::from_secs(45))?;
    let response = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|error| format!("не удалось связаться с GitHub: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        let message = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|value| {
                value
                    .get("message")
                    .and_then(|message| message.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| text.chars().take(180).collect());
        return Err(format!("GitHub API {status}: {message}"));
    }
    response
        .json::<serde_json::Value>()
        .await
        .map_err(|error| format!("некорректный ответ GitHub: {error}"))
}

fn parse_asset(value: &serde_json::Value) -> Option<Asset> {
    let name = value.get("name")?.as_str()?.trim();
    let url = value.get("browser_download_url")?.as_str()?.trim();
    if name.is_empty() || url.is_empty() {
        return None;
    }
    Some(Asset {
        name: name.to_string(),
        url: url.to_string(),
        size: value
            .get("size")
            .and_then(|size| size.as_u64())
            .unwrap_or(0),
        digest: value
            .get("digest")
            .and_then(|digest| digest.as_str())
            .map(str::to_string),
    })
}

fn parse_release(value: &serde_json::Value) -> Option<Release> {
    if value
        .get("draft")
        .and_then(|draft| draft.as_bool())
        .unwrap_or(false)
    {
        return None;
    }
    let tag = value.get("tag_name")?.as_str()?.trim();
    if tag.is_empty() {
        return None;
    }
    let assets = value
        .get("assets")
        .and_then(|assets| assets.as_array())
        .map(|assets| assets.iter().filter_map(parse_asset).collect())
        .unwrap_or_default();
    Some(Release {
        tag: tag.to_string(),
        notes: value
            .get("body")
            .and_then(|body| body.as_str())
            .unwrap_or("")
            .to_string(),
        page_url: value
            .get("html_url")
            .and_then(|url| url.as_str())
            .unwrap_or("")
            .to_string(),
        published_at: value
            .get("published_at")
            .and_then(|date| date.as_str())
            .unwrap_or("")
            .to_string(),
        prerelease: value
            .get("prerelease")
            .and_then(|flag| flag.as_bool())
            .unwrap_or(false),
        assets,
    })
}

pub async fn fetch_releases(repo: &str) -> Result<Vec<Release>, String> {
    let value = fetch_json(&format!(
        "https://api.github.com/repos/{repo}/releases?per_page={RELEASE_LIMIT}"
    ))
    .await?;
    let list = value
        .as_array()
        .ok_or_else(|| "GitHub вернул не список релизов".to_string())?;
    Ok(list.iter().filter_map(parse_release).collect())
}

/// Desktop releases use `vX.Y.Z`; Android-only releases are tagged
/// `android-vX.Y.Z` and must not trigger a launcher update.
pub fn pick_app_release(releases: &[Release]) -> Option<&Release> {
    let desktop: Vec<&Release> = releases
        .iter()
        .filter(|release| {
            !release.tag.starts_with("android-")
                && release
                    .tag
                    .trim_start_matches('v')
                    .chars()
                    .next()
                    .is_some_and(|first| first.is_ascii_digit())
        })
        .collect();
    desktop
        .into_iter()
        .max_by(|left, right| version_key(&left.tag).cmp(&version_key(&right.tag)))
}

/// The Android APK may live in a `v*` release together with the desktop
/// assets, or in a dedicated `android-v*` release. The desktop launcher never
/// installs APKs, but keeping the rule here documents the release layout and
/// is covered by the tests.
#[allow(dead_code)]
pub fn pick_android_release(releases: &[Release]) -> Option<&Release> {
    let android: Vec<&Release> = releases
        .iter()
        .filter(|release| {
            release
                .assets
                .iter()
                .any(|asset| asset.name.to_ascii_lowercase().ends_with(".apk"))
        })
        .collect();
    android
        .into_iter()
        .max_by(|left, right| version_key(&left.tag).cmp(&version_key(&right.tag)))
}

pub fn pick_core_release(releases: &[Release]) -> Option<&Release> {
    // Never offer a beta of a core when a stable release exists.
    let stable: Vec<&Release> = releases
        .iter()
        .filter(|release| !release.prerelease)
        .collect();
    let pool: Vec<&Release> = if stable.is_empty() {
        releases.iter().collect()
    } else {
        stable
    };
    pool.into_iter()
        .max_by(|left, right| version_key(&left.tag).cmp(&version_key(&right.tag)))
}

fn find_asset<'a>(release: &'a Release, name: &str) -> Option<&'a Asset> {
    release
        .assets
        .iter()
        .find(|asset| asset.name.eq_ignore_ascii_case(name))
}

/// Asset that matches this build. The Windows packages are the only ones
/// currently published; on other systems the caller falls back to the release
/// page instead of downloading an incompatible binary.
pub fn app_asset(release: &Release, portable: bool) -> Option<&Asset> {
    let version = version_of(&release.tag);
    let mut candidates = Vec::new();
    if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        let setup = format!("ProtoGrid-{version}-windows-x64-setup.exe");
        let zip = format!("ProtoGrid-{version}-windows-x64-portable.zip");
        if portable {
            candidates.push(zip);
            candidates.push(setup);
        } else {
            candidates.push(setup);
            candidates.push(zip);
        }
    }
    for candidate in candidates {
        if let Some(asset) = find_asset(release, &candidate) {
            return Some(asset);
        }
    }
    // Fall back to a name-based match so a changed prefix does not hide a
    // perfectly good archive from the same release.
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    release.assets.iter().find(|asset| {
        let name = asset.name.to_ascii_lowercase();
        name.contains(os)
            && (name.ends_with(".zip")
                || name.ends_with(".exe")
                || name.ends_with(".dmg")
                || name.ends_with(".appimage"))
    })
}

pub async fn download(
    url: &str,
    name: &str,
    digest: Option<&str>,
    expected_size: u64,
    dest_dir: &Path,
) -> Result<PathBuf, String> {
    let file_name = Path::new(name)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "у файла обновления некорректное имя".to_string())?;
    std::fs::create_dir_all(dest_dir)
        .map_err(|error| format!("не удалось создать {}: {error}", dest_dir.display()))?;
    let target = dest_dir.join(file_name);
    let temp = dest_dir.join(format!("{file_name}.part"));
    let client = client(Duration::from_secs(900))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("не удалось скачать обновление: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("скачивание обновления: HTTP {status}"));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("не удалось прочитать обновление: {error}"))?;
    if expected_size > 0 && bytes.len() as u64 != expected_size {
        return Err(format!(
            "размер файла не совпал: ожидалось {expected_size} Б, получено {} Б",
            bytes.len()
        ));
    }
    if let Some(expected) = digest.filter(|value| !value.trim().is_empty()) {
        use sha2::{Digest, Sha256};
        let expected = expected
            .trim()
            .strip_prefix("sha256:")
            .unwrap_or(expected.trim())
            .to_ascii_lowercase();
        let actual = format!("{:x}", Sha256::digest(&bytes));
        if actual != expected {
            return Err("контрольная сумма обновления не совпала — файл повреждён".into());
        }
    }
    std::fs::write(&temp, &bytes)
        .map_err(|error| format!("не удалось сохранить {}: {error}", temp.display()))?;
    if target.exists() {
        std::fs::remove_file(&target)
            .map_err(|error| format!("не удалось заменить {}: {error}", target.display()))?;
    }
    std::fs::rename(&temp, &target)
        .map_err(|error| format!("не удалось переименовать обновление: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755));
    }
    Ok(target)
}

/// Open a release page or any other HTTPS link in the system browser.
pub fn open_url(url: &str) -> Result<(), String> {
    if url.trim().is_empty() {
        return Err("ссылка на релиз недоступна".into());
    }
    let (program, args): (&str, Vec<&str>) = if cfg!(target_os = "windows") {
        ("cmd", vec!["/C", "start", "", url])
    } else if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else {
        ("xdg-open", vec![url])
    };
    std::process::Command::new(program)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("не удалось открыть ссылку: {error}"))
}

/// Show the downloaded file in the system file manager.
pub fn reveal_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err("скачанный файл не найден".into());
    }
    if cfg!(target_os = "windows") {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("не удалось открыть папку: {error}"))
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("не удалось открыть папку: {error}"))
    } else {
        let dir = path.parent().unwrap_or(path);
        std::process::Command::new("xdg-open")
            .arg(dir)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("не удалось открыть папку: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, assets: &[&str]) -> Release {
        Release {
            tag: tag.into(),
            assets: assets
                .iter()
                .map(|name| Asset {
                    name: (*name).into(),
                    url: format!("https://example.com/{name}"),
                    size: 1,
                    digest: None,
                })
                .collect(),
            ..Release::default()
        }
    }

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("0.2.2", "0.2.1"));
        assert!(is_newer("0.3.0", "0.2.9"));
        assert!(!is_newer("0.2.1", "0.2.1"));
        assert!(!is_newer("0.2.0", "0.2.1"));
        assert!(!is_newer("0.1.9", "0.2.0"));
        assert!(!is_newer("1.14.4", "1.14.4-protogrid.3"));
        assert!(is_newer("1.15.0", "1.14.4-protogrid.3"));
    }

    #[test]
    fn picks_desktop_and_android_releases_separately() {
        let releases = vec![
            release("v0.2.1", &["ProtoGrid-0.2.1-windows-x64-setup.exe"]),
            release("android-v0.3.0", &["ProtoGrid-0.3.0-arm64-v8a.apk"]),
        ];
        assert_eq!(pick_app_release(&releases).unwrap().tag, "v0.2.1");
        assert_eq!(
            pick_android_release(&releases).unwrap().tag,
            "android-v0.3.0"
        );
    }

    #[test]
    fn prefers_stable_core_release() {
        let mut beta = release("v1.15.0-beta.1", &[]);
        beta.prerelease = true;
        let releases = vec![release("v1.14.4", &[]), beta];
        assert_eq!(pick_core_release(&releases).unwrap().tag, "v1.14.4");
    }

    #[test]
    fn selects_windows_asset() {
        let release = release(
            "v0.2.1",
            &[
                "ProtoGrid-0.2.1-windows-x64-portable.zip",
                "ProtoGrid-0.2.1-windows-x64-setup.exe",
            ],
        );
        if cfg!(target_os = "windows") {
            assert!(app_asset(&release, false)
                .unwrap()
                .name
                .ends_with("setup.exe"));
            assert!(app_asset(&release, true)
                .unwrap()
                .name
                .ends_with("portable.zip"));
        }
    }
}
