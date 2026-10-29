//! Free Transform (Cmd+T): scale, rotate and move the selected surfaces as
//! one group through a bounding box with eight handles, like Photoshop.
//!
//! The box is the axis-aligned bounds of the selection when the transform
//! starts. Everything afterwards is a single affine transform `xf` applied to
//! the original corners, so rotating and then scaling keeps working in the
//! box's own (rotated) frame and never accumulates rounding errors.

use glam::{Affine2, Vec2};
use om_core::geom::point_in_polygon;
use om_core::{Project, SurfaceId};

/// A handle position on the box in half-extents: x and y are each -1, 0 or 1.
pub type Handle = (i8, i8);

pub const HANDLES: [Handle; 8] = [(-1, -1), (0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0)];

pub struct FreeTransform {
    /// The selection the transform was started on.
    pub ids: Vec<SurfaceId>,
    originals: Vec<(SurfaceId, [Vec2; 4])>,
    min: Vec2,
    max: Vec2,
    /// Original canvas space -> transformed canvas space.
    pub xf: Affine2,
}

impl FreeTransform {
    /// Starts a transform on the unlocked surfaces among `ids`.
    pub fn begin(project: &Project, ids: &[SurfaceId]) -> Option<Self> {
        let originals: Vec<(SurfaceId, [Vec2; 4])> = ids
            .iter()
            .filter_map(|id| project.surface(*id))
            .filter(|s| !s.locked)
            .map(|s| (s.id, s.corners))
            .collect();
        let mut pts = originals.iter().flat_map(|(_, c)| c.iter().copied());
        let first = pts.next()?;
        let (min, max) = pts.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p)));
        Some(Self { ids: ids.to_vec(), originals, min, max, xf: Affine2::IDENTITY })
    }

    fn center_local(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    fn local_point(&self, h: Handle) -> Vec2 {
        self.center_local() + (self.max - self.min) * 0.5 * Vec2::new(h.0 as f32, h.1 as f32)
    }

    /// Handle position in canvas space.
    pub fn handle_pos(&self, h: Handle) -> Vec2 {
        self.xf.transform_point2(self.local_point(h))
    }

    pub fn center(&self) -> Vec2 {
        self.xf.transform_point2(self.center_local())
    }

    /// Box corners in canvas space (TL, TR, BR, BL).
    pub fn outline(&self) -> [Vec2; 4] {
        [(-1, -1), (1, -1), (1, 1), (-1, 1)].map(|h| self.handle_pos(h))
    }

    pub fn contains(&self, p: Vec2) -> bool {
        point_in_polygon(p, &self.outline())
    }

    pub fn bounds(&self) -> (Vec2, Vec2) {
        (self.min, self.max)
    }

    /// Writes the transformed corners into the project.
    pub fn apply(&self, project: &mut Project) {
        for (id, corners) in &self.originals {
            if let Some(s) = project.surface_mut(*id) {
                s.corners = corners.map(|c| self.xf.transform_point2(c));
            }
        }
    }

    /// Puts every surface back where it was when the transform started.
    pub fn restore(&self, project: &mut Project) {
        for (id, corners) in &self.originals {
            if let Some(s) = project.surface_mut(*id) {
                s.corners = *corners;
            }
        }
    }

    /// Transformed corners of all surfaces under `xf` (for snapping).
    pub fn corners_under(&self, xf: Affine2) -> impl Iterator<Item = Vec2> + '_ {
        self.originals.iter().flat_map(move |(_, c)| c.iter().map(move |p| xf.transform_point2(*p)))
    }

    pub fn surface_ids(&self) -> Vec<SurfaceId> {
        self.originals.iter().map(|(id, _)| *id).collect()
    }

    /// Scale (1 = unchanged, negative = flipped) and rotation in degrees,
    /// for the toolbar readout.
    pub fn readout(&self) -> (Vec2, f32) {
        let (scale, angle, _) = self.xf.to_scale_angle_translation();
        // Snap float noise to zero so the readout never shows "-0.0°".
        let deg = angle.to_degrees();
        (scale, if deg.abs() < 0.05 { 0.0 } else { deg })
    }
}

/// Scales by dragging `handle` to `pointer` (canvas space).
///
/// - `proportional` (Shift) keeps the aspect ratio.
/// - `from_center` (Alt/Option) scales around the box center instead of the
///   opposite handle.
pub fn scaled(
    start: Affine2,
    (min, max): (Vec2, Vec2),
    handle: Handle,
    pointer: Vec2,
    proportional: bool,
    from_center: bool,
) -> Affine2 {
    let q = start.inverse().transform_point2(pointer);
    let center = (min + max) * 0.5;
    let half = (max - min) * 0.5;
    let h = Vec2::new(handle.0 as f32, handle.1 as f32);
    let anchor = if from_center { center } else { center - half * h };
    let span = center + half * h - anchor;
    let d = q - anchor;

    let mut s = Vec2::ONE;
    if handle.0 != 0 && span.x.abs() > 1e-3 {
        s.x = d.x / span.x;
    }
    if handle.1 != 0 && span.y.abs() > 1e-3 {
        s.y = d.y / span.y;
    }
    if proportional {
        let k = match handle {
            (0, _) => s.y,
            (_, 0) => s.x,
            _ => d.dot(span) / span.length_squared().max(1e-6),
        };
        s = Vec2::splat(k);
    }
    // Never collapse to zero: the inverse is needed for the next drag.
    let s = s.map(|v| if v.abs() < 0.01 { 0.01_f32.copysign(v) } else { v });

    start * Affine2::from_translation(anchor) * Affine2::from_scale(s) * Affine2::from_translation(-anchor)
}

/// Rotates around `pivot` by the angle swept from `from` to `to`.
/// With `snap` the resulting absolute angle lands on multiples of 15°.
pub fn rotated(start: Affine2, pivot: Vec2, from: Vec2, to: Vec2, snap: bool) -> Affine2 {
    let mut angle = (to - pivot).to_angle() - (from - pivot).to_angle();
    if snap {
        let (_, base, _) = start.to_scale_angle_translation();
        let step = 15f32.to_radians();
        angle = ((base + angle) / step).round() * step - base;
    }
    Affine2::from_translation(pivot) * Affine2::from_angle(angle) * Affine2::from_translation(-pivot) * start
}

pub fn moved(start: Affine2, delta: Vec2) -> Affine2 {
    Affine2::from_translation(delta) * start
}

#[cfg(test)]
mod tests {
    use super::*;
    use om_core::Surface;

    const BOX: (Vec2, Vec2) = (Vec2::ZERO, Vec2::new(100.0, 100.0));

    fn close(a: Vec2, b: Vec2) {
        assert!(a.distance(b) < 1e-3, "{a:?} != {b:?}");
    }

    #[test]
    fn corner_handle_scales_from_opposite_corner() {
        let xf = scaled(Affine2::IDENTITY, BOX, (1, 1), Vec2::new(200.0, 150.0), false, false);
        close(xf.transform_point2(Vec2::ZERO), Vec2::ZERO);
        close(xf.transform_point2(Vec2::new(100.0, 100.0)), Vec2::new(200.0, 150.0));
    }

    #[test]
    fn shift_keeps_aspect_ratio() {
        let xf = scaled(Affine2::IDENTITY, BOX, (1, 1), Vec2::new(200.0, 150.0), true, false);
        close(xf.transform_point2(Vec2::new(100.0, 100.0)), Vec2::new(175.0, 175.0));
    }

    #[test]
    fn alt_scales_around_center() {
        let xf = scaled(Affine2::IDENTITY, BOX, (1, 1), Vec2::new(150.0, 150.0), false, true);
        close(xf.transform_point2(Vec2::ZERO), Vec2::new(-50.0, -50.0));
        close(xf.transform_point2(Vec2::splat(50.0)), Vec2::splat(50.0));
    }

    #[test]
    fn edge_handle_scales_one_axis() {
        let xf = scaled(Affine2::IDENTITY, BOX, (1, 0), Vec2::new(300.0, 999.0), false, false);
        close(xf.transform_point2(Vec2::new(100.0, 100.0)), Vec2::new(300.0, 100.0));
    }

    #[test]
    fn scale_never_collapses() {
        let xf = scaled(Affine2::IDENTITY, BOX, (1, 1), Vec2::ZERO, false, false);
        assert!(xf.matrix2.determinant().abs() > 0.0);
    }

    #[test]
    fn rotation_follows_pointer_and_snaps() {
        let pivot = Vec2::splat(50.0);
        let xf = rotated(Affine2::IDENTITY, pivot, Vec2::new(100.0, 50.0), Vec2::new(50.0, 100.0), false);
        close(xf.transform_point2(Vec2::new(100.0, 50.0)), Vec2::new(50.0, 100.0));

        let to = pivot + Vec2::from_angle(20f32.to_radians()) * 50.0;
        let xf = rotated(Affine2::IDENTITY, pivot, Vec2::new(100.0, 50.0), to, true);
        let (_, angle, _) = xf.to_scale_angle_translation();
        assert!((angle.to_degrees() - 15.0).abs() < 1e-3);
    }

    #[test]
    fn scaling_after_rotation_uses_the_rotated_frame() {
        let pivot = Vec2::splat(50.0);
        let r = rotated(Affine2::IDENTITY, pivot, Vec2::new(100.0, 50.0), Vec2::new(50.0, 100.0), false);
        // After +90°, the box's right edge handle sits below the center.
        let handle = r.transform_point2(Vec2::new(100.0, 50.0));
        close(handle, Vec2::new(50.0, 100.0));
        let xf = scaled(r, BOX, (1, 0), Vec2::new(50.0, 150.0), false, false);
        close(xf.transform_point2(Vec2::new(100.0, 50.0)), Vec2::new(50.0, 150.0));
    }

    #[test]
    fn begin_skips_locked_and_apply_restore_round_trip() {
        let mut p = Project::empty();
        let a = p.new_surface_id();
        let b = p.new_surface_id();
        p.surfaces.push(Surface::rect(a, "a", Vec2::ZERO, Vec2::splat(100.0)));
        p.surfaces.push(Surface::rect(b, "b", Vec2::splat(500.0), Vec2::splat(100.0)));
        p.surfaces[1].locked = true;

        let mut tf = FreeTransform::begin(&p, &[a, b]).unwrap();
        assert_eq!(tf.bounds(), (Vec2::ZERO, Vec2::splat(100.0)));
        tf.xf = moved(tf.xf, Vec2::new(10.0, 0.0));
        tf.apply(&mut p);
        assert_eq!(p.surfaces[0].corners[0], Vec2::new(10.0, 0.0));
        assert_eq!(p.surfaces[1].corners[0], Vec2::splat(500.0));
        tf.restore(&mut p);
        assert_eq!(p.surfaces[0].corners[0], Vec2::ZERO);

        p.surfaces[0].locked = true;
        assert!(FreeTransform::begin(&p, &[a, b]).is_none());
    }
}
