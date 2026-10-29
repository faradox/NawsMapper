//! NawsMapper desktop application.

// No console window behind the app in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod dialogs;
mod document;
mod editor;
mod output;
mod panels;
mod theme;
mod transform;
mod widgets;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn,om_app=info,om_render=info"),
    )
    .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("NawsMapper")
            .with_app_id("net.nawsmapper.NawsMapper")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 600.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native("NawsMapper", options, Box::new(|cc| Ok(Box::new(app::NawsMapperApp::new(cc)?))))
}
