//! Autosave for crash recovery. While there are unsaved changes, the
//! project is written to `recovery.json` in the app's data folder every
//! [`INTERVAL`]. The file is removed once the changes are saved or
//! discarded, so finding it at startup means the last session ended without
//! that: the app offers to restore it.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use om_core::Project;
use serde::{Deserialize, Serialize};

pub const INTERVAL: Duration = Duration::from_secs(30);
const FILE_NAME: &str = "recovery.json";

/// What the recovery file holds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recovered {
    /// The file the project belongs to, if it had been saved before.
    pub original: Option<PathBuf>,
    /// Seconds since 1970 when it was written.
    pub saved_at: u64,
    pub project: Project,
}

impl Recovered {
    pub fn name(&self) -> String {
        self.original
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into())
    }

    /// "12 minutes ago", "3 hours ago", "2 days ago".
    pub fn age(&self, now: u64) -> String {
        let secs = now.saturating_sub(self.saved_at);
        let (n, unit) = match secs {
            0..120 => return "just now".into(),
            120..7200 => (secs / 60, "minutes"),
            7200..172800 => (secs / 3600, "hours"),
            _ => (secs / 86400, "days"),
        };
        format!("{n} {unit} ago")
    }
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// File operations, done in order on one worker thread: a slow disk never
/// stalls the frame loop, and a removal can't be overtaken by an earlier write.
enum Op {
    Write(String),
    Remove,
}

pub struct Autosave {
    file: Option<PathBuf>,
    last_write: Option<Instant>,
    /// The project as last written, to skip writing the same thing again.
    written: Option<Project>,
    worker: Option<Sender<Op>>,
}

impl Autosave {
    /// Recovery file inside `dir` (the app's data folder); `None` disables autosave.
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self { file: dir.map(|d| d.join(FILE_NAME)), last_write: None, written: None, worker: None }
    }

    /// The recovery file left by a previous session, if any.
    pub fn find(&self) -> Option<Recovered> {
        let file = self.file.as_ref()?;
        let json = std::fs::read_to_string(file).ok()?;
        match serde_json::from_str(&json) {
            Ok(r) => Some(r),
            Err(e) => {
                log::warn!("ignoring unreadable recovery file {}: {e}", file.display());
                None
            }
        }
    }

    /// Call every frame. Writes the project when it has unsaved changes and
    /// the last write is [`INTERVAL`] old; removes the file once everything is saved.
    pub fn tick(&mut self, project: &Project, path: Option<&Path>, dirty: bool) {
        if !dirty {
            if self.written.is_some() {
                self.clear();
            }
            return;
        }
        let due = self.last_write.is_none_or(|t| t.elapsed() >= INTERVAL);
        if !due || self.written.as_ref() == Some(project) {
            return;
        }
        self.last_write = Some(Instant::now());
        self.written = Some(project.clone());
        let recovered =
            Recovered { original: path.map(Path::to_path_buf), saved_at: now(), project: project.clone() };
        match serde_json::to_string(&recovered) {
            Ok(json) => self.send(Op::Write(json)),
            Err(e) => log::error!("autosave failed: {e}"),
        }
    }

    /// Removes the recovery file: the changes were saved or deliberately discarded.
    pub fn clear(&mut self) {
        self.written = None;
        self.last_write = None;
        self.send(Op::Remove);
    }

    fn send(&mut self, op: Op) {
        let Some(file) = self.file.clone() else { return };
        if self.worker.is_none() {
            let (tx, rx) = channel::<Op>();
            let spawned = std::thread::Builder::new().name("autosave".into()).spawn(move || {
                for op in rx {
                    let result = match op {
                        Op::Write(json) => write_atomically(&file, json.as_bytes()),
                        Op::Remove if file.exists() => std::fs::remove_file(&file),
                        Op::Remove => Ok(()),
                    };
                    if let Err(e) = result {
                        log::error!("autosave ({}): {e}", file.display());
                    }
                }
            });
            match spawned {
                Ok(_) => self.worker = Some(tx),
                Err(e) => return log::error!("autosave thread failed to start: {e}"),
            }
        }
        if let Some(worker) = &self.worker {
            let _ = worker.send(op);
        }
    }
}

/// Writes to a temporary file and renames it, so a crash mid-write never
/// leaves half a recovery file.
fn write_atomically(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait_for(file: &Path, present: bool) {
        for _ in 0..300 {
            if file.exists() == present {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("{} never became {}", file.display(), if present { "present" } else { "absent" });
    }

    #[test]
    fn writes_unsaved_changes_and_clears_after_saving() {
        let dir = std::env::temp_dir().join(format!("om-recovery-test-{}", std::process::id()));
        let file = dir.join(FILE_NAME);
        let mut autosave = Autosave::new(Some(dir.clone()));
        let project = Project::demo();
        let path = Path::new("/shows/facade.omap");

        autosave.tick(&project, Some(path), false);
        assert!(!file.exists(), "nothing unsaved, nothing written");

        autosave.tick(&project, Some(path), true);
        wait_for(&file, true);
        let found = autosave.find().unwrap();
        assert_eq!(found.project, project);
        assert_eq!(found.original.as_deref(), Some(path));
        assert_eq!(found.name(), "facade");

        autosave.tick(&project, Some(path), false);
        wait_for(&file, false);
        assert!(autosave.find().is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn waits_for_the_interval_between_writes() {
        let mut autosave = Autosave::new(None);
        autosave.last_write = Some(Instant::now());
        autosave.tick(&Project::demo(), None, true);
        assert!(autosave.written.is_none(), "too early");
        autosave.last_write = Some(Instant::now() - INTERVAL);
        autosave.tick(&Project::demo(), None, true);
        assert!(autosave.written.is_some());
    }

    #[test]
    fn age_reads_naturally() {
        let r = Recovered { original: None, saved_at: 1000, project: Project::empty() };
        assert_eq!(r.name(), "Untitled");
        assert_eq!(r.age(1030), "just now");
        assert_eq!(r.age(1000 + 12 * 60), "12 minutes ago");
        assert_eq!(r.age(1000 + 3 * 3600), "3 hours ago");
        assert_eq!(r.age(1000 + 2 * 86400), "2 days ago");
    }
}
