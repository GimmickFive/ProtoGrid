#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod autostart;
mod brand;
mod engine;
mod flags;
mod installer;
mod theme;
mod ui;

fn main() -> eframe::Result<()> {
    autostart::remove_legacy();
    if std::env::args().any(|arg| arg == "--init") {
        if let Err(error) = engine::init_profile() {
            eprintln!("ProtoGrid --init: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }
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
