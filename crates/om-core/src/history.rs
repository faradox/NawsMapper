//! Snapshot-based undo/redo.
//!
//! Projects are small (a few KB of JSON-equivalent data), so storing whole
//! snapshots is simpler and more robust than a command pattern. The app
//! records a snapshot whenever the project changed and the user finished an
//! interaction (pointer released), which coalesces drags and slider moves
//! into a single undo step.

use crate::project::Project;

#[derive(Debug, Clone)]
pub struct History {
    undo: Vec<Project>,
    redo: Vec<Project>,
    limit: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::new(200)
    }
}

impl History {
    pub fn new(limit: usize) -> Self {
        Self { undo: Vec::new(), redo: Vec::new(), limit }
    }

    /// Records the state *before* a change.
    pub fn record(&mut self, before: Project) {
        self.undo.push(before);
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Returns the state to restore, given the current one.
    pub fn undo(&mut self, current: &Project) -> Option<Project> {
        let prev = self.undo.pop()?;
        self.redo.push(current.clone());
        Some(prev)
    }

    pub fn redo(&mut self, current: &Project) -> Option<Project> {
        let next = self.redo.pop()?;
        self.undo.push(current.clone());
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_round_trip() {
        let mut h = History::default();
        let a = Project::empty();
        let mut b = a.clone();
        b.canvas.width = 1234;

        h.record(a.clone());
        let restored = h.undo(&b).unwrap();
        assert_eq!(restored, a);
        let again = h.redo(&restored).unwrap();
        assert_eq!(again, b);
        assert!(!h.can_redo());
    }
}
