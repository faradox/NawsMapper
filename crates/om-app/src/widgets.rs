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

/// A combo box that never makes its panel wider: long text (a media file
/// name, say) is cut off with "…" instead of pushing the panel open.
pub fn combo<'a>(ui: &Ui, id: &str, text: impl egui::IntoAtoms<'a>) -> egui::ComboBox<'a> {
    egui::ComboBox::from_id_salt(id)
        .selected_text(text)
        .truncate()
        .width(ui.available_width().clamp(60.0, 180.0))
}

/// A two-column property grid with consistent spacing.
pub fn grid(ui: &mut Ui, id: &str, add: impl FnOnce(&mut Ui)) {
    egui::Grid::new(id).num_columns(2).spacing([10.0, 6.0]).min_col_width(70.0).show(ui, add);
}

/// A section header that remembers its open state.
pub fn section(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(egui::RichText::new(title).strong()).default_open(true).show(ui, add);
}

/// A slider that fits the space it has, so panels can be made narrow. The
/// rail leaves room for the value box as measured last frame: if the row
/// came out wider than the space, the panel would grow a little every frame.
pub fn fit_slider(ui: &mut Ui, slider: egui::Slider<'_>) -> Response {
    let id = ui.next_auto_id().with("fit_slider");
    let value_w: f32 = ui.data(|d| d.get_temp(id)).unwrap_or(100.0);
    let rail = (ui.available_width() - value_w).clamp(30.0, 200.0);
    let r = ui
        .scope(|ui| {
            ui.spacing_mut().slider_width = rail;
            ui.add(slider)
        })
        .inner;
    ui.data_mut(|d| d.insert_temp(id, (r.rect.width() - rail).max(0.0) + 1.0));
    r
}

/// A drag value for a 2D vector (`x`/`y` side by side, stacked when narrow).
pub fn vec2_drag(ui: &mut Ui, v: &mut glam::Vec2, speed: f32, suffix: &str) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
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
