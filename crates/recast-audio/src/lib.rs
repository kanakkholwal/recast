//! Audio for the export graph: the WSOLA time stretch shared with the preview,
//! plus the resampling and mixing around it.

#![forbid(unsafe_code)]

pub mod stretch;

pub use stretch::{resample_linear, time_stretch, DEFAULT_SAMPLE_RATE};
