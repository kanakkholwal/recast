//! MP4 writing and reading, progressive by construction.
//! The writer buffers samples and sizes `moov` before `mdat`, so `+faststart` stops being a post-process to remember.

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
