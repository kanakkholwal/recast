//! MP4 writing and reading. Writing is progressive by construction.
//!
//! `moov` goes before `mdat` because the writer buffers samples and sizes the
//! header first, so `+faststart` stops being a post-process the export has to
//! remember to ask for.

#![forbid(unsafe_code)]

pub mod avc;
pub mod boxes;
pub mod fragment;
pub mod reader;
pub mod track;
pub mod writer;

pub use avc::{annex_b_to_avcc, split_access_units, split_annex_b, AvcConfig, Converted};
pub use boxes::top_level_boxes;
pub use fragment::{FragmentError, FragmentedWriter};
pub use reader::{Mp4Reader, ReadError, SampleRef, Track, TrackKind};
pub use track::{Sample, SampleTable};
pub use writer::{AudioFormat, Mp4Writer, VideoFormat};
