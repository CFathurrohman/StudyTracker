#![windows_subsystem = "windows"]

mod app;
mod cat;
mod glass;
mod scene;
mod mascot;
mod shot;
mod store;
mod sys;
mod theme;
mod timer;
mod ui;

use eframe::egui;

fn main() -> eframe::Result {
    let data = store::load();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("StudyTracker")
            .with_app_id("StudyTracker")
            .with_inner_size([1140.0, 740.0])
            .with_min_inner_size([960.0, 620.0])
            .with_icon(mascot::icon(256)),
        ..Default::default()
    };
    eframe::run_native("StudyTracker", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, data)))))
}
