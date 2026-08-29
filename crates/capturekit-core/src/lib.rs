//! Vocabulary types shared by every capturekit backend.
//!
//! Pure: no OS calls, no `unsafe`, no allocation beyond the enumeration results.
//! Everything a backend would otherwise reimplement per platform (stride and
//! buffer arithmetic, crop fitting, timestamp monotonicity) lives here once and
//! is tested on any host, with or without a display.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

mod audio;
mod capabilities;
mod color;
mod cursor;
mod error;
mod format;
mod geom;
mod gpu;
mod pacing;
mod permission;
mod target;
mod time;

pub use audio::{
    interleave, AudioDesc, AudioDevice, AudioDeviceId, AudioDirection, AudioFormat, AudioTimeline,
    SampleFormat,
};
pub use capabilities::{Capabilities, ExclusionSupport, RegionCrop};
pub use color::{
    ChromaSiting, ColorRange, ColorSpace, ColorSpaceRequest, MatrixCoefficients, Primaries,
    TransferFunction,
};
pub use cursor::{
    point_in_surface, point_offset_in_surface, CursorButtons, CursorSample, CursorShape,
    CursorShapeKind,
};
pub use error::{CaptureError, LostReason, Result};
pub use format::{PixelFormat, PlaneFormat};
pub use geom::{DirtyRects, Rect, Rotation};
pub use gpu::GpuHandle;
pub use pacing::{Pacer, Pacing};
pub use permission::{Permission, PermissionKind};
pub use target::{
    Camera, CameraFormat, CameraId, Display, DisplayId, SourceDesc, Target, Window, WindowId,
};
pub use time::{MonotonicClock, Timestamp};
