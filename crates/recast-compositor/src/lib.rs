#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

pub mod annotation;
pub mod camera;
pub mod caption;
pub mod eval;
pub mod geometry;
pub mod layout;
pub mod render;
pub mod session;
pub mod source;
pub mod text;
pub mod yuv;

pub use annotation::{
    annotation_alpha, annotation_params, sorted_visible, AnnotationParams, AnnotationShape,
};
pub use camera::{bubble_params, bubble_shadow, BubbleParams};
pub use caption::{layout_caption, CaptionClock, CaptionFrame, CaptionPill, VideoRect};
pub use eval::{
    Affine2, BackgroundParams, CursorDraw, CursorSlot, Evaluator, FrameParams, HighlightDraw,
    LayerParams, SourceGeometry,
};
pub use geometry::{canvas_geometry, parse_aspect_ratio, CanvasGeometry, MAX_PADDING_PCT};
pub use layout::{layout_at, resolve as resolve_layout, LayoutRects};
pub use recast_cursor::{CursorPlacement, CursorSettings, CursorTrack, Highlight};
pub use render::{
    BackgroundImage, Compositor, CursorSprite, FrameInputs, LayerInput, MissingInput, RenderStats,
};
pub use session::{caption_face_available, screen_only, OutputSize, Session};
pub use source::{RenderSource, Renderable};
pub use text::GlyphQuad;
pub use yuv::{
    chroma_offset, decode_matrix, encode_matrix, gamut_matrix, ChromaSiting, Plane, PlaneData,
    PlaneLayout, SourceColor, SourcePlanes, YuvError,
};
