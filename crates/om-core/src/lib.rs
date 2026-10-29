//! NawsMapper core: the document model and everything that can be computed
//! without a GPU or a window.
//!
//! Nothing in this crate depends on wgpu or egui, so it stays fast to compile
//! and fully unit-testable. The renderer (`om-render`) and the app (`om-app`)
//! both build on these types.

pub mod anim;
pub mod catalog;
pub mod diff;
pub mod geom;
pub mod history;
pub mod project;
pub mod shape;

pub use anim::{Animation, Lfo, SurfaceFrame, Wave};
pub use geom::Homography;
pub use history::History;
pub use project::*;
pub use shape::{Mask, PathPoint};

/// Straight (non-premultiplied) RGBA color in display (sRGB-encoded) space, 0..1.
pub type Color = [f32; 4];
