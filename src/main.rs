#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

fn main() -> eframe::Result<()> {
    // setup logger to log automate events
    env_logger::builder()
        .filter_module("automate", log::LevelFilter::Info)
        .init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_min_inner_size([740.0, 407.0])
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(
                    &include_bytes!("../assets/icons/icon-256.png")[..],
                )
                .expect("Failed to load icon"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "untitled - Automate",
        native_options,
        Box::new(|cc| Ok(Box::new(automate::App::new(cc)))),
    )
}
