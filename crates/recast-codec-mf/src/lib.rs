//! What Media Foundation can encode on this machine, asked directly via `MFTEnumEx`.
//! Replaces the one-frame FFmpeg probe, which cost a process per candidate and reported "software" whenever a hardware encoder was busy.

#![cfg_attr(not(windows), allow(unused_imports))]
// Documented 1:1 today and must stay so: this is the whole encode path.
#![deny(clippy::undocumented_unsafe_blocks, unsafe_op_in_unsafe_fn)]

pub use recast_codec::{EncoderDescriptor, Vendor, VideoCodec};

#[cfg(windows)]
mod aac;
#[cfg(windows)]
mod audio;
#[cfg(windows)]
mod d3d;
#[cfg(windows)]
mod decoder;
#[cfg(windows)]
mod encoder;
#[cfg(windows)]
mod windows_mf;

#[cfg(windows)]
pub use aac::AacEncoder;
#[cfg(windows)]
pub use audio::{AudioFormat, AudioReader};
#[cfg(windows)]
pub use d3d::{D3dContext, Nv12Converter, Nv12Frame, SharedSurface, SyncFence};
#[cfg(windows)]
pub use decoder::{DecodeError, DecodedFrame, VideoInfo, VideoReader};
#[cfg(windows)]
pub use encoder::{EncodeConfig, EncodeError, EncodedSample, H264Encoder};

/// Every hardware and software video encoder Media Foundation exposes, in the order the system ranks them.
/// Empty off Windows, and empty rather than failing when Media Foundation cannot start: the caller falls back to its other backend.
#[cfg(windows)]
pub fn enumerate_encoders() -> Vec<EncoderDescriptor> {
    windows_mf::enumerate_encoders()
}

#[cfg(not(windows))]
pub fn enumerate_encoders() -> Vec<EncoderDescriptor> {
    Vec::new()
}
