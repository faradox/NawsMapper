//! A neutral dark theme in the spirit of Photoshop / pro creative tools:
//! mid-gray panels, a darker workspace, one blue accent.

use egui::{Color32, CornerRadius, Stroke, Visuals};

pub const ACCENT: Color32 = Color32::from_rgb(45, 140, 235);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(120, 180, 240);
pub const WORKSPACE_BG: Color32 = Color32::from_rgb(30, 30, 30);
pub const PANEL_BG: Color32 = Color32::from_rgb(50, 50, 50);
pub const OUTLINE: Color32 = Color32::from_rgba_premultiplied(150, 150, 150, 150);
pub const WARN: Color32 = Color32::from_rgb(240, 170, 60);
/// Outline of cutout surfaces.
pub const CUTOUT: Color32 = Color32::from_rgb(235, 110, 150);

pub fn apply(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.panel_fill = PANEL_BG;
    v.window_fill = Color32::from_rgb(58, 58, 58);
    v.extreme_bg_color = Color32::from_rgb(36, 36, 36);
    v.faint_bg_color = Color32::from_rgb(56, 56, 56);
    v.selection.bg_fill = ACCENT.linear_multiply(0.6);
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.hyperlink_color = ACCENT_SOFT;
    v.window_corner_radius = CornerRadius::same(6);
    v.menu_corner_radius = CornerRadius::same(4);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(3);
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(38, 38, 38));
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(66, 66, 66);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(78, 78, 78);
    v.widgets.active.weak_bg_fill = ACCENT.linear_multiply(0.8);
    ctx.set_visuals(v);

    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 5.0);
        s.spacing.button_padding = egui::vec2(8.0, 3.0);
        s.spacing.slider_width = 140.0;
        s.spacing.interact_size.y = 20.0;
    });
}
