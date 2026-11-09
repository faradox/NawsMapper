//! Package Project: a self-contained folder with the project file and a copy
//! of every image, video and venue photo it uses (like InDesign's Package),
//! ready to move to the show computer. The open document is not changed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use om_core::{FILE_EXTENSION, Project};

pub const MEDIA_DIR: &str = "Media";

/// What packaging will do.
#[derive(Debug, PartialEq)]
pub struct Plan {
    /// The new folder (does not exist yet).
    pub folder: PathBuf,
    pub project_file: PathBuf,
    /// The project as it is written into the package: media paths are
    /// relative ("Media/photo.jpg"), except for files that are missing.
    pub project: Project,
    /// (original, copy) pairs.
    pub copies: Vec<(PathBuf, PathBuf)>,
    /// Media files that don't exist; their paths are left as they were.
    pub missing: Vec<PathBuf>,
}

/// Plans a package called `name` inside `parent`. `exists` tells which files
/// and folders exist (the file system, or a fake in tests).
pub fn plan(project: &Project, name: &str, parent: &Path, exists: impl Fn(&Path) -> bool) -> Plan {
    let name = sanitize(name);
    let folder = unique(&exists, |n| parent.join(numbered(&name, n)));
    let project_file =
        folder.join(format!("{}.{FILE_EXTENSION}", folder.file_name().unwrap().to_string_lossy()));

    let mut packaged = project.clone();
    let mut copies: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut missing: Vec<PathBuf> = Vec::new();
    // Lowercased file names already used in Media (case-insensitive file systems).
    let mut taken: Vec<String> = Vec::new();
    let mut done: HashMap<PathBuf, PathBuf> = HashMap::new();
    for path in packaged.media_paths_mut() {
        if let Some(relative) = done.get(&*path) {
            *path = relative.clone();
            continue;
        }
        if !exists(path) {
            if !missing.contains(path) {
                missing.push(path.clone());
            }
            continue;
        }
        let file =
            path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_else(|| "media".into());
        let (stem, ext) = split_ext(&file);
        let unique_name = (1..)
            .map(|n| if n == 1 { file.clone() } else { format!("{stem} {n}{ext}") })
            .find(|f| !taken.contains(&f.to_lowercase()))
            .expect("unbounded");
        taken.push(unique_name.to_lowercase());
        let relative = Path::new(MEDIA_DIR).join(&unique_name);
        copies.push((path.clone(), folder.join(&relative)));
        done.insert(path.clone(), relative.clone());
        *path = relative;
    }
    Plan { folder, project_file, project: packaged, copies, missing }
}

fn sanitize(name: &str) -> String {
    let clean: String = name.chars().map(|c| if matches!(c, '/' | '\\' | ':') { '-' } else { c }).collect();
    let clean = clean.trim().to_string();
    if clean.is_empty() { "Untitled".into() } else { clean }
}

fn numbered(name: &str, n: usize) -> String {
    if n == 1 { name.to_string() } else { format!("{name} {n}") }
}

fn unique(exists: impl Fn(&Path) -> bool, path: impl Fn(usize) -> PathBuf) -> PathBuf {
    (1..).map(path).find(|p| !exists(p)).expect("unbounded")
}

/// ("clip", ".mp4"); ("README", "").
fn split_ext(file: &str) -> (&str, &str) {
    match file.rfind('.') {
        Some(i) if i > 0 => (&file[..i], &file[i..]),
        _ => (file, ""),
    }
}

/// Writes a planned package: folders, copies, then the project file.
pub fn execute(plan: &Plan, progress: &AtomicUsize) -> anyhow::Result<()> {
    std::fs::create_dir_all(plan.folder.join(MEDIA_DIR))?;
    for (from, to) in &plan.copies {
        std::fs::copy(from, to).map_err(|e| anyhow::anyhow!("copying {}: {e}", from.display()))?;
        progress.fetch_add(1, Ordering::Relaxed);
    }
    plan.project.save(&plan.project_file)?;
    Ok(())
}

/// A package being written on a background thread.
pub struct Job {
    pub plan_summary: Summary,
    pub copied: Arc<AtomicUsize>,
    rx: Receiver<Result<(), String>>,
}

#[derive(Clone, Debug)]
pub struct Summary {
    pub folder: PathBuf,
    pub files: usize,
    pub missing: Vec<PathBuf>,
}

impl Job {
    pub fn start(plan: Plan, ctx: &egui::Context) -> Self {
        let copied = Arc::new(AtomicUsize::new(0));
        let plan_summary =
            Summary { folder: plan.folder.clone(), files: plan.copies.len(), missing: plan.missing.clone() };
        let (tx, rx) = channel();
        let (progress, ctx) = (copied.clone(), ctx.clone());
        let spawned = std::thread::Builder::new().name("package".into()).spawn(move || {
            let _ = tx.send(execute(&plan, &progress).map_err(|e| format!("{e:#}")));
            ctx.request_repaint();
        });
        if let Err(e) = spawned {
            let (tx, rx2) = channel();
            let _ = tx.send(Err(format!("could not start: {e}")));
            return Self { plan_summary, copied, rx: rx2 };
        }
        Self { plan_summary, copied, rx }
    }

    /// The result once the copying is done.
    pub fn poll(&self) -> Option<Result<(), String>> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("packaging stopped unexpectedly".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use om_core::SourceKind;

    fn exists_in(files: &[&str]) -> impl Fn(&Path) -> bool {
        let files: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
        move |p| files.iter().any(|f| f == p)
    }

    #[test]
    fn media_are_copied_with_unique_names_and_relative_paths() {
        let mut p = Project::empty();
        let a = p.add_source("a", SourceKind::Image { path: "/show/a/photo.png".into() });
        p.add_source("b", SourceKind::Image { path: "/show/b/Photo.png".into() });
        p.add_source("clip", SourceKind::Video { path: "/show/clip.mp4".into() });
        // The same file used twice is copied once.
        p.add_source("a again", SourceKind::Image { path: "/show/a/photo.png".into() });
        p.canvas.backdrop.path = Some("/photos/venue.jpg".into());
        let exists =
            exists_in(&["/show/a/photo.png", "/show/b/Photo.png", "/show/clip.mp4", "/photos/venue.jpg"]);

        let plan = plan(&p, "Facade", Path::new("/usb"), exists);
        assert_eq!(plan.folder, PathBuf::from("/usb/Facade"));
        assert_eq!(plan.project_file, PathBuf::from("/usb/Facade/Facade.omap"));
        let copies: Vec<(&str, &str)> =
            plan.copies.iter().map(|(a, b)| (a.to_str().unwrap(), b.to_str().unwrap())).collect();
        assert_eq!(
            copies,
            [
                ("/show/a/photo.png", "/usb/Facade/Media/photo.png"),
                ("/show/b/Photo.png", "/usb/Facade/Media/Photo 2.png"),
                ("/show/clip.mp4", "/usb/Facade/Media/clip.mp4"),
                ("/photos/venue.jpg", "/usb/Facade/Media/venue.jpg"),
            ]
        );
        let paths: Vec<&Path> = plan.project.sources.iter().filter_map(|s| s.kind.media_path()).collect();
        assert_eq!(
            paths,
            ["Media/photo.png", "Media/Photo 2.png", "Media/clip.mp4", "Media/photo.png"].map(Path::new)
        );
        assert_eq!(plan.project.canvas.backdrop.path, Some("Media/venue.jpg".into()));
        assert!(plan.missing.is_empty());
        // The open project is untouched.
        assert_eq!(p.source(a).unwrap().kind.media_path(), Some(Path::new("/show/a/photo.png")));
    }

    #[test]
    fn missing_files_are_reported_and_kept_as_they_were() {
        let mut p = Project::empty();
        p.add_source("gone", SourceKind::Video { path: "/old/gone.mp4".into() });
        p.add_source("gone too", SourceKind::Video { path: "/old/gone.mp4".into() });
        let plan = plan(&p, "Show", Path::new("/out"), exists_in(&[]));
        assert_eq!(plan.missing, [PathBuf::from("/old/gone.mp4")]);
        assert!(plan.copies.is_empty());
        assert_eq!(plan.project.sources[0].kind.media_path(), Some(Path::new("/old/gone.mp4")));
    }

    #[test]
    fn folder_name_is_unique_and_safe() {
        let plan = plan(&Project::empty(), "a/b: c", Path::new("/out"), exists_in(&["/out/a-b- c"]));
        assert_eq!(plan.folder, PathBuf::from("/out/a-b- c 2"));
        assert_eq!(plan.project_file, PathBuf::from("/out/a-b- c 2/a-b- c 2.omap"));
        let plan2 = super::plan(&Project::empty(), "  ", Path::new("/out"), exists_in(&[]));
        assert_eq!(plan2.folder, PathBuf::from("/out/Untitled"));
    }

    #[test]
    fn execute_writes_a_package_that_opens() {
        let root = std::env::temp_dir().join(format!("om-package-test-{}", std::process::id()));
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("pic.png"), b"not really a png").unwrap();
        let mut p = Project::empty();
        p.add_source("pic", SourceKind::Image { path: src.join("pic.png") });

        let plan = plan(&p, "Pkg", &root, |path| path.exists());
        let progress = AtomicUsize::new(0);
        execute(&plan, &progress).unwrap();
        let opened = Project::load(&plan.project_file).unwrap();
        let copied = opened.sources[0].kind.media_path().unwrap().to_path_buf();
        let content = std::fs::read(&copied).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(copied, root.join("Pkg/Media/pic.png"));
        assert_eq!(content, b"not really a png");
        assert_eq!(progress.load(Ordering::Relaxed), 1);
    }
}
