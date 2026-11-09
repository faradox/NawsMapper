//! The venue photo behind the canvas in the editor (`Canvas::backdrop`).
//! It is an egui texture, not a render source: it never reaches the
//! projector. Decoded on a background thread like media images.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use glam::Vec2;

/// Longest side kept for the editor; a preview doesn't need a 50-megapixel photo.
const MAX_SIDE: u32 = 4096;

#[derive(Default)]
pub struct BackdropImage {
    path: Option<PathBuf>,
    texture: Option<egui::TextureHandle>,
    /// Size of the original photo, for placing it on the canvas.
    size: Vec2,
    loading: Option<Receiver<Result<(egui::ColorImage, Vec2), String>>>,
    pub error: Option<String>,
}

impl BackdropImage {
    /// Starts loading when the project's photo changed; picks up finished loads.
    pub fn sync(&mut self, ctx: &egui::Context, path: Option<&Path>) {
        if self.path.as_deref() != path {
            self.path = path.map(Path::to_path_buf);
            self.texture = None;
            self.error = None;
            self.loading = path.map(|p| start(ctx, p.to_path_buf()));
        }
        let Some(rx) = &self.loading else { return };
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("the decoder stopped".into()),
        };
        self.loading = None;
        match result {
            Ok((image, size)) => {
                self.texture = Some(ctx.load_texture("venue photo", image, egui::TextureOptions::LINEAR));
                self.size = size;
            }
            Err(e) => {
                log::warn!("venue photo: {e}");
                self.error = Some(e);
            }
        }
    }

    /// The texture and the photo's original pixel size, once loaded.
    pub fn get(&self) -> Option<(egui::TextureId, Vec2)> {
        self.texture.as_ref().map(|t| (t.id(), self.size))
    }

    pub fn is_loading(&self) -> bool {
        self.loading.is_some()
    }
}

fn start(ctx: &egui::Context, path: PathBuf) -> Receiver<Result<(egui::ColorImage, Vec2), String>> {
    let (tx, rx) = channel();
    let ctx = ctx.clone();
    let spawned = std::thread::Builder::new().name("venue photo".into()).spawn(move || {
        let _ = tx.send(decode(&path).map_err(|e| format!("{e:#}")));
        ctx.request_repaint();
    });
    if let Err(e) = spawned {
        log::error!("could not start loading the venue photo: {e}");
    }
    rx
}

fn decode(path: &Path) -> anyhow::Result<(egui::ColorImage, Vec2)> {
    let img = image::ImageReader::open(path)?.with_guessed_format()?.decode()?;
    let size = Vec2::new(img.width() as f32, img.height() as f32);
    let img = if img.width().max(img.height()) > MAX_SIDE {
        img.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let color = egui::ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    );
    Ok((color, size))
}
