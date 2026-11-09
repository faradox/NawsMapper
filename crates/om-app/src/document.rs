//! The open document: project data, file path, undo history and the edit
//! operations the UI triggers.

use std::path::PathBuf;

use glam::Vec2;
use om_core::{
    Cue, CueId, Generator, History, Mask, PathPoint, Pattern, Project, SourceId, SourceKind, Surface,
    SurfaceId,
};

pub struct Document {
    pub project: Project,
    /// Last state recorded in history; compared against `project` to detect edits.
    committed: Project,
    history: History,
    pub path: Option<PathBuf>,
    pub dirty: bool,
}

impl Document {
    pub fn new(project: Project, path: Option<PathBuf>) -> Self {
        Self { committed: project.clone(), project, history: History::default(), path, dirty: false }
    }

    /// A document restored after a crash: it belongs to `path` but has
    /// changes that were never saved there.
    pub fn restored(project: Project, path: Option<PathBuf>) -> Self {
        Self { dirty: true, ..Self::new(project, path) }
    }

    /// Records an undo step if the project changed since the last commit.
    /// Call once per frame while no pointer interaction is in progress so
    /// drags and slider moves become a single step.
    pub fn commit_if_changed(&mut self) -> bool {
        if self.project == self.committed {
            return false;
        }
        let before = std::mem::replace(&mut self.committed, self.project.clone());
        self.history.record(before);
        self.dirty = true;
        true
    }

    pub fn undo(&mut self) -> bool {
        self.commit_if_changed();
        match self.history.undo(&self.project) {
            Some(p) => {
                self.project = p.clone();
                self.committed = p;
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.history.redo(&self.project) {
            Some(p) => {
                self.project = p.clone();
                self.committed = p;
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo() || self.project != self.committed
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn display_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into())
    }

    pub fn save_to(&mut self, path: PathBuf) -> anyhow::Result<()> {
        self.project.save(&path)?;
        self.path = Some(path);
        self.dirty = false;
        Ok(())
    }

    // ---- Edit operations -------------------------------------------------

    /// The source new surfaces get: the test grid, created on demand, so a
    /// fresh surface is immediately visible and easy to align.
    fn default_source(&mut self) -> SourceId {
        let existing = self
            .project
            .sources
            .iter()
            .find(|s| matches!(&s.kind, SourceKind::Generator(g) if g.pattern == Pattern::TestGrid));
        if let Some(s) = existing {
            return s.id;
        }
        self.project.add_source(
            "Test Grid",
            SourceKind::Generator(Generator {
                pattern: Pattern::TestGrid,
                color_a: [0.0, 0.0, 0.0, 1.0],
                color_b: [1.0; 4],
                scale: 8.0,
                speed: 0.0,
                angle: 0.0,
                ..Default::default()
            }),
        )
    }

    /// Adds a new surface centered on the canvas (nudged so repeated adds
    /// don't stack exactly) and returns its id.
    pub fn add_surface(&mut self, mask: Mask) -> SurfaceId {
        let source = self.default_source();
        let p = &mut self.project;
        let canvas = p.canvas.size();
        let size = Vec2::new(canvas.x * 0.3, canvas.y * 0.36);
        let step = (p.surfaces.len() % 8) as f32 * 24.0;
        let min = (canvas - size) * 0.5 + Vec2::splat(step);
        let id = p.new_surface_id();
        let name = p.unique_surface_name(mask.label());
        let mut s = Surface::rect(id, name, min, size);
        s.mask = mask;
        s.source = Some(source);
        p.surfaces.push(s);
        id
    }

    /// Adds a cutout on top of the stack: a quad that punches a hole in
    /// everything below it.
    pub fn add_cutout(&mut self) -> SurfaceId {
        let id = self.add_surface(Mask::Quad);
        let name = self.project.unique_surface_name("Cutout");
        let s = self.project.surface_mut(id).expect("just added");
        s.cutout = true;
        s.source = None;
        s.name = name;
        id
    }

    /// Adds a free-form surface through `points` (canvas pixels) on top.
    pub fn add_freeform(&mut self, points: &[PathPoint]) -> Option<SurfaceId> {
        let source = self.default_source();
        let id = self.project.new_surface_id();
        let name = self.project.unique_surface_name("Shape");
        let mut s = Surface::freeform(id, name, points)?;
        s.source = Some(source);
        self.project.surfaces.push(s);
        Some(id)
    }

    /// Adds a source made from a shader library preset.
    pub fn add_generator(&mut self, name: &str, generator: Generator) -> SourceId {
        self.project.add_source(name, SourceKind::Generator(generator))
    }

    /// Duplicates surfaces (placed above the originals, slightly offset).
    pub fn duplicate(&mut self, ids: &[SurfaceId]) -> Vec<SurfaceId> {
        let mut new_ids = Vec::new();
        for &id in ids {
            let Some(idx) = self.project.surface_index(id) else { continue };
            let mut copy = self.project.surfaces[idx].clone();
            copy.id = self.project.new_surface_id();
            copy.name = self.project.unique_surface_name(&format!("{} copy", copy.name));
            copy.translate(Vec2::splat(24.0));
            new_ids.push(copy.id);
            self.project.surfaces.insert(idx + 1, copy);
        }
        new_ids
    }

    pub fn delete(&mut self, ids: &[SurfaceId]) {
        self.project.surfaces.retain(|s| !ids.contains(&s.id));
    }

    /// Moves the given surfaces one step toward the front (`+1`) or back (`-1`).
    pub fn restack(&mut self, ids: &[SurfaceId], dir: i32) {
        let s = &mut self.project.surfaces;
        let sel = |s: &Surface| ids.contains(&s.id);
        if dir > 0 {
            for i in (0..s.len().saturating_sub(1)).rev() {
                if sel(&s[i]) && !sel(&s[i + 1]) {
                    s.swap(i, i + 1);
                }
            }
        } else {
            for i in 1..s.len() {
                if sel(&s[i]) && !sel(&s[i - 1]) {
                    s.swap(i, i - 1);
                }
            }
        }
    }

    /// Moves surfaces all the way to the front or back, keeping their order.
    pub fn restack_extreme(&mut self, ids: &[SurfaceId], front: bool) {
        let (mut picked, mut rest): (Vec<_>, Vec<_>) =
            self.project.surfaces.drain(..).partition(|s| ids.contains(&s.id));
        if front {
            rest.append(&mut picked);
            self.project.surfaces = rest;
        } else {
            picked.append(&mut rest);
            self.project.surfaces = picked;
        }
    }

    /// Gives each surface (in the order given — i.e. the order they were
    /// selected) an increasing animation time offset, creating a chase.
    pub fn stagger(&mut self, ids: &[SurfaceId], step: f32) {
        for (i, id) in ids.iter().enumerate() {
            if let Some(s) = self.project.surface_mut(*id) {
                s.animation.time_offset = i as f32 * step;
            }
        }
    }

    // ---- Cues -------------------------------------------------------------

    /// Records how every surface looks now as a new cue, after `after` (or
    /// at the end of the list). Returns its id.
    pub fn record_cue(&mut self, after: Option<CueId>) -> CueId {
        let id = self.project.new_cue_id();
        let name = (self.project.cues.len() + 1..)
            .map(|n| format!("Cue {n}"))
            .find(|n| !self.project.cues.iter().any(|c| &c.name == n))
            .expect("unbounded");
        let cue = Cue::record(id, name, &self.project);
        let at = after.and_then(|a| self.project.cue_index(a)).map_or(self.project.cues.len(), |i| i + 1);
        self.project.cues.insert(at, cue);
        id
    }

    /// Replaces a cue's looks with the current ones (name, fade and follow stay).
    pub fn update_cue(&mut self, id: CueId) {
        let looks = Cue::record(id, "", &self.project).looks;
        if let Some(i) = self.project.cue_index(id) {
            self.project.cues[i].looks = looks;
        }
    }

    pub fn delete_cue(&mut self, id: CueId) {
        self.project.cues.retain(|c| c.id != id);
    }

    /// Moves a cue one place earlier (-1) or later (+1) in the list.
    pub fn move_cue(&mut self, id: CueId, dir: i32) {
        let Some(i) = self.project.cue_index(id) else { return };
        let j = i as i64 + dir.signum() as i64;
        if (0..self.project.cues.len() as i64).contains(&j) {
            self.project.cues.swap(i, j as usize);
        }
    }

    pub fn assign_source(&mut self, ids: &[SurfaceId], source: Option<SourceId>) {
        for id in ids {
            if let Some(s) = self.project.surface_mut(*id) {
                s.source = source;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(doc: &Document) -> Vec<u64> {
        doc.project.surfaces.iter().map(|s| s.id.0).collect()
    }

    #[test]
    fn add_cutout_goes_on_top_without_source() {
        let mut doc = Document::new(Project::empty(), None);
        doc.add_surface(Mask::Quad);
        let id = doc.add_cutout();
        let top = doc.project.surfaces.last().unwrap();
        assert_eq!(top.id, id);
        assert!(top.cutout);
        assert_eq!(top.source, None);
        assert_eq!(top.name, "Cutout 1");
    }

    #[test]
    fn add_freeform_needs_three_points() {
        let mut doc = Document::new(Project::empty(), None);
        let pts: Vec<PathPoint> = [(10.0, 10.0), (200.0, 40.0), (90.0, 150.0)]
            .map(|(x, y)| PathPoint::corner(glam::Vec2::new(x, y)))
            .to_vec();
        assert!(doc.add_freeform(&pts[..2]).is_none());
        let id = doc.add_freeform(&pts).unwrap();
        let s = doc.project.surface(id).unwrap();
        assert_eq!((s.name.as_str(), s.mask), ("Shape 1", Mask::Freeform));
        assert!(s.source.is_some());
    }

    #[test]
    fn restack_moves_selection_together() {
        let mut doc = Document::new(Project::empty(), None);
        let a = doc.add_surface(Mask::Quad);
        let b = doc.add_surface(Mask::Quad);
        let c = doc.add_surface(Mask::Quad);
        doc.restack(&[a, b], 1);
        assert_eq!(ids(&doc), vec![c.0, a.0, b.0]);
        doc.restack_extreme(&[b], false);
        assert_eq!(ids(&doc), vec![b.0, c.0, a.0]);
    }

    #[test]
    fn edits_commit_and_undo() {
        let mut doc = Document::new(Project::empty(), None);
        doc.add_surface(Mask::Quad);
        assert!(doc.commit_if_changed());
        assert!(!doc.commit_if_changed());
        assert!(doc.undo());
        assert!(doc.project.surfaces.is_empty());
        assert!(doc.redo());
        assert_eq!(doc.project.surfaces.len(), 1);
    }

    #[test]
    fn duplicate_places_copy_above_original() {
        let mut doc = Document::new(Project::empty(), None);
        let a = doc.add_surface(Mask::Quad);
        let b = doc.add_surface(Mask::Quad);
        let copies = doc.duplicate(&[a]);
        assert_eq!(ids(&doc), vec![a.0, copies[0].0, b.0]);
    }

    #[test]
    fn cues_are_recorded_in_place_named_and_updated() {
        let mut doc = Document::new(Project::empty(), None);
        let s = doc.add_surface(Mask::Quad);
        let first = doc.record_cue(None);
        let last = doc.record_cue(Some(first));
        // Recorded after the first cue: goes between, named by count.
        let between = doc.record_cue(Some(first));
        let names: Vec<&str> = doc.project.cues.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Cue 1", "Cue 3", "Cue 2"]);
        assert_eq!(doc.project.cues[1].id, between);
        assert_eq!(doc.project.cues[2].id, last);

        doc.project.surface_mut(s).unwrap().opacity = 0.25;
        assert!(!doc.project.cue(first).unwrap().matches(&doc.project));
        doc.update_cue(first);
        assert!(doc.project.cue(first).unwrap().matches(&doc.project));
        assert_eq!(doc.project.cue(first).unwrap().name, "Cue 1");

        doc.move_cue(last, -1);
        doc.move_cue(first, -1);
        let order: Vec<CueId> = doc.project.cues.iter().map(|c| c.id).collect();
        assert_eq!(order, [first, last, between], "first can't move further up");
        doc.delete_cue(last);
        assert_eq!(doc.project.cues.len(), 2);
    }
}
