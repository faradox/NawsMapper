//! Cue playback: which cue is live, which one GO plays next, the running
//! crossfade and auto-follow. The cue data lives in the project
//! (`om_core::cue`); this is the show's moment-to-moment state.
//!
//! GO applies the cue's looks to the document right away (one undo step);
//! during the fade, the renderer gets a crossfaded copy instead
//! ([`CuePlayer::render_project`]), so what you edit is always the target.

use std::borrow::Cow;
use std::time::{Duration, Instant};

use om_core::cue::{crossfade, ease};
use om_core::{CueId, Project};

struct Fade {
    from: Project,
    started: Instant,
    secs: f32,
}

#[derive(Default)]
pub struct CuePlayer {
    /// The cue played last.
    pub live: Option<CueId>,
    /// Chosen in the list: GO plays it next instead of the one after `live`.
    pub selected: Option<CueId>,
    fade: Option<Fade>,
    /// When the live cue's follow time runs out.
    follow_at: Option<Instant>,
}

impl CuePlayer {
    /// The cue GO plays: the selected one, or the one after the live cue
    /// (the first when nothing is live, none after the last).
    pub fn next(&self, project: &Project) -> Option<CueId> {
        if let Some(id) = self.selected.filter(|id| Some(*id) != self.live && project.cue(*id).is_some()) {
            return Some(id);
        }
        let index = match self.live.and_then(|id| project.cue_index(id)) {
            Some(i) => i + 1,
            None => 0,
        };
        project.cues.get(index).map(|c| c.id)
    }

    /// Plays cue `id`: applies its looks to `project` and starts the fade
    /// from the old looks. Returns false if there is no such cue.
    pub fn go(&mut self, project: &mut Project, id: CueId, now: Instant) -> bool {
        let Some(cue) = project.cue(id).cloned() else { return false };
        // A GO during a fade starts from what is on the wall right now.
        let from = self.render_project(project, now).into_owned();
        *project = cue.apply(project);
        self.fade = (cue.fade > 0.0).then_some(Fade { from, started: now, secs: cue.fade });
        self.live = Some(id);
        self.selected = None;
        self.follow_at = cue.follow.map(|s| now + Duration::from_secs_f32(s.max(0.0)));
        true
    }

    /// The cue whose turn it is by auto-follow, once.
    pub fn due(&mut self, project: &Project, now: Instant) -> Option<CueId> {
        let at = self.follow_at?;
        if now < at {
            return None;
        }
        self.follow_at = None;
        self.next(project)
    }

    /// What the renderer should draw: the crossfade while one runs.
    pub fn render_project<'a>(&self, project: &'a Project, now: Instant) -> Cow<'a, Project> {
        match self.progress(now) {
            Some(t) => Cow::Owned(crossfade(&self.fade.as_ref().expect("fading").from, project, ease(t))),
            None => Cow::Borrowed(project),
        }
    }

    /// Linear fade progress 0..1, while a fade runs.
    pub fn progress(&self, now: Instant) -> Option<f32> {
        let f = self.fade.as_ref()?;
        let t = now.saturating_duration_since(f.started).as_secs_f32() / f.secs;
        (t < 1.0).then_some(t)
    }

    /// Ends the fade (the target look shows at once) and cancels auto-follow.
    pub fn stop(&mut self) {
        self.fade = None;
        self.follow_at = None;
    }

    /// Forgets everything (another document).
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Seconds until auto-follow plays the next cue.
    pub fn follow_in(&self, now: Instant) -> Option<f32> {
        self.follow_at.map(|at| at.saturating_duration_since(now).as_secs_f32())
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec2;
    use om_core::{Cue, Surface, SurfaceId};

    use super::*;

    /// A project with one surface and three cues: opacity 1.0, 0.5, 0.0.
    fn show() -> Project {
        let mut p = Project::empty();
        p.surfaces.push(Surface::rect(SurfaceId(1), "wall", Vec2::ZERO, Vec2::splat(10.0)));
        for (i, opacity) in [1.0, 0.5, 0.0].into_iter().enumerate() {
            p.surfaces[0].opacity = opacity;
            let id = p.new_cue_id();
            p.cues.push(Cue { fade: 2.0, ..Cue::record(id, format!("Cue {}", i + 1), &p) });
        }
        p.surfaces[0].opacity = 1.0;
        p
    }

    fn secs(s: f32) -> Duration {
        Duration::from_secs_f32(s)
    }

    #[test]
    fn go_steps_through_the_list() {
        let mut p = show();
        let ids: Vec<CueId> = p.cues.iter().map(|c| c.id).collect();
        let mut player = CuePlayer::default();
        let t0 = Instant::now();
        assert_eq!(player.next(&p), Some(ids[0]));
        player.go(&mut p, ids[0], t0);
        assert_eq!(player.next(&p), Some(ids[1]));
        player.go(&mut p, ids[1], t0);
        player.go(&mut p, ids[2], t0);
        assert_eq!(player.next(&p), None, "nothing after the last cue");
        // Choosing a cue in the list makes it next.
        player.selected = Some(ids[0]);
        assert_eq!(player.next(&p), Some(ids[0]));
    }

    #[test]
    fn the_document_takes_the_target_and_the_renderer_fades() {
        let mut p = show();
        let ids: Vec<CueId> = p.cues.iter().map(|c| c.id).collect();
        let mut player = CuePlayer::default();
        let t0 = Instant::now();
        player.go(&mut p, ids[2], t0);
        assert_eq!(p.surfaces[0].opacity, 0.0, "the document is at the target at once");
        let mid = player.render_project(&p, t0 + secs(1.0));
        assert!((mid.surfaces[0].opacity - 0.5).abs() < 1e-3, "{}", mid.surfaces[0].opacity);
        assert_eq!(player.progress(t0 + secs(0.5)), Some(0.25));
        let end = player.render_project(&p, t0 + secs(2.5));
        assert!(matches!(end, Cow::Borrowed(_)), "after the fade: the document itself");
    }

    #[test]
    fn go_during_a_fade_continues_from_what_is_showing() {
        let mut p = show();
        let ids: Vec<CueId> = p.cues.iter().map(|c| c.id).collect();
        let mut player = CuePlayer::default();
        let t0 = Instant::now();
        player.go(&mut p, ids[2], t0); // 1.0 -> 0.0
        player.go(&mut p, ids[0], t0 + secs(1.0)); // from 0.5 (showing) back to 1.0
        let start = player.render_project(&p, t0 + secs(1.0));
        assert!((start.surfaces[0].opacity - 0.5).abs() < 1e-3);
    }

    #[test]
    fn follow_plays_the_next_cue_once() {
        let mut p = show();
        let first = p.cues[0].id;
        p.cues[0].follow = Some(3.0);
        let mut player = CuePlayer::default();
        let t0 = Instant::now();
        player.go(&mut p, first, t0);
        assert_eq!(player.due(&p, t0 + secs(2.9)), None);
        assert_eq!(player.follow_in(t0 + secs(1.0)).map(|s| s.round()), Some(2.0));
        assert_eq!(player.due(&p, t0 + secs(3.0)), Some(p.cues[1].id));
        assert_eq!(player.due(&p, t0 + secs(4.0)), None, "only once");
    }

    #[test]
    fn zero_fade_cuts_and_stop_ends_a_fade() {
        let mut p = show();
        p.cues[1].fade = 0.0;
        let (a, b) = (p.cues[2].id, p.cues[1].id);
        let mut player = CuePlayer::default();
        let t0 = Instant::now();
        player.go(&mut p, b, t0);
        assert_eq!(player.progress(t0), None);
        player.go(&mut p, a, t0);
        assert!(player.progress(t0 + secs(0.1)).is_some());
        player.stop();
        assert_eq!(player.progress(t0 + secs(0.1)), None);
        assert!(!player.go(&mut p, CueId(999), t0), "unknown cue");
    }
}
