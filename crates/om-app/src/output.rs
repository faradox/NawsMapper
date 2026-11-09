//! The projector output: a second native window that shows the composed
//! canvas edge-to-edge. Pick the projector in Output → Projector and the
//! window goes full screen there; or drag it onto the projector's screen
//! and press `F` (or double-click).
//!
//! Blackout is a fade of this window's image only: the rendered texture (and
//! with it the editor preview) is untouched.

use std::time::Instant;

use egui::{Color32, Key, Pos2, Rect, Sense, Stroke, ViewportBuilder, ViewportCommand, ViewportId};
use glam::Vec2;
use om_core::Project;

use crate::displays::{Display, DisplayChoice};
use crate::editor::{Selection, draw_output_overlays};

/// How often the list of connected displays is refreshed.
const DISPLAY_POLL_SECS: f32 = 1.0;

pub struct OutputWindow {
    pub open: bool,
    /// Draw surface outlines and selected handles on the projector,
    /// which makes aligning corners on the real object much easier.
    pub show_outlines: bool,
    pub fullscreen: bool,
    /// Draw a mouse pointer on the projector where the pointer is on the
    /// editor canvas, so you can trace the real object while looking at it.
    pub show_cursor: bool,
    /// The projector the output goes full screen on; `None` is a normal window.
    pub display: Option<DisplayChoice>,
    /// Connected displays, refreshed about once a second.
    pub displays: Vec<Display>,
    displays_checked: Option<Instant>,
    /// Index of the display the window was last sent to.
    placed_on: Option<usize>,
    /// Blackout: the projector fades to black (and back when turned off).
    pub blackout: bool,
    /// Seconds a blackout fade takes (0 = cut).
    pub fade_secs: f32,
    /// Current brightness of the projected image, 0..1.
    pub level: f32,
    /// Last OS cursor visibility sent to the window.
    cursor_visible: Option<bool>,
}

impl Default for OutputWindow {
    fn default() -> Self {
        Self {
            open: false,
            show_outlines: true,
            fullscreen: false,
            show_cursor: false,
            display: None,
            displays: Vec::new(),
            displays_checked: None,
            placed_on: None,
            blackout: false,
            fade_secs: 1.0,
            level: 1.0,
            cursor_visible: None,
        }
    }
}

/// What the output window shows this frame.
pub struct OutputFrame<'a> {
    /// The live output, or the frozen copy.
    pub texture: egui::TextureId,
    pub project: &'a Project,
    pub selection: &'a Selection,
    pub pointer: Option<Vec2>,
    /// Show mode: no overlays, no cursor, keys and close requests ignored.
    pub show_mode: bool,
    /// The projector holds a still frame: no live overlays either, or the
    /// audience would see the edits the freeze is hiding.
    pub frozen: bool,
}

impl OutputWindow {
    fn id() -> ViewportId {
        ViewportId::from_hash_of("om_output")
    }

    /// Refreshes the display list now and then (projectors get plugged in
    /// while the app runs).
    pub fn poll_displays(&mut self, frame: &eframe::Frame) {
        let due = self.displays_checked.is_none_or(|t| t.elapsed().as_secs_f32() >= DISPLAY_POLL_SECS);
        if due {
            self.displays = crate::displays::connected(frame);
            self.displays_checked = Some(Instant::now());
        }
    }

    /// The chosen projector, if it is connected.
    pub fn projector(&self) -> Option<&Display> {
        self.display.as_ref()?.resolve(&self.displays).map(|i| &self.displays[i])
    }

    /// Native size of the chosen projector when it differs from the canvas:
    /// the output is then scaled and not pixel-exact.
    pub fn size_mismatch(&self, project: &Project) -> Option<(u32, u32)> {
        let size = self.projector()?.size;
        (size != (project.canvas.width, project.canvas.height)).then_some(size)
    }

    pub fn choose_display(&mut self, index: Option<usize>) {
        self.display = index.and_then(|i| DisplayChoice::of(&self.displays, i));
        if self.display.is_some() {
            self.open = true;
            // Picking it again also puts the window back after Esc.
            self.placed_on = None;
        }
    }

    /// Sends the output full screen to the chosen projector again, e.g. after
    /// someone pressed Esc on it: Show mode always starts on the projector.
    pub fn replace_on_projector(&mut self) {
        self.placed_on = None;
    }

    /// Moves the brightness toward the blackout target. Returns true while fading.
    pub fn step_fade(&mut self, dt: f32) -> bool {
        let target = if self.blackout { 0.0 } else { 1.0 };
        self.level = approach(self.level, target, dt, self.fade_secs);
        self.level != target
    }

    pub fn show(&mut self, ctx: &egui::Context, out: OutputFrame<'_>) {
        if !self.open {
            self.cursor_visible = None;
            self.placed_on = None;
            return;
        }
        // Send the window to the chosen projector when the choice (or the
        // display's position in the list) changes.
        let target = self.display.as_ref().and_then(|c| c.resolve(&self.displays));
        if target != self.placed_on {
            match target {
                Some(i) => ctx.send_viewport_cmd_to(Self::id(), ViewportCommand::SetMonitor(i)),
                None => ctx.send_viewport_cmd_to(Self::id(), ViewportCommand::Fullscreen(false)),
            }
            self.placed_on = target;
        }

        let builder =
            ViewportBuilder::default().with_title("NawsMapper Output").with_inner_size([960.0, 540.0]);
        let level = self.level;
        ctx.show_viewport_immediate(Self::id(), builder, |ui, _class| {
            let (close, fullscreen) =
                ui.input(|i| (i.viewport().close_requested(), i.viewport().fullscreen.unwrap_or(false)));
            if close && !out.show_mode {
                self.open = false;
            }
            self.fullscreen = fullscreen;

            let rect = ui.max_rect();
            let resp = ui.allocate_rect(rect, Sense::click());
            let painter = ui.painter();
            painter.rect_filled(rect, 0.0, Color32::BLACK);
            let tint = Color32::from_gray((level * 255.0).round() as u8);
            painter.image(out.texture, rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), tint);

            // Nothing but the picture during a show or while (fading to) black.
            let extras = !out.show_mode && !out.frozen && level >= 1.0;
            if extras && self.show_outlines {
                draw_output_overlays(painter, rect, out.project, out.selection);
            }
            if extras
                && self.show_cursor
                && let Some(p) = out.pointer
            {
                let size = out.project.canvas.size();
                let at = rect.min + egui::vec2(p.x / size.x * rect.width(), p.y / size.y * rect.height());
                if rect.contains(at) {
                    draw_pointer(painter, at, (rect.height() / 36.0).clamp(18.0, 48.0));
                }
            }
            if extras && !fullscreen && resp.hovered() {
                painter.text(
                    rect.center_bottom() - egui::vec2(0.0, 12.0),
                    egui::Align2::CENTER_BOTTOM,
                    "Pick the projector in Output → Projector, or move this window there and press F",
                    egui::FontId::proportional(13.0),
                    Color32::from_white_alpha(180),
                );
            }

            if !out.show_mode {
                let toggle = resp.double_clicked() || ui.input(|i| i.key_pressed(Key::F));
                let leave = ui.input(|i| i.key_pressed(Key::Escape));
                if toggle || (leave && fullscreen) {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Fullscreen(toggle && !fullscreen));
                }
            }
            // Full screen hides the OS pointer, unless the projector shows the mouse anyway.
            let visible = !out.show_mode && (!fullscreen || self.show_cursor);
            if self.cursor_visible != Some(visible) {
                ui.ctx().send_viewport_cmd(ViewportCommand::CursorVisible(visible));
                self.cursor_visible = Some(visible);
            }
        });
    }
}

/// Moves `level` toward `target` so that a full 0↔1 fade takes `fade_secs`.
fn approach(level: f32, target: f32, dt: f32, fade_secs: f32) -> f32 {
    if fade_secs <= 0.0 {
        return target;
    }
    let step = dt.max(0.0) / fade_secs;
    if level < target { (level + step).min(target) } else { (level - step).max(target) }
}

/// A classic arrow pointer, `size` px tall, with its tip at `tip`: white with
/// a black outline, so it reads on any content.
fn draw_pointer(painter: &egui::Painter, tip: Pos2, size: f32) {
    const OUTLINE: [[f32; 2]; 7] =
        [[0.0, 0.0], [0.0, 17.0], [4.0, 13.0], [7.0, 20.0], [9.0, 19.0], [6.0, 12.0], [12.0, 12.0]];
    // The outline is not convex: fill it as triangles.
    const TRIANGLES: [u32; 15] = [0, 1, 2, 0, 2, 5, 0, 5, 6, 2, 3, 4, 2, 4, 5];
    let k = size / 20.0;
    let pts: Vec<Pos2> = OUTLINE.iter().map(|[x, y]| tip + egui::vec2(x * k, y * k)).collect();
    let mut mesh = egui::Mesh::default();
    for p in &pts {
        mesh.colored_vertex(*p, Color32::WHITE);
    }
    mesh.indices.extend(TRIANGLES);
    painter.add(mesh);
    painter.add(egui::Shape::closed_line(pts, Stroke::new((size / 14.0).max(1.5), Color32::BLACK)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blackout_fades_over_the_fade_time() {
        let mut out = OutputWindow { blackout: true, fade_secs: 2.0, ..Default::default() };
        assert!(out.step_fade(0.5));
        assert!((out.level - 0.75).abs() < 1e-6);
        assert!(out.step_fade(1.0));
        assert!(!out.step_fade(1.0), "reaches black after 2 s");
        assert_eq!(out.level, 0.0);
        out.blackout = false;
        assert!(out.step_fade(1.0));
        assert!((out.level - 0.5).abs() < 1e-6);
    }

    #[test]
    fn zero_fade_time_cuts() {
        assert_eq!(approach(1.0, 0.0, 0.016, 0.0), 0.0);
        assert_eq!(approach(0.0, 1.0, 0.016, 0.0), 1.0);
        assert_eq!(approach(0.5, 0.5, 0.016, 1.0), 0.5);
    }

    #[test]
    fn mismatch_only_with_a_connected_projector() {
        let mut out = OutputWindow::default();
        let project = Project::empty();
        assert_eq!(out.size_mismatch(&project), None);
        out.displays =
            vec![Display { name: "EPSON".into(), size: (1920, 1200), refresh_hz: None, has_editor: false }];
        out.choose_display(Some(0));
        assert!(out.open, "choosing a projector opens the output");
        out.placed_on = Some(0);
        out.choose_display(None);
        assert_eq!(out.placed_on, Some(0), "kept, so the window leaves full screen");
        out.choose_display(Some(0));
        assert_eq!(out.placed_on, None, "re-sent to the projector");
        assert_eq!(out.size_mismatch(&project), Some((1920, 1200)));
        out.displays[0].size = (1920, 1080);
        assert_eq!(out.size_mismatch(&project), None);
        out.displays.clear();
        assert_eq!(out.projector(), None);
    }
}
