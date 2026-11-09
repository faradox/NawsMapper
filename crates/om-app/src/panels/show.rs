//! Show mode: the window becomes a small control surface for the
//! performance (what the audience sees, the clock, Blackout, Freeze) and the
//! editor is out of reach, so nothing on the wall changes by accident.

use egui::{Button, Color32, Pos2, Rect, RichText, Ui};

use crate::app::{Action, NawsMapperApp, shortcuts};
use crate::theme;

pub fn show_view(ui: &mut Ui, app: &mut NawsMapperApp) {
    let ctx = ui.ctx().clone();
    egui::Panel::top("show_bar").show(ui, |ui| {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("● SHOW").strong().color(theme::LIVE));
            ui.label(RichText::new(app.doc.display_name()).strong());
            ui.separator();

            let play = if app.transport.playing { "⏸ Pause" } else { "▶ Play" };
            if ui.button(play).clicked() {
                app.act(Action::TogglePlay);
            }
            ui.monospace(format!("{:>7.2}s", app.transport.time));
            ui.separator();

            let black = app.output.blackout;
            if ui
                .add(live_button(if black { "■ Blackout ON" } else { "■ Blackout" }, black))
                .on_hover_text(format!(
                    "Fade the projector to black and back ({})",
                    key(&ctx, &shortcuts::BLACKOUT)
                ))
                .clicked()
            {
                app.act(Action::ToggleBlackout);
            }
            ui.label("Fade");
            ui.add(
                egui::DragValue::new(&mut app.output.fade_secs).range(0.0..=10.0).speed(0.05).suffix(" s"),
            )
            .on_hover_text("How long the fade to black takes. 0 cuts.");
            ui.separator();

            let frozen = app.frozen();
            if ui
                .add(live_button(if frozen { "❄ Frozen" } else { "❄ Freeze" }, frozen))
                .on_hover_text(format!(
                    "Hold the projector on the current frame ({})",
                    key(&ctx, &shortcuts::FREEZE)
                ))
                .clicked()
            {
                app.act(Action::ToggleFreeze);
            }
            ui.separator();
            projector_status(ui, app);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Exit Show Mode").on_hover_text(key(&ctx, &shortcuts::SHOW_MODE)).clicked() {
                    app.act(Action::ToggleShowMode);
                }
            });
        });
        ui.add_space(6.0);
    });

    egui::Panel::bottom("show_status").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(format!("{:.0} fps", app.fps()));
            ui.separator();
            ui.label(
                RichText::new(format!(
                    "Editing is locked. {} blackout · {} freeze · {} exit Show mode",
                    key(&ctx, &shortcuts::BLACKOUT),
                    key(&ctx, &shortcuts::FREEZE),
                    key(&ctx, &shortcuts::SHOW_MODE)
                ))
                .weak(),
            );
        });
    });

    egui::CentralPanel::default().show(ui, |ui| {
        ui.label(RichText::new("ON THE PROJECTOR").small().strong().color(Color32::from_gray(160)));
        let area = ui.available_rect_before_wrap().shrink(8.0);
        let size = app.doc.project.canvas.size();
        let scale = (area.width() / size.x).min(area.height() / size.y).max(0.01);
        let rect = Rect::from_center_size(area.center(), egui::vec2(size.x, size.y) * scale);
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, Color32::BLACK);
        let tint = Color32::from_gray((app.output.level * 255.0).round() as u8);
        painter.image(
            app.projector_texture(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            tint,
        );
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, Color32::from_gray(90)),
            egui::StrokeKind::Outside,
        );
        let mut badges = Vec::new();
        if app.output.blackout || app.output.level < 1.0 {
            badges.push(if app.output.level <= 0.0 { "BLACKOUT" } else { "FADING" });
        }
        if app.frozen() {
            badges.push("FROZEN");
        }
        if !badges.is_empty() {
            painter.text(
                rect.left_top() + egui::vec2(12.0, 10.0),
                egui::Align2::LEFT_TOP,
                badges.join(" · "),
                egui::FontId::proportional(16.0),
                theme::LIVE,
            );
        }
    });
}

/// Where the output is going, with a warning when it isn't pixel-exact.
fn projector_status(ui: &mut Ui, app: &NawsMapperApp) {
    match app.output.projector() {
        Some(p) => {
            let (w, h) = p.size;
            ui.label(format!("📽 {} {w}×{h}", p.name));
            if app.output.size_mismatch(&app.doc.project).is_some() {
                let c = &app.doc.project.canvas;
                ui.colored_label(theme::WARN, format!("⚠ canvas {}×{}: scaled", c.width, c.height))
                    .on_hover_text("Leave Show mode and use Output → Match Canvas to Projector");
            }
        }
        None if app.output.display.is_some() => {
            ui.colored_label(theme::WARN, "⚠ chosen projector not connected");
        }
        None if app.output.fullscreen => {
            ui.label("📽 Output full screen");
        }
        None => {
            ui.colored_label(theme::WARN, "⚠ output is a window")
                .on_hover_text("Pick the projector in Output → Projector, or press F on the output window");
        }
    }
}

/// A banner over the editor canvas while the projector doesn't show the
/// live picture (frozen or blacked out), with the way back.
pub fn projector_banner(ui: &mut Ui, area: Rect, app: &mut NawsMapperApp) {
    let (long, short, action, button) = if app.frozen() {
        (
            "❄ Projector frozen: edits don't show until you unfreeze",
            "❄ Projector frozen",
            Action::ToggleFreeze,
            "Unfreeze",
        )
    } else if app.output.blackout || app.output.level < 1.0 {
        ("■ Projector is black", "■ Projector is black", Action::ToggleBlackout, "Lights On")
    } else {
        return;
    };
    // Fits narrow canvas areas: the short text, or nothing when there's no room.
    let width = (area.width() - 16.0).min(470.0);
    if width < 240.0 {
        return;
    }
    let text = if width >= 470.0 { long } else { short };
    let rect =
        Rect::from_center_size(egui::pos2(area.center().x, area.top() + 22.0), egui::vec2(width, 30.0));
    let painter = ui.painter().with_clip_rect(area);
    painter.rect_filled(rect, 6.0, Color32::from_rgba_unmultiplied(20, 20, 20, 230));
    painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, theme::LIVE), egui::StrokeKind::Inside);
    painter.text(
        rect.left_center() + egui::vec2(12.0, 0.0),
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional(13.0),
        Color32::WHITE,
    );
    let button_rect = Rect::from_min_size(rect.right_top() + egui::vec2(-92.0, 4.0), egui::vec2(84.0, 22.0));
    if ui.put(button_rect, Button::new(button)).clicked() {
        app.act(action);
    }
}

/// A toggle for something the audience sees (Blackout, Freeze): solid red while on.
pub fn live_button(text: &str, active: bool) -> Button<'static> {
    if active {
        Button::new(RichText::new(text).color(Color32::WHITE).strong()).fill(theme::LIVE)
    } else {
        Button::new(text.to_owned())
    }
}

fn key(ctx: &egui::Context, sc: &egui::KeyboardShortcut) -> String {
    ctx.format_shortcut(sc)
}
