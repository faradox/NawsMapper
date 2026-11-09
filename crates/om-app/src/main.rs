//! NawsMapper desktop application.

// No console window behind the app in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod backdrop;
mod build_info;
mod cues;
mod dialogs;
mod displays;
mod document;
mod editor;
mod output;
mod package;
mod panels;
mod recovery;
mod template;
mod theme;
mod transform;
mod widgets;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn,nawsmapper=info,om_render=info"),
    )
    .init();
    log::info!("NawsMapper {}", build_info::version_label());

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("NawsMapper")
            .with_app_id(app::APP_ID)
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 600.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native("NawsMapper", options, Box::new(|cc| Ok(Box::new(app::NawsMapperApp::new(cc)?))))
}
