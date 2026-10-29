//! Geometry helpers: perspective (homography) mapping between the unit square
//! and a four-corner surface in canvas pixel space.

use glam::{Mat3, Vec2, Vec3};

/// A projective transform that maps the unit square onto a quad.
///
/// Corner order everywhere in NawsMapper is: top-left, top-right,
/// bottom-right, bottom-left, matching unit-square UVs
/// `(0,0) (1,0) (1,1) (0,1)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Homography {
    /// Unit square -> quad.
    pub forward: Mat3,
    /// Quad -> unit square. Falls back to identity for degenerate quads.
    pub inverse: Mat3,
}

impl Homography {
    /// Builds the square-to-quad mapping (Heckbert, "Fundamentals of Texture
    /// Mapping and Image Warping", 1989).
    pub fn from_corners(c: [Vec2; 4]) -> Self {
        let [p0, p1, p2, p3] = c;
        let d1 = p1 - p2;
        let d2 = p3 - p2;
        let s = p0 - p1 + p2 - p3;

        let (g, h) = if s.x.abs() < 1e-9 && s.y.abs() < 1e-9 {
            (0.0, 0.0)
        } else {
            let det = d1.x * d2.y - d2.x * d1.y;
            if det.abs() < 1e-12 {
                (0.0, 0.0)
            } else {
                ((s.x * d2.y - d2.x * s.y) / det, (d1.x * s.y - s.x * d1.y) / det)
            }
        };

        let a = p1.x - p0.x + g * p1.x;
        let b = p3.x - p0.x + h * p3.x;
        let d = p1.y - p0.y + g * p1.y;
        let e = p3.y - p0.y + h * p3.y;

        let forward = Mat3::from_cols(Vec3::new(a, d, g), Vec3::new(b, e, h), Vec3::new(p0.x, p0.y, 1.0));
        let inverse = if forward.determinant().abs() > 1e-12 { forward.inverse() } else { Mat3::IDENTITY };
        Self { forward, inverse }
    }

    /// Maps a unit-square UV to canvas space.
    pub fn map(&self, uv: Vec2) -> Vec2 {
        project(self.forward * uv.extend(1.0))
    }

    /// Maps a canvas point back to unit-square UV.
    pub fn unmap(&self, p: Vec2) -> Vec2 {
        project(self.inverse * p.extend(1.0))
    }
}

fn project(v: Vec3) -> Vec2 {
    if v.z.abs() < 1e-12 { Vec2::splat(f32::NAN) } else { v.truncate() / v.z }
}

/// True when `p` lies inside the (convex or concave) polygon.
pub fn point_in_polygon(p: Vec2, poly: &[Vec2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Approximate width / height of a quad, used to keep texture rotation
/// from shearing on non-square surfaces.
pub fn quad_aspect(c: [Vec2; 4]) -> f32 {
    let w = (c[0].distance(c[1]) + c[3].distance(c[2])) * 0.5;
    let h = (c[0].distance(c[3]) + c[1].distance(c[2])) * 0.5;
    if h < 1e-3 { 1.0 } else { (w / h).max(1e-3) }
}

/// Center of a quad (average of its corners).
pub fn quad_center(c: [Vec2; 4]) -> Vec2 {
    (c[0] + c[1] + c[2] + c[3]) * 0.25
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(a: Vec2, b: Vec2) {
        assert!(a.distance(b) < 1e-2, "{a:?} != {b:?}");
    }

    #[test]
    fn maps_unit_corners_to_quad_corners() {
        let quad = [
            Vec2::new(100.0, 50.0),
            Vec2::new(700.0, 120.0),
            Vec2::new(650.0, 500.0),
            Vec2::new(80.0, 420.0),
        ];
        let h = Homography::from_corners(quad);
        let uvs = [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y];
        for (uv, p) in uvs.iter().zip(quad) {
            assert_close(h.map(*uv), p);
            assert_close(h.unmap(p), *uv);
        }
    }

    #[test]
    fn affine_quad_round_trips() {
        let quad =
            [Vec2::new(0.0, 0.0), Vec2::new(200.0, 0.0), Vec2::new(200.0, 100.0), Vec2::new(0.0, 100.0)];
        let h = Homography::from_corners(quad);
        assert_close(h.map(Vec2::splat(0.5)), Vec2::new(100.0, 50.0));
        assert_close(h.unmap(Vec2::new(50.0, 25.0)), Vec2::new(0.25, 0.25));
    }

    #[test]
    fn degenerate_quad_does_not_panic() {
        let h = Homography::from_corners([Vec2::ZERO; 4]);
        assert_eq!(h.inverse, Mat3::IDENTITY);
    }

    #[test]
    fn point_in_polygon_works() {
        let sq = [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y];
        assert!(point_in_polygon(Vec2::splat(0.5), &sq));
        assert!(!point_in_polygon(Vec2::new(1.5, 0.5), &sq));
    }
}
