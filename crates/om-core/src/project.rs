//! The project document: canvas, media sources and mapped surfaces.

use std::path::{Path, PathBuf};

use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::Color;
pub use crate::anim::TextureTransform;
use crate::anim::{Animation, Lfo, Wave};
use crate::border::Border;
use crate::shape::{Mask, PathPoint};

/// Bumped whenever the on-disk format changes incompatibly.
pub const FORMAT_VERSION: u32 = 1;

/// File extension for project files.
pub const FILE_EXTENSION: &str = "omap";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SurfaceId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourceId(pub u64);

/// The output raster that gets sent to the projector.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub background: Color,
}

impl Default for Canvas {
    fn default() -> Self {
        Self { width: 1920, height: 1080, background: [0.0, 0.0, 0.0, 1.0] }
    }
}

impl Canvas {
    pub fn size(&self) -> Vec2 {
        Vec2::new(self.width as f32, self.height as f32)
    }
}

/// Built-in procedural patterns ("shaders"). Rendered on the GPU, so they
/// cost nothing to load and animate smoothly at any resolution.
///
/// New patterns go at the end: the shader identifies them by position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pattern {
    #[default]
    Gradient,
    Checker,
    Stripes,
    Rings,
    Noise,
    Plasma,
    /// Calibration grid with crosshair and border.
    TestGrid,
    Tunnel,
    Kaleidoscope,
    Spiral,
    Liquid,
    Cells,
    Starfield,
    Lava,
    Truchet,
    Moire,
    Julia,
    Aurora,
    Fire,
    Ripples,
    Hexagons,
    Vortex,
    Sunburst,
    OpArt,
    Rain,
    Melt,
    Kali,
    Quasicrystal,
    Caustics,
    RetroGrid,
    Galaxy,
    Electric,
    Bubbles,
    Mandala,
    InkBlot,
    Silk,
    Glitch,
    Zoom,
    Halftone,
    TieDye,
    Groovy,
    TwistedChecker,
    Mandelbrot,
    Crystal,
    Chladni,
    FlowerOfLife,
    Rosettes,
    Cubes,
    Lasers,
    Bokeh,
    Marble,
    Gyroid,
    Equalizer,
}

impl Pattern {
    pub const ALL: [Pattern; 53] = [
        Pattern::Gradient,
        Pattern::Checker,
        Pattern::Stripes,
        Pattern::Rings,
        Pattern::Noise,
        Pattern::Plasma,
        Pattern::TestGrid,
        Pattern::Tunnel,
        Pattern::Kaleidoscope,
        Pattern::Spiral,
        Pattern::Liquid,
        Pattern::Cells,
        Pattern::Starfield,
        Pattern::Lava,
        Pattern::Truchet,
        Pattern::Moire,
        Pattern::Julia,
        Pattern::Aurora,
        Pattern::Fire,
        Pattern::Ripples,
        Pattern::Hexagons,
        Pattern::Vortex,
        Pattern::Sunburst,
        Pattern::OpArt,
        Pattern::Rain,
        Pattern::Melt,
        Pattern::Kali,
        Pattern::Quasicrystal,
        Pattern::Caustics,
        Pattern::RetroGrid,
        Pattern::Galaxy,
        Pattern::Electric,
        Pattern::Bubbles,
        Pattern::Mandala,
        Pattern::InkBlot,
        Pattern::Silk,
        Pattern::Glitch,
        Pattern::Zoom,
        Pattern::Halftone,
        Pattern::TieDye,
        Pattern::Groovy,
        Pattern::TwistedChecker,
        Pattern::Mandelbrot,
        Pattern::Crystal,
        Pattern::Chladni,
        Pattern::FlowerOfLife,
        Pattern::Rosettes,
        Pattern::Cubes,
        Pattern::Lasers,
        Pattern::Bokeh,
        Pattern::Marble,
        Pattern::Gyroid,
        Pattern::Equalizer,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Pattern::Gradient => "Gradient",
            Pattern::Checker => "Checker",
            Pattern::Stripes => "Stripes",
            Pattern::Rings => "Rings",
            Pattern::Noise => "Noise",
            Pattern::Plasma => "Plasma",
            Pattern::TestGrid => "Test Grid",
            Pattern::Tunnel => "Tunnel",
            Pattern::Kaleidoscope => "Kaleidoscope",
            Pattern::Spiral => "Hypno Spiral",
            Pattern::Liquid => "Liquid",
            Pattern::Cells => "Cells",
            Pattern::Starfield => "Warp Speed",
            Pattern::Lava => "Lava Lamp",
            Pattern::Truchet => "Truchet",
            Pattern::Moire => "Moiré",
            Pattern::Julia => "Julia Fractal",
            Pattern::Aurora => "Aurora",
            Pattern::Fire => "Fire",
            Pattern::Ripples => "Ripples",
            Pattern::Hexagons => "Hexagons",
            Pattern::Vortex => "Vortex",
            Pattern::Sunburst => "Sunburst",
            Pattern::OpArt => "Op Art",
            Pattern::Rain => "Digital Rain",
            Pattern::Melt => "Melt",
            Pattern::Kali => "Fractal Lace",
            Pattern::Quasicrystal => "Quasicrystal",
            Pattern::Caustics => "Caustics",
            Pattern::RetroGrid => "Retro Grid",
            Pattern::Galaxy => "Galaxy",
            Pattern::Electric => "Electric",
            Pattern::Bubbles => "Bubbles",
            Pattern::Mandala => "Mandala",
            Pattern::InkBlot => "Ink Blot",
            Pattern::Silk => "Silk",
            Pattern::Glitch => "Glitch",
            Pattern::Zoom => "Infinite Zoom",
            Pattern::Halftone => "Halftone",
            Pattern::TieDye => "Tie-Dye",
            Pattern::Groovy => "Groovy",
            Pattern::TwistedChecker => "Twisted Checker",
            Pattern::Mandelbrot => "Mandelbrot Dive",
            Pattern::Crystal => "Crystal",
            Pattern::Chladni => "Chladni",
            Pattern::FlowerOfLife => "Flower of Life",
            Pattern::Rosettes => "Rosettes",
            Pattern::Cubes => "Tumbling Blocks",
            Pattern::Lasers => "Lasers",
            Pattern::Bokeh => "Bokeh",
            Pattern::Marble => "Marble",
            Pattern::Gyroid => "Gyroid",
            Pattern::Equalizer => "Equalizer",
        }
    }

    /// What the generator's `detail` slider does for this pattern, if anything.
    pub fn detail_label(self) -> Option<&'static str> {
        Some(match self {
            Pattern::Tunnel => "Twist",
            Pattern::Kaleidoscope => "Mirrors",
            Pattern::Spiral => "Arms",
            Pattern::Liquid => "Warp",
            Pattern::Cells => "Edges",
            Pattern::Starfield => "Star size",
            Pattern::Lava => "Blobs",
            Pattern::Truchet => "Line width",
            Pattern::Moire => "Drift",
            Pattern::Julia => "Detail",
            Pattern::Aurora => "Softness",
            Pattern::Fire => "Height",
            Pattern::Ripples => "Drops",
            Pattern::Hexagons => "Wave size",
            Pattern::Vortex => "Twist",
            Pattern::Sunburst => "Color sweep",
            Pattern::OpArt => "Warp",
            Pattern::Rain => "Trail length",
            Pattern::Melt => "Warp",
            Pattern::Kali => "Depth",
            Pattern::Quasicrystal => "Symmetry",
            Pattern::Caustics => "Sharpness",
            Pattern::RetroGrid => "Sun size",
            Pattern::Galaxy => "Arms",
            Pattern::Electric => "Glow",
            Pattern::Bubbles => "Amount",
            Pattern::Mandala => "Petals",
            Pattern::InkBlot => "Spread",
            Pattern::Silk => "Strands",
            Pattern::Glitch => "Chaos",
            Pattern::Zoom => "Twist",
            Pattern::Halftone => "Dot size",
            Pattern::TieDye => "Crinkle",
            Pattern::Groovy => "Petals",
            Pattern::TwistedChecker => "Twist",
            Pattern::Mandelbrot => "Depth",
            Pattern::Crystal => "Folds",
            Pattern::Chladni => "Glow",
            Pattern::FlowerOfLife => "Line width",
            Pattern::Rosettes => "Layers",
            Pattern::Cubes => "Edges",
            Pattern::Lasers => "Haze",
            Pattern::Bokeh => "Blur",
            Pattern::Marble => "Veins",
            Pattern::Gyroid => "Thickness",
            Pattern::Equalizer => "Segments",
            _ => return None,
        })
    }

    /// Index used by the shader (`0` and `1` are reserved for image and solid).
    pub fn shader_index(self) -> u32 {
        2 + Pattern::ALL.iter().position(|p| *p == self).unwrap_or(0) as u32
    }
}

/// Colors a pattern is painted with. The pattern computes a position on the
/// palette per pixel; the palette turns that into a color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Palette {
    /// The generator's two colors.
    #[default]
    Duotone,
    Rainbow,
    Neon,
    Fire,
    Ocean,
    Sunset,
    Aurora,
    Acid,
    Ice,
    Candy,
    Mono,
    Vapor,
    Gold,
    Cosmic,
}

impl Palette {
    pub const ALL: [Palette; 14] = [
        Palette::Duotone,
        Palette::Rainbow,
        Palette::Neon,
        Palette::Fire,
        Palette::Ocean,
        Palette::Sunset,
        Palette::Aurora,
        Palette::Acid,
        Palette::Ice,
        Palette::Candy,
        Palette::Vapor,
        Palette::Gold,
        Palette::Cosmic,
        Palette::Mono,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Palette::Duotone => "Two colors",
            Palette::Rainbow => "Rainbow",
            Palette::Neon => "Neon",
            Palette::Fire => "Fire",
            Palette::Ocean => "Ocean",
            Palette::Sunset => "Sunset",
            Palette::Aurora => "Aurora",
            Palette::Acid => "Acid",
            Palette::Ice => "Ice",
            Palette::Candy => "Candy",
            Palette::Mono => "Black & white",
            Palette::Vapor => "Vaporwave",
            Palette::Gold => "Gold",
            Palette::Cosmic => "Cosmic",
        }
    }

    /// Rainbow wraps around (red follows violet); the other palettes run
    /// from their first color to their last and back.
    pub fn is_cyclic(self) -> bool {
        self == Palette::Rainbow
    }

    /// Gradient stops, sRGB. `a` and `b` are the generator's two colors.
    pub fn stops(self, a: Color, b: Color) -> Vec<Color> {
        let rgb = |c: &[[f32; 3]]| c.iter().map(|&[r, g, b]| [r, g, b, 1.0]).collect();
        match self {
            Palette::Duotone => vec![a, b],
            Palette::Rainbow => rgb(&[
                [1.0, 0.1, 0.1],
                [1.0, 0.85, 0.0],
                [0.1, 1.0, 0.2],
                [0.0, 0.85, 1.0],
                [0.25, 0.2, 1.0],
                [1.0, 0.1, 0.9],
            ]),
            Palette::Neon => rgb(&[
                [0.05, 0.0, 0.2],
                [0.55, 0.0, 1.0],
                [1.0, 0.1, 0.75],
                [0.0, 0.9, 1.0],
                [0.85, 1.0, 1.0],
            ]),
            Palette::Fire => rgb(&[
                [0.0, 0.0, 0.0],
                [0.55, 0.02, 0.0],
                [0.95, 0.3, 0.0],
                [1.0, 0.75, 0.1],
                [1.0, 1.0, 0.85],
            ]),
            Palette::Ocean => rgb(&[
                [0.0, 0.02, 0.1],
                [0.0, 0.15, 0.4],
                [0.0, 0.5, 0.7],
                [0.2, 0.85, 0.85],
                [0.85, 1.0, 1.0],
            ]),
            Palette::Sunset => rgb(&[
                [0.1, 0.0, 0.2],
                [0.45, 0.05, 0.45],
                [0.9, 0.2, 0.4],
                [1.0, 0.55, 0.2],
                [1.0, 0.85, 0.4],
            ]),
            Palette::Aurora => rgb(&[
                [0.0, 0.02, 0.08],
                [0.0, 0.45, 0.35],
                [0.2, 1.0, 0.5],
                [0.3, 0.6, 1.0],
                [0.75, 0.3, 1.0],
            ]),
            Palette::Acid => rgb(&[
                [0.0, 0.1, 0.0],
                [0.3, 0.9, 0.0],
                [0.95, 1.0, 0.0],
                [1.0, 0.15, 0.6],
                [0.35, 0.0, 0.85],
            ]),
            Palette::Ice => rgb(&[
                [0.0, 0.0, 0.03],
                [0.05, 0.1, 0.4],
                [0.2, 0.45, 0.9],
                [0.6, 0.85, 1.0],
                [1.0, 1.0, 1.0],
            ]),
            Palette::Candy => rgb(&[
                [1.0, 0.55, 0.75],
                [0.75, 0.6, 1.0],
                [0.5, 0.9, 0.85],
                [1.0, 0.85, 0.55],
                [1.0, 0.6, 0.65],
            ]),
            Palette::Mono => rgb(&[[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]]),
            Palette::Vapor => rgb(&[
                [0.08, 0.0, 0.2],
                [0.95, 0.25, 0.7],
                [0.45, 0.35, 1.0],
                [0.2, 0.9, 1.0],
                [1.0, 0.92, 0.95],
            ]),
            Palette::Gold => rgb(&[
                [0.02, 0.01, 0.0],
                [0.35, 0.15, 0.02],
                [0.8, 0.5, 0.1],
                [1.0, 0.82, 0.35],
                [1.0, 0.97, 0.85],
            ]),
            Palette::Cosmic => rgb(&[
                [0.0, 0.0, 0.05],
                [0.1, 0.05, 0.35],
                [0.5, 0.1, 0.6],
                [0.95, 0.35, 0.45],
                [1.0, 0.75, 0.45],
                [1.0, 1.0, 0.95],
            ]),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Generator {
    pub pattern: Pattern,
    pub color_a: Color,
    pub color_b: Color,
    /// Pattern repetitions across the texture.
    pub scale: f32,
    /// Built-in motion speed of the pattern itself.
    pub speed: f32,
    /// Degrees.
    pub angle: f32,
    /// With any palette but `Duotone`, `color_a` is the background.
    pub palette: Palette,
    /// Pattern-specific extra control, 0..1 (see `Pattern::detail_label`).
    pub detail: f32,
}

impl Default for Generator {
    fn default() -> Self {
        Self {
            pattern: Pattern::Gradient,
            color_a: [0.05, 0.1, 0.9, 1.0],
            color_b: [1.0, 0.2, 0.6, 1.0],
            scale: 4.0,
            speed: 0.25,
            angle: 0.0,
            palette: Palette::Duotone,
            detail: 0.5,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SourceKind {
    Solid {
        color: Color,
    },
    Generator(Generator),
    Image {
        path: PathBuf,
    },
    /// MP4 (H.264). Plays in a loop, following the transport clock.
    Video {
        path: PathBuf,
    },
}

impl SourceKind {
    pub fn label(&self) -> &'static str {
        match self {
            SourceKind::Solid { .. } => "Color",
            SourceKind::Generator(_) => "Generator",
            SourceKind::Image { .. } => "Image",
            SourceKind::Video { .. } => "Video",
        }
    }

    /// File backing this source, for images and videos.
    pub fn media_path(&self) -> Option<&Path> {
        match self {
            SourceKind::Image { path } | SourceKind::Video { path } => Some(path),
            _ => None,
        }
    }
}

/// A piece of media that surfaces can display. Several surfaces may share
/// one source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub id: SourceId,
    pub name: String,
    pub kind: SourceKind,
}

/// How the texture is laid onto the surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mapping {
    /// The whole texture is fitted into each surface (perspective-correct).
    #[default]
    Surface,
    /// The texture spans the whole canvas and each surface reveals the part
    /// underneath it — one image across many surfaces.
    Canvas,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlendMode {
    #[default]
    Normal,
    Add,
    Screen,
    Multiply,
    Darken,
    ColorBurn,
    LinearBurn,
    Lighten,
    ColorDodge,
    Overlay,
    SoftLight,
    HardLight,
    VividLight,
    LinearLight,
    PinLight,
    HardMix,
    Difference,
    Exclusion,
    Subtract,
    Divide,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl BlendMode {
    /// Photoshop's groups and order: darken, lighten, contrast, inversion, color.
    pub const GROUPS: [&[BlendMode]; 6] = [
        &[BlendMode::Normal],
        &[BlendMode::Darken, BlendMode::Multiply, BlendMode::ColorBurn, BlendMode::LinearBurn],
        &[BlendMode::Lighten, BlendMode::Screen, BlendMode::ColorDodge, BlendMode::Add],
        &[
            BlendMode::Overlay,
            BlendMode::SoftLight,
            BlendMode::HardLight,
            BlendMode::VividLight,
            BlendMode::LinearLight,
            BlendMode::PinLight,
            BlendMode::HardMix,
        ],
        &[BlendMode::Difference, BlendMode::Exclusion, BlendMode::Subtract, BlendMode::Divide],
        &[BlendMode::Hue, BlendMode::Saturation, BlendMode::Color, BlendMode::Luminosity],
    ];

    /// Every mode in menu order.
    pub fn all() -> impl Iterator<Item = BlendMode> {
        Self::GROUPS.into_iter().flatten().copied()
    }

    /// The next (`step` = 1) or previous (-1) mode in menu order, wrapping.
    pub fn cycle(self, step: i32) -> BlendMode {
        let all: Vec<BlendMode> = Self::all().collect();
        let i = all.iter().position(|m| *m == self).unwrap_or(0) as i32;
        all[(i + step).rem_euclid(all.len() as i32) as usize]
    }

    pub fn label(self) -> &'static str {
        match self {
            BlendMode::Normal => "Normal",
            BlendMode::Add => "Add (Linear Dodge)",
            BlendMode::Screen => "Screen",
            BlendMode::Multiply => "Multiply",
            BlendMode::Darken => "Darken",
            BlendMode::ColorBurn => "Color Burn",
            BlendMode::LinearBurn => "Linear Burn",
            BlendMode::Lighten => "Lighten",
            BlendMode::ColorDodge => "Color Dodge",
            BlendMode::Overlay => "Overlay",
            BlendMode::SoftLight => "Soft Light",
            BlendMode::HardLight => "Hard Light",
            BlendMode::VividLight => "Vivid Light",
            BlendMode::LinearLight => "Linear Light",
            BlendMode::PinLight => "Pin Light",
            BlendMode::HardMix => "Hard Mix",
            BlendMode::Difference => "Difference",
            BlendMode::Exclusion => "Exclusion",
            BlendMode::Subtract => "Subtract",
            BlendMode::Divide => "Divide",
            BlendMode::Hue => "Hue",
            BlendMode::Saturation => "Saturation",
            BlendMode::Color => "Color",
            BlendMode::Luminosity => "Luminosity",
        }
    }
}

/// A mapped shape on the canvas: four perspective corners plus everything
/// that controls what is drawn inside them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Surface {
    pub id: SurfaceId,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    /// Canvas pixels; order: top-left, top-right, bottom-right, bottom-left.
    pub corners: [Vec2; 4],
    pub mask: Mask,
    /// Anchor points of a `Mask::Freeform` shape, in the unit square of `corners`.
    pub path: Vec<PathPoint>,
    pub source: Option<SourceId>,
    pub opacity: f32,
    pub tint: Color,
    pub blend: BlendMode,
    /// Soft edge width as a fraction of the surface (0..0.5).
    pub feather: f32,
    /// Stroke along the inside of the shape's edge.
    pub border: Border,
    pub mapping: Mapping,
    pub texture: TextureTransform,
    pub animation: Animation,
    /// Cutout: instead of drawing its source, the surface punches a hole in
    /// every surface below it (showing the canvas background).
    pub cutout: bool,
}

impl Default for Surface {
    fn default() -> Self {
        Self::new(SurfaceId(0), "Surface", [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y])
    }
}

impl Surface {
    pub fn new(id: SurfaceId, name: impl Into<String>, corners: [Vec2; 4]) -> Self {
        Self {
            id,
            name: name.into(),
            visible: true,
            locked: false,
            corners,
            mask: Mask::Quad,
            path: Vec::new(),
            source: None,
            opacity: 1.0,
            tint: [1.0; 4],
            blend: BlendMode::Normal,
            feather: 0.0,
            border: Border::default(),
            mapping: Mapping::Surface,
            texture: TextureTransform::default(),
            animation: Animation::default(),
            cutout: false,
        }
    }

    /// An axis-aligned rectangle.
    pub fn rect(id: SurfaceId, name: impl Into<String>, min: Vec2, size: Vec2) -> Self {
        let max = min + size;
        Self::new(id, name, [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)])
    }

    pub fn translate(&mut self, delta: Vec2) {
        for c in &mut self.corners {
            *c += delta;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub format_version: u32,
    pub canvas: Canvas,
    pub sources: Vec<Source>,
    /// Draw order: index 0 is at the back.
    pub surfaces: Vec<Surface>,
    pub next_id: u64,
}

impl Default for Project {
    fn default() -> Self {
        Self::empty()
    }
}

impl Project {
    pub fn empty() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            canvas: Canvas::default(),
            sources: Vec::new(),
            surfaces: Vec::new(),
            next_id: 1,
        }
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn new_surface_id(&mut self) -> SurfaceId {
        SurfaceId(self.alloc_id())
    }

    pub fn add_source(&mut self, name: impl Into<String>, kind: SourceKind) -> SourceId {
        let id = SourceId(self.alloc_id());
        self.sources.push(Source { id, name: name.into(), kind });
        id
    }

    pub fn source(&self, id: SourceId) -> Option<&Source> {
        self.sources.iter().find(|s| s.id == id)
    }

    pub fn source_mut(&mut self, id: SourceId) -> Option<&mut Source> {
        self.sources.iter_mut().find(|s| s.id == id)
    }

    pub fn surface(&self, id: SurfaceId) -> Option<&Surface> {
        self.surfaces.iter().find(|s| s.id == id)
    }

    pub fn surface_mut(&mut self, id: SurfaceId) -> Option<&mut Surface> {
        self.surfaces.iter_mut().find(|s| s.id == id)
    }

    pub fn surface_index(&self, id: SurfaceId) -> Option<usize> {
        self.surfaces.iter().position(|s| s.id == id)
    }

    /// Removes a source and detaches it from all surfaces.
    pub fn remove_source(&mut self, id: SourceId) {
        self.sources.retain(|s| s.id != id);
        for s in &mut self.surfaces {
            if s.source == Some(id) {
                s.source = None;
            }
        }
    }

    /// A name like "Quad 3" that is not used yet.
    pub fn unique_surface_name(&self, base: &str) -> String {
        (1..)
            .map(|n| format!("{base} {n}"))
            .find(|name| !self.surfaces.iter().any(|s| &s.name == name))
            .unwrap()
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        let p: Project = serde_json::from_str(json)?;
        anyhow::ensure!(
            p.format_version <= FORMAT_VERSION,
            "project was saved by a newer NawsMapper (format {}, this build reads up to {})",
            p.format_version,
            FORMAT_VERSION
        );
        Ok(p)
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        std::fs::write(path, self.to_json()?)?;
        Ok(())
    }

    /// Loads a project. Relative media paths are resolved against the
    /// project file's directory.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let mut p = Self::from_json(&std::fs::read_to_string(path)?)?;
        if let Some(dir) = path.parent() {
            for s in &mut p.sources {
                if let SourceKind::Image { path } | SourceKind::Video { path } = &mut s.kind
                    && path.is_relative()
                {
                    *path = dir.join(&*path);
                }
            }
        }
        Ok(p)
    }

    /// A ready-to-project scene: three faces of a box (as seen from the
    /// front, slightly above) with staggered animated patterns, plus a
    /// calibration grid source.
    pub fn demo() -> Self {
        let mut p = Self::empty();
        p.add_source(
            "Test Grid",
            SourceKind::Generator(Generator {
                pattern: Pattern::TestGrid,
                color_a: [0.0, 0.0, 0.0, 1.0],
                color_b: [1.0, 1.0, 1.0, 1.0],
                scale: 8.0,
                speed: 0.0,
                angle: 0.0,
                ..Default::default()
            }),
        );
        let stripes = p.add_source(
            "Neon Stripes",
            SourceKind::Generator(Generator {
                pattern: Pattern::Stripes,
                color_a: [0.02, 0.02, 0.08, 1.0],
                color_b: [0.0, 0.9, 1.0, 1.0],
                scale: 6.0,
                speed: 0.6,
                angle: 45.0,
                ..Default::default()
            }),
        );
        let plasma = p.add_source(
            "Plasma",
            SourceKind::Generator(Generator {
                pattern: Pattern::Plasma,
                color_a: [0.3, 0.0, 0.6, 1.0],
                color_b: [1.0, 0.5, 0.1, 1.0],
                scale: 3.0,
                speed: 0.4,
                angle: 0.0,
                ..Default::default()
            }),
        );
        let rings = p.add_source(
            "Rings",
            SourceKind::Generator(Generator {
                pattern: Pattern::Rings,
                color_a: [0.0, 0.0, 0.0, 1.0],
                color_b: [1.0, 0.25, 0.55, 1.0],
                scale: 5.0,
                speed: 0.5,
                angle: 0.0,
                ..Default::default()
            }),
        );
        p.add_source("White", SourceKind::Solid { color: [1.0; 4] });

        // An isometric box centered on the canvas.
        let c = Vec2::new(960.0, 560.0);
        let (w, h, d) = (330.0, 190.0, 330.0);
        let top = [c + Vec2::new(0.0, -2.0 * h), c + Vec2::new(w, -h), c, c + Vec2::new(-w, -h)];
        let left = [c + Vec2::new(-w, -h), c, c + Vec2::new(0.0, d), c + Vec2::new(-w, d - h)];
        let right = [c, c + Vec2::new(w, -h), c + Vec2::new(w, d - h), c + Vec2::new(0.0, d)];

        for (i, (name, corners, src)) in
            [("Box Top", top, rings), ("Box Left", left, stripes), ("Box Right", right, plasma)]
                .into_iter()
                .enumerate()
        {
            let id = p.new_surface_id();
            let mut s = Surface::new(id, name, corners);
            s.source = Some(src);
            s.animation.time_offset = i as f32 * 0.4;
            p.surfaces.push(s);
        }
        p.surfaces[1].animation.zoom = Lfo { wave: Wave::Sine, rate: 0.25, amount: 0.15 };
        p.surfaces[2].animation.hue_speed = 0.05;
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trip() {
        let p = Project::demo();
        let back = Project::from_json(&p.to_json().unwrap()).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn missing_fields_get_defaults() {
        let p = Project::from_json(r#"{"surfaces":[{"id":5,"name":"x"}]}"#).unwrap();
        assert_eq!(p.surfaces[0].opacity, 1.0);
        assert_eq!(p.surfaces[0].animation.speed, 1.0);
    }

    #[test]
    fn rejects_newer_format() {
        assert!(Project::from_json(r#"{"format_version":999}"#).is_err());
    }

    #[test]
    fn remove_source_detaches_surfaces() {
        let mut p = Project::demo();
        let id = p.surfaces[0].source.unwrap();
        p.remove_source(id);
        assert_eq!(p.surfaces[0].source, None);
    }

    #[test]
    fn load_resolves_relative_media_paths() {
        let dir = std::env::temp_dir().join(format!("om-load-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut p = Project::empty();
        p.add_source("clip", SourceKind::Video { path: "media/clip.mp4".into() });
        let file = dir.join("p.omap");
        p.save(&file).unwrap();
        let back = Project::load(&file).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(back.sources[0].kind.media_path(), Some(dir.join("media/clip.mp4").as_path()));
    }

    #[test]
    fn shader_indices_are_stable() {
        assert_eq!(Pattern::Gradient.shader_index(), 2);
        assert_eq!(Pattern::TestGrid.shader_index(), 8);
        assert_eq!(Pattern::Tunnel.shader_index(), 9);
        assert_eq!(Pattern::Rain.shader_index(), 26);
        assert_eq!(Pattern::Melt.shader_index(), 27);
        assert_eq!(Pattern::Halftone.shader_index(), 40);
        assert_eq!(Pattern::TieDye.shader_index(), 41);
        assert_eq!(Pattern::Equalizer.shader_index(), 54);
    }

    #[test]
    fn blend_modes_cycle_through_every_mode() {
        let all: Vec<BlendMode> = BlendMode::all().collect();
        assert_eq!(all.len(), 24);
        for (i, m) in all.iter().enumerate() {
            assert!(!all[..i].contains(m), "{m:?} listed twice");
        }
        assert_eq!(BlendMode::Normal.cycle(1), BlendMode::Darken);
        assert_eq!(BlendMode::Normal.cycle(-1), BlendMode::Luminosity);
        assert_eq!(BlendMode::Luminosity.cycle(1), BlendMode::Normal);
    }

    #[test]
    fn palettes_have_stops() {
        for p in Palette::ALL {
            let stops = p.stops([0.0; 4], [1.0; 4]);
            assert!((2..=6).contains(&stops.len()), "{p:?}");
        }
    }
}
