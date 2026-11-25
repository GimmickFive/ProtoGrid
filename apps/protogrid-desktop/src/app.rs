use std::time::Duration;

use crate::engine::{ConnStatus, Engine, Snapshot};
use crate::theme;
use crate::ui::{self, View};

pub struct ProtoGridApp {
    engine: Engine,
    view: View,
    snapshot: Option<Snapshot>,
    last_revision: u64,
    tray: Option<tray_icon::TrayIcon>,
    quitting: bool,
}

fn rounded_icon(size: u32) -> Vec<u8> {
    let mut rgba = vec![0_u8; (size * size * 4) as usize];
    let radius = size as f32 * 0.23;
    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let dx = (fx - size as f32 / 2.0).abs() - (size as f32 / 2.0 - radius);
            let dy = (fy - size as f32 / 2.0).abs() - (size as f32 / 2.0 - radius);
            let inside = dx <= 0.0 || dy <= 0.0 || (dx * dx + dy * dy) <= radius * radius;
            if !inside {
                continue;
            }
            let t = (fx + fy) / (size as f32 * 2.0);
            let r = (79.0 + (139.0 - 79.0) * t) as u8;
            let g = (108.0 + (92.0 - 108.0) * t) as u8;
            let b = (255.0 + (246.0 - 255.0) * t) as u8;
            let index = ((y * size + x) * 4) as usize;
            rgba[index] = r;
            rgba[index + 1] = g;
            rgba[index + 2] = b;
            rgba[index + 3] = 255;

            let cx = size as f32 / 2.0;
            let cy = size as f32 / 2.0;
            let px = fx - cx;
            let py = fy - cy;
            let in_triangle = py > -(size as f32) * 0.30
                && py < size as f32 * 0.34
                && px > -(size as f32) * 0.28
                && px < size as f32 * 0.30
                && (px * 0.9 + py * 0.7) < size as f32 * 0.12;
            if in_triangle {
                rgba[index] = 255;
                rgba[index + 1] = 255;
                rgba[index + 2] = 255;
            }
        }
    }
    rgba
}

pub fn icon_data(size: u32) -> egui::IconData {
    egui::IconData {
        rgba: rounded_icon(size),
        width: size,
        height: size,
    }
}

#[cfg(target_os = "linux")]
fn linux_tray_host_available() -> bool {
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        return false;
    }
    [
        "org.kde.StatusNotifierWatcher",
        "org.freedesktop.StatusNotifierWatcher",
    ]
    .iter()
    .any(|service| {
        std::process::Command::new("gdbus")
            .args([
                "introspect",
                "--session",
                "--dest",
                service,
                "--object-path",
                "/StatusNotifierWatcher",
            ])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    })
}

fn create_tray() -> Option<tray_icon::TrayIcon> {
    #[cfg(target_os = "linux")]
    {
        if !linux_tray_host_available() {
            return None;
        }
        if gtk::init().is_err() {
            return None;
        }
    }
    use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};
    let menu = Menu::new();
    let show = MenuItem::with_id("show", "Открыть ProtoGrid", true, None);
    let toggle = MenuItem::with_id("toggle", "Подключить / отключить", true, None);
    let separator = PredefinedMenuItem::separator();
    let quit = MenuItem::with_id("quit", "Выход", true, None);
    menu.append_items(&[&show, &toggle, &separator, &quit])
        .ok()?;
    let icon = tray_icon::Icon::from_rgba(rounded_icon(32), 32, 32).ok()?;
    tray_icon::TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("ProtoGrid")
        .with_icon(icon)
        .build()
        .ok()
}

impl ProtoGridApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let dark = cc
            .storage
            .and_then(|storage| eframe::get_value::<bool>(storage, "dark"))
            .unwrap_or(true);
        theme::install(&cc.egui_ctx, dark);
        let engine = Engine::new(cc.egui_ctx.clone());
        let tray = create_tray();
        let mut view = View::new(dark);
        view.tray_available = tray.is_some();
        Self {
            engine,
            view,
            snapshot: None,
            last_revision: u64::MAX,
            tray,
            quitting: false,
        }
    }

    fn show_window(&self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    fn poll_tray(&mut self, ctx: &egui::Context) {
        if self.tray.is_some() {
            #[cfg(target_os = "linux")]
            while gtk::events_pending() {
                gtk::main_iteration_do(false);
            }
        }
        while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            match event.id().as_ref() {
                "show" => self.show_window(ctx),
                "toggle" => {
                    let connected = self
                        .snapshot
                        .as_ref()
                        .map(|snapshot| {
                            matches!(
                                snapshot.status,
                                ConnStatus::Connected | ConnStatus::Connecting
                            )
                        })
                        .unwrap_or(false);
                    if connected {
                        self.engine.disconnect();
                    } else {
                        self.engine.connect();
                    }
                }
                "quit" => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                _ => {}
            }
        }
        while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
            if let tray_icon::TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            } = event
            {
                self.show_window(ctx);
            }
        }
        let close_to_tray = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.config.settings.close_to_tray)
            .unwrap_or(true);
        if self.tray.is_some()
            && !self.quitting
            && close_to_tray
            && ctx.input(|input| input.viewport().close_requested())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        if self.tray.is_some() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }
}

impl eframe::App for ProtoGridApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(snapshot) = self.engine.snapshot_if_changed(&mut self.last_revision) {
            self.snapshot = Some(snapshot);
        }
        self.view.take_engine_toasts(self.engine.drain_toasts());
        self.poll_tray(ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "dark", &self.view.dark);
    }

    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if let Some(snapshot) = self.engine.snapshot_if_changed(&mut self.last_revision) {
            self.snapshot = Some(snapshot);
        }
        self.view.take_engine_toasts(self.engine.drain_toasts());
        if let Some(snapshot) = &self.snapshot {
            ui::show(root_ui, &mut self.view, snapshot, &self.engine);
            if snapshot.is_connected() || snapshot.is_connecting() || !snapshot.busy.is_empty() {
                root_ui
                    .ctx()
                    .request_repaint_after(Duration::from_millis(500));
            } else if !self.view.toasts.is_empty() {
                root_ui
                    .ctx()
                    .request_repaint_after(Duration::from_millis(300));
            }
        } else {
            root_ui.vertical_centered(|ui| {
                ui.add_space(120.0);
                ui.spinner();
                ui.label("Запуск ProtoGrid…");
            });
            root_ui
                .ctx()
                .request_repaint_after(Duration::from_millis(100));
        }
    }
}

impl Drop for ProtoGridApp {
    fn drop(&mut self) {
        self.engine.shutdown_blocking();
    }
}
