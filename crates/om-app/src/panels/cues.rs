//! The cue list: record looks, play them in order with GO.
//! The same cards appear in Show mode (without editing).

use std::time::Instant;

use egui::{Button, Color32, CornerRadius, RichText, Sense, Stroke, StrokeKind, Ui};
use om_core::CueId;

use crate::app::{Action, NawsMapperApp};
use crate::theme;

const CARD: egui::Vec2 = egui::vec2(150.0, 52.0);

/// Bottom dock under the canvas.
pub fn cues_panel(ui: &mut Ui, app: &mut NawsMapperApp) {
    egui::Panel::bottom("cues").resizable(false).show(ui, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("CUES").small().strong().color(Color32::from_gray(180)));
            ui.add_space(8.0);
            if ui
                .button(RichText::new("● Record").color(theme::LIVE))
                .on_hover_text("Remember how all surfaces look now as a new cue")
                .clicked()
            {
                app.act(Action::RecordCue);
            }
            go_button(ui, app);
            if app.cues.progress(Instant::now()).is_some()
                && ui.button("Stop").on_hover_text("Finish the fade now").clicked()
            {
                app.act(Action::StopCue);
            }
            status(ui, app);
        });
        ui.add_space(2.0);
        if app.doc.project.cues.is_empty() {
            ui.label(
                RichText::new(
                    "Cues remember how all surfaces look (sources, colors, opacity, animation, visibility), \
                     not where they are. Set up a look, click ● Record, set up the next one, Record again. \
                     GO plays them in order with a crossfade.",
                )
                .weak(),
            );
        } else {
            cue_cards(ui, app, true);
            settings(ui, app);
        }
        ui.add_space(4.0);
    });
}

/// "▶ GO: 3 Blue wave": plays the next cue.
pub fn go_button(ui: &mut Ui, app: &mut NawsMapperApp) {
    let next = app.cues.next(&app.doc.project);
    let label = match next.and_then(|id| app.doc.project.cue_index(id)) {
        Some(i) => format!("▶ GO: {} {}", i + 1, app.doc.project.cues[i].name),
        None => "▶ GO".to_string(),
    };
    let button = Button::new(RichText::new(label).strong().color(Color32::WHITE)).fill(theme::ACCENT);
    if ui
        .add_enabled(next.is_some(), button)
        .on_hover_text("Play the next cue (Space in Show mode)")
        .on_disabled_hover_text("No cue to play next")
        .clicked()
    {
        app.act(Action::Go);
    }
}

/// Live cue, fade progress, auto-follow countdown.
fn status(ui: &mut Ui, app: &NawsMapperApp) {
    let now = Instant::now();
    let project = &app.doc.project;
    if let Some(i) = app.cues.live.and_then(|id| project.cue_index(id)) {
        ui.separator();
        ui.label(RichText::new(format!("Live: {} {}", i + 1, project.cues[i].name)).color(theme::LIVE));
        if let Some(t) = app.cues.progress(now) {
            ui.add(egui::ProgressBar::new(t).desired_width(80.0).text("fading"));
        }
        if let Some(s) = app.cues.follow_in(now) {
            ui.label(RichText::new(format!("next in {s:.1} s")).weak());
        }
    }
}

/// The cards, left to right. `editable`: context menu with Update, Move, Delete.
pub fn cue_cards(ui: &mut Ui, app: &mut NawsMapperApp, editable: bool) {
    let next = app.cues.next(&app.doc.project);
    let mut actions = Vec::new();
    egui::ScrollArea::horizontal().id_salt(("cue_cards", editable)).show(ui, |ui| {
        ui.horizontal(|ui| {
            for (i, cue) in app.doc.project.cues.iter().enumerate() {
                let live = app.cues.live == Some(cue.id);
                let changed = live && !cue.matches(&app.doc.project);
                let (rect, resp) = ui.allocate_exact_size(CARD, Sense::click());
                let painter = ui.painter();
                let fill = if resp.hovered() { Color32::from_gray(70) } else { Color32::from_gray(56) };
                painter.rect_filled(rect, CornerRadius::same(5), fill);
                let (stroke, tag) = if live {
                    (Stroke::new(2.0, theme::LIVE), Some(("LIVE", theme::LIVE)))
                } else if next == Some(cue.id) {
                    (Stroke::new(2.0, theme::ACCENT), Some(("NEXT", theme::ACCENT)))
                } else {
                    (Stroke::new(1.0, Color32::from_gray(80)), None)
                };
                painter.rect_stroke(rect, CornerRadius::same(5), stroke, StrokeKind::Inside);
                let left = rect.left_top() + egui::vec2(8.0, 6.0);
                painter.text(
                    left,
                    egui::Align2::LEFT_TOP,
                    format!("{}  {}", i + 1, cue.name),
                    egui::FontId::proportional(13.0),
                    Color32::WHITE,
                );
                let timing = match cue.follow {
                    Some(f) => format!("fade {:.1} s · then {f:.1} s", cue.fade),
                    None => format!("fade {:.1} s", cue.fade),
                };
                painter.text(
                    rect.left_bottom() + egui::vec2(8.0, -6.0),
                    egui::Align2::LEFT_BOTTOM,
                    timing,
                    egui::FontId::proportional(11.0),
                    Color32::from_gray(170),
                );
                if let Some((text, color)) = tag {
                    let text = if changed { format!("{text} ●") } else { text.to_string() };
                    painter.text(
                        rect.right_top() + egui::vec2(-8.0, 6.0),
                        egui::Align2::RIGHT_TOP,
                        text,
                        egui::FontId::proportional(10.0),
                        color,
                    );
                }
                let hint = if changed {
                    "The look changed since this cue was played. Right-click → Update to keep the change."
                } else {
                    "Click: play this one next · double-click: play now"
                };
                let resp = resp.on_hover_text(hint);
                if resp.double_clicked() {
                    actions.push(Action::GoTo(cue.id));
                } else if resp.clicked() {
                    actions.push(Action::SelectCue(Some(cue.id)));
                }
                if editable {
                    resp.context_menu(|ui| {
                        if ui.button("▶ Play Now").clicked() {
                            actions.push(Action::GoTo(cue.id));
                        }
                        if ui.button("Update with Current Look").clicked() {
                            actions.push(Action::UpdateCue(cue.id));
                        }
                        ui.separator();
                        if ui.button("Move Earlier").clicked() {
                            actions.push(Action::MoveCue(cue.id, -1));
                        }
                        if ui.button("Move Later").clicked() {
                            actions.push(Action::MoveCue(cue.id, 1));
                        }
                        ui.separator();
                        if ui.button("Delete Cue").clicked() {
                            actions.push(Action::DeleteCue(cue.id));
                        }
                    });
                }
            }
        });
    });
    for a in actions {
        app.act(a);
    }
}

/// Name, fade and auto-follow of the cue in focus: the one chosen in the
/// list, else the live one.
fn settings(ui: &mut Ui, app: &mut NawsMapperApp) {
    let focus: Option<CueId> = app.cues.selected.or(app.cues.live);
    let Some(i) = focus.and_then(|id| app.doc.project.cue_index(id)) else {
        ui.label(RichText::new("Click a cue to change its name and timing.").weak());
        return;
    };
    let cue = &mut app.doc.project.cues[i];
    ui.horizontal(|ui| {
        ui.label(format!("Cue {}", i + 1));
        ui.add(egui::TextEdit::singleline(&mut cue.name).desired_width(160.0));
        ui.separator();
        ui.label("Fade");
        ui.add(egui::DragValue::new(&mut cue.fade).range(0.0..=60.0).speed(0.05).suffix(" s"))
            .on_hover_text("Crossfade time when this cue plays. 0 cuts.");
        ui.separator();
        let mut auto = cue.follow.is_some();
        if ui
            .checkbox(&mut auto, "Then play the next cue after")
            .on_hover_text("Auto-follow: no GO needed for the next cue")
            .changed()
        {
            cue.follow = auto.then_some(cue.fade.max(5.0));
        }
        if let Some(f) = &mut cue.follow {
            ui.add(egui::DragValue::new(f).range(0.0..=3600.0).speed(0.1).suffix(" s"))
                .on_hover_text("Counted from the moment this cue starts");
        }
    });
}
