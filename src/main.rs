#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod app;
mod expiration;
mod inside_curve;
use app::ManualDataCleanerApp;

fn main() -> eframe::Result<()> {
    expiration::panic_if_expired();

    let native_options = eframe::NativeOptions {
        initial_window_size: Some([1150.0, 720.0].into()),
        min_window_size: Some([1150.0, 720.0].into()),
        icon_data: Some(
            eframe::IconData::try_from_png_bytes(&include_bytes!("../assets/icon-256.png")[..])
                .expect("Failed to load icon"),
        ),
        ..Default::default()
    };
    eframe::run_native(
        "Manual Data cleaner",
        native_options,
        Box::new(|cc| Box::new(ManualDataCleanerApp::new(cc))),
    )
}
