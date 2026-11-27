use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::model::{RoutingRule, RuleKind};
use crate::util::split_list;

/// Remote GeoIP/GeoSite rule-set used by sing-box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSetSource {
    pub tag: String,
    pub url: String,
}

#[derive(Debug, Default)]
pub struct PreparedRuleSets {
    pub available: Vec<String>,
    pub downloaded: usize,
    pub errors: Vec<String>,
}

pub fn sanitize_code(code: &str) -> String {
    code.trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Collect the sing-box rule-set sources referenced by enabled routing rules.
pub fn sources(rules: &[RoutingRule]) -> Vec<RuleSetSource> {
    let mut sources: Vec<RuleSetSource> = Vec::new();
    for rule in rules.iter().filter(|rule| rule.enabled) {
        let (kind, base) = match rule.kind {
            RuleKind::Geoip => ("geoip", "sing-geoip"),
            RuleKind::Geosite => ("geosite", "sing-geosite"),
            _ => continue,
        };
        for code in split_list(&rule.value) {
            let raw = code.trim().to_ascii_lowercase();
            let tag_code = sanitize_code(&raw);
            // SagerNet geosite names may contain '!' (geolocation-!cn), so keep
            // the characters that are valid in the repository file names.
            let url_code: String = raw
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '!'))
                .collect();
            if tag_code.is_empty() || url_code.is_empty() {
                continue;
            }
            let tag = format!("{kind}-{tag_code}");
            if sources.iter().any(|source| source.tag == tag) {
                continue;
            }
            let url = format!(
                "https://raw.githubusercontent.com/SagerNet/{base}/rule-set/{kind}-{url_code}.srs"
            );
            sources.push(RuleSetSource { tag, url });
        }
    }
    sources
}

pub fn local_path(dir: &Path, tag: &str) -> PathBuf {
    dir.join(format!("{tag}.srs"))
}

/// Copies the supplied rule-set bytes into `dir` unless a non-empty file is
/// already there. Returns the names that were seeded.
///
/// Callers pass either bytes compiled into their binary (Android) or files
/// shipped next to the executable (desktop). The normal `prepare` refresh
/// still updates stale copies from the network and keeps the previous copy if
/// GitHub is unavailable.
pub fn seed_bytes(dir: &Path, files: &[(&str, &[u8])]) -> std::io::Result<Vec<String>> {
    std::fs::create_dir_all(dir)?;
    let mut seeded = Vec::new();
    for (name, bytes) in files {
        let path = dir.join(name);
        if matches!(std::fs::metadata(&path), Ok(metadata) if metadata.len() > 0) {
            continue;
        }
        std::fs::write(&path, bytes)?;
        seeded.push((*name).to_string());
    }
    Ok(seeded)
}

fn is_fresh(path: &Path, max_age: Duration) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if metadata.len() == 0 {
        return false;
    }
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    match SystemTime::now().duration_since(modified) {
        Ok(age) => age < max_age,
        Err(_) => true,
    }
}

/// Download missing rule sets into `dir` and keep stale copies as a fallback.
///
/// The app uses the returned directory to build a sing-box config with local
/// rule sets, so a subsequent start does not depend on GitHub being reachable.
pub async fn prepare(sources: &[RuleSetSource], dir: &Path, max_age: Duration) -> PreparedRuleSets {
    let mut prepared = PreparedRuleSets::default();
    if sources.is_empty() {
        return prepared;
    }
    if let Err(error) = std::fs::create_dir_all(dir) {
        prepared
            .errors
            .push(format!("не удалось создать {}: {error}", dir.display()));
        return prepared;
    }

    let client = match reqwest::Client::builder()
        .user_agent(format!("ProtoGrid/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(15))
        .no_proxy()
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            prepared.errors.push(format!("HTTP-клиент: {error}"));
            return prepared;
        }
    };

    let mut jobs = Vec::new();
    for source in sources {
        let path = local_path(dir, &source.tag);
        if is_fresh(&path, max_age) {
            prepared.available.push(source.tag.clone());
        } else {
            jobs.push((source.clone(), path));
        }
    }

    let mut handles = Vec::new();
    for (source, path) in jobs {
        let client = client.clone();
        handles.push(tokio::spawn(async move {
            let result = async {
                let response = client
                    .get(&source.url)
                    .send()
                    .await
                    .map_err(|error| format!("{error}"))?;
                if !response.status().is_success() {
                    return Err(format!("HTTP {}", response.status()));
                }
                let bytes = response.bytes().await.map_err(|error| format!("{error}"))?;
                if bytes.len() < 16 {
                    return Err("пустой файл rule-set".to_string());
                }
                let temp = path.with_extension("srs.tmp");
                std::fs::write(&temp, &bytes)
                    .map_err(|error| format!("не удалось записать кэш: {error}"))?;
                std::fs::rename(&temp, &path)
                    .map_err(|error| format!("не удалось сохранить кэш: {error}"))?;
                Ok(())
            }
            .await;
            (source.tag, path, result)
        }));
    }

    for handle in handles {
        match handle.await {
            Ok((tag, _path, Ok(()))) => {
                prepared.downloaded += 1;
                prepared.available.push(tag);
            }
            Ok((tag, path, Err(error))) => {
                if path.is_file() {
                    prepared.available.push(tag.clone());
                    prepared
                        .errors
                        .push(format!("{tag}: {error}; используется сохранённая копия"));
                } else {
                    prepared
                        .errors
                        .push(format!("{tag}: {error}; правило пропущено"));
                }
            }
            Err(error) => prepared.errors.push(format!("загрузка rule-set: {error}")),
        }
    }

    prepared
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{RoutingRule, RuleAction};

    fn rule(kind: RuleKind, value: &str) -> RoutingRule {
        RoutingRule {
            kind,
            value: value.into(),
            action: RuleAction::Direct,
            ..Default::default()
        }
    }

    #[test]
    fn collects_deduplicated_sources() {
        let rules = vec![
            rule(RuleKind::Geosite, "category-ru, category-ads-all"),
            rule(RuleKind::Geoip, "ru"),
            rule(RuleKind::Geosite, "category-ru"),
        ];
        let sources = sources(&rules);
        assert_eq!(sources.len(), 3);
        assert!(sources.iter().any(|s| s.tag == "geosite-category-ru"));
        assert!(sources.iter().any(|s| s.tag == "geoip-ru"));
    }

    #[test]
    fn keeps_exclamation_mark_in_geosite_url() {
        let rules = vec![rule(RuleKind::Geosite, "geolocation-!cn")];
        let sources = sources(&rules);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].tag, "geosite-geolocation--cn");
        assert!(sources[0].url.ends_with("geosite-geolocation-!cn.srs"));
    }

    #[test]
    fn seed_bytes_fill_missing_files_only() {
        let dir = std::env::temp_dir().join(format!("protogrid-seed-{}", uuid::Uuid::new_v4()));
        let _ = std::fs::remove_dir_all(&dir);
        let files: &[(&str, &[u8])] = &[
            ("geoip-test.srs", b"bundled-geoip"),
            ("geosite-test.srs", b"bundled-geosite"),
        ];

        let seeded = seed_bytes(&dir, files).unwrap();
        assert!(seeded.iter().any(|name| name == "geoip-test.srs"));
        assert!(seeded.iter().any(|name| name == "geosite-test.srs"));
        let geoip = dir.join("geoip-test.srs");
        assert_eq!(std::fs::read(&geoip).unwrap(), b"bundled-geoip");

        // The cache may already contain a downloaded update: never overwrite it.
        std::fs::write(&geoip, b"downloaded-update").unwrap();
        let seeded = seed_bytes(&dir, files).unwrap();
        assert!(!seeded.iter().any(|name| name == "geoip-test.srs"));
        assert_eq!(std::fs::read(&geoip).unwrap(), b"downloaded-update");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn uses_stale_cache_when_download_fails() {
        let dir = std::env::temp_dir().join(format!("protogrid-rules-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("geosite-test.srs"), b"cached-rule-set").unwrap();
        let sources = vec![RuleSetSource {
            tag: "geosite-test".into(),
            url: "http://127.0.0.1:1/missing.srs".into(),
        }];
        // Zero max age forces a refresh attempt; the local copy must survive.
        let prepared = prepare(&sources, &dir, Duration::from_secs(0)).await;
        assert_eq!(prepared.available, vec!["geosite-test".to_string()]);
        assert!(!prepared.errors.is_empty());
        assert!(dir.join("geosite-test.srs").is_file());
        let _ = std::fs::remove_dir_all(dir);
    }
}
