//! Small reusable UI helpers.

use egui::{Color32, Response, Ui};
use om_core::Color;

/// Color swatch + picker for an unmultiplied sRGB `[f32; 4]`.
pub fn color_edit(ui: &mut Ui, color: &mut Color) -> Response {
    let mut c = to_u8(*color);
    let r = ui.color_edit_button_srgba_unmultiplied(&mut c);
    if r.changed() {
        *color = c.map(|v| v as f32 / 255.0);
    }
    r
}

pub fn to_u8(c: Color) -> [u8; 4] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

pub fn to_color32(c: Color) -> Color32 {
    let [r, g, b, a] = to_u8(c);
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// A two-column property grid with consistent spacing.
pub fn grid(ui: &mut Ui, id: &str, add: impl FnOnce(&mut Ui)) {
    egui::Grid::new(id).num_columns(2).spacing([10.0, 6.0]).min_col_width(70.0).show(ui, add);
}

/// A section header that remembers its open state.
pub fn section(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(egui::RichText::new(title).strong()).default_open(true).show(ui, add);
}

/// A drag value for a 2D vector (`x`/`y` side by side).
pub fn vec2_drag(ui: &mut Ui, v: &mut glam::Vec2, speed: f32, suffix: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        changed |= ui.add(egui::DragValue::new(&mut v.x).speed(speed).prefix("x ").suffix(suffix)).changed();
        changed |= ui.add(egui::DragValue::new(&mut v.y).speed(speed).prefix("y ").suffix(suffix)).changed();
    });
    changed
}

/// Plain label with a hover hint, used for property names.
pub fn prop_label(ui: &mut Ui, text: &str, hint: &str) {
    let r = ui.label(text);
    if !hint.is_empty() {
        r.on_hover_text(hint);
    }
}
