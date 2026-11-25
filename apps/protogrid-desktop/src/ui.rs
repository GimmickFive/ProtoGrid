use std::collections::{BTreeSet, VecDeque};
use std::time::{Duration, Instant};

use eframe::egui;
use egui::{
    Align, Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Id, Layout, Margin, Modal,
    Rect, RichText, ScrollArea, Sense, Stroke, TextEdit, Ui, UiBuilder, Vec2,
};
use protogrid_core::model::{
    CoreCapabilities, CoreKind, Protocol, RoutingRule, RuleAction, RuleKind, Security, Server,
    Settings, Subscription, Transport,
};

use crate::engine::{
    now_ms, ConnStatus, Engine, LogEntry, Snapshot, ToastEvent, ToastKind, TrafficStats,
};
use crate::installer::CoreFeatures;
use crate::theme::{self, Palette};

/* -------------------------------------------------------------------------- */
/* State                                                                      */
/* -------------------------------------------------------------------------- */

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Overview,
    Servers,
    Subscriptions,
    Routing,
    Charts,
    Logs,
    Settings,
}

impl Page {
    const ALL: [Page; 7] = [
        Page::Overview,
        Page::Servers,
        Page::Subscriptions,
        Page::Routing,
        Page::Charts,
        Page::Logs,
        Page::Settings,
    ];

    fn label(self) -> &'static str {
        match self {
            Page::Overview => "Обзор",
            Page::Servers => "Серверы",
            Page::Subscriptions => "Подписки",
            Page::Routing => "Маршрутизация",
            Page::Charts => "Графики",
            Page::Logs => "Журнал",
            Page::Settings => "Настройки",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Page::Overview => egui_phosphor::regular::SQUARES_FOUR,
            Page::Servers => egui_phosphor::regular::HARD_DRIVES,
            Page::Subscriptions => egui_phosphor::regular::STACK,
            Page::Routing => egui_phosphor::regular::SIGNPOST,
            Page::Charts => egui_phosphor::regular::CHART_BAR,
            Page::Logs => egui_phosphor::regular::TERMINAL_WINDOW,
            Page::Settings => egui_phosphor::regular::GEAR_SIX,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum SortMode {
    #[default]
    Server,
    Name,
    Latency,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Down,
    Up,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum LogFilter {
    #[default]
    All,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Copy, Default)]
pub struct TrafficPoint {
    pub down: f32,
    pub up: f32,
    pub conns: f32,
    pub proxy_down: f32,
    pub proxy_up: f32,
    pub direct_down: f32,
    pub direct_up: f32,
    pub ping: f32,
}

#[derive(Clone, Copy, Default)]
pub struct Totals {
    pub down: f64,
    pub up: f64,
    pub proxy_down: f64,
    pub proxy_up: f64,
    pub direct_down: f64,
    pub direct_up: f64,
}

pub struct UiToast {
    pub text: String,
    pub kind: ToastKind,
    pub created: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ModalState {
    Server,
    Rule,
    Import,
    Subscription,
    Confirm,
    Help,
}

#[derive(Clone)]
pub enum ConfirmAction {
    DeleteServer(String),
    DeleteSubscription(String),
    DeleteRule(String),
}

#[derive(Clone)]
pub struct Confirm {
    pub title: String,
    pub message: String,
    pub label: String,
    pub danger: bool,
    pub action: ConfirmAction,
}

pub struct View {
    pub page: Page,
    pub dark: bool,
    pub search: String,
    pub group: Option<String>,
    pub sort: SortMode,
    pub logs_filter: String,
    pub log_level: LogFilter,
    pub logs_paused: bool,
    pub auto_scroll: bool,
    pub frozen_logs: Option<Vec<LogEntry>>,
    pub modal: Option<ModalState>,
    pub server_draft: Option<Server>,
    pub rule_draft: Option<RoutingRule>,
    pub import_text: String,
    pub subscription_name: String,
    pub subscription_url: String,
    pub subscription_edit: Option<String>,
    pub confirm: Option<Confirm>,
    pub settings_draft: Option<Settings>,
    pub settings_dirty: bool,
    pub toasts: Vec<UiToast>,
    pub server_alpn: String,
    pub tray_available: bool,
    pub ov_range: u64,
    pub chart_range: u64,
    pub chart_dir: Direction,
    pub series: VecDeque<TrafficPoint>,
    pub totals: Totals,
    pub last_sample: Option<Instant>,
    pub last_status: Option<ConnStatus>,
}

impl View {
    pub fn new(dark: bool) -> Self {
        let mut series = VecDeque::new();
        for _ in 0..120 {
            series.push_back(TrafficPoint::default());
        }
        Self {
            page: Page::Overview,
            dark,
            search: String::new(),
            group: None,
            sort: SortMode::Server,
            logs_filter: String::new(),
            log_level: LogFilter::All,
            logs_paused: false,
            auto_scroll: true,
            frozen_logs: None,
            modal: None,
            server_draft: None,
            rule_draft: None,
            import_text: String::new(),
            subscription_name: String::new(),
            subscription_url: String::new(),
            subscription_edit: None,
            confirm: None,
            settings_draft: None,
            settings_dirty: false,
            toasts: Vec::new(),
            server_alpn: String::new(),
            tray_available: false,
            ov_range: 60,
            chart_range: 60,
            chart_dir: Direction::Down,
            series,
            totals: Totals::default(),
            last_sample: None,
            last_status: None,
        }
    }

    pub fn toast(&mut self, text: impl Into<String>, kind: ToastKind) {
        self.toasts.push(UiToast {
            text: text.into(),
            kind,
            created: Instant::now(),
        });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    pub fn take_engine_toasts(&mut self, toasts: Vec<ToastEvent>) {
        for toast in toasts {
            self.toast(toast.text, toast.kind);
        }
    }

    fn tick(&mut self, snapshot: &Snapshot) {
        if snapshot.is_connected() && self.last_status != Some(ConnStatus::Connected) {
            self.totals = Totals::default();
            self.series.clear();
            for _ in 0..120 {
                self.series.push_back(TrafficPoint::default());
            }
        }
        self.last_status = Some(snapshot.status);

        let now = Instant::now();
        let due = self
            .last_sample
            .map(|at| now.duration_since(at) >= Duration::from_millis(900))
            .unwrap_or(true);
        if !due {
            return;
        }
        self.last_sample = Some(now);
        let stats: TrafficStats = snapshot.traffic;
        let ping = snapshot
            .active_server()
            .and_then(|server| snapshot.pings.get(&server.id).copied().flatten())
            .unwrap_or(0) as f32;
        let point = TrafficPoint {
            down: stats.down_bps as f32,
            up: stats.up_bps as f32,
            conns: stats.conns as f32,
            proxy_down: stats.proxy_down_bps as f32,
            proxy_up: stats.proxy_up_bps as f32,
            direct_down: stats.direct_down_bps as f32,
            direct_up: stats.direct_up_bps as f32,
            ping,
        };
        self.totals.down += point.down as f64;
        self.totals.up += point.up as f64;
        self.totals.proxy_down += point.proxy_down as f64;
        self.totals.proxy_up += point.proxy_up as f64;
        self.totals.direct_down += point.direct_down as f64;
        self.totals.direct_up += point.direct_up as f64;
        self.series.push_back(point);
        while self.series.len() > 900 {
            self.series.pop_front();
        }
    }

    fn last_points(&self, range: u64) -> VecDeque<TrafficPoint> {
        let take = (range as usize).min(self.series.len());
        self.series
            .iter()
            .skip(self.series.len().saturating_sub(take))
            .copied()
            .collect()
    }
}

/* -------------------------------------------------------------------------- */
/* Widgets                                                                    */
/* -------------------------------------------------------------------------- */

fn semantics_color(p: &Palette, kind: ToastKind) -> Color32 {
    match kind {
        ToastKind::Success => p.green,
        ToastKind::Error => p.red,
        ToastKind::Warn => p.amber,
        ToastKind::Info => p.accent,
    }
}

fn measure(ui: &Ui, text: &str, font: FontId) -> f32 {
    ui.ctx().fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(text.to_owned(), font, Color32::WHITE)
            .size()
            .x
    })
}

fn medium_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("Medium".into()))
}

/// Equal-width grid that always spans the full available width.
///
/// `ui.columns` keeps the outer item spacing out of the last column, which
/// leaves card grids a few pixels short of the content edge. This helper
/// measures the columns explicitly and inserts the gaps itself.
fn card_grid(ui: &mut Ui, columns: usize, gap: f32, mut add: impl FnMut(&mut Ui, usize)) {
    if columns == 0 {
        return;
    }
    let available = ui.available_width();
    let width = ((available - gap * columns.saturating_sub(1) as f32) / columns as f32).max(1.0);
    let inherited_spacing = ui.spacing().item_spacing;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for index in 0..columns {
            if index > 0 {
                ui.add_space(gap);
            }
            ui.allocate_ui_with_layout(Vec2::new(width, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing = inherited_spacing;
                ui.set_min_width(width);
                ui.set_max_width(width);
                add(ui, index);
            });
        }
    });
}

#[derive(Clone, Copy, PartialEq)]
enum BtnKind {
    Primary,
    Ghost,
    Outline,
}

fn action_button(
    ui: &mut Ui,
    p: &Palette,
    icon: &str,
    label: &str,
    kind: BtnKind,
) -> egui::Response {
    let font = medium_font(12.5);
    let text_w = if label.is_empty() {
        0.0
    } else {
        measure(ui, label, font.clone())
    };
    let icon_w = if icon.is_empty() { 0.0 } else { 15.0 };
    let gap = if !icon.is_empty() && !label.is_empty() {
        7.0
    } else {
        0.0
    };
    let width = (24.0 + icon_w + gap + text_w).max(32.0);
    let height = 32.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    let hovered = response.hovered() && ui.is_enabled();
    let (fill, stroke, text_color) = match kind {
        BtnKind::Primary => (
            if hovered { p.accent_2 } else { p.accent },
            Stroke::NONE,
            Color32::WHITE,
        ),
        BtnKind::Ghost => (
            if hovered {
                p.panel_2
            } else {
                Color32::TRANSPARENT
            },
            Stroke::NONE,
            if hovered { p.text } else { p.muted },
        ),
        BtnKind::Outline => (
            if hovered {
                p.accent_soft
            } else {
                Color32::TRANSPARENT
            },
            Stroke::new(1.0, p.accent_line),
            p.accent_2,
        ),
    };
    let fill = if ui.is_enabled() {
        fill
    } else {
        Color32::TRANSPARENT
    };
    let text_color = if ui.is_enabled() { text_color } else { p.faint };
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(8),
        fill,
        stroke,
        egui::StrokeKind::Inside,
    );
    let mut x = rect.left() + 12.0;
    if !icon.is_empty() {
        painter.text(
            egui::pos2(x, rect.center().y),
            Align2::LEFT_CENTER,
            icon,
            FontId::new(15.0, FontFamily::Proportional),
            text_color,
        );
        x += icon_w + gap;
    }
    if !label.is_empty() {
        painter.text(
            egui::pos2(x, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            font,
            text_color,
        );
    }
    response
}

fn top_button(
    ui: &mut Ui,
    p: &Palette,
    icon: &str,
    label: &str,
    kind: BtnKind,
    enabled: bool,
) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| action_button(ui, p, icon, label, kind))
        .inner
}

fn round_icon_button(ui: &mut Ui, p: &Palette, icon: &str, tooltip: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
    let hovered = response.hovered();
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(7),
        if hovered {
            p.panel_2
        } else {
            Color32::TRANSPARENT
        },
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        FontId::new(16.0, FontFamily::Proportional),
        if hovered { p.text } else { p.faint },
    );
    response.on_hover_text(tooltip)
}

fn small_ghost(ui: &mut Ui, p: &Palette, icon: &str, tooltip: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
    let hovered = response.hovered();
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(6),
        if hovered {
            p.panel_2
        } else {
            Color32::TRANSPARENT
        },
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        FontId::new(14.0, FontFamily::Proportional),
        if hovered { p.cyan } else { p.faint },
    );
    response.on_hover_text(tooltip)
}

/// Compact action pill: 21 px tall, 7 px horizontal padding, like `.act` in the
/// web concept. Painted manually so the surrounding column layout cannot
/// stretch it to the full row height.
fn pill(ui: &mut Ui, text: &str, color: Color32, fill: Color32, stroke: Stroke) {
    let font = FontId::new(9.5, FontFamily::Name("MonoMedium".into()));
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let size = Vec2::new(galley.size().x + 14.0, 21.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let radius = CornerRadius::same(5);
    ui.painter().rect_filled(rect, radius, fill);
    if stroke.width > 0.0 {
        ui.painter()
            .rect_stroke(rect, radius, stroke, egui::StrokeKind::Inside);
    }
    ui.painter().galley(
        egui::pos2(
            rect.center().x - galley.size().x * 0.5,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        color,
    );
}

fn status_color(p: &Palette, status: ConnStatus) -> Color32 {
    match status {
        ConnStatus::Connected => p.green,
        ConnStatus::Connecting => p.amber,
        ConnStatus::Error => p.red,
        ConnStatus::Disconnected => p.faint,
    }
}

fn switch(ui: &mut Ui, p: &Palette, value: &mut bool) -> bool {
    switch_sized(ui, p, value, Vec2::new(34.0, 19.0))
}

fn switch_small(ui: &mut Ui, p: &Palette, value: &mut bool) -> bool {
    switch_sized(ui, p, value, Vec2::new(28.0, 16.0))
}

fn switch_sized(ui: &mut Ui, p: &Palette, value: &mut bool, size: Vec2) -> bool {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *value = !*value;
    }
    let hovered = response.hovered();
    let track = if *value {
        p.accent
    } else if hovered {
        p.line_strong
    } else {
        p.line
    };
    let radius = size.y * 0.5;
    ui.painter()
        .rect_filled(rect, CornerRadius::same(radius as u8), track);
    let inset = size.y * 0.16;
    let thumb = size.y * 0.5 - inset;
    let center = if *value {
        egui::pos2(rect.right() - radius, rect.center().y)
    } else {
        egui::pos2(rect.left() + radius, rect.center().y)
    };
    ui.painter().circle_filled(center, thumb, Color32::WHITE);
    response.clicked()
}

fn seg_button(ui: &mut Ui, p: &Palette, text: &str, selected: bool) -> egui::Response {
    let font = medium_font(10.5);
    let width = measure(ui, text, font.clone()) + 16.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 23.0), Sense::click());
    if selected {
        ui.painter().rect(
            rect,
            CornerRadius::same(5),
            p.panel,
            Stroke::new(1.0, p.line),
            egui::StrokeKind::Inside,
        );
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(5), p.panel_3);
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        font,
        if selected { p.text } else { p.faint },
    );
    response
}

fn seg(ui: &mut Ui, p: &Palette, items: &[(u64, &str)], selected: u64) -> Option<u64> {
    let mut chosen = None;
    let mut total = 4.0;
    for (index, (_, label)) in items.iter().enumerate() {
        total += measure(ui, label, medium_font(10.5)) + 16.0;
        if index + 1 < items.len() {
            total += 1.0;
        }
    }
    Frame::new()
        .fill(p.panel_2)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(7)
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(total, 25.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.set_min_width(total);
                    ui.spacing_mut().item_spacing.x = 1.0;
                    for (value, label) in items {
                        if seg_button(ui, p, label, selected == *value).clicked() {
                            chosen = Some(*value);
                        }
                    }
                },
            );
        });
    chosen
}

fn dashed_round_rect(painter: &egui::Painter, rect: Rect, radius: f32, stroke: Stroke) {
    use std::f32::consts::{FRAC_PI_2, PI};
    let r = radius.min(rect.width() * 0.5).min(rect.height() * 0.5);
    let mut points = Vec::with_capacity(40);
    let arcs = [
        (rect.left() + r, rect.top() + r, PI, PI + FRAC_PI_2),
        (rect.right() - r, rect.top() + r, PI + FRAC_PI_2, 2.0 * PI),
        (rect.right() - r, rect.bottom() - r, 0.0, FRAC_PI_2),
        (rect.left() + r, rect.bottom() - r, FRAC_PI_2, PI),
    ];
    for (cx, cy, start, end) in arcs {
        for step in 0..=8 {
            let angle = start + (end - start) * step as f32 / 8.0;
            points.push(egui::pos2(cx + r * angle.cos(), cy + r * angle.sin()));
        }
    }
    if let Some(first) = points.first().copied() {
        points.push(first);
    }
    painter.extend(egui::Shape::dashed_line(&points, stroke, 6.0, 4.0));
}

fn progress_bar(ui: &mut Ui, p: &Palette, fraction: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 5.0), Sense::hover());
    let radius = CornerRadius::same(3);
    ui.painter().rect_filled(rect, radius, p.panel_2);
    let width = rect.width() * fraction.clamp(0.0, 1.0);
    if width > 0.5 {
        let filled = Rect::from_min_size(rect.min, Vec2::new(width, rect.height()));
        ui.painter().rect_filled(filled, radius, color);
    }
    ui.painter().rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, p.line),
        egui::StrokeKind::Inside,
    );
}

fn icon_box(ui: &mut Ui, _p: &Palette, icon: &str, fg: Color32, bg: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(6), bg);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        FontId::new(size * 0.55, FontFamily::Proportional),
        fg,
    );
}

fn paint_area(
    ui: &Ui,
    rect: Rect,
    values: &[f32],
    max: f32,
    color: Color32,
    fill_alpha: u8,
    line_width: f32,
) {
    if values.len() < 2 || rect.width() <= 1.0 {
        return;
    }
    let painter = ui.painter_at(rect);
    let max = max.max(0.001);
    let last = (values.len() - 1).max(1) as f32;
    let mut mesh = egui::epaint::Mesh::default();
    let mut line = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let x = rect.left() + rect.width() * index as f32 / last;
        let y = rect.bottom() - rect.height() * (*value / max).clamp(0.0, 1.0);
        line.push(egui::pos2(x, y));
        mesh.colored_vertex(
            egui::pos2(x, rect.bottom()),
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 0),
        );
        mesh.colored_vertex(
            egui::pos2(x, y),
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), fill_alpha),
        );
    }
    for index in 0..values.len().saturating_sub(1) {
        let base = (index * 2) as u32;
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base + 1, base + 3, base + 2);
    }
    painter.add(egui::Shape::mesh(mesh));
    painter.add(egui::Shape::line(
        line,
        Stroke::new(line_width, color.gamma_multiply(0.95)),
    ));
}

fn sparkline(ui: &mut Ui, _p: &Palette, values: &[f32], color: Color32, height: f32) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let max = values.iter().copied().fold(0.0_f32, f32::max);
    paint_area(ui, rect, values, (max * 1.25).max(1.0), color, 46, 1.6);
}

fn chart_grid(ui: &Ui, p: &Palette, rect: Rect) {
    let painter = ui.painter_at(rect);
    for index in 0..=4 {
        let y = rect.top() + rect.height() * index as f32 / 4.0;
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            Stroke::new(1.0, p.chart_grid),
        );
    }
}

fn page_head(ui: &mut Ui, p: &Palette, title: &str, subtitle: &str, actions: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(theme::heading(title));
            ui.add_space(1.0);
            ui.label(RichText::new(subtitle).size(12.0).color(p.faint));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), actions);
    });
    ui.add_space(18.0);
}

fn empty_state(
    ui: &mut Ui,
    p: &Palette,
    icon: &str,
    title: &str,
    text: &str,
    actions: impl FnOnce(&mut Ui),
) {
    Frame::new().inner_margin(Margin::same(40)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(icon)
                    .size(34.0)
                    .color(p.accent_2)
                    .family(FontFamily::Proportional),
            );
            ui.add_space(8.0);
            ui.label(theme::semibold(title, 15.0));
            ui.label(RichText::new(text).size(11.5).color(p.faint));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() / 2.0 - 90.0).max(0.0));
                actions(ui);
            });
        });
    });
}

/* -------------------------------------------------------------------------- */
/* Formatting                                                                 */
/* -------------------------------------------------------------------------- */

fn bytes(value: u64) -> String {
    let mut size = value as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < 4 {
        size /= 1024.0;
        unit += 1;
    }
    let units = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
    if unit == 0 {
        format!("{size:.0} {}", units[unit])
    } else {
        format!("{size:.1} {}", units[unit])
    }
}

fn bytes_f(value: f64) -> String {
    bytes(value.max(0.0) as u64)
}

/// Integer traffic sizes for compact subscription cards ("224 ГБ").
fn bytes_compact(value: u64) -> String {
    let mut size = value as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < 4 {
        size /= 1024.0;
        unit += 1;
    }
    let units = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
    format!("{size:.0} {}", units[unit])
}

fn rate(value: f32) -> (String, &'static str) {
    if value >= 1024.0 * 1024.0 {
        (format!("{:.1}", value / (1024.0 * 1024.0)), "МБ/с")
    } else if value >= 1024.0 {
        (format!("{:.0}", value / 1024.0), "КБ/с")
    } else {
        (format!("{value:.0}"), "Б/с")
    }
}

fn rate_plain(value: f32) -> String {
    let (number, unit) = rate(value);
    format!("{number} {unit}")
}

fn date(seconds: i64) -> String {
    if seconds <= 0 {
        return "—".into();
    }
    chrono::DateTime::from_timestamp(seconds, 0)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%d.%m.%Y")
                .to_string()
        })
        .unwrap_or_else(|| "—".into())
}

fn hhmmss(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%H:%M:%S")
                .to_string()
        })
        .unwrap_or_default()
}

fn date_ms(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_default()
}

fn uptime(ms: i64) -> String {
    let seconds = (ms.max(0) / 1000) as u64;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        (seconds % 3600) / 60,
        seconds % 60
    )
}

fn plural(n: usize, one: &'static str, few: &'static str, many: &'static str) -> &'static str {
    let n100 = n % 100;
    let n10 = n % 10;
    if (11..=19).contains(&n100) {
        many
    } else if n10 == 1 {
        one
    } else if (2..=4).contains(&n10) {
        few
    } else {
        many
    }
}

fn protocol_name(protocol: Protocol) -> &'static str {
    protocol.label()
}

const ALL_PROTOCOLS: [Protocol; 7] = [
    Protocol::Vless,
    Protocol::Vmess,
    Protocol::Trojan,
    Protocol::Shadowsocks,
    Protocol::Hysteria2,
    Protocol::Tuic,
    Protocol::Masque,
];

/// One accent per protocol, used for every protocol badge in the app.
fn protocol_color(protocol: Protocol, p: &Palette) -> Color32 {
    let color = match protocol {
        Protocol::Vless => p.violet,
        Protocol::Vmess => p.cyan,
        Protocol::Trojan => p.amber,
        Protocol::Shadowsocks => p.green,
        Protocol::Hysteria2 => p.red,
        Protocol::Tuic => Color32::from_rgb(251, 146, 60),
        Protocol::Masque => Color32::from_rgb(236, 120, 200),
    };
    if p.dark {
        color
    } else {
        color.gamma_multiply(0.72)
    }
}

fn tinted(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

fn protocol_badge(ui: &mut Ui, p: &Palette, protocol: Protocol) {
    let color = protocol_color(protocol, p);
    let fill = tinted(color, if p.dark { 38 } else { 30 });
    let stroke = Stroke::new(1.0, tinted(color, if p.dark { 90 } else { 70 }));
    pill(ui, protocol_name(protocol), color, fill, stroke);
}

fn mini_chip(ui: &mut Ui, text: &str, fg: Color32, bg: Color32) {
    let font = FontId::new(8.5, FontFamily::Name("MonoMedium".into()));
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, fg);
    let size = Vec2::new(galley.size().x + 10.0, 17.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let radius = CornerRadius::same(4);
    ui.painter().rect_filled(rect, radius, bg);
    ui.painter().galley(
        egui::pos2(
            rect.center().x - galley.size().x * 0.5,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        fg,
    );
}

fn mini_protocol_chip(ui: &mut Ui, p: &Palette, protocol: Protocol) {
    let color = protocol_color(protocol, p);
    mini_chip(
        ui,
        protocol_name(protocol),
        color,
        tinted(color, if p.dark { 34 } else { 26 }),
    );
}

fn mini_feature_chip(ui: &mut Ui, p: &Palette, text: &str) {
    mini_chip(ui, text, p.muted, p.panel_3);
}

/// Protocols a core build can actually run.
fn core_protocols(kind: CoreKind, features: CoreFeatures) -> Vec<Protocol> {
    ALL_PROTOCOLS
        .iter()
        .copied()
        .filter(|protocol| match kind {
            CoreKind::Xray => matches!(
                protocol,
                Protocol::Vless
                    | Protocol::Vmess
                    | Protocol::Trojan
                    | Protocol::Shadowsocks
                    | Protocol::Masque
            ),
            CoreKind::Singbox | CoreKind::Protogrid => {
                *protocol != Protocol::Masque || features.masque
            }
        })
        .collect()
}

/// Features reported by probes against the concrete core binary.
fn core_feature_list(features: CoreFeatures) -> Vec<&'static str> {
    let mut list = Vec::new();
    if features.tun {
        list.push("TUN");
    }
    if features.xhttp {
        list.push("XHTTP");
    }
    if features.kcp {
        list.push("mKCP");
    }
    if features.xdrive {
        list.push("XDRIVE");
    }
    if features.mldsa65 {
        list.push("ML-DSA");
    }
    if features.vless_encryption {
        list.push("VLESS-ENC");
    }
    if features.fakedns {
        list.push("FakeDNS");
    }
    if features.clash_api {
        list.push("Clash");
    }
    if features.noise {
        list.push("noise");
    }
    list
}

fn transport_short(transport: Transport) -> &'static str {
    match transport {
        Transport::Tcp => "raw",
        Transport::Ws => "ws",
        Transport::Grpc => "grpc",
        Transport::Http => "http",
        Transport::HttpUpgrade => "httpup",
        Transport::Xhttp => "xhttp",
        Transport::Kcp => "kcp",
        Transport::XDrive => "xdrive",
    }
}

fn security_short(security: Security) -> &'static str {
    match security {
        Security::None => "none",
        Security::Tls => "tls",
        Security::Reality => "reality",
    }
}

fn security_label(security: Security) -> &'static str {
    match security {
        Security::None => "Без TLS",
        Security::Tls => "TLS",
        Security::Reality => "Reality",
    }
}

fn xhttp_mode_label(mode: &str) -> &'static str {
    match mode {
        "packet-up" => "packet-up (CDN)",
        "stream-up" => "stream-up",
        "stream-one" => "stream-one",
        _ => "auto (stream-one)",
    }
}

fn transport_label(transport: Transport) -> &'static str {
    match transport {
        Transport::Tcp => "TCP",
        Transport::Ws => "WebSocket",
        Transport::Grpc => "gRPC",
        Transport::Http => "HTTP/2",
        Transport::HttpUpgrade => "HTTP Upgrade",
        Transport::Xhttp => "XHTTP",
        Transport::Kcp => "mKCP",
        Transport::XDrive => "XDRIVE",
    }
}

fn country_code(name: &str) -> Option<&'static str> {
    const PLACES: &[(&str, &str)] = &[
        ("нью-йорк", "US"),
        ("нью йорк", "US"),
        ("new york", "US"),
        ("лос-анджелес", "US"),
        ("los angeles", "US"),
        ("сан-франциско", "US"),
        ("san francisco", "US"),
        ("сан-паулу", "BR"),
        ("sao paulo", "BR"),
        ("são paulo", "BR"),
        ("буэнос-айрес", "AR"),
        ("buenos aires", "AR"),
        ("тель-авив", "IL"),
        ("tel aviv", "IL"),
        ("эр-рияд", "SA"),
        ("riyadh", "SA"),
        ("амстердам", "NL"),
        ("amsterdam", "NL"),
        ("франкфурт", "DE"),
        ("frankfurt", "DE"),
        ("хельсинки", "FI"),
        ("helsinki", "FI"),
        ("токио", "JP"),
        ("tokyo", "JP"),
        ("сингапур", "SG"),
        ("singapore", "SG"),
        ("лондон", "GB"),
        ("london", "GB"),
        ("париж", "FR"),
        ("paris", "FR"),
        ("берлин", "DE"),
        ("berlin", "DE"),
        ("варшава", "PL"),
        ("warsaw", "PL"),
        ("стамбул", "TR"),
        ("istanbul", "TR"),
        ("дубай", "AE"),
        ("dubai", "AE"),
        ("сеул", "KR"),
        ("seoul", "KR"),
        ("гонконг", "HK"),
        ("hong kong", "HK"),
        ("тайвань", "TW"),
        ("taiwan", "TW"),
        ("рига", "LV"),
        ("riga", "LV"),
        ("вильнюс", "LT"),
        ("vilnius", "LT"),
        ("таллин", "EE"),
        ("tallinn", "EE"),
        ("киев", "UA"),
        ("kyiv", "UA"),
        ("kiev", "UA"),
        ("москва", "RU"),
        ("moscow", "RU"),
        ("петербург", "RU"),
        ("saint petersburg", "RU"),
        ("алматы", "KZ"),
        ("almaty", "KZ"),
        ("ереван", "AM"),
        ("yerevan", "AM"),
        ("тбилиси", "GE"),
        ("tbilisi", "GE"),
        ("баку", "AZ"),
        ("baku", "AZ"),
        ("сидней", "AU"),
        ("sydney", "AU"),
        ("мельбурн", "AU"),
        ("melbourne", "AU"),
        ("торонто", "CA"),
        ("toronto", "CA"),
        ("монреаль", "CA"),
        ("montreal", "CA"),
        ("чикаго", "US"),
        ("chicago", "US"),
        ("майами", "US"),
        ("miami", "US"),
        ("даллас", "US"),
        ("dallas", "US"),
        ("сиэтл", "US"),
        ("seattle", "US"),
        ("ашберн", "US"),
        ("ashburn", "US"),
        ("финикс", "US"),
        ("phoenix", "US"),
        ("атланта", "US"),
        ("atlanta", "US"),
        ("денвер", "US"),
        ("denver", "US"),
        ("вашингтон", "US"),
        ("washington", "US"),
        ("хьюстон", "US"),
        ("houston", "US"),
        ("нью-джерси", "US"),
        ("new jersey", "US"),
        ("цюрих", "CH"),
        ("zurich", "CH"),
        ("вена", "AT"),
        ("vienna", "AT"),
        ("прага", "CZ"),
        ("prague", "CZ"),
        ("будапешт", "HU"),
        ("budapest", "HU"),
        ("бухарест", "RO"),
        ("bucharest", "RO"),
        ("софия", "BG"),
        ("sofia", "BG"),
        ("афины", "GR"),
        ("athens", "GR"),
        ("мадрид", "ES"),
        ("madrid", "ES"),
        ("лиссабон", "PT"),
        ("lisbon", "PT"),
        ("милан", "IT"),
        ("milan", "IT"),
        ("рим", "IT"),
        ("rome", "IT"),
        ("осло", "NO"),
        ("oslo", "NO"),
        ("стокгольм", "SE"),
        ("stockholm", "SE"),
        ("копенгаген", "DK"),
        ("copenhagen", "DK"),
        ("минск", "BY"),
        ("minsk", "BY"),
        ("кишинёв", "MD"),
        ("кишинев", "MD"),
        ("chisinau", "MD"),
        ("ташкент", "UZ"),
        ("tashkent", "UZ"),
        ("бишкек", "KG"),
        ("bishkek", "KG"),
        ("мумбаи", "IN"),
        ("mumbai", "IN"),
        ("дели", "IN"),
        ("delhi", "IN"),
        ("бангалор", "IN"),
        ("bangalore", "IN"),
        ("джакарта", "ID"),
        ("jakarta", "ID"),
        ("бангкок", "TH"),
        ("bangkok", "TH"),
        ("ханой", "VN"),
        ("hanoi", "VN"),
        ("манила", "PH"),
        ("manila", "PH"),
        ("каир", "EG"),
        ("cairo", "EG"),
        ("йоханнесбург", "ZA"),
        ("johannesburg", "ZA"),
        ("россия", "RU"),
        ("russia", "RU"),
        ("германия", "DE"),
        ("germany", "DE"),
        ("нидерланды", "NL"),
        ("netherlands", "NL"),
        ("финляндия", "FI"),
        ("finland", "FI"),
        ("япония", "JP"),
        ("japan", "JP"),
        ("сша", "US"),
        ("usa", "US"),
        ("united states", "US"),
        ("великобритания", "GB"),
        ("united kingdom", "GB"),
        ("england", "GB"),
        ("франция", "FR"),
        ("france", "FR"),
        ("швейцария", "CH"),
        ("switzerland", "CH"),
        ("швеция", "SE"),
        ("sweden", "SE"),
        ("польша", "PL"),
        ("poland", "PL"),
        ("турция", "TR"),
        ("turkey", "TR"),
    ];
    let name = name.to_lowercase();
    PLACES
        .iter()
        .find(|(needle, _)| name.starts_with(needle))
        .map(|(_, code)| *code)
}

fn server_badge(server: &Server) -> String {
    if let Some(code) = country_code(&server.name) {
        return code.to_string();
    }
    if let Some(word) = server.name.split_whitespace().next() {
        let letters: String = word.chars().filter(|c| c.is_alphabetic()).take(2).collect();
        if !letters.is_empty() {
            return letters.to_uppercase();
        }
    }
    server
        .protocol
        .label()
        .chars()
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

fn server_target(server: &Server) -> String {
    format!("{}:{}", server.address, server.port)
}

fn server_group(snapshot: &Snapshot, server: &Server) -> String {
    server
        .subscription_id
        .as_deref()
        .and_then(|id| snapshot.subscription(id))
        .map(|sub| sub.name.clone())
        .unwrap_or_else(|| "Вручную".into())
}

fn latency_color(p: &Palette, latency: u32) -> Color32 {
    if latency < 70 {
        p.green
    } else if latency < 140 {
        p.amber
    } else {
        p.red
    }
}

fn server_compatible(server: &Server, core: CoreKind, capabilities: CoreCapabilities) -> bool {
    match core {
        CoreKind::Singbox | CoreKind::Protogrid => {
            (server.protocol != Protocol::Masque || capabilities.masque)
                && (server.transport != Transport::Xhttp || capabilities.xhttp)
                && (server.transport != Transport::Kcp || capabilities.kcp)
                && (server.transport != Transport::XDrive || capabilities.xdrive)
        }
        CoreKind::Xray => {
            matches!(
                server.protocol,
                Protocol::Vless
                    | Protocol::Vmess
                    | Protocol::Trojan
                    | Protocol::Shadowsocks
                    | Protocol::Masque
            ) && server.transport != Transport::Http
        }
    }
}

fn rule_action_label(action: RuleAction) -> &'static str {
    match action {
        RuleAction::Proxy => "Через прокси",
        RuleAction::Direct => "Напрямую",
        RuleAction::Block => "Блокировать",
    }
}

fn rule_kind_label(kind: RuleKind) -> &'static str {
    match kind {
        RuleKind::Domain => "Домен",
        RuleKind::DomainSuffix => "Суффикс домена",
        RuleKind::DomainKeyword => "Ключевое слово",
        RuleKind::IpCidr => "IP / CIDR",
        RuleKind::Geoip => "GeoIP",
        RuleKind::Geosite => "GeoSite",
        RuleKind::ProcessName => "Процесс",
    }
}

fn rule_kinds() -> Vec<(RuleKind, &'static str)> {
    vec![
        (RuleKind::Domain, "Домен"),
        (RuleKind::DomainSuffix, "Суффикс домена"),
        (RuleKind::DomainKeyword, "Ключевое слово"),
        (RuleKind::IpCidr, "IP / CIDR"),
        (RuleKind::Geoip, "GeoIP"),
        (RuleKind::Geosite, "GeoSite"),
        (RuleKind::ProcessName, "Процесс"),
    ]
}

fn rule_hint(kind: RuleKind) -> &'static str {
    match kind {
        RuleKind::Domain => "example.com",
        RuleKind::DomainSuffix => "example.com",
        RuleKind::DomainKeyword => "google",
        RuleKind::IpCidr => "10.0.0.0/8, 192.168.0.0/16",
        RuleKind::Geoip => "ru",
        RuleKind::Geosite => "category-ru, category-ads-all",
        RuleKind::ProcessName => "chrome.exe",
    }
}

fn action_badge(ui: &mut Ui, p: &Palette, action: RuleAction) {
    let (text, fg, bg, stroke) = match action {
        RuleAction::Proxy => ("PROXY", p.accent_2, p.accent_soft, Stroke::NONE),
        RuleAction::Direct => ("DIRECT", p.muted, p.panel_2, Stroke::new(1.0, p.line)),
        RuleAction::Block => ("BLOCK", p.red, p.red_soft, Stroke::NONE),
    };
    pill(ui, text, fg, bg, stroke);
}

fn level_color(p: &Palette, level: &str) -> Color32 {
    match level {
        "error" => p.red,
        "warn" => p.amber,
        "debug" => p.faint,
        _ => p.accent_2,
    }
}

/* -------------------------------------------------------------------------- */
/* Sidebar & topbar                                                           */
/* -------------------------------------------------------------------------- */

fn logo(ui: &mut Ui, p: &Palette, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same((size * 0.3) as u8), p.accent);
    let triangle = vec![
        egui::pos2(rect.center().x - 1.0, rect.top() + size * 0.2),
        egui::pos2(rect.center().x + size * 0.18, rect.center().y),
        egui::pos2(rect.center().x - 1.0, rect.bottom() - size * 0.2),
        egui::pos2(rect.center().x - size * 0.18, rect.center().y),
    ];
    ui.painter().add(egui::Shape::convex_polygon(
        triangle,
        Color32::WHITE,
        Stroke::NONE,
    ));
}

fn nav_item(
    ui: &mut Ui,
    p: &Palette,
    icon: &str,
    label: &str,
    selected: bool,
    collapsed: bool,
    badge: Option<usize>,
) -> egui::Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 38.0), Sense::click());
    let hovered = response.hovered();
    if selected {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(8), p.accent_soft);
        // Full-height 2 px accent border on the left edge, following the
        // rounded corners (the reference uses `border-left` on the pill).
        // The clip is as wide as the corner radius so the arc at the top and
        // bottom stays visible instead of being cut into a short straight bar.
        let border_clip =
            Rect::from_min_max(rect.min, egui::pos2(rect.left() + 8.0, rect.bottom()));
        ui.painter().with_clip_rect(border_clip).rect_stroke(
            rect,
            CornerRadius::same(8),
            Stroke::new(2.0, p.accent),
            egui::StrokeKind::Inside,
        );
    } else if hovered {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(8), p.panel_2);
    }
    let color = if selected {
        p.accent_2
    } else if hovered {
        p.text
    } else {
        p.muted
    };
    let icon_color = if selected { p.accent_2 } else { p.faint };
    let icon_x = if collapsed {
        rect.center().x
    } else {
        rect.left() + 19.0
    };
    ui.painter().text(
        egui::pos2(icon_x, rect.center().y),
        Align2::CENTER_CENTER,
        icon,
        FontId::new(17.0, FontFamily::Proportional),
        icon_color,
    );
    if !collapsed {
        ui.painter().text(
            egui::pos2(rect.left() + 36.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            medium_font(13.5),
            color,
        );
        if let Some(count) = badge {
            let text = count.to_string();
            let tw = measure(ui, &text, FontId::new(10.0, FontFamily::Monospace));
            let badge_rect = Rect::from_center_size(
                egui::pos2(rect.right() - 13.0 - tw / 2.0, rect.center().y),
                Vec2::new((tw + 10.0).max(20.0), 18.0),
            );
            ui.painter().rect(
                badge_rect,
                CornerRadius::same(5),
                if selected { p.accent_soft } else { p.panel_2 },
                Stroke::NONE,
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                badge_rect.center(),
                Align2::CENTER_CENTER,
                text,
                FontId::new(10.0, FontFamily::Monospace),
                if selected { p.accent_2 } else { p.faint },
            );
        }
    }
    response
}

fn sidebar(ui: &mut Ui, p: &Palette, view: &mut View, snapshot: &Snapshot, collapsed: bool) {
    ui.set_width(ui.available_width());
    ui.horizontal(|ui| {
        logo(ui, p, 30.0);
        if !collapsed {
            ui.add_space(2.0);
            ui.label(theme::bold("ProtoGrid", 19.0));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let icon = if view.dark {
                egui_phosphor::regular::MOON
            } else {
                egui_phosphor::regular::SUN
            };
            if round_icon_button(ui, p, icon, "Сменить тему").clicked() {
                view.dark = !view.dark;
                theme::install(ui.ctx(), view.dark);
            }
            if !collapsed
                && round_icon_button(ui, p, egui_phosphor::regular::QUESTION, "Справка").clicked()
            {
                view.modal = Some(ModalState::Help);
            }
        });
    });
    ui.add_space(16.0);

    if !collapsed {
        let id = Id::new("side-search");
        let mut submit = false;
        Frame::new()
            .fill(p.panel_2)
            .stroke(Stroke::new(1.0, p.line))
            .corner_radius(8)
            .inner_margin(Margin::symmetric(9, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS)
                            .size(14.0)
                            .color(p.faint),
                    );
                    let response = ui.add(
                        TextEdit::singleline(&mut view.search)
                            .id(id)
                            .hint_text("Поиск…")
                            .frame(egui::Frame::NONE)
                            .desired_width(ui.available_width() - 46.0),
                    );
                    submit = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        Frame::new()
                            .stroke(Stroke::new(1.0, p.line))
                            .corner_radius(4)
                            .inner_margin(Margin::symmetric(4, 0))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("Ctrl K")
                                        .size(8.5)
                                        .color(p.faint)
                                        .family(FontFamily::Monospace),
                                );
                            });
                    });
                });
            });
        if !view.search.trim().is_empty() || submit {
            view.page = Page::Servers;
        }
        ui.add_space(14.0);
    }

    ui.spacing_mut().item_spacing.y = 1.0;
    let server_count = snapshot.config.servers.len();
    for (index, page) in Page::ALL.iter().enumerate() {
        let badge = if *page == Page::Servers && server_count > 0 {
            Some(server_count)
        } else {
            None
        };
        if nav_item(
            ui,
            p,
            page.icon(),
            page.label(),
            view.page == *page,
            collapsed,
            badge,
        )
        .clicked()
        {
            view.page = *page;
            if *page == Page::Settings {
                view.settings_draft = Some(snapshot.config.settings.clone());
                view.settings_dirty = false;
            }
            let _ = index;
        }
    }
    ui.spacing_mut().item_spacing.y = 7.0;

    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        if !collapsed {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .size(10.0)
                        .color(p.faint),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new("native · egui").size(10.0).color(p.faint));
                });
            });
            ui.add_space(8.0);
        }
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), if collapsed { 40.0 } else { 54.0 }),
            Sense::click(),
        );
        let hovered = response.hovered();
        ui.painter().rect(
            rect,
            CornerRadius::same(8),
            p.panel_2,
            Stroke::new(1.0, if hovered { p.line_strong } else { p.line }),
            egui::StrokeKind::Inside,
        );
        if collapsed {
            let color = status_color(p, snapshot.status);
            ui.painter().circle_filled(rect.center(), 4.5, color);
        } else {
            let color = status_color(p, snapshot.status);
            let dot_center = egui::pos2(rect.left() + 16.0, rect.top() + 18.0);
            if matches!(
                snapshot.status,
                ConnStatus::Connected | ConnStatus::Connecting
            ) {
                ui.painter().circle_filled(
                    dot_center,
                    7.0,
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 28),
                );
            }
            ui.painter().circle_filled(dot_center, 3.5, color);
            ui.painter().text(
                egui::pos2(rect.left() + 28.0, rect.top() + 13.0),
                Align2::LEFT_CENTER,
                snapshot.status.label(),
                medium_font(12.0),
                p.text,
            );
            let subtitle = snapshot
                .active_server()
                .map(|server| {
                    let ping = snapshot
                        .pings
                        .get(&server.id)
                        .copied()
                        .flatten()
                        .map(|value| format!(" · {value} мс"))
                        .unwrap_or_default();
                    format!("{}{ping}", server.display_name())
                })
                .unwrap_or_else(|| "Сервер не выбран".into());
            ui.painter().text(
                egui::pos2(rect.left() + 28.0, rect.top() + 33.0),
                Align2::LEFT_CENTER,
                truncate(&subtitle, 24),
                FontId::proportional(10.0),
                p.faint,
            );
        }
        if response.clicked() {
            view.page = Page::Overview;
        }
    });
}

fn truncate(text: &str, limit: usize) -> String {
    let count = text.chars().count();
    if count <= limit {
        text.to_string()
    } else {
        let mut value: String = text.chars().take(limit.saturating_sub(1)).collect();
        value.push('…');
        value
    }
}

fn status_pill(ui: &mut Ui, p: &Palette, snapshot: &Snapshot) -> egui::Response {
    let label = snapshot.status.label();
    let core = snapshot
        .core(snapshot.config.settings.core)
        .and_then(|info| info.version.as_deref())
        .map(|version| format!("{version}"))
        .unwrap_or_else(|| snapshot.config.settings.core.name().to_string());
    let font = medium_font(12.0);
    let width = 12.0
        + 8.0
        + 9.0
        + measure(ui, label, font.clone())
        + 9.0
        + measure(ui, &core, FontId::monospace(10.0))
        + 12.0;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width.max(110.0), 32.0), Sense::click());
    ui.painter().rect(
        rect,
        CornerRadius::same(99),
        p.panel,
        Stroke::new(1.0, p.line),
        egui::StrokeKind::Inside,
    );
    let color = status_color(p, snapshot.status);
    ui.painter()
        .circle_filled(egui::pos2(rect.left() + 14.0, rect.center().y), 3.5, color);
    let mut x = rect.left() + 24.0;
    ui.painter().text(
        egui::pos2(x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font.clone(),
        p.muted,
    );
    x += measure(ui, label, font) + 9.0;
    ui.painter().line_segment(
        [
            egui::pos2(x - 5.0, rect.center().y - 8.0),
            egui::pos2(x - 5.0, rect.center().y + 8.0),
        ],
        Stroke::new(1.0, p.line),
    );
    ui.painter().text(
        egui::pos2(x, rect.center().y),
        Align2::LEFT_CENTER,
        &core,
        FontId::monospace(10.0),
        p.faint,
    );
    response
}

fn server_pill(ui: &mut Ui, p: &Palette, snapshot: &Snapshot) -> egui::Response {
    let name = snapshot
        .active_server()
        .map(|server| truncate(&server.display_name(), 18))
        .unwrap_or_else(|| "Выберите сервер".into());
    let badge = snapshot
        .active_server()
        .map(server_badge)
        .unwrap_or_else(|| "—".into());
    let font = medium_font(12.5);
    let width = 9.0 + 26.0 + 8.0 + measure(ui, &name, font.clone()).min(170.0) + 8.0 + 14.0 + 9.0;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width.max(150.0), 32.0), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(8), p.panel_2);
    }
    let badge_rect = Rect::from_min_size(
        egui::pos2(rect.left() + 9.0, rect.center().y - 9.0),
        Vec2::new(26.0, 18.0),
    );
    ui.painter().rect(
        badge_rect,
        CornerRadius::same(5),
        p.panel_2,
        Stroke::new(1.0, p.line),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        badge_rect.center(),
        Align2::CENTER_CENTER,
        &badge,
        FontId::monospace(9.5),
        p.muted,
    );
    let text_x = badge_rect.right() + 8.0;
    let clip = Rect::from_min_max(
        egui::pos2(text_x, rect.top()),
        egui::pos2(rect.right() - 20.0, rect.bottom()),
    );
    ui.painter().with_clip_rect(clip).text(
        egui::pos2(text_x, rect.center().y),
        Align2::LEFT_CENTER,
        &name,
        font,
        if response.hovered() { p.text } else { p.muted },
    );
    ui.painter().text(
        egui::pos2(rect.right() - 14.0, rect.center().y),
        Align2::CENTER_CENTER,
        egui_phosphor::regular::CARET_DOWN,
        FontId::new(12.0, FontFamily::Proportional),
        p.faint,
    );
    response
}

fn topbar(ui: &mut Ui, p: &Palette, view: &mut View, snapshot: &Snapshot, engine: &Engine) {
    ui.horizontal(|ui| {
        ui.set_height(64.0);
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            if status_pill(ui, p, snapshot).clicked() {
                view.page = Page::Overview;
            }
            ui.add_space(2.0);
            if server_pill(ui, p, snapshot).clicked() {
                view.page = Page::Servers;
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if top_button(
                ui,
                p,
                egui_phosphor::regular::GEAR_SIX,
                "Настройки",
                BtnKind::Ghost,
                true,
            )
            .clicked()
            {
                view.page = Page::Settings;
                view.settings_draft = Some(snapshot.config.settings.clone());
                view.settings_dirty = false;
            }
            if top_button(
                ui,
                p,
                egui_phosphor::regular::TERMINAL_WINDOW,
                "Логи",
                BtnKind::Ghost,
                true,
            )
            .clicked()
            {
                view.page = Page::Logs;
            }
            let (divider_rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 22.0), Sense::hover());
            ui.painter().rect_filled(divider_rect, 0, p.line);
            if top_button(
                ui,
                p,
                egui_phosphor::regular::ARROWS_CLOCKWISE,
                "Перезапуск",
                BtnKind::Outline,
                snapshot.active_server().is_some(),
            )
            .clicked()
            {
                engine.restart_or_stop();
            }
            if top_button(
                ui,
                p,
                egui_phosphor::regular::ACTIVITY,
                "Пинг",
                BtnKind::Ghost,
                !snapshot.config.servers.is_empty(),
            )
            .clicked()
            {
                engine.ping_all();
            }
            let busy = snapshot.busy("connection");
            let label = if busy {
                "Подключение…"
            } else if snapshot.is_connected() {
                "Отключиться"
            } else {
                "Подключиться"
            };
            let kind = BtnKind::Primary;
            let enabled = !busy && (snapshot.active_server().is_some() || snapshot.is_connected());
            let icon = if busy {
                egui_phosphor::regular::CIRCLE_NOTCH
            } else {
                egui_phosphor::regular::POWER
            };
            if top_button(ui, p, icon, label, kind, enabled).clicked() {
                if snapshot.is_connected() || snapshot.is_connecting() {
                    engine.disconnect();
                } else {
                    engine.connect();
                }
            }
        });
    });
}

/* -------------------------------------------------------------------------- */
/* Overview                                                                   */
/* -------------------------------------------------------------------------- */

fn stat_card(
    ui: &mut Ui,
    p: &Palette,
    icon: &str,
    label: &str,
    value: &str,
    unit: &str,
    left_hint: String,
    right_hint: String,
    values: &[f32],
    color: Color32,
    icon_bg: Color32,
) {
    Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(14)
        .inner_margin(Margin {
            left: 16,
            right: 16,
            top: 15,
            bottom: 0,
        })
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                icon_box(ui, p, icon, color, icon_bg, 21.0);
                ui.label(
                    RichText::new(label.to_uppercase())
                        .size(10.0)
                        .color(p.faint)
                        .family(FontFamily::Name("Medium".into())),
                );
            });
            ui.add_space(9.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(theme::semibold(value, 32.0));
                ui.label(RichText::new(unit).size(12.0).color(p.faint));
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(left_hint).size(10.0).color(p.faint));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(right_hint).size(10.0).color(p.faint));
                });
            });
            ui.add_space(4.0);
            sparkline(ui, p, values, color, 46.0);
        });
}

fn overview_page(ui: &mut Ui, p: &Palette, view: &mut View, snapshot: &Snapshot, engine: &Engine) {
    if let Some(error) = &snapshot.error {
        alert_banner(ui, p, error, true, |ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new("Открыть журнал").size(11.0).color(p.red))
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::new(1.0, p.red_soft))
                        .corner_radius(7),
                )
                .clicked()
            {
                view.page = Page::Logs;
            }
        });
        ui.add_space(12.0);
    } else if let Some(warning) = &snapshot.warning {
        alert_banner(ui, p, warning, false, |ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new("Настройки").size(11.0).color(p.amber))
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::new(1.0, p.amber_soft))
                        .corner_radius(7),
                )
                .clicked()
            {
                view.page = Page::Settings;
            }
        });
        ui.add_space(12.0);
    } else if snapshot
        .core(snapshot.config.settings.core)
        .map(|core| !core.found)
        .unwrap_or(true)
    {
        alert_banner(
            ui,
            p,
            "Ядро не найдено. Скачайте sing-box или Xray в настройках.",
            false,
            |ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new("Настроить").size(11.0).color(p.amber))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::new(1.0, p.amber_soft))
                            .corner_radius(7),
                    )
                    .clicked()
                {
                    view.page = Page::Settings;
                }
            },
        );
        ui.add_space(12.0);
    }

    let points = view.last_points(view.ov_range);
    let down = view.series.back().copied().unwrap_or_default();
    let down_values: Vec<f32> = points.iter().map(|pt| pt.down).collect();
    let up_values: Vec<f32> = points.iter().map(|pt| pt.up).collect();
    let conn_values: Vec<f32> = points.iter().map(|pt| pt.conns).collect();
    let ping_values: Vec<f32> = points.iter().map(|pt| pt.ping).collect();
    let avg = |values: &[f32]| {
        if values.is_empty() {
            0.0
        } else {
            values.iter().sum::<f32>() / values.len() as f32
        }
    };
    let peak = |values: &[f32]| values.iter().copied().fold(0.0_f32, f32::max);
    let (down_value, down_unit) = rate(down.down);
    let (up_value, up_unit) = rate(down.up);
    let active = snapshot.active_server();
    let latency = active
        .and_then(|server| snapshot.pings.get(&server.id).copied().flatten())
        .unwrap_or(0);

    card_grid(ui, 4, 14.0, |ui, index| match index {
        0 => stat_card(
            ui,
            p,
            egui_phosphor::regular::ARROW_DOWN,
            "Входящий",
            &down_value,
            down_unit,
            format!("среднее {}", rate_plain(avg(&down_values))),
            format!("пик {}", rate_plain(peak(&down_values))),
            &down_values,
            p.accent_2,
            p.accent_soft,
        ),
        1 => stat_card(
            ui,
            p,
            egui_phosphor::regular::ARROW_UP,
            "Исходящий",
            &up_value,
            up_unit,
            format!("среднее {}", rate_plain(avg(&up_values))),
            format!("пик {}", rate_plain(peak(&up_values))),
            &up_values,
            p.cyan,
            p.cyan_soft,
        ),
        2 => stat_card(
            ui,
            p,
            egui_phosphor::regular::ACTIVITY,
            "Задержка",
            &latency.to_string(),
            "мс",
            active
                .map(|server| truncate(&server.display_name(), 18))
                .unwrap_or_else(|| "нет сервера".into()),
            "потери 0%".into(),
            &ping_values,
            p.green,
            p.green_soft,
        ),
        _ => stat_card(
            ui,
            p,
            egui_phosphor::regular::WIFI_HIGH,
            "Соединения",
            &format!("{:.0}", down.conns),
            "шт",
            "TCP + UDP".into(),
            if snapshot.is_connected() {
                "активно".into()
            } else {
                "нет связи".into()
            },
            &conn_values,
            p.violet,
            p.violet_soft,
        ),
    });
    ui.add_space(14.0);

    let available = ui.available_width();
    let gap = 14.0;
    let left = (available - gap) * 0.618;
    let right = available - gap - left;
    let inherited_spacing = ui.spacing().item_spacing;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.allocate_ui_with_layout(Vec2::new(left, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing = inherited_spacing;
            ui.set_min_width(left);
            ui.set_max_width(left);
            speed_card(ui, p, view, snapshot, engine, &points);
        });
        ui.add_space(gap);
        ui.allocate_ui_with_layout(Vec2::new(right, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing = inherited_spacing;
            ui.set_min_width(right);
            ui.set_max_width(right);
            route_card(ui, p, view, snapshot);
        });
    });
    ui.add_space(14.0);
    info_card(ui, p, snapshot, engine);
}

fn alert_banner(
    ui: &mut Ui,
    p: &Palette,
    message: &str,
    error: bool,
    actions: impl FnOnce(&mut Ui),
) {
    let color = if error { p.red } else { p.amber };
    let fill = if error { p.red_soft } else { p.amber_soft };
    Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.35)))
        .corner_radius(10)
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(if error {
                        egui_phosphor::regular::X_CIRCLE
                    } else {
                        egui_phosphor::regular::WARNING
                    })
                    .size(16.0)
                    .color(color),
                );
                ui.add(
                    egui::Label::new(RichText::new(message).size(11.5).color(color))
                        .truncate()
                        .selectable(false),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), actions);
            });
        });
}

fn speed_card(
    ui: &mut Ui,
    p: &Palette,
    view: &mut View,
    snapshot: &Snapshot,
    engine: &Engine,
    points: &VecDeque<TrafficPoint>,
) {
    Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(14)
        .inner_margin(Margin {
            left: 18,
            right: 18,
            top: 16,
            bottom: 0,
        })
        .show(ui, |ui| {
            ui.set_min_height(304.0);
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(theme::semibold("Общая скорость", 13.5));
                    let peak = points
                        .iter()
                        .map(|point| point.down.max(point.up))
                        .fold(0.0_f32, f32::max);
                    ui.label(
                        RichText::new(format!("интерфейс · пик {}", rate_plain(peak)))
                            .size(11.0)
                            .color(p.faint),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(range) = seg(
                        ui,
                        p,
                        &[(60, "60 с"), (300, "5 мин"), (900, "15 мин")],
                        view.ov_range,
                    ) {
                        view.ov_range = range;
                    }
                    ui.add_enabled_ui(snapshot.is_connected(), |ui| {
                        if round_icon_button(
                            ui,
                            p,
                            egui_phosphor::regular::ACTIVITY,
                            "Проверить текущее соединение",
                        )
                        .clicked()
                        {
                            engine.url_test();
                        }
                    });
                });
            });
            ui.add_space(4.0);
            let height = 190.0;
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
            chart_grid(ui, p, rect);
            let down: Vec<f32> = points.iter().map(|point| point.down).collect();
            let up: Vec<f32> = points.iter().map(|point| point.up).collect();
            let max = down
                .iter()
                .chain(up.iter())
                .copied()
                .fold(0.0_f32, f32::max)
                * 1.15;
            paint_area(ui, rect, &down, max, p.accent, 44, 2.0);
            paint_area(ui, rect, &up, max, p.cyan, 38, 1.8);
            ui.add_space(14.0);
            let (line, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(line, 0, p.line);
            ui.add_space(10.0);
            let avg_up = if points.is_empty() {
                0.0
            } else {
                points.iter().map(|point| point.up).sum::<f32>() / points.len() as f32
            };
            let avg_down = if points.is_empty() {
                0.0
            } else {
                points.iter().map(|point| point.down).sum::<f32>() / points.len() as f32
            };
            let texts = [
                ("Получено", bytes_f(view.totals.down)),
                ("Отправлено", bytes_f(view.totals.up)),
                (
                    "Средняя скорость",
                    format!("↑ {}  ↓ {}", rate_plain(avg_up), rate_plain(avg_down)),
                ),
            ];
            card_grid(ui, 3, 16.0, |ui, index| {
                let (label, value) = &texts[index];
                ui.label(
                    RichText::new(label.to_uppercase())
                        .size(9.5)
                        .color(p.faint)
                        .family(FontFamily::Name("Medium".into())),
                );
                ui.label(theme::semibold(value, 14.0));
            });
            ui.add_space(14.0);
            let _ = snapshot;
        });
}

fn route_card(ui: &mut Ui, p: &Palette, view: &mut View, snapshot: &Snapshot) {
    let total = view.totals.proxy_down
        + view.totals.proxy_up
        + view.totals.direct_down
        + view.totals.direct_up;
    let proxy = view.totals.proxy_down + view.totals.proxy_up;
    let share = if total > 0.0 {
        (proxy / total * 100.0).round() as u32
    } else {
        0
    };
    let current = view.series.back().copied().unwrap_or_default();
    Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(14)
        .inner_margin(Margin::same(18))
        .show(ui, |ui| {
            ui.set_min_height(298.0);
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(theme::semibold("Маршруты", 13.5));
                    ui.label(
                        RichText::new("трафик через прокси и напрямую")
                            .size(11.0)
                            .color(p.faint),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Настроить →").size(11.5).color(p.faint),
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        view.page = Page::Routing;
                    }
                });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(theme::semibold(format!("{share}%"), 40.0));
                ui.label(RichText::new("через прокси").size(11.5).color(p.faint));
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                if legend(ui, p.accent, "Прокси", &bytes_f(proxy)) {}
                if legend(ui, p.amber, "Напрямую", &bytes_f(total - proxy)) {}
            });
            ui.add_space(6.0);
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 74.0), Sense::hover());
            let points = view.last_points(60);
            let proxy_values: Vec<f32> = points
                .iter()
                .map(|point| point.proxy_down + point.proxy_up)
                .collect();
            let stack_values: Vec<f32> = points
                .iter()
                .map(|point| {
                    point.proxy_down + point.proxy_up + point.direct_down + point.direct_up
                })
                .collect();
            let max = stack_values.iter().copied().fold(0.0_f32, f32::max) * 1.2;
            paint_area(ui, rect, &stack_values, max, p.amber, 42, 1.6);
            paint_area(ui, rect, &proxy_values, max, p.accent, 48, 1.6);
            ui.add_space(10.0);
            let (line, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(line, 0, p.line);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "Прокси ↓ {} ↑ {}",
                        rate_plain(current.proxy_down),
                        rate_plain(current.proxy_up)
                    ))
                    .size(10.0)
                    .color(p.faint),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "Напрямую ↓ {} ↑ {}",
                            rate_plain(current.direct_down),
                            rate_plain(current.direct_up)
                        ))
                        .size(10.0)
                        .color(p.faint),
                    );
                });
            });
            let _ = snapshot;
        });
}

fn legend(ui: &mut Ui, color: Color32, label: &str, value: &str) -> bool {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(9.0), Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::same(2), color);
        ui.label(RichText::new(label).size(11.0).color(p_muted(ui)));
        ui.label(theme::semibold(value, 11.0));
    });
    false
}

fn p_muted(ui: &Ui) -> Color32 {
    Palette::from_ctx(ui.ctx()).muted
}

fn info_card(ui: &mut Ui, p: &Palette, snapshot: &Snapshot, engine: &Engine) {
    Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(14)
        .inner_margin(Margin::symmetric(18, 15))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let session = snapshot
                .connected_at
                .map(|start| uptime(now_ms() - start))
                .unwrap_or_else(|| "—".into());
            let server = snapshot.active_server();
            let name = server
                .map(|server| server.display_name().to_string())
                .unwrap_or_else(|| "Сервер не выбран".into());
            let meta = server
                .map(|server| {
                    format!(
                        "{} · {} · {}",
                        protocol_name(server.protocol),
                        security_short(server.security),
                        server_target(server)
                    )
                })
                .unwrap_or_else(|| "Добавьте подключение".into());
            let settings = &snapshot.config.settings;
            let ports = match settings.core {
                CoreKind::Singbox | CoreKind::Protogrid => {
                    format!("mixed:{}", settings.mixed_port)
                }
                CoreKind::Xray => format!(
                    "socks:{} · http:{}",
                    settings.socks_port, settings.http_port
                ),
            };
            let lan_tun = format!(
                "LAN: {} · TUN: {}",
                if settings.allow_lan {
                    "вкл"
                } else {
                    "нет"
                },
                if settings.tun_enabled {
                    "вкл"
                } else {
                    "нет"
                }
            );
            let core = snapshot.core(settings.core);
            let core_value = format!(
                "{} {}",
                settings.core.name(),
                core.and_then(|core| core.version.clone())
                    .unwrap_or_default()
            );
            let core_hint = format!(
                "{}маршрутизация: {}",
                snapshot
                    .pid
                    .map(|pid| format!("PID {pid} · "))
                    .unwrap_or_default(),
                match settings.final_action {
                    RuleAction::Proxy => "через прокси",
                    RuleAction::Direct => "напрямую",
                    RuleAction::Block => "блокировка",
                }
            );
            let cells = [
                ("Сессия", session.as_str(), "с момента подключения"),
                ("Активный сервер", name.as_str(), meta.as_str()),
                ("Локально", ports.as_str(), lan_tun.as_str()),
                ("Ядро", core_value.as_str(), core_hint.as_str()),
            ];
            card_grid(ui, 4, 0.0, |ui, index| {
                let (label, value, hint) = cells[index];
                info_cell(ui, p, label, value, hint);
            });
            let _ = engine;
        });
}

fn info_cell(ui: &mut Ui, p: &Palette, label: &str, value: &str, hint: &str) {
    ui.label(
        RichText::new(label.to_uppercase())
            .size(9.0)
            .color(p.faint)
            .family(FontFamily::Name("Medium".into())),
    );
    ui.add_space(2.0);
    ui.add(
        egui::Label::new(theme::semibold(value, 14.0))
            .truncate()
            .selectable(false),
    );
    ui.add_space(1.0);
    ui.add(
        egui::Label::new(RichText::new(hint).size(10.5).color(p.faint))
            .truncate()
            .selectable(false),
    );
}

/* -------------------------------------------------------------------------- */
/* Servers                                                                    */
/* -------------------------------------------------------------------------- */

fn search_box(ui: &mut Ui, p: &Palette, value: &mut String, hint: &str, width: f32) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 34.0), Sense::click());
    let radius = CornerRadius::same(8);
    ui.painter().rect(
        rect,
        radius,
        p.panel_2,
        Stroke::new(1.0, p.line),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 17.5, rect.center().y),
        Align2::CENTER_CENTER,
        egui_phosphor::regular::MAGNIFYING_GLASS,
        FontId::new(15.0, FontFamily::Proportional),
        p.faint,
    );
    let edit_rect = Rect::from_min_max(
        egui::pos2(rect.left() + 32.0, rect.top() + 3.0),
        egui::pos2(rect.right() - 10.0, rect.bottom() - 3.0),
    );
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(edit_rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    let edit = child.add(
        TextEdit::singleline(value)
            .hint_text(hint)
            .frame(egui::Frame::NONE)
            .desired_width(edit_rect.width()),
    );
    if response.clicked() {
        edit.request_focus();
    }
    edit.changed()
}

fn servers_page(ui: &mut Ui, p: &Palette, view: &mut View, snapshot: &Snapshot, engine: &Engine) {
    let groups: Vec<String> = {
        let mut set = BTreeSet::new();
        for server in &snapshot.config.servers {
            set.insert(server_group(snapshot, server));
        }
        set.into_iter().collect()
    };
    let subtitle = format!(
        "{} подключений · {} {}",
        snapshot.config.servers.len(),
        groups.len(),
        plural(groups.len(), "группа", "группы", "групп")
    );
    page_head(ui, p, "Серверы", &subtitle, |ui| {
        if top_button(
            ui,
            p,
            egui_phosphor::regular::PLUS,
            "Добавить",
            BtnKind::Primary,
            true,
        )
        .clicked()
        {
            let server = Server::new(Protocol::Vless);
            view.server_alpn = server.alpn.join(", ");
            view.server_draft = Some(server);
            view.modal = Some(ModalState::Server);
        }
        if top_button(
            ui,
            p,
            egui_phosphor::regular::ACTIVITY,
            "Проверить пинг",
            BtnKind::Ghost,
            !snapshot.config.servers.is_empty(),
        )
        .clicked()
        {
            engine.ping_all();
        }
        search_box(ui, p, &mut view.search, "Поиск сервера…", 250.0);
    });

    Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(14)
        .inner_margin(Margin::ZERO)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            Frame::new()
                .inner_margin(Margin::symmetric(16, 10))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        if ptab(
                            ui,
                            p,
                            "Все",
                            snapshot.config.servers.len(),
                            view.group.is_none(),
                        )
                        .clicked()
                        {
                            view.group = None;
                        }
                        for sub in &snapshot.config.subscriptions {
                            let count = snapshot
                                .config
                                .servers
                                .iter()
                                .filter(|server| {
                                    server.subscription_id.as_deref() == Some(sub.id.as_str())
                                })
                                .count();
                            if ptab(
                                ui,
                                p,
                                &sub.name,
                                count,
                                view.group.as_deref() == Some(sub.id.as_str()),
                            )
                            .clicked()
                            {
                                view.group = Some(sub.id.clone());
                            }
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if small_ghost(
                                ui,
                                p,
                                egui_phosphor::regular::ARROWS_DOWN_UP,
                                "Сортировка",
                            )
                            .clicked()
                            {
                                view.sort = match view.sort {
                                    SortMode::Server => SortMode::Name,
                                    SortMode::Name => SortMode::Latency,
                                    SortMode::Latency => SortMode::Server,
                                };
                            }
                        });
                    });
                });
            let (line, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(line, 0, p.line);

            if snapshot.config.servers.is_empty() {
                ui.add_space(10.0);
                empty_state(
                    ui,
                    p,
                    egui_phosphor::regular::HARD_DRIVES,
                    "Пока нет серверов",
                    "Вставьте share-ссылку, добавьте подписку или создайте сервер вручную.",
                    |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Импортировать ссылку")
                                        .size(11.5)
                                        .color(Color32::WHITE),
                                )
                                .fill(p.accent)
                                .corner_radius(8),
                            )
                            .clicked()
                        {
                            view.import_text.clear();
                            view.modal = Some(ModalState::Import);
                        }
                        if ui
                            .add(egui::Button::new("Добавить сервер").corner_radius(8))
                            .clicked()
                        {
                            let server = Server::new(Protocol::Vless);
                            view.server_alpn = server.alpn.join(", ");
                            view.server_draft = Some(server);
                            view.modal = Some(ModalState::Server);
                        }
                    },
                );
                ui.add_space(14.0);
                return;
            }

            let query = view.search.trim().to_lowercase();
            let mut visible: Vec<Server> = snapshot
                .config
                .servers
                .iter()
                .filter(|server| {
                    let group = server_group(snapshot, server);
                    if let Some(selected) = view.group.as_deref() {
                        if server.subscription_id.as_deref() != Some(selected) {
                            return false;
                        }
                    }
                    query.is_empty()
                        || server.name.to_lowercase().contains(&query)
                        || server.address.to_lowercase().contains(&query)
                        || group.to_lowercase().contains(&query)
                        || protocol_name(server.protocol)
                            .to_lowercase()
                            .contains(&query)
                })
                .cloned()
                .collect();
            match view.sort {
                SortMode::Server => {}
                SortMode::Name => {
                    visible.sort_by_key(|server| server.display_name().to_lowercase())
                }
                SortMode::Latency => visible.sort_by_key(|server| {
                    snapshot
                        .pings
                        .get(&server.id)
                        .copied()
                        .flatten()
                        .unwrap_or(u32::MAX)
                }),
            }
            if visible.is_empty() {
                ui.add_space(10.0);
                empty_state(
                    ui,
                    p,
                    egui_phosphor::regular::FUNNEL,
                    "Ничего не найдено",
                    "Измените фильтр или выберите группу «Все».",
                    |ui| {
                        if ui
                            .add(egui::Button::new("Сбросить фильтр").corner_radius(8))
                            .clicked()
                        {
                            view.search.clear();
                            view.group = None;
                        }
                    },
                );
                ui.add_space(14.0);
                return;
            }

            let available = ui.available_width().max(880.0);
            let widths = TableWidths::new(available - 32.0);
            ScrollArea::both()
                .id_salt("servers-table")
                .auto_shrink([false, false])
                .max_height(ui.available_height() - 8.0)
                .show(ui, |ui| {
                    ui.set_min_width(available);
                    // Rows and separators must be flush; the default 7 px item
                    // spacing left gaps above/below the active row fill.
                    ui.spacing_mut().item_spacing.y = 0.0;
                    table_header(ui, p, view, &widths);
                    for (index, server) in visible.iter().enumerate() {
                        server_row(
                            ui,
                            p,
                            view,
                            snapshot,
                            engine,
                            server,
                            index,
                            visible.len(),
                            &widths,
                        );
                    }
                });
            ui.add_space(4.0);
        });
    ui.add_space(8.0);
    ui.label(
        RichText::new(format!(
            "{}  Двойной щелчок по строке выбирает сервер. Кнопка питания подключает ядро.",
            egui_phosphor::regular::INFO
        ))
        .size(10.0)
        .color(p.faint),
    );
}

fn ptab(ui: &mut Ui, p: &Palette, label: &str, count: usize, selected: bool) -> egui::Response {
    let label_font = FontId::proportional(12.0);
    let count_font = FontId::monospace(10.0);
    let count_text = count.to_string();
    let label_width = measure(ui, label, label_font.clone());
    let count_width = measure(ui, &count_text, count_font.clone());
    let width = 10.0 + label_width + 6.0 + count_width + 10.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::click());
    if selected {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(7), p.accent_soft);
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(7), p.panel_2);
    }
    let color = if selected {
        p.accent_2
    } else if response.hovered() {
        p.text
    } else {
        p.faint
    };
    let y = rect.center().y;
    ui.painter().text(
        egui::pos2(rect.left() + 10.0, y),
        Align2::LEFT_CENTER,
        label,
        label_font,
        color,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 10.0 + label_width + 6.0, y),
        Align2::LEFT_CENTER,
        count_text,
        count_font,
        color.gamma_multiply(0.65),
    );
    response
}

struct TableWidths {
    index: f32,
    server: f32,
    protocol: f32,
    group: f32,
    latency: f32,
    speed: f32,
    status: f32,
}

impl TableWidths {
    fn new(available: f32) -> Self {
        let index = 36.0;
        let protocol = 112.0;
        let group = 120.0;
        let latency = 96.0;
        let speed = 90.0;
        // Status owns the right edge (like the reference); row actions sit to
        // its left inside the same column.
        let status = 196.0;
        let server = (available - index - protocol - group - latency - speed - status).max(200.0);
        Self {
            index,
            server,
            protocol,
            group,
            latency,
            speed,
            status,
        }
    }
}

fn table_header(ui: &mut Ui, p: &Palette, view: &mut View, widths: &TableWidths) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 38.0), Sense::hover());
    ui.painter().rect_filled(rect, 0, p.panel_2);
    let inner = Rect::from_min_max(
        egui::pos2(rect.left() + 16.0, rect.top()),
        egui::pos2(rect.right() - 16.0, rect.bottom()),
    );
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
    );
    child.spacing_mut().item_spacing.x = 0.0;
    let cell = |ui: &mut Ui, width: f32, text: &str, right: bool| {
        ui.allocate_ui_with_layout(
            Vec2::new(width, 38.0),
            if right {
                Layout::right_to_left(Align::Center)
            } else {
                Layout::left_to_right(Align::Center)
            },
            |ui| {
                ui.set_min_width(width);
                ui.label(
                    RichText::new(text.to_uppercase())
                        .size(10.0)
                        .color(p.faint)
                        .family(FontFamily::Name("Medium".into())),
                );
            },
        );
    };
    cell(&mut child, widths.index, "#", false);
    let server_label = if view.sort == SortMode::Name {
        "Сервер ▾"
    } else {
        "Сервер"
    };
    let response = child.allocate_ui_with_layout(
        Vec2::new(widths.server, 38.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(widths.server);
            ui.label(RichText::new(server_label).size(10.0).color(
                if view.sort == SortMode::Name {
                    p.accent_2
                } else {
                    p.faint
                },
            ));
        },
    );
    if response.response.interact(Sense::click()).clicked() {
        view.sort = if view.sort == SortMode::Name {
            SortMode::Server
        } else {
            SortMode::Name
        };
    }
    cell(&mut child, widths.protocol, "Протокол", false);
    cell(&mut child, widths.group, "Группа", false);
    let latency_label = if view.sort == SortMode::Latency {
        "Задержка ▴"
    } else {
        "Задержка"
    };
    let response = child.allocate_ui_with_layout(
        Vec2::new(widths.latency, 38.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(widths.latency);
            ui.label(RichText::new(latency_label).size(10.0).color(
                if view.sort == SortMode::Latency {
                    p.accent_2
                } else {
                    p.faint
                },
            ));
        },
    );
    if response.response.interact(Sense::click()).clicked() {
        view.sort = if view.sort == SortMode::Latency {
            SortMode::Server
        } else {
            SortMode::Latency
        };
    }
    cell(&mut child, widths.speed, "Скорость", false);
    cell(&mut child, widths.status, "Статус", true);
    let (line, _) = ui.allocate_exact_size(Vec2::new(rect.width(), 1.0), Sense::hover());
    ui.painter().rect_filled(line, 0, p.line);
}

fn server_row(
    ui: &mut Ui,
    p: &Palette,
    view: &mut View,
    snapshot: &Snapshot,
    engine: &Engine,
    server: &Server,
    index: usize,
    total: usize,
    widths: &TableWidths,
) {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 57.0), Sense::click());
    let active = snapshot.config.active_server_id.as_deref() == Some(server.id.as_str());
    let hovered = ui.rect_contains_pointer(rect);
    if active {
        ui.painter().rect_filled(rect, 0, p.accent_soft);
    } else if hovered {
        ui.painter().rect_filled(rect, 0, p.panel_2);
    }
    if index + 1 < total {
        let (line, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
        ui.painter().rect_filled(line, 0, p.line);
    }

    if response.double_clicked() {
        engine.set_active_server(&server.id);
    }

    let inner = Rect::from_min_max(
        egui::pos2(rect.left() + 16.0, rect.top()),
        egui::pos2(rect.right() - 16.0, rect.bottom()),
    );
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
    );
    child.spacing_mut().item_spacing.x = 0.0;

    child.allocate_ui_with_layout(
        Vec2::new(widths.index, 57.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(widths.index);
            ui.label(
                RichText::new(format!("{}", index + 1))
                    .size(11.0)
                    .color(p.faint)
                    .family(FontFamily::Monospace),
            );
        },
    );

    child.allocate_ui_with_layout(
        Vec2::new(widths.server, 57.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(widths.server);
            ui.spacing_mut().item_spacing.x = 11.0;
            let (badge, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
            ui.painter().rect(
                badge,
                CornerRadius::same(9),
                p.panel_2,
                Stroke::new(1.0, p.line),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                badge.center(),
                Align2::CENTER_CENTER,
                server_badge(server),
                FontId::monospace(10.0),
                p.muted,
            );
            let name_font = FontId::new(13.0, FontFamily::Name("Semibold".into()));
            let address_font = FontId::new(10.5, FontFamily::Monospace);
            let name_height = ui.ctx().fonts_mut(|fonts| fonts.row_height(&name_font));
            let address_height = ui.ctx().fonts_mut(|fonts| fonts.row_height(&address_font));
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), name_height + 1.0 + address_height),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.add(
                        egui::Label::new(theme::semibold(server.display_name(), 13.0))
                            .truncate()
                            .selectable(false),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(server_target(server))
                                .size(10.5)
                                .color(p.faint)
                                .family(FontFamily::Monospace),
                        )
                        .truncate()
                        .selectable(false),
                    );
                },
            );
        },
    );

    child.allocate_ui_with_layout(
        Vec2::new(widths.protocol, 57.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(widths.protocol);
            let sub_font = FontId::new(10.0, FontFamily::Monospace);
            let sub_height = ui.ctx().fonts_mut(|fonts| fonts.row_height(&sub_font));
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), 21.0 + 3.0 + sub_height),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    protocol_badge(ui, p, server.protocol);
                    ui.label(
                        RichText::new(format!(
                            "{} · {}",
                            security_short(server.security),
                            transport_short(server.transport)
                        ))
                        .size(10.0)
                        .color(p.faint)
                        .family(FontFamily::Monospace),
                    );
                },
            );
        },
    );

    child.allocate_ui_with_layout(
        Vec2::new(widths.group, 57.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(widths.group);
            ui.add(
                egui::Label::new(
                    RichText::new(server_group(snapshot, server))
                        .size(12.0)
                        .color(p.muted),
                )
                .truncate()
                .selectable(false),
            );
        },
    );

    child.allocate_ui_with_layout(
        Vec2::new(widths.latency, 57.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(widths.latency);
            if snapshot.loading_pings.contains(&server.id) {
                ui.spinner();
            } else if let Some(Some(value)) = snapshot.pings.get(&server.id) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let color = latency_color(p, *value);
                    let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
                    ui.painter().circle_filled(dot_rect.center(), 3.0, color);
                    ui.label(
                        RichText::new(format!("{value}"))
                            .size(12.0)
                            .color(p.muted)
                            .family(FontFamily::Monospace),
                    );
                    ui.label(RichText::new("мс").size(9.5).color(p.faint));
                });
            } else {
                ui.label(
                    RichText::new("— мс")
                        .size(11.0)
                        .color(p.faint)
                        .family(FontFamily::Monospace),
                );
            }
        },
    );

    child.allocate_ui_with_layout(
        Vec2::new(widths.speed, 57.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(widths.speed);
            if active && snapshot.is_connected() {
                let (value, unit) = rate(snapshot.traffic.down_bps as f32);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    ui.label(
                        RichText::new(value)
                            .size(12.0)
                            .color(p.muted)
                            .family(FontFamily::Monospace),
                    );
                    ui.label(RichText::new(unit).size(9.5).color(p.faint));
                });
            } else {
                ui.label(
                    RichText::new("—")
                        .size(11.0)
                        .color(p.faint)
                        .family(FontFamily::Monospace),
                );
            }
        },
    );

    child.allocate_ui_with_layout(
        Vec2::new(widths.status, 57.0),
        Layout::right_to_left(Align::Center),
        |ui| {
            ui.set_min_width(widths.status);
            if active && snapshot.is_connected() {
                badge(ui, p, "Активен", p.green, p.green_soft, true);
            } else if active {
                badge(ui, p, "Выбран", p.faint, p.panel_2, true);
            } else {
                if ui
                    .add(
                        egui::Button::new(RichText::new("Подключить").size(11.5).color(p.muted))
                            .fill(p.panel)
                            .stroke(Stroke::new(1.0, p.line))
                            .corner_radius(7)
                            .min_size(Vec2::new(0.0, 28.0)),
                    )
                    .clicked()
                {
                    engine.set_active_server(&server.id);
                }
            }
            // Row actions stay available, placed just left of the status.
            ui.add_space(10.0);
            ui.spacing_mut().item_spacing.x = 4.0;
            if small_ghost(ui, p, egui_phosphor::regular::TRASH, "Удалить").clicked() {
                view.confirm = Some(Confirm {
                    title: "Удалить сервер?".into(),
                    message: format!("«{}» будет удалён из списка.", server.display_name()),
                    label: "Удалить".into(),
                    danger: true,
                    action: ConfirmAction::DeleteServer(server.id.clone()),
                });
                view.modal = Some(ModalState::Confirm);
            }
            if small_ghost(ui, p, egui_phosphor::regular::PENCIL_SIMPLE, "Изменить").clicked()
            {
                view.server_draft = Some(server.clone());
                view.server_alpn = server.alpn.join(", ");
                view.modal = Some(ModalState::Server);
            }
            if small_ghost(ui, p, egui_phosphor::regular::ACTIVITY, "Пинг").clicked() {
                engine.ping_server(&server.id);
            }
        },
    );
}

fn badge(ui: &mut Ui, _p: &Palette, text: &str, fg: Color32, bg: Color32, dot: bool) {
    // Fixed 23 px pill (`.badge` in the web concept). Painted manually so the
    // surrounding row layout cannot stretch it vertically.
    let font = FontId::new(10.5, FontFamily::Name("Medium".into()));
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, fg);
    let dot_space = if dot { 10.0 } else { 0.0 };
    let size = Vec2::new(galley.size().x + dot_space + 16.0, 23.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let radius = CornerRadius::same(6);
    ui.painter().rect_filled(rect, radius, bg);
    let mut x = rect.left() + 8.0;
    let center_y = rect.center().y;
    if dot {
        ui.painter()
            .circle_filled(egui::pos2(x + 2.5, center_y), 2.5, fg);
        x += dot_space;
    }
    ui.painter()
        .galley(egui::pos2(x, center_y - galley.size().y * 0.5), galley, fg);
}

/* -------------------------------------------------------------------------- */
/* Subscriptions                                                              */
/* -------------------------------------------------------------------------- */

fn subscriptions_page(
    ui: &mut Ui,
    p: &Palette,
    view: &mut View,
    snapshot: &Snapshot,
    engine: &Engine,
) {
    page_head(
        ui,
        p,
        "Подписки",
        "Ссылки, лимиты и автообновление",
        |ui| {
            if top_button(
                ui,
                p,
                egui_phosphor::regular::PLUS,
                "Добавить подписку",
                BtnKind::Primary,
                true,
            )
            .clicked()
            {
                view.subscription_edit = None;
                view.subscription_name.clear();
                view.subscription_url.clear();
                view.modal = Some(ModalState::Subscription);
            }
        },
    );

    let subs = snapshot.config.subscriptions.clone();
    if subs.is_empty() {
        empty_state(
            ui,
            p,
            egui_phosphor::regular::STACK,
            "Подписок пока нет",
            "Добавьте ссылку — ProtoGrid загрузит серверы и данные о трафике.",
            |ui| {
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("Добавить подписку")
                                .color(Color32::WHITE)
                                .size(11.5),
                        )
                        .fill(p.accent)
                        .corner_radius(8),
                    )
                    .clicked()
                {
                    view.subscription_edit = None;
                    view.subscription_name.clear();
                    view.subscription_url.clear();
                    view.modal = Some(ModalState::Subscription);
                }
            },
        );
        return;
    }

    for pair in subs.chunks(3) {
        card_grid(ui, 3, 14.0, |ui, index| {
            if let Some(sub) = pair.get(index) {
                subscription_card(ui, p, view, snapshot, engine, sub);
            }
        });
        ui.add_space(14.0);
    }
    add_subscription_tile(ui, p, view);
    let _ = engine;
}

fn subscription_card(
    ui: &mut Ui,
    p: &Palette,
    view: &mut View,
    snapshot: &Snapshot,
    engine: &Engine,
    sub: &Subscription,
) {
    let servers = snapshot
        .config
        .servers
        .iter()
        .filter(|server| server.subscription_id.as_deref() == Some(sub.id.as_str()))
        .count();
    let info = sub.userinfo.as_ref();
    let used = info.map(|info| info.used()).unwrap_or(0);
    let total = info.map(|info| info.total).unwrap_or(0);
    let percent = if total > 0 {
        (used.saturating_mul(100) / total).min(100)
    } else {
        0
    };
    let now = now_ms() / 1000;
    let expired = info
        .map(|info| info.expire > 0 && info.expire < now)
        .unwrap_or(false);
    let expiring = info
        .map(|info| info.expire > 0 && info.expire >= now && info.expire - now < 7 * 86400)
        .unwrap_or(false);
    let (status_text, status_fg, status_bg) = if sub.last_error.is_some() {
        ("Ошибка", p.red, p.red_soft)
    } else if expired {
        ("Истекла", p.red, p.red_soft)
    } else if expiring || (total > 0 && percent >= 90) {
        ("Истекает", p.amber, p.amber_soft)
    } else {
        ("Активна", p.green, p.green_soft)
    };
    let traffic = if used > 0 {
        bytes_compact(used)
    } else {
        "—".into()
    };
    let expires = info
        .map(|info| date(info.expire))
        .unwrap_or_else(|| "—".into());

    Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(14)
        .shadow(egui::epaint::Shadow {
            offset: [0, 1],
            blur: 2,
            spread: 0,
            color: Color32::from_black_alpha(60),
        })
        .inner_margin(Margin::same(17))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_min_height(190.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
                ui.painter()
                    .rect_filled(icon_rect, CornerRadius::same(10), p.accent_soft);
                ui.painter().text(
                    icon_rect.center(),
                    Align2::CENTER_CENTER,
                    egui_phosphor::regular::STACK,
                    FontId::new(17.0, FontFamily::Proportional),
                    p.accent_2,
                );
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    ui.add(
                        egui::Label::new(theme::semibold(&sub.name, 13.5))
                            .truncate()
                            .selectable(false),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(&sub.url)
                                .size(10.5)
                                .color(p.faint)
                                .family(FontFamily::Monospace),
                        )
                        .truncate()
                        .selectable(false),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    badge(ui, p, status_text, status_fg, status_bg, true);
                });
            });
            ui.add_space(13.0);
            let boxes = [
                ("Серверы", servers.to_string()),
                ("Трафик", traffic),
                ("До", expires),
            ];
            card_grid(ui, 3, 7.0, |ui, index| {
                let (label, value) = &boxes[index];
                mini_box(ui, p, label, value);
            });
            ui.add_space(13.0);

            let percent_text = if total > 0 {
                format!("{percent}%")
            } else {
                String::new()
            };
            let percent_font = FontId::new(10.5, FontFamily::Monospace);
            let percent_width = if total > 0 {
                measure(ui, &percent_text, percent_font.clone())
            } else {
                0.0
            };
            let left_width = (ui.available_width() - percent_width - 10.0).max(20.0);
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(left_width, 0.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.set_max_width(left_width);
                        let (text, color) = if let Some(error) = &sub.last_error {
                            (
                                format!("{}  {error}", egui_phosphor::regular::WARNING),
                                p.red,
                            )
                        } else if total > 0 {
                            (format!("Лимит {}", bytes_compact(total)), p.faint)
                        } else {
                            ("Провайдер не передаёт статистику".into(), p.faint)
                        };
                        let response = ui.add(
                            egui::Label::new(RichText::new(text).size(10.5).color(color))
                                .truncate()
                                .selectable(false),
                        );
                        if let Some(error) = &sub.last_error {
                            response.on_hover_text(error);
                        }
                    },
                );
                if total > 0 {
                    ui.label(
                        RichText::new(percent_text)
                            .font(percent_font)
                            .color(p.muted),
                    );
                }
            });
            ui.add_space(6.0);
            let fraction = if total > 0 {
                used as f32 / total as f32
            } else {
                0.0
            };
            progress_bar(
                ui,
                p,
                fraction,
                if fraction > 0.9 {
                    p.red
                } else if fraction > 0.75 {
                    p.amber
                } else {
                    p.accent
                },
            );
            ui.add_space(13.0);
            let (line, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(line, 0, p.line);
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Автообновление").size(11.0).color(p.muted));
                let mut auto = sub.auto_update;
                if switch(ui, p, &mut auto) {
                    engine.set_subscription_auto_update(&sub.id, auto);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    if small_ghost(ui, p, egui_phosphor::regular::TRASH, "Удалить").clicked()
                    {
                        view.confirm = Some(Confirm {
                            title: "Удалить подписку?".into(),
                            message: format!("«{}» и её серверы будут удалены.", sub.name),
                            label: "Удалить".into(),
                            danger: true,
                            action: ConfirmAction::DeleteSubscription(sub.id.clone()),
                        });
                        view.modal = Some(ModalState::Confirm);
                    }
                    if small_ghost(ui, p, egui_phosphor::regular::PENCIL_SIMPLE, "Изменить")
                        .clicked()
                    {
                        view.subscription_edit = Some(sub.id.clone());
                        view.subscription_name = sub.name.clone();
                        view.subscription_url = sub.url.clone();
                        view.modal = Some(ModalState::Subscription);
                    }
                    let busy = snapshot.busy(&format!("sub-{}", sub.id));
                    if small_ghost(
                        ui,
                        p,
                        if busy {
                            egui_phosphor::regular::CIRCLE_NOTCH
                        } else {
                            egui_phosphor::regular::ARROWS_CLOCKWISE
                        },
                        "Обновить",
                    )
                    .clicked()
                    {
                        engine.refresh_subscription(sub.id.clone());
                    }
                });
            });
        });
}

fn add_subscription_tile(ui: &mut Ui, p: &Palette, view: &mut View) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 74.0), Sense::click());
    let hovered = response.hovered();
    if hovered {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(14), p.accent_soft);
    }
    dashed_round_rect(
        ui.painter(),
        rect,
        14.0,
        Stroke::new(
            1.0,
            if hovered {
                p.accent_line
            } else {
                p.line_strong
            },
        ),
    );
    let fg = if hovered { p.accent_2 } else { p.faint };
    ui.painter().text(
        egui::pos2(rect.left() + 30.0, rect.center().y),
        Align2::CENTER_CENTER,
        egui_phosphor::regular::PLUS,
        FontId::new(20.0, FontFamily::Proportional),
        fg,
    );
    let label_font = FontId::new(13.0, FontFamily::Name("Medium".into()));
    let hint_font = FontId::new(11.5, FontFamily::Proportional);
    let label = ui
        .painter()
        .layout_no_wrap("Добавить подписку".to_owned(), label_font, fg);
    let hint =
        ui.painter()
            .layout_no_wrap("URL или vless:// ссылка".to_owned(), hint_font, p.faint);
    let x = rect.left() + 50.0;
    ui.painter().galley(
        egui::pos2(x, rect.center().y - label.size().y * 0.5),
        label.clone(),
        fg,
    );
    ui.painter().galley(
        egui::pos2(
            x + label.size().x + 12.0,
            rect.center().y - hint.size().y * 0.5,
        ),
        hint.clone(),
        p.faint,
    );
    if response.clicked() {
        view.subscription_edit = None;
        view.subscription_name.clear();
        view.subscription_url.clear();
        view.modal = Some(ModalState::Subscription);
    }
}

fn mini_box(ui: &mut Ui, p: &Palette, label: &str, value: &str) {
    Frame::new()
        .fill(p.panel_2)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(8)
        .inner_margin(Margin::symmetric(9, 7))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(
                RichText::new(label.to_uppercase())
                    .size(9.0)
                    .color(p.faint)
                    .family(FontFamily::Name("Medium".into())),
            );
            ui.add(
                egui::Label::new(theme::semibold(value, 12.0))
                    .truncate()
                    .selectable(false),
            );
        });
}

/* -------------------------------------------------------------------------- */
/* Routing                                                                    */
/* -------------------------------------------------------------------------- */

fn routing_page(ui: &mut Ui, p: &Palette, view: &mut View, snapshot: &Snapshot, engine: &Engine) {
    page_head(
        ui,
        p,
        "Маршрутизация",
        "Что идёт через прокси, а что напрямую",
        |ui| {
            if top_button(
                ui,
                p,
                egui_phosphor::regular::PLUS,
                "Добавить правило",
                BtnKind::Primary,
                true,
            )
            .clicked()
            {
                view.rule_draft = Some(RoutingRule {
                    id: String::new(),
                    name: String::new(),
                    enabled: true,
                    action: RuleAction::Proxy,
                    kind: RuleKind::DomainSuffix,
                    value: String::new(),
                });
                view.modal = Some(ModalState::Rule);
            }
        },
    );

    card_grid(ui, 3, 12.0, |ui, index| {
        let presets = [
            (
                "bypass",
                "Обход РФ",
                "Российские сайты напрямую, остальное — через прокси",
            ),
            (
                "global",
                "Глобальный прокси",
                "Весь трафик идёт через выбранный сервер",
            ),
            ("direct", "Прямой", "Прокси отключён, правила игнорируются"),
        ];
        let (id, title, text) = presets[index];
        preset_card(ui, p, id, title, text, snapshot, engine);
    });
    ui.add_space(14.0);

    Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(14)
        .inner_margin(Margin::ZERO)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            Frame::new()
                .inner_margin(Margin::symmetric(18, 16))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(theme::semibold("Правила", 13.5));
                            ui.label(
                                RichText::new(
                                    "Применяются сверху вниз, первое совпадение выигрывает",
                                )
                                .size(11.0)
                                .color(p.faint),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            badge(
                                ui,
                                p,
                                &format!(
                                    "{} {}",
                                    snapshot.config.rules.len(),
                                    plural(
                                        snapshot.config.rules.len(),
                                        "правило",
                                        "правила",
                                        "правил"
                                    )
                                ),
                                p.accent_2,
                                p.accent_soft,
                                false,
                            );
                        });
                    });
                });
            let (line, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(line, 0, p.line);
            rules_header(ui, p);
            let rules = snapshot.config.rules.clone();
            for (index, rule) in rules.iter().enumerate() {
                rule_row(ui, p, view, engine, rule, index, rules.len());
            }
            if rules.is_empty() {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Правил пока нет").size(12.5).color(p.faint));
                });
                ui.add_space(30.0);
            }
            let (line, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(line, 0, p.line);
            Frame::new()
                .inner_margin(Margin::symmetric(18, 14))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Если ни одно правило не совпало")
                                .size(12.0)
                                .color(p.muted),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            egui::ComboBox::from_id_salt("final_action")
                                .selected_text(rule_action_label(
                                    snapshot.config.settings.final_action,
                                ))
                                .width(170.0)
                                .show_ui(ui, |ui| {
                                    for action in
                                        [RuleAction::Proxy, RuleAction::Direct, RuleAction::Block]
                                    {
                                        if ui
                                            .selectable_label(
                                                snapshot.config.settings.final_action == action,
                                                rule_action_label(action),
                                            )
                                            .clicked()
                                        {
                                            engine.set_final_action(action);
                                        }
                                    }
                                });
                        });
                    });
                });
        });
}

/// Vertical fade from `color` at the top to transparent at `stop` of the height,
/// clipped to a rounded rectangle. Mirrors the CSS gradient of the active preset
/// card in the web concept (`linear-gradient(180deg, accent-soft, transparent 80%)`).
fn paint_vertical_fade(
    painter: &egui::Painter,
    rect: Rect,
    color: Color32,
    stop: f32,
    radius: f32,
) {
    let height = rect.height();
    if height <= 0.0 || color.a() == 0 {
        return;
    }
    let radius = radius.min(rect.width() * 0.5).min(height * 0.5);
    let stop = stop.clamp(0.01, 1.0);
    // `Color32` stores premultiplied channels; take the straight sRGB values so
    // each strip recomputes the same tint with a lower alpha.
    let [red, green, blue, alpha] = color.to_srgba_unmultiplied();
    let alpha = alpha as f32;
    for row in 0..height.ceil() as usize {
        let y0 = rect.top() + row as f32;
        let y1 = (y0 + 1.0).min(rect.bottom());
        let mid = (y0 + y1) * 0.5;
        let t = ((mid - rect.top()) / height).min(stop) / stop;
        let a = (alpha * (1.0 - t)).round() as u8;
        if a == 0 {
            continue;
        }
        let inset = if mid < rect.top() + radius {
            let dy = rect.top() + radius - mid;
            radius - (radius * radius - dy * dy).max(0.0).sqrt()
        } else if mid > rect.bottom() - radius {
            let dy = mid - (rect.bottom() - radius);
            radius - (radius * radius - dy * dy).max(0.0).sqrt()
        } else {
            0.0
        };
        if rect.width() <= inset * 2.0 {
            continue;
        }
        painter.rect_filled(
            Rect::from_min_max(
                egui::pos2(rect.left() + inset, y0),
                egui::pos2(rect.right() - inset, y1),
            ),
            0,
            Color32::from_rgba_unmultiplied(red, green, blue, a),
        );
    }
}

fn preset_card(
    ui: &mut Ui,
    p: &Palette,
    id: &str,
    title: &str,
    text: &str,
    snapshot: &Snapshot,
    engine: &Engine,
) {
    let active = match id {
        "direct" => snapshot.config.settings.final_action == RuleAction::Direct,
        "global" => {
            snapshot.config.settings.final_action == RuleAction::Proxy
                && snapshot.config.rules.len() <= 1
        }
        _ => {
            snapshot.config.settings.final_action == RuleAction::Proxy
                && snapshot.config.rules.len() > 1
        }
    };
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 82.0), Sense::click());
    let radius = CornerRadius::same(12);
    if active {
        paint_vertical_fade(ui.painter(), rect, p.accent_soft, 0.8, 12.0);
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, p.accent),
            egui::StrokeKind::Inside,
        );
    } else {
        ui.painter().rect(
            rect,
            radius,
            p.panel,
            Stroke::new(1.0, p.line),
            egui::StrokeKind::Inside,
        );
    }
    let inner = rect.shrink(15.0);
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Min)),
    );
    child.spacing_mut().item_spacing.x = 12.0;
    let (radio, _) = child.allocate_exact_size(Vec2::splat(19.0), Sense::hover());
    if active {
        child.painter().circle_filled(radio.center(), 9.5, p.accent);
        child.painter().text(
            radio.center(),
            Align2::CENTER_CENTER,
            egui_phosphor::regular::CHECK,
            FontId::new(11.0, FontFamily::Proportional),
            Color32::WHITE,
        );
    } else {
        child
            .painter()
            .circle_stroke(radio.center(), 9.0, Stroke::new(1.5, p.line_strong));
    }
    let remaining = child.available_width();
    child.allocate_ui_with_layout(
        Vec2::new(remaining, 0.0),
        Layout::top_down(Align::Min),
        |ui| {
            ui.set_max_width(remaining);
            ui.label(theme::semibold(title, 13.0));
            ui.label(RichText::new(text).size(11.0).color(p.faint));
        },
    );
    if response.clicked() {
        engine.apply_routing_preset(id);
    }
}

fn rules_header(ui: &mut Ui, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::hover());
    ui.painter().rect_filled(rect, 0, p.panel_2);
    let inner = Rect::from_min_max(
        egui::pos2(rect.left() + 16.0, rect.top()),
        egui::pos2(rect.right() - 16.0, rect.bottom()),
    );
    let inner_width = inner.width();
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
    );
    child.spacing_mut().item_spacing.x = 0.0;
    let font = FontId::new(10.0, FontFamily::Name("Medium".into()));
    for (width, text) in [
        (110.0, "ТИП"),
        ((inner_width - 426.0).max(160.0), "ЗНАЧЕНИЕ"),
        (120.0, "ДЕЙСТВИЕ"),
        (80.0, "ВКЛЮЧЕНО"),
        (116.0, ""),
    ] {
        child.allocate_ui_with_layout(
            Vec2::new(width, 34.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_width(width);
                ui.label(
                    RichText::new(text)
                        .size(10.0)
                        .color(p.faint)
                        .font(font.clone()),
                );
            },
        );
    }
    let (line, _) = ui.allocate_exact_size(Vec2::new(rect.width(), 1.0), Sense::hover());
    ui.painter().rect_filled(line, 0, p.line);
}

fn rule_row(
    ui: &mut Ui,
    p: &Palette,
    view: &mut View,
    engine: &Engine,
    rule: &RoutingRule,
    index: usize,
    total: usize,
) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 50.0), Sense::click());
    if ui.rect_contains_pointer(rect) {
        ui.painter().rect_filled(rect, 0, p.panel_2);
    }
    if index + 1 < total {
        let (line, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
        ui.painter().rect_filled(line, 0, p.line);
    }
    let inner = Rect::from_min_max(
        egui::pos2(rect.left() + 16.0, rect.top()),
        egui::pos2(rect.right() - 16.0, rect.bottom()),
    );
    let inner_width = inner.width();
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
    );
    child.spacing_mut().item_spacing.x = 0.0;
    child.allocate_ui_with_layout(
        Vec2::new(110.0, 50.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(110.0);
            ui.label(
                RichText::new(rule_kind_label(rule.kind).to_uppercase())
                    .size(9.5)
                    .color(p.faint)
                    .family(FontFamily::Monospace),
            );
        },
    );
    child.allocate_ui_with_layout(
        Vec2::new((inner_width - 426.0).max(160.0), 50.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(ui.available_width());
            ui.add(
                egui::Label::new(
                    RichText::new(&rule.value)
                        .size(11.5)
                        .color(p.muted)
                        .family(FontFamily::Monospace),
                )
                .truncate()
                .selectable(false),
            );
        },
    );
    child.allocate_ui_with_layout(
        Vec2::new(120.0, 50.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(120.0);
            action_badge(ui, p, rule.action);
        },
    );
    child.allocate_ui_with_layout(
        Vec2::new(80.0, 50.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(80.0);
            let mut enabled = rule.enabled;
            if switch_small(ui, p, &mut enabled) {
                let mut copy = rule.clone();
                copy.enabled = enabled;
                engine.save_rule(copy);
            }
        },
    );
    child.allocate_ui_with_layout(
        Vec2::new(116.0, 50.0),
        Layout::right_to_left(Align::Center),
        |ui| {
            ui.set_min_width(116.0);
            ui.spacing_mut().item_spacing.x = 1.0;
            if small_ghost(ui, p, egui_phosphor::regular::TRASH, "Удалить").clicked() {
                view.confirm = Some(Confirm {
                    title: "Удалить правило?".into(),
                    message: format!("Правило «{}» перестанет применяться.", rule.name),
                    label: "Удалить".into(),
                    danger: true,
                    action: ConfirmAction::DeleteRule(rule.id.clone()),
                });
                view.modal = Some(ModalState::Confirm);
            }
            if small_ghost(ui, p, egui_phosphor::regular::PENCIL_SIMPLE, "Изменить").clicked()
            {
                view.rule_draft = Some(rule.clone());
                view.modal = Some(ModalState::Rule);
            }
            ui.add_enabled_ui(index + 1 < total, |ui| {
                if small_ghost(ui, p, egui_phosphor::regular::ARROW_DOWN, "Ниже").clicked() {
                    engine.move_rule(&rule.id, "down");
                }
            });
            ui.add_enabled_ui(index > 0, |ui| {
                if small_ghost(ui, p, egui_phosphor::regular::ARROW_UP, "Выше").clicked() {
                    engine.move_rule(&rule.id, "up");
                }
            });
        },
    );
}

/* -------------------------------------------------------------------------- */
/* Charts                                                                     */
/* -------------------------------------------------------------------------- */

fn charts_page(ui: &mut Ui, p: &Palette, view: &mut View, snapshot: &Snapshot) {
    page_head(
        ui,
        p,
        "Графики",
        "Трафик по маршрутам и направлениям",
        |ui| {
            let selected = match view.chart_dir {
                Direction::Down => 1,
                Direction::Up => 2,
            };
            if let Some(value) = seg(ui, p, &[(1, "↓ Входящий"), (2, "↑ Исходящий")], selected)
            {
                view.chart_dir = if value == 1 {
                    Direction::Down
                } else {
                    Direction::Up
                };
            }
        },
    );

    let points = view.last_points(view.chart_range);
    let proxy_total = view.totals.proxy_down + view.totals.proxy_up;
    let direct_total = view.totals.direct_down + view.totals.direct_up;
    let all_total = proxy_total + direct_total;
    let share = if all_total > 0.0 {
        (proxy_total / all_total * 100.0).round() as u32
    } else {
        0
    };
    let peak = view
        .series
        .iter()
        .map(|point| match view.chart_dir {
            Direction::Down => point.down,
            Direction::Up => point.up,
        })
        .fold(0.0_f32, f32::max);

    card_grid(ui, 4, 12.0, |ui, index| match index {
        0 => mini_stat(
            ui,
            p,
            "Через прокси",
            &bytes_f(proxy_total),
            "за сеанс",
            None,
        ),
        1 => mini_stat(ui, p, "Напрямую", &bytes_f(direct_total), "за сеанс", None),
        2 => mini_stat(
            ui,
            p,
            "Доля прокси",
            &format!("{share}%"),
            "",
            Some((share, p.accent)),
        ),
        _ => mini_stat(
            ui,
            p,
            "Пиковая скорость",
            &rate_plain(peak),
            &format!("за {}", range_label(view.chart_range)),
            None,
        ),
    });
    ui.add_space(14.0);

    Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(14)
        .inner_margin(Margin::symmetric(18, 16))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(theme::semibold("Скорость по маршрутам", 13.5));
                    ui.label(
                        RichText::new(format!(
                            "{} трафик · {}",
                            match view.chart_dir {
                                Direction::Down => "входящий",
                                Direction::Up => "исходящий",
                            },
                            range_label(view.chart_range)
                        ))
                        .size(11.0)
                        .color(p.faint),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(range) = seg(
                        ui,
                        p,
                        &[(60, "60 с"), (300, "5 мин"), (900, "15 мин")],
                        view.chart_range,
                    ) {
                        view.chart_range = range;
                    }
                });
            });
            ui.add_space(6.0);
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 360.0), Sense::hover());
            chart_grid(ui, p, rect);
            let (proxy_values, direct_values): (Vec<f32>, Vec<f32>) = points
                .iter()
                .map(|point| match view.chart_dir {
                    Direction::Down => (point.proxy_down, point.direct_down),
                    Direction::Up => (point.proxy_up, point.direct_up),
                })
                .unzip();
            let stacked: Vec<f32> = proxy_values
                .iter()
                .zip(direct_values.iter())
                .map(|(proxy, direct)| proxy + direct)
                .collect();
            let max = stacked.iter().copied().fold(0.0_f32, f32::max) * 1.15;
            paint_area(ui, rect, &stacked, max, p.amber, 38, 1.7);
            paint_area(ui, rect, &proxy_values, max, p.accent, 50, 2.0);
            ui.add_space(12.0);
            let (line, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(line, 0, p.line);
            ui.add_space(10.0);
            let proxy = view.totals.proxy_down + view.totals.proxy_up;
            let direct = view.totals.direct_down + view.totals.direct_up;
            card_grid(ui, 4, 14.0, |ui, index| match index {
                0 => chart_foot(
                    ui,
                    p,
                    "Прокси ↓ / ↑",
                    &format!(
                        "{} / {}",
                        bytes_f(view.totals.proxy_down),
                        bytes_f(view.totals.proxy_up)
                    ),
                ),
                1 => chart_foot(
                    ui,
                    p,
                    "Напрямую ↓ / ↑",
                    &format!(
                        "{} / {}",
                        bytes_f(view.totals.direct_down),
                        bytes_f(view.totals.direct_up)
                    ),
                ),
                2 => chart_foot(ui, p, "Всего за сеанс", &bytes_f(proxy + direct)),
                _ => chart_foot(ui, p, "Доля прокси", &format!("{share}%")),
            });
            let _ = snapshot;
        });
}

fn mini_stat(
    ui: &mut Ui,
    p: &Palette,
    label: &str,
    value: &str,
    hint: &str,
    progress: Option<(u32, Color32)>,
) {
    Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(12)
        .inner_margin(Margin::symmetric(15, 13))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                RichText::new(label.to_uppercase())
                    .size(9.5)
                    .color(p.faint)
                    .family(FontFamily::Name("Medium".into())),
            );
            ui.add(
                egui::Label::new(theme::semibold(value, 21.0))
                    .truncate()
                    .selectable(false),
            );
            if hint.is_empty() {
                ui.add_space(3.0);
            } else {
                ui.label(RichText::new(hint).size(10.0).color(p.faint));
            }
            if let Some((share, color)) = progress {
                ui.add_space(6.0);
                progress_bar(ui, p, share as f32 / 100.0, color);
            }
        });
}

fn chart_foot(ui: &mut Ui, p: &Palette, label: &str, value: &str) {
    ui.label(
        RichText::new(label.to_uppercase())
            .size(9.5)
            .color(p.faint)
            .family(FontFamily::Name("Medium".into())),
    );
    ui.label(
        RichText::new(value)
            .size(13.0)
            .color(p.text)
            .family(FontFamily::Monospace),
    );
}

fn range_label(range: u64) -> &'static str {
    match range {
        300 => "5 мин",
        900 => "15 мин",
        _ => "60 с",
    }
}

/* -------------------------------------------------------------------------- */
/* Logs                                                                       */
/* -------------------------------------------------------------------------- */

fn logs_page(ui: &mut Ui, p: &Palette, view: &mut View, snapshot: &Snapshot, engine: &Engine) {
    let mut logs = view
        .frozen_logs
        .clone()
        .unwrap_or_else(|| snapshot.logs.clone());
    page_head(
        ui,
        p,
        "Журнал",
        &format!(
            "{} · {} · вывод в реальном времени",
            snapshot.config.settings.core.name(),
            snapshot
                .core(snapshot.config.settings.core)
                .and_then(|core| core.version.as_deref())
                .unwrap_or("—")
        ),
        |ui| {
            if top_button(
                ui,
                p,
                egui_phosphor::regular::TRASH,
                "Очистить",
                BtnKind::Ghost,
                true,
            )
            .clicked()
            {
                engine.clear_logs();
                view.frozen_logs = None;
            }
            if top_button(
                ui,
                p,
                egui_phosphor::regular::FLOPPY_DISK,
                "Сохранить",
                BtnKind::Ghost,
                true,
            )
            .clicked()
            {
                match engine.save_logs() {
                    Ok(path) => view.toast(format!("Логи сохранены: {path}"), ToastKind::Success),
                    Err(error) => view.toast(error, ToastKind::Error),
                }
            }
            if top_button(
                ui,
                p,
                egui_phosphor::regular::COPY,
                "Копировать",
                BtnKind::Ghost,
                true,
            )
            .clicked()
            {
                let text = logs
                    .iter()
                    .map(|entry| {
                        format!(
                            "{} [{}] {}: {}",
                            date_ms(entry.ts),
                            entry.level,
                            entry.source,
                            entry.line
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                match arboard::Clipboard::new() {
                    Ok(mut clipboard) => {
                        if clipboard.set_text(text).is_ok() {
                            view.toast("Логи скопированы", ToastKind::Success);
                        } else {
                            view.toast("Не удалось скопировать логи", ToastKind::Error);
                        }
                    }
                    Err(_) => view.toast("Буфер обмена недоступен", ToastKind::Error),
                }
            }
            if top_button(
                ui,
                p,
                if view.logs_paused {
                    egui_phosphor::regular::PLAY
                } else {
                    egui_phosphor::regular::PAUSE
                },
                if view.logs_paused {
                    "Продолжить"
                } else {
                    "Пауза"
                },
                BtnKind::Ghost,
                true,
            )
            .clicked()
            {
                view.logs_paused = !view.logs_paused;
                if view.logs_paused {
                    view.frozen_logs = Some(snapshot.logs.clone());
                } else {
                    view.frozen_logs = None;
                }
            }
        },
    );

    let query = view.logs_filter.to_lowercase();
    let filtered: Vec<&LogEntry> = logs
        .iter()
        .filter(|entry| {
            let level_ok = match view.log_level {
                LogFilter::All => true,
                LogFilter::Debug => entry.level == "debug" || entry.level == "trace",
                LogFilter::Info => entry.level == "info",
                LogFilter::Warn => entry.level == "warn" || entry.level == "warning",
                LogFilter::Error => entry.level == "error",
            };
            if !level_ok {
                return false;
            }
            query.is_empty()
                || entry.line.to_lowercase().contains(&query)
                || entry.level.to_lowercase().contains(&query)
                || entry.source.to_lowercase().contains(&query)
        })
        .collect();

    Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(14)
        .inner_margin(Margin::ZERO)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            Frame::new()
                .inner_margin(Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        search_box(ui, p, &mut view.logs_filter, "Фильтр по тексту…", 260.0);
                        ui.add_space(2.0);
                        ui.spacing_mut().item_spacing.x = 3.0;
                        for (filter, label) in [
                            (LogFilter::All, "Все"),
                            (LogFilter::Debug, "Отладка"),
                            (LogFilter::Info, "Инфо"),
                            (LogFilter::Warn, "Внимание"),
                            (LogFilter::Error, "Ошибки"),
                        ] {
                            if log_chip(ui, p, label, view.log_level == filter).clicked() {
                                view.log_level = filter;
                            }
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let mut auto_scroll = view.auto_scroll;
                            if switch_small(ui, p, &mut auto_scroll) {
                                view.auto_scroll = auto_scroll;
                            }
                            ui.label(RichText::new("Автопрокрутка").size(11.0).color(p.muted));
                        });
                    });
                });
            let (line, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(line, 0, p.line);
            Frame::new()
                .fill(p.code_bg)
                .inner_margin(Margin::symmetric(0, 6))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ScrollArea::vertical()
                        .id_salt("log-scroll")
                        .auto_shrink([false, false])
                        .stick_to_bottom(view.auto_scroll)
                        .max_height((ui.available_height() - 70.0).max(300.0))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            if filtered.is_empty() {
                                ui.add_space(28.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        RichText::new("Нет записей по текущему фильтру")
                                            .size(12.5)
                                            .color(p.faint),
                                    );
                                });
                                ui.add_space(28.0);
                            }
                            for entry in &filtered {
                                log_line(ui, p, entry);
                            }
                        });
                });
            let (line, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(line, 0, p.line);
            Frame::new()
                .inner_margin(Margin::symmetric(16, 8))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} {}",
                                filtered.len(),
                                plural(filtered.len(), "запись", "записи", "записей")
                            ))
                            .size(10.0)
                            .color(p.faint)
                            .family(FontFamily::Monospace),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!(
                                    "фильтр: {}",
                                    match view.log_level {
                                        LogFilter::All => "все",
                                        LogFilter::Debug => "отладка",
                                        LogFilter::Info => "инфо",
                                        LogFilter::Warn => "внимание",
                                        LogFilter::Error => "ошибки",
                                    }
                                ))
                                .size(10.0)
                                .color(p.faint)
                                .family(FontFamily::Monospace),
                            );
                        });
                    });
                });
        });
    logs.clear();
}

fn log_chip(ui: &mut Ui, p: &Palette, text: &str, selected: bool) -> egui::Response {
    let width = measure(ui, text, FontId::proportional(11.0)) + 20.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 28.0), Sense::click());
    if selected {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(7), p.accent_soft);
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(7), p.panel_2);
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        medium_font(11.0),
        if selected { p.accent_2 } else { p.faint },
    );
    response
}

fn log_line(ui: &mut Ui, p: &Palette, entry: &LogEntry) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 0, Color32::from_rgba_unmultiplied(127, 127, 127, 12));
    }
    let inner = Rect::from_min_max(
        egui::pos2(rect.left() + 16.0, rect.top()),
        egui::pos2(rect.right() - 16.0, rect.bottom()),
    );
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
    );
    child.spacing_mut().item_spacing.x = 12.0;
    child.label(
        RichText::new(hhmmss(entry.ts))
            .size(10.5)
            .color(p.faint)
            .family(FontFamily::Monospace),
    );
    child.allocate_ui_with_layout(
        Vec2::new(42.0, 20.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(42.0);
            ui.label(
                RichText::new(entry.level.to_uppercase())
                    .size(9.0)
                    .color(level_color(p, &entry.level))
                    .family(FontFamily::Name("Medium".into())),
            );
        },
    );
    child.allocate_ui_with_layout(
        Vec2::new(52.0, 20.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(52.0);
            ui.label(
                RichText::new(&entry.source)
                    .size(10.5)
                    .color(p.faint)
                    .family(FontFamily::Monospace),
            );
        },
    );
    child.add(
        egui::Label::new(
            RichText::new(&entry.line)
                .size(11.5)
                .color(if entry.level == "error" {
                    p.red
                } else if entry.level == "warn" || entry.level == "warning" {
                    p.amber
                } else {
                    p.muted
                })
                .family(FontFamily::Monospace),
        )
        .truncate()
        .selectable(false),
    );
}

/* -------------------------------------------------------------------------- */
/* Settings                                                                   */
/* -------------------------------------------------------------------------- */

fn settings_page(ui: &mut Ui, p: &Palette, view: &mut View, snapshot: &Snapshot, engine: &Engine) {
    if view.settings_draft.is_none() {
        view.settings_draft = Some(snapshot.config.settings.clone());
        view.settings_dirty = false;
    }
    let Some(mut draft) = view.settings_draft.clone() else {
        return;
    };
    let saved = snapshot.config.settings.clone();
    let dirty = draft != saved;
    let mut save = false;
    let mut reset = false;

    page_head(
        ui,
        p,
        "Настройки",
        "Соединение, DNS, ядро и интерфейс",
        |ui| {
            if top_button(
                ui,
                p,
                egui_phosphor::regular::FLOPPY_DISK,
                "Сохранить",
                BtnKind::Primary,
                dirty,
            )
            .clicked()
            {
                save = true;
            }
            if top_button(
                ui,
                p,
                egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE,
                "Отменить",
                BtnKind::Ghost,
                dirty,
            )
            .clicked()
            {
                reset = true;
            }
        },
    );
    if reset {
        view.settings_draft = Some(saved);
        view.settings_dirty = false;
        return;
    }
    if save {
        engine.save_settings(draft.clone());
        view.settings_draft = Some(draft);
        view.settings_dirty = false;
        return;
    }

    if dirty {
        Frame::new()
            .fill(p.amber_soft)
            .corner_radius(8)
            .inner_margin(Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(
                    RichText::new(format!(
                        "{}  Есть несохранённые изменения",
                        egui_phosphor::regular::INFO
                    ))
                    .size(11.0)
                    .color(p.amber),
                );
            });
        ui.add_space(12.0);
    }

    ui.spacing_mut().item_spacing.x = 14.0;
    card_grid(ui, 2, 14.0, |ui, index| {
        if index == 0 {
            settings_connection(ui, p, view, snapshot, engine, &mut draft);
        } else {
            settings_interface(ui, p, view, snapshot, engine, &mut draft);
        }
    });
    ui.add_space(14.0);
    card_grid(ui, 2, 14.0, |ui, index| {
        if index == 0 {
            settings_dns(ui, p, view, snapshot, &mut draft);
        } else {
            settings_core(ui, p, view, snapshot, engine, &mut draft);
        }
    });
    ui.add_space(14.0);
    settings_about(ui, p, snapshot, engine);

    view.settings_dirty = draft != saved;
    view.settings_draft = Some(draft);
}

fn settings_card(
    ui: &mut Ui,
    p: &Palette,
    title: &str,
    subtitle: &str,
    body: impl FnOnce(&mut Ui),
) {
    Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(14)
        .inner_margin(Margin::ZERO)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            Frame::new()
                .inner_margin(Margin::symmetric(18, 16))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(theme::semibold(title, 13.5));
                    ui.label(RichText::new(subtitle).size(11.0).color(p.faint));
                });
            let (line, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(line, 0, p.line);
            body(ui);
        });
}

fn set_row(ui: &mut Ui, p: &Palette, title: &str, text: &str, control: impl FnOnce(&mut Ui)) {
    set_row_impl(ui, p, title, text, false, true, control);
}

fn set_row_last(ui: &mut Ui, p: &Palette, title: &str, text: &str, control: impl FnOnce(&mut Ui)) {
    set_row_impl(ui, p, title, text, true, true, control);
}

/// A row whose control is greyed out when the selected core does not support it.
fn set_row_if(
    ui: &mut Ui,
    p: &Palette,
    title: &str,
    text: &str,
    enabled: bool,
    control: impl FnOnce(&mut Ui),
) {
    set_row_impl(ui, p, title, text, false, enabled, control);
}

fn set_row_last_if(
    ui: &mut Ui,
    p: &Palette,
    title: &str,
    text: &str,
    enabled: bool,
    control: impl FnOnce(&mut Ui),
) {
    set_row_impl(ui, p, title, text, true, enabled, control);
}

fn set_row_impl(
    ui: &mut Ui,
    p: &Palette,
    title: &str,
    text: &str,
    last: bool,
    enabled: bool,
    control: impl FnOnce(&mut Ui),
) {
    Frame::new()
        .inner_margin(Margin::symmetric(18, 10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_max_width((ui.available_width() - 190.0).max(120.0));
                    ui.label(theme::medium(title, 12.5));
                    let text = if enabled {
                        text.to_string()
                    } else {
                        format!("{text} · недоступно для выбранного ядра")
                    };
                    ui.label(RichText::new(text).size(10.5).color(p.faint));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_enabled_ui(enabled, |ui| control(ui));
                });
            });
        });
    if !last {
        let (line, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
        ui.painter().rect_filled(line, 0, p.line);
    }
}

fn plain_input(ui: &mut Ui, value: &mut String, width: f32) {
    ui.add_sized(
        [width, 34.0],
        TextEdit::singleline(value).desired_width(width),
    );
}

fn settings_connection(
    ui: &mut Ui,
    p: &Palette,
    view: &mut View,
    snapshot: &Snapshot,
    engine: &Engine,
    draft: &mut Settings,
) {
    let core = draft.core;
    let features = snapshot
        .core(core)
        .map(|info| info.features)
        .unwrap_or_default();
    if !features.tun {
        draft.tun_enabled = false;
    }
    settings_card(
        ui,
        p,
        "Соединение",
        "Локальные прокси и перехват трафика",
        |ui| {
            set_row(
                ui,
                p,
                "Системный прокси",
                "Прописывать прокси в настройках системы",
                |ui| {
                    switch(ui, p, &mut draft.system_proxy);
                },
            );
            if !snapshot.proxy_supported {
                Frame::new()
                    .inner_margin(Margin::symmetric(18, 0))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(
                            RichText::new(
                                "Системный прокси недоступен в этом окружении — настройте прокси вручную.",
                            )
                            .size(10.0)
                            .color(p.amber),
                        );
                        ui.add_space(8.0);
                    });
            }
            set_row_if(
                ui,
                p,
                "TUN-режим",
                "Перехватывать весь трафик; нужны права администратора",
                features.tun,
                |ui| {
                    switch(ui, p, &mut draft.tun_enabled);
                },
            );
            set_row(
                ui,
                p,
                "Разрешить LAN",
                "Доступ к прокси с других устройств сети",
                |ui| {
                    switch(ui, p, &mut draft.allow_lan);
                },
            );
            set_row(
                ui,
                p,
                "Автозапуск",
                "Запускать ProtoGrid при входе в систему",
                |ui| {
                    let mut autostart = snapshot.autostart;
                    if switch(ui, p, &mut autostart) {
                        engine.set_autostart(autostart);
                    }
                },
            );
            set_row_if(
                ui,
                p,
                "Порт mixed",
                "sing-box: HTTP и SOCKS одновременно",
                core.is_singbox_like(),
                |ui| {
                    port_input(ui, &mut draft.mixed_port);
                },
            );
            set_row_if(
                ui,
                p,
                "Порт SOCKS",
                "Xray-core",
                !core.is_singbox_like(),
                |ui| {
                    port_input(ui, &mut draft.socks_port);
                },
            );
            set_row_if(
                ui,
                p,
                "Порт HTTP",
                "Xray-core и системный прокси",
                !core.is_singbox_like(),
                |ui| {
                    port_input(ui, &mut draft.http_port);
                },
            );
            set_row_last_if(
                ui,
                p,
                "Clash API",
                "Проверка задержки и телеметрия sing-box",
                features.clash_api,
                |ui| {
                    port_input(ui, &mut draft.clash_api_port);
                },
            );
            let _ = view;
        },
    );
}

fn port_input(ui: &mut Ui, value: &mut u16) {
    ui.add_sized(
        [92.0, 34.0],
        egui::DragValue::new(value).range(1..=65535).speed(1.0),
    );
}

fn settings_interface(
    ui: &mut Ui,
    p: &Palette,
    view: &mut View,
    snapshot: &Snapshot,
    engine: &Engine,
    draft: &mut Settings,
) {
    settings_card(
        ui,
        p,
        "Интерфейс",
        "Тема, запуск и фон",
        |ui| {
            set_row(
                ui,
                p,
                "Тема",
                "Оформление панели",
                |ui| {
                    if theme_choice(ui, p, "Светлая", !view.dark, egui_phosphor::regular::SUN)
                    {
                        view.dark = false;
                        theme::install(ui.ctx(), false);
                    }
                    if theme_choice(ui, p, "Тёмная", view.dark, egui_phosphor::regular::MOON)
                    {
                        view.dark = true;
                        theme::install(ui.ctx(), true);
                    }
                },
            );
            set_row(
                ui,
                p,
                "Автоподключение",
                "Подключаться при старте приложения",
                |ui| {
                    switch(ui, p, &mut draft.auto_connect);
                },
            );
            set_row(
                ui,
                p,
                "Сворачивать в трей",
                "Кнопка закрытия скрывает окно",
                |ui| {
                    ui.add_enabled_ui(view.tray_available, |ui| {
                        switch(ui, p, &mut draft.close_to_tray);
                    });
                },
            );
            set_row(
                ui,
                p,
                "Обновлять подписки",
                "Автоматически раз в 12 часов",
                |ui| {
                    switch(ui, p, &mut draft.auto_update_subscriptions);
                },
            );
            set_row_last(
                ui,
                p,
                "IPv6",
                "Разрешить IPv6 в TUN и DNS",
                |ui| {
                    switch(ui, p, &mut draft.ipv6);
                },
            );
            let _ = (snapshot, engine);
        },
    );
}

fn theme_choice(ui: &mut Ui, p: &Palette, label: &str, selected: bool, icon: &str) -> bool {
    let width = measure(ui, label, FontId::proportional(11.5)) + 48.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 32.0), Sense::click());
    ui.painter().rect(
        rect,
        CornerRadius::same(8),
        if selected { p.accent_soft } else { p.panel_2 },
        Stroke::new(1.0, if selected { p.accent } else { p.line }),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        icon,
        FontId::new(14.0, FontFamily::Proportional),
        if selected { p.accent_2 } else { p.muted },
    );
    ui.painter().text(
        egui::pos2(rect.left() + 32.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        medium_font(11.5),
        if selected { p.accent_2 } else { p.muted },
    );
    response.clicked()
}

fn settings_dns(
    ui: &mut Ui,
    p: &Palette,
    view: &mut View,
    snapshot: &Snapshot,
    draft: &mut Settings,
) {
    let features = snapshot
        .core(draft.core)
        .map(|info| info.features)
        .unwrap_or_default();
    if !features.fakedns {
        draft.fake_dns = false;
    }
    settings_card(
        ui,
        p,
        "DNS и маршрутизация",
        "Как резолвить домены внутри туннеля",
        |ui| {
            set_row(
                ui,
                p,
                "Удалённый DNS",
                "DoH, DoT или адрес сервера",
                |ui| {
                    plain_input(ui, &mut draft.dns_remote, 240.0);
                },
            );
            set_row_if(
                ui,
                p,
                "FakeDNS",
                "sing-box: отдавать приложениям фейковые IP",
                features.fakedns,
                |ui| {
                    switch(ui, p, &mut draft.fake_dns);
                },
            );
            set_row(
                ui,
                p,
                "Автовыбор сервера",
                "urltest: выбирать узел с минимальной задержкой",
                |ui| {
                    switch(ui, p, &mut draft.auto_select);
                },
            );
            set_row(
                ui,
                p,
                "URL проверки",
                "Используется для пинга через прокси",
                |ui| {
                    plain_input(ui, &mut draft.test_url, 240.0);
                },
            );
            set_row_last(
                ui,
                p,
                "Уровень логов",
                "Подробность вывода ядра",
                |ui| {
                    egui::ComboBox::from_id_salt("log_level")
                        .selected_text(&draft.log_level)
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            for level in ["trace", "debug", "info", "warn", "error"] {
                                ui.selectable_value(&mut draft.log_level, level.to_string(), level);
                            }
                        });
                },
            );
            let _ = (view, snapshot);
        },
    );
}

fn settings_core(
    ui: &mut Ui,
    p: &Palette,
    view: &mut View,
    snapshot: &Snapshot,
    engine: &Engine,
    draft: &mut Settings,
) {
    settings_card(
        ui,
        p,
        "Ядро",
        "ProtoGrid, sing-box или Xray-core — каждое со своим бинарником",
        |ui| {
            Frame::new()
                .inner_margin(Margin::symmetric(18, 10))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.spacing_mut().item_spacing.x = 8.0;
                    card_grid(ui, 3, 8.0, |ui, index| {
                        let kind = match index {
                            0 => CoreKind::Protogrid,
                            1 => CoreKind::Singbox,
                            _ => CoreKind::Xray,
                        };
                        core_option(ui, p, snapshot, kind, draft);
                    });
                });
            let (line, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(line, 0, p.line);
            set_row(
                ui,
                p,
                "Путь к sing-box",
                "Пусто — автоматический поиск",
                |ui| {
                    plain_input(ui, &mut draft.singbox_path, 220.0);
                },
            );
            set_row(
                ui,
                p,
                "Путь к Xray",
                "Пусто — автоматический поиск",
                |ui| {
                    plain_input(ui, &mut draft.xray_path, 220.0);
                },
            );
            Frame::new()
                .inner_margin(Margin::symmetric(18, 12))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        if action_button(
                            ui,
                            p,
                            egui_phosphor::regular::ARROWS_CLOCKWISE,
                            "Проверить ядра",
                            BtnKind::Ghost,
                        )
                        .clicked()
                        {
                            engine.refresh_cores();
                        }
                        let (icon, label) = if draft.core == CoreKind::Protogrid {
                            (egui_phosphor::regular::PACKAGE, "Восстановить из комплекта")
                        } else {
                            (
                                egui_phosphor::regular::DOWNLOAD_SIMPLE,
                                "Скачать обновление",
                            )
                        };
                        if action_button(ui, p, icon, label, BtnKind::Outline).clicked() {
                            engine.install_core(draft.core);
                        }
                    });
                });
            let _ = view;
        },
    );
}

fn core_option(
    ui: &mut Ui,
    p: &Palette,
    snapshot: &Snapshot,
    kind: CoreKind,
    draft: &mut Settings,
) {
    let selected = draft.core == kind;
    let info = snapshot.core(kind);
    let features = info.map(|info| info.features).unwrap_or_default();
    let card = Frame::new()
        .fill(if selected { p.accent_soft } else { p.panel_2 })
        .stroke(Stroke::new(1.0, if selected { p.accent } else { p.line }))
        .corner_radius(10)
        .inner_margin(Margin::same(11))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(theme::medium(
                        match kind {
                            CoreKind::Protogrid => "ProtoGrid",
                            CoreKind::Singbox => "sing-box",
                            CoreKind::Xray => "Xray-core",
                        },
                        12.0,
                    ));
                    ui.label(
                        RichText::new(
                            info.and_then(|info| info.version.clone())
                                .map(|version| format!("v{version}"))
                                .unwrap_or_else(|| "не найден".into()),
                        )
                        .size(9.5)
                        .color(p.faint),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (radio, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                    ui.painter().circle_stroke(
                        radio.center(),
                        7.0,
                        Stroke::new(1.5, if selected { p.accent } else { p.line_strong }),
                    );
                    if selected {
                        ui.painter().circle_filled(radio.center(), 4.0, p.accent);
                    }
                });
            });
            ui.add_space(9.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                for protocol in core_protocols(kind, features) {
                    mini_protocol_chip(ui, p, protocol);
                }
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                for feature in core_feature_list(features) {
                    mini_feature_chip(ui, p, feature);
                }
            });
        });
    let response = ui
        .interact(
            card.response.rect,
            ui.id().with(("core", kind.name())),
            Sense::click(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.clicked() {
        draft.core = kind;
        if !kind.is_singbox_like() {
            draft.tun_enabled = false;
        }
    }
}

fn settings_about(ui: &mut Ui, p: &Palette, snapshot: &Snapshot, engine: &Engine) {
    settings_card(
        ui,
        p,
        "О приложении",
        "ProtoGrid · нативный клиент ProtoGrid, sing-box и Xray-core",
        |ui| {
            Frame::new()
                .inner_margin(Margin::symmetric(18, 14))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    card_grid(ui, 3, 0.0, |ui, index| match index {
                        0 => {
                            ui.label(
                                RichText::new("Версия")
                                    .size(9.5)
                                    .color(p.faint)
                                    .family(FontFamily::Name("Medium".into())),
                            );
                            ui.label(theme::medium(
                                format!("{} · {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS),
                                12.5,
                            ));
                            ui.label(
                                RichText::new("Без WebView и телеметрии")
                                    .size(10.0)
                                    .color(p.faint),
                            );
                        }
                        1 => {
                            ui.label(
                                RichText::new("Папка данных")
                                    .size(9.5)
                                    .color(p.faint)
                                    .family(FontFamily::Name("Medium".into())),
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&snapshot.data_dir)
                                        .size(11.0)
                                        .color(p.muted)
                                        .family(FontFamily::Monospace),
                                )
                                .truncate()
                                .selectable(false),
                            );
                            ui.label(
                                RichText::new(&snapshot.config_path)
                                    .size(9.5)
                                    .color(p.faint)
                                    .family(FontFamily::Monospace),
                            );
                        }
                        _ => {
                            ui.label(
                                RichText::new("Данные")
                                    .size(9.5)
                                    .color(p.faint)
                                    .family(FontFamily::Name("Medium".into())),
                            );
                            ui.label(
                                RichText::new("Настройки, подписки и кэш ядер хранятся локально.")
                                    .size(10.5)
                                    .color(p.muted),
                            );
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Открыть папку").size(11.0).color(p.muted),
                                    )
                                    .fill(p.panel_2)
                                    .stroke(Stroke::new(1.0, p.line))
                                    .corner_radius(7),
                                )
                                .clicked()
                            {
                                engine.open_data_dir();
                            }
                        }
                    });
                });
        },
    );
}

/* -------------------------------------------------------------------------- */
/* Modals                                                                     */
/* -------------------------------------------------------------------------- */

pub fn modals(
    ctx: &egui::Context,
    p: &Palette,
    view: &mut View,
    snapshot: &Snapshot,
    engine: &Engine,
) {
    if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
        view.modal = None;
        view.confirm = None;
        return;
    }
    if view.modal.is_none() {
        return;
    }
    let frame = Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0, p.line_strong))
        .corner_radius(16)
        .inner_margin(Margin::same(20));
    let viewport_height = ctx.input(|input| input.viewport_rect().height());
    let max_scroll = (viewport_height - 150.0).clamp(320.0, 720.0);
    let min_scroll = (viewport_height - 260.0).clamp(300.0, 620.0);
    let response = Modal::new(Id::new("protogrid-modal"))
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(600.0);
            ScrollArea::vertical()
                .max_height(max_scroll)
                .min_scrolled_height(min_scroll)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .auto_shrink([false, true])
                .show(ui, |ui| match view.modal {
                    Some(ModalState::Import) => import_modal(ui, p, view, engine),
                    Some(ModalState::Server) => server_modal(ui, p, view, snapshot, engine),
                    Some(ModalState::Subscription) => subscription_modal(ui, p, view, engine),
                    Some(ModalState::Rule) => rule_modal(ui, p, view, engine),
                    Some(ModalState::Confirm) => confirm_modal(ui, p, view, engine),
                    Some(ModalState::Help) => help_modal(ui, p),
                    None => {}
                });
        });
    if response.should_close() {
        view.modal = None;
        view.confirm = None;
    }
}

fn modal_title(ui: &mut Ui, p: &Palette, title: &str, text: &str) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(theme::semibold(title, 16.0));
            if !text.is_empty() {
                ui.label(RichText::new(text).size(10.5).color(p.muted));
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if round_icon_button(ui, p, egui_phosphor::regular::X, "Закрыть").clicked() {
                ui.close();
            }
        });
    });
    ui.add_space(12.0);
}

fn help_modal(ui: &mut Ui, p: &Palette) {
    modal_title(
        ui,
        p,
        "ProtoGrid",
        "Нативный клиент sing-box и Xray-core на egui.",
    );
    ui.label(
        RichText::new(
            "Серверы выбираются двойным щелчком по строке или кнопкой «Подключить». \
             Верхняя панель управляет соединением, пингом и перезапуском ядра.",
        )
        .size(11.5)
        .color(p.muted),
    );
    ui.add_space(10.0);
    for (title, text) in [
        (
            "Группы",
            "Вкладки над таблицей фильтруют серверы по подписке. «Все» показывает полный список.",
        ),
        (
            "Журнал",
            "Вывод приложения и ядра в реальном времени с фильтрами по уровню и тексту.",
        ),
        (
            "Маршрутизация",
            "Пресеты и собственные правила: PROXY, DIRECT или BLOCK.",
        ),
        (
            "Графики",
            "Скорость и распределение трафика по маршрутам за последние 15 минут.",
        ),
        (
            "Данные",
            "Настройки, подписки и кэш ядер хранятся локально в папке данных приложения.",
        ),
    ] {
        Frame::new()
            .fill(p.panel_2)
            .stroke(Stroke::new(1.0, p.line))
            .corner_radius(8)
            .inner_margin(Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(theme::medium(title, 11.5));
                ui.label(RichText::new(text).size(10.5).color(p.muted));
            });
        ui.add_space(6.0);
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new("Закрыть").size(11.5).color(Color32::WHITE))
                        .fill(p.accent)
                        .corner_radius(8)
                        .min_size(Vec2::new(0.0, 32.0)),
                )
                .clicked()
            {
                ui.close();
            }
        });
    });
}

fn import_modal(ui: &mut Ui, p: &Palette, view: &mut View, engine: &Engine) {
    modal_title(
        ui,
        p,
        "Импорт серверов",
        "Ссылки v2rayN, base64-подписка или Clash YAML.",
    );
    ui.label(
        RichText::new("Ссылки или подписка")
            .size(10.5)
            .color(p.muted),
    );
    ui.add(
        TextEdit::multiline(&mut view.import_text)
            .desired_rows(8)
            .desired_width(f32::INFINITY)
            .hint_text(
                "vless://...\nvmess://...\ntrojan://...\nss://...\nhysteria2://...\ntuic://...",
            ),
    );
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    RichText::new(format!(
                        "{}  Вставить из буфера",
                        egui_phosphor::regular::CLIPBOARD
                    ))
                    .size(11.0)
                    .color(p.muted),
                )
                .fill(p.panel_2)
                .stroke(Stroke::new(1.0, p.line))
                .corner_radius(8),
            )
            .clicked()
        {
            match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
                Ok(text) => view.import_text = text,
                Err(_) => view.toast("Буфер обмена недоступен", ToastKind::Warn),
            }
        }
        ui.label(
            RichText::new("VLESS · VMess · Trojan · SS · Hysteria2 · TUIC")
                .size(9.0)
                .color(p.faint),
        );
    });
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(
                        RichText::new("Импортировать")
                            .size(11.5)
                            .color(Color32::WHITE),
                    )
                    .fill(p.accent)
                    .corner_radius(8)
                    .min_size(Vec2::new(0.0, 32.0)),
                )
                .clicked()
            {
                match engine.import_links(&view.import_text) {
                    Ok(added) => {
                        view.toast(format!("Добавлено серверов: {added}"), ToastKind::Success);
                        view.modal = None;
                    }
                    Err(error) => view.toast(error, ToastKind::Error),
                }
            }
            if ui
                .add(
                    egui::Button::new(RichText::new("Отмена").size(11.5))
                        .fill(Color32::TRANSPARENT)
                        .corner_radius(8)
                        .min_size(Vec2::new(0.0, 32.0)),
                )
                .clicked()
            {
                view.modal = None;
            }
        });
    });
}

fn subscription_modal(ui: &mut Ui, p: &Palette, view: &mut View, engine: &Engine) {
    let editing = view.subscription_edit.is_some();
    modal_title(
        ui,
        p,
        if editing {
            "Изменить подписку"
        } else {
            "Новая подписка"
        },
        "ProtoGrid скачает серверы и данные о трафике.",
    );
    field(
        ui,
        p,
        "Название",
        TextEdit::singleline(&mut view.subscription_name)
            .hint_text("Например, Premium")
            .desired_width(f32::INFINITY),
    );
    ui.add_space(8.0);
    field(
        ui,
        p,
        "Ссылка на подписку",
        TextEdit::singleline(&mut view.subscription_url)
            .hint_text("https://example.com/sub?token=...")
            .desired_width(f32::INFINITY),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new("Ссылка должна начинаться с http:// или https://.")
            .size(9.5)
            .color(p.faint),
    );
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(if editing {
                            "Сохранить"
                        } else {
                            "Загрузить"
                        })
                        .size(11.5)
                        .color(Color32::WHITE),
                    )
                    .fill(p.accent)
                    .corner_radius(8)
                    .min_size(Vec2::new(0.0, 32.0)),
                )
                .clicked()
            {
                if view.subscription_url.trim().is_empty() {
                    view.toast("Укажите ссылку на подписку", ToastKind::Warn);
                } else if let Some(id) = view.subscription_edit.clone() {
                    engine.update_subscription(
                        id,
                        view.subscription_name.clone(),
                        view.subscription_url.clone(),
                    );
                    view.subscription_edit = None;
                    view.modal = None;
                } else {
                    engine.add_subscription(
                        view.subscription_name.clone(),
                        view.subscription_url.clone(),
                    );
                    view.modal = None;
                }
            }
            if ui
                .add(
                    egui::Button::new(RichText::new("Отмена").size(11.5))
                        .fill(Color32::TRANSPARENT)
                        .corner_radius(8)
                        .min_size(Vec2::new(0.0, 32.0)),
                )
                .clicked()
            {
                view.subscription_edit = None;
                view.modal = None;
            }
        });
    });
}

fn rule_modal(ui: &mut Ui, p: &Palette, view: &mut View, engine: &Engine) {
    let Some(mut rule) = view.rule_draft.clone() else {
        view.modal = None;
        return;
    };
    modal_title(
        ui,
        p,
        if rule.id.is_empty() {
            "Новое правило"
        } else {
            "Изменить правило"
        },
        "Правило сработает только при совпадении условия.",
    );
    field(
        ui,
        p,
        "Название",
        TextEdit::singleline(&mut rule.name).desired_width(f32::INFINITY),
    );
    ui.add_space(8.0);
    ui.columns(2, |columns| {
        columns[0].label(RichText::new("Действие").size(10.5).color(p.muted));
        egui::ComboBox::from_id_salt("rule-action")
            .selected_text(rule_action_label(rule.action))
            .width(190.0)
            .show_ui(&mut columns[0], |ui| {
                ui.selectable_value(&mut rule.action, RuleAction::Proxy, "Через прокси");
                ui.selectable_value(&mut rule.action, RuleAction::Direct, "Напрямую");
                ui.selectable_value(&mut rule.action, RuleAction::Block, "Блокировать");
            });
        columns[1].label(RichText::new("Тип").size(10.5).color(p.muted));
        egui::ComboBox::from_id_salt("rule-kind")
            .selected_text(rule_kind_label(rule.kind))
            .width(190.0)
            .show_ui(&mut columns[1], |ui| {
                for (kind, label) in rule_kinds() {
                    ui.selectable_value(&mut rule.kind, kind, label);
                }
            });
    });
    ui.add_space(8.0);
    field(
        ui,
        p,
        "Значение",
        TextEdit::singleline(&mut rule.value)
            .hint_text(rule_hint(rule.kind))
            .desired_width(f32::INFINITY),
    );
    ui.add_space(6.0);
    ui.checkbox(&mut rule.enabled, "Правило включено");
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new("Сохранить").size(11.5).color(Color32::WHITE))
                        .fill(p.accent)
                        .corner_radius(8)
                        .min_size(Vec2::new(0.0, 32.0)),
                )
                .clicked()
            {
                engine.save_rule(rule.clone());
                view.modal = None;
            }
            if ui
                .add(
                    egui::Button::new(RichText::new("Отмена").size(11.5))
                        .fill(Color32::TRANSPARENT)
                        .corner_radius(8)
                        .min_size(Vec2::new(0.0, 32.0)),
                )
                .clicked()
            {
                view.modal = None;
            }
        });
    });
    view.rule_draft = Some(rule);
}

fn server_modal(ui: &mut Ui, p: &Palette, view: &mut View, snapshot: &Snapshot, engine: &Engine) {
    let Some(mut server) = view.server_draft.clone() else {
        view.modal = None;
        return;
    };
    let is_new = snapshot.server(&server.id).is_none();
    let core = snapshot.config.settings.core;
    let features = snapshot
        .core(core)
        .map(|info| info.features)
        .unwrap_or_default();
    modal_title(
        ui,
        p,
        if is_new {
            "Новый сервер"
        } else {
            "Изменить сервер"
        },
        "Параметры можно ввести вручную или импортировать ссылку.",
    );
    ui.horizontal_wrapped(|ui| {
        for protocol in [
            Protocol::Vless,
            Protocol::Vmess,
            Protocol::Trojan,
            Protocol::Shadowsocks,
            Protocol::Hysteria2,
            Protocol::Tuic,
            Protocol::Masque,
        ] {
            let supported =
                protocol.supports(core) && (protocol != Protocol::Masque || features.masque);
            if ui
                .add_enabled_ui(supported, |ui| {
                    ui.selectable_label(server.protocol == protocol, protocol_name(protocol))
                })
                .inner
                .clicked()
            {
                server.protocol = protocol;
                server.port = protocol.default_port();
                if matches!(
                    protocol,
                    Protocol::Hysteria2 | Protocol::Tuic | Protocol::Masque
                ) {
                    server.security = Security::Tls;
                }
                if protocol == Protocol::Shadowsocks {
                    server.security = Security::None;
                }
            }
        }
    });
    if !server_compatible(
        &server,
        snapshot.config.settings.core,
        snapshot.capabilities(snapshot.config.settings.core),
    ) {
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!(
                "{}  Несовместимо с ядром {}",
                egui_phosphor::regular::WARNING,
                snapshot.config.settings.core.name()
            ))
            .size(10.0)
            .color(p.amber),
        );
    }
    ui.add_space(8.0);
    ui.columns(2, |columns| {
        field(
            &mut columns[0],
            p,
            "Название",
            TextEdit::singleline(&mut server.name).desired_width(f32::INFINITY),
        );
        field(
            &mut columns[1],
            p,
            "Адрес",
            TextEdit::singleline(&mut server.address).desired_width(f32::INFINITY),
        );
    });
    ui.add_space(6.0);
    ui.columns(2, |columns| {
        columns[0].label(RichText::new("Порт").size(10.5).color(p.muted));
        columns[0].add(egui::DragValue::new(&mut server.port).range(1..=65535));
        match server.protocol {
            Protocol::Vless | Protocol::Vmess => {
                field(
                    &mut columns[1],
                    p,
                    "UUID",
                    TextEdit::singleline(&mut server.uuid).desired_width(f32::INFINITY),
                );
            }
            Protocol::Trojan | Protocol::Hysteria2 => {
                field(
                    &mut columns[1],
                    p,
                    "Пароль",
                    TextEdit::singleline(&mut server.password).desired_width(f32::INFINITY),
                );
            }
            Protocol::Shadowsocks => {
                field(
                    &mut columns[1],
                    p,
                    "Пароль",
                    TextEdit::singleline(&mut server.password).desired_width(f32::INFINITY),
                );
            }
            Protocol::Tuic => {
                field(
                    &mut columns[1],
                    p,
                    "UUID",
                    TextEdit::singleline(&mut server.uuid).desired_width(f32::INFINITY),
                );
            }
            Protocol::Masque => {}
        }
    });
    if server.protocol == Protocol::Tuic {
        field(
            ui,
            p,
            "Пароль",
            TextEdit::singleline(&mut server.password).desired_width(f32::INFINITY),
        );
    }
    if server.protocol == Protocol::Shadowsocks {
        field(
            ui,
            p,
            "Метод",
            TextEdit::singleline(&mut server.method).desired_width(f32::INFINITY),
        );
    }
    if server.protocol == Protocol::Vless {
        field(
            ui,
            p,
            "Flow",
            TextEdit::singleline(&mut server.flow)
                .hint_text("xtls-rprx-vision")
                .desired_width(f32::INFINITY),
        );
        ui.add_enabled_ui(features.vless_encryption, |ui| {
            ui.label(
                RichText::new("VLESS Encryption (Xray 26, необязательно)")
                    .size(10.0)
                    .color(p.muted),
            );
            ui.add(
                TextEdit::multiline(&mut server.encryption)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY)
                    .hint_text("mlkem768x25519plus..."),
            );
        });
    }
    if server.protocol == Protocol::Vmess {
        ui.columns(2, |columns| {
            columns[0].label(RichText::new("VMess security").size(10.5).color(p.muted));
            egui::ComboBox::from_id_salt("vmess-security")
                .selected_text(if server.vmess_security.is_empty() {
                    "auto"
                } else {
                    &server.vmess_security
                })
                .width(190.0)
                .show_ui(&mut columns[0], |ui| {
                    for value in ["auto", "aes-128-gcm", "chacha20-poly1305", "none", "zero"] {
                        ui.selectable_value(&mut server.vmess_security, value.to_string(), value);
                    }
                });
            columns[1].label(RichText::new("Alter ID").size(10.5).color(p.muted));
            columns[1].add(egui::DragValue::new(&mut server.vmess_alter_id).range(0..=65535));
        });
    }
    if server.protocol == Protocol::Masque {
        ui.columns(2, |columns| {
            field(
                &mut columns[0],
                p,
                "User",
                TextEdit::singleline(&mut server.masque_user).desired_width(f32::INFINITY),
            );
            field(
                &mut columns[1],
                p,
                "Password",
                TextEdit::singleline(&mut server.masque_pass)
                    .password(true)
                    .desired_width(f32::INFINITY),
            );
        });
        field(
            ui,
            p,
            "MASQUE host",
            TextEdit::singleline(&mut server.host).desired_width(f32::INFINITY),
        );
        field(
            ui,
            p,
            "MASQUE path",
            TextEdit::singleline(&mut server.path)
                .hint_text("/.well-known/masque/ip/*/*/")
                .desired_width(f32::INFINITY),
        );
        field(
            ui,
            p,
            "Pinned cert SHA256 (hex)",
            TextEdit::singleline(&mut server.pinned_peer_cert_sha256).desired_width(f32::INFINITY),
        );
        field(
            ui,
            p,
            "Remote DNS (через запятую)",
            TextEdit::singleline(&mut server.masque_remote_dns).desired_width(f32::INFINITY),
        );
    }
    if server.protocol != Protocol::Shadowsocks {
        ui.add_space(8.0);
        ui.separator();
        ui.label(
            RichText::new("TLS И МАСКИРОВКА")
                .size(9.0)
                .color(p.faint)
                .family(FontFamily::Name("Medium".into())),
        );
        ui.columns(2, |columns| {
            if matches!(
                server.protocol,
                Protocol::Hysteria2 | Protocol::Tuic | Protocol::Masque
            ) {
                columns[0].label(RichText::new("Защита").size(10.5).color(p.muted));
                columns[0].label("TLS включён");
            } else {
                columns[0].label(RichText::new("Защита").size(10.5).color(p.muted));
                egui::ComboBox::from_id_salt("server-security")
                    .selected_text(security_label(server.security))
                    .width(190.0)
                    .show_ui(&mut columns[0], |ui| {
                        ui.selectable_value(&mut server.security, Security::None, "Без TLS");
                        ui.selectable_value(&mut server.security, Security::Tls, "TLS");
                        if matches!(server.protocol, Protocol::Vless | Protocol::Vmess) {
                            ui.selectable_value(&mut server.security, Security::Reality, "Reality");
                        }
                    });
            }
            field(
                &mut columns[1],
                p,
                "SNI",
                TextEdit::singleline(&mut server.sni).desired_width(f32::INFINITY),
            );
        });
        ui.columns(2, |columns| {
            columns[0].label(RichText::new("ALPN").size(10.5).color(p.muted));
            columns[0]
                .add(TextEdit::singleline(&mut view.server_alpn).desired_width(f32::INFINITY));
            field(
                &mut columns[1],
                p,
                "Отпечаток",
                TextEdit::singleline(&mut server.fingerprint)
                    .hint_text("chrome")
                    .desired_width(f32::INFINITY),
            );
        });
        ui.checkbox(
            &mut server.allow_insecure,
            "Разрешить недействительный сертификат",
        );
        if server.security == Security::Reality {
            ui.columns(3, |columns| {
                field(
                    &mut columns[0],
                    p,
                    "Public Key",
                    TextEdit::singleline(&mut server.reality_public_key)
                        .desired_width(f32::INFINITY),
                );
                field(
                    &mut columns[1],
                    p,
                    "Short ID",
                    TextEdit::singleline(&mut server.reality_short_id).desired_width(f32::INFINITY),
                );
                field(
                    &mut columns[2],
                    p,
                    "SpiderX",
                    TextEdit::singleline(&mut server.reality_spider_x).desired_width(f32::INFINITY),
                );
            });
            ui.add_enabled_ui(features.mldsa65, |ui| {
                ui.label(
                    RichText::new("ML-DSA-65 Verify (Xray 26, необязательно)")
                        .size(10.0)
                        .color(p.muted),
                );
                ui.add(
                    TextEdit::multiline(&mut server.reality_mldsa65_verify)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY)
                        .hint_text("mldsa65Verify"),
                );
            });
        }
    }
    if matches!(
        server.protocol,
        Protocol::Vless | Protocol::Vmess | Protocol::Trojan
    ) {
        ui.add_space(6.0);
        ui.columns(2, |columns| {
            columns[0].label(RichText::new("Транспорт").size(10.5).color(p.muted));
            egui::ComboBox::from_id_salt("server-transport")
                .selected_text(transport_label(server.transport))
                .width(190.0)
                .show_ui(&mut columns[0], |ui| {
                    for (transport, label) in [
                        (Transport::Tcp, "TCP"),
                        (Transport::Ws, "WebSocket"),
                        (Transport::Grpc, "gRPC"),
                        (Transport::Http, "HTTP/2"),
                        (Transport::HttpUpgrade, "HTTP Upgrade"),
                        (Transport::Xhttp, "XHTTP"),
                        (Transport::Kcp, "mKCP"),
                        (Transport::XDrive, "XDRIVE"),
                    ] {
                        let supported = match transport {
                            Transport::Http => core != CoreKind::Xray,
                            Transport::Xhttp => features.xhttp,
                            Transport::Kcp => features.kcp,
                            Transport::XDrive => features.xdrive,
                            _ => true,
                        };
                        ui.add_enabled_ui(supported, |ui| {
                            ui.selectable_value(&mut server.transport, transport, label);
                        });
                    }
                });
            if server.transport == Transport::Grpc {
                field(
                    &mut columns[1],
                    p,
                    "Service Name",
                    TextEdit::singleline(&mut server.service_name).desired_width(f32::INFINITY),
                );
            } else if server.transport == Transport::Kcp {
                columns[1].label(RichText::new("MTU").size(10.5).color(p.muted));
                columns[1].add(egui::DragValue::new(&mut server.kcp_mtu).range(21..=1500));
                columns[1].label(RichText::new("TTI").size(10.5).color(p.muted));
                columns[1].add(egui::DragValue::new(&mut server.kcp_tti).range(10..=1000));
            } else {
                field(
                    &mut columns[1],
                    p,
                    "Path",
                    TextEdit::singleline(&mut server.path).desired_width(f32::INFINITY),
                );
            }
        });
        if matches!(
            server.transport,
            Transport::Ws | Transport::Http | Transport::HttpUpgrade | Transport::Xhttp
        ) {
            field(
                ui,
                p,
                "Host",
                TextEdit::singleline(&mut server.host).desired_width(f32::INFINITY),
            );
        }
        if server.transport == Transport::Xhttp {
            ui.horizontal(|ui| {
                ui.label(RichText::new("XHTTP mode").size(10.5).color(p.muted));
                egui::ComboBox::from_id_salt("server-xhttp-mode")
                    .selected_text(xhttp_mode_label(server.effective_xhttp_mode()))
                    .width(190.0)
                    .show_ui(ui, |ui| {
                        for mode in ["auto", "packet-up", "stream-up", "stream-one"] {
                            ui.selectable_value(
                                &mut server.xhttp_mode,
                                mode.to_string(),
                                xhttp_mode_label(mode),
                            );
                        }
                    });
            });
            ui.label(
                RichText::new(
                    "packet-up — для CDN; stream-one — для прямого TLS/Reality; auto выбирает stream-one.",
                )
                .size(10.0)
                .color(p.muted),
            );
        }
        if server.transport == Transport::Kcp {
            ui.columns(2, |columns| {
                columns[0].label(RichText::new("Uplink, Мбит/с").size(10.5).color(p.muted));
                columns[0].add(egui::DragValue::new(&mut server.kcp_uplink).range(1..=1000));
                columns[1].label(RichText::new("Downlink, Мбит/с").size(10.5).color(p.muted));
                columns[1].add(egui::DragValue::new(&mut server.kcp_downlink).range(1..=1000));
            });
            ui.checkbox(&mut server.kcp_congestion, "Congestion control");
            field(
                ui,
                p,
                "Seed",
                TextEdit::singleline(&mut server.kcp_seed).desired_width(f32::INFINITY),
            );
        }
        if server.transport == Transport::XDrive {
            ui.columns(2, |columns| {
                field(
                    &mut columns[0],
                    p,
                    "Service",
                    TextEdit::singleline(&mut server.xdrive_service)
                        .hint_text("local / google-drive / s3")
                        .desired_width(f32::INFINITY),
                );
                field(
                    &mut columns[1],
                    p,
                    "Remote folder",
                    TextEdit::singleline(&mut server.xdrive_remote_folder)
                        .desired_width(f32::INFINITY),
                );
            });
            field(
                ui,
                p,
                "Secrets (через запятую)",
                TextEdit::singleline(&mut server.xdrive_secrets).desired_width(f32::INFINITY),
            );
            if server.xdrive_service == "s3" {
                ui.columns(2, |columns| {
                    field(
                        &mut columns[0],
                        p,
                        "S3 endpoint",
                        TextEdit::singleline(&mut server.xdrive_s3_endpoint)
                            .hint_text("http://127.0.0.1:9000")
                            .desired_width(f32::INFINITY),
                    );
                    field(
                        &mut columns[1],
                        p,
                        "S3 region",
                        TextEdit::singleline(&mut server.xdrive_s3_region)
                            .hint_text("us-east-1")
                            .desired_width(f32::INFINITY),
                    );
                });
                field(
                    ui,
                    p,
                    "S3 bucket",
                    TextEdit::singleline(&mut server.xdrive_s3_bucket).desired_width(f32::INFINITY),
                );
                ui.checkbox(&mut server.xdrive_s3_path_style, "S3 path-style");
            }
        }
    }
    ui.collapsing("Дополнительно", |ui| {
        ui.columns(2, |columns| {
            field(
                &mut columns[0],
                p,
                "Bind interface",
                TextEdit::singleline(&mut server.bind_interface)
                    .hint_text("eth0")
                    .desired_width(f32::INFINITY),
            );
            columns[1].label(RichText::new("Routing mark").size(10.5).color(p.muted));
            columns[1].add(egui::DragValue::new(&mut server.routing_mark).range(0..=u32::MAX));
        });
        ui.checkbox(&mut server.tcp_fast_open, "TCP Fast Open");
        ui.checkbox(
            &mut server.fragment_enabled,
            "Fragment (Xray finalmask / sing-box TLS fragment)",
        );
        if server.fragment_enabled {
            ui.columns(2, |columns| {
                field(
                    &mut columns[0],
                    p,
                    "Packets",
                    TextEdit::singleline(&mut server.fragment_packets)
                        .hint_text("tlshello")
                        .desired_width(f32::INFINITY),
                );
                field(
                    &mut columns[1],
                    p,
                    "Length",
                    TextEdit::singleline(&mut server.fragment_length)
                        .hint_text("100-200")
                        .desired_width(f32::INFINITY),
                );
                field(
                    &mut columns[0],
                    p,
                    "Delay",
                    TextEdit::singleline(&mut server.fragment_delay)
                        .hint_text("10-20")
                        .desired_width(f32::INFINITY),
                );
                field(
                    &mut columns[1],
                    p,
                    "Max split",
                    TextEdit::singleline(&mut server.fragment_max_split)
                        .hint_text("3-6")
                        .desired_width(f32::INFINITY),
                );
            });
        }
        ui.add_enabled_ui(features.noise, |ui| {
            ui.checkbox(&mut server.noise_enabled, "UDP noise (finalmask)");
            if server.noise_enabled {
                ui.columns(2, |columns| {
                    field(
                        &mut columns[0],
                        p,
                        "Noise type",
                        TextEdit::singleline(&mut server.noise_type)
                            .hint_text("str / hex / base64")
                            .desired_width(f32::INFINITY),
                    );
                    field(
                        &mut columns[1],
                        p,
                        "Rand",
                        TextEdit::singleline(&mut server.noise_rand)
                            .hint_text("1-3")
                            .desired_width(f32::INFINITY),
                    );
                });
                field(
                    ui,
                    p,
                    "Noise packet",
                    TextEdit::singleline(&mut server.noise_packet).desired_width(f32::INFINITY),
                );
            }
            ui.label(
                RichText::new("Finalmask TCP (JSON-массив масок, например filter/header-custom)")
                    .size(10.0)
                    .color(p.muted),
            );
            ui.add(
                TextEdit::multiline(&mut server.finalmask_tcp)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY)
                    .hint_text(
                        r#"[{"type":"filter","settings":{"blockedSourceIPs":["192.0.2.0/24"]}}]"#,
                    ),
            );
            ui.label(
                RichText::new("Finalmask UDP (JSON-массив масок)")
                    .size(10.0)
                    .color(p.muted),
            );
            ui.add(
                TextEdit::multiline(&mut server.finalmask_udp)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
        });
        if !features.noise {
            ui.label(
                RichText::new("UDP noise и finalmask доступны только в Xray-core")
                    .size(10.0)
                    .color(p.faint),
            );
        }
        if server.protocol == Protocol::Shadowsocks {
            field(
                ui,
                p,
                "Плагин",
                TextEdit::singleline(&mut server.ss_plugin).desired_width(f32::INFINITY),
            );
            field(
                ui,
                p,
                "Параметры плагина",
                TextEdit::singleline(&mut server.ss_plugin_opts).desired_width(f32::INFINITY),
            );
        }
        if server.protocol == Protocol::Hysteria2 {
            ui.columns(2, |columns| {
                field(
                    &mut columns[0],
                    p,
                    "Obfs",
                    TextEdit::singleline(&mut server.obfs)
                        .hint_text("salamander")
                        .desired_width(f32::INFINITY),
                );
                field(
                    &mut columns[1],
                    p,
                    "Пароль obfs",
                    TextEdit::singleline(&mut server.obfs_password).desired_width(f32::INFINITY),
                );
            });
        }
        if server.protocol == Protocol::Tuic {
            ui.columns(2, |columns| {
                field(
                    &mut columns[0],
                    p,
                    "Congestion Control",
                    TextEdit::singleline(&mut server.congestion_control)
                        .desired_width(f32::INFINITY),
                );
                field(
                    &mut columns[1],
                    p,
                    "UDP Relay Mode",
                    TextEdit::singleline(&mut server.udp_relay_mode).desired_width(f32::INFINITY),
                );
            });
        }
    });
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new("Сохранить").size(11.5).color(Color32::WHITE))
                        .fill(p.accent)
                        .corner_radius(8)
                        .min_size(Vec2::new(0.0, 32.0)),
                )
                .clicked()
            {
                if server.address.trim().is_empty() {
                    view.toast("Укажите адрес сервера", ToastKind::Warn);
                } else if server.port == 0 {
                    view.toast("Укажите порт", ToastKind::Warn);
                } else {
                    if server.name.trim().is_empty() {
                        server.name = server_target(&server);
                    }
                    server.alpn = view
                        .server_alpn
                        .split(',')
                        .map(|item| item.trim().to_string())
                        .filter(|item| !item.is_empty())
                        .collect();
                    engine.save_server(server.clone());
                    view.modal = None;
                }
            }
            if ui
                .add(
                    egui::Button::new(RichText::new("Отмена").size(11.5))
                        .fill(Color32::TRANSPARENT)
                        .corner_radius(8)
                        .min_size(Vec2::new(0.0, 32.0)),
                )
                .clicked()
            {
                view.modal = None;
            }
        });
    });
    view.server_draft = Some(server);
}

fn confirm_modal(ui: &mut Ui, p: &Palette, view: &mut View, engine: &Engine) {
    let Some(confirm) = view.confirm.clone() else {
        view.modal = None;
        return;
    };
    modal_title(ui, p, &confirm.title, "");
    ui.label(RichText::new(&confirm.message).size(11.5).color(p.muted));
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let button = if confirm.danger {
                egui::Button::new(
                    RichText::new(&confirm.label)
                        .size(11.5)
                        .color(Color32::WHITE),
                )
                .fill(p.red)
                .corner_radius(8)
                .min_size(Vec2::new(0.0, 32.0))
            } else {
                egui::Button::new(
                    RichText::new(&confirm.label)
                        .size(11.5)
                        .color(Color32::WHITE),
                )
                .fill(p.accent)
                .corner_radius(8)
                .min_size(Vec2::new(0.0, 32.0))
            };
            if ui.add(button).clicked() {
                match &confirm.action {
                    ConfirmAction::DeleteServer(id) => engine.delete_server(id.as_str()),
                    ConfirmAction::DeleteSubscription(id) => {
                        engine.delete_subscription(id.as_str())
                    }
                    ConfirmAction::DeleteRule(id) => engine.delete_rule(id.as_str()),
                }
                view.modal = None;
                view.confirm = None;
            }
            if ui
                .add(
                    egui::Button::new(RichText::new("Отмена").size(11.5))
                        .fill(Color32::TRANSPARENT)
                        .corner_radius(8)
                        .min_size(Vec2::new(0.0, 32.0)),
                )
                .clicked()
            {
                view.modal = None;
                view.confirm = None;
            }
        });
    });
}

fn field(ui: &mut Ui, p: &Palette, label: &str, widget: TextEdit<'_>) {
    ui.label(RichText::new(label).size(10.5).color(p.muted));
    ui.add(widget);
}

/* -------------------------------------------------------------------------- */
/* Toasts                                                                     */
/* -------------------------------------------------------------------------- */

pub fn toasts(ctx: &egui::Context, p: &Palette, view: &mut View) {
    view.toasts
        .retain(|toast| toast.created.elapsed() < Duration::from_secs(6));
    if view.toasts.is_empty() {
        return;
    }
    egui::Area::new(Id::new("toasts"))
        .anchor(Align2::RIGHT_BOTTOM, Vec2::new(-22.0, -22.0))
        .order(egui::Order::Tooltip)
        .show(ctx, |ui| {
            ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                for toast in view.toasts.iter().rev() {
                    let color = semantics_color(p, toast.kind);
                    Frame::new()
                        .fill(p.panel)
                        .stroke(Stroke::new(1.0, p.line_strong))
                        .corner_radius(10)
                        .inner_margin(Margin::symmetric(14, 11))
                        .show(ui, |ui| {
                            ui.set_max_width(340.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 9.0;
                                let (strip, _) =
                                    ui.allocate_exact_size(Vec2::new(3.0, 18.0), Sense::hover());
                                ui.painter()
                                    .rect_filled(strip, CornerRadius::same(2), color);
                                ui.label(
                                    RichText::new(match toast.kind {
                                        ToastKind::Success => egui_phosphor::regular::CHECK_CIRCLE,
                                        ToastKind::Error => egui_phosphor::regular::X_CIRCLE,
                                        ToastKind::Warn => egui_phosphor::regular::WARNING,
                                        ToastKind::Info => egui_phosphor::regular::INFO,
                                    })
                                    .size(15.0)
                                    .color(color),
                                );
                                ui.label(RichText::new(&toast.text).size(12.0).color(p.text));
                            });
                        });
                    ui.add_space(8.0);
                }
            });
        });
    ctx.request_repaint_after(Duration::from_millis(300));
}

/* -------------------------------------------------------------------------- */
/* Root                                                                       */
/* -------------------------------------------------------------------------- */

pub fn show(ui: &mut Ui, view: &mut View, snapshot: &Snapshot, engine: &Engine) {
    let p = Palette::from_ctx(ui.ctx());
    view.tick(snapshot);

    let width = ui.ctx().input(|input| input.viewport_rect().width());
    let collapsed = width < 1040.0;

    if ui
        .ctx()
        .input(|input| input.modifiers.command && input.key_pressed(egui::Key::K))
    {
        ui.memory_mut(|memory| memory.request_focus(Id::new("side-search")));
    }
    if ui.ctx().input(|input| input.modifiers.command) {
        for (index, page) in Page::ALL.iter().enumerate() {
            let key = match index {
                0 => egui::Key::Num1,
                1 => egui::Key::Num2,
                2 => egui::Key::Num3,
                3 => egui::Key::Num4,
                4 => egui::Key::Num5,
                5 => egui::Key::Num6,
                _ => egui::Key::Num7,
            };
            if ui.ctx().input(|input| input.key_pressed(key)) {
                view.page = *page;
            }
        }
    }

    let sidebar_margin = if collapsed { 8 } else { 12 };
    egui::Panel::left("sidebar")
        .exact_size(if collapsed { 72.0 } else { 236.0 })
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().fill(p.sidebar).inner_margin(Margin {
            left: sidebar_margin,
            right: sidebar_margin,
            top: 16,
            bottom: 12,
        }))
        .show(ui, |ui| {
            let screen = ui.ctx().input(|input| input.viewport_rect());
            let content = ui.max_rect();
            sidebar(ui, &p, view, snapshot, collapsed);
            let x = content.right() + sidebar_margin as f32 - 1.0;
            ui.painter().rect_filled(
                Rect::from_min_size(egui::pos2(x, screen.top()), Vec2::new(1.0, screen.height())),
                0,
                p.line,
            );
        });

    egui::Panel::top("topbar")
        .exact_size(64.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(p.bg)
                .inner_margin(Margin::symmetric(24, 0)),
        )
        .show(ui, |ui| {
            let content = ui.max_rect();
            topbar(ui, &p, view, snapshot, engine);
            let y = content.bottom() - 1.0;
            ui.painter().rect_filled(
                Rect::from_min_size(
                    egui::pos2(content.left() - 24.0, y),
                    Vec2::new(content.width() + 48.0, 1.0),
                ),
                0,
                p.line,
            );
        });

    egui::CentralPanel::default()
        .frame(Frame::new().fill(p.bg).inner_margin(Margin {
            left: 24,
            right: 24,
            top: 22,
            bottom: 30,
        }))
        .show(ui, |ui| {
            ScrollArea::vertical()
                .id_salt("main-scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    match view.page {
                        Page::Overview => overview_page(ui, &p, view, snapshot, engine),
                        Page::Servers => servers_page(ui, &p, view, snapshot, engine),
                        Page::Subscriptions => subscriptions_page(ui, &p, view, snapshot, engine),
                        Page::Routing => routing_page(ui, &p, view, snapshot, engine),
                        Page::Charts => charts_page(ui, &p, view, snapshot),
                        Page::Logs => logs_page(ui, &p, view, snapshot, engine),
                        Page::Settings => settings_page(ui, &p, view, snapshot, engine),
                    }
                });
        });

    modals(ui.ctx(), &p, view, snapshot, engine);
    toasts(ui.ctx(), &p, view);
}
