#![forbid(unsafe_code)]

pub mod eval;
pub mod geometry;
pub mod render;

pub use eval::{Affine2, BackgroundParams, Evaluator, FrameParams, LayerParams, SourceGeometry};
pub use geometry::{canvas_geometry, parse_aspect_ratio, CanvasGeometry, MAX_PADDING_PCT};
pub use render::{Compositor, FrameInputs, LayerInput, RenderStats};
