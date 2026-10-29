//! GPU textures for image and video sources, plus a 1x1 white fallback used
//! by solid and generator sources (the shader always samples *something*)
//! and a black one for media that failed to load (no glaring white on the wall).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use om_core::{Project, SourceId, SourceKind};
use om_media::{VideoInfo, VideoPlayer};

/// Called from decoder threads when a new video frame is ready.
pub type Waker = Arc<dyn Fn() + Send + Sync>;

struct Entry {
    path: PathBuf,
    is_video: bool,
    /// `None` when loading failed; the error is kept in `errors`.
    gpu: Option<Gpu>,
    size: (u32, u32),
    video: Option<VideoPlayer>,
}

struct Gpu {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

pub struct TextureCache {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    fallback: wgpu::BindGroup,
    missing: wgpu::BindGroup,
    entries: HashMap<SourceId, Entry>,
    errors: HashMap<SourceId, String>,
    waker: Waker,
}

impl TextureCache {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("om source texture"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        // Repeat addressing so scrolling/zooming textures tile seamlessly.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("om source sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let fallback = upload(device, queue, &layout, &sampler, 1, 1, &[255; 4], "om white").bind_group;
        let missing =
            upload(device, queue, &layout, &sampler, 1, 1, &[0, 0, 0, 255], "om missing").bind_group;
        Self {
            layout,
            sampler,
            fallback,
            missing,
            entries: HashMap::new(),
            errors: HashMap::new(),
            waker: Arc::new(|| {}),
        }
    }

    pub fn set_waker(&mut self, waker: Waker) {
        self.waker = waker;
    }

    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    pub fn error(&self, id: SourceId) -> Option<&str> {
        self.errors.get(&id).map(String::as_str)
    }

    pub fn size(&self, id: SourceId) -> Option<(u32, u32)> {
        self.entries.get(&id).filter(|e| e.gpu.is_some()).map(|e| e.size)
    }

    pub fn video_info(&self, id: SourceId) -> Option<&VideoInfo> {
        self.entries.get(&id)?.video.as_ref().map(VideoPlayer::info)
    }

    /// Loads new or changed media sources, drops removed ones and uploads
    /// the video frames for `time`.
    pub fn sync(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, project: &Project, time: f64) {
        self.entries.retain(|id, _| project.source(*id).is_some());
        self.errors.retain(|id, _| project.source(*id).is_some());

        for src in &project.sources {
            let (path, is_video) = match &src.kind {
                SourceKind::Image { path } => (path, false),
                SourceKind::Video { path } => (path, true),
                _ => {
                    self.entries.remove(&src.id);
                    self.errors.remove(&src.id);
                    continue;
                }
            };
            let current =
                self.entries.get(&src.id).is_some_and(|e| &e.path == path && e.is_video == is_video);
            if !current {
                let entry = if is_video {
                    self.open_video(device, queue, src.id, path)
                } else {
                    self.load_image(device, queue, src.id, path)
                };
                self.entries.insert(src.id, entry);
            }
            if is_video {
                self.update_video(device, queue, src.id, time);
            }
        }
    }

    fn load_image(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, id: SourceId, path: &Path) -> Entry {
        let max = device.limits().max_texture_dimension_2d;
        match load_rgba(path, max) {
            Ok(img) => {
                self.errors.remove(&id);
                let (w, h) = img.dimensions();
                let gpu = upload(device, queue, &self.layout, &self.sampler, w, h, &img, "om image");
                Entry { path: path.to_path_buf(), is_video: false, gpu: Some(gpu), size: (w, h), video: None }
            }
            Err(e) => {
                log::warn!("failed to load image {}: {e:#}", path.display());
                self.errors.insert(id, format!("{e:#}"));
                Entry { path: path.to_path_buf(), is_video: false, gpu: None, size: (0, 0), video: None }
            }
        }
    }

    fn open_video(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, id: SourceId, path: &Path) -> Entry {
        let waker = self.waker.clone();
        match VideoPlayer::open(path, move || waker()) {
            Ok(player) => {
                self.errors.remove(&id);
                let (w, h) = (player.info().width.max(1), player.info().height.max(1));
                // Black until the first frame is decoded.
                let black = [0, 0, 0, 255].repeat((w * h) as usize);
                let gpu = upload(device, queue, &self.layout, &self.sampler, w, h, &black, "om video");
                Entry {
                    path: path.to_path_buf(),
                    is_video: true,
                    gpu: Some(gpu),
                    size: (w, h),
                    video: Some(player),
                }
            }
            Err(e) => {
                log::warn!("failed to open video {}: {e:#}", path.display());
                self.errors.insert(id, format!("{e:#}"));
                Entry { path: path.to_path_buf(), is_video: true, gpu: None, size: (0, 0), video: None }
            }
        }
    }

    fn update_video(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, id: SourceId, time: f64) {
        let Some(entry) = self.entries.get_mut(&id) else { return };
        let Some(player) = &entry.video else { return };
        player.seek(time);
        if let Some(err) = player.error() {
            self.errors.insert(id, err);
        }
        let Some(frame) = player.take_frame() else { return };
        if entry.size != (frame.width, frame.height) || entry.gpu.is_none() {
            entry.size = (frame.width, frame.height);
            entry.gpu = Some(upload(
                device,
                queue,
                &self.layout,
                &self.sampler,
                frame.width,
                frame.height,
                &frame.rgba,
                "om video",
            ));
        } else if let Some(gpu) = &entry.gpu {
            write_rgba(queue, &gpu.texture, frame.width, frame.height, &frame.rgba);
        }
    }

    pub fn bind_group_for(&self, source: Option<SourceId>, project: &Project) -> &wgpu::BindGroup {
        let Some(id) = source.filter(|id| project.source(*id).is_some_and(|s| s.kind.media_path().is_some()))
        else {
            return &self.fallback;
        };
        self.entries.get(&id).and_then(|e| e.gpu.as_ref()).map_or(&self.missing, |g| &g.bind_group)
    }
}

fn load_rgba(path: &Path, max_dim: u32) -> anyhow::Result<image::RgbaImage> {
    let img = image::ImageReader::open(path)?.with_guessed_format()?.decode()?;
    let img = if img.width() > max_dim || img.height() > max_dim {
        img.resize(max_dim, max_dim, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    Ok(img.to_rgba8())
}

#[allow(clippy::too_many_arguments)]
fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    width: u32,
    height: u32,
    rgba: &[u8],
    label: &str,
) -> Gpu {
    let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    write_rgba(queue, &texture, width, height, rgba);
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    });
    Gpu { texture, bind_group }
}

fn write_rgba(queue: &wgpu::Queue, texture: &wgpu::Texture, width: u32, height: u32, rgba: &[u8]) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
}
