//! Menu bar, toolbar and status bar.

use egui::{Button, Color32, Pos2, RichText, Sense, Stroke, Ui};
use om_core::shape::outline_uv;
use om_core::{Mask, PathPoint};

use crate::app::{Action, NawsMapperApp, shortcuts};
use crate::theme;

fn item(ui: &mut Ui, label: &str, shortcut: Option<&egui::KeyboardShortcut>, enabled: bool) -> bool {
    let mut b = Button::new(label);
    if let Some(sc) = shortcut {
        b = b.shortcut_text(ui.ctx().format_shortcut(sc));
    }
    ui.add_enabled(enabled, b).clicked()
}

pub fn menu_bar(ui: &mut Ui, app: &mut NawsMapperApp) {
    egui::Panel::top("menu_bar").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            let has_sel = !app.editor.selection.surfaces.is_empty();
            ui.menu_button("File", |ui| {
                if item(ui, "New Demo Project", Some(&shortcuts::NEW), true) {
                    app.act(Action::NewDemo);
                }
                if item(ui, "New Empty Project", None, true) {
                    app.act(Action::NewEmpty);
                }
                if item(ui, "Open…", Some(&shortcuts::OPEN), true) {
                    app.act(Action::Open);
                }
                ui.separator();
                if item(ui, "Save", Some(&shortcuts::SAVE), true) {
                    app.act(Action::Save);
                }
                if item(ui, "Save As…", Some(&shortcuts::SAVE_AS), true) {
                    app.act(Action::SaveAs);
                }
                ui.separator();
                if item(ui, "Import Images & Videos…", None, true) {
                    app.act(Action::ImportMedia);
                }
                ui.separator();
                if item(ui, "Quit", None, true) {
                    app.act(Action::Quit);
                }
            });
            ui.menu_button("Edit", |ui| {
                if item(ui, "Undo", Some(&shortcuts::UNDO), app.doc.can_undo()) {
                    app.act(Action::Undo);
                }
                if item(ui, "Redo", Some(&shortcuts::REDO), app.doc.can_redo()) {
                    app.act(Action::Redo);
                }
                ui.separator();
                if item(ui, "Select All", Some(&shortcuts::SELECT_ALL), true) {
                    app.act(Action::SelectAll);
                }
                if item(ui, "Deselect", None, has_sel) {
                    app.act(Action::Deselect);
                }
                ui.separator();
                if item(ui, "Duplicate", Some(&shortcuts::DUPLICATE), has_sel) {
                    app.act(Action::Duplicate);
                }
                if item(ui, "Delete", None, has_sel) {
                    app.act(Action::Delete);
                }
            });
            ui.menu_button("Surface", |ui| {
                if item(ui, "Add Quad", Some(&shortcuts::ADD_QUAD), true) {
                    app.act(Action::AddSurface(Mask::Quad));
                }
                if item(ui, "Add Ellipse", None, true) {
                    app.act(Action::AddSurface(Mask::Ellipse));
                }
                if item(ui, "Add Triangle", None, true) {
                    app.act(Action::AddSurface(Mask::Triangle));
                }
                ui.menu_button("Add Shape", |ui| {
                    for m in Mask::SHAPES.into_iter().skip(3) {
                        if item(ui, m.label(), None, true) {
                            app.act(Action::AddSurface(m));
                        }
                    }
                });
                if item(ui, "Add Cutout", None, true) {
                    app.act(Action::AddCutout);
                }
                ui.separator();
                if item(ui, "Pen: Draw a Shape", Some(&shortcuts::PEN), true) {
                    app.act(Action::Pen);
                }
                if item(ui, "Edit Points", None, app.editor.selection.surfaces.len() == 1) {
                    app.act(Action::EditPoints);
                }
                ui.separator();
                let label = if app.editor.transform.is_some() { "Apply Transform" } else { "Free Transform" };
                if item(ui, label, Some(&shortcuts::FREE_TRANSFORM), has_sel) {
                    app.act(Action::FreeTransform);
                }
                ui.separator();
                if item(ui, "Bring to Front", Some(&shortcuts::TO_FRONT), has_sel) {
                    app.act(Action::RestackExtreme { front: true });
                }
                if item(ui, "Bring Forward", Some(&shortcuts::FORWARD), has_sel) {
                    app.act(Action::Restack(1));
                }
                if item(ui, "Send Backward", Some(&shortcuts::BACKWARD), has_sel) {
                    app.act(Action::Restack(-1));
                }
                if item(ui, "Send to Back", Some(&shortcuts::TO_BACK), has_sel) {
                    app.act(Action::RestackExtreme { front: false });
                }
                ui.separator();
                if item(ui, "Stagger Animation (chase)", None, app.editor.selection.surfaces.len() > 1) {
                    app.act(Action::Stagger);
                }
            });
            ui.menu_button("View", |ui| {
                if item(ui, "Fit Canvas", Some(&shortcuts::ZOOM_FIT), true) {
                    app.act(Action::ZoomFit);
                }
                if item(ui, "Actual Pixels", Some(&shortcuts::ZOOM_100), true) {
                    app.act(Action::Zoom(1.0));
                }
                if item(ui, "Zoom In", Some(&shortcuts::ZOOM_IN), true) {
                    app.act(Action::ZoomBy(1.25));
                }
                if item(ui, "Zoom Out", Some(&shortcuts::ZOOM_OUT), true) {
                    app.act(Action::ZoomBy(0.8));
                }
                ui.separator();
                ui.checkbox(&mut app.editor.show_outlines, "Show Outlines in Editor");
                ui.checkbox(&mut app.editor.snapping, "Snap to Corners");
                ui.separator();
                if item(ui, "Shader Library", Some(&shortcuts::LIBRARY), true) {
                    app.act(Action::ToggleLibrary);
                }
            });
            ui.menu_button("Output", |ui| {
                let label = if app.output.open { "Close Output Window" } else { "Open Output Window" };
                if item(ui, label, Some(&shortcuts::OUTPUT), true) {
                    app.act(Action::ToggleOutput);
                }
                ui.checkbox(&mut app.output.show_outlines, "Show Outlines on Projector");
            });
            ui.menu_button("Help", |ui| {
                ui.label(RichText::new("NawsMapper").strong());
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                ui.separator();
                ui.label("Space — play / pause (hold + drag to pan)");
                ui.label("Q — add quad · P — pen · Arrows — nudge (Shift ×10)");
                ui.label("Double-click a free-form shape — edit its points");
                ui.label("Alt/Option while dragging — no snap, unweld corners");
                ui.label("Shift/Cmd-click — add to selection");
            });
        });
    });
}

pub fn toolbar(ui: &mut Ui, app: &mut NawsMapperApp) {
    egui::Panel::top("toolbar").show(ui, |ui| {
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Add").weak());
            if ui.button("▭ Quad").on_hover_text("Add a four-corner surface (Q)").clicked() {
                app.act(Action::AddSurface(Mask::Quad));
            }
            if ui.button("◯ Ellipse").clicked() {
                app.act(Action::AddSurface(Mask::Ellipse));
            }
            ui.menu_button("◆ Shapes", |ui| {
                ui.label(RichText::new("Add a shape").weak());
                egui::Grid::new("shape_picker").spacing([4.0, 4.0]).show(ui, |ui| {
                    for (i, m) in Mask::SHAPES.into_iter().enumerate() {
                        if shape_tile(ui, m).clicked() {
                            app.act(Action::AddSurface(m));
                            ui.close();
                        }
                        if i % 5 == 4 {
                            ui.end_row();
                        }
                    }
                });
                ui.separator();
                ui.label(
                    RichText::new("Or draw your own with the Pen (P).\nAny shape: Properties → Edit Points.")
                        .weak(),
                );
            })
            .response
            .on_hover_text("More shapes: star, heart, hexagon, arrow…");
            let pen_on = app.editor.pen.is_some();
            if ui
                .add(Button::new("✒ Pen").selected(pen_on))
                .on_hover_text("Draw a free-form shape point by point (P)")
                .clicked()
            {
                app.act(Action::Pen);
            }
            if ui
                .button("✂ Cutout")
                .on_hover_text("Add a shape that cuts a hole in the surfaces below it")
                .clicked()
            {
                app.act(Action::AddCutout);
            }
            ui.separator();

            if let Some(tf) = &app.editor.transform {
                let (scale, angle) = tf.readout();
                ui.label(RichText::new("Transform").strong());
                ui.monospace(format!(
                    "W {:>4.0}%  H {:>4.0}%  ∠ {:>6.1}°",
                    scale.x * 100.0,
                    scale.y * 100.0,
                    angle
                ));
                if ui
                    .add(Button::new(RichText::new("Apply").color(theme::ACCENT)))
                    .on_hover_text("Enter")
                    .clicked()
                {
                    app.act(Action::ApplyTransform);
                }
                if ui.button("Cancel").on_hover_text("Esc").clicked() {
                    app.act(Action::CancelTransform);
                }
            } else {
                let has_sel = !app.editor.selection.surfaces.is_empty();
                if ui
                    .add_enabled(has_sel, Button::new("⬚ Transform"))
                    .on_hover_text("Free Transform: scale and rotate the selection (Cmd+T)")
                    .clicked()
                {
                    app.act(Action::FreeTransform);
                }
            }
            ui.separator();

            let play = if app.transport.playing { "⏸ Pause" } else { "▶ Play" };
            if ui.button(play).on_hover_text("Space").clicked() {
                app.act(Action::TogglePlay);
            }
            if ui.button("⏮").on_hover_text("Restart animation clock").clicked() {
                app.act(Action::Restart);
            }
            ui.monospace(format!("{:>7.2}s", app.transport.time));
            ui.separator();

            ui.toggle_value(&mut app.editor.snapping, "Snap")
                .on_hover_text("Snap corners to other corners (hold Alt to bypass)");
            ui.toggle_value(&mut app.editor.show_outlines, "Outlines");
            ui.separator();

            let (label, color) = if app.output.open {
                ("📽 Output: ON", theme::ACCENT)
            } else {
                ("📽 Output: OFF", ui.visuals().text_color())
            };
            if ui
                .add(Button::new(RichText::new(label).color(color)))
                .on_hover_text("Open the projector window (Cmd+Enter)")
                .clicked()
            {
                app.act(Action::ToggleOutput);
            }
            if app.output.open {
                ui.checkbox(&mut app.output.show_outlines, "Outlines on projector");
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(Button::new("✨ Shaders").selected(app.library.open))
                    .on_hover_text("Shader Library: animated patterns to put on surfaces (Cmd+L)")
                    .clicked()
                {
                    app.act(Action::ToggleLibrary);
                }
                if ui.button("+ Image / Video").on_hover_text("Import images and MP4 videos").clicked() {
                    app.act(Action::ImportMedia);
                }
            });
        });
        ui.add_space(2.0);
    });
}

pub fn status_bar(ui: &mut Ui, app: &mut NawsMapperApp) {
    egui::Panel::bottom("status_bar").show(ui, |ui| {
        ui.horizontal(|ui| {
            let p = &app.doc.project;
            ui.label(format!("Canvas {}×{}", p.canvas.width, p.canvas.height));
            ui.separator();
            ui.label(format!("Zoom {:.0}%", app.editor.zoom * 100.0));
            ui.separator();
            ui.label(format!(
                "{} surfaces · {} selected",
                p.surfaces.len(),
                app.editor.selection.surfaces.len()
            ));
            ui.separator();
            ui.label(format!("{:.0} fps", app.fps()));
            if let Some(msg) = app.status() {
                ui.separator();
                ui.label(RichText::new(msg).color(theme::ACCENT_SOFT));
            }
        });
    });
}

/// Outline drawing of a shape (`path` for free-form ones), fitted into a
/// `size`-sized square.
pub fn shape_icon(ui: &mut Ui, mask: Mask, path: &[PathPoint], size: f32, color: Color32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), Sense::hover());
    paint_shape(ui.painter(), rect.shrink(2.0), mask, path, color);
    resp
}

fn paint_shape(painter: &egui::Painter, rect: egui::Rect, mask: Mask, path: &[PathPoint], color: Color32) {
    let pts: Vec<Pos2> =
        outline_uv(mask, path).iter().map(|p| rect.min + egui::vec2(p.x, p.y) * rect.size()).collect();
    painter.add(egui::Shape::closed_line(pts, Stroke::new(1.5, color)));
}

/// A clickable icon + name cell for the shape picker.
fn shape_tile(ui: &mut Ui, mask: Mask) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(64.0, 58.0), Sense::click());
    let visuals = ui.style().interact(&resp);
    if resp.hovered() {
        ui.painter().rect_filled(rect, 4.0, visuals.weak_bg_fill);
    }
    let icon = egui::Rect::from_center_size(rect.center() - egui::vec2(0.0, 8.0), egui::vec2(30.0, 30.0));
    paint_shape(ui.painter(), icon, mask, &[], visuals.text_color());
    ui.painter().text(
        egui::pos2(rect.center().x, rect.max.y - 4.0),
        egui::Align2::CENTER_BOTTOM,
        mask.label(),
        egui::FontId::proportional(11.0),
        visuals.text_color(),
    );
    resp
}
