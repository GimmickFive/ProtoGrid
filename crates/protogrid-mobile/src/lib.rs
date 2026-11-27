//! JNI bridge for the ProtoGrid Android client.
//!
//! The Android UI is Kotlin, but parsing share links and rendering sing-box
//! configurations stay in the shared Rust core so desktop and mobile keep the
//! same protocol coverage and validation rules.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jlong, jstring};
use jni::JNIEnv;
use protogrid_core::model::{
    default_rules, routing_preset, CoreCapabilities, CoreKind, RoutingPreset, RoutingRule,
    RuleAction, RuleKind, Server, Settings,
};
use protogrid_core::singbox;
use serde::Deserialize;
use serde_json::{json, Value};

/// libbox in the Android app is the ProtoGrid-core build (sing-box + patches).
const ANDROID_CORE_VERSION: &str = "1.14.4";

/// Startup GeoIP/GeoSite snapshots compiled into the Android bridge. The
/// desktop package ships the same files next to the executable instead, so
/// the MIT launcher stays separate from the GPL rule-set data.
static GEOIP_RU_SRS: &[u8] = include_bytes!("../../../assets/rulesets/geoip-ru.srs");
static GEOSITE_CATEGORY_RU_SRS: &[u8] =
    include_bytes!("../../../assets/rulesets/geosite-category-ru.srs");
static BUNDLED_RULESETS: &[(&str, &[u8])] = &[
    ("geoip-ru.srs", GEOIP_RU_SRS),
    ("geosite-category-ru.srs", GEOSITE_CATEGORY_RU_SRS),
];

/// Set from Kotlin before the first config build: the Android lite flavor
/// links a libbox built with `protogrid_lite`, which only registers VLESS over
/// plain TCP. The flag keeps the shared Rust core from emitting configs the
/// lite build cannot run.
static LITE_MODE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RoutingInput {
    preset: String,
    final_action: String,
    rules: Vec<RoutingRule>,
    rule_set_dir: String,
}

fn capabilities() -> CoreCapabilities {
    capabilities_for(LITE_MODE.load(Ordering::Relaxed))
}

fn capabilities_for(lite: bool) -> CoreCapabilities {
    if lite {
        return CoreCapabilities {
            // Reality still uses the patched TLS stack, including ML-DSA-65
            // verification; only extra protocols/transports are absent.
            mldsa65: true,
            vless_only: true,
            ..CoreCapabilities::default()
        };
    }
    CoreCapabilities {
        xhttp: true,
        kcp: true,
        xdrive: true,
        mldsa65: true,
        masque: true,
        noise: true,
        openvpn: true,
        vless_only: false,
    }
}

fn android_settings() -> Settings {
    Settings {
        core: CoreKind::Protogrid,
        // libbox owns the TUN interface through VpnService; the mixed inbound
        // is removed below because Android has no system proxy to point at it.
        tun_enabled: true,
        system_proxy: false,
        mixed_port: 2080,
        clash_api_port: 9090,
        routing_preset: Some(RoutingPreset::Global),
        final_action: RuleAction::Proxy,
        ..Settings::default()
    }
}

/// Remove desktop-only bits and pick a TUN stack that works over a
/// `VpnService` file descriptor on Android.
fn android_tweaks(mut config: Value) -> Value {
    if let Some(inbounds) = config.get_mut("inbounds").and_then(Value::as_array_mut) {
        inbounds.retain(|inbound| inbound.get("tag").and_then(Value::as_str) != Some("mixed-in"));
        for inbound in inbounds.iter_mut() {
            if inbound.get("type").and_then(Value::as_str) == Some("tun") {
                inbound["stack"] = json!("gvisor");
                inbound["strict_route"] = json!(false);
            }
        }
    }
    config
}

/// The mobile "Жизнь в РФ" preset: RU domains, RU GeoIP/GeoSite and the local
/// network go direct, everything else goes through the proxy. Process rules
/// are omitted because Android's connection-owner API is not wired yet.
///
/// The GeoIP/GeoSite rule sets are seeded from binaries compiled into the app
/// (see `protogrid_core::ruleset::seed_bundled`) and refreshed from SagerNet
/// when the cache is stale. If neither the seed nor the download is available,
/// `route_rule` drops those two rules and the `.ru/.su/.рф` suffix rule keeps
/// the preset useful.
fn mobile_bypass_rules() -> Vec<RoutingRule> {
    let mut rules = default_rules();
    rules.retain(|rule| {
        // RU GeoIP/GeoSite stay; unrelated Geo rules (ads) and process rules do not.
        !matches!(rule.kind, RuleKind::ProcessName) && rule.id != "default-ads"
    });
    rules.push(RoutingRule {
        id: "android-ru-domains".into(),
        name: "RU-домены".into(),
        enabled: true,
        action: RuleAction::Direct,
        kind: RuleKind::DomainSuffix,
        value: ".ru, .su, .рф".into(),
    });
    rules
}

fn parse_action(value: &str) -> RuleAction {
    match value {
        "direct" => RuleAction::Direct,
        "block" => RuleAction::Block,
        _ => RuleAction::Proxy,
    }
}

fn routing_for(input: &RoutingInput) -> (Vec<RoutingRule>, RuleAction, RoutingPreset) {
    match input.preset.as_str() {
        "custom" => (
            input.rules.clone(),
            parse_action(&input.final_action),
            RoutingPreset::Custom,
        ),
        "bypass" => (
            mobile_bypass_rules(),
            RuleAction::Proxy,
            RoutingPreset::Bypass,
        ),
        "direct" => (Vec::new(), RuleAction::Direct, RoutingPreset::Direct),
        _ => {
            let (rules, action) =
                routing_preset("global").unwrap_or_else(|| (Vec::new(), RuleAction::Proxy));
            (rules, action, RoutingPreset::Global)
        }
    }
}

fn parse_links_json(input: &str) -> String {
    let outcome = protogrid_core::link::parse_links(input);
    json!({
        "servers": outcome.servers,
        "errors": outcome.errors,
    })
    .to_string()
}

fn build_config_json(
    servers_json: &str,
    active_id: &str,
    routing_json: &str,
    clash_api_secret: &str,
) -> Result<String, String> {
    let servers: Vec<Server> =
        serde_json::from_str(servers_json).map_err(|error| format!("профили: {error}"))?;
    if servers.is_empty() {
        return Err("нет сохранённых серверов".into());
    }
    let active = if active_id.is_empty() {
        servers[0].id.clone()
    } else {
        active_id.to_string()
    };

    let routing: RoutingInput = serde_json::from_str(routing_json).unwrap_or_default();
    let (rules, final_action, preset) = routing_for(&routing);
    let rules: Vec<RoutingRule> = rules
        .into_iter()
        .filter(|rule| rule.kind != RuleKind::ProcessName)
        .collect();
    let settings = Settings {
        final_action,
        routing_preset: Some(preset),
        clash_api_secret: clash_api_secret.to_string(),
        ..android_settings()
    };
    let rule_set_dir = if routing.rule_set_dir.trim().is_empty() {
        None
    } else {
        Some(Path::new(routing.rule_set_dir.trim()))
    };

    let config = singbox::build_config_with_capabilities(
        &servers,
        &active,
        &settings,
        &rules,
        Some(ANDROID_CORE_VERSION),
        rule_set_dir,
        capabilities(),
    )?;
    let config = android_tweaks(config);
    serde_json::to_string(&config).map_err(|error| error.to_string())
}

fn prepare_rule_sets_json(
    routing_json: &str,
    dir: &str,
    max_age_seconds: i64,
) -> Result<String, String> {
    let routing: RoutingInput = serde_json::from_str(routing_json).unwrap_or_default();
    let (rules, _, _) = routing_for(&routing);
    let ruleset_dir = Path::new(dir);
    let mut errors = Vec::new();
    let seeded = match protogrid_core::ruleset::seed_bytes(ruleset_dir, BUNDLED_RULESETS) {
        Ok(seeded) => seeded,
        Err(error) => {
            errors.push(format!("встроенные GeoIP/GeoSite: {error}"));
            Vec::new()
        }
    };
    let sources = protogrid_core::ruleset::sources(&rules);
    if sources.is_empty() {
        return Ok(json!({
            "available": Vec::<String>::new(),
            "downloaded": 0,
            "seeded": seeded,
            "errors": errors,
        })
        .to_string());
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("tokio: {error}"))?;
    let max_age = Duration::from_secs(max_age_seconds.max(0) as u64);
    let prepared = runtime.block_on(protogrid_core::ruleset::prepare(
        &sources,
        ruleset_dir,
        max_age,
    ));
    errors.extend(prepared.errors);
    Ok(json!({
        "available": prepared.available,
        "downloaded": prepared.downloaded,
        "seeded": seeded,
        "errors": errors,
    })
    .to_string())
}

fn fetch_subscription_json(url: &str) -> Result<String, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("tokio: {error}"))?;
    let fetched = runtime.block_on(protogrid_core::subscription::fetch_subscription(url))?;
    Ok(json!({
        "servers": fetched.servers,
        "errors": fetched.errors,
        "userinfo": fetched.userinfo,
        "display_url": protogrid_core::subscription::display_url(url),
    })
    .to_string())
}

fn new_string(env: &mut JNIEnv<'_>, value: String) -> jstring {
    match env.new_string(value) {
        Ok(string) => string.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn error_json(message: impl Into<String>) -> String {
    json!({ "error": message.into() }).to_string()
}

/// Kotlin: `CoreBridge.setLiteMode(enabled: Boolean): Unit`
#[no_mangle]
pub extern "system" fn Java_app_protogrid_client_core_CoreBridge_setLiteMode<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
    enabled: jboolean,
) {
    LITE_MODE.store(enabled != 0, Ordering::Relaxed);
}

/// Kotlin: `CoreBridge.parseLinks(input: String): String`
#[no_mangle]
pub extern "system" fn Java_app_protogrid_client_core_CoreBridge_parseLinks<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    input: JString<'local>,
) -> jstring {
    let input: String = match env.get_string(&input) {
        Ok(value) => value.into(),
        Err(error) => return new_string(&mut env, error_json(format!("JNI: {error}"))),
    };
    let result = catch_unwind(AssertUnwindSafe(|| parse_links_json(&input)))
        .unwrap_or_else(|_| error_json("внутренняя ошибка разбора ссылок"));
    new_string(&mut env, result)
}

/// Kotlin: `CoreBridge.fetchSubscription(url: String): String`
#[no_mangle]
pub extern "system" fn Java_app_protogrid_client_core_CoreBridge_fetchSubscription<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    url: JString<'local>,
) -> jstring {
    let url: String = match env.get_string(&url) {
        Ok(value) => value.into(),
        Err(error) => return new_string(&mut env, error_json(format!("JNI: {error}"))),
    };
    let result = catch_unwind(AssertUnwindSafe(|| fetch_subscription_json(&url)))
        .unwrap_or_else(|_| Err("внутренняя ошибка загрузки подписки".into()));
    new_string(
        &mut env,
        match result {
            Ok(status) => status,
            Err(error) => error_json(error),
        },
    )
}

/// Kotlin: `CoreBridge.buildConfig(serversJson: String, activeId: String, routingJson: String, apiSecret: String): String`
#[no_mangle]
pub extern "system" fn Java_app_protogrid_client_core_CoreBridge_buildConfig<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    servers_json: JString<'local>,
    active_id: JString<'local>,
    routing_json: JString<'local>,
    api_secret: JString<'local>,
) -> jstring {
    let servers_json: String = match env.get_string(&servers_json) {
        Ok(value) => value.into(),
        Err(error) => return new_string(&mut env, error_json(format!("JNI: {error}"))),
    };
    let active_id: String = match env.get_string(&active_id) {
        Ok(value) => value.into(),
        Err(error) => return new_string(&mut env, error_json(format!("JNI: {error}"))),
    };
    let routing_json: String = match env.get_string(&routing_json) {
        Ok(value) => value.into(),
        Err(error) => return new_string(&mut env, error_json(format!("JNI: {error}"))),
    };
    let api_secret: String = match env.get_string(&api_secret) {
        Ok(value) => value.into(),
        Err(error) => return new_string(&mut env, error_json(format!("JNI: {error}"))),
    };
    let result = catch_unwind(AssertUnwindSafe(|| {
        build_config_json(&servers_json, &active_id, &routing_json, &api_secret)
    }))
    .unwrap_or_else(|_| Err("внутренняя ошибка сборки конфигурации".into()));
    new_string(
        &mut env,
        match result {
            Ok(config) => config,
            Err(error) => error_json(error),
        },
    )
}

/// Kotlin: `CoreBridge.prepareRuleSets(routingJson: String, dir: String, maxAgeSeconds: Long): String`
#[no_mangle]
pub extern "system" fn Java_app_protogrid_client_core_CoreBridge_prepareRuleSets<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    routing_json: JString<'local>,
    dir: JString<'local>,
    max_age_seconds: jlong,
) -> jstring {
    let routing_json: String = match env.get_string(&routing_json) {
        Ok(value) => value.into(),
        Err(error) => return new_string(&mut env, error_json(format!("JNI: {error}"))),
    };
    let dir: String = match env.get_string(&dir) {
        Ok(value) => value.into(),
        Err(error) => return new_string(&mut env, error_json(format!("JNI: {error}"))),
    };
    let result = catch_unwind(AssertUnwindSafe(|| {
        prepare_rule_sets_json(&routing_json, &dir, max_age_seconds)
    }))
    .unwrap_or_else(|_| Err("внутренняя ошибка загрузки GeoIP/GeoSite".into()));
    new_string(
        &mut env,
        match result {
            Ok(status) => status,
            Err(error) => error_json(error),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_servers() -> String {
        let outcome = protogrid_core::link::parse_links(
            "vless://11111111-1111-1111-1111-111111111111@example.com:443?security=tls&sni=example.com#Test",
        );
        assert_eq!(outcome.errors, Vec::<String>::new());
        assert_eq!(outcome.servers.len(), 1);
        serde_json::to_string(&outcome.servers).unwrap()
    }

    #[test]
    fn parses_and_builds_android_config() {
        let servers = sample_servers();
        let active = serde_json::from_str::<Vec<Server>>(&servers).unwrap()[0]
            .id
            .clone();
        let config =
            build_config_json(&servers, &active, r#"{"preset":"global"}"#, "test-secret").unwrap();
        let config: Value = serde_json::from_str(&config).unwrap();
        let inbounds = config["inbounds"].as_array().unwrap();
        assert!(inbounds.iter().all(|inbound| inbound["tag"] != "mixed-in"));
        let tun = inbounds
            .iter()
            .find(|inbound| inbound["type"] == "tun")
            .unwrap();
        assert_eq!(tun["stack"], "gvisor");
        assert_eq!(tun["strict_route"], false);
        assert_eq!(config["route"]["auto_detect_interface"], true);
        assert_eq!(config["experimental"]["clash_api"]["secret"], "test-secret");
    }

    #[test]
    fn applies_custom_routing_rules() {
        let servers = sample_servers();
        let active = serde_json::from_str::<Vec<Server>>(&servers).unwrap()[0]
            .id
            .clone();
        let routing = r#"{"preset":"custom","final_action":"proxy","rules":[{"id":"r1","name":"RU","enabled":true,"action":"direct","kind":"domain_suffix","value":".ru"}]}"#;
        let config = build_config_json(&servers, &active, routing, "test-secret").unwrap();
        let config: Value = serde_json::from_str(&config).unwrap();
        let rules = config["route"]["rules"].as_array().unwrap();
        assert!(rules
            .iter()
            .any(|rule| rule["domain_suffix"] == json!([".ru"]) && rule["outbound"] == "direct"));
    }

    #[test]
    fn builds_local_geo_rule_sets() {
        let servers = sample_servers();
        let active = serde_json::from_str::<Vec<Server>>(&servers).unwrap()[0]
            .id
            .clone();
        let dir = std::env::temp_dir().join(format!("protogrid-geo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("geoip-ru.srs"), b"fake-rule-set").unwrap();

        let routing = json!({
            "preset": "custom",
            "final_action": "proxy",
            "rule_set_dir": dir.to_string_lossy(),
            "rules": [
                {"id": "g1", "enabled": true, "action": "direct", "kind": "geoip", "value": "ru"}
            ]
        })
        .to_string();
        let config = build_config_json(&servers, &active, &routing, "test-secret").unwrap();
        let config: Value = serde_json::from_str(&config).unwrap();
        let sets = config["route"]["rule_set"].as_array().unwrap();
        assert!(sets.iter().any(|set| {
            set["type"] == "local"
                && set["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("geoip-ru.srs"))
        }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[ignore = "network: downloads geoip-ru.srs from GitHub"]
    fn downloads_geoip_rule_set() {
        let dir = std::env::temp_dir().join(format!("protogrid-geo-dl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let status = prepare_rule_sets_json(
            r#"{"preset":"custom","final_action":"proxy","rules":[{"id":"g","enabled":true,"action":"direct","kind":"geoip","value":"ru"}]}"#,
            &dir.to_string_lossy(),
            0,
        )
        .unwrap();
        let status: Value = serde_json::from_str(&status).unwrap();
        assert!(
            status["available"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tag| tag == "geoip-ru"),
            "{status}"
        );
        assert!(dir.join("geoip-ru.srs").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_rule_set_prepare_is_offline() {
        let dir = std::env::temp_dir().join(format!("protogrid-geo-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let status = prepare_rule_sets_json(
            r#"{"preset":"custom","final_action":"proxy","rules":[]}"#,
            &dir.to_string_lossy(),
            0,
        )
        .unwrap();
        let status: Value = serde_json::from_str(&status).unwrap();
        assert_eq!(status["downloaded"], 0);
        assert_eq!(status["errors"], json!([]));
        // Even with no custom Geo rules, the bundled RU seed is in the cache.
        assert!(dir.join("geoip-ru.srs").is_file());
        assert!(dir.join("geosite-category-ru.srs").is_file());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn bypass_preset_includes_ru_geo_rules_without_process_rules() {
        let input = RoutingInput {
            preset: "bypass".into(),
            ..Default::default()
        };
        let (rules, final_action, preset) = routing_for(&input);
        assert_eq!(final_action, RuleAction::Proxy);
        assert_eq!(preset, RoutingPreset::Bypass);
        assert!(rules
            .iter()
            .any(|rule| rule.kind == RuleKind::Geoip && rule.value == "ru"));
        assert!(rules
            .iter()
            .any(|rule| rule.kind == RuleKind::Geosite && rule.value == "category-ru"));
        assert!(rules.iter().any(|rule| {
            rule.id == "default-forced-proxy"
                && rule.kind == RuleKind::Domain
                && rule.action == RuleAction::Proxy
                && rule.value.contains("web.telegram.ru")
                && rule.value.contains("instagram.ru")
        }));
        assert!(!rules.iter().any(|rule| rule.kind == RuleKind::ProcessName));
        assert!(!rules.iter().any(|rule| rule.id == "default-ads"));
    }

    #[test]
    fn lite_capabilities_only_allow_vless_tcp() {
        let lite = capabilities_for(true);
        assert!(lite.vless_only);
        assert!(lite.mldsa65);
        assert!(!lite.xhttp);
        assert!(!lite.openvpn);

        let full = capabilities_for(false);
        assert!(!full.vless_only);
        assert!(full.xhttp);
        assert!(full.openvpn);
    }

    #[test]
    fn bypass_preset_uses_geo_rule_sets() {
        let servers = sample_servers();
        let active = serde_json::from_str::<Vec<Server>>(&servers).unwrap()[0]
            .id
            .clone();
        let config =
            build_config_json(&servers, &active, r#"{"preset":"bypass"}"#, "test-secret").unwrap();
        let config: Value = serde_json::from_str(&config).unwrap();
        let sets = config["route"]["rule_set"].as_array().unwrap();
        assert!(sets.iter().any(|set| set["tag"] == "geoip-ru"));
        assert!(sets.iter().any(|set| set["tag"] == "geosite-category-ru"));
        assert!(config["route"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .any(|rule| rule["domain_suffix"] == json!([".ru", ".su", ".рф"])));
        assert!(config["route"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |rule| rule["domain"] == json!(["web.telegram.ru", "instagram.ru"])
                    && rule["outbound"] == "proxy"
            ));
    }
}
