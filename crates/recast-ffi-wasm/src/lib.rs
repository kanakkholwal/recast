#![forbid(unsafe_code)]

mod scene_io;

pub use scene_io::{parse_scene, SceneParseError};

#[cfg(target_arch = "wasm32")]
mod preview;

#[cfg(target_arch = "wasm32")]
pub use preview::PreviewEngine;
