//! Docked panels around the canvas.

mod chrome;
mod layers;
mod library;
mod properties;
mod sources;

pub use chrome::{menu_bar, shape_icon, status_bar, toolbar};
pub use library::{Library, library_window};
pub use properties::properties_panel;

use crate::app::NawsMapperApp;

/// Left dock: Layers (surfaces) on top, Sources (media library) below.
pub fn left_panel(ui: &mut egui::Ui, app: &mut NawsMapperApp) {
    egui::Panel::left("left_dock").default_size(250.0).min_size(200.0).show(ui, |ui| {
        egui::Panel::bottom("sources_dock").resizable(true).default_size(330.0).min_size(160.0).show(
            ui,
            |ui| {
                sources::sources_panel(ui, app);
            },
        );
        egui::CentralPanel::default_margins().show(ui, |ui| {
            layers::layers_panel(ui, app);
        });
    });
}

/// Panel title row in the style of creative-tool palettes.
fn panel_header(ui: &mut egui::Ui, title: &str) {
    ui.add_space(2.0);
    ui.label(egui::RichText::new(title.to_uppercase()).small().strong().color(egui::Color32::from_gray(180)));
    ui.separator();
}
