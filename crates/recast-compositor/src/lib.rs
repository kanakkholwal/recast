#![forbid(unsafe_code)]

pub mod annotation;
pub mod camera;
pub mod eval;
pub mod geometry;
pub mod render;
pub mod session;

pub use annotation::{
    annotation_alpha, annotation_params, sorted_visible, AnnotationParams, AnnotationShape,
};
pub use camera::{bubble_params, bubble_shadow, BubbleParams};
pub use eval::{Affine2, BackgroundParams, Evaluator, FrameParams, LayerParams, SourceGeometry};
pub use geometry::{canvas_geometry, parse_aspect_ratio, CanvasGeometry, MAX_PADDING_PCT};
pub use render::{BackgroundImage, Compositor, FrameInputs, LayerInput, RenderStats};
pub use session::{screen_only, OutputSize, Session};
