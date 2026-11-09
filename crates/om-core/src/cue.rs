//! Cues: saved looks of the show, played one after another with GO.
//!
//! A cue remembers how every surface *looks* (source, colors, opacity, blend,
//! border, animation, visibility…), never where it is or what it is
//! (corners, shape, mesh warp, name). Playing a cue changes the looks with a
//! crossfade:
//!
//! - numbers glide (opacity, tint, feather, border width, texture offset…);
//! - anything that can't glide (another source or blend mode, visibility,
//!   animation speeds, which would make a time-based pattern jump) is
//!   crossfaded by drawing the surface twice: the old look, and the new one
//!   fading in on top.
//!
//! Looks are stored as the surface's serialized fields, so new `Surface`
//! fields become part of cues automatically. Fields that describe geometry
//! or identity must be listed in [`NOT_IN_LOOKS`].

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::project::{Project, Surface, SurfaceId};

/// A surface's look: its serialized fields minus [`NOT_IN_LOOKS`].
pub type Look = Map<String, Value>;

/// Surface fields a cue never touches: what the surface is and where it is.
pub const NOT_IN_LOOKS: [&str; 8] = ["id", "name", "locked", "corners", "mask", "path", "warp", "cutout"];

/// Fields that never glide even though they are numbers: speeds of
/// time-based motion (interpolating them makes the pattern jump).
const NO_GLIDE: [&str; 2] = ["animation", "flow"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CueId(pub u64);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cue {
    pub id: CueId,
    pub name: String,
    /// Crossfade time in seconds when the cue is played.
    pub fade: f32,
    /// Play the next cue automatically this many seconds after this one
    /// started; `None` waits for GO.
    pub follow: Option<f32>,
    pub looks: Vec<SurfaceLook>,
}

impl Default for Cue {
    fn default() -> Self {
        Self { id: CueId(0), name: String::new(), fade: 1.0, follow: None, looks: Vec::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurfaceLook {
    pub surface: SurfaceId,
    pub look: Look,
}

impl Cue {
    /// How every surface looks right now.
    pub fn record(id: CueId, name: impl Into<String>, project: &Project) -> Self {
        let looks =
            project.surfaces.iter().map(|s| SurfaceLook { surface: s.id, look: look_of(s) }).collect();
        Self { id, name: name.into(), looks, ..Default::default() }
    }

    /// `project` with this cue's looks applied. Surfaces the cue doesn't
    /// know (added later) keep their look.
    pub fn apply(&self, project: &Project) -> Project {
        let mut out = project.clone();
        for s in &mut out.surfaces {
            if let Some(l) = self.looks.iter().find(|l| l.surface == s.id) {
                *s = with_look(s, &l.look);
            }
        }
        out
    }

    /// Whether `project` looks exactly like this cue (nothing changed since
    /// it was recorded or played).
    pub fn matches(&self, project: &Project) -> bool {
        project
            .surfaces
            .iter()
            .all(|s| self.looks.iter().find(|l| l.surface == s.id).is_none_or(|l| l.look == look_of(s)))
    }
}

pub fn look_of(s: &Surface) -> Look {
    let Ok(Value::Object(mut fields)) = serde_json::to_value(s) else { return Look::new() };
    for key in NOT_IN_LOOKS {
        fields.remove(key);
    }
    fields
}

/// `s` with its look replaced by `look` (geometry and identity untouched).
pub fn with_look(s: &Surface, look: &Look) -> Surface {
    let Ok(Value::Object(mut fields)) = serde_json::to_value(s) else { return s.clone() };
    for (key, value) in look {
        if !NOT_IN_LOOKS.contains(&key.as_str()) {
            fields.insert(key.clone(), value.clone());
        }
    }
    serde_json::from_value(Value::Object(fields)).unwrap_or_else(|_| s.clone())
}

/// What to draw for one surface `t` (0..1) of the way from look `from` to
/// look `to`. Geometry always comes from `to`. One surface when the change
/// can glide; two (old look under, new look fading in) when it can't.
pub fn transition(from: &Surface, to: &Surface, t: f32) -> Vec<Surface> {
    let (a, b) = (look_of(from), look_of(to));
    if t >= 1.0 || a == b {
        return vec![to.clone()];
    }
    if t <= 0.0 {
        return vec![with_look(to, &a)];
    }
    if let Some(Value::Object(mixed)) = glide(&Value::Object(a.clone()), &Value::Object(b), t, "") {
        return vec![with_look(to, &mixed)];
    }
    let mut old = with_look(to, &a);
    let mut new = to.clone();
    if !new.visible {
        // Fading out to nothing.
        old.opacity *= 1.0 - t;
        return vec![old];
    }
    new.opacity *= t;
    if !old.visible {
        return vec![new];
    }
    vec![old, new]
}

/// The whole project `t` of the way from `from`'s looks to `to`'s. For
/// rendering a crossfade; `to` provides everything but the old looks.
pub fn crossfade(from: &Project, to: &Project, t: f32) -> Project {
    let mut out = to.clone();
    out.surfaces = to
        .surfaces
        .iter()
        .flat_map(|s| match from.surface(s.id) {
            Some(f) => transition(f, s, t),
            None => vec![s.clone()],
        })
        .collect();
    out
}

/// Eases a linear 0..1 fade (slow start and end, like a lighting desk).
pub fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Interpolates two JSON values, or `None` when they differ in a way that
/// can't glide. Only floating-point numbers glide: integers are ids or counts.
fn glide(a: &Value, b: &Value, t: f32, key: &str) -> Option<Value> {
    if a == b {
        return Some(b.clone());
    }
    if NO_GLIDE.contains(&key) {
        return None;
    }
    match (a, b) {
        (Value::Number(x), Value::Number(y)) if x.is_f64() || y.is_f64() => {
            let (x, y) = (x.as_f64()?, y.as_f64()?);
            serde_json::Number::from_f64(x + (y - x) * t as f64).map(Value::Number)
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            x.iter().zip(y).map(|(x, y)| glide(x, y, t, key)).collect::<Option<Vec<_>>>().map(Value::Array)
        }
        (Value::Object(x), Value::Object(y)) if x.len() == y.len() => {
            let mut out = Map::new();
            for (k, xv) in x {
                out.insert(k.clone(), glide(xv, y.get(k)?, t, k)?);
            }
            Some(Value::Object(out))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec2;

    use super::*;
    use crate::{BlendMode, SourceKind};

    fn project() -> Project {
        let mut p = Project::empty();
        let red = p.add_source("red", SourceKind::Solid { color: [1.0, 0.0, 0.0, 1.0] });
        p.add_source("blue", SourceKind::Solid { color: [0.0, 0.0, 1.0, 1.0] });
        let mut s = Surface::rect(SurfaceId(10), "wall", Vec2::ZERO, Vec2::splat(100.0));
        s.source = Some(red);
        p.surfaces.push(s);
        p
    }

    #[test]
    fn a_cue_restores_looks_but_not_geometry() {
        let mut p = project();
        let cue = Cue::record(CueId(1), "Opening", &p);
        assert!(cue.matches(&p));
        p.surfaces[0].opacity = 0.3;
        p.surfaces[0].corners[0] = Vec2::new(-5.0, -5.0);
        p.surfaces[0].name = "renamed".into();
        assert!(!cue.matches(&p));
        let back = cue.apply(&p);
        assert_eq!(back.surfaces[0].opacity, 1.0);
        assert_eq!(back.surfaces[0].corners[0], Vec2::new(-5.0, -5.0), "geometry stays");
        assert_eq!(back.surfaces[0].name, "renamed");
    }

    #[test]
    fn numbers_glide() {
        let from = project();
        let mut to = from.clone();
        to.surfaces[0].opacity = 0.0;
        to.surfaces[0].tint = [0.0, 0.5, 1.0, 1.0];
        let mid = crossfade(&from, &to, 0.25);
        assert_eq!(mid.surfaces.len(), 1);
        assert!((mid.surfaces[0].opacity - 0.75).abs() < 1e-6);
        assert!((mid.surfaces[0].tint[1] - 0.875).abs() < 1e-6);
        assert_eq!(crossfade(&from, &to, 1.0).surfaces[0].opacity, 0.0);
    }

    #[test]
    fn a_new_source_fades_in_on_top_of_the_old_one() {
        let from = project();
        let mut to = from.clone();
        let blue = to.sources[1].id;
        to.surfaces[0].source = Some(blue);
        to.surfaces[0].blend = BlendMode::Screen;
        let mid = crossfade(&from, &to, 0.4);
        assert_eq!(mid.surfaces.len(), 2);
        let (old, new) = (&mid.surfaces[0], &mid.surfaces[1]);
        assert_eq!((old.source, old.opacity, old.blend), (from.surfaces[0].source, 1.0, BlendMode::Normal));
        assert_eq!((new.source, new.blend), (Some(blue), BlendMode::Screen));
        assert!((new.opacity - 0.4).abs() < 1e-6);
    }

    #[test]
    fn hiding_fades_out_and_showing_fades_in() {
        let from = project();
        let mut to = from.clone();
        to.surfaces[0].visible = false;
        let out = crossfade(&from, &to, 0.25);
        assert_eq!(out.surfaces.len(), 1);
        assert!(out.surfaces[0].visible && (out.surfaces[0].opacity - 0.75).abs() < 1e-6);
        let back = crossfade(&to, &from, 0.25);
        assert_eq!(back.surfaces.len(), 1);
        assert!((back.surfaces[0].opacity - 0.25).abs() < 1e-6);
    }

    #[test]
    fn animation_speeds_switch_instead_of_gliding() {
        let from = project();
        let mut to = from.clone();
        to.surfaces[0].animation.spin = 1.0;
        assert_eq!(crossfade(&from, &to, 0.5).surfaces.len(), 2);
        assert_eq!(
            glide(&serde_json::json!(1.0), &serde_json::json!(3.0), 0.5, "x"),
            Some(serde_json::json!(2.0))
        );
        assert_eq!(glide(&serde_json::json!(1), &serde_json::json!(3), 0.5, "x"), None, "integers are ids");
    }

    #[test]
    fn surfaces_a_cue_does_not_know_keep_their_look() {
        let mut p = project();
        let cue = Cue::record(CueId(1), "Opening", &p);
        let mut extra = Surface::rect(SurfaceId(11), "new", Vec2::ZERO, Vec2::ONE);
        extra.opacity = 0.5;
        p.surfaces.push(extra);
        assert_eq!(cue.apply(&p).surfaces[1].opacity, 0.5);
        assert!(cue.matches(&p), "an unknown surface doesn't make the look differ");
        let from = Project { surfaces: vec![], ..p.clone() };
        assert_eq!(crossfade(&from, &p, 0.5).surfaces.len(), 2);
    }

    #[test]
    fn easing_is_slow_at_both_ends() {
        assert_eq!((ease(0.0), ease(0.5), ease(1.0), ease(2.0)), (0.0, 0.5, 1.0, 1.0));
        assert!(ease(0.1) < 0.1 && ease(0.9) > 0.9);
    }

    #[test]
    fn looks_skip_geometry_fields() {
        let look = look_of(&project().surfaces[0]);
        for key in NOT_IN_LOOKS {
            assert!(!look.contains_key(key), "{key}");
        }
        assert!(look.contains_key("opacity") && look.contains_key("source") && look.contains_key("visible"));
    }
}
