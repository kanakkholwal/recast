//! The caption model and the placement maths, shared by the ASS burn-in and the
//! compositor so neither becomes a second authority on where a caption sits.
//!
//! Everything here is pure and renderer-agnostic: colours come back as hex, not
//! as ASS literals, and no function measures text for itself.

#![forbid(unsafe_code)]

pub mod highlight;
pub mod layout;
pub mod model;

pub use highlight::{karaoke_centiseconds, spoken_word_count, word_color, word_scaled};
pub use layout::{
    active_chunk_index, active_word_index, break_into_lines, caption_height_frac, caption_top_frac,
    chunk_words, pill_box, PillBox, MAX_CAP_FRAC,
};
pub use model::{CaptionAnimation, CaptionStyle, TranscriptWord};
