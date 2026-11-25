use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use protogrid_core::link::{display_error_list, parse_links};
use protogrid_core::model::{
    routing_preset, AppConfig, CoreCapabilities, CoreKind, RoutingRule, RuleAction, Server,
    Settings, Subscription,
};
use protogrid_core::{ping, ruleset, singbox, subscription, sysproxy, xray};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::sync::Semaphore;

use crate::autostart;
use crate::installer::{self, CoreInfo};

pub const MAX_LOG_LINES: usize = 800;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnStatus {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Error,
}

impl ConnStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Disconnected => "Отключено",
            Self::Connecting => "Подключение",
            Self::Connected => "Подключено",
            Self::Error => "Ошибка",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warn,
    Error,
}

#[derive(Debug, Clone)]
pub struct ToastEvent {
    pub text: String,
    pub kind: ToastKind,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub ts: i64,
    pub source: String,
    pub level: String,
    pub line: String,
}

/// Live traffic counters sampled once per second while a core is running.
#[derive(Debug, Clone, Copy, Default)]
pub struct TrafficStats {
    pub down_bps: u64,
    pub up_bps: u64,
    pub conns: u32,
    pub proxy_down_bps: u64,
    pub proxy_up_bps: u64,
    pub direct_down_bps: u64,
    pub direct_up_bps: u64,
}

pub struct Inner {
    pub config: AppConfig,
    pub status: ConnStatus,
    pub error: Option<String>,
    pub warning: Option<String>,
    pub logs: VecDeque<LogEntry>,
    pub pings: HashMap<String, Option<u32>>,
    pub loading_pings: HashSet<String>,
    pub busy: HashSet<String>,
    pub child: Option<tokio::process::Child>,
    pub pid: Option<u32>,
    pub connected_at: Option<i64>,
    pub active_core: Option<CoreKind>,
    pub run_id: u64,
    pub proxy_set: bool,
    pub data_dir: PathBuf,
    pub config_path: PathBuf,
    pub core_singbox: Option<CoreInfo>,
    pub core_xray: Option<CoreInfo>,
    pub autostart: bool,
    pub proxy_supported: bool,
    pub toasts: Vec<ToastEvent>,
    pub revision: u64,
    pub traffic: TrafficStats,
}

impl Inner {
    fn touch(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    fn persist(&self) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.config)
            .map_err(|error| format!("не удалось сериализовать настройки: {error}"))?;
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("не удалось создать папку настроек: {error}"))?;
        }
        let tmp = self.config_path.with_extension("json.tmp");
        std::fs::write(&tmp, json)
            .map_err(|error| format!("не удалось записать настройки: {error}"))?;
        #[cfg(windows)]
        {
            if self.config_path.exists() {
                let _ = std::fs::remove_file(&self.config_path);
            }
        }
        std::fs::rename(&tmp, &self.config_path)
            .map_err(|error| format!("не удалось сохранить настройки: {error}"))?;
        Ok(())
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            config: self.config.clone(),
            status: self.status,
            error: self.error.clone(),
            warning: self.warning.clone(),
            logs: self.logs.iter().cloned().collect(),
            pings: self.pings.clone(),
            loading_pings: self.loading_pings.clone(),
            busy: self.busy.clone(),
            core_singbox: self.core_singbox.clone(),
            core_xray: self.core_xray.clone(),
            autostart: self.autostart,
            proxy_supported: self.proxy_supported,
            pid: self.pid,
            connected_at: self.connected_at,
            data_dir: self.data_dir.to_string_lossy().to_string(),
            config_path: self.config_path.to_string_lossy().to_string(),
            traffic: self.traffic,
        }
    }
}

#[derive(Clone)]
pub struct Snapshot {
    pub config: AppConfig,
    pub status: ConnStatus,
    pub error: Option<String>,
    pub warning: Option<String>,
    pub logs: Vec<LogEntry>,
    pub pings: HashMap<String, Option<u32>>,
    pub loading_pings: HashSet<String>,
    pub busy: HashSet<String>,
    pub core_singbox: Option<CoreInfo>,
    pub core_xray: Option<CoreInfo>,
    pub autostart: bool,
    pub proxy_supported: bool,
    pub pid: Option<u32>,
    pub connected_at: Option<i64>,
    pub data_dir: String,
    pub config_path: String,
    pub traffic: TrafficStats,
}

impl Snapshot {
    pub fn core(&self, kind: CoreKind) -> Option<&CoreInfo> {
        match kind {
            CoreKind::Singbox => self.core_singbox.as_ref(),
            CoreKind::Xray => self.core_xray.as_ref(),
        }
    }

    pub fn supports_xhttp(&self, kind: CoreKind) -> bool {
        self.core(kind)
            .is_some_and(installer::CoreInfo::supports_xhttp)
    }

    pub fn supports_kcp(&self, kind: CoreKind) -> bool {
        self.core(kind)
            .is_some_and(installer::CoreInfo::supports_kcp)
    }

    pub fn supports_xdrive(&self, kind: CoreKind) -> bool {
        self.core(kind)
            .is_some_and(installer::CoreInfo::supports_xdrive)
    }

    pub fn capabilities(&self, kind: CoreKind) -> CoreCapabilities {
        CoreCapabilities {
            xhttp: self.supports_xhttp(kind),
            kcp: self.supports_kcp(kind),
            xdrive: self.supports_xdrive(kind),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.status == ConnStatus::Connected
    }

    pub fn is_connecting(&self) -> bool {
        self.status == ConnStatus::Connecting
    }

    pub fn active_server(&self) -> Option<&Server> {
        self.config.active_server()
    }

    pub fn server(&self, id: &str) -> Option<&Server> {
        self.config.server(id)
    }

    pub fn subscription(&self, id: &str) -> Option<&Subscription> {
        self.config.subscriptions.iter().find(|sub| sub.id == id)
    }

    pub fn busy(&self, key: &str) -> bool {
        self.busy.contains(key)
    }
}

pub struct Engine {
    inner: Arc<Mutex<Inner>>,
    runtime: tokio::runtime::Runtime,
    ctx: egui::Context,
}

/// Copies the profile from the pre-rename data directory (`app.rast.client`)
/// the first time ProtoGrid starts, so subscriptions and cores are kept.
fn migrate_legacy_profile(data_root: &Path, data_dir: &Path) {
    let legacy = data_root.join("app.rast.client");
    if data_dir.join("config.json").exists() || !legacy.exists() {
        return;
    }
    let _ = copy_dir_all(&legacy, data_dir);
}

fn copy_dir_all(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let target = to.join(entry.file_name());
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => {
                let _ = copy_dir_all(&path, &target);
            }
            Ok(_) => {
                let _ = std::fs::copy(&path, &target);
            }
            Err(_) => {}
        }
    }
    Ok(())
}

impl Engine {
    pub fn new(ctx: egui::Context) -> Self {
        let data_root = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        let data_dir = data_root.join("app.protogrid.client");
        migrate_legacy_profile(&data_root, &data_dir);
        let _ = std::fs::create_dir_all(data_dir.join("runtime"));
        let _ = std::fs::create_dir_all(data_dir.join("cores"));
        let _ = std::fs::create_dir_all(data_dir.join("rulesets"));
        let config_path = data_dir.join("config.json");

        let mut config = if config_path.exists() {
            std::fs::read_to_string(&config_path)
                .ok()
                .and_then(|text| serde_json::from_str::<AppConfig>(&text).ok())
                .unwrap_or_default()
        } else {
            AppConfig::default()
        };
        config.normalize();

        let inner = Arc::new(Mutex::new(Inner {
            config,
            status: ConnStatus::Disconnected,
            error: None,
            warning: None,
            logs: VecDeque::new(),
            pings: HashMap::new(),
            loading_pings: HashSet::new(),
            busy: HashSet::new(),
            child: None,
            pid: None,
            connected_at: None,
            active_core: None,
            run_id: 0,
            proxy_set: false,
            data_dir,
            config_path,
            core_singbox: None,
            core_xray: None,
            autostart: autostart::is_enabled(),
            proxy_supported: sysproxy::is_supported(),
            toasts: Vec::new(),
            revision: 0,
            traffic: TrafficStats::default(),
        }));
        {
            let guard = inner.lock().unwrap_or_else(|e| e.into_inner());
            let _ = guard.persist();
        }

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("protogrid-async")
            .build()
            .expect("не удалось создать async runtime");

        let engine = Self {
            inner,
            runtime,
            ctx,
        };
        engine.bootstrap();
        engine
    }

    pub fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn snapshot_if_changed(&self, last_revision: &mut u64) -> Option<Snapshot> {
        let guard = self.lock();
        if guard.revision == *last_revision {
            return None;
        }
        *last_revision = guard.revision;
        Some(guard.snapshot())
    }

    pub fn drain_toasts(&self) -> Vec<ToastEvent> {
        let mut guard = self.lock();
        if guard.toasts.is_empty() {
            return Vec::new();
        }
        let toasts = std::mem::take(&mut guard.toasts);
        guard.touch();
        self.ctx.request_repaint();
        toasts
    }

    fn spawn<F>(&self, future: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.runtime.spawn(future);
    }

    pub fn set_busy(&self, key: &str, busy: bool) {
        let mut guard = self.lock();
        if busy {
            guard.busy.insert(key.to_string());
        } else {
            guard.busy.remove(key);
        }
        guard.touch();
        drop(guard);
        self.ctx.request_repaint();
    }

    pub fn bootstrap(&self) {
        let inner = self.inner.clone();
        let ctx = self.ctx.clone();
        self.spawn(async move {
            push_log(
                &inner,
                &ctx,
                "app",
                Some("info"),
                format!(
                    "ProtoGrid {} · нативный egui · {}",
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::OS
                ),
            );
            detect_both(&inner, &ctx).await;
            log_core_versions(&inner, &ctx);
            let (auto_connect, has_active) = {
                let guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                (
                    guard.config.settings.auto_connect,
                    guard.config.active_server_id.is_some(),
                )
            };
            if auto_connect && has_active {
                tokio::time::sleep(Duration::from_millis(400)).await;
                if let Err(error) = connect_core(&inner, &ctx).await {
                    push_log(&inner, &ctx, "app", Some("warn"), error);
                }
            }
            refresh_stale_subscriptions(&inner, &ctx).await;
        });
    }

    pub fn refresh_cores(&self) {
        if self.lock().busy.contains("cores") {
            return;
        }
        self.set_busy("cores", true);
        let inner = self.inner.clone();
        let ctx = self.ctx.clone();
        self.spawn(async move {
            detect_both(&inner, &ctx).await;
            log_core_versions(&inner, &ctx);
            clear_busy(&inner, &ctx, "cores");
        });
    }

    pub fn install_core(&self, kind: CoreKind) {
        let key = format!("install-{kind:?}");
        if self.lock().busy.contains(&key) {
            return;
        }
        self.set_busy(&key, true);
        let inner = self.inner.clone();
        let ctx = self.ctx.clone();
        self.spawn(async move {
            match installer::install(kind, &inner, &ctx).await {
                Ok(info) => {
                    {
                        let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                        match kind {
                            CoreKind::Singbox => {
                                if let Some(path) = &info.path {
                                    guard.config.settings.singbox_path = path.clone();
                                }
                                guard.core_singbox = Some(info.clone());
                            }
                            CoreKind::Xray => {
                                if let Some(path) = &info.path {
                                    guard.config.settings.xray_path = path.clone();
                                }
                                guard.core_xray = Some(info.clone());
                            }
                        }
                        let _ = guard.persist();
                        guard.touch();
                    }
                    push_log(
                        &inner,
                        &ctx,
                        "app",
                        Some("info"),
                        format!(
                            "{} {} установлен",
                            kind.name(),
                            info.version.unwrap_or_default()
                        ),
                    );
                    push_toast(
                        &inner,
                        &ctx,
                        format!("{} установлен", kind.name()),
                        ToastKind::Success,
                    );
                }
                Err(error) => {
                    push_log(&inner, &ctx, "app", Some("error"), error.clone());
                    push_toast(&inner, &ctx, error, ToastKind::Error);
                }
            }
            clear_busy(&inner, &ctx, &key);
        });
    }

    pub fn connect(&self) {
        if self.lock().busy.contains("connection") {
            return;
        }
        self.set_busy("connection", true);
        {
            let mut guard = self.lock();
            guard.status = ConnStatus::Connecting;
            guard.error = None;
            guard.warning = None;
            guard.touch();
        }
        self.ctx.request_repaint();
        let inner = self.inner.clone();
        let ctx = self.ctx.clone();
        self.spawn(async move {
            match connect_core(&inner, &ctx).await {
                Ok(()) => push_toast(&inner, &ctx, "Соединение установлено", ToastKind::Success),
                Err(error) => push_toast(&inner, &ctx, error, ToastKind::Error),
            }
            clear_busy(&inner, &ctx, "connection");
        });
    }

    pub fn disconnect(&self) {
        if self.lock().busy.contains("connection") {
            return;
        }
        self.set_busy("connection", true);
        let inner = self.inner.clone();
        let ctx = self.ctx.clone();
        self.spawn(async move {
            stop_core(&inner, &ctx, false).await;
            push_toast(&inner, &ctx, "Соединение отключено", ToastKind::Info);
            clear_busy(&inner, &ctx, "connection");
        });
    }

    pub fn save_settings(&self, settings: Settings) {
        let mut settings = settings;
        settings.sanitize();
        let (was_connected, old_settings) = {
            let mut guard = self.lock();
            let old = guard.config.settings.clone();
            let connected = matches!(guard.status, ConnStatus::Connected | ConnStatus::Connecting);
            guard.config.settings = settings.clone();
            match guard.persist() {
                Ok(()) => {}
                Err(error) => {
                    push_toast(&self.inner, &self.ctx, error, ToastKind::Error);
                    return;
                }
            }
            guard.touch();
            (connected, old)
        };
        self.ctx.request_repaint();
        if was_connected {
            if old_settings.network_eq(&settings) {
                if old_settings.system_proxy != settings.system_proxy {
                    self.apply_proxy(settings.system_proxy);
                }
                push_toast(
                    &self.inner,
                    &self.ctx,
                    "Настройки сохранены",
                    ToastKind::Success,
                );
            } else {
                self.connect();
                push_toast(
                    &self.inner,
                    &self.ctx,
                    "Настройки сохранены, соединение перезапускается",
                    ToastKind::Success,
                );
            }
        } else {
            push_toast(
                &self.inner,
                &self.ctx,
                "Настройки сохранены",
                ToastKind::Success,
            );
        }
    }

    pub fn apply_proxy(&self, enabled: bool) {
        let settings = self.lock().config.settings.clone();
        let inner = self.inner.clone();
        let ctx = self.ctx.clone();
        self.spawn(async move {
            let mut warning = None;
            let mut proxy_set = false;
            if enabled {
                match sysproxy::enable(
                    settings.effective_http_port(),
                    settings.effective_socks_port(),
                ) {
                    Ok(()) => proxy_set = true,
                    Err(error) => warning = Some(format!("Системный прокси: {error}")),
                }
            } else if let Err(error) = sysproxy::disable() {
                warning = Some(format!("Системный прокси: {error}"));
            }
            {
                let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                guard.proxy_set = proxy_set;
                guard.warning = warning;
                guard.touch();
            }
            ctx.request_repaint();
        });
    }

    pub fn save_server(&self, server: Server) {
        let mut server = server;
        if server.id.trim().is_empty() {
            server.id = uuid::Uuid::new_v4().to_string();
        }
        if server.name.trim().is_empty() {
            server.name = server.target();
        }
        let (was_active, was_connected) = {
            let mut guard = self.lock();
            let was_active = guard.config.active_server_id.as_deref() == Some(server.id.as_str());
            let was_connected =
                matches!(guard.status, ConnStatus::Connected | ConnStatus::Connecting);
            if let Some(existing) = guard.config.servers.iter_mut().find(|s| s.id == server.id) {
                *existing = server.clone();
            } else {
                guard.config.servers.push(server.clone());
            }
            if guard.config.active_server_id.is_none() {
                guard.config.active_server_id = Some(server.id.clone());
            }
            let error = guard.persist().err();
            guard.touch();
            drop(guard);
            if let Some(error) = error {
                push_toast(&self.inner, &self.ctx, error, ToastKind::Error);
                return;
            }
            (was_active, was_connected)
        };
        self.ctx.request_repaint();
        if was_active && was_connected {
            self.connect();
        }
        push_toast(
            &self.inner,
            &self.ctx,
            "Сервер сохранён",
            ToastKind::Success,
        );
    }

    pub fn delete_server(&self, id: &str) {
        let (was_active, was_connected) = {
            let mut guard = self.lock();
            let was_active = guard.config.active_server_id.as_deref() == Some(id);
            guard.config.servers.retain(|server| server.id != id);
            if was_active {
                guard.config.active_server_id =
                    guard.config.servers.first().map(|server| server.id.clone());
            }
            guard.config.normalize();
            let error = guard.persist().err();
            guard.touch();
            drop(guard);
            if let Some(error) = error {
                push_toast(&self.inner, &self.ctx, error, ToastKind::Error);
                return;
            }
            let connected = matches!(
                self.lock().status,
                ConnStatus::Connected | ConnStatus::Connecting
            );
            (was_active, connected)
        };
        self.ctx.request_repaint();
        if was_active && was_connected {
            self.restart_or_stop();
        }
        push_toast(&self.inner, &self.ctx, "Сервер удалён", ToastKind::Success);
    }

    pub fn set_active_server(&self, id: &str) {
        let connected = {
            let mut guard = self.lock();
            if !guard.config.servers.iter().any(|server| server.id == id) {
                return;
            }
            guard.config.active_server_id = Some(id.to_string());
            let error = guard.persist().err();
            guard.touch();
            drop(guard);
            if let Some(error) = error {
                push_toast(&self.inner, &self.ctx, error, ToastKind::Error);
                return;
            }
            matches!(
                self.lock().status,
                ConnStatus::Connected | ConnStatus::Connecting
            )
        };
        self.ctx.request_repaint();
        if connected {
            self.connect();
        }
    }

    pub fn import_links(&self, text: &str) -> Result<usize, String> {
        let outcome = parse_links(text);
        if outcome.servers.is_empty() {
            let message = if outcome.errors.is_empty() {
                "не найдено ни одной ссылки".to_string()
            } else {
                display_error_list(&outcome.errors)
            };
            return Err(message);
        }
        let added = outcome.servers.len();
        let was_connected = self.lock().status == ConnStatus::Connected;
        {
            let mut guard = self.lock();
            guard.config.servers.extend(outcome.servers);
            if guard.config.active_server_id.is_none() {
                guard.config.active_server_id =
                    guard.config.servers.first().map(|server| server.id.clone());
            }
            guard.persist()?;
            guard.touch();
        }
        self.ctx.request_repaint();
        if was_connected {
            self.connect();
        }
        Ok(added)
    }

    pub fn add_subscription(&self, name: String, url: String) {
        if self.lock().busy.contains("sub-add") {
            return;
        }
        self.set_busy("sub-add", true);
        let inner = self.inner.clone();
        let ctx = self.ctx.clone();
        self.spawn(async move {
            match subscription::fetch_subscription(&url).await {
                Ok(fetched) => {
                    let id = uuid::Uuid::new_v4().to_string();
                    let count = fetched.servers.len();
                    {
                        let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                        guard.config.subscriptions.push(Subscription {
                            id: id.clone(),
                            name: if name.trim().is_empty() {
                                url.trim()
                                    .trim_start_matches("https://")
                                    .trim_start_matches("http://")
                                    .split('/')
                                    .next()
                                    .unwrap_or("Подписка")
                                    .to_string()
                            } else {
                                name.trim().to_string()
                            },
                            url,
                            auto_update: true,
                            last_updated: Some(now_secs()),
                            userinfo: fetched.userinfo,
                            last_error: None,
                        });
                        for mut server in fetched.servers {
                            server.subscription_id = Some(id.clone());
                            guard.config.servers.push(server);
                        }
                        if guard.config.active_server_id.is_none() {
                            guard.config.active_server_id =
                                guard.config.servers.first().map(|s| s.id.clone());
                        }
                        let _ = guard.persist();
                        guard.touch();
                    }
                    push_log(
                        &inner,
                        &ctx,
                        "app",
                        Some("info"),
                        format!("Подписка добавлена: {count} серверов"),
                    );
                    push_toast(
                        &inner,
                        &ctx,
                        format!("Подписка добавлена: {count} серверов"),
                        ToastKind::Success,
                    );
                }
                Err(error) => {
                    push_log(&inner, &ctx, "app", Some("warn"), error.clone());
                    push_toast(&inner, &ctx, error, ToastKind::Error);
                }
            }
            clear_busy(&inner, &ctx, "sub-add");
        });
    }

    pub fn refresh_subscription(&self, id: String) {
        let key = format!("sub-{id}");
        if self.lock().busy.contains(&key) {
            return;
        }
        self.set_busy(&key, true);
        let inner = self.inner.clone();
        let ctx = self.ctx.clone();
        self.spawn(async move {
            match refresh_subscription_inner(&inner, &ctx, &id).await {
                Ok(()) => push_toast(&inner, &ctx, "Подписка обновлена", ToastKind::Success),
                Err(error) => {
                    push_toast(&inner, &ctx, error, ToastKind::Error);
                }
            }
            clear_busy(&inner, &ctx, &key);
        });
    }

    pub fn set_subscription_auto_update(&self, id: &str, enabled: bool) {
        let mut guard = self.lock();
        if let Some(sub) = guard
            .config
            .subscriptions
            .iter_mut()
            .find(|sub| sub.id == id)
        {
            sub.auto_update = enabled;
            let _ = guard.persist();
            guard.touch();
            drop(guard);
            self.ctx.request_repaint();
        }
    }

    pub fn delete_subscription(&self, id: &str) {
        let (active_changed, was_connected) = {
            let mut guard = self.lock();
            let active_before = guard.config.active_server_id.clone();
            guard.config.subscriptions.retain(|sub| sub.id != id);
            guard
                .config
                .servers
                .retain(|server| server.subscription_id.as_deref() != Some(id));
            guard.config.normalize();
            let changed = guard.config.active_server_id != active_before;
            let error = guard.persist().err();
            guard.touch();
            drop(guard);
            if let Some(error) = error {
                push_toast(&self.inner, &self.ctx, error, ToastKind::Error);
                return;
            }
            let connected = matches!(
                self.lock().status,
                ConnStatus::Connected | ConnStatus::Connecting
            );
            (changed, connected)
        };
        self.ctx.request_repaint();
        if was_connected && active_changed {
            self.restart_or_stop();
        }
        push_toast(
            &self.inner,
            &self.ctx,
            "Подписка удалена",
            ToastKind::Success,
        );
    }

    pub fn apply_routing_preset(&self, preset: &str) {
        let Some((rules, final_action)) = routing_preset(preset) else {
            push_toast(
                &self.inner,
                &self.ctx,
                "Неизвестный пресет",
                ToastKind::Error,
            );
            return;
        };
        let was_connected = self.lock().status == ConnStatus::Connected;
        {
            let mut guard = self.lock();
            guard.config.rules = rules;
            guard.config.settings.final_action = final_action;
            let _ = guard.persist();
            guard.touch();
        }
        self.ctx.request_repaint();
        if was_connected {
            self.connect();
        }
        push_toast(
            &self.inner,
            &self.ctx,
            "Сценарий маршрутизации применён",
            ToastKind::Success,
        );
    }

    pub fn save_rule(&self, mut rule: RoutingRule) {
        rule.value = rule.value.trim().to_string();
        if rule.value.is_empty() {
            push_toast(
                &self.inner,
                &self.ctx,
                "Укажите значение правила",
                ToastKind::Warn,
            );
            return;
        }
        if rule.name.trim().is_empty() {
            rule.name = rule.kind.label().to_string();
        }
        if rule.id.trim().is_empty() {
            rule.id = uuid::Uuid::new_v4().to_string();
        }
        let was_connected = self.lock().status == ConnStatus::Connected;
        {
            let mut guard = self.lock();
            if let Some(existing) = guard.config.rules.iter_mut().find(|r| r.id == rule.id) {
                *existing = rule;
            } else {
                guard.config.rules.push(rule);
            }
            let _ = guard.persist();
            guard.touch();
        }
        self.ctx.request_repaint();
        if was_connected {
            self.connect();
        }
        push_toast(
            &self.inner,
            &self.ctx,
            "Правило сохранено",
            ToastKind::Success,
        );
    }

    pub fn delete_rule(&self, id: &str) {
        let was_connected = self.lock().status == ConnStatus::Connected;
        {
            let mut guard = self.lock();
            guard.config.rules.retain(|rule| rule.id != id);
            let _ = guard.persist();
            guard.touch();
        }
        self.ctx.request_repaint();
        if was_connected {
            self.connect();
        }
        push_toast(
            &self.inner,
            &self.ctx,
            "Правило удалено",
            ToastKind::Success,
        );
    }

    pub fn move_rule(&self, id: &str, direction: &str) {
        let was_connected = self.lock().status == ConnStatus::Connected;
        {
            let mut guard = self.lock();
            let Some(index) = guard.config.rules.iter().position(|rule| rule.id == id) else {
                return;
            };
            let target = match direction {
                "up" if index > 0 => index - 1,
                "down" if index + 1 < guard.config.rules.len() => index + 1,
                _ => index,
            };
            if target != index {
                guard.config.rules.swap(index, target);
                let _ = guard.persist();
                guard.touch();
            }
        }
        self.ctx.request_repaint();
        if was_connected {
            self.connect();
        }
    }

    pub fn set_final_action(&self, action: RuleAction) {
        let mut settings = self.lock().config.settings.clone();
        settings.final_action = action;
        self.save_settings(settings);
    }

    pub fn ping_all(&self) {
        if self.lock().busy.contains("ping-all") {
            return;
        }
        let (servers, settings, active_id, connected) = {
            let guard = self.lock();
            (
                guard.config.servers.clone(),
                guard.config.settings.clone(),
                guard.config.active_server_id.clone(),
                guard.status == ConnStatus::Connected,
            )
        };
        {
            let mut guard = self.lock();
            for server in &servers {
                guard.loading_pings.insert(server.id.clone());
            }
            guard.busy.insert("ping-all".into());
            guard.touch();
        }
        self.ctx.request_repaint();
        let inner = self.inner.clone();
        let ctx = self.ctx.clone();
        self.spawn(async move {
            let semaphore = Arc::new(Semaphore::new(8));
            let mut handles = Vec::new();
            for server in servers {
                let permit = semaphore.clone();
                let settings = settings.clone();
                let active_id = active_id.clone();
                handles.push(tokio::spawn(async move {
                    let _permit = permit.acquire_owned().await.ok();
                    ping_one(server, &settings, active_id.as_deref(), connected).await
                }));
            }
            for handle in handles {
                if let Ok((id, latency)) = handle.await {
                    let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                    guard.pings.insert(id.clone(), latency);
                    guard.loading_pings.remove(&id);
                    guard.touch();
                }
            }
            {
                let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                guard.busy.remove("ping-all");
                guard.touch();
            }
            ctx.request_repaint();
            push_toast(
                &inner,
                &ctx,
                "Проверка серверов завершена",
                ToastKind::Success,
            );
        });
    }

    pub fn ping_server(&self, id: &str) {
        let (server, settings, active_id, connected) = {
            let guard = self.lock();
            let Some(server) = guard.config.server(id).cloned() else {
                return;
            };
            (
                server,
                guard.config.settings.clone(),
                guard.config.active_server_id.clone(),
                guard.status == ConnStatus::Connected,
            )
        };
        {
            let mut guard = self.lock();
            guard.loading_pings.insert(id.to_string());
            guard.touch();
        }
        self.ctx.request_repaint();
        let inner = self.inner.clone();
        let ctx = self.ctx.clone();
        self.spawn(async move {
            let (id, latency) = ping_one(server, &settings, active_id.as_deref(), connected).await;
            let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
            guard.pings.insert(id.clone(), latency);
            guard.loading_pings.remove(&id);
            guard.touch();
            drop(guard);
            ctx.request_repaint();
        });
    }

    pub fn url_test(&self) {
        let (settings, connected) = {
            let guard = self.lock();
            (
                guard.config.settings.clone(),
                guard.status == ConnStatus::Connected,
            )
        };
        if !connected {
            push_toast(
                &self.inner,
                &self.ctx,
                "Сначала подключитесь",
                ToastKind::Warn,
            );
            return;
        }
        let inner = self.inner.clone();
        let ctx = self.ctx.clone();
        self.spawn(async move {
            match ping::http_proxy_test(settings.effective_http_port(), &settings.test_url, 9000)
                .await
            {
                Ok(latency) => {
                    let active_id = inner
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .config
                        .active_server_id
                        .clone();
                    if let Some(id) = active_id {
                        let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                        guard.pings.insert(id, Some(latency));
                        guard.touch();
                    }
                    push_toast(
                        &inner,
                        &ctx,
                        format!("Задержка: {latency} мс"),
                        ToastKind::Success,
                    );
                }
                Err(error) => push_toast(&inner, &ctx, error, ToastKind::Error),
            }
        });
    }

    pub fn clear_logs(&self) {
        let mut guard = self.lock();
        guard.logs.clear();
        guard.touch();
        drop(guard);
        self.ctx.request_repaint();
    }

    pub fn set_autostart(&self, enabled: bool) {
        match autostart::set_enabled(enabled) {
            Ok(value) => {
                {
                    let mut guard = self.lock();
                    guard.autostart = value;
                    guard.touch();
                }
                self.ctx.request_repaint();
                push_toast(
                    &self.inner,
                    &self.ctx,
                    if value {
                        "Автозапуск включён"
                    } else {
                        "Автозапуск отключён"
                    },
                    ToastKind::Success,
                );
            }
            Err(error) => push_toast(&self.inner, &self.ctx, error, ToastKind::Error),
        }
    }

    pub fn open_data_dir(&self) {
        let dir = self.lock().data_dir.clone();
        #[cfg(target_os = "windows")]
        let program = "explorer";
        #[cfg(target_os = "macos")]
        let program = "open";
        #[cfg(all(unix, not(target_os = "macos")))]
        let program = "xdg-open";
        if let Err(error) = std::process::Command::new(program).arg(&dir).spawn() {
            push_toast(
                &self.inner,
                &self.ctx,
                format!("Не удалось открыть папку: {error}"),
                ToastKind::Error,
            );
        }
    }

    pub fn restart_or_stop(&self) {
        if self.lock().config.active_server_id.is_some() {
            self.connect();
        } else {
            self.disconnect();
        }
    }

    pub fn shutdown_blocking(&self) {
        let (child, proxy_set) = {
            let mut guard = self.lock();
            guard.run_id = guard.run_id.wrapping_add(1);
            let child = guard.child.take();
            let proxy_set = guard.proxy_set;
            guard.proxy_set = false;
            guard.status = ConnStatus::Disconnected;
            (child, proxy_set)
        };
        if let Some(mut child) = child {
            let _ = child.start_kill();
        }
        if proxy_set {
            let _ = sysproxy::disable();
        }
    }
}

/* -------------------------------------------------------------------------- */
/* Helpers                                                                    */
/* -------------------------------------------------------------------------- */

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn now_secs() -> i64 {
    now_ms() / 1000
}

fn strip_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for code in chars.by_ref() {
                    if code.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        output.push(character);
    }
    output
}

fn level_from_line(line: &str) -> String {
    let upper = line.to_ascii_uppercase();
    if upper.contains("FATAL") || upper.contains("ERROR") || upper.contains("PANIC") {
        "error".into()
    } else if upper.contains("WARN") {
        "warn".into()
    } else if upper.contains("DEBUG") || upper.contains("TRACE") {
        "debug".into()
    } else {
        "info".into()
    }
}

pub fn push_log(
    inner: &Arc<Mutex<Inner>>,
    ctx: &egui::Context,
    source: &str,
    level: Option<&str>,
    line: impl Into<String>,
) {
    let line = strip_ansi(&line.into());
    if line.trim().is_empty() {
        return;
    }
    {
        let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.logs.push_back(LogEntry {
            ts: now_ms(),
            source: source.to_string(),
            level: level
                .map(str::to_string)
                .unwrap_or_else(|| level_from_line(&line)),
            line,
        });
        while guard.logs.len() > MAX_LOG_LINES {
            guard.logs.pop_front();
        }
        guard.touch();
    }
    ctx.request_repaint();
}

pub fn push_toast(
    inner: &Arc<Mutex<Inner>>,
    ctx: &egui::Context,
    text: impl Into<String>,
    kind: ToastKind,
) {
    let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
    guard.toasts.push(ToastEvent {
        text: text.into(),
        kind,
    });
    if guard.toasts.len() > 5 {
        guard.toasts.remove(0);
    }
    guard.touch();
    drop(guard);
    ctx.request_repaint();
}

fn clear_busy(inner: &Arc<Mutex<Inner>>, ctx: &egui::Context, key: &str) {
    let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
    guard.busy.remove(key);
    guard.touch();
    drop(guard);
    ctx.request_repaint();
}

async fn detect_both(inner: &Arc<Mutex<Inner>>, ctx: &egui::Context) {
    let (settings, data_dir) = {
        let guard = inner.lock().unwrap_or_else(|e| e.into_inner());
        (guard.config.settings.clone(), guard.data_dir.clone())
    };
    let (singbox, xray) = tokio::join!(
        installer::detect(CoreKind::Singbox, &settings, &data_dir),
        installer::detect(CoreKind::Xray, &settings, &data_dir)
    );
    {
        let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.core_singbox = Some(singbox);
        guard.core_xray = Some(xray);
        guard.touch();
    }
    ctx.request_repaint();
}

fn log_core_versions(inner: &Arc<Mutex<Inner>>, ctx: &egui::Context) {
    let (singbox, xray) = {
        let guard = inner.lock().unwrap_or_else(|e| e.into_inner());
        let singbox = guard
            .core_singbox
            .as_ref()
            .map(|info| {
                if info.found {
                    format!("sing-box {}", info.version.clone().unwrap_or_default())
                } else {
                    "sing-box не найден".into()
                }
            })
            .unwrap_or_default();
        let xray = guard
            .core_xray
            .as_ref()
            .map(|info| {
                if info.found {
                    format!("Xray {}", info.version.clone().unwrap_or_default())
                } else {
                    "Xray не найден".into()
                }
            })
            .unwrap_or_default();
        (singbox, xray)
    };
    push_log(
        inner,
        ctx,
        "app",
        Some("info"),
        format!("Ядра: {singbox} · {xray}"),
    );
}

async fn refresh_stale_subscriptions(inner: &Arc<Mutex<Inner>>, ctx: &egui::Context) {
    let stale = {
        let guard = inner.lock().unwrap_or_else(|e| e.into_inner());
        if !guard.config.settings.auto_update_subscriptions {
            Vec::new()
        } else {
            guard
                .config
                .subscriptions
                .iter()
                .filter(|sub| sub.auto_update)
                .filter(|sub| {
                    sub.last_updated
                        .map(|time| now_secs() - time > 12 * 3600)
                        .unwrap_or(false)
                })
                .map(|sub| sub.id.clone())
                .collect::<Vec<_>>()
        }
    };
    for id in stale {
        if let Err(error) = refresh_subscription_inner(inner, ctx, &id).await {
            push_log(inner, ctx, "app", Some("warn"), error);
        }
    }
}

async fn refresh_subscription_inner(
    inner: &Arc<Mutex<Inner>>,
    ctx: &egui::Context,
    id: &str,
) -> Result<(), String> {
    let (url, name) = {
        let guard = inner.lock().unwrap_or_else(|e| e.into_inner());
        let sub = guard
            .config
            .subscriptions
            .iter()
            .find(|sub| sub.id == id)
            .ok_or_else(|| "подписка не найдена".to_string())?;
        (sub.url.clone(), sub.name.clone())
    };
    let fetched = match subscription::fetch_subscription(&url).await {
        Ok(fetched) => fetched,
        Err(error) => {
            {
                let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(sub) = guard.config.subscriptions.iter_mut().find(|s| s.id == id) {
                    sub.last_error = Some(error.clone());
                }
                let _ = guard.persist();
                guard.touch();
            }
            ctx.request_repaint();
            return Err(error);
        }
    };

    let was_connected =
        inner.lock().unwrap_or_else(|e| e.into_inner()).status == ConnStatus::Connected;
    let active_before = inner
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .config
        .active_server_id
        .clone();
    {
        let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
        let index = guard
            .config
            .subscriptions
            .iter()
            .position(|sub| sub.id == id)
            .ok_or_else(|| "подписка не найдена".to_string())?;
        guard.config.subscriptions[index].last_updated = Some(now_secs());
        guard.config.subscriptions[index].userinfo = fetched.userinfo;
        guard.config.subscriptions[index].last_error = None;

        let old_servers: Vec<Server> = guard
            .config
            .servers
            .iter()
            .filter(|server| server.subscription_id.as_deref() == Some(id))
            .cloned()
            .collect();
        let mut old_by_key: HashMap<String, String> = old_servers
            .iter()
            .map(|server| (server_key(server), server.id.clone()))
            .collect();
        let mut new_servers = Vec::new();
        for mut server in fetched.servers {
            server.subscription_id = Some(id.to_string());
            if let Some(old_id) = old_by_key.remove(&server_key(&server)) {
                server.id = old_id;
            }
            new_servers.push(server);
        }
        guard
            .config
            .servers
            .retain(|server| server.subscription_id.as_deref() != Some(id));
        guard.config.servers.extend(new_servers);
        if let Some(active) = guard.config.active_server_id.clone() {
            if !guard
                .config
                .servers
                .iter()
                .any(|server| server.id == active)
            {
                guard.config.active_server_id = guard
                    .config
                    .servers
                    .iter()
                    .find(|server| server.subscription_id.as_deref() == Some(id))
                    .map(|server| server.id.clone())
                    .or_else(|| guard.config.servers.first().map(|server| server.id.clone()));
            }
        }
        guard.config.normalize();
        let _ = guard.persist();
        guard.touch();
    }
    ctx.request_repaint();
    push_log(
        inner,
        ctx,
        "app",
        Some("info"),
        format!("Подписка «{name}» обновлена"),
    );
    let active_after = inner
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .config
        .active_server_id
        .clone();
    if was_connected && active_before != active_after {
        if active_after.is_some() {
            let _ = connect_core(inner, ctx).await;
        } else {
            stop_core(inner, ctx, false).await;
        }
    }
    Ok(())
}

fn server_key(server: &Server) -> String {
    format!(
        "{:?}|{}|{}|{}|{}",
        server.protocol,
        server.address.trim().to_lowercase(),
        server.port,
        server.uuid,
        server.password
    )
}

fn set_error(inner: &Arc<Mutex<Inner>>, ctx: &egui::Context, message: String) -> String {
    {
        let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.status = ConnStatus::Error;
        guard.error = Some(message.clone());
        guard.pid = None;
        guard.connected_at = None;
        guard.active_core = None;
        guard.warning = None;
        guard.traffic = TrafficStats::default();
        guard.touch();
    }
    push_log(inner, ctx, "app", Some("error"), message.clone());
    ctx.request_repaint();
    message
}

async fn stop_core(inner: &Arc<Mutex<Inner>>, ctx: &egui::Context, silent: bool) {
    let (child, proxy_set, was_active) = {
        let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.run_id = guard.run_id.wrapping_add(1);
        let child = guard.child.take();
        let proxy_set = guard.proxy_set;
        let was_active = matches!(guard.status, ConnStatus::Connected | ConnStatus::Connecting);
        guard.proxy_set = false;
        guard.touch();
        (child, proxy_set, was_active)
    };
    if let Some(mut child) = child {
        let _ = child.start_kill();
        let _ = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;
    }
    if proxy_set {
        let _ = sysproxy::disable();
    }
    {
        let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.status = ConnStatus::Disconnected;
        guard.pid = None;
        guard.connected_at = None;
        guard.active_core = None;
        guard.error = None;
        guard.warning = None;
        guard.traffic = TrafficStats::default();
        guard.touch();
    }
    if !silent {
        if was_active {
            push_log(inner, ctx, "app", Some("info"), "Отключено");
        }
        ctx.request_repaint();
    }
}

fn spawn_log_reader<R>(inner: Arc<Mutex<Inner>>, ctx: egui::Context, run_id: u64, reader: R)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    if inner.lock().unwrap_or_else(|e| e.into_inner()).run_id != run_id {
                        break;
                    }
                    push_log(&inner, &ctx, "core", None, line);
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }
    });
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClashTraffic {
    download_total: u64,
    upload_total: u64,
    #[serde(default)]
    connections: Vec<ClashConnection>,
}

#[derive(Debug, serde::Deserialize)]
struct ClashConnection {
    #[serde(default)]
    download: u64,
    #[serde(default)]
    upload: u64,
    #[serde(default)]
    chains: Vec<String>,
}

struct SystemMeter {
    networks: Option<sysinfo::Networks>,
    rx: u64,
    tx: u64,
    at: Instant,
}

impl SystemMeter {
    fn new() -> Self {
        let networks = sysinfo::Networks::new_with_refreshed_list().ok();
        let (rx, tx) = networks.as_ref().map(system_totals).unwrap_or((0, 0));
        Self {
            networks,
            rx,
            tx,
            at: Instant::now(),
        }
    }

    fn sample(&mut self) -> (u64, u64) {
        let Some(networks) = self.networks.as_mut() else {
            return (0, 0);
        };
        networks.refresh(true);
        let (rx, tx) = system_totals(networks);
        let elapsed = self.at.elapsed().as_secs_f64().max(0.2);
        let down = rx.saturating_sub(self.rx);
        let up = tx.saturating_sub(self.tx);
        self.rx = rx;
        self.tx = tx;
        self.at = Instant::now();
        ((down as f64 / elapsed) as u64, (up as f64 / elapsed) as u64)
    }
}

fn system_totals(networks: &sysinfo::Networks) -> (u64, u64) {
    let mut rx = 0_u64;
    let mut tx = 0_u64;
    for (name, data) in networks {
        let name = name.to_string_lossy();
        if name == "lo"
            || name.starts_with("Loopback")
            || name.contains("Loopback Pseudo-Interface")
        {
            continue;
        }
        rx = rx.saturating_add(data.total_received());
        tx = tx.saturating_add(data.total_transmitted());
    }
    (rx, tx)
}

async fn clash_traffic(client: &reqwest::Client, port: u16) -> Result<ClashTraffic, String> {
    let endpoint = format!("http://127.0.0.1:{port}/connections");
    let response = client
        .get(endpoint)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    response
        .json::<ClashTraffic>()
        .await
        .map_err(|error| error.to_string())
}

fn spawn_traffic_monitor(inner: Arc<Mutex<Inner>>, ctx: egui::Context, run_id: u64) {
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_millis(1500))
            .build()
            .ok();
        let mut system = SystemMeter::new();
        let mut prev_clash: Option<(u64, u64)> = None;
        loop {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            let snapshot = {
                let guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                if guard.run_id != run_id || guard.status != ConnStatus::Connected {
                    if guard.traffic.conns != 0
                        || guard.traffic.down_bps != 0
                        || guard.traffic.up_bps != 0
                    {
                        drop(guard);
                        let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                        guard.traffic = TrafficStats::default();
                        guard.touch();
                        ctx.request_repaint();
                    }
                    return;
                }
                (
                    guard.config.settings.core,
                    guard.config.settings.clash_api_port,
                    guard.config.settings.system_proxy || guard.config.settings.tun_enabled,
                )
            };
            let (core, port, proxied) = snapshot;
            let (sys_down, sys_up) = system.sample();
            let mut stats = TrafficStats::default();
            let mut used_clash = false;
            if core == CoreKind::Singbox {
                if let Some(client) = client.as_ref() {
                    if let Ok(data) = clash_traffic(client, port).await {
                        let total_down = data.download_total;
                        let total_up = data.upload_total;
                        let (prev_down, prev_up) = prev_clash.unwrap_or((total_down, total_up));
                        let down = total_down.saturating_sub(prev_down);
                        let up = total_up.saturating_sub(prev_up);
                        prev_clash = Some((total_down, total_up));
                        stats.conns = data.connections.len().min(u32::MAX as usize) as u32;
                        let mut proxy_bytes = 0_u64;
                        let mut direct_bytes = 0_u64;
                        for conn in &data.connections {
                            let bytes = conn.download.saturating_add(conn.upload);
                            if conn.chains.iter().any(|chain| chain == "direct") {
                                direct_bytes = direct_bytes.saturating_add(bytes);
                            } else {
                                proxy_bytes = proxy_bytes.saturating_add(bytes);
                            }
                        }
                        let all = proxy_bytes.saturating_add(direct_bytes);
                        let (proxy_down, proxy_up) = if all == 0 {
                            if proxied {
                                (down, up)
                            } else {
                                (0, 0)
                            }
                        } else {
                            (
                                down.saturating_mul(proxy_bytes) / all,
                                up.saturating_mul(proxy_bytes) / all,
                            )
                        };
                        stats.down_bps = down;
                        stats.up_bps = up;
                        stats.proxy_down_bps = proxy_down;
                        stats.proxy_up_bps = proxy_up;
                        stats.direct_down_bps = down.saturating_sub(proxy_down);
                        stats.direct_up_bps = up.saturating_sub(proxy_up);
                        used_clash = true;
                    }
                }
            }
            if !used_clash {
                stats.down_bps = sys_down;
                stats.up_bps = sys_up;
                if proxied {
                    stats.proxy_down_bps = sys_down;
                    stats.proxy_up_bps = sys_up;
                } else {
                    stats.direct_down_bps = sys_down;
                    stats.direct_up_bps = sys_up;
                }
            }
            {
                let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                if guard.run_id != run_id {
                    return;
                }
                guard.traffic = stats;
                guard.touch();
            }
            ctx.request_repaint();
        }
    });
}

fn spawn_monitor(inner: Arc<Mutex<Inner>>, ctx: egui::Context, run_id: u64) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(400)).await;
            let result = {
                let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                if guard.run_id != run_id {
                    return;
                }
                let Some(child) = guard.child.as_mut() else {
                    return;
                };
                child.try_wait()
            };
            match result {
                Ok(Some(status)) => {
                    let code = status
                        .code()
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "неизвестен".into());
                    let (proxy_set, message) = {
                        let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                        guard.child = None;
                        guard.pid = None;
                        guard.connected_at = None;
                        guard.active_core = None;
                        guard.status = ConnStatus::Error;
                        let message =
                            format!("Ядро неожиданно завершилось (код {code}). Проверьте логи.");
                        guard.error = Some(message.clone());
                        guard.warning = None;
                        let proxy = guard.proxy_set;
                        guard.proxy_set = false;
                        guard.touch();
                        (proxy, message)
                    };
                    if proxy_set {
                        let _ = sysproxy::disable();
                    }
                    push_log(&inner, &ctx, "app", Some("error"), message);
                    ctx.request_repaint();
                    return;
                }
                Ok(None) => {}
                Err(error) => {
                    push_log(
                        &inner,
                        &ctx,
                        "app",
                        Some("warn"),
                        format!("Не удалось проверить состояние ядра: {error}"),
                    );
                }
            }
        }
    });
}

fn write_runtime_config(
    guard: &Inner,
    core: CoreKind,
    value: &serde_json::Value,
) -> Result<PathBuf, String> {
    let runtime_dir = guard.data_dir.join("runtime");
    std::fs::create_dir_all(&runtime_dir)
        .map_err(|error| format!("не удалось создать {}: {error}", runtime_dir.display()))?;
    let file = runtime_dir.join(match core {
        CoreKind::Singbox => "singbox.json",
        CoreKind::Xray => "xray.json",
    });
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("не удалось сериализовать конфиг: {error}"))?;
    std::fs::write(&file, text)
        .map_err(|error| format!("не удалось записать {}: {error}", file.display()))?;
    Ok(file)
}

async fn connect_core(inner: &Arc<Mutex<Inner>>, ctx: &egui::Context) -> Result<(), String> {
    stop_core(inner, ctx, true).await;
    let (config, data_dir) = {
        let guard = inner.lock().unwrap_or_else(|e| e.into_inner());
        (guard.config.clone(), guard.data_dir.clone())
    };
    let settings = config.settings.clone();
    let active_id = match config.active_server_id.clone() {
        Some(id) => id,
        None => return Err(set_error(inner, ctx, "Сначала выберите сервер".into())),
    };
    if settings.tun_enabled && settings.core == CoreKind::Xray {
        return Err(set_error(
            inner,
            ctx,
            "TUN-режим поддерживается только ядром sing-box".into(),
        ));
    }
    let cached = {
        let guard = inner.lock().unwrap_or_else(|e| e.into_inner());
        match settings.core {
            CoreKind::Singbox => guard.core_singbox.clone(),
            CoreKind::Xray => guard.core_xray.clone(),
        }
    };
    let info = match cached {
        Some(info) if info.found && info.compatible => info,
        _ => installer::detect(settings.core, &settings, &data_dir).await,
    };
    if !info.found {
        return Err(set_error(
            inner,
            ctx,
            info.error
                .clone()
                .unwrap_or_else(|| "ядро не найдено".into()),
        ));
    }
    if !info.compatible {
        return Err(set_error(
            inner,
            ctx,
            info.error
                .clone()
                .unwrap_or_else(|| "версия ядра не поддерживается".into()),
        ));
    }
    let core_path = info
        .path
        .clone()
        .ok_or_else(|| "нет пути к ядру".to_string())?;

    let ruleset_dir = data_dir.join("rulesets");
    if settings.core == CoreKind::Singbox {
        let sources = ruleset::sources(&config.rules);
        if !sources.is_empty() {
            let prepared =
                ruleset::prepare(&sources, &ruleset_dir, Duration::from_secs(7 * 24 * 3600)).await;
            for error in &prepared.errors {
                push_log(
                    inner,
                    ctx,
                    "app",
                    Some("warn"),
                    format!("База правил: {error}"),
                );
            }
            if prepared.downloaded > 0 {
                push_log(
                    inner,
                    ctx,
                    "app",
                    Some("info"),
                    format!("Обновлено баз GeoIP/GeoSite: {}", prepared.downloaded),
                );
            }
        }
    }

    let value = match settings.core {
        CoreKind::Singbox => singbox::build_config_with_capabilities(
            &config.servers,
            &active_id,
            &settings,
            &config.rules,
            info.version.as_deref(),
            Some(ruleset_dir.as_path()),
            CoreCapabilities {
                xhttp: info.supports_xhttp(),
                kcp: info.supports_kcp(),
                xdrive: info.supports_xdrive(),
            },
        ),
        CoreKind::Xray => xray::build_config(&config.servers, &active_id, &settings, &config.rules),
    }
    .map_err(|error| set_error(inner, ctx, error))?;

    let config_file = {
        let guard = inner.lock().unwrap_or_else(|e| e.into_inner());
        write_runtime_config(&guard, settings.core, &value)
    }
    .map_err(|error| set_error(inner, ctx, error))?;

    let run_id = {
        let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.run_id = guard.run_id.wrapping_add(1);
        guard.status = ConnStatus::Connecting;
        guard.error = None;
        guard.warning = None;
        guard.active_core = Some(settings.core);
        guard.pid = None;
        guard.connected_at = None;
        guard.touch();
        guard.run_id
    };
    ctx.request_repaint();
    let active_name = config
        .server(&active_id)
        .map(Server::display_name)
        .unwrap_or_default();
    push_log(
        inner,
        ctx,
        "app",
        Some("info"),
        format!(
            "Запуск {} {} · {}",
            settings.core.name(),
            info.version.clone().unwrap_or_default(),
            active_name
        ),
    );

    let mut command = Command::new(&core_path);
    command
        .arg("run")
        .arg("-c")
        .arg(&config_file)
        .current_dir(config_file.parent().unwrap_or(Path::new(".")))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    if settings.core == CoreKind::Xray {
        if let Some(parent) = Path::new(&core_path).parent() {
            command.env("XRAY_LOCATION_ASSET", parent);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.as_std_mut().creation_flags(0x08000000);
    }

    let mut child = command
        .spawn()
        .map_err(|error| set_error(inner, ctx, format!("Не удалось запустить ядро: {error}")))?;
    let pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    {
        let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.child = Some(child);
        guard.pid = pid;
        guard.touch();
    }
    if let Some(stdout) = stdout {
        spawn_log_reader(inner.clone(), ctx.clone(), run_id, stdout);
    }
    if let Some(stderr) = stderr {
        spawn_log_reader(inner.clone(), ctx.clone(), run_id, stderr);
    }
    spawn_monitor(inner.clone(), ctx.clone(), run_id);

    let port = settings.effective_http_port();
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut ready = false;
    while Instant::now() < deadline {
        let exited = {
            let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
            match guard.child.as_mut() {
                Some(child) => matches!(child.try_wait(), Ok(Some(_))),
                None => true,
            }
        };
        if exited {
            tokio::time::sleep(Duration::from_millis(180)).await;
            stop_core(inner, ctx, true).await;
            return Err(set_error(
                inner,
                ctx,
                format!(
                    "Ядро {} не смогло запуститься. Подробности — в логах.",
                    settings.core.name()
                ),
            ));
        }
        if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    if !ready {
        stop_core(inner, ctx, true).await;
        return Err(set_error(
            inner,
            ctx,
            format!("Не удалось дождаться запуска ядра на порту {port}. Порт занят?"),
        ));
    }

    let mut warning = None;
    let mut proxy_set = false;
    if settings.system_proxy {
        match sysproxy::enable(
            settings.effective_http_port(),
            settings.effective_socks_port(),
        ) {
            Ok(()) => proxy_set = true,
            Err(error) => warning = Some(format!("Не удалось включить системный прокси: {error}")),
        }
    }
    {
        let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.status = ConnStatus::Connected;
        guard.connected_at = Some(now_ms());
        guard.pid = pid;
        guard.error = None;
        guard.warning = warning;
        guard.proxy_set = proxy_set;
        guard.touch();
    }
    push_log(
        inner,
        ctx,
        "app",
        Some("info"),
        format!("Подключено: {active_name}"),
    );
    spawn_traffic_monitor(inner.clone(), ctx.clone(), run_id);
    ctx.request_repaint();
    Ok(())
}

async fn ping_one(
    server: Server,
    settings: &Settings,
    active_id: Option<&str>,
    connected: bool,
) -> (String, Option<u32>) {
    let id = server.id.clone();
    let is_active = active_id == Some(id.as_str());
    let mut latency = None;
    if connected && is_active {
        latency = ping::http_proxy_test(settings.effective_http_port(), &settings.test_url, 7000)
            .await
            .ok();
    } else if connected && settings.core == CoreKind::Singbox {
        latency = ping::clash_delay(settings.clash_api_port, &id, &settings.test_url, 5000)
            .await
            .ok();
    }
    if latency.is_none() {
        latency = ping::tcp_ping(&server, 4000).await.ok();
    }
    (id, latency)
}
