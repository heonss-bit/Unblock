// В релизе без чёрного окна консоли.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod bat;
mod engines;
mod scan;
mod settings;
mod sys;
mod tgproxy;
mod tray;
mod ui;
mod zapret;

use eframe::egui;

fn main() -> eframe::Result<()> {
    // Иконка окна: готовые пиксели 64x64 RGBA, без декодирования картинок.
    let icon = egui::IconData {
        rgba: include_bytes!("../assets/icon64.rgba").to_vec(),
        width: 64,
        height: 64,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Unblock")
            .with_inner_size([900.0, 600.0])
            .with_min_inner_size([780.0, 540.0])
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native(
        "Unblock",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
