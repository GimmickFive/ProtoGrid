#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod autostart;
mod engine;
mod installer;
mod theme;
mod ui;

fn main() -> eframe::Result<()> {
    autostart::remove_legacy();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("ProtoGrid")
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([980.0, 660.0])
            .with_icon(app::icon_data(256)),
        ..Default::default()
    };
    eframe::run_native(
        "ProtoGrid",
        options,
        Box::new(|cc| Ok(Box::new(app::ProtoGridApp::new(cc)))),
    )
}
