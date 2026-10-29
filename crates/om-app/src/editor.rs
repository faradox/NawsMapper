//! The central canvas: preview of the output plus direct manipulation of
//! surfaces (select, move, corner-pin), with pan/zoom like an image editor.
//!
//! Interaction summary:
//! - Click a surface to select it; Shift/Cmd-click toggles it in the selection.
//! - Drag on empty canvas draws a selection marquee.
//! - Drag a surface to move the whole selection.
//! - Drag a corner handle to pin it. Corners that sit exactly on top of each
//!   other are *welded* and move together (hold Alt/Option to move one alone).
//! - Corners and moves snap to other surfaces' corners and the canvas corners
//!   (hold Alt/Option to disable snapping).
//! - Space-drag or middle-drag pans; scroll pans; pinch / Cmd+scroll zooms.
//! - Free Transform (Cmd+T) replaces the corner handles with a bounding box:
//!   handles scale (Shift proportional, Alt from center), dragging outside
//!   rotates (Shift snaps to 15°), dragging inside moves. See `transform.rs`.
//! - Pen (P): each click adds an anchor point (press and drag for a smooth
//!   one); clicking the first point, double-clicking or Enter finishes.
//! - Edit Points (double-click a free-form shape): drag anchors, click an
//!   edge to add one, Alt-click or Delete removes, double-click toggles
//!   smooth/corner. The math lives in `om_core::shape`.

use egui::{Color32, Key, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2 as EVec2};
use glam::{Affine2, Vec2};
use om_core::geom::{point_in_polygon, quad_center};
use om_core::shape::{MAX_PATH_POINTS, flatten_with_edges};
use om_core::{Mask, PathPoint, Project, Surface, SurfaceId};

use crate::theme;
use crate::transform::{self, FreeTransform, HANDLES, Handle};

const HANDLE_RADIUS: f32 = 6.0;
const SNAP_DISTANCE_PX: f32 = 10.0;
/// Corners closer than this (canvas px) count as welded.
const WELD_EPSILON: f32 = 0.5;
/// Screen-space grab radius of free-transform handles.
const TF_HANDLE_GRAB: f32 = 9.0;
/// Screen-space grab distance of anchor points and path edges.
const ANCHOR_GRAB: f32 = 7.0;
/// Dragging this far (screen px) while placing a pen point makes it smooth.
const SMOOTH_DRAG: f32 = 6.0;

#[derive(Default)]
pub struct Selection {
    /// Selected surfaces in the order they were selected; last is primary.
    pub surfaces: Vec<SurfaceId>,
    /// The corner last clicked, for keyboard nudging.
    pub corner: Option<(SurfaceId, usize)>,
}

impl Selection {
    pub fn contains(&self, id: SurfaceId) -> bool {
        self.surfaces.contains(&id)
    }

    pub fn primary(&self) -> Option<SurfaceId> {
        self.surfaces.last().copied()
    }

    pub fn set(&mut self, ids: Vec<SurfaceId>) {
        self.surfaces = ids;
        self.corner = None;
    }

    pub fn toggle(&mut self, id: SurfaceId) {
        if let Some(i) = self.surfaces.iter().position(|s| *s == id) {
            self.surfaces.remove(i);
        } else {
            self.surfaces.push(id);
        }
        self.corner = None;
    }

    pub fn clear(&mut self) {
        self.set(Vec::new());
    }

    /// Drops ids that no longer exist (after undo, delete, load...).
    pub fn retain_existing(&mut self, project: &Project) {
        self.surfaces.retain(|id| project.surface(*id).is_some());
        if let Some((id, _)) = self.corner
            && !self.surfaces.contains(&id)
        {
            self.corner = None;
        }
    }
}

enum Drag {
    None,
    Pan { start_pan: EVec2 },
    Move { origin: Vec2, start: Vec<(SurfaceId, [Vec2; 4])> },
    Corner { origin: Vec2, targets: Vec<(SurfaceId, usize, Vec2)> },
    Marquee { start: Pos2, base: Vec<SurfaceId> },
    TfScale { start: Affine2, handle: Handle },
    TfRotate { start: Affine2, from: Vec2 },
    TfMove { start: Affine2, from: Vec2 },
    PenPoint { index: usize, origin: Pos2 },
    Anchor { id: SurfaceId, index: usize, start: Box<Surface> },
}

/// Anchor point editing on one free-form surface.
pub struct PointEdit {
    pub id: SurfaceId,
    pub selected: Option<usize>,
}

pub struct Editor {
    pub selection: Selection,
    pub selected_source: Option<om_core::SourceId>,
    /// Scroll the Sources list to the selected source on the next frame.
    pub scroll_to_source: bool,
    pub zoom: f32,
    pan: EVec2,
    /// Keep the canvas fitted to the view until the user zooms or pans.
    auto_fit: bool,
    drag: Drag,
    pub snapping: bool,
    pub show_outlines: bool,
    /// Space was used for panning during the current press (so releasing it
    /// should not toggle playback).
    pub space_panned: bool,
    /// Active Free Transform session, if any.
    pub transform: Option<FreeTransform>,
    /// Pen tool in progress: the anchor points placed so far (canvas px).
    pub pen: Option<Vec<PathPoint>>,
    /// A finished pen path; the app turns it into a surface.
    pub finished_pen: Option<Vec<PathPoint>>,
    pub points: Option<PointEdit>,
    last_rect: Rect,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            selection: Selection::default(),
            selected_source: None,
            scroll_to_source: false,
            zoom: 1.0,
            pan: EVec2::ZERO,
            auto_fit: true,
            drag: Drag::None,
            snapping: true,
            show_outlines: true,
            space_panned: false,
            transform: None,
            pen: None,
            finished_pen: None,
            points: None,
            last_rect: Rect::NOTHING,
        }
    }
}

/// Maps between canvas pixels and screen points for one frame.
#[derive(Clone, Copy)]
pub struct ViewXf {
    origin: Pos2,
    zoom: f32,
}

impl ViewXf {
    pub fn to_screen(self, p: Vec2) -> Pos2 {
        self.origin + EVec2::new(p.x, p.y) * self.zoom
    }

    pub fn to_canvas(self, p: Pos2) -> Vec2 {
        let v = (p - self.origin) / self.zoom;
        Vec2::new(v.x, v.y)
    }
}

impl Editor {
    pub fn is_interacting(&self) -> bool {
        !matches!(self.drag, Drag::None)
    }

    /// Starts Free Transform on the selection. Returns false if there is
    /// nothing (unlocked) to transform.
    pub fn begin_transform(&mut self, project: &Project) -> bool {
        self.transform = FreeTransform::begin(project, &self.selection.surfaces);
        self.selection.corner = None;
        self.transform.is_some()
    }

    /// Keeps the transformed geometry and leaves transform mode.
    pub fn apply_transform(&mut self) {
        self.transform = None;
    }

    /// Restores the geometry from before Cmd+T and leaves transform mode.
    pub fn cancel_transform(&mut self, project: &mut Project) {
        if let Some(tf) = self.transform.take() {
            tf.restore(project);
        }
    }

    pub fn nudge_transform(&mut self, project: &mut Project, delta: Vec2) {
        if let Some(tf) = &mut self.transform {
            tf.xf = transform::moved(tf.xf, delta);
            tf.apply(project);
        }
    }

    /// Selects a source and scrolls the Sources list to it.
    pub fn reveal_source(&mut self, id: om_core::SourceId) {
        self.selected_source = Some(id);
        self.scroll_to_source = true;
    }

    /// Finishes the pen path (if it has enough points) and leaves pen mode.
    pub fn finish_pen(&mut self) {
        if let Some(points) = self.pen.take()
            && points.len() >= 3
        {
            self.finished_pen = Some(points);
        }
    }

    /// Starts editing the anchor points of `id` (which must be free-form).
    pub fn edit_points(&mut self, id: SurfaceId) {
        self.transform = None;
        self.pen = None;
        self.selection.set(vec![id]);
        self.points = Some(PointEdit { id, selected: None });
    }

    /// Removes the selected anchor point. Returns false if there is none
    /// (or the shape is down to three points).
    pub fn delete_point(&mut self, project: &mut Project) -> bool {
        let Some(pe) = &mut self.points else { return false };
        let (Some(i), Some(s)) = (pe.selected, project.surface_mut(pe.id)) else { return false };
        let removed = s.remove_path_point(i);
        if removed {
            pe.selected = None;
        }
        removed
    }

    /// Moves the selected anchor point by `delta` canvas pixels.
    pub fn nudge_point(&mut self, project: &mut Project, delta: Vec2) -> bool {
        let Some(pe) = &self.points else { return false };
        let (Some(i), Some(s)) = (pe.selected, project.surface_mut(pe.id)) else { return false };
        if let Some(pos) = s.path_on_canvas().get(i).copied() {
            s.move_path_point(i, pos + delta);
        }
        true
    }

    pub fn fit(&mut self) {
        self.auto_fit = true;
        self.pan = EVec2::ZERO;
    }

    pub fn set_zoom(&mut self, project: &Project, zoom: f32) {
        self.auto_fit = false;
        let center = self.last_rect.center();
        self.zoom_about(project, center, zoom);
    }

    fn view(&self, rect: Rect, project: &Project) -> ViewXf {
        let size = project.canvas.size();
        let origin = rect.center() + self.pan - EVec2::new(size.x, size.y) * 0.5 * self.zoom;
        ViewXf { origin, zoom: self.zoom }
    }

    fn zoom_about(&mut self, project: &Project, screen: Pos2, new_zoom: f32) {
        let new_zoom = new_zoom.clamp(0.02, 32.0);
        let xf = self.view(self.last_rect, project);
        let anchor = xf.to_canvas(screen);
        let size = project.canvas.size();
        let new_origin = screen - EVec2::new(anchor.x, anchor.y) * new_zoom;
        self.pan = new_origin - self.last_rect.center() + EVec2::new(size.x, size.y) * 0.5 * new_zoom;
        self.zoom = new_zoom;
    }

    /// Draws the canvas and handles all pointer interaction on it.
    pub fn ui(&mut self, ui: &mut Ui, project: &mut Project, output_tex: egui::TextureId) {
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        self.last_rect = rect;
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, theme::WORKSPACE_BG);

        let size = project.canvas.size();
        if self.auto_fit {
            self.zoom = ((rect.width() - 48.0) / size.x).min((rect.height() - 48.0) / size.y).max(0.02);
            self.pan = EVec2::ZERO;
        }

        // ---- Navigation (scroll / pinch / space / middle mouse) ----------
        let (scroll, zoom_delta, space, alt, shift, toggle_mod, hover) = ui.input(|i| {
            (
                i.smooth_scroll_delta(),
                i.zoom_delta(),
                i.key_down(Key::Space),
                i.modifiers.alt,
                i.modifiers.shift,
                i.modifiers.shift || i.modifiers.command,
                i.pointer.hover_pos(),
            )
        });
        let text_focus = ui.ctx().egui_wants_keyboard_input();
        let space = space && !text_focus;
        if let Some(hp) = hover.filter(|p| rect.contains(*p)) {
            if zoom_delta != 1.0 {
                self.auto_fit = false;
                self.zoom_about(project, hp, self.zoom * zoom_delta);
            } else if scroll != EVec2::ZERO {
                self.auto_fit = false;
                self.pan += scroll;
            }
        }

        let xf = self.view(rect, project);
        let canvas_rect = Rect::from_min_max(xf.to_screen(Vec2::ZERO), xf.to_screen(size));

        // ---- Pointer interaction ----------------------------------------
        let press_origin = ui.input(|i| i.pointer.press_origin());
        let snap = self.snapping && !alt;
        let snap_dist = SNAP_DISTANCE_PX / self.zoom;

        // Selecting something else (e.g. in the Layers panel) applies the transform.
        if self.transform.as_ref().is_some_and(|tf| tf.ids != self.selection.surfaces) {
            self.transform = None;
        }
        if let Some(pe) = &self.points
            && (self.selection.surfaces != [pe.id]
                || project.surface(pe.id).is_none_or(|s| s.mask != Mask::Freeform || s.locked || !s.visible))
        {
            self.points = None;
        }

        // Pen and point editing get the first say over presses.
        let mut press_used = false;
        if resp.drag_started() && !(space || resp.dragged_by(egui::PointerButton::Middle)) {
            let start = press_origin.unwrap_or(rect.center());
            if self.pen.is_some() {
                let p = snap_pen_point(project, xf.to_canvas(start), snap, snap_dist);
                if let Some(index) = self.pen_add(&xf, p) {
                    self.drag = Drag::PenPoint { index, origin: start };
                }
                press_used = true;
            } else if let Some(pe) = &mut self.points
                && let Some(s) = project.surface_mut(pe.id)
            {
                let target = hit_anchor(s, &xf, start).or_else(|| {
                    let (edge, pos) = hit_edge(s, &xf, start)?;
                    s.insert_path_point(edge, pos)
                });
                if let Some(index) = target {
                    pe.selected = Some(index);
                    self.drag = Drag::Anchor { id: s.id, index, start: Box::new(s.clone()) };
                    press_used = true;
                } else {
                    self.points = None;
                }
            }
        }
        if !press_used && self.pen.is_some() {
            if resp.double_clicked() {
                self.finish_pen();
            } else if resp.clicked()
                && let Some(p) = resp.interact_pointer_pos()
            {
                let p = snap_pen_point(project, xf.to_canvas(p), snap, snap_dist);
                self.pen_add(&xf, p);
            }
        } else if !press_used
            && let Some(pe) = &mut self.points
            && let Some(s) = project.surface_mut(pe.id)
            && (resp.clicked() || resp.double_clicked())
            && let Some(p) = resp.interact_pointer_pos()
        {
            if let Some(i) = hit_anchor(s, &xf, p) {
                if alt {
                    s.remove_path_point(i);
                    pe.selected = None;
                } else if resp.double_clicked() {
                    s.toggle_smooth(i);
                    pe.selected = Some(i);
                } else {
                    pe.selected = Some(i);
                }
            } else if let Some((edge, pos)) = hit_edge(s, &xf, p) {
                pe.selected = s.insert_path_point(edge, pos);
            } else if !point_in_polygon(xf.to_canvas(p), &s.outline()) {
                self.points = None;
                self.selection.clear();
            }
        }
        let tool_active = self.pen.is_some() || self.points.is_some();

        if resp.drag_started() && !press_used {
            let start = press_origin.unwrap_or(rect.center());
            let middle = resp.dragged_by(egui::PointerButton::Middle);
            if space || middle {
                self.drag = Drag::Pan { start_pan: self.pan };
                self.space_panned |= space;
                self.auto_fit = false;
            } else if let Some(tf) = &self.transform {
                let p = xf.to_canvas(start);
                self.drag = if let Some(handle) = hit_tf_handle(tf, &xf, start) {
                    Drag::TfScale { start: tf.xf, handle }
                } else if tf.contains(p) {
                    Drag::TfMove { start: tf.xf, from: p }
                } else {
                    Drag::TfRotate { start: tf.xf, from: p }
                };
            } else if let Some((id, idx)) = self.hit_corner(project, &xf, start) {
                let origin = project.surface(id).map(|s| s.corners[idx]).unwrap_or_default();
                let mut targets = vec![(id, idx, origin)];
                if !alt {
                    targets.extend(welded_corners(project, origin, id));
                }
                self.selection.corner = Some((id, idx));
                self.drag = Drag::Corner { origin, targets };
            } else if let Some(id) = hit_surface(project, xf.to_canvas(start)) {
                if toggle_mod {
                    if !self.selection.contains(id) {
                        self.selection.surfaces.push(id);
                    }
                } else if !self.selection.contains(id) {
                    self.selection.set(vec![id]);
                }
                self.selection.corner = None;
                let start_corners = self
                    .selection
                    .surfaces
                    .iter()
                    .filter_map(|sid| project.surface(*sid))
                    .filter(|s| !s.locked)
                    .map(|s| (s.id, s.corners))
                    .collect();
                self.drag = Drag::Move { origin: xf.to_canvas(start), start: start_corners };
            } else {
                let base = if toggle_mod { self.selection.surfaces.clone() } else { Vec::new() };
                self.drag = Drag::Marquee { start, base };
            }
        }

        let pointer = resp.interact_pointer_pos().or(hover);
        if resp.dragged() {
            match &mut self.drag {
                Drag::Pan { start_pan } => {
                    if let Some(origin) = press_origin {
                        let cur = pointer.unwrap_or(origin);
                        self.pan = *start_pan + (cur - origin);
                    }
                }
                Drag::Move { origin, start } => {
                    if let Some(p) = pointer {
                        let mut delta = xf.to_canvas(p) - *origin;
                        if snap {
                            let moving: Vec<SurfaceId> = start.iter().map(|(id, _)| *id).collect();
                            let candidates = snap_candidates(project, &moving, None);
                            let corners = start.iter().flat_map(|(_, c)| c.iter().map(|c| *c + delta));
                            delta += snap_adjustment(&candidates, corners, snap_dist);
                        }
                        for (id, corners) in start.iter() {
                            if let Some(s) = project.surface_mut(*id) {
                                s.corners = corners.map(|c| c + delta);
                            }
                        }
                    }
                }
                Drag::Corner { origin, targets } => {
                    if let Some(p) = pointer {
                        let (id0, idx0, _) = targets[0];
                        let mut pos = *origin + (xf.to_canvas(p) - xf.to_canvas(press_origin.unwrap_or(p)));
                        if snap {
                            let welded: Vec<SurfaceId> = targets.iter().map(|t| t.0).collect();
                            let candidates = snap_candidates(project, &welded, Some((id0, idx0)));
                            if let Some((_, target)) = nearest(&candidates, pos, snap_dist) {
                                pos = target;
                            }
                        }
                        for (id, idx, _) in targets.iter() {
                            if let Some(s) = project.surface_mut(*id)
                                && !s.locked
                            {
                                s.corners[*idx] = pos;
                            }
                        }
                    }
                }
                Drag::Marquee { start, base } => {
                    if let Some(p) = pointer {
                        let r = Rect::from_two_pos(*start, p);
                        let mut ids = base.clone();
                        for s in &project.surfaces {
                            if !s.visible || s.locked || ids.contains(&s.id) {
                                continue;
                            }
                            let pts = s.corners.map(|c| xf.to_screen(c));
                            let center = xf.to_screen(quad_center(s.corners));
                            if r.contains(center) || pts.iter().any(|p| r.contains(*p)) {
                                ids.push(s.id);
                            }
                        }
                        self.selection.set(ids);
                    }
                }
                Drag::TfScale { start, handle } => {
                    if let (Some(tf), Some(p)) = (&mut self.transform, pointer) {
                        tf.xf = transform::scaled(*start, tf.bounds(), *handle, xf.to_canvas(p), shift, alt);
                        tf.apply(project);
                    }
                }
                Drag::TfRotate { start, from } => {
                    if let (Some(tf), Some(p)) = (&mut self.transform, pointer) {
                        let pivot = start.transform_point2((tf.bounds().0 + tf.bounds().1) * 0.5);
                        tf.xf = transform::rotated(*start, pivot, *from, xf.to_canvas(p), shift);
                        tf.apply(project);
                    }
                }
                Drag::TfMove { start, from } => {
                    if let (Some(tf), Some(p)) = (&mut self.transform, pointer) {
                        let mut delta = xf.to_canvas(p) - *from;
                        if snap {
                            let candidates = snap_candidates(project, &tf.surface_ids(), None);
                            let corners = tf.corners_under(transform::moved(*start, delta));
                            delta += snap_adjustment(&candidates, corners, snap_dist);
                        }
                        tf.xf = transform::moved(*start, delta);
                        tf.apply(project);
                    }
                }
                Drag::PenPoint { index, origin } => {
                    if let (Some(pen), Some(p)) = (&mut self.pen, pointer)
                        && p.distance(*origin) > SMOOTH_DRAG
                        && let Some(pt) = pen.get_mut(*index)
                    {
                        pt.smooth = true;
                    }
                }
                Drag::Anchor { id, index, start } => {
                    if let Some(p) = pointer {
                        let origin = start.path_on_canvas()[*index];
                        let mut pos = origin + (xf.to_canvas(p) - xf.to_canvas(press_origin.unwrap_or(p)));
                        if snap {
                            let candidates = snap_candidates(project, &[*id], None);
                            if let Some((_, target)) = nearest(&candidates, pos, snap_dist) {
                                pos = target;
                            }
                        }
                        let mut moved = (**start).clone();
                        moved.move_path_point(*index, pos);
                        if let Some(s) = project.surface_mut(*id) {
                            s.corners = moved.corners;
                            s.path = moved.path;
                        }
                    }
                }
                Drag::None => {}
            }
        }

        if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
            self.drag = Drag::None;
        }

        if tool_active {
            // Handled above.
        } else if let Some(tf) = &self.transform {
            // In transform mode clicks don't change the selection;
            // double-clicking inside the box applies (like Photoshop).
            if resp.double_clicked()
                && resp.interact_pointer_pos().is_some_and(|p| tf.contains(xf.to_canvas(p)))
            {
                self.transform = None;
            }
        } else if resp.double_clicked()
            && let Some(id) = resp.interact_pointer_pos().and_then(|p| hit_surface(project, xf.to_canvas(p)))
            && project.surface(id).is_some_and(|s| s.mask == Mask::Freeform)
        {
            self.edit_points(id);
        } else if resp.clicked() {
            let p = resp.interact_pointer_pos().unwrap_or(rect.center());
            if let Some((id, idx)) = self.hit_corner(project, &xf, p) {
                self.selection.corner = Some((id, idx));
            } else if let Some(id) = hit_surface(project, xf.to_canvas(p)) {
                if toggle_mod {
                    self.selection.toggle(id);
                } else {
                    self.selection.set(vec![id]);
                }
            } else if !toggle_mod {
                self.selection.clear();
            }
        }

        // ---- Cursor ------------------------------------------------------
        if let Some(hp) = hover.filter(|p| rect.contains(*p)) {
            let icon = if matches!(self.drag, Drag::Pan { .. }) {
                egui::CursorIcon::Grabbing
            } else if space {
                egui::CursorIcon::Grab
            } else if self.pen.is_some() {
                egui::CursorIcon::Crosshair
            } else if let Some(s) = self.points.as_ref().and_then(|pe| project.surface(pe.id)) {
                if matches!(self.drag, Drag::Anchor { .. }) || hit_anchor(s, &xf, hp).is_some() {
                    egui::CursorIcon::Crosshair
                } else if hit_edge(s, &xf, hp).is_some() {
                    egui::CursorIcon::Cell
                } else {
                    egui::CursorIcon::Default
                }
            } else if let Some(tf) = &self.transform {
                tf_cursor(tf, &xf, &self.drag, hp)
            } else if matches!(self.drag, Drag::Corner { .. }) || self.hit_corner(project, &xf, hp).is_some()
            {
                egui::CursorIcon::Crosshair
            } else if matches!(self.drag, Drag::Move { .. })
                || hit_surface(project, xf.to_canvas(hp)).is_some()
            {
                egui::CursorIcon::Move
            } else {
                egui::CursorIcon::Default
            };
            ui.ctx().set_cursor_icon(icon);
        }

        // ---- Drawing -----------------------------------------------------
        painter.rect_filled(canvas_rect.expand(1.0), 0.0, Color32::BLACK);
        painter.image(
            output_tex,
            canvas_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
        painter.rect_stroke(canvas_rect, 0.0, Stroke::new(1.0, Color32::from_gray(90)), StrokeKind::Outside);

        let hovered = hover.filter(|p| rect.contains(*p)).and_then(|p| hit_surface(project, xf.to_canvas(p)));
        let show_handles = self.transform.is_none() && !tool_active;
        draw_overlays(&painter, project, &xf, &self.selection, hovered, self.show_outlines, show_handles);
        let hint = |text: &str| {
            painter.text(
                rect.left_top() + EVec2::new(10.0, 8.0),
                egui::Align2::LEFT_TOP,
                text,
                egui::FontId::proportional(12.0),
                Color32::from_gray(200),
            );
        };
        if let Some(pen) = &self.pen {
            let cursor = hover
                .filter(|p| rect.contains(*p))
                .map(|p| snap_pen_point(project, xf.to_canvas(p), snap, snap_dist));
            draw_pen(&painter, &xf, pen, cursor);
            hint(
                "Pen: click to add points (drag for a curve point) · click the first point, double-click or \
                  Enter to finish · Backspace removes the last point · Esc cancels",
            );
        }
        if let Some(pe) = &self.points
            && let Some(s) = project.surface(pe.id)
        {
            draw_points(&painter, &xf, s, pe.selected);
            hint(
                "Edit Points: drag points · click an edge to add one · Alt-click or Delete removes · \
                  double-click a point for curve/corner · Enter or Esc when done",
            );
        }
        if let Some(tf) = &self.transform {
            draw_transform_box(&painter, tf, &xf);
            hint(
                "Free Transform: drag handles to scale (Shift: keep proportions, Alt: from center) · \
                 drag outside to rotate (Shift: 15° steps) · Enter applies, Esc cancels",
            );
        }

        if let (Drag::Marquee { start, .. }, Some(p)) = (&self.drag, pointer) {
            let r = Rect::from_two_pos(*start, p);
            painter.rect_filled(r, 0.0, theme::ACCENT.linear_multiply(0.12));
            painter.rect_stroke(r, 0.0, Stroke::new(1.0, theme::ACCENT), StrokeKind::Inside);
        }
    }

    fn hit_corner(&self, project: &Project, xf: &ViewXf, p: Pos2) -> Option<(SurfaceId, usize)> {
        let mut best: Option<(f32, SurfaceId, usize)> = None;
        for id in self.selection.surfaces.iter().rev() {
            let Some(s) = project.surface(*id).filter(|s| s.visible && !s.locked) else { continue };
            for (i, c) in s.corners.iter().enumerate() {
                let d = xf.to_screen(*c).distance(p);
                if d <= HANDLE_RADIUS + 3.0 && best.is_none_or(|(bd, ..)| d < bd) {
                    best = Some((d, s.id, i));
                }
            }
        }
        best.map(|(_, id, i)| (id, i))
    }

    /// Adds a pen point at `p`, or finishes the path when `p` is on the
    /// first point. Returns the new point's index.
    fn pen_add(&mut self, xf: &ViewXf, p: Vec2) -> Option<usize> {
        let pen = self.pen.as_mut()?;
        if pen.len() >= 3 && xf.to_screen(pen[0].pos).distance(xf.to_screen(p)) <= ANCHOR_GRAB {
            self.finish_pen();
            return None;
        }
        if pen.len() >= MAX_PATH_POINTS || pen.last().is_some_and(|last| last.pos.distance(p) < 0.5) {
            return None;
        }
        pen.push(PathPoint::corner(p));
        Some(pen.len() - 1)
    }
}

fn snap_pen_point(project: &Project, p: Vec2, snap: bool, max: f32) -> Vec2 {
    if !snap {
        return p;
    }
    nearest(&snap_candidates(project, &[], None), p, max).map_or(p, |(_, target)| target)
}

/// Anchor point of a free-form surface under the screen point `p`.
fn hit_anchor(s: &Surface, xf: &ViewXf, p: Pos2) -> Option<usize> {
    s.path_on_canvas()
        .iter()
        .enumerate()
        .map(|(i, c)| (xf.to_screen(*c).distance(p), i))
        .filter(|(d, _)| *d <= ANCHOR_GRAB)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, i)| i)
}

/// Edge of a free-form surface near the screen point `p`: the anchor the
/// edge starts at and the closest canvas point on it.
fn hit_edge(s: &Surface, xf: &ViewXf, p: Pos2) -> Option<(usize, Vec2)> {
    let h = om_core::Homography::from_corners(s.corners);
    let pts: Vec<(Pos2, usize)> =
        flatten_with_edges(&s.path).into_iter().map(|(uv, e)| (xf.to_screen(h.map(uv)), e)).collect();
    let mut best: Option<(f32, usize, Pos2)> = None;
    for i in 0..pts.len() {
        let ((a, edge), (b, _)) = (pts[i], pts[(i + 1) % pts.len()]);
        let ab = b - a;
        let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
        let q = a + ab * t;
        let d = q.distance(p);
        if d <= ANCHOR_GRAB && best.is_none_or(|(bd, ..)| d < bd) {
            best = Some((d, edge, q));
        }
    }
    best.map(|(_, edge, q)| (edge, xf.to_canvas(q)))
}

fn draw_pen(painter: &egui::Painter, xf: &ViewXf, pen: &[PathPoint], cursor: Option<Vec2>) {
    let mut pts = pen.to_vec();
    pts.extend(cursor.map(PathPoint::corner));
    let n = pts.len();
    if n >= 2 {
        // The path so far, plus a faint closing edge back to the start.
        let line = flatten_with_edges(&pts);
        for i in 0..line.len() {
            let (a, edge) = line[i];
            let b = line[(i + 1) % line.len()].0;
            let closing = edge == n - 1;
            if closing && n < 3 {
                continue;
            }
            let stroke = if closing {
                Stroke::new(1.0, theme::ACCENT.linear_multiply(0.4))
            } else {
                Stroke::new(1.5, theme::ACCENT)
            };
            painter.line_segment([xf.to_screen(a), xf.to_screen(b)], stroke);
        }
    }
    let closing = pen.len() >= 3
        && cursor.is_some_and(|c| xf.to_screen(c).distance(xf.to_screen(pen[0].pos)) <= ANCHOR_GRAB);
    for (i, p) in pen.iter().enumerate() {
        let fill = if i == 0 && closing { theme::ACCENT } else { Color32::WHITE };
        draw_anchor(painter, xf.to_screen(p.pos), p.smooth, fill, i == 0);
    }
}

fn draw_points(painter: &egui::Painter, xf: &ViewXf, s: &Surface, selected: Option<usize>) {
    let frame: Vec<Pos2> = s.corners.iter().map(|c| xf.to_screen(*c)).collect();
    let mut closed = frame.clone();
    closed.push(frame[0]);
    painter.extend(Shape::dashed_line(&closed, Stroke::new(1.0, theme::OUTLINE), 3.0, 3.0));
    for (i, (c, p)) in s.path_on_canvas().iter().zip(&s.path).enumerate() {
        let fill = if selected == Some(i) { theme::ACCENT } else { Color32::WHITE };
        draw_anchor(painter, xf.to_screen(*c), p.smooth, fill, false);
    }
}

/// Square for corner points, circle for smooth ones (like Illustrator).
fn draw_anchor(painter: &egui::Painter, p: Pos2, smooth: bool, fill: Color32, big: bool) {
    let r = if big { 5.5 } else { 4.0 };
    let stroke = Stroke::new(1.5, theme::ACCENT);
    if smooth {
        painter.circle(p, r, fill, stroke);
    } else {
        let rect = Rect::from_center_size(p, EVec2::splat(r * 2.0));
        painter.rect_filled(rect, 0.0, fill);
        painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Middle);
    }
}

fn hit_tf_handle(tf: &FreeTransform, xf: &ViewXf, p: Pos2) -> Option<Handle> {
    HANDLES
        .iter()
        .map(|h| (xf.to_screen(tf.handle_pos(*h)).distance(p), *h))
        .filter(|(d, _)| *d <= TF_HANDLE_GRAB)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, h)| h)
}

/// Resize cursor matching the on-screen direction of a (possibly rotated) handle.
fn tf_cursor(tf: &FreeTransform, xf: &ViewXf, drag: &Drag, hover: Pos2) -> egui::CursorIcon {
    use egui::CursorIcon as C;
    let handle = match drag {
        Drag::TfScale { handle, .. } => Some(*handle),
        Drag::TfMove { .. } => return C::Move,
        Drag::TfRotate { .. } => return C::Crosshair,
        _ => hit_tf_handle(tf, xf, hover),
    };
    match handle {
        Some(h) => {
            let dir = tf.handle_pos(h) - tf.center();
            match dir.y.atan2(dir.x).to_degrees().rem_euclid(180.0) {
                a if !(22.5..157.5).contains(&a) => C::ResizeHorizontal,
                a if a < 67.5 => C::ResizeNwSe,
                a if a < 112.5 => C::ResizeVertical,
                _ => C::ResizeNeSw,
            }
        }
        None if tf.contains(xf.to_canvas(hover)) => C::Move,
        None => C::Crosshair,
    }
}

fn draw_transform_box(painter: &egui::Painter, tf: &FreeTransform, xf: &ViewXf) {
    let outline: Vec<Pos2> = tf.outline().iter().map(|p| xf.to_screen(*p)).collect();
    painter.add(Shape::closed_line(outline, Stroke::new(1.0, Color32::WHITE)));
    for h in HANDLES {
        let r = Rect::from_center_size(xf.to_screen(tf.handle_pos(h)), EVec2::splat(8.0));
        painter.rect_filled(r, 0.0, Color32::WHITE);
        painter.rect_stroke(r, 0.0, Stroke::new(1.0, theme::ACCENT), StrokeKind::Inside);
    }
    let c = xf.to_screen(tf.center());
    let s = Stroke::new(1.0, Color32::WHITE);
    painter.line_segment([c - EVec2::new(5.0, 0.0), c + EVec2::new(5.0, 0.0)], s);
    painter.line_segment([c - EVec2::new(0.0, 5.0), c + EVec2::new(0.0, 5.0)], s);
}

/// Smallest correction that makes one of the moving `corners` land on a
/// snap candidate (zero if none is close enough).
fn snap_adjustment(candidates: &[Vec2], corners: impl Iterator<Item = Vec2>, max: f32) -> Vec2 {
    let mut best: Option<(f32, Vec2)> = None;
    for c in corners {
        if let Some((d, target)) = nearest(candidates, c, max)
            && best.is_none_or(|(bd, _)| d < bd)
        {
            best = Some((d, target - c));
        }
    }
    best.map_or(Vec2::ZERO, |(_, adjust)| adjust)
}

/// Front-most visible, unlocked surface under a canvas point.
fn hit_surface(project: &Project, p: Vec2) -> Option<SurfaceId> {
    project
        .surfaces
        .iter()
        .rev()
        .filter(|s| s.visible && !s.locked)
        .find(|s| point_in_polygon(p, &s.outline()))
        .map(|s| s.id)
}

/// Corners of other unlocked surfaces sitting exactly on `pos`.
fn welded_corners(project: &Project, pos: Vec2, except: SurfaceId) -> Vec<(SurfaceId, usize, Vec2)> {
    let mut out = Vec::new();
    for s in project.surfaces.iter().filter(|s| s.id != except && !s.locked) {
        for (i, c) in s.corners.iter().enumerate() {
            if c.distance(pos) <= WELD_EPSILON {
                out.push((s.id, i, *c));
            }
        }
    }
    out
}

/// Points things can snap to: corners of visible surfaces not being moved,
/// plus the canvas corners and center.
fn snap_candidates(
    project: &Project,
    moving: &[SurfaceId],
    own_corner: Option<(SurfaceId, usize)>,
) -> Vec<Vec2> {
    let size = project.canvas.size();
    let mut pts = vec![Vec2::ZERO, Vec2::new(size.x, 0.0), size, Vec2::new(0.0, size.y), size * 0.5];
    for s in project.surfaces.iter().filter(|s| s.visible) {
        let moving_surface = moving.contains(&s.id);
        for (i, c) in s.corners.iter().enumerate() {
            // A surface's own other corners are fine to snap to while
            // pinning one corner, but never while moving the whole thing.
            let is_own = own_corner.is_some_and(|(id, idx)| id == s.id && idx == i);
            let same_surface_pin = own_corner.is_some_and(|(id, _)| id == s.id);
            if is_own || (moving_surface && !same_surface_pin) {
                continue;
            }
            pts.push(*c);
        }
    }
    pts
}

fn nearest(candidates: &[Vec2], p: Vec2, max: f32) -> Option<(f32, Vec2)> {
    candidates
        .iter()
        .map(|c| (c.distance(p), *c))
        .filter(|(d, _)| *d <= max && *d > 1e-4)
        .min_by(|a, b| a.0.total_cmp(&b.0))
}

/// Draws surface outlines, names and corner handles. Shared by the editor
/// and (optionally) the projector output.
pub fn draw_overlays(
    painter: &egui::Painter,
    project: &Project,
    xf: &ViewXf,
    selection: &Selection,
    hovered: Option<SurfaceId>,
    show_outlines: bool,
    show_handles: bool,
) {
    for s in project.surfaces.iter().filter(|s| s.visible) {
        let selected = selection.contains(s.id);
        if !show_outlines && !selected {
            continue;
        }
        let pts: Vec<Pos2> = s.outline().iter().map(|c| xf.to_screen(*c)).collect();
        let frame: Vec<Pos2> = s.corners.iter().map(|c| xf.to_screen(*c)).collect();
        if selected && s.mask != Mask::Quad && show_handles {
            painter.add(Shape::closed_line(
                frame.clone(),
                Stroke::new(1.0, theme::ACCENT.linear_multiply(0.35)),
            ));
        }
        let stroke = if selected {
            Stroke::new(1.5, theme::ACCENT)
        } else if hovered == Some(s.id) {
            Stroke::new(1.5, theme::ACCENT_SOFT)
        } else if s.cutout {
            Stroke::new(1.0, theme::CUTOUT)
        } else {
            Stroke::new(1.0, theme::OUTLINE)
        };
        if s.cutout {
            // Dashed, like a selection marquee: it removes, it doesn't draw.
            let mut closed = pts.clone();
            closed.push(pts[0]);
            painter.extend(Shape::dashed_line(&closed, stroke, 6.0, 4.0));
        } else {
            painter.add(Shape::closed_line(pts.clone(), stroke));
        }

        if show_outlines {
            let label_pos = frame[0] + EVec2::new(6.0, 4.0);
            painter.text(
                label_pos,
                egui::Align2::LEFT_TOP,
                &s.name,
                egui::FontId::proportional(11.0),
                if selected { Color32::WHITE } else { Color32::from_gray(170) },
            );
        }
    }

    if !show_handles {
        return;
    }
    for id in &selection.surfaces {
        let Some(s) = project.surface(*id).filter(|s| s.visible) else { continue };
        for (i, c) in s.corners.iter().enumerate() {
            let p = xf.to_screen(*c);
            let active = selection.corner == Some((s.id, i));
            let fill = if s.locked {
                Color32::from_gray(110)
            } else if active {
                theme::ACCENT
            } else {
                Color32::WHITE
            };
            painter.circle(p, HANDLE_RADIUS, fill, Stroke::new(1.5, theme::ACCENT));
        }
    }
}

/// Like [`draw_overlays`] but for a non-uniform stretch (output window).
pub fn draw_output_overlays(painter: &egui::Painter, rect: Rect, project: &Project, selection: &Selection) {
    let size = project.canvas.size();
    let map = |c: Vec2| {
        Pos2::new(rect.min.x + c.x / size.x * rect.width(), rect.min.y + c.y / size.y * rect.height())
    };
    for s in project.surfaces.iter().filter(|s| s.visible) {
        let selected = selection.contains(s.id);
        let outline: Vec<Pos2> = s.outline().iter().map(|c| map(*c)).collect();
        let stroke =
            if selected { Stroke::new(2.0, theme::ACCENT) } else { Stroke::new(1.0, theme::OUTLINE) };
        painter.add(Shape::closed_line(outline, stroke));
        if selected {
            let pts: Vec<Pos2> = s.corners.iter().map(|c| map(*c)).collect();
            for (i, p) in pts.iter().enumerate() {
                let active = selection.corner == Some((s.id, i));
                painter.circle(*p, if active { 7.0 } else { 4.0 }, theme::ACCENT, Stroke::NONE);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use om_core::Surface;

    fn two_adjacent() -> Project {
        let mut p = Project::empty();
        let a = p.new_surface_id();
        let b = p.new_surface_id();
        p.surfaces.push(Surface::rect(a, "a", Vec2::new(100.0, 100.0), Vec2::new(100.0, 100.0)));
        p.surfaces.push(Surface::rect(b, "b", Vec2::new(200.0, 100.0), Vec2::new(100.0, 100.0)));
        p
    }

    #[test]
    fn shared_corners_are_welded() {
        let p = two_adjacent();
        let a = p.surfaces[0].id;
        // a's top-right corner sits on b's top-left corner.
        let welded = welded_corners(&p, Vec2::new(200.0, 100.0), a);
        assert_eq!(welded.len(), 1);
        assert_eq!((welded[0].0, welded[0].1), (p.surfaces[1].id, 0));
    }

    #[test]
    fn locked_surfaces_are_not_welded() {
        let mut p = two_adjacent();
        p.surfaces[1].locked = true;
        assert!(welded_corners(&p, Vec2::new(200.0, 100.0), p.surfaces[0].id).is_empty());
    }

    #[test]
    fn snapping_ignores_moving_surfaces_and_finds_neighbors() {
        let p = two_adjacent();
        let a = p.surfaces[0].id;
        let pts = snap_candidates(&p, &[a], None);
        assert!(!pts.contains(&Vec2::new(100.0, 100.0)), "own corner must not be a target");
        assert!(pts.contains(&Vec2::new(300.0, 200.0)));
        let hit = nearest(&pts, Vec2::new(296.0, 203.0), 10.0).unwrap();
        assert_eq!(hit.1, Vec2::new(300.0, 200.0));
    }

    #[test]
    fn hit_test_follows_the_shape() {
        let mut p = two_adjacent();
        p.surfaces[0].mask = Mask::Diamond;
        // Inside a's frame but outside its diamond.
        assert_eq!(hit_surface(&p, Vec2::new(105.0, 105.0)), None);
        assert_eq!(hit_surface(&p, Vec2::new(150.0, 150.0)), Some(p.surfaces[0].id));
    }

    #[test]
    fn pen_closes_on_first_point_and_skips_duplicates() {
        let xf = ViewXf { origin: Pos2::ZERO, zoom: 1.0 };
        let mut e = Editor { pen: Some(Vec::new()), ..Default::default() };
        for p in [(0.0, 0.0), (100.0, 0.0), (100.0, 0.0), (50.0, 80.0)] {
            e.pen_add(&xf, Vec2::new(p.0, p.1));
        }
        assert_eq!(e.pen.as_ref().unwrap().len(), 3, "the repeated point is skipped");
        e.pen_add(&xf, Vec2::new(2.0, 1.0));
        assert!(e.pen.is_none());
        assert_eq!(e.finished_pen.as_ref().map(Vec::len), Some(3));
    }

    #[test]
    fn edges_are_found_between_anchors() {
        let mut s = Surface::rect(SurfaceId(1), "s", Vec2::ZERO, Vec2::splat(100.0));
        s.make_freeform();
        let xf = ViewXf { origin: Pos2::ZERO, zoom: 1.0 };
        let (edge, pos) = hit_edge(&s, &xf, Pos2::new(50.0, 3.0)).unwrap();
        assert_eq!(edge, 0);
        assert!(pos.distance(Vec2::new(50.0, 0.0)) < 1e-3);
        assert_eq!(hit_anchor(&s, &xf, Pos2::new(99.0, 98.0)), Some(2));
        assert!(hit_edge(&s, &xf, Pos2::new(50.0, 50.0)).is_none());
    }

    #[test]
    fn hit_test_prefers_front_surface() {
        let mut p = two_adjacent();
        p.surfaces[1].translate(Vec2::new(-50.0, 0.0)); // overlap
        assert_eq!(hit_surface(&p, Vec2::new(175.0, 150.0)), Some(p.surfaces[1].id));
        p.surfaces[1].locked = true;
        assert_eq!(hit_surface(&p, Vec2::new(175.0, 150.0)), Some(p.surfaces[0].id));
    }
}
