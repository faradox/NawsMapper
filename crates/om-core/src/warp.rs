//! Mesh warp: bends a surface with a grid of control points, for curved and
//! uneven objects (columns, arches, round sculptures, fabric).
//!
//! The grid lives in the surface's unit square *before* corner pinning:
//! `canvas = H(warp(uv))`, where `H` is the corners' homography. Shapes,
//! borders and textures are all defined in `uv`, so they bend with the mesh,
//! while moving corners, Free Transform, snapping and welding keep working on
//! the four corners with the mesh riding along. The untouched grid is a
//! regular lattice, which maps every point to itself.
//!
//! Smooth meshes interpolate with Catmull-Rom splines, so the surface passes
//! through every control point and needs no handles (like smooth path points);
//! otherwise the cells are bilinear.

use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

use crate::geom::Homography;

/// Fewest and most control points per row or column.
pub const MIN_POINTS: usize = 2;
pub const MAX_POINTS: usize = 12;
/// The renderer draws a warped surface as this many cells per side.
pub const TESSELLATION: usize = 48;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Warp {
    /// Control points per row.
    pub cols: usize,
    /// Control points per column.
    pub rows: usize,
    /// Row by row, top to bottom, in the unit square before corner pinning.
    pub points: Vec<Vec2>,
    /// Curves through the points; off: straight lines between them.
    pub smooth: bool,
}

impl Default for Warp {
    fn default() -> Self {
        Self::identity(4, 4)
    }
}

/// One vertex of a tessellated warped surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshVertex {
    /// Canvas position in homogeneous coordinates (x·w, y·w, w), w > 0 on the
    /// surface, so the GPU interpolates `uv` with correct perspective.
    pub position: Vec3,
    /// The surface's own unit-square coordinates (outside 0..1 in the margin).
    pub uv: Vec2,
}

impl Warp {
    /// The untouched grid: `cols` × `rows` points on a regular lattice.
    pub fn identity(cols: usize, rows: usize) -> Self {
        let (cols, rows) = (cols.clamp(MIN_POINTS, MAX_POINTS), rows.clamp(MIN_POINTS, MAX_POINTS));
        let points = (0..rows).flat_map(|r| (0..cols).map(move |c| lattice(c, r, cols, rows))).collect();
        Self { cols, rows, points, smooth: true }
    }

    /// Grid size in range and one finite point per grid position.
    pub fn is_valid(&self) -> bool {
        (MIN_POINTS..=MAX_POINTS).contains(&self.cols)
            && (MIN_POINTS..=MAX_POINTS).contains(&self.rows)
            && self.points.len() == self.cols * self.rows
            && self.points.iter().all(|p| p.is_finite())
    }

    /// Every point is still on its lattice position (nothing bent).
    pub fn is_identity(&self) -> bool {
        self.points
            .iter()
            .enumerate()
            .all(|(i, p)| p.distance(lattice(i % self.cols, i / self.cols, self.cols, self.rows)) < 1e-6)
    }

    pub fn index(&self, col: usize, row: usize) -> usize {
        row * self.cols + col
    }

    /// One of the four grid corners. Those stay on the surface's corners;
    /// moving them is corner pinning.
    pub fn is_corner(&self, index: usize) -> bool {
        let (c, r) = (index % self.cols, index / self.cols);
        (c == 0 || c == self.cols - 1) && (r == 0 || r == self.rows - 1)
    }

    /// Maps a unit-square point through the mesh. Outside the unit square the
    /// mesh continues in a straight line (used for borders that paint
    /// outside the shape).
    pub fn map(&self, uv: Vec2) -> Vec2 {
        if !self.is_valid() {
            return uv;
        }
        let inside = uv.clamp(Vec2::ZERO, Vec2::ONE);
        let base = self.eval(inside);
        let off = uv - inside;
        if off == Vec2::ZERO {
            return base;
        }
        // Linear continuation along the edge's slope.
        let step = 1e-3;
        let slope = |axis: Vec2, at: f32| {
            let back = if at > 0.5 { -step } else { step };
            (self.eval(inside + axis * back) - base) / back
        };
        let mut out = base;
        if off.x != 0.0 {
            out += slope(Vec2::X, inside.x) * off.x;
        }
        if off.y != 0.0 {
            out += slope(Vec2::Y, inside.y) * off.y;
        }
        out
    }

    /// The same mesh with a different number of points, keeping its shape.
    pub fn resized(&self, cols: usize, rows: usize) -> Self {
        let mut out = Self::identity(cols, rows);
        out.smooth = self.smooth;
        if self.is_valid() {
            for p in &mut out.points {
                *p = self.map(*p);
            }
        }
        out
    }

    /// The control points on the canvas.
    pub fn canvas_points(&self, corners: [Vec2; 4]) -> Vec<Vec2> {
        let h = Homography::from_corners(corners);
        self.points.iter().map(|p| h.map(*p)).collect()
    }

    /// The surface as a grid of [`TESSELLATION`]² cells, row by row, covering
    /// the unit square grown by `margin` (uv units) on every side. Pair with
    /// [`grid_indices`].
    pub fn tessellate(&self, corners: [Vec2; 4], margin: f32) -> Vec<MeshVertex> {
        let h = Homography::from_corners(corners);
        // Homogeneous coordinates are only defined up to scale: pick the sign
        // that makes w positive on the surface, as the GPU's clipper expects.
        let sign = if (h.forward * Vec3::new(0.5, 0.5, 1.0)).z < 0.0 { -1.0 } else { 1.0 };
        let n = TESSELLATION;
        let (lo, span) = (-margin, 1.0 + 2.0 * margin);
        (0..=n)
            .flat_map(|r| (0..=n).map(move |c| Vec2::new(c as f32, r as f32) / n as f32 * span + lo))
            .map(|uv| MeshVertex { position: h.forward * self.map(uv).extend(1.0) * sign, uv })
            .collect()
    }

    /// Point (col, row), continuing the grid in a straight line past its
    /// edges, which keeps the lattice mapping to itself at the borders.
    fn at(&self, c: isize, r: isize) -> Vec2 {
        let (cn, rn) = (self.cols as isize, self.rows as isize);
        if c < 0 {
            return 2.0 * self.at(0, r) - self.at(1, r);
        }
        if c >= cn {
            return 2.0 * self.at(cn - 1, r) - self.at(cn - 2, r);
        }
        if r < 0 {
            return 2.0 * self.at(c, 0) - self.at(c, 1);
        }
        if r >= rn {
            return 2.0 * self.at(c, rn - 1) - self.at(c, rn - 2);
        }
        self.points[(r * cn + c) as usize]
    }

    /// The mesh inside the unit square.
    fn eval(&self, uv: Vec2) -> Vec2 {
        let cell = |t: f32, n: usize| {
            let g = t * (n - 1) as f32;
            let i = (g.floor() as isize).clamp(0, n as isize - 2);
            (i, g - i as f32)
        };
        let (i, tx) = cell(uv.x, self.cols);
        let (j, ty) = cell(uv.y, self.rows);
        if !self.smooth {
            let top = self.at(i, j).lerp(self.at(i + 1, j), tx);
            let bottom = self.at(i, j + 1).lerp(self.at(i + 1, j + 1), tx);
            return top.lerp(bottom, ty);
        }
        let row = |k: isize| {
            catmull_rom(
                self.at(i - 1, j + k),
                self.at(i, j + k),
                self.at(i + 1, j + k),
                self.at(i + 2, j + k),
                tx,
            )
        };
        catmull_rom(row(-1), row(0), row(1), row(2), ty)
    }
}

/// Triangle indices for a tessellated grid (see [`Warp::tessellate`]).
pub fn grid_indices() -> Vec<u16> {
    let n = TESSELLATION;
    let at = |c: usize, r: usize| (r * (n + 1) + c) as u16;
    (0..n)
        .flat_map(|r| (0..n).map(move |c| (c, r)))
        .flat_map(|(c, r)| {
            [at(c, r), at(c + 1, r), at(c + 1, r + 1), at(c, r), at(c + 1, r + 1), at(c, r + 1)]
        })
        .collect()
}

fn lattice(c: usize, r: usize, cols: usize, rows: usize) -> Vec2 {
    Vec2::new(c as f32 / (cols - 1) as f32, r as f32 / (rows - 1) as f32)
}

/// Uniform Catmull-Rom spline through p1 (t = 0) and p2 (t = 1).
fn catmull_rom(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let (t2, t3) = (t * t, t * t * t);
    0.5 * (2.0 * p1
        + (p2 - p0) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec2, b: Vec2) {
        assert!(a.distance(b) < 1e-4, "{a} != {b}");
    }

    fn samples() -> Vec<Vec2> {
        (0..=10).flat_map(|i| (0..=10).map(move |j| Vec2::new(i as f32, j as f32) / 10.0)).collect()
    }

    #[test]
    fn untouched_mesh_changes_nothing() {
        for (cols, rows) in [(2, 2), (3, 5), (4, 4), (12, 12)] {
            for smooth in [true, false] {
                let w = Warp { smooth, ..Warp::identity(cols, rows) };
                assert!(w.is_identity());
                for uv in samples() {
                    close(w.map(uv), uv);
                }
                // Also past the edges (the margin for outside borders).
                close(w.map(Vec2::new(-0.1, 1.2)), Vec2::new(-0.1, 1.2));
            }
        }
    }

    #[test]
    fn the_surface_passes_through_every_control_point() {
        let mut w = Warp::identity(4, 3);
        w.points[5] += Vec2::new(0.1, -0.05);
        w.points[6] += Vec2::new(-0.07, 0.2);
        for smooth in [true, false] {
            w.smooth = smooth;
            for (i, p) in w.points.iter().enumerate() {
                close(w.map(lattice(i % 4, i / 4, 4, 3)), *p);
            }
        }
        assert!(!w.is_identity());
    }

    #[test]
    fn smooth_bends_softly_linear_bends_at_the_points() {
        // Pull the middle of a 3×3 grid upward.
        let mut w = Warp::identity(3, 3);
        w.points[4].y -= 0.2;
        w.smooth = false;
        // Linear: halfway between the center and the right edge midpoint, the
        // bump has dropped exactly halfway.
        close(w.map(Vec2::new(0.75, 0.5)), Vec2::new(0.75, 0.4));
        w.smooth = true;
        // Smooth: the curve stays higher near the peak.
        assert!(w.map(Vec2::new(0.75, 0.5)).y < 0.4);
    }

    #[test]
    fn resizing_keeps_the_shape() {
        let mut w = Warp::identity(3, 3);
        w.points[4] += Vec2::new(0.1, 0.1);
        w.smooth = false;
        let bigger = w.resized(5, 5);
        assert_eq!((bigger.cols, bigger.rows, bigger.smooth), (5, 5, false));
        for uv in samples() {
            close(bigger.map(uv), w.map(uv));
        }
        assert_eq!(Warp::identity(1, 40).cols, MIN_POINTS);
        assert_eq!(Warp::identity(1, 40).rows, MAX_POINTS);
    }

    #[test]
    fn invalid_meshes_change_nothing() {
        let broken = Warp { cols: 3, rows: 3, points: vec![Vec2::ZERO; 4], smooth: true };
        assert!(!broken.is_valid());
        close(broken.map(Vec2::new(0.3, 0.6)), Vec2::new(0.3, 0.6));
        let default: Warp = serde_json::from_str("{}").unwrap();
        assert!(default.is_valid() && default.is_identity());
    }

    #[test]
    fn corners_are_recognized() {
        let w = Warp::identity(4, 3);
        let corners: Vec<usize> = (0..12).filter(|i| w.is_corner(*i)).collect();
        assert_eq!(corners, [0, 3, 8, 11]);
    }

    #[test]
    fn tessellation_covers_the_surface_with_positive_w() {
        let corners =
            [Vec2::new(100.0, 50.0), Vec2::new(300.0, 80.0), Vec2::new(280.0, 260.0), Vec2::new(90.0, 240.0)];
        // Reversed corner order flips the homogeneous sign; w must stay positive.
        for corners in [corners, [corners[1], corners[0], corners[3], corners[2]]] {
            let verts = Warp::identity(3, 3).tessellate(corners, 0.0);
            let n = TESSELLATION + 1;
            assert_eq!(verts.len(), n * n);
            assert!(verts.iter().all(|v| v.position.z > 0.0));
            let at = |v: &MeshVertex| v.position.truncate() / v.position.z;
            close(at(&verts[0]), corners[0]);
            close(at(&verts[n - 1]), corners[1]);
            close(at(&verts[n * n - 1]), corners[2]);
            close(verts[n * n - 1].uv, Vec2::ONE);
        }
        let grown = Warp::identity(2, 2).tessellate(corners, 0.1);
        close(grown[0].uv, Vec2::splat(-0.1));
        let indices = grid_indices();
        assert_eq!(indices.len(), TESSELLATION * TESSELLATION * 6);
        assert!(indices.iter().all(|i| (*i as usize) < (TESSELLATION + 1).pow(2)));
    }
}
