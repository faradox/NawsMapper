//! Texture animation: per-surface motion (scroll, spin), LFO modulation
//! (zoom pulse, opacity pulse / strobe) and hue cycling.
//!
//! Everything is a pure function of time so playback, scrubbing and the
//! projector output always agree.

use glam::{Mat3, Vec2};
use serde::{Deserialize, Serialize};

use crate::geom::{Homography, quad_aspect};
use crate::project::{Mapping, Surface};

/// LFO waveform.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wave {
    #[default]
    Sine,
    Triangle,
    Saw,
    Square,
}

impl Wave {
    pub const ALL: [Wave; 4] = [Wave::Sine, Wave::Triangle, Wave::Saw, Wave::Square];

    pub fn label(self) -> &'static str {
        match self {
            Wave::Sine => "Sine",
            Wave::Triangle => "Triangle",
            Wave::Saw => "Saw",
            Wave::Square => "Square",
        }
    }

    /// Unipolar value in `0..=1` at `phase` (in cycles).
    pub fn sample(self, phase: f32) -> f32 {
        let p = phase.rem_euclid(1.0);
        match self {
            Wave::Sine => 0.5 - 0.5 * (p * std::f32::consts::TAU).cos(),
            Wave::Triangle => 1.0 - (2.0 * p - 1.0).abs(),
            Wave::Saw => p,
            Wave::Square => {
                if p < 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }
}

/// Low-frequency oscillator. `amount == 0` disables it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Lfo {
    pub wave: Wave,
    /// Cycles per second.
    pub rate: f32,
    /// Modulation depth; meaning depends on the target.
    pub amount: f32,
}

impl Default for Lfo {
    fn default() -> Self {
        Self { wave: Wave::Sine, rate: 0.5, amount: 0.0 }
    }
}

impl Lfo {
    /// Unipolar output in `0..=amount`.
    pub fn unipolar(&self, t: f32) -> f32 {
        self.wave.sample(t * self.rate) * self.amount
    }

    /// Bipolar output in `-amount..=amount`.
    pub fn bipolar(&self, t: f32) -> f32 {
        (self.wave.sample(t * self.rate) * 2.0 - 1.0) * self.amount
    }
}

/// Per-surface animation settings.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Animation {
    /// Texture scroll in texture-widths per second.
    pub scroll: Vec2,
    /// Texture rotation in turns per second.
    pub spin: f32,
    /// Zoom pulse: scale is multiplied by `1 + bipolar(t)`.
    pub zoom: Lfo,
    /// Opacity pulse: opacity is multiplied by `1 - unipolar(t)`.
    /// A square wave makes a strobe.
    pub pulse: Lfo,
    /// Hue rotation in turns per second.
    pub hue_speed: f32,
    /// Seconds added to the clock for this surface. Staggering this across
    /// several surfaces creates chase effects.
    pub time_offset: f32,
    /// Local playback speed multiplier.
    pub speed: f32,
}

impl Default for Animation {
    fn default() -> Self {
        Self {
            scroll: Vec2::ZERO,
            spin: 0.0,
            zoom: Lfo::default(),
            pulse: Lfo { wave: Wave::Square, rate: 2.0, amount: 0.0 },
            hue_speed: 0.0,
            time_offset: 0.0,
            speed: 1.0,
        }
    }
}

impl Animation {
    pub fn local_time(&self, t: f32) -> f32 {
        (t + self.time_offset) * self.speed
    }

    pub fn is_static(&self) -> bool {
        self.scroll == Vec2::ZERO
            && self.spin == 0.0
            && self.zoom.amount == 0.0
            && self.pulse.amount == 0.0
            && self.hue_speed == 0.0
    }
}

/// Static texture placement on a surface (before animation).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextureTransform {
    pub offset: Vec2,
    pub scale: Vec2,
    /// Degrees.
    pub rotation: f32,
    pub flip_x: bool,
    pub flip_y: bool,
}

impl Default for TextureTransform {
    fn default() -> Self {
        Self { offset: Vec2::ZERO, scale: Vec2::ONE, rotation: 0.0, flip_x: false, flip_y: false }
    }
}

/// Everything the renderer needs to draw one surface at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceFrame {
    /// Canvas pixels -> surface UV (unit square).
    pub canvas_to_surface: Mat3,
    /// Base UV (surface or canvas, see [`Mapping`]) -> texture UV.
    pub uv_matrix: Mat3,
    pub opacity: f32,
    /// Hue rotation in turns.
    pub hue_shift: f32,
    /// Surface-local animation clock in seconds (drives generators).
    pub local_time: f32,
}

/// Evaluates a surface's animation at global time `t` (seconds).
pub fn evaluate(surface: &Surface, canvas_size: Vec2, t: f32) -> SurfaceFrame {
    let anim = &surface.animation;
    let lt = anim.local_time(t);
    let tex = &surface.texture;

    let aspect = match surface.mapping {
        Mapping::Surface => quad_aspect(surface.corners),
        Mapping::Canvas => canvas_size.x / canvas_size.y.max(1.0),
    };

    let zoom = (1.0 + anim.zoom.bipolar(lt)).max(0.01);
    let scale = (tex.scale * zoom).max(Vec2::splat(1e-3));
    let rotation = tex.rotation.to_radians() + anim.spin * lt * std::f32::consts::TAU;
    let offset = tex.offset + anim.scroll * lt;
    let flip = Vec2::new(if tex.flip_x { -1.0 } else { 1.0 }, if tex.flip_y { -1.0 } else { 1.0 });

    // Read right-to-left: center the UV, undo the surface aspect so rotation
    // is rigid, rotate, zoom, restore aspect, flip, un-center, then scroll.
    let uv_matrix = Mat3::from_translation(Vec2::splat(0.5) - offset)
        * Mat3::from_scale(flip)
        * Mat3::from_scale(Vec2::new(1.0 / aspect, 1.0))
        * Mat3::from_scale(1.0 / scale)
        * Mat3::from_angle(-rotation)
        * Mat3::from_scale(Vec2::new(aspect, 1.0))
        * Mat3::from_translation(Vec2::splat(-0.5));

    let opacity = surface.opacity * (1.0 - anim.pulse.unipolar(lt)).clamp(0.0, 1.0);

    SurfaceFrame {
        canvas_to_surface: Homography::from_corners(surface.corners).inverse,
        uv_matrix,
        opacity,
        hue_shift: (anim.hue_speed * lt).rem_euclid(1.0),
        local_time: lt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{Surface, SurfaceId};

    #[test]
    fn waves_stay_in_range() {
        for w in Wave::ALL {
            for i in 0..100 {
                let v = w.sample(i as f32 * 0.037 - 1.0);
                assert!((0.0..=1.0).contains(&v), "{w:?} {v}");
            }
        }
    }

    #[test]
    fn default_animation_is_identity() {
        let s = Surface::new(SurfaceId(1), "s", [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y]);
        let f = evaluate(&s, Vec2::new(1920.0, 1080.0), 3.7);
        let p = f.uv_matrix.transform_point2(Vec2::new(0.25, 0.75));
        assert!(p.distance(Vec2::new(0.25, 0.75)) < 1e-5);
        assert_eq!(f.opacity, 1.0);
    }

    #[test]
    fn scroll_moves_texture() {
        let mut s = Surface::new(SurfaceId(1), "s", [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y]);
        s.animation.scroll = Vec2::new(0.5, 0.0);
        let f = evaluate(&s, Vec2::ONE, 1.0);
        let p = f.uv_matrix.transform_point2(Vec2::ZERO);
        assert!((p.x - -0.5).abs() < 1e-5);
    }

    #[test]
    fn strobe_toggles_opacity() {
        let mut s = Surface::new(SurfaceId(1), "s", [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y]);
        s.animation.pulse = Lfo { wave: Wave::Square, rate: 1.0, amount: 1.0 };
        assert_eq!(evaluate(&s, Vec2::ONE, 0.25).opacity, 0.0);
        assert_eq!(evaluate(&s, Vec2::ONE, 0.75).opacity, 1.0);
    }
}
