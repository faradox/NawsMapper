//! Borders (strokes) around shapes: style, animated effects, and how
//! patterns are laid out along the outline.
//!
//! By default a border is drawn *inside* the shape's edge, so it never spills
//! off the object it is mapped onto; it can also straddle the edge or sit
//! outside it. Widths and lengths are canvas pixels.

use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::Color;
use crate::geom::Homography;
use crate::project::Palette;
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

/// What a border is painted with. Serialized by name; new effects go at the end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderEffect {
    /// One color.
    #[default]
    Color,
    /// The palette running along the outline.
    Gradient,
    /// Twinkling stars travelling along the border.
    Stars,
    /// Bright heads with fading tails.
    Comets,
    /// Two strands twisting around each other.
    Helix,
    /// Bursts of sparks along the border.
    Fireworks,
}

impl BorderEffect {
    pub const ALL: [BorderEffect; 6] = [
        BorderEffect::Color,
        BorderEffect::Gradient,
        BorderEffect::Stars,
        BorderEffect::Comets,
        BorderEffect::Helix,
        BorderEffect::Fireworks,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BorderEffect::Color => "Color",
            BorderEffect::Gradient => "Gradient",
            BorderEffect::Stars => "Stars",
            BorderEffect::Comets => "Comets",
            BorderEffect::Helix => "Helix",
            BorderEffect::Fireworks => "Fireworks",
        }
    }

    /// Index used by `border_paint` in surface.wgsl.
    pub fn shader_index(self) -> u32 {
        match self {
            BorderEffect::Color => 0,
            BorderEffect::Gradient => 1,
            BorderEffect::Stars => 2,
            BorderEffect::Comets => 3,
            BorderEffect::Helix => 4,
            BorderEffect::Fireworks => 5,
        }
    }

    /// Line effects take the border style (dashes, dots…); the others are
    /// made of moving things and ignore it.
    pub fn uses_style(self) -> bool {
        matches!(self, BorderEffect::Color | BorderEffect::Gradient)
    }

    /// What the `length` setting means for this effect, if anything.
    pub fn length_label(self) -> Option<&'static str> {
        match self {
            BorderEffect::Color => None,
            BorderEffect::Gradient => Some("Gradient length"),
            BorderEffect::Helix => Some("Twist length"),
            BorderEffect::Stars | BorderEffect::Comets | BorderEffect::Fireworks => Some("Spacing"),
        }
    }

    /// How far (in border widths) the effect reaches beyond its band.
    fn reach(self) -> f32 {
        match self {
            BorderEffect::Color | BorderEffect::Gradient => 0.0,
            BorderEffect::Comets | BorderEffect::Helix => 0.5,
            BorderEffect::Stars => 1.5,
            BorderEffect::Fireworks => 5.0,
        }
    }
}

/// Where the border sits relative to the shape's edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderPosition {
    #[default]
    Inside,
    Center,
    Outside,
}

impl BorderPosition {
    pub const ALL: [BorderPosition; 3] =
        [BorderPosition::Inside, BorderPosition::Center, BorderPosition::Outside];

    pub fn label(self) -> &'static str {
        match self {
            BorderPosition::Inside => "Inside",
            BorderPosition::Center => "Center",
            BorderPosition::Outside => "Outside",
        }
    }
}

/// A one-click border look: name, hint, and what it changes.
pub type BorderPreset = (&'static str, &'static str, fn(&mut Border));

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Border {
    pub enabled: bool,
    /// Color of the `Color` effect; with the two-color palette, the first color.
    pub color: Color,
    /// Second color of the two-color palette.
    pub color_b: Color,
    /// Canvas pixels.
    pub width: f32,
    pub position: BorderPosition,
    pub effect: BorderEffect,
    /// Colors of every effect but `Color`.
    pub palette: Palette,
    /// Neon glow around the border, 0..1.
    pub glow: f32,
    pub style: BorderStyle,
    /// Length of a dash, pixels.
    pub dash: f32,
    /// Space between dashes or dots, pixels.
    pub gap: f32,
    /// Repeat length of the effect along the outline (gradient length,
    /// spacing of stars…), pixels.
    pub length: f32,
    /// Everything on the border travels around the shape at this many
    /// pixels per second (negative: the other way).
    pub flow: f32,
}

impl Default for Border {
    fn default() -> Self {
        Self {
            enabled: false,
            color: [1.0; 4],
            color_b: [1.0, 0.2, 0.6, 1.0],
            width: 6.0,
            position: BorderPosition::Inside,
            effect: BorderEffect::Color,
            palette: Palette::Rainbow,
            glow: 0.0,
            style: BorderStyle::Solid,
            dash: 24.0,
            gap: 12.0,
            length: 300.0,
            flow: 0.0,
        }
    }
}

impl Border {
    /// The band the border covers, as (from, to) pixels measured inward
    /// from the shape's edge (negative is outside the shape).
    pub fn band(&self) -> (f32, f32) {
        let w = self.width.max(0.0);
        match self.position {
            BorderPosition::Inside => (0.0, w),
            BorderPosition::Center => (-w * 0.5, w * 0.5),
            BorderPosition::Outside => (-w, 0.0),
        }
    }

    /// Distance over which the glow fades, pixels.
    pub fn glow_radius(&self) -> f32 {
        if self.glow <= 0.0 { 0.0 } else { self.glow.min(1.0) * (self.width * 4.0).max(30.0) }
    }

    /// How far beyond its band the border paints anything (glow, sparks…).
    pub fn reach(&self) -> f32 {
        self.glow_radius().max(self.effect.reach() * self.width)
    }

    /// How far outside the shape's edge the border paints, pixels. The
    /// renderer grows the surface's draw area by this much.
    pub fn outer_extent(&self) -> f32 {
        if !self.enabled || self.width <= 0.0 {
            return 0.0;
        }
        let (from, _) = self.band();
        let reach = self.reach();
        let extent = (-from).max(0.0) + reach;
        if extent > 0.0 { extent + 2.0 } else { 0.0 }
    }

    /// Whether the shader needs the distance along the outline.
    pub fn needs_path(&self) -> bool {
        self.effect != BorderEffect::Color || self.style.is_broken()
    }

    /// One-click looks, `(name, hint, apply)`. Each sets only what defines the look.
    pub fn presets() -> [BorderPreset; 7] {
        fn base(b: &mut Border, effect: BorderEffect, palette: Palette, width: f32, glow: f32) {
            b.enabled = true;
            b.effect = effect;
            b.palette = palette;
            b.width = width;
            b.glow = glow;
            b.style = BorderStyle::Solid;
            b.position = BorderPosition::Center;
        }
        [
            ("Neon", "A glowing neon tube with colors flowing along it", |b| {
                base(b, BorderEffect::Gradient, Palette::Neon, 6.0, 0.6);
                (b.length, b.flow) = (500.0, 80.0);
            }),
            ("Rainbow", "A rainbow running around the shape", |b| {
                base(b, BorderEffect::Gradient, Palette::Rainbow, 10.0, 0.15);
                (b.length, b.flow) = (600.0, 150.0);
            }),
            ("Marching ants", "Dashes walking around the shape", |b| {
                base(b, BorderEffect::Color, b.palette, 3.0, 0.0);
                b.style = BorderStyle::Dashed;
                (b.dash, b.gap, b.flow) = (12.0, 8.0, 30.0);
            }),
            ("Stars", "Twinkling stars flowing around the border", |b| {
                base(b, BorderEffect::Stars, Palette::Gold, 18.0, 0.3);
                (b.length, b.flow) = (90.0, 40.0);
            }),
            ("Comets", "Comets with glowing tails racing around", |b| {
                base(b, BorderEffect::Comets, Palette::Fire, 10.0, 0.3);
                (b.length, b.flow) = (320.0, 260.0);
            }),
            ("Helix", "Two strands twirling around the edge", |b| {
                base(b, BorderEffect::Helix, Palette::Neon, 24.0, 0.4);
                (b.length, b.flow) = (120.0, 30.0);
            }),
            ("Fireworks", "Bursts of sparks all along the border", |b| {
                base(b, BorderEffect::Fireworks, Palette::Rainbow, 10.0, 0.2);
                (b.length, b.flow) = (140.0, 0.0);
            }),
        ]
    }

    /// Length of one repeat of the line style along the outline in pixels,
    /// `0` for continuous styles and for effects that ignore the style.
    /// Dots are as wide as the border.
    pub fn period(&self) -> f32 {
        if !self.effect.uses_style() {
            return 0.0;
        }
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
    fn style_and_effect_indices_are_unique() {
        for (i, s) in BorderStyle::ALL.iter().enumerate() {
            assert_eq!(s.shader_index(), i as u32);
        }
        for (i, e) in BorderEffect::ALL.iter().enumerate() {
            assert_eq!(e.shader_index(), i as u32);
        }
    }

    #[test]
    fn plain_inside_border_stays_inside() {
        let b = Border { enabled: true, ..Default::default() };
        assert_eq!(b.band(), (0.0, 6.0));
        assert_eq!(b.outer_extent(), 0.0, "no need to draw outside the shape");
        assert!(!b.needs_path());
    }

    #[test]
    fn glow_and_position_reach_outside() {
        let mut b = Border { enabled: true, width: 10.0, ..Default::default() };
        b.position = BorderPosition::Center;
        assert_eq!(b.band(), (-5.0, 5.0));
        assert_eq!(b.outer_extent(), 7.0);
        b.glow = 0.5;
        assert_eq!(b.outer_extent(), 5.0 + 20.0 + 2.0);
        b.position = BorderPosition::Outside;
        b.glow = 0.0;
        b.effect = BorderEffect::Fireworks;
        assert_eq!(b.outer_extent(), 10.0 + 50.0 + 2.0);
        b.enabled = false;
        assert_eq!(b.outer_extent(), 0.0);
    }

    #[test]
    fn effects_ignore_the_line_style() {
        let mut b = Border { style: BorderStyle::Dashed, ..Default::default() };
        assert!(b.period() > 0.0);
        b.effect = BorderEffect::Stars;
        assert_eq!(b.period(), 0.0);
        assert!(b.needs_path());
    }

    #[test]
    fn presets_enable_the_border() {
        for (name, _, apply) in Border::presets() {
            let mut b = Border::default();
            apply(&mut b);
            assert!(b.enabled && b.width > 0.0, "{name}");
        }
    }
}
