//! Turning a composition into a file. Here rather than in the desktop app so
//! the CLI and the agent surface drive the same renderer.

#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

pub mod ffmpeg;
pub mod frames;
pub mod nv12;
pub mod nv12_gpu;

#[cfg(windows)]
pub mod mp4;
#[cfg(windows)]
pub mod reader;
pub mod walk;

pub use ffmpeg::{FfmpegError, FfmpegPictures, FfmpegSink, SourceInfo};
pub use frames::{Extras, Frame, FrameLoop, NoPictures, PictureSource, PixelLayout, RenderError};
#[cfg(windows)]
pub use mp4::{Mp4Error, Mp4Sink};
pub use nv12::{rgba_to_nv12, Nv12Encoder, Nv12Error};
pub use nv12_gpu::GpuNv12;
#[cfg(windows)]
pub use reader::VideoPictures;
pub use walk::FrameWalk;
