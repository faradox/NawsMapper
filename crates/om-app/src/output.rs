//! The projector output: a second native window that shows the composed
//! canvas edge-to-edge. Drag it onto the projector's screen and press `F`
//! (or double-click) to go full screen.

use egui::{Color32, Key, Pos2, Rect, Sense, ViewportBuilder, ViewportCommand, ViewportId};
use om_core::Project;

use crate::editor::{Selection, draw_output_overlays};

pub struct OutputWindow {
    pub open: bool,
    /// Draw surface outlines and selected handles on the projector,
    /// which makes aligning corners on the real object much easier.
    pub show_outlines: bool,
    pub fullscreen: bool,
}

impl Default for OutputWindow {
    fn default() -> Self {
        Self { open: false, show_outlines: true, fullscreen: false }
    }
}

impl OutputWindow {
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        tex: egui::TextureId,
        project: &Project,
        selection: &Selection,
    ) {
        if !self.open {
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
                let fs = toggle && !fullscreen;
                ui.ctx().send_viewport_cmd(ViewportCommand::Fullscreen(fs));
                ui.ctx().send_viewport_cmd(ViewportCommand::CursorVisible(!fs));
            }
        });
    }
}
