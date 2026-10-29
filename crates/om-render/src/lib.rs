//! NawsMapper GPU compositor.
//!
//! [`Renderer`] draws a [`Project`] at a point in time into an offscreen
//! `Rgba8Unorm` texture the size of the canvas. The app displays that one
//! texture both in the editor preview and full-screen on the projector, so
//! what you see while editing is exactly what gets projected.
//!
//! Colors are kept in display (sRGB-encoded) space end to end, matching how
//! designers pick colors and how egui displays user textures.
//!
//! Blend modes that fixed-function blending can express exactly (Normal, Add,
//! Screen, Multiply, Subtract, Exclusion) use their own pipeline. All others
//! read what is already drawn: before such a surface, the area under it is
//! copied into a backdrop texture, and the shader blends against that.

mod textures;

use std::collections::HashMap;
use std::num::NonZeroU64;

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Vec2};
use om_core::anim::evaluate;
use om_core::border::border_path;
use om_core::geom::quad_aspect;
use om_core::shape::{MAX_OUTLINE_POINTS, outline_uv};
use om_core::{BlendMode, Generator, Mapping, Mask, Palette, Project, SourceId, SourceKind, Surface};

pub use om_media::{VIDEO_EXTENSIONS, VideoInfo};
use textures::TextureCache;

/// Format of the output texture (required by egui for native textures).
pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Pixel size of one tile in the thumbnail atlas.
pub const THUMB_SIZE: (u32, u32) = (192, 120);
const THUMB_COLUMNS: u32 = 6;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SurfaceUniforms {
    corners0: [f32; 4],
    corners1: [f32; 4],
    inv_h: [[f32; 4]; 3],
    uv_m: [[f32; 4]; 3],
    canvas: [f32; 4],
    tint: [f32; 4],
    color_a: [f32; 4],
    color_b: [f32; 4],
    gen_params: [f32; 4],
    misc: [f32; 4],
    pal: [f32; 4],
    stops: [[f32; 4]; MAX_STOPS],
    shape: [f32; 4],
    points: [[f32; 4]; MAX_OUTLINE_POINTS / 2],
    border: [f32; 4],
    border_p: [f32; 4],
    border_q: [f32; 4],
    arc: [[f32; 4]; MAX_OUTLINE_POINTS / 4],
}

const MAX_STOPS: usize = 6;

fn mat3_cols(m: Mat3) -> [[f32; 4]; 3] {
    [m.x_axis.extend(0.0).into(), m.y_axis.extend(0.0).into(), m.z_axis.extend(0.0).into()]
}

/// The offscreen render target that receives the composed canvas.
pub struct OutputTarget {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// One pipeline per fixed-function blend mode.
    pipelines: HashMap<BlendMode, wgpu::RenderPipeline>,
    /// Normal blending; the shader does the blend math against the backdrop.
    backdrop_pipeline: wgpu::RenderPipeline,
    backdrop_layout: wgpu::BindGroupLayout,
    /// Copy of the canvas under the surface being drawn. Grows to the
    /// largest target it was used with.
    backdrop: (wgpu::Texture, wgpu::BindGroup),
    uniform_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    uniform_capacity: usize,
    uniform_stride: u64,
    textures: TextureCache,
    output: OutputTarget,
    output_generation: u64,
    /// Live previews of generators (shader library), one tile each.
    thumbnails: Option<OutputTarget>,
    thumbnail_generation: u64,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("om surface shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("surface.wgsl").into()),
        });

        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("om surface uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: NonZeroU64::new(size_of::<SurfaceUniforms>() as u64),
                },
                count: None,
            }],
        });

        let textures = TextureCache::new(device, queue);

        let backdrop_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("om backdrop"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("om surface layout"),
            bind_group_layouts: &[Some(&uniform_layout), Some(textures.layout()), Some(&backdrop_layout)],
            immediate_size: 0,
        });

        let pipelines = BlendMode::all()
            .filter_map(|mode| {
                let blend = fixed_blend(mode)?;
                Some((
                    mode,
                    create_pipeline(device, &layout, &shader, blend, &format!("om surface {mode:?}")),
                ))
            })
            .collect();
        let backdrop_pipeline = create_pipeline(
            device,
            &layout,
            &shader,
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "om surface backdrop blend",
        );
        let backdrop = create_backdrop(device, &backdrop_layout, 1, 1);

        let align = device.limits().min_uniform_buffer_offset_alignment as u64;
        let uniform_stride = (size_of::<SurfaceUniforms>() as u64).div_ceil(align) * align;
        let (uniform_buffer, uniform_bind_group) =
            create_uniforms(device, &uniform_layout, uniform_stride, 16);

        let output = create_output(device, 1920, 1080);

        Self {
            device: device.clone(),
            queue: queue.clone(),
            pipelines,
            backdrop_pipeline,
            backdrop_layout,
            backdrop,
            uniform_layout,
            uniform_buffer,
            uniform_bind_group,
            uniform_capacity: 16,
            uniform_stride,
            textures,
            output,
            output_generation: 0,
            thumbnails: None,
            thumbnail_generation: 0,
        }
    }

    pub fn output(&self) -> &OutputTarget {
        &self.output
    }

    /// Increments whenever the output texture is re-created (canvas resize),
    /// so callers know to re-register it with their UI toolkit.
    pub fn output_generation(&self) -> u64 {
        self.output_generation
    }

    /// Error message for a source that failed to load, if any.
    pub fn source_error(&self, id: SourceId) -> Option<&str> {
        self.textures.error(id)
    }

    /// Pixel size of a loaded image or video source.
    pub fn source_size(&self, id: SourceId) -> Option<(u32, u32)> {
        self.textures.size(id)
    }

    pub fn video_info(&self, id: SourceId) -> Option<&VideoInfo> {
        self.textures.video_info(id)
    }

    /// `waker` is called from decoder threads when a new video frame is
    /// ready, so the UI can repaint even while paused.
    pub fn set_frame_waker(&mut self, waker: impl Fn() + Send + Sync + 'static) {
        self.textures.set_waker(std::sync::Arc::new(waker));
    }

    /// Renders the project at `time` seconds and submits the work.
    pub fn render(&mut self, project: &Project, time: f64) {
        let (w, h) = (project.canvas.width.max(1), project.canvas.height.max(1));
        if (w, h) != (self.output.width, self.output.height) {
            self.output = create_output(&self.device, w, h);
            self.output_generation += 1;
        }
        self.textures.sync(&self.device, &self.queue, project, time);
        let target = self.output.texture.clone();
        self.draw(project, time, &target);
    }

    /// Renders one [`THUMB_SIZE`] tile per generator into the thumbnail
    /// atlas, row by row. See [`Renderer::thumbnail_uv`].
    pub fn render_thumbnails(&mut self, generators: &[Generator], time: f64) {
        let rows = (generators.len() as u32).div_ceil(THUMB_COLUMNS).max(1);
        let (w, h) = (THUMB_SIZE.0 * THUMB_COLUMNS, THUMB_SIZE.1 * rows);
        if self.thumbnails.as_ref().is_none_or(|t| (t.width, t.height) != (w, h)) {
            self.thumbnails = Some(create_output(&self.device, w, h));
            self.thumbnail_generation += 1;
        }
        let mut project = Project::empty();
        project.canvas.width = w;
        project.canvas.height = h;
        let tile = Vec2::new(THUMB_SIZE.0 as f32, THUMB_SIZE.1 as f32);
        for (i, g) in generators.iter().enumerate() {
            let source = project.add_source("", SourceKind::Generator(g.clone()));
            let id = project.new_surface_id();
            let cell = Vec2::new((i as u32 % THUMB_COLUMNS) as f32, (i as u32 / THUMB_COLUMNS) as f32);
            let mut s = Surface::rect(id, "", cell * tile, tile);
            s.source = Some(source);
            project.surfaces.push(s);
        }
        let target = self.thumbnails.as_ref().map(|t| t.texture.clone()).expect("created above");
        self.draw(&project, time, &target);
    }

    pub fn thumbnails(&self) -> Option<&OutputTarget> {
        self.thumbnails.as_ref()
    }

    /// Increments whenever the thumbnail atlas is re-created.
    pub fn thumbnail_generation(&self) -> u64 {
        self.thumbnail_generation
    }

    /// Texture coordinates (min, max) of tile `index` in the thumbnail atlas.
    pub fn thumbnail_uv(&self, index: usize) -> ([f32; 2], [f32; 2]) {
        let Some(t) = &self.thumbnails else { return ([0.0; 2], [0.0; 2]) };
        let (col, row) = (index as u32 % THUMB_COLUMNS, index as u32 / THUMB_COLUMNS);
        let (tw, th) = (THUMB_SIZE.0 as f32 / t.width as f32, THUMB_SIZE.1 as f32 / t.height as f32);
        ([col as f32 * tw, row as f32 * th], [(col + 1) as f32 * tw, (row + 1) as f32 * th])
    }

    fn draw(&mut self, project: &Project, time: f64, target: &wgpu::Texture) {
        let visible: Vec<&Surface> = project.surfaces.iter().filter(|s| s.visible).collect();
        self.ensure_uniform_capacity(visible.len());
        let size = (target.width(), target.height());
        let needs_backdrop = visible.iter().any(|s| fixed_blend(effective_blend(s)).is_none());
        let (bw, bh) = (self.backdrop.0.width(), self.backdrop.0.height());
        if needs_backdrop && (bw < size.0 || bh < size.1) {
            self.backdrop =
                create_backdrop(&self.device, &self.backdrop_layout, bw.max(size.0), bh.max(size.1));
        }
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());

        let canvas = project.canvas.size();
        let mut data = vec![0u8; self.uniform_stride as usize * visible.len().max(1)];
        for (i, s) in visible.iter().enumerate() {
            let u = surface_uniforms(project, s, canvas, time as f32);
            let off = i * self.uniform_stride as usize;
            data[off..off + size_of::<SurfaceUniforms>()].copy_from_slice(bytemuck::bytes_of(&u));
        }
        self.queue.write_buffer(&self.uniform_buffer, 0, &data);

        let bg = project.canvas.background;
        let clear = wgpu::Color { r: bg[0] as f64, g: bg[1] as f64, b: bg[2] as f64, a: bg[3] as f64 };
        let mut encoder =
            self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("om render") });
        let mut pass = Some(begin_pass(&mut encoder, &view, wgpu::LoadOp::Clear(clear), &self.backdrop.1));
        for (i, s) in visible.iter().enumerate() {
            let pipeline = match self.pipelines.get(&effective_blend(s)) {
                Some(p) => p,
                None => {
                    // Off-canvas surfaces draw nothing.
                    let Some((origin, extent)) = pixel_bounds(s.corners, size) else { continue };
                    // A copy can't happen inside a pass: end it, copy, continue.
                    drop(pass.take());
                    encoder.copy_texture_to_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: target,
                            mip_level: 0,
                            origin,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::TexelCopyTextureInfo {
                            texture: &self.backdrop.0,
                            mip_level: 0,
                            origin,
                            aspect: wgpu::TextureAspect::All,
                        },
                        extent,
                    );
                    pass = Some(begin_pass(&mut encoder, &view, wgpu::LoadOp::Load, &self.backdrop.1));
                    &self.backdrop_pipeline
                }
            };
            let pass = pass.as_mut().expect("a pass is always open here");
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.uniform_bind_group, &[(i as u64 * self.uniform_stride) as u32]);
            pass.set_bind_group(1, self.textures.bind_group_for(s.source, project), &[]);
            pass.draw(0..6, 0..1);
        }
        drop(pass);
        self.queue.submit([encoder.finish()]);
    }

    fn ensure_uniform_capacity(&mut self, count: usize) {
        if count <= self.uniform_capacity {
            return;
        }
        let cap = count.next_power_of_two();
        let (buf, bg) = create_uniforms(&self.device, &self.uniform_layout, self.uniform_stride, cap);
        self.uniform_buffer = buf;
        self.uniform_bind_group = bg;
        self.uniform_capacity = cap;
    }
}

fn begin_pass(
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
    backdrop: &wgpu::BindGroup,
) -> wgpu::RenderPass<'static> {
    let mut pass = encoder
        .begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("om canvas"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
        .forget_lifetime();
    pass.set_bind_group(2, backdrop, &[]);
    pass
}

/// Cutouts always paint normally.
fn effective_blend(s: &Surface) -> BlendMode {
    if s.cutout { BlendMode::Normal } else { s.blend }
}

/// Pixel rectangle covered by a surface's corners, clipped to the target.
fn pixel_bounds(corners: [Vec2; 4], size: (u32, u32)) -> Option<(wgpu::Origin3d, wgpu::Extent3d)> {
    let min = corners.iter().fold(Vec2::INFINITY, |a, c| a.min(*c)).floor().max(Vec2::ZERO);
    let max = corners.iter().fold(Vec2::NEG_INFINITY, |a, c| a.max(*c)).ceil();
    let max = max.min(Vec2::new(size.0 as f32, size.1 as f32));
    if !(min.x < max.x && min.y < max.y) {
        return None;
    }
    Some((
        wgpu::Origin3d { x: min.x as u32, y: min.y as u32, z: 0 },
        wgpu::Extent3d {
            width: (max.x - min.x) as u32,
            height: (max.y - min.y) as u32,
            depth_or_array_layers: 1,
        },
    ))
}

/// Blend state for modes that fixed-function blending computes exactly
/// (the shader outputs premultiplied alpha), `None` for the backdrop modes.
fn fixed_blend(mode: BlendMode) -> Option<wgpu::BlendState> {
    use wgpu::{BlendComponent, BlendFactor as F, BlendOperation as Op};
    let comp = |src, dst| BlendComponent { src_factor: src, dst_factor: dst, operation: Op::Add };
    let alpha = comp(F::One, F::OneMinusSrcAlpha);
    let color = match mode {
        BlendMode::Normal => return Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        BlendMode::Add => comp(F::One, F::One),
        BlendMode::Screen => comp(F::One, F::OneMinusSrc),
        BlendMode::Multiply => comp(F::Dst, F::OneMinusSrcAlpha),
        // dst - src·a
        BlendMode::Subtract => {
            BlendComponent { src_factor: F::One, dst_factor: F::One, operation: Op::ReverseSubtract }
        }
        // src·a·(1 - dst) + dst·(1 - src·a) = dst + a·(src + dst - 2·src·dst)
        BlendMode::Exclusion => comp(F::OneMinusDst, F::OneMinusSrc),
        _ => return None,
    };
    Some(wgpu::BlendState { color, alpha })
}

/// The shader's blend function for a backdrop mode (`0` = blending is done
/// by the pipeline). Must match `blend()` in surface.wgsl.
fn backdrop_index(mode: BlendMode) -> u32 {
    match mode {
        BlendMode::Darken => 1,
        BlendMode::Lighten => 2,
        BlendMode::ColorBurn => 3,
        BlendMode::LinearBurn => 4,
        BlendMode::ColorDodge => 5,
        BlendMode::Overlay => 6,
        BlendMode::SoftLight => 7,
        BlendMode::HardLight => 8,
        BlendMode::VividLight => 9,
        BlendMode::LinearLight => 10,
        BlendMode::PinLight => 11,
        BlendMode::HardMix => 12,
        BlendMode::Difference => 13,
        BlendMode::Divide => 14,
        BlendMode::Hue => 15,
        BlendMode::Saturation => 16,
        BlendMode::Color => 17,
        BlendMode::Luminosity => 18,
        BlendMode::Normal
        | BlendMode::Add
        | BlendMode::Screen
        | BlendMode::Multiply
        | BlendMode::Subtract
        | BlendMode::Exclusion => 0,
    }
}

fn surface_uniforms(project: &Project, s: &Surface, canvas: Vec2, time: f32) -> SurfaceUniforms {
    let frame = evaluate(s, canvas, time);
    let c = s.corners;

    // A cutout paints the canvas background over everything below it, using
    // the same coverage (mask, feather, opacity) as a normal surface.
    let bg = project.canvas.background;
    let mut palette = (Palette::Duotone, 0.5);
    let (kind, color_a, color_b, scale, speed, angle) = match s
        .source
        .and_then(|id| project.source(id))
        .map(|src| &src.kind)
    {
        _ if s.cutout => (1.0, [bg[0], bg[1], bg[2], 1.0], [0.0; 4], 1.0, 0.0, 0.0),
        None => (1.0, [0.0; 4], [0.0; 4], 1.0, 0.0, 0.0),
        Some(SourceKind::Solid { color }) => (1.0, *color, *color, 1.0, 0.0, 0.0),
        Some(SourceKind::Image { .. } | SourceKind::Video { .. }) => (0.0, [1.0; 4], [1.0; 4], 1.0, 0.0, 0.0),
        Some(SourceKind::Generator(g)) => {
            palette = (g.palette, g.detail);
            (g.pattern.shader_index() as f32, g.color_a, g.color_b, g.scale, g.speed, g.angle.to_radians())
        }
    };
    let stop_list = palette.0.stops(color_a, color_b);
    let mut stops = [[0.0; 4]; MAX_STOPS];
    for (dst, src) in stops.iter_mut().zip(&stop_list) {
        *dst = *src;
    }

    let mask = match s.mask {
        Mask::Quad => 0.0,
        Mask::Ellipse => 1.0,
        Mask::Triangle => 2.0,
        _ => 3.0,
    };
    // Broken borders need the outline with distances along it (for any
    // shape); polygon shapes need it for their coverage. The subdivided
    // border outline is the same polygon, so one list serves both.
    let border = &s.border;
    let border_on = border.enabled && border.width > 0.0 && border.color[3] > 0.0;
    let period = border.period();
    let mut arc = [[0.0; 4]; MAX_OUTLINE_POINTS / 4];
    let mut perimeter = 0.0;
    let outline = if border_on && border.style.is_broken() {
        let bp = border_path(s.mask, &s.path, c, period);
        for (i, d) in bp.distances.iter().take(MAX_OUTLINE_POINTS).enumerate() {
            arc[i / 4][i % 4] = *d;
        }
        perimeter = bp.perimeter;
        bp.points
    } else if s.mask.is_polygon() {
        outline_uv(s.mask, &s.path)
    } else {
        Vec::new()
    };
    let mut points = [[0.0; 4]; MAX_OUTLINE_POINTS / 2];
    let point_count = outline.len().min(MAX_OUTLINE_POINTS);
    for (i, p) in outline.iter().take(point_count).enumerate() {
        points[i / 2][(i % 2) * 2] = p.x;
        points[i / 2][(i % 2) * 2 + 1] = p.y;
    }
    // Flow is a pure function of (local) time, wrapped to keep precision.
    let flow = if period > 0.0 { (border.flow * frame.local_time).rem_euclid(period) } else { 0.0 };
    let mapping = match s.mapping {
        Mapping::Surface => 0.0,
        Mapping::Canvas => 1.0,
    };
    let aspect = match s.mapping {
        Mapping::Surface => quad_aspect(c),
        Mapping::Canvas => canvas.x / canvas.y.max(1.0),
    };

    SurfaceUniforms {
        corners0: [c[0].x, c[0].y, c[1].x, c[1].y],
        corners1: [c[2].x, c[2].y, c[3].x, c[3].y],
        inv_h: mat3_cols(frame.canvas_to_surface),
        uv_m: mat3_cols(frame.uv_matrix),
        canvas: [canvas.x, canvas.y, frame.local_time, mapping],
        tint: if s.cutout { [1.0; 4] } else { s.tint },
        color_a,
        color_b,
        gen_params: [kind, scale, speed, angle],
        misc: [frame.opacity, if s.cutout { 0.0 } else { frame.hue_shift }, s.feather.clamp(0.0, 0.5), mask],
        pal: [
            stop_list.len().min(MAX_STOPS) as f32,
            if palette.0.is_cyclic() { 1.0 } else { 0.0 },
            palette.1.clamp(0.0, 1.0),
            aspect,
        ],
        stops,
        shape: [point_count as f32, backdrop_index(effective_blend(s)) as f32, 0.0, 0.0],
        points,
        border: border.color,
        border_p: [
            if border_on { border.width } else { 0.0 },
            border.style.shader_index() as f32,
            border.dash.max(0.5),
            border.gap.max(0.0),
        ],
        border_q: [flow, perimeter, 0.0, 0.0],
        arc,
    }
}

fn create_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    blend: wgpu::BlendState,
    label: &str,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: OUTPUT_FORMAT,
                blend: Some(blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn create_uniforms(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    stride: u64,
    capacity: usize,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("om surface uniforms"),
        size: stride * capacity as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("om surface uniforms"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: NonZeroU64::new(size_of::<SurfaceUniforms>() as u64),
            }),
        }],
    });
    (buffer, bind_group)
}

fn create_backdrop(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::BindGroup) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("om backdrop"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OUTPUT_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("om backdrop"),
        layout,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
    });
    (texture, bind_group)
}

fn create_output(device: &wgpu::Device, width: u32, height: u32) -> OutputTarget {
    let max = device.limits().max_texture_dimension_2d;
    let (width, height) = (width.min(max), height.min(max));
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("om output"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OUTPUT_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    OutputTarget { texture, view, width, height }
}

#[cfg(test)]
mod tests {
    #[test]
    fn surface_shader_validates() {
        let module = naga::front::wgsl::parse_str(include_str!("surface.wgsl"))
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(include_str!("surface.wgsl"))));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    #[test]
    fn cutout_paints_the_background() {
        let mut p = om_core::Project::demo();
        p.canvas.background = [0.1, 0.2, 0.3, 1.0];
        p.surfaces[0].cutout = true;
        p.surfaces[0].tint = [0.5; 4];
        p.surfaces[0].animation.hue_speed = 1.0;
        let u = super::surface_uniforms(&p, &p.surfaces[0], p.canvas.size(), 0.5);
        assert_eq!(u.gen_params[0], 1.0, "drawn as a solid color");
        assert_eq!(u.color_a, [0.1, 0.2, 0.3, 1.0]);
        assert_eq!(u.tint, [1.0; 4]);
        assert_eq!(u.misc[1], 0.0, "no hue shift");
    }

    #[test]
    fn polygon_shapes_send_their_outline() {
        let mut p = om_core::Project::demo();
        let u = super::surface_uniforms(&p, &p.surfaces[0], p.canvas.size(), 0.0);
        assert_eq!((u.misc[3], u.shape[0]), (0.0, 0.0), "quad is analytic");

        let s = &mut p.surfaces[0];
        s.mask = om_core::Mask::Star;
        let u = super::surface_uniforms(&p, &p.surfaces[0], p.canvas.size(), 0.0);
        assert_eq!((u.misc[3], u.shape[0]), (3.0, 10.0));
        // First star tip points up from the top-center.
        assert!((u.points[0][0] - 0.5).abs() < 1e-4 && u.points[0][1].abs() < 1e-4);
    }

    #[test]
    fn borders_reach_the_shader() {
        let mut p = om_core::Project::demo();
        let u = super::surface_uniforms(&p, &p.surfaces[0], p.canvas.size(), 0.0);
        assert_eq!(u.border_p[0], 0.0, "off by default");

        let b = &mut p.surfaces[0].border;
        b.enabled = true;
        b.width = 8.0;
        let u = super::surface_uniforms(&p, &p.surfaces[0], p.canvas.size(), 0.0);
        assert_eq!((u.border_p[0], u.border_p[1]), (8.0, 0.0));
        assert_eq!(u.shape[0], 0.0, "a solid border on a quad needs no outline");

        let b = &mut p.surfaces[0].border;
        b.style = om_core::BorderStyle::Dashed;
        b.flow = 10.0;
        let u = super::surface_uniforms(&p, &p.surfaces[0], p.canvas.size(), 1.0);
        assert_eq!(u.misc[3], 0.0, "coverage stays analytic");
        assert!(u.shape[0] > 4.0, "subdivided outline for the dashes");
        let period = p.surfaces[0].border.period();
        let periods = u.border_q[1] / period;
        assert!((periods - periods.round()).abs() < 1e-4, "whole number of dashes");
        // Local time 1 s (minus the demo's time offset of 0) → 10 px of flow.
        assert_eq!(u.border_q[0], 10.0);
    }

    #[test]
    fn generator_palette_reaches_the_shader() {
        let mut p = om_core::Project::demo();
        let id = p.surfaces[0].source.unwrap();
        if let Some(om_core::SourceKind::Generator(g)) = p.source_mut(id).map(|s| &mut s.kind) {
            g.palette = om_core::Palette::Fire;
            g.detail = 0.25;
        }
        let u = super::surface_uniforms(&p, &p.surfaces[0], p.canvas.size(), 0.0);
        assert_eq!(u.pal[0], 5.0);
        assert_eq!(u.pal[2], 0.25);
        assert_eq!(u.stops[0], [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn every_blend_mode_has_a_pipeline_or_a_shader_function() {
        let mut seen = Vec::new();
        for mode in om_core::BlendMode::all() {
            let index = super::backdrop_index(mode);
            assert_eq!(super::fixed_blend(mode).is_some(), index == 0, "{mode:?}");
            if index > 0 {
                assert!(!seen.contains(&index), "{mode:?} reuses {index}");
                seen.push(index);
            }
        }
        assert_eq!(seen.len(), 18);
    }

    #[test]
    fn backdrop_blend_reaches_the_shader() {
        let mut p = om_core::Project::demo();
        p.surfaces[0].blend = om_core::BlendMode::Overlay;
        let u = super::surface_uniforms(&p, &p.surfaces[0], p.canvas.size(), 0.0);
        assert_eq!(u.shape[1], 6.0);
        p.surfaces[0].cutout = true;
        let u = super::surface_uniforms(&p, &p.surfaces[0], p.canvas.size(), 0.0);
        assert_eq!(u.shape[1], 0.0, "cutouts paint normally");
    }

    #[test]
    fn pixel_bounds_are_clipped_to_the_target() {
        use glam::Vec2;
        let c = [Vec2::new(-10.0, 5.2), Vec2::new(50.5, 5.0), Vec2::new(60.0, 300.0), Vec2::new(0.0, 40.0)];
        let (o, e) = super::pixel_bounds(c, (100, 100)).unwrap();
        assert_eq!((o.x, o.y, e.width, e.height), (0, 5, 60, 95));
        let off = c.map(|v| v + Vec2::new(500.0, 0.0));
        assert!(super::pixel_bounds(off, (100, 100)).is_none());
    }

    #[test]
    fn uniforms_match_shader_layout() {
        // 121 vec4s, see `SurfaceUniforms` in surface.wgsl.
        assert_eq!(size_of::<super::SurfaceUniforms>(), 121 * 16);
    }
}
