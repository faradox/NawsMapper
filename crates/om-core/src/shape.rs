//! Surface shapes. Every shape lives in the surface's unit square (UV, y
//! down) and is warped onto the canvas by the surface's four corners, so
//! built-in shapes and free-form paths follow corner pinning and Free
//! Transform alike.
//!
//! Quad, ellipse and triangle are drawn analytically by the shader. Every
//! other shape is a list of anchor points ([`PathPoint`]) that [`flatten`]
//! turns into a polygon, which the shader fills.

use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::geom::Homography;
use crate::project::Surface;

/// Most anchor points a free-form path can have.
pub const MAX_PATH_POINTS: usize = 64;
/// Most points of a flattened outline (what the shader receives).
pub const MAX_OUTLINE_POINTS: usize = 128;
/// Points of the polygon that approximates an ellipse outline.
const ELLIPSE_SEGMENTS: usize = 64;

/// Shape of a surface within its four corners.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mask {
    #[default]
    Quad,
    Ellipse,
    Triangle,
    RoundedRect,
    Diamond,
    Pentagon,
    Hexagon,
    Octagon,
    Star,
    Burst,
    Heart,
    Cross,
    Arrow,
    Bolt,
    /// The surface's own anchor points (`Surface::path`).
    Freeform,
}

impl Mask {
    /// Built-in shapes in menu order (everything except `Freeform`).
    pub const SHAPES: [Mask; 14] = [
        Mask::Quad,
        Mask::Ellipse,
        Mask::Triangle,
        Mask::RoundedRect,
        Mask::Diamond,
        Mask::Pentagon,
        Mask::Hexagon,
        Mask::Octagon,
        Mask::Star,
        Mask::Burst,
        Mask::Heart,
        Mask::Cross,
        Mask::Arrow,
        Mask::Bolt,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Mask::Quad => "Quad",
            Mask::Ellipse => "Ellipse",
            Mask::Triangle => "Triangle",
            Mask::RoundedRect => "Rounded",
            Mask::Diamond => "Diamond",
            Mask::Pentagon => "Pentagon",
            Mask::Hexagon => "Hexagon",
            Mask::Octagon => "Octagon",
            Mask::Star => "Star",
            Mask::Burst => "Burst",
            Mask::Heart => "Heart",
            Mask::Cross => "Cross",
            Mask::Arrow => "Arrow",
            Mask::Bolt => "Bolt",
            Mask::Freeform => "Free-form",
        }
    }

    /// The shader fills these as polygons from [`outline_uv`]; the others
    /// have exact formulas.
    pub fn is_polygon(self) -> bool {
        !matches!(self, Mask::Quad | Mask::Ellipse | Mask::Triangle)
    }

    /// Anchor points of the shape in the unit square. These become the
    /// editable points when a built-in shape is turned into a free-form one.
    /// `Freeform` has no shape of its own and returns the quad.
    pub fn anchors(self) -> Vec<PathPoint> {
        let corners = |pts: &[(f32, f32)]| -> Vec<PathPoint> {
            pts.iter().map(|&(x, y)| PathPoint::corner(Vec2::new(x, y))).collect()
        };
        let mut pts = match self {
            Mask::Quad | Mask::Freeform => corners(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]),
            Mask::Ellipse => (0..8)
                .map(|i| {
                    let a = i as f32 / 8.0 * std::f32::consts::TAU;
                    PathPoint::smooth(Vec2::splat(0.5) + Vec2::from_angle(a) * 0.5)
                })
                .collect(),
            Mask::Triangle => corners(&[(0.5, 0.0), (1.0, 1.0), (0.0, 1.0)]),
            Mask::RoundedRect => rounded_rect(0.2, 5),
            Mask::Diamond => corners(&[(0.5, 0.0), (1.0, 0.5), (0.5, 1.0), (0.0, 0.5)]),
            Mask::Pentagon => star(5, 1.0),
            Mask::Hexagon => star(6, 1.0),
            Mask::Octagon => star(8, 1.0),
            Mask::Star => star(5, 0.4),
            Mask::Burst => star(12, 0.72),
            Mask::Heart => {
                let mut h = corners(&[
                    (0.5, 0.24),
                    (0.76, 0.0),
                    (1.0, 0.27),
                    (0.84, 0.6),
                    (0.5, 1.0),
                    (0.16, 0.6),
                    (0.0, 0.27),
                    (0.24, 0.0),
                ]);
                for i in [1, 2, 3, 5, 6, 7] {
                    h[i].smooth = true;
                }
                h
            }
            Mask::Cross => {
                let (a, b) = (1.0 / 3.0, 2.0 / 3.0);
                corners(&[
                    (a, 0.0),
                    (b, 0.0),
                    (b, a),
                    (1.0, a),
                    (1.0, b),
                    (b, b),
                    (b, 1.0),
                    (a, 1.0),
                    (a, b),
                    (0.0, b),
                    (0.0, a),
                    (a, a),
                ])
            }
            Mask::Arrow => corners(&[
                (0.0, 0.3),
                (0.55, 0.3),
                (0.55, 0.0),
                (1.0, 0.5),
                (0.55, 1.0),
                (0.55, 0.7),
                (0.0, 0.7),
            ]),
            Mask::Bolt => corners(&[
                (0.62, 0.0),
                (0.12, 0.56),
                (0.46, 0.56),
                (0.34, 1.0),
                (0.88, 0.4),
                (0.54, 0.4),
                (0.8, 0.0),
            ]),
        };
        fit_unit(&mut pts);
        pts
    }
}

/// An anchor point of a shape, in the surface's unit square.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PathPoint {
    pub pos: Vec2,
    /// Smooth points round off the outline through them; corner points stay sharp.
    pub smooth: bool,
}

impl PathPoint {
    pub fn corner(pos: Vec2) -> Self {
        Self { pos, smooth: false }
    }

    pub fn smooth(pos: Vec2) -> Self {
        Self { pos, smooth: true }
    }
}

/// Regular polygon (`inner == 1`) or star with `n` tips, first tip pointing up.
fn star(n: usize, inner: f32) -> Vec<PathPoint> {
    let count = if inner < 1.0 { n * 2 } else { n };
    (0..count)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 + i as f32 / count as f32 * std::f32::consts::TAU;
            let r = if inner < 1.0 && i % 2 == 1 { inner * 0.5 } else { 0.5 };
            PathPoint::corner(Vec2::splat(0.5) + Vec2::from_angle(a) * r)
        })
        .collect()
}

/// Rectangle with circular corners of `radius`, each corner arc made of
/// `steps` straight segments.
fn rounded_rect(radius: f32, steps: usize) -> Vec<PathPoint> {
    let centers = [
        (Vec2::new(1.0 - radius, radius), -90.0_f32),
        (Vec2::new(1.0 - radius, 1.0 - radius), 0.0),
        (Vec2::new(radius, 1.0 - radius), 90.0),
        (Vec2::new(radius, radius), 180.0),
    ];
    centers
        .iter()
        .flat_map(|&(c, start)| {
            (0..=steps).map(move |k| {
                let a = (start + 90.0 * k as f32 / steps as f32).to_radians();
                PathPoint::corner(c + Vec2::from_angle(a) * radius)
            })
        })
        .collect()
}

/// Scales and moves `points` so their outline exactly fills the unit square.
fn fit_unit(points: &mut [PathPoint]) {
    let Some((min, max)) = bounds(&flatten(points)) else { return };
    let size = (max - min).max(Vec2::splat(1e-6));
    for p in points {
        p.pos = (p.pos - min) / size;
    }
}

pub fn bounds(points: &[Vec2]) -> Option<(Vec2, Vec2)> {
    let first = *points.first()?;
    Some(points.iter().fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p))))
}

/// The closed outline through `points`, each outline point tagged with the
/// index of the anchor its edge starts at.
///
/// Edges between two corner points are straight. Edges touching a smooth
/// point are cubic Béziers whose tangents come from the neighbors
/// (Catmull-Rom), so the curve passes through every anchor. The result has
/// at most [`MAX_OUTLINE_POINTS`] points.
pub fn flatten_with_edges(points: &[PathPoint]) -> Vec<(Vec2, usize)> {
    let points = &points[..points.len().min(MAX_OUTLINE_POINTS)];
    let n = points.len();
    if n < 3 {
        return points.iter().enumerate().map(|(i, p)| (p.pos, i)).collect();
    }
    let curved = |i: usize| points[i].smooth || points[(i + 1) % n].smooth;
    let curved_edges = (0..n).filter(|&i| curved(i)).count();
    let segments = (MAX_OUTLINE_POINTS - n).checked_div(curved_edges).map_or(1, |s| (s + 1).min(12));
    let tangent = |i: usize| {
        if points[i].smooth {
            (points[(i + 1) % n].pos - points[(i + n - 1) % n].pos) / 6.0
        } else {
            Vec2::ZERO
        }
    };

    let mut out = Vec::with_capacity(n + curved_edges * (segments - 1));
    for i in 0..n {
        let (p0, p3) = (points[i].pos, points[(i + 1) % n].pos);
        out.push((p0, i));
        if curved(i) {
            let (p1, p2) = (p0 + tangent(i), p3 - tangent((i + 1) % n));
            for k in 1..segments {
                let t = k as f32 / segments as f32;
                let u = 1.0 - t;
                let p = p0 * (u * u * u) + p1 * (3.0 * u * u * t) + p2 * (3.0 * u * t * t) + p3 * (t * t * t);
                out.push((p, i));
            }
        }
    }
    out
}

/// The closed outline through `points` (see [`flatten_with_edges`]).
pub fn flatten(points: &[PathPoint]) -> Vec<Vec2> {
    flatten_with_edges(points).into_iter().map(|(p, _)| p).collect()
}

/// Outline of a shape in the surface's unit square.
pub fn outline_uv(mask: Mask, path: &[PathPoint]) -> Vec<Vec2> {
    match mask {
        Mask::Ellipse => (0..ELLIPSE_SEGMENTS)
            .map(|i| {
                let a = i as f32 / ELLIPSE_SEGMENTS as f32 * std::f32::consts::TAU;
                Vec2::splat(0.5) + Vec2::from_angle(a) * 0.5
            })
            .collect(),
        Mask::Freeform if path.len() >= 3 => flatten(path),
        m => flatten(&m.anchors()),
    }
}

impl Surface {
    /// A free-form surface through `points` given in canvas pixels. Its four
    /// corners become the upright bounding box of the outline.
    pub fn freeform(id: crate::SurfaceId, name: impl Into<String>, points: &[PathPoint]) -> Option<Self> {
        let (min, max) = bounds(&flatten(points))?;
        let size = max - min;
        if points.len() < 3 || size.x < 1.0 || size.y < 1.0 {
            return None;
        }
        let mut s = Surface::rect(id, name, min, size);
        s.mask = Mask::Freeform;
        s.path = points.iter().map(|p| PathPoint { pos: (p.pos - min) / size, smooth: p.smooth }).collect();
        Some(s)
    }

    /// The shape's outline in canvas pixels.
    pub fn outline(&self) -> Vec<Vec2> {
        let h = Homography::from_corners(self.corners);
        outline_uv(self.mask, &self.path).into_iter().map(|p| h.map(p)).collect()
    }

    /// Anchor points of a free-form surface in canvas pixels.
    pub fn path_on_canvas(&self) -> Vec<Vec2> {
        let h = Homography::from_corners(self.corners);
        self.path.iter().map(|p| h.map(p.pos)).collect()
    }

    /// Turns a built-in shape into editable anchor points. Free-form
    /// surfaces keep their points.
    pub fn make_freeform(&mut self) {
        if self.mask != Mask::Freeform || self.path.len() < 3 {
            self.path = self.mask.anchors();
            self.mask = Mask::Freeform;
        }
    }

    /// Moves anchor `index` to the canvas point `pos`.
    pub fn move_path_point(&mut self, index: usize, pos: Vec2) {
        let uv = Homography::from_corners(self.corners).unmap(pos);
        if let Some(p) = self.path.get_mut(index)
            && uv.is_finite()
        {
            p.pos = uv;
            self.fit_frame_to_path();
        }
    }

    /// Inserts an anchor at canvas point `pos` after anchor `after`; returns
    /// the new point's index. Does nothing when the path is full.
    pub fn insert_path_point(&mut self, after: usize, pos: Vec2) -> Option<usize> {
        let uv = Homography::from_corners(self.corners).unmap(pos);
        if self.path.len() >= MAX_PATH_POINTS || after >= self.path.len() || !uv.is_finite() {
            return None;
        }
        let n = self.path.len();
        let smooth = self.path[after].smooth && self.path[(after + 1) % n].smooth;
        self.path.insert(after + 1, PathPoint { pos: uv, smooth });
        self.fit_frame_to_path();
        Some(after + 1)
    }

    /// Removes an anchor, keeping at least three.
    pub fn remove_path_point(&mut self, index: usize) -> bool {
        if self.path.len() <= 3 || index >= self.path.len() {
            return false;
        }
        self.path.remove(index);
        self.fit_frame_to_path();
        true
    }

    pub fn toggle_smooth(&mut self, index: usize) {
        if let Some(p) = self.path.get_mut(index) {
            p.smooth = !p.smooth;
            self.fit_frame_to_path();
        }
    }

    /// Moves the four corners onto the bounds of the outline (through the
    /// current perspective) and re-expresses the points in that frame. The
    /// shape stays exactly where it is on the canvas, and it can never
    /// stick out of its frame, which is the area that gets drawn.
    pub fn fit_frame_to_path(&mut self) {
        let Some((min, max)) = bounds(&flatten(&self.path)) else { return };
        let size = max - min;
        if size.x < 1e-4
            || size.y < 1e-4
            || (min.abs().max_element() < 1e-5 && (max - 1.0).abs().max_element() < 1e-5)
        {
            return;
        }
        let h = Homography::from_corners(self.corners);
        self.corners = [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)].map(|uv| h.map(uv));
        for p in &mut self.path {
            p.pos = (p.pos - min) / size;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SurfaceId;
    use crate::geom::point_in_polygon;

    fn close(a: Vec2, b: Vec2) {
        assert!(a.distance(b) < 1e-3, "{a:?} != {b:?}");
    }

    #[test]
    fn built_in_shapes_fill_the_unit_square() {
        for m in Mask::SHAPES {
            let outline = outline_uv(m, &[]);
            assert!(outline.len() >= 3 && outline.len() <= MAX_OUTLINE_POINTS, "{m:?}");
            let (min, max) = bounds(&outline).unwrap();
            assert!(min.abs().max_element() < 1e-3 && (max - 1.0).abs().max_element() < 1e-3, "{m:?}");
            assert!(point_in_polygon(Vec2::new(0.5, 0.55), &outline), "{m:?} center");
        }
    }

    #[test]
    fn corner_paths_flatten_to_their_anchors() {
        let quad = Mask::Quad.anchors();
        assert_eq!(flatten(&quad), quad.iter().map(|p| p.pos).collect::<Vec<_>>());
    }

    #[test]
    fn smooth_points_curve_through_anchors_within_budget() {
        let circle = Mask::Ellipse.anchors();
        let pts = flatten(&circle);
        assert!(pts.len() > circle.len() && pts.len() <= MAX_OUTLINE_POINTS);
        for a in &circle {
            assert!(pts.contains(&a.pos), "curve passes through its anchors");
        }
        // Close to a real circle.
        for p in pts {
            assert!((p.distance(Vec2::splat(0.5)) - 0.5).abs() < 0.01, "{p:?}");
        }
        let many: Vec<PathPoint> = (0..MAX_PATH_POINTS)
            .map(|i| PathPoint::smooth(Vec2::from_angle(i as f32 * 0.1) * (1.0 + i as f32)))
            .collect();
        assert!(flatten(&many).len() <= MAX_OUTLINE_POINTS);
    }

    #[test]
    fn freeform_surface_frames_its_points() {
        let pts =
            [(100.0, 100.0), (300.0, 120.0), (200.0, 260.0)].map(|(x, y)| PathPoint::corner(Vec2::new(x, y)));
        let s = Surface::freeform(SurfaceId(1), "f", &pts).unwrap();
        assert_eq!(s.corners[0], Vec2::new(100.0, 100.0));
        assert_eq!(s.corners[2], Vec2::new(300.0, 260.0));
        for (a, b) in s.path_on_canvas().iter().zip(&pts) {
            close(*a, b.pos);
        }
        assert!(Surface::freeform(SurfaceId(2), "line", &pts[..2]).is_none());
    }

    #[test]
    fn moving_a_point_outside_refits_the_frame_and_keeps_the_others() {
        let mut s = Surface::rect(SurfaceId(1), "s", Vec2::ZERO, Vec2::new(200.0, 100.0));
        s.make_freeform();
        let before = s.path_on_canvas();
        s.move_path_point(2, Vec2::new(300.0, 150.0));
        let after = s.path_on_canvas();
        close(after[2], Vec2::new(300.0, 150.0));
        for i in [0, 1, 3] {
            close(after[i], before[i]);
        }
        assert_eq!(s.corners[2], Vec2::new(300.0, 150.0));
        assert!(s.path.iter().all(|p| p.pos.min_element() >= -1e-5 && p.pos.max_element() <= 1.0 + 1e-5));
    }

    #[test]
    fn refit_is_exact_under_perspective() {
        let mut s = Surface::new(
            SurfaceId(1),
            "p",
            [Vec2::new(0.0, 0.0), Vec2::new(400.0, 40.0), Vec2::new(380.0, 300.0), Vec2::new(20.0, 260.0)],
        );
        s.mask = Mask::Freeform;
        s.path =
            [(0.2, 0.2), (0.7, 0.3), (0.5, 0.8)].map(|(x, y)| PathPoint::corner(Vec2::new(x, y))).to_vec();
        let before = s.path_on_canvas();
        s.fit_frame_to_path();
        for (a, b) in s.path_on_canvas().iter().zip(&before) {
            close(*a, *b);
        }
    }

    #[test]
    fn freeform_paths_survive_save_and_load() {
        let mut p = crate::Project::empty();
        let id = p.new_surface_id();
        let pts = [(0.0, 0.0), (90.0, 10.0), (40.0, 80.0)].map(|(x, y)| PathPoint::smooth(Vec2::new(x, y)));
        p.surfaces.push(Surface::freeform(id, "f", &pts).unwrap());
        let back = crate::Project::from_json(&p.to_json().unwrap()).unwrap();
        assert_eq!(back.surfaces[0].path, p.surfaces[0].path);
        assert_eq!(back.surfaces[0].mask, Mask::Freeform);
    }

    #[test]
    fn insert_and_remove_points() {
        let mut s = Surface::rect(SurfaceId(1), "s", Vec2::ZERO, Vec2::splat(100.0));
        s.mask = Mask::Triangle;
        s.make_freeform();
        assert_eq!(s.path.len(), 3);
        assert!(!s.remove_path_point(0), "keeps at least three points");
        let i = s.insert_path_point(1, Vec2::new(50.0, 100.0)).unwrap();
        assert_eq!((i, s.path.len()), (2, 4));
        close(s.path_on_canvas()[2], Vec2::new(50.0, 100.0));
        assert!(s.remove_path_point(2));
        assert_eq!(s.path.len(), 3);
    }
}
