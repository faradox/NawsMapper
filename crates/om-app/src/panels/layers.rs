//! Layers panel: the surface stack, front-most on top (like Photoshop).

use egui::{Button, RichText, Ui};
use om_core::SurfaceId;

use super::panel_header;
use crate::app::{Action, NawsMapperApp};
use crate::theme;

pub fn layers_panel(ui: &mut Ui, app: &mut NawsMapperApp) {
    panel_header(ui, "Layers");

    ui.horizontal(|ui| {
        let has_sel = !app.editor.selection.surfaces.is_empty();
        if ui.small_button("+").on_hover_text("Add quad (Q)").clicked() {
            app.act(Action::AddSurface(om_core::Mask::Quad));
        }
        if ui.add_enabled(has_sel, Button::new("⧉").small()).on_hover_text("Duplicate (Cmd+D)").clicked() {
            app.act(Action::Duplicate);
        }
        if ui.add_enabled(has_sel, Button::new("⬆").small()).on_hover_text("Bring forward (Cmd+])").clicked()
        {
            app.act(Action::Restack(1));
        }
        if ui.add_enabled(has_sel, Button::new("⬇").small()).on_hover_text("Send backward (Cmd+[)").clicked()
        {
            app.act(Action::Restack(-1));
        }
        if ui.add_enabled(has_sel, Button::new("🗑").small()).on_hover_text("Delete (Del)").clicked() {
            app.act(Action::Delete);
        }
    });
    ui.add_space(2.0);

    if app.doc.project.surfaces.is_empty() {
        ui.add_space(12.0);
        ui.label(RichText::new("No surfaces yet.\nPress Q or click + to add one.").weak());
        return;
    }

    let order: Vec<SurfaceId> = app.doc.project.surfaces.iter().rev().map(|s| s.id).collect();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for id in order.iter().copied() {
            let Some(idx) = app.doc.project.surface_index(id) else { continue };
            let selected = app.editor.selection.contains(id);
            let source_name = app.doc.project.surfaces[idx]
                .source
                .and_then(|sid| app.doc.project.source(sid))
                .map(|s| s.name.clone())
                .unwrap_or_else(|| "no source".into());

            let s = &mut app.doc.project.surfaces[idx];
            let row = ui.horizontal(|ui| {
                let eye = if s.visible { "👁" } else { "  " };
                if ui.add(Button::new(eye).small().frame(false)).on_hover_text("Show / hide").clicked() {
                    s.visible = !s.visible;
                }
                let lock = if s.locked { "🔒" } else { "🔓" };
                let lock_text = if s.locked { RichText::new(lock) } else { RichText::new(lock).weak() };
                if ui
                    .add(Button::new(lock_text).small().frame(false))
                    .on_hover_text("Lock / unlock")
                    .clicked()
                {
                    s.locked = !s.locked;
                }
                let mut text = RichText::new(&s.name);
                if !s.visible {
                    text = text.weak();
                }
                if selected {
                    text = text.color(egui::Color32::WHITE);
                }
                let kind = if s.cutout { "✂ Cutout" } else { s.mask.label() };
                let hint = RichText::new(kind).small().weak();
                let label = ui.add(
                    Button::selectable(selected, text)
                        .right_text(hint)
                        .truncate()
                        .min_size(egui::vec2(ui.available_width() - 4.0, 0.0)),
                );
                if s.cutout {
                    label.on_hover_text(format!("{} cutout: hides the layers below it", s.mask.label()))
                } else {
                    label.on_hover_text(format!("{} · {}", s.mask.label(), source_name))
                }
            });
            let resp = row.inner;
            if resp.clicked() {
                let mods = ui.input(|i| i.modifiers);
                select_with_modifiers(app, &order, id, mods);
            }
            if selected {
                ui.painter().rect_stroke(
                    row.response.rect.expand(1.0),
                    3.0,
                    egui::Stroke::new(1.0, theme::ACCENT),
                    egui::StrokeKind::Inside,
                );
            }
        }
    });
}

/// Photoshop-style list selection: Cmd toggles, Shift selects a range.
fn select_with_modifiers(app: &mut NawsMapperApp, order: &[SurfaceId], id: SurfaceId, mods: egui::Modifiers) {
    let sel = &mut app.editor.selection;
    if mods.command {
        sel.toggle(id);
    } else if mods.shift
        && let Some(anchor) = sel.primary()
    {
        let a = order.iter().position(|x| *x == anchor).unwrap_or(0);
        let b = order.iter().position(|x| *x == id).unwrap_or(0);
        let (lo, hi) = (a.min(b), a.max(b));
        let mut ids: Vec<SurfaceId> = order[lo..=hi].to_vec();
        // Keep the clicked item as the primary selection.
        ids.retain(|x| *x != id);
        ids.push(id);
        sel.set(ids);
    } else {
        sel.set(vec![id]);
    }
}
