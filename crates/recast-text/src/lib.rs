//! Font resolution, shaping and glyph rasterisation for the caption pass.
//!
//! Split from the export's `text_measure`, which shapes a line to size the ASS
//! pill but never rasterises: the compositor has to draw the glyphs itself, and
//! sharing one shaper is what keeps the burn-in and the preview on the same
//! metrics.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

mod atlas;
mod face;
mod raster;
mod shape;

pub use atlas::{AtlasGlyph, GlyphAtlas};
pub use face::{FontFace, Metrics};
pub use raster::{rasterize, GlyphMask};
pub use shape::{shape_line, PositionedGlyph, ShapedLine};

#[cfg(not(target_arch = "wasm32"))]
mod resolve;
#[cfg(not(target_arch = "wasm32"))]
pub use resolve::{resolve_face, ResolvedFace};
