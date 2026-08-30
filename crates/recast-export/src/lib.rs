//! Turning a composition into a file. Here rather than in the desktop app so
//! the CLI and the agent surface drive the same renderer.

#![forbid(unsafe_code)]

pub mod frames;
pub mod nv12;

#[cfg(windows)]
pub mod mp4;
pub mod walk;

pub use frames::{FrameLoop, NoPictures, PictureSource, RenderError};
#[cfg(windows)]
pub use mp4::{Mp4Error, Mp4Sink};
pub use nv12::{rgba_to_nv12, Nv12Error};
pub use walk::FrameWalk;
