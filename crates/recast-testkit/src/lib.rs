#![forbid(unsafe_code)]

pub mod audio;
pub mod compare;
pub mod media;
pub mod timecode;

pub use compare::{digest_hex, frame_delta, FrameDelta};
pub use media::{ffmpeg_path, SourceSpec};
