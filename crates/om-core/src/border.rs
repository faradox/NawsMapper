//! Borders (strokes) around shapes: style, and how dashes are laid out along
//! the outline.
//!
//! A border is drawn *inside* the shape's edge, so it never spills off the
//! object it is mapped onto. Widths and dash lengths are canvas pixels.

use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::Color;
use crate::geom::Homography;
use crate::shape::{MAX_OUTLINE_POINTS, Mask, PathPoint, outline_uv};

/// Most pieces one outline edge is split into (see [`border_path`]).
const MAX_EDGE_PIECES: usize = 16;

/// Line style of a border. Serialized by name; new styles go at the end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
    /// Two thin lines.
    Double,
    DashDot,
}

impl BorderStyle {
    pub const ALL: [BorderStyle; 5] = [
        BorderStyle::Solid,
        BorderStyle::Dashed,
        BorderStyle::Dotted,
        BorderStyle::Double,
        BorderStyle::DashDot,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BorderStyle::Solid => "Solid",
            BorderStyle::Dashed => "Dashed",
            BorderStyle::Dotted => "Dotted",
            BorderStyle::Double => "Double",
            BorderStyle::DashDot => "Dash-dot",
        }
    }

    /// Index used by `border_coverage` in surface.wgsl.
    pub fn shader_index(self) -> u32 {
        match self {
            BorderStyle::Solid => 0,
            BorderStyle::Dashed => 1,
            BorderStyle::Dotted => 2,
            BorderStyle::Double => 3,
            BorderStyle::DashDot => 4,
        }
    }

    /// Styles that repeat along the outline (and can flow around it).
    pub fn is_broken(self) -> bool {
        matches!(self, BorderStyle::Dashed | BorderStyle::Dotted | BorderStyle::DashDot)
    }

    /// Styles whose dash length matters.
    pub fn has_dashes(self) -> bool {
        matches!(self, BorderStyle::Dashed | BorderStyle::DashDot)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Border {
    pub enabled: bool,
    pub color: Color,
    /// Canvas pixels, measured inward from the shape's edge.
    pub width: f32,
    pub style: BorderStyle,
    /// Length of a dash, pixels.
    pub dash: f32,
    /// Space between dashes or dots, pixels.
    pub gap: f32,
    /// Dashes and dots travel around the shape at this many pixels per
    /// second (negative: the other way).
    pub flow: f32,
}

impl Default for Border {
    fn default() -> Self {
        Self {
            enabled: false,
            color: [1.0; 4],
            width: 6.0,
            style: BorderStyle::Solid,
            dash: 24.0,
            gap: 12.0,
            flow: 0.0,
        }
    }
}

impl Border {
    /// Length of one repeat of the pattern along the outline in pixels,
    /// `0` for continuous styles. Dots are as wide as the border.
    pub fn period(&self) -> f32 {
        let (w, dash, gap) = (self.width.max(0.5), self.dash.max(0.5), self.gap.max(0.0));
        match self.style {
            BorderStyle::Solid | BorderStyle::Double => 0.0,
            BorderStyle::Dashed => dash + gap,
            BorderStyle::Dotted => w + gap,
            BorderStyle::DashDot => dash + w + 2.0 * gap,
        }
    }
}

/// The outline a broken border runs along.
#[derive(Clone, Debug, PartialEq)]
pub struct BorderPath {
    /// The shape's outline in its unit square, long edges subdivided.
    pub points: Vec<Vec2>,
    /// Distance along the outline at each point, in (stretched) pixels.
    pub distances: Vec<f32>,
    /// Total length of the closed outline, in (stretched) pixels.
    pub perimeter: f32,
}

/// The outline of a shape with the distance along it at every point.
///
/// Edges are split in proportion to their length on the canvas (at most
/// [`MAX_OUTLINE_POINTS`] points in total), so dashes stay evenly spaced
/// under perspective. With a `period`, distances are stretched slightly so a
/// whole number of periods fits around the shape and the pattern closes
/// without a seam.
pub fn border_path(mask: Mask, path: &[PathPoint], corners: [Vec2; 4], period: f32) -> BorderPath {
    let outline = outline_uv(mask, path);
    let h = Homography::from_corners(corners);
    let n = outline.len();
    let edge_len = |i: usize| h.map(outline[i]).distance(h.map(outline[(i + 1) % n]));
    let total: f32 = (0..n).map(edge_len).sum();

    let mut points = Vec::with_capacity(MAX_OUTLINE_POINTS);
    let budget = MAX_OUTLINE_POINTS.saturating_sub(n);
    for i in 0..n {
        let extra = if total > 0.0 { (budget as f32 * edge_len(i) / total) as usize } else { 0 };
        let pieces = (1 + extra).min(MAX_EDGE_PIECES);
        // Equal steps on the canvas (straight edges stay straight under a homography).
        let (a, b) = (h.map(outline[i]), h.map(outline[(i + 1) % n]));
        points.push(outline[i]);
        points.extend((1..pieces).map(|k| h.unmap(a.lerp(b, k as f32 / pieces as f32))));
    }

    let mut distances = Vec::with_capacity(points.len());
    let mut along = 0.0;
    for (i, p) in points.iter().enumerate() {
        distances.push(along);
        along += h.map(*p).distance(h.map(points[(i + 1) % points.len()]));
    }
    let mut perimeter = along;
    if period > 0.0 && along > 0.0 {
        perimeter = (along / period).round().max(1.0) * period;
        let stretch = perimeter / along;
        distances.iter_mut().for_each(|d| *d *= stretch);
    }
    BorderPath { points, distances, perimeter }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(size: f32) -> [Vec2; 4] {
        [Vec2::ZERO, Vec2::new(size, 0.0), Vec2::splat(size), Vec2::new(0.0, size)]
    }

    #[test]
    fn whole_periods_fit_around_the_shape() {
        let b = border_path(Mask::Quad, &[], square(100.0), 30.0);
        assert_eq!(b.perimeter, 390.0, "400 px of outline holds 13 periods of 30 px");
        assert_eq!(b.distances[0], 0.0);
        assert!(b.distances.windows(2).all(|w| w[0] < w[1]));
        assert!(*b.distances.last().unwrap() < b.perimeter);
    }

    #[test]
    fn continuous_styles_keep_the_true_length() {
        let b = border_path(Mask::Quad, &[], square(100.0), 0.0);
        assert!((b.perimeter - 400.0).abs() < 1e-3);
    }

    #[test]
    fn subdivided_outline_keeps_the_shape() {
        let b = border_path(Mask::Quad, &[], square(100.0), 0.0);
        assert!(b.points.len() > 4 && b.points.len() <= MAX_OUTLINE_POINTS);
        for p in &b.points {
            let on_edge =
                p.x.abs() < 1e-5 || p.y.abs() < 1e-5 || (p.x - 1.0).abs() < 1e-5 || (p.y - 1.0).abs() < 1e-5;
            assert!(on_edge, "{p}");
        }
        let star = border_path(Mask::Star, &[], square(100.0), 10.0);
        assert!(star.points.len() <= MAX_OUTLINE_POINTS);
        assert!(star.points.len() > outline_uv(Mask::Star, &[]).len());
    }

    #[test]
    fn dashes_stay_even_under_perspective() {
        // Strong keystone: the bottom edge is 4× the top one.
        let c =
            [Vec2::new(150.0, 0.0), Vec2::new(250.0, 0.0), Vec2::new(400.0, 300.0), Vec2::new(0.0, 300.0)];
        let b = border_path(Mask::Quad, &[], c, 0.0);
        let h = Homography::from_corners(c);
        // The shader interpolates distances linearly in the unit square
        // within each piece; that must stay close to the true distance.
        let n = b.points.len();
        for i in 0..n {
            let (a, z) = (b.points[i], b.points[(i + 1) % n]);
            let len = h.map(a).distance(h.map(z));
            let to_mid = h.map(a).distance(h.map(a.lerp(z, 0.5)));
            assert!((to_mid - len * 0.5).abs() < 1.5, "piece {i}: {to_mid} vs {}", len * 0.5);
        }
    }

    #[test]
    fn period_per_style() {
        let mut b = Border { width: 4.0, dash: 20.0, gap: 10.0, ..Default::default() };
        assert_eq!(b.period(), 0.0);
        b.style = BorderStyle::Dashed;
        assert_eq!(b.period(), 30.0);
        b.style = BorderStyle::Dotted;
        assert_eq!(b.period(), 14.0);
        b.style = BorderStyle::DashDot;
        assert_eq!(b.period(), 44.0);
    }

    #[test]
    fn style_indices_are_unique() {
        for (i, s) in BorderStyle::ALL.iter().enumerate() {
            assert_eq!(s.shader_index(), i as u32);
        }
    }
}
