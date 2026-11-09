//! File dialogs that never block the event loop, plus in-app prompts
//! (unsaved changes, errors).
//!
//! A blocking file dialog opened from inside winit's event handler runs a
//! nested macOS run loop that can re-enter the handler, which winit treats as
//! a fatal error. So every file dialog goes through rfd's async API: it is
//! created on the UI thread (as a sheet on our window on macOS), awaited on a
//! helper thread, and its result comes back through a channel that the app
//! polls once per frame. Confirmations and errors are egui modals for the
//! same reason.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use om_core::SourceId;

/// What to do once the user has dealt with unsaved changes.
#[derive(Clone, Debug, PartialEq)]
pub enum Then {
    NewDemo,
    NewEmpty,
    /// Show the Open dialog.
    Open,
    OpenPath(PathBuf),
    Quit,
}

/// Why a file dialog was opened; decides what happens with the picked paths.
#[derive(Clone, Debug, PartialEq)]
pub enum Purpose {
    Open,
    SaveAs {
        then: Option<Then>,
    },
    ImportMedia,
    ReplaceMedia(SourceId),
    ExportTemplate,
    /// Pick the folder to create the package in.
    Package,
    ChooseBackdrop,
}

pub enum Kind {
    PickFile,
    PickFiles,
    PickFolder,
    SaveFile,
}

type PathsFuture = Pin<Box<dyn Future<Output = Vec<PathBuf>> + Send>>;

#[derive(Default)]
pub struct FileDialogs {
    pending: Option<(Purpose, Receiver<Vec<PathBuf>>)>,
}

impl FileDialogs {
    /// A dialog is open; further file actions are ignored until it closes.
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    /// Opens `dialog`. Must be called on the UI thread.
    pub fn start(&mut self, ctx: &egui::Context, purpose: Purpose, dialog: rfd::AsyncFileDialog, kind: Kind) {
        if self.busy() {
            return;
        }
        let one = |h: Option<rfd::FileHandle>| h.map(|h| vec![h.path().to_path_buf()]).unwrap_or_default();
        // Calling `pick_file()` etc. here (not inside the async block) is what
        // creates the dialog on this thread.
        let fut: PathsFuture = match kind {
            Kind::PickFile => {
                let f = dialog.pick_file();
                Box::pin(async move { one(f.await) })
            }
            Kind::SaveFile => {
                let f = dialog.save_file();
                Box::pin(async move { one(f.await) })
            }
            Kind::PickFolder => {
                let f = dialog.pick_folder();
                Box::pin(async move { one(f.await) })
            }
            Kind::PickFiles => {
                let f = dialog.pick_files();
                Box::pin(async move {
                    f.await.unwrap_or_default().iter().map(|h| h.path().to_path_buf()).collect()
                })
            }
        };
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new().name("file dialog".into()).spawn(move || {
            let _ = tx.send(pollster::block_on(fut));
            ctx.request_repaint();
        });
        match spawned {
            Ok(_) => self.pending = Some((purpose, rx)),
            Err(e) => log::error!("could not start file dialog thread: {e}"),
        }
    }

    /// The finished dialog, if any. An empty list means it was cancelled.
    pub fn poll(&mut self) -> Option<(Purpose, Vec<PathBuf>)> {
        let (_, rx) = self.pending.as_ref()?;
        let paths = match rx.try_recv() {
            Ok(paths) => paths,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Vec::new(),
        };
        self.pending.take().map(|(purpose, _)| (purpose, paths))
    }
}

/// An in-app modal question or message.
#[derive(Clone, Debug, PartialEq)]
pub enum Prompt {
    Unsaved {
        name: String,
        then: Then,
    },
    /// An error or a notice, with an OK button.
    Message {
        title: String,
        message: String,
    },
    /// Quitting in Show mode turns the projector dark: ask first.
    QuitShow,
    /// The last session left unsaved changes behind (a crash).
    Recover {
        name: String,
        age: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum PromptAnswer {
    Save(Then),
    Discard(Then),
    Dismiss,
    QuitShow,
    Restore,
    DropRecovery,
}

/// Shows `prompt` as a modal; returns the answer once the user picks one.
pub fn show_prompt(ctx: &egui::Context, prompt: &Prompt) -> Option<PromptAnswer> {
    let mut answer = None;
    let (enter, escape) = ctx.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
        )
    });
    let modal = egui::Modal::new(egui::Id::unique("prompt")).show(ctx, |ui| {
        ui.set_width(380.0);
        match prompt {
            Prompt::Unsaved { name, then } => {
                ui.heading("Unsaved changes");
                ui.add_space(4.0);
                ui.label(format!("Save changes to \"{name}\" first?"));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Don't Save").clicked() {
                        answer = Some(PromptAnswer::Discard(then.clone()));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Save").strong()).clicked() || enter {
                            answer = Some(PromptAnswer::Save(then.clone()));
                        }
                        if ui.button("Cancel").clicked() {
                            answer = Some(PromptAnswer::Dismiss);
                        }
                    });
                });
            }
            Prompt::Message { title, message } => {
                ui.heading(title);
                ui.add_space(4.0);
                ui.label(message);
                ui.add_space(10.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("OK").clicked() || enter {
                        answer = Some(PromptAnswer::Dismiss);
                    }
                });
            }
            Prompt::QuitShow => {
                ui.heading("Quit during the show?");
                ui.add_space(4.0);
                ui.label("Show mode is on. When NawsMapper quits, the projector goes dark.");
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Quit").clicked() {
                        answer = Some(PromptAnswer::QuitShow);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Keep Running").strong()).clicked() || enter {
                            answer = Some(PromptAnswer::Dismiss);
                        }
                    });
                });
            }
            Prompt::Recover { name, age } => {
                ui.heading("Restore unsaved work?");
                ui.add_space(4.0);
                ui.label(format!(
                    "NawsMapper didn't close properly last time. Unsaved changes to \"{name}\" \
                     from {age} can be restored."
                ));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Discard Them").clicked() {
                        answer = Some(PromptAnswer::DropRecovery);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Restore").strong()).clicked() || enter {
                            answer = Some(PromptAnswer::Restore);
                        }
                    });
                });
            }
        }
    });
    // Recovery needs an explicit choice: carrying on would overwrite it.
    let dismissable = !matches!(prompt, Prompt::Recover { .. });
    if dismissable && (escape || modal.should_close()) {
        answer.get_or_insert(PromptAnswer::Dismiss);
    }
    answer
}
