//! Application shell: owns the document, renderer and UI state, routes
//! actions from menus / shortcuts / panels, and drives the frame loop.

use std::path::PathBuf;
use std::time::Instant;

use egui::{Key, KeyboardShortcut, Modifiers};
use glam::Vec2;
use om_core::{CueId, FILE_EXTENSION, Generator, Mask, Project, SourceId, SourceKind};
use om_render::{Renderer, VIDEO_EXTENSIONS};

use crate::backdrop::BackdropImage;
use crate::cues::CuePlayer;
use crate::dialogs::{self, FileDialogs, Kind, Prompt, PromptAnswer, Purpose, Then};
use crate::displays::DisplayChoice;
use crate::document::Document;
use crate::editor::Editor;
use crate::output::{OutputFrame, OutputWindow};
use crate::panels::{self, Library};
use crate::recovery::{self, Autosave, Recovered};

/// Identifies the app to the OS (window grouping, settings folder).
pub const APP_ID: &str = "net.nawsmapper.NawsMapper";
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tga"];
const LAST_PROJECT_KEY: &str = "last_project";
const DISPLAY_KEY: &str = "output_display";
const FADE_KEY: &str = "blackout_fade";

pub mod shortcuts {
    use super::*;
    const CMD: Modifiers = Modifiers::COMMAND;
    const CMD_SHIFT: Modifiers = Modifiers::COMMAND.plus(Modifiers::SHIFT);

    pub const NEW: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::N);
    pub const OPEN: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::O);
    pub const SAVE: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::S);
    pub const SAVE_AS: KeyboardShortcut = KeyboardShortcut::new(CMD_SHIFT, Key::S);
    pub const UNDO: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::Z);
    pub const REDO: KeyboardShortcut = KeyboardShortcut::new(CMD_SHIFT, Key::Z);
    pub const SELECT_ALL: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::A);
    pub const DUPLICATE: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::D);
    pub const FORWARD: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::CloseBracket);
    pub const BACKWARD: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::OpenBracket);
    pub const TO_FRONT: KeyboardShortcut = KeyboardShortcut::new(CMD_SHIFT, Key::CloseBracket);
    pub const TO_BACK: KeyboardShortcut = KeyboardShortcut::new(CMD_SHIFT, Key::OpenBracket);
    pub const ZOOM_FIT: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::Num0);
    pub const ZOOM_100: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::Num1);
    pub const ZOOM_IN: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::Equals);
    pub const ZOOM_OUT: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::Minus);
    pub const OUTPUT: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::Enter);
    pub const SHOW_MODE: KeyboardShortcut = KeyboardShortcut::new(CMD_SHIFT, Key::Enter);
    /// In Show mode only, so a stray key while editing never blacks out the wall.
    pub const BLACKOUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::B);
    pub const FREEZE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F);
    /// In Show mode: play the next cue (like QLab). In the editor Space is play/pause.
    pub const GO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::Space);
    /// In Show mode: finish a running fade at once and cancel auto-follow.
    pub const STOP_CUE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::Escape);
    pub const ADD_QUAD: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::Q);
    pub const FREE_TRANSFORM: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::T);
    pub const PEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::P);
    pub const MESH: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::W);
    pub const LIBRARY: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::L);
}

/// Everything the user can trigger from menus, toolbar, panels or keys.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    NewDemo,
    NewEmpty,
    Open,
    Save,
    SaveAs,
    Quit,
    Undo,
    Redo,
    SelectAll,
    Deselect,
    Duplicate,
    Delete,
    Restack(i32),
    /// Next (1) or previous (-1) blend mode on the selected surfaces.
    CycleBlend(i32),
    RestackExtreme {
        front: bool,
    },
    AddSurface(Mask),
    Stagger,
    TogglePlay,
    Restart,
    ZoomFit,
    Zoom(f32),
    ZoomBy(f32),
    ToggleOutput,
    AddSource(SourceKind),
    /// Import images and videos through the file dialog.
    ImportMedia,
    /// Pick a new file for an image or video source.
    ReplaceMedia(SourceId),
    /// Add a surface that cuts a hole in the surfaces below it.
    AddCutout,
    Nudge(Vec2),
    /// Cmd+T: start Free Transform, or apply it if already active.
    FreeTransform,
    ApplyTransform,
    CancelTransform,
    /// P: start (or finish) drawing a free-form shape.
    Pen,
    FinishPen,
    CancelPen,
    RemovePenPoint,
    /// Edit the anchor points of the selected surface (turns built-in
    /// shapes into free-form ones).
    EditPoints,
    ExitPoints,
    DeletePoint,
    /// W: bend the selected surface with a mesh (adds one if needed).
    EditMesh,
    ExitMesh,
    /// Straighten the selected surfaces' meshes (keeping the grid size).
    ResetMesh,
    RemoveMesh,
    /// Change the grid size of the selected surfaces' meshes, keeping their shape.
    ResizeMesh {
        cols: usize,
        rows: usize,
    },
    SetMeshSmooth(bool),
    ToggleLibrary,
    /// Add shader library preset `index` as a source; with `apply`, also
    /// show it on the selected surfaces.
    UsePreset {
        index: usize,
        apply: bool,
    },
    /// Lock the editor for a performance: show view, no overlays on the projector.
    ToggleShowMode,
    /// Fade the projector to black, or back.
    ToggleBlackout,
    /// Hold the projector on the current frame while editing continues.
    ToggleFreeze,
    /// Send the output full screen to `displays[index]`; `None` makes it a window.
    ChooseDisplay(Option<usize>),
    /// Resize the canvas to the projector's native size, scaling the surfaces.
    MatchCanvasToProjector,
    ExportTemplate,
    PackageProject,
    ChooseBackdrop,
    RemoveBackdrop,
    /// Remember how all surfaces look now as a new cue.
    RecordCue,
    UpdateCue(CueId),
    DeleteCue(CueId),
    MoveCue(CueId, i32),
    /// Play the next cue.
    Go,
    GoTo(CueId),
    /// Finish the running fade now and cancel auto-follow.
    StopCue,
    /// Choose the cue GO plays next (and whose settings the panel shows).
    SelectCue(Option<CueId>),
}

pub struct Transport {
    pub playing: bool,
    pub time: f64,
    last_tick: Option<Instant>,
}

impl Transport {
    fn tick(&mut self) {
        let now = Instant::now();
        if self.playing
            && let Some(last) = self.last_tick
        {
            self.time += (now - last).as_secs_f64().min(0.25);
        }
        self.last_tick = Some(now);
    }
}

pub struct NawsMapperApp {
    pub doc: Document,
    pub editor: Editor,
    pub transport: Transport,
    pub output: OutputWindow,
    pub renderer: Renderer,
    pub library: Library,
    pub stagger_step: f32,
    /// Show mode: the editor is locked and replaced by the show view.
    pub show_mode: bool,
    pub backdrop: BackdropImage,
    pub cues: CuePlayer,
    render_state: egui_wgpu::RenderState,
    output_tex: egui::TextureId,
    output_generation: u64,
    /// The still frame the projector shows while frozen.
    frozen_tex: Option<egui::TextureId>,
    actions: Vec<Action>,
    dialogs: FileDialogs,
    prompt: Option<Prompt>,
    autosave: Autosave,
    /// Found at startup, waiting for the user's answer.
    recovered: Option<Recovered>,
    package: Option<crate::package::Job>,
    /// Set once the user agreed to quit, so the next close request goes through.
    allow_close: bool,
    status: Option<(String, Instant)>,
    title: String,
    frame_times: std::collections::VecDeque<f32>,
}

impl NawsMapperApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        crate::theme::apply(&cc.egui_ctx);
        let render_state = cc
            .wgpu_render_state
            .clone()
            .ok_or("NawsMapper needs the wgpu renderer (no GPU adapter found?)")?;
        let mut renderer = Renderer::new(&render_state.device, &render_state.queue);
        let ctx = cc.egui_ctx.clone();
        renderer.set_frame_waker(move || ctx.request_repaint());
        let output_tex = render_state.renderer.write().register_native_texture(
            &render_state.device,
            &renderer.output().view,
            wgpu::FilterMode::Linear,
        );

        // A project passed on the command line wins over the last one used.
        let arg = std::env::args_os().nth(1).map(PathBuf::from);
        let last: Option<PathBuf> =
            arg.or_else(|| cc.storage.and_then(|s| eframe::get_value(s, LAST_PROJECT_KEY)));
        let doc = match last.filter(|p| p.exists()) {
            Some(path) => match Project::load(&path) {
                Ok(p) => Document::new(p, Some(path)),
                Err(e) => {
                    log::warn!("could not reopen {}: {e:#}", path.display());
                    Document::new(Project::demo(), None)
                }
            },
            None => Document::new(Project::demo(), None),
        };

        let mut output = OutputWindow::default();
        if let Some(storage) = cc.storage {
            load_output_settings(storage, &mut output);
        }
        let autosave = Autosave::new(eframe::storage_dir(APP_ID));
        let recovered = autosave.find();
        let prompt =
            recovered.as_ref().map(|r| Prompt::Recover { name: r.name(), age: r.age(recovery::now()) });

        Ok(Self {
            doc,
            editor: Editor::default(),
            transport: Transport { playing: true, time: 0.0, last_tick: None },
            output,
            renderer,
            library: Library::default(),
            stagger_step: 0.25,
            show_mode: false,
            backdrop: BackdropImage::default(),
            cues: CuePlayer::default(),
            render_state,
            output_tex,
            output_generation: 0,
            frozen_tex: None,
            actions: Vec::new(),
            dialogs: FileDialogs::default(),
            prompt,
            autosave,
            recovered,
            package: None,
            allow_close: false,
            status: None,
            title: String::new(),
            frame_times: Default::default(),
        })
    }

    pub fn act(&mut self, action: Action) {
        self.actions.push(action);
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), Instant::now()));
    }

    pub fn status(&self) -> Option<&str> {
        self.status.as_ref().filter(|(_, t)| t.elapsed().as_secs_f32() < 4.0).map(|(m, _)| m.as_str())
    }

    pub fn fps(&self) -> f32 {
        let n = self.frame_times.len().max(1) as f32;
        let avg = self.frame_times.iter().sum::<f32>() / n;
        if avg > 0.0 { 1.0 / avg } else { 0.0 }
    }

    // ---- Keyboard ----------------------------------------------------------

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        use shortcuts::*;
        if self.prompt.is_some() || self.dialogs.busy() {
            return;
        }
        if self.show_mode {
            let mut acts = Vec::new();
            ctx.input_mut(|i| {
                if i.consume_shortcut(&SHOW_MODE) {
                    acts.push(Action::ToggleShowMode);
                }
                if i.consume_shortcut(&BLACKOUT) {
                    acts.push(Action::ToggleBlackout);
                }
                if i.consume_shortcut(&FREEZE) {
                    acts.push(Action::ToggleFreeze);
                }
                if i.consume_shortcut(&GO) {
                    acts.push(Action::Go);
                }
                if i.consume_shortcut(&STOP_CUE) {
                    acts.push(Action::StopCue);
                }
                // Every other key is dropped: a stray press must not change the show.
                i.events.retain(|e| !matches!(e, egui::Event::Key { .. } | egui::Event::Text(_)));
            });
            self.actions.extend(acts);
            return;
        }
        let text_focus = ctx.egui_wants_keyboard_input();
        let transforming = self.editor.transform.is_some();
        let pen = self.editor.pen.is_some();
        let points = self.editor.points.is_some();
        let mesh = self.editor.mesh.is_some();
        let mut acts = Vec::new();
        ctx.input_mut(|i| {
            // Order matters: shortcuts with Shift must be checked first.
            let table: &[(KeyboardShortcut, Action)] = &[
                (SAVE_AS, Action::SaveAs),
                (SAVE, Action::Save),
                (OPEN, Action::Open),
                (NEW, Action::NewDemo),
                (SHOW_MODE, Action::ToggleShowMode),
                (OUTPUT, Action::ToggleOutput),
                (LIBRARY, Action::ToggleLibrary),
            ];
            for (sc, a) in table {
                if i.consume_shortcut(sc) {
                    acts.push(a.clone());
                }
            }
            if text_focus {
                return;
            }
            // Space belongs to the canvas (pan / play). Swallow the press so
            // it never activates whichever button last had keyboard focus.
            i.consume_key(Modifiers::NONE, Key::Space);
            let table: &[(KeyboardShortcut, Action)] = &[
                (REDO, Action::Redo),
                (UNDO, Action::Undo),
                (SELECT_ALL, Action::SelectAll),
                (DUPLICATE, Action::Duplicate),
                (TO_FRONT, Action::RestackExtreme { front: true }),
                (TO_BACK, Action::RestackExtreme { front: false }),
                (FORWARD, Action::Restack(1)),
                (BACKWARD, Action::Restack(-1)),
                (ZOOM_FIT, Action::ZoomFit),
                (ZOOM_100, Action::Zoom(1.0)),
                (ZOOM_IN, Action::ZoomBy(1.25)),
                (ZOOM_OUT, Action::ZoomBy(0.8)),
                (ADD_QUAD, Action::AddSurface(Mask::Quad)),
                (FREE_TRANSFORM, Action::FreeTransform),
                (PEN, Action::Pen),
                (MESH, Action::EditMesh),
            ];
            for (sc, a) in table {
                if i.consume_shortcut(sc) {
                    acts.push(a.clone());
                }
            }
            if i.consume_key(Modifiers::COMMAND, Key::Y) {
                acts.push(Action::Redo);
            }
            // Photoshop: Shift + / Shift - step through blend modes.
            if i.consume_key(Modifiers::SHIFT, Key::Equals) || i.consume_key(Modifiers::SHIFT, Key::Plus) {
                acts.push(Action::CycleBlend(1));
            }
            if i.consume_key(Modifiers::SHIFT, Key::Minus) {
                acts.push(Action::CycleBlend(-1));
            }
            let delete =
                i.consume_key(Modifiers::NONE, Key::Delete) || i.consume_key(Modifiers::NONE, Key::Backspace);
            // In Mesh Warp, Delete would remove the surface being bent: ignore it.
            if delete && !mesh {
                acts.push(if pen {
                    Action::RemovePenPoint
                } else if points {
                    Action::DeletePoint
                } else {
                    Action::Delete
                });
            }
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                acts.push(if pen {
                    Action::CancelPen
                } else if points {
                    Action::ExitPoints
                } else if mesh {
                    Action::ExitMesh
                } else if transforming {
                    Action::CancelTransform
                } else {
                    Action::Deselect
                });
            }
            if i.consume_key(Modifiers::NONE, Key::Enter) {
                if pen {
                    acts.push(Action::FinishPen);
                } else if points {
                    acts.push(Action::ExitPoints);
                } else if mesh {
                    acts.push(Action::ExitMesh);
                } else if transforming {
                    acts.push(Action::ApplyTransform);
                }
            }
            let step = if i.modifiers.shift { 10.0 } else { 1.0 };
            for (key, dir) in [
                (Key::ArrowLeft, Vec2::NEG_X),
                (Key::ArrowRight, Vec2::X),
                (Key::ArrowUp, Vec2::NEG_Y),
                (Key::ArrowDown, Vec2::Y),
            ] {
                if i.key_pressed(key) {
                    acts.push(Action::Nudge(dir * step));
                }
            }
        });

        // Space: hold to pan, tap to play/pause.
        if !text_focus && ctx.input(|i| i.key_released(Key::Space)) {
            if !self.editor.space_panned {
                acts.push(Action::TogglePlay);
            }
            self.editor.space_panned = false;
        }
        self.actions.extend(acts);
    }

    // ---- Actions -------------------------------------------------------------

    fn run_actions(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        for action in std::mem::take(&mut self.actions) {
            self.run_action(ctx, frame, action);
        }
    }

    fn run_action(&mut self, ctx: &egui::Context, frame: &eframe::Frame, action: Action) {
        log::debug!("action: {action:?}");
        let sel = self.editor.selection.surfaces.clone();
        // Anything that changes what exists or what is selected first applies
        // a running Free Transform, like Photoshop does.
        if self.editor.transform.is_some()
            && matches!(
                action,
                Action::NewDemo
                    | Action::NewEmpty
                    | Action::Open
                    | Action::Undo
                    | Action::Redo
                    | Action::SelectAll
                    | Action::Deselect
                    | Action::Duplicate
                    | Action::Delete
                    | Action::AddSurface(_)
                    | Action::AddCutout
                    | Action::Pen
                    | Action::EditPoints
                    | Action::ToggleShowMode
                    | Action::MatchCanvasToProjector
                    | Action::EditMesh
            )
        {
            self.editor.apply_transform();
        }
        // Same for drawing and point editing.
        if matches!(
            action,
            Action::NewDemo
                | Action::NewEmpty
                | Action::Open
                | Action::Undo
                | Action::Redo
                | Action::SelectAll
                | Action::Duplicate
                | Action::Delete
                | Action::AddSurface(_)
                | Action::AddCutout
                | Action::FreeTransform
                | Action::ToggleShowMode
                | Action::MatchCanvasToProjector
                | Action::EditMesh
        ) {
            self.editor.finish_pen();
            self.editor.points = None;
        }
        // Other tools and documents end mesh editing.
        if matches!(
            action,
            Action::NewDemo
                | Action::NewEmpty
                | Action::Open
                | Action::Undo
                | Action::Redo
                | Action::SelectAll
                | Action::Delete
                | Action::FreeTransform
                | Action::Pen
                | Action::EditPoints
                | Action::ToggleShowMode
                | Action::RemoveMesh
        ) {
            self.editor.mesh = None;
        }
        match action {
            Action::NewDemo => self.guard(ctx, frame, Then::NewDemo),
            Action::NewEmpty => self.guard(ctx, frame, Then::NewEmpty),
            Action::Open => self.guard(ctx, frame, Then::Open),
            Action::Save => self.save(ctx, frame, false, None),
            Action::SaveAs => self.save(ctx, frame, true, None),
            Action::Quit if self.show_mode => self.prompt = Some(Prompt::QuitShow),
            Action::Quit => self.guard(ctx, frame, Then::Quit),
            Action::Undo => {
                if self.doc.undo() {
                    self.set_status("Undo");
                }
            }
            Action::Redo => {
                if self.doc.redo() {
                    self.set_status("Redo");
                }
            }
            Action::SelectAll => {
                let ids = self
                    .doc
                    .project
                    .surfaces
                    .iter()
                    .filter(|s| s.visible && !s.locked)
                    .map(|s| s.id)
                    .collect();
                self.editor.selection.set(ids);
            }
            Action::Deselect => self.editor.selection.clear(),
            Action::Duplicate => {
                let ids = self.doc.duplicate(&sel);
                if !ids.is_empty() {
                    self.editor.selection.set(ids);
                }
            }
            Action::Delete => {
                self.doc.delete(&sel);
                self.editor.selection.clear();
            }
            Action::Restack(dir) => self.doc.restack(&sel, dir),
            Action::RestackExtreme { front } => self.doc.restack_extreme(&sel, front),
            Action::AddSurface(mask) => {
                let id = self.doc.add_surface(mask);
                self.editor.selection.set(vec![id]);
            }
            Action::AddCutout => {
                let id = self.doc.add_cutout();
                self.editor.selection.set(vec![id]);
            }
            Action::Stagger => {
                if sel.len() > 1 {
                    self.doc.stagger(&sel, self.stagger_step);
                    self.set_status(format!(
                        "Staggered {} surfaces by {:.2}s (in selection order)",
                        sel.len(),
                        self.stagger_step
                    ));
                }
            }
            Action::TogglePlay => self.transport.playing = !self.transport.playing,
            Action::Restart => self.transport.time = 0.0,
            Action::ZoomFit => self.editor.fit(),
            Action::Zoom(z) => self.editor.set_zoom(&self.doc.project, z),
            Action::ZoomBy(f) => {
                let z = self.editor.zoom * f;
                self.editor.set_zoom(&self.doc.project, z);
            }
            Action::ToggleOutput => self.output.open = !self.output.open,
            Action::AddSource(kind) => {
                let name = match &kind {
                    SourceKind::Generator(g) => g.pattern.label().to_string(),
                    other => other.label().to_string(),
                };
                let id = self.doc.project.add_source(name, kind);
                self.editor.reveal_source(id);
            }
            Action::ImportMedia => {
                let dialog = media_dialog(frame, "Import Images and Videos");
                self.dialogs.start(ctx, Purpose::ImportMedia, dialog, Kind::PickFiles);
            }
            Action::ReplaceMedia(id) => {
                let dialog = media_dialog(frame, "Replace Media");
                self.dialogs.start(ctx, Purpose::ReplaceMedia(id), dialog, Kind::PickFile);
            }
            Action::Nudge(delta) => self.nudge(delta),
            Action::CycleBlend(step) => {
                // All selected surfaces take the primary's next mode, like a combo pick.
                let primary = self.editor.selection.primary().and_then(|id| self.doc.project.surface(id));
                if let Some(next) = primary.map(|s| s.blend.cycle(step)) {
                    for id in &self.editor.selection.surfaces {
                        if let Some(s) = self.doc.project.surface_mut(*id).filter(|s| !s.locked && !s.cutout)
                        {
                            s.blend = next;
                        }
                    }
                }
            }
            Action::FreeTransform => {
                if self.editor.transform.is_some() {
                    self.editor.apply_transform();
                } else if !self.editor.begin_transform(&self.doc.project) {
                    self.set_status("Select one or more unlocked surfaces to transform");
                }
            }
            Action::ApplyTransform => self.editor.apply_transform(),
            Action::CancelTransform => self.editor.cancel_transform(&mut self.doc.project),
            Action::Pen => {
                if self.editor.pen.is_some() {
                    self.editor.finish_pen();
                } else {
                    self.editor.points = None;
                    self.editor.pen = Some(Vec::new());
                }
            }
            Action::FinishPen => self.editor.finish_pen(),
            Action::CancelPen => self.editor.pen = None,
            Action::RemovePenPoint => {
                if let Some(pen) = &mut self.editor.pen
                    && pen.pop().is_none()
                {
                    self.editor.pen = None;
                }
            }
            Action::EditPoints => {
                let target = self.editor.selection.primary().and_then(|id| self.doc.project.surface_mut(id));
                match target {
                    Some(s) if s.warp.is_some() => {
                        self.set_status("Edit Points works on unwarped shapes: remove the mesh warp first")
                    }
                    Some(s) if !s.locked => {
                        s.make_freeform();
                        let id = s.id;
                        self.editor.edit_points(id);
                    }
                    Some(_) => self.set_status("The surface is locked"),
                    None => self.set_status("Select a surface to edit its points"),
                }
            }
            Action::ExitPoints => self.editor.points = None,
            Action::DeletePoint => {
                if !self.editor.delete_point(&mut self.doc.project) {
                    self.set_status("Select a point first. A shape keeps at least three points.");
                }
            }
            Action::EditMesh => {
                let target = self.editor.selection.primary().and_then(|id| self.doc.project.surface_mut(id));
                match target {
                    Some(s) if !s.locked => {
                        s.warp.get_or_insert_with(om_core::Warp::default);
                        let id = s.id;
                        self.editor.edit_mesh(id);
                    }
                    Some(_) => self.set_status("The surface is locked"),
                    None => self.set_status("Select a surface to bend it with a mesh"),
                }
            }
            Action::ExitMesh => self.editor.mesh = None,
            Action::ResetMesh => self.each_mesh(&sel, |w| {
                *w = om_core::Warp { smooth: w.smooth, ..om_core::Warp::identity(w.cols, w.rows) }
            }),
            Action::RemoveMesh => {
                for id in &sel {
                    if let Some(s) = self.doc.project.surface_mut(*id).filter(|s| !s.locked) {
                        s.warp = None;
                    }
                }
            }
            Action::ResizeMesh { cols, rows } => self.each_mesh(&sel, |w| *w = w.resized(cols, rows)),
            Action::SetMeshSmooth(smooth) => self.each_mesh(&sel, |w| w.smooth = smooth),
            Action::ToggleLibrary => self.library.open = !self.library.open,
            Action::ToggleShowMode => {
                self.show_mode = !self.show_mode;
                if self.show_mode {
                    self.editor.pen = None;
                    self.library.open = false;
                    self.output.open = true;
                    self.output.replace_on_projector();
                    if self.output.projector().is_none() {
                        self.set_status("Show mode. Tip: pick the projector in Output → Projector.");
                    }
                }
            }
            Action::ToggleBlackout => self.output.blackout = !self.output.blackout,
            Action::ToggleFreeze => self.toggle_freeze(),
            Action::ChooseDisplay(index) => {
                self.output.choose_display(index);
                if let Some(p) = self.output.projector() {
                    let msg = format!("Output on {}", p.name);
                    self.set_status(msg);
                }
            }
            Action::MatchCanvasToProjector => {
                if let Some((w, h)) = self.output.size_mismatch(&self.doc.project) {
                    self.doc.project.resize_canvas(w, h);
                    self.editor.fit();
                    self.set_status(format!("Canvas is now {w}×{h}, pixel for pixel on the projector"));
                }
            }
            Action::ExportTemplate => {
                let dialog = rfd::AsyncFileDialog::new()
                    .set_parent(frame)
                    .set_title("Export Mapping Template")
                    .add_filter("PNG image (for Photoshop, After Effects…)", &["png"])
                    .add_filter("SVG vector (for Illustrator, Affinity…)", &["svg"])
                    .set_file_name(format!("{} template.png", self.doc.display_name()));
                self.dialogs.start(ctx, Purpose::ExportTemplate, dialog, Kind::SaveFile);
            }
            Action::PackageProject => {
                if self.package.is_some() {
                    self.set_status("Still packaging…");
                } else {
                    let dialog = rfd::AsyncFileDialog::new()
                        .set_parent(frame)
                        .set_title("Package Project: choose where to put the package folder");
                    self.dialogs.start(ctx, Purpose::Package, dialog, Kind::PickFolder);
                }
            }
            Action::ChooseBackdrop => {
                let dialog = rfd::AsyncFileDialog::new()
                    .set_parent(frame)
                    .set_title("Choose a Venue Photo")
                    .add_filter("Images", IMAGE_EXTENSIONS);
                self.dialogs.start(ctx, Purpose::ChooseBackdrop, dialog, Kind::PickFile);
            }
            Action::RemoveBackdrop => self.doc.project.canvas.backdrop.path = None,
            Action::RecordCue => {
                let id = self.doc.record_cue(self.cues.selected.or(self.cues.live));
                // You are "in" the cue you just recorded.
                self.cues.live = Some(id);
                self.cues.selected = None;
                let name = self.doc.project.cue(id).map(|c| c.name.clone()).unwrap_or_default();
                self.set_status(format!("Recorded \"{name}\": how all surfaces look now"));
            }
            Action::UpdateCue(id) => {
                self.doc.update_cue(id);
                self.set_status("Cue updated with the current look");
            }
            Action::DeleteCue(id) => {
                self.doc.delete_cue(id);
                if self.cues.live == Some(id) {
                    self.cues.live = None;
                }
                if self.cues.selected == Some(id) {
                    self.cues.selected = None;
                }
            }
            Action::MoveCue(id, dir) => self.doc.move_cue(id, dir),
            Action::Go => match self.cues.next(&self.doc.project) {
                Some(id) => self.go_cue(id),
                None if self.doc.project.cues.is_empty() => {
                    self.set_status("No cues yet: set up a look and click ● Record")
                }
                None => self.set_status("That was the last cue"),
            },
            Action::GoTo(id) => self.go_cue(id),
            Action::StopCue => self.cues.stop(),
            Action::SelectCue(id) => self.cues.selected = id,
            Action::UsePreset { index, apply } => {
                let Some(preset) = self.library.presets.get(index) else { return };
                let (name, generator) = (preset.name, preset.generator.clone());
                let id = self.doc.add_generator(name, generator);
                self.editor.reveal_source(id);
                if apply && !sel.is_empty() {
                    self.doc.assign_source(&sel, Some(id));
                    self.set_status(format!("\"{name}\" is on {} surface(s)", sel.len()));
                } else {
                    self.set_status(format!("Added \"{name}\" to Sources"));
                }
            }
        }
    }

    fn toggle_freeze(&mut self) {
        let mut egui_renderer = self.render_state.renderer.write();
        if let Some(id) = self.frozen_tex.take() {
            egui_renderer.free_texture(&id);
            self.renderer.unfreeze();
        } else {
            let view = &self.renderer.freeze().view;
            let id = egui_renderer.register_native_texture(
                &self.render_state.device,
                view,
                wgpu::FilterMode::Linear,
            );
            self.frozen_tex = Some(id);
        }
    }

    pub fn frozen(&self) -> bool {
        self.frozen_tex.is_some()
    }

    /// What the projector shows: the frozen still, or the live output.
    pub fn projector_texture(&self) -> egui::TextureId {
        self.frozen_tex.unwrap_or(self.output_tex)
    }

    fn export_template(&mut self, path: PathBuf) {
        let svg = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg"));
        let path = if path.extension().is_none() { path.with_extension("png") } else { path };
        let title = self.doc.display_name();
        let bytes = if svg {
            Ok(crate::template::svg(&self.doc.project, &title).into_bytes())
        } else {
            crate::template::png(&self.doc.project, &title)
        };
        match bytes.and_then(|b| Ok(std::fs::write(&path, b)?)) {
            Ok(()) => {
                let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                self.set_status(format!("Exported {name}"));
            }
            Err(e) => self.show_error("Could not export the template", format!("{e:#}")),
        }
    }

    fn start_package(&mut self, ctx: &egui::Context, parent: PathBuf) {
        let plan = crate::package::plan(&self.doc.project, &self.doc.display_name(), &parent, |p| p.exists());
        self.set_status(format!("Packaging {} files…", plan.copies.len()));
        self.package = Some(crate::package::Job::start(plan, ctx));
    }

    fn poll_package(&mut self) {
        let Some(job) = &self.package else { return };
        let Some(result) = job.poll() else { return };
        let summary = job.plan_summary.clone();
        self.package = None;
        let folder = summary.folder.display().to_string();
        match result {
            Ok(()) if summary.missing.is_empty() => {
                self.set_status(format!("Packaged {} media files into {folder}", summary.files));
            }
            Ok(()) => {
                let list: Vec<String> =
                    summary.missing.iter().map(|p| format!("• {}", p.display())).collect();
                self.prompt = Some(Prompt::Message {
                    title: "Packaged, with missing files".into(),
                    message: format!(
                        "The package is in {folder}, but these files could not be found and are not in it:\n\n{}",
                        list.join("\n")
                    ),
                });
            }
            Err(e) => self.show_error("Could not package the project", e),
        }
    }

    /// Packaging progress for the status bar: (copied, total).
    pub fn package_progress(&self) -> Option<(usize, usize)> {
        let job = self.package.as_ref()?;
        Some((job.copied.load(std::sync::atomic::Ordering::Relaxed), job.plan_summary.files))
    }

    fn go_cue(&mut self, id: CueId) {
        if self.cues.go(&mut self.doc.project, id, Instant::now())
            && let Some(i) = self.doc.project.cue_index(id)
        {
            let name = self.doc.project.cues[i].name.clone();
            self.set_status(format!("▶ {} {name}", i + 1));
        }
    }

    /// Applies `f` to the mesh of every selected, unlocked surface that has one.
    fn each_mesh(&mut self, ids: &[om_core::SurfaceId], f: impl Fn(&mut om_core::Warp)) {
        for id in ids {
            if let Some(w) =
                self.doc.project.surface_mut(*id).filter(|s| !s.locked).and_then(|s| s.warp.as_mut())
            {
                f(w);
            }
        }
    }

    fn nudge(&mut self, delta: Vec2) {
        if self.editor.mesh.is_some() {
            if !self.editor.nudge_mesh_point(&mut self.doc.project, delta) {
                self.set_status("Click a mesh point first");
            }
            return;
        }
        if self.editor.points.is_some() {
            self.editor.nudge_point(&mut self.doc.project, delta);
            return;
        }
        if self.editor.transform.is_some() {
            self.editor.nudge_transform(&mut self.doc.project, delta);
            return;
        }
        let project = &mut self.doc.project;
        if let Some((id, idx)) = self.editor.selection.corner {
            let Some(pos) = project.surface(id).map(|s| s.corners[idx]) else { return };
            // Welded corners move along, same as when dragging.
            for s in project.surfaces.iter_mut().filter(|s| !s.locked) {
                for c in s.corners.iter_mut() {
                    if c.distance(pos) <= 0.5 {
                        *c += delta;
                    }
                }
            }
        } else {
            for id in &self.editor.selection.surfaces {
                if let Some(s) = project.surface_mut(*id).filter(|s| !s.locked) {
                    s.translate(delta);
                }
            }
        }
    }

    /// Adds images and videos as sources (other files are skipped) and
    /// shows the last one on the selected surfaces.
    pub fn import_media(&mut self, paths: Vec<PathBuf>) {
        let mut last = None;
        let mut skipped = Vec::new();
        for path in paths {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            let kind = if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
                SourceKind::Image { path: path.clone() }
            } else if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
                SourceKind::Video { path: path.clone() }
            } else {
                skipped.push(path.file_name().unwrap_or_default().to_string_lossy().into_owned());
                continue;
            };
            let name =
                path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or(kind.label().into());
            last = Some(self.doc.project.add_source(name, kind));
        }
        if let Some(id) = last {
            self.editor.reveal_source(id);
            if !self.editor.selection.surfaces.is_empty() {
                let sel = self.editor.selection.surfaces.clone();
                self.doc.assign_source(&sel, Some(id));
            } else {
                self.set_status("Imported. Select surfaces and double-click the source to show it.");
            }
        }
        if !skipped.is_empty() {
            self.set_status(format!("Not an image or video: {}", skipped.join(", ")));
        }
    }

    /// Saves to the current file, or asks for one first. `then` runs after a
    /// successful save (used by the unsaved-changes prompt).
    fn save(&mut self, ctx: &egui::Context, frame: &eframe::Frame, force_dialog: bool, then: Option<Then>) {
        match (&self.doc.path, force_dialog) {
            (Some(p), false) => {
                let path = p.clone();
                self.write_project(ctx, frame, path, then);
            }
            _ => {
                let dialog = rfd::AsyncFileDialog::new()
                    .set_parent(frame)
                    .set_title("Save Project")
                    .add_filter("NawsMapper project", &[FILE_EXTENSION])
                    .set_file_name(format!("{}.{FILE_EXTENSION}", self.doc.display_name()));
                self.dialogs.start(ctx, Purpose::SaveAs { then }, dialog, Kind::SaveFile);
            }
        }
    }

    fn write_project(
        &mut self,
        ctx: &egui::Context,
        frame: &eframe::Frame,
        path: PathBuf,
        then: Option<Then>,
    ) {
        let path = if path.extension().is_none() { path.with_extension(FILE_EXTENSION) } else { path };
        self.doc.commit_if_changed();
        match self.doc.save_to(path) {
            Ok(()) => {
                self.set_status("Saved");
                if let Some(then) = then {
                    self.proceed(ctx, frame, then);
                }
            }
            Err(e) => self.show_error("Could not save project", format!("{e:#}")),
        }
    }

    /// Runs `then`, first asking to save unsaved changes.
    fn guard(&mut self, ctx: &egui::Context, frame: &eframe::Frame, then: Then) {
        self.doc.commit_if_changed();
        if self.doc.dirty {
            self.prompt = Some(Prompt::Unsaved { name: self.doc.display_name(), then });
        } else {
            self.proceed(ctx, frame, then);
        }
    }

    fn proceed(&mut self, ctx: &egui::Context, frame: &eframe::Frame, then: Then) {
        match then {
            Then::NewDemo | Then::NewEmpty => {
                let p = if then == Then::NewDemo { Project::demo() } else { Project::empty() };
                self.replace_document(Document::new(p, None));
            }
            Then::Open => {
                let dialog = rfd::AsyncFileDialog::new()
                    .set_parent(frame)
                    .set_title("Open Project")
                    .add_filter("NawsMapper project", &[FILE_EXTENSION]);
                self.dialogs.start(ctx, Purpose::Open, dialog, Kind::PickFile);
            }
            Then::OpenPath(path) => match Project::load(&path) {
                Ok(p) => {
                    self.replace_document(Document::new(p, Some(path)));
                    self.set_status("Project opened");
                }
                Err(e) => self.show_error("Could not open project", format!("{e:#}")),
            },
            Then::Quit => {
                // Saved, or discarded on purpose: nothing to recover next time.
                self.autosave.clear();
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn replace_document(&mut self, doc: Document) {
        // The old document was saved or deliberately discarded.
        self.autosave.clear();
        self.editor.transform = None;
        self.editor.pen = None;
        self.editor.points = None;
        self.editor.mesh = None;
        self.cues.reset();
        self.doc = doc;
        self.editor.selection.clear();
        self.editor.selected_source = None;
        self.editor.fit();
    }

    pub fn show_error(&mut self, title: &str, message: String) {
        log::error!("{title}: {message}");
        self.prompt = Some(Prompt::Message { title: title.into(), message });
    }

    /// Handles a file dialog that just closed.
    fn finish_dialog(
        &mut self,
        ctx: &egui::Context,
        frame: &eframe::Frame,
        purpose: Purpose,
        paths: Vec<PathBuf>,
    ) {
        let Some(first) = paths.first().cloned() else { return };
        match purpose {
            Purpose::Open => self.proceed(ctx, frame, Then::OpenPath(first)),
            Purpose::SaveAs { then } => self.write_project(ctx, frame, first, then),
            Purpose::ImportMedia => self.import_media(paths),
            Purpose::ReplaceMedia(id) => {
                let ext = first.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                let kind = if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
                    SourceKind::Video { path: first }
                } else {
                    SourceKind::Image { path: first }
                };
                if let Some(src) = self.doc.project.source_mut(id) {
                    src.kind = kind;
                }
            }
            Purpose::ExportTemplate => self.export_template(first),
            Purpose::Package => self.start_package(ctx, first),
            Purpose::ChooseBackdrop => self.doc.project.canvas.backdrop.path = Some(first),
        }
    }

    fn show_prompt(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let Some(prompt) = &self.prompt else { return };
        let Some(answer) = dialogs::show_prompt(ctx, prompt) else { return };
        self.prompt = None;
        match answer {
            PromptAnswer::Save(then) => self.save(ctx, frame, false, Some(then)),
            PromptAnswer::Discard(then) => self.proceed(ctx, frame, then),
            PromptAnswer::Dismiss => {}
            PromptAnswer::QuitShow => {
                // Leave Show mode, then quit the usual way (asks about unsaved changes).
                self.show_mode = false;
                self.guard(ctx, frame, Then::Quit);
            }
            PromptAnswer::Restore => {
                if let Some(r) = self.recovered.take() {
                    let name = r.name();
                    self.replace_document(Document::restored(r.project, r.original));
                    self.set_status(format!("Restored unsaved changes to \"{name}\". Save to keep them."));
                }
            }
            PromptAnswer::DropRecovery => {
                self.recovered = None;
                self.autosave.clear();
            }
        }
    }

    // ---- Frame -------------------------------------------------------------

    fn render_output(&mut self) {
        // During a cue fade the projector shows the crossfade, not the document.
        let project = self.cues.render_project(&self.doc.project, Instant::now());
        self.renderer.render(&project, self.transport.time);
        if self.renderer.output_generation() != self.output_generation {
            self.output_generation = self.renderer.output_generation();
            self.render_state.renderer.write().update_egui_texture_from_wgpu_texture(
                &self.render_state.device,
                &self.renderer.output().view,
                wgpu::FilterMode::Linear,
                self.output_tex,
            );
        }
    }

    /// Renders the shader library previews and returns their egui texture.
    pub fn library_thumbnails(&mut self) -> egui::TextureId {
        let generators: Vec<Generator> = self.library.presets.iter().map(|p| p.generator.clone()).collect();
        self.renderer.render_thumbnails(&generators, self.library.clock.elapsed().as_secs_f64());
        let generation = self.renderer.thumbnail_generation();
        let view = &self.renderer.thumbnails().expect("rendered above").view;
        let device = &self.render_state.device;
        let mut egui_renderer = self.render_state.renderer.write();
        let id = match self.library.texture {
            Some((id, g)) if g == generation => return id,
            Some((id, _)) => {
                egui_renderer.update_egui_texture_from_wgpu_texture(
                    device,
                    view,
                    wgpu::FilterMode::Linear,
                    id,
                );
                id
            }
            None => egui_renderer.register_native_texture(device, view, wgpu::FilterMode::Linear),
        };
        self.library.texture = Some((id, generation));
        id
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title =
            format!("{}{} — NawsMapper", self.doc.display_name(), if self.doc.dirty { " •" } else { "" });
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let dropped: Vec<PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if dropped.is_empty() || self.prompt.is_some() {
            return;
        }
        if let Some(project) = dropped.iter().find(|p| p.extension().is_some_and(|e| e == FILE_EXTENSION)) {
            self.guard(ctx, frame, Then::OpenPath(project.clone()));
        } else {
            self.import_media(dropped);
        }
    }

    fn handle_close(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        if !ctx.input(|i| i.viewport().close_requested()) || self.allow_close {
            return;
        }
        if self.show_mode {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.prompt.get_or_insert(Prompt::QuitShow);
            return;
        }
        self.doc.commit_if_changed();
        if self.doc.dirty {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.prompt.is_none() {
                self.guard(ctx, frame, Then::Quit);
            }
        }
    }
}

impl eframe::App for NawsMapperApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dt = ctx.input(|i| i.unstable_dt);
        self.frame_times.push_back(dt);
        if self.frame_times.len() > 60 {
            self.frame_times.pop_front();
        }

        self.transport.tick();
        self.output.poll_displays(frame);
        if self.output.step_fade(dt) {
            ctx.request_repaint();
        }
        self.handle_shortcuts(&ctx);
        if !self.show_mode {
            self.handle_dropped_files(&ctx, frame);
        }
        if let Some((purpose, paths)) = self.dialogs.poll() {
            self.finish_dialog(&ctx, frame, purpose, paths);
        }
        self.poll_package();
        let now = Instant::now();
        if let Some(id) = self.cues.due(&self.doc.project, now) {
            self.go_cue(id);
        }
        self.backdrop.sync(&ctx, self.doc.project.canvas.backdrop.path.as_deref());
        self.render_output();

        if self.show_mode {
            panels::show_view(ui, self);
        } else {
            panels::menu_bar(ui, self);
            panels::toolbar(ui, self);
            panels::status_bar(ui, self);
            panels::left_panel(ui, self);
            panels::properties_panel(ui, self);
            panels::cues_panel(ui, self);
            egui::CentralPanel::no_frame().show(ui, |ui| {
                let rect = ui.max_rect();
                self.editor.ui(ui, &mut self.doc.project, self.output_tex, self.backdrop.get());
                panels::projector_banner(ui, rect, self);
            });
            if let Some(points) = self.editor.finished_pen.take() {
                match self.doc.add_freeform(&points) {
                    Some(id) => self.editor.selection.set(vec![id]),
                    None => self.set_status("That shape is too small"),
                }
            }
            panels::library_window(&ctx, self);
        }

        let texture = self.projector_texture();
        self.output.show(
            &ctx,
            OutputFrame {
                texture,
                project: &self.doc.project,
                selection: &self.editor.selection,
                pointer: if self.show_mode { None } else { self.editor.pointer },
                show_mode: self.show_mode,
                frozen: self.frozen(),
                mesh: self.editor.mesh,
            },
        );

        self.run_actions(&ctx, frame);
        self.show_prompt(&ctx, frame);
        self.editor.selection.retain_existing(&self.doc.project);
        if let Some(id) = self.editor.selected_source
            && self.doc.project.source(id).is_none()
        {
            self.editor.selected_source = None;
        }

        // Commit an undo step once the user lets go of the mouse.
        if !ctx.input(|i| i.pointer.any_down()) && !self.editor.is_interacting() {
            self.doc.commit_if_changed();
        }

        if self.recovered.is_none() {
            self.autosave.tick(&self.doc.project, self.doc.path.as_deref(), self.doc.dirty);
        }
        self.update_title(&ctx);
        self.handle_close(&ctx, frame);

        if let Some(s) = self.cues.follow_in(now) {
            ctx.request_repaint_after(std::time::Duration::from_secs_f32(s));
        }
        if self.transport.playing || self.package.is_some() || self.cues.progress(now).is_some() {
            ctx.request_repaint();
        } else if self.output.open && self.output.display.is_some() {
            // Notice the chosen projector being plugged in, even while idle.
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        } else if self.doc.dirty {
            ctx.request_repaint_after(recovery::INTERVAL);
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Some(path) = &self.doc.path {
            eframe::set_value(storage, LAST_PROJECT_KEY, path);
        }
        save_output_settings(storage, &self.output);
    }
}

/// The projector choice and fade time, remembered across launches.
fn save_output_settings(storage: &mut dyn eframe::Storage, output: &OutputWindow) {
    eframe::set_value(storage, DISPLAY_KEY, &output.display);
    eframe::set_value(storage, FADE_KEY, &output.fade_secs);
}

fn load_output_settings(storage: &dyn eframe::Storage, output: &mut OutputWindow) {
    output.display = eframe::get_value::<Option<DisplayChoice>>(storage, DISPLAY_KEY).flatten();
    output.fade_secs = eframe::get_value(storage, FADE_KEY).unwrap_or(output.fade_secs);
}

fn media_dialog(frame: &eframe::Frame, title: &str) -> rfd::AsyncFileDialog {
    let all: Vec<&str> = IMAGE_EXTENSIONS.iter().chain(VIDEO_EXTENSIONS).copied().collect();
    rfd::AsyncFileDialog::new()
        .set_parent(frame)
        .set_title(title)
        .add_filter("Images and videos", &all)
        .add_filter("Images", IMAGE_EXTENSIONS)
        .add_filter("Videos (H.264 MP4)", VIDEO_EXTENSIONS)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[derive(Default)]
    struct MemoryStorage(HashMap<String, String>);

    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }
        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }
        fn flush(&mut self) {}
    }

    #[test]
    fn projector_choice_and_fade_survive_a_restart() {
        let mut storage = MemoryStorage::default();
        let mut before = OutputWindow::default();
        before.display = Some(DisplayChoice { name: "EPSON PJ".into(), ordinal: 1 });
        before.fade_secs = 2.5;
        save_output_settings(&mut storage, &before);

        let mut after = OutputWindow::default();
        load_output_settings(&storage, &mut after);
        assert_eq!(after.display, before.display);
        assert_eq!(after.fade_secs, 2.5);

        // "Window" (no projector) is remembered too.
        before.display = None;
        save_output_settings(&mut storage, &before);
        after.display = Some(DisplayChoice { name: "stale".into(), ordinal: 0 });
        load_output_settings(&storage, &mut after);
        assert_eq!(after.display, None);
    }

    #[test]
    fn nothing_stored_keeps_the_defaults() {
        let mut out = OutputWindow::default();
        load_output_settings(&MemoryStorage::default(), &mut out);
        assert_eq!((out.display, out.fade_secs), (None, 1.0));
    }
}
