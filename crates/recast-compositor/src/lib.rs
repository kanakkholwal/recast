#![forbid(unsafe_code)]

pub mod annotation;
pub mod caption;
pub mod camera;
pub mod eval;
pub mod geometry;
pub mod render;
pub mod session;
pub mod text;

pub use annotation::{
    annotation_alpha, annotation_params, sorted_visible, AnnotationParams, AnnotationShape,
};
pub use caption::{layout_caption, CaptionFrame, CaptionPill, VideoRect};
pub use camera::{bubble_params, bubble_shadow, BubbleParams};
pub use eval::{
    Affine2, BackgroundParams, CursorDraw, CursorSlot, Evaluator, FrameParams, HighlightDraw,
    LayerParams, SourceGeometry,
};
pub use geometry::{canvas_geometry, parse_aspect_ratio, CanvasGeometry, MAX_PADDING_PCT};
pub use recast_cursor::{CursorPlacement, CursorSettings, CursorTrack, Highlight};
pub use render::{BackgroundImage, Compositor, CursorSprite, FrameInputs, LayerInput, RenderStats};
pub use text::GlyphQuad;
pub use session::{screen_only, OutputSize, Session};
