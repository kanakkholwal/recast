//! MP4 writing, progressive by construction.
//!
//! `moov` goes before `mdat` because the writer buffers samples and sizes the
//! header first, so `+faststart` stops being a post-process the export has to
//! remember to ask for.

#![forbid(unsafe_code)]

pub mod avc;
pub mod boxes;
pub mod track;
pub mod writer;

pub use avc::{annex_b_to_avcc, split_access_units, split_annex_b, AvcConfig, Converted};
pub use boxes::top_level_boxes;
pub use track::{Sample, SampleTable};
pub use writer::{Mp4Writer, VideoFormat};
