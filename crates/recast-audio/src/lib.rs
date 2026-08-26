//! Audio for the export graph: the WSOLA time stretch shared with the preview,
//! plus the resampling, mixing and loudness around it.

#![forbid(unsafe_code)]

pub mod loudness;
pub mod mix;
pub mod resample;
#[cfg(feature = "scene")]
pub mod scene;
pub mod source;
pub mod stretch;

pub use loudness::{integrated_lufs, normalizing_gain, DEFAULT_CEILING, DEFAULT_TARGET_LUFS};
pub use mix::{Master, Mixer, Placement, Track, MASTER_CHANNELS, MASTER_RATE};
pub use resample::Kernel;
#[cfg(feature = "scene")]
pub use scene::{mixer_for, RecordingKind, SceneSources};
pub use source::{to_stereo, SampleSource, Samples};
pub use stretch::{resample_linear, time_stretch, DEFAULT_SAMPLE_RATE};
