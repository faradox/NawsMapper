//! Shader Library: a floating window with live previews of every generator
//! preset. Click to pick one, double-click (or the buttons) to use it.

use std::time::Instant;

use egui::{Color32, Rect, RichText, Sense, Stroke, StrokeKind};
use om_core::catalog::{CATEGORIES, Preset, presets};

use crate::app::{Action, NawsMapperApp};
use crate::theme;

const TILE: egui::Vec2 = egui::vec2(168.0, 105.0);
const LABEL_HEIGHT: f32 = 20.0;

pub struct Library {
    pub open: bool,
    pub presets: Vec<Preset>,
    category: Option<&'static str>,
    selected: Option<usize>,
    /// Registered egui texture of the preview atlas and the renderer's
    /// atlas generation it shows.
    pub texture: Option<(egui::TextureId, u64)>,
    /// Previews animate on their own clock, also while playback is paused.
    pub clock: Instant,
}

impl Default for Library {
    fn default() -> Self {
        Self {
            open: false,
            presets: presets(),
            category: None,
            selected: None,
            texture: None,
            clock: Instant::now(),
        }
    }
}

pub fn library_window(ctx: &egui::Context, app: &mut NawsMapperApp) {
    if !app.library.open {
        return;
    }
    let texture = app.library_thumbnails();
    let uvs: Vec<Rect> = (0..app.library.presets.len())
        .map(|i| {
            let (min, max) = app.renderer.thumbnail_uv(i);
            Rect::from_min_max(min.into(), max.into())
        })
        .collect();
    let selection = app.editor.selection.surfaces.len();
    let mut action = None;
    let mut open = true;

    egui::Window::new("Shader Library")
        .open(&mut open)
        .default_pos([320.0, 110.0])
        .default_size([760.0, 560.0])
        .collapsible(false)
        .show(ctx, |ui| {
            let lib = &mut app.library;
            ui.horizontal_wrapped(|ui| {
                if ui.selectable_label(lib.category.is_none(), "All").clicked() {
                    lib.category = None;
                }
                for c in CATEGORIES {
                    if ui.selectable_label(lib.category == Some(c), c).clicked() {
                        lib.category = Some(c);
                    }
                }
            });
            ui.separator();

            egui::Panel::bottom("library_actions").show(ui, |ui| {
                ui.add_space(4.0);
                match lib.selected.and_then(|i| lib.presets.get(i).map(|p| (i, p))) {
                    None => {
                        ui.label(
                            RichText::new("Click a shader to pick it. Double-click to use it right away.")
                                .weak(),
                        );
                    }
                    Some((index, p)) => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(p.name).strong());
                            ui.label(RichText::new(p.description).weak());
                        });
                        ui.horizontal(|ui| {
                            let apply = egui::Button::new(format!(
                                "Apply to {selection} selected surface{}",
                                if selection == 1 { "" } else { "s" }
                            ));
                            if ui
                                .add_enabled(selection > 0, apply)
                                .on_disabled_hover_text("Select surfaces on the canvas first")
                                .clicked()
                            {
                                action = Some(Action::UsePreset { index, apply: true });
                            }
                            if ui.button("Add to Sources").clicked() {
                                action = Some(Action::UsePreset { index, apply: false });
                            }
                        });
                        ui.label(
                            RichText::new("Every setting stays editable in the Sources panel afterwards.")
                                .small()
                                .weak(),
                        );
                    }
                }
                ui.add_space(2.0);
            });

            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
                    for (i, p) in lib.presets.iter().enumerate() {
                        if lib.category.is_some_and(|c| c != p.category) {
                            continue;
                        }
                        let (rect, resp) =
                            ui.allocate_exact_size(TILE + egui::vec2(0.0, LABEL_HEIGHT), Sense::click());
                        let image = Rect::from_min_size(rect.min, TILE);
                        let painter = ui.painter();
                        painter.image(texture, image, uvs[i], Color32::WHITE);
                        let stroke = if lib.selected == Some(i) {
                            Stroke::new(2.0, theme::ACCENT)
                        } else if resp.hovered() {
                            Stroke::new(1.0, theme::ACCENT_SOFT)
                        } else {
                            Stroke::new(1.0, Color32::from_gray(30))
                        };
                        painter.rect_stroke(image, 3.0, stroke, StrokeKind::Outside);
                        painter.text(
                            egui::pos2(rect.min.x + 2.0, image.max.y + 4.0),
                            egui::Align2::LEFT_TOP,
                            p.name,
                            egui::FontId::proportional(12.5),
                            if lib.selected == Some(i) { Color32::WHITE } else { Color32::from_gray(200) },
                        );
                        let resp = resp.on_hover_text(format!("{}\n{}", p.description, p.category));
                        if resp.clicked() {
                            lib.selected = Some(i);
                        }
                        if resp.double_clicked() {
                            action = Some(Action::UsePreset { index: i, apply: selection > 0 });
                        }
                    }
                });
            });
        });

    app.library.open = open;
    if let Some(a) = action {
        app.act(a);
    }
    ctx.request_repaint();
}
