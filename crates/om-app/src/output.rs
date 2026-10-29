//! The projector output: a second native window that shows the composed
//! canvas edge-to-edge. Drag it onto the projector's screen and press `F`
//! (or double-click) to go full screen.

use egui::{Color32, Key, Pos2, Rect, Sense, Stroke, ViewportBuilder, ViewportCommand, ViewportId};
use glam::Vec2;
use om_core::Project;

use crate::editor::{Selection, draw_output_overlays};

pub struct OutputWindow {
    pub open: bool,
    /// Draw surface outlines and selected handles on the projector,
    /// which makes aligning corners on the real object much easier.
    pub show_outlines: bool,
    pub fullscreen: bool,
    /// Draw a mouse pointer on the projector where the pointer is on the
    /// editor canvas, so you can trace the real object while looking at it.
    pub show_cursor: bool,
    /// Last OS cursor visibility sent to the window.
    cursor_visible: Option<bool>,
}

impl Default for OutputWindow {
    fn default() -> Self {
        Self { open: false, show_outlines: true, fullscreen: false, show_cursor: false, cursor_visible: None }
    }
}

impl OutputWindow {
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        tex: egui::TextureId,
        project: &Project,
        selection: &Selection,
        pointer: Option<Vec2>,
    ) {
        if !self.open {
            self.cursor_visible = None;
            return;
        }
        let builder =
            ViewportBuilder::default().with_title("NawsMapper Output").with_inner_size([960.0, 540.0]);

        ctx.show_viewport_immediate(ViewportId::from_hash_of("om_output"), builder, |ui, _class| {
            let (close, fullscreen) =
                ui.input(|i| (i.viewport().close_requested(), i.viewport().fullscreen.unwrap_or(false)));
            if close {
                self.open = false;
            }
            self.fullscreen = fullscreen;

            let rect = ui.max_rect();
            let resp = ui.allocate_rect(rect, Sense::click());
            let painter = ui.painter();
            painter.rect_filled(rect, 0.0, Color32::BLACK);
            painter.image(tex, rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            if self.show_outlines {
                draw_output_overlays(painter, rect, project, selection);
            }
            if self.show_cursor
                && let Some(p) = pointer
            {
                let size = project.canvas.size();
                let at = rect.min + egui::vec2(p.x / size.x * rect.width(), p.y / size.y * rect.height());
                if rect.contains(at) {
                    draw_pointer(painter, at, (rect.height() / 36.0).clamp(18.0, 48.0));
                }
            }
            if !fullscreen && resp.hovered() {
                painter.text(
                    rect.center_bottom() - egui::vec2(0.0, 12.0),
                    egui::Align2::CENTER_BOTTOM,
                    "Move this window to the projector, then press F or double-click for full screen",
                    egui::FontId::proportional(13.0),
                    Color32::from_white_alpha(180),
                );
            }

            let toggle = resp.double_clicked() || ui.input(|i| i.key_pressed(Key::F));
            let leave = ui.input(|i| i.key_pressed(Key::Escape));
            if toggle || (leave && fullscreen) {
                ui.ctx().send_viewport_cmd(ViewportCommand::Fullscreen(toggle && !fullscreen));
            }
            // Full screen hides the OS pointer, unless the projector shows the mouse anyway.
            let visible = !fullscreen || self.show_cursor;
            if self.cursor_visible != Some(visible) {
                ui.ctx().send_viewport_cmd(ViewportCommand::CursorVisible(visible));
                self.cursor_visible = Some(visible);
            }
        });
    }
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
