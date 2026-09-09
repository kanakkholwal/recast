#![forbid(unsafe_code)]

mod backend;
mod bench_io;
mod cursor_io;
mod project_io;
mod ring;
mod scene_io;
mod slot;

pub use backend::{backend_name, backends_for};
pub use cursor_io::parse_track;
pub use project_io::Replica;
pub use ring::pick_slot;
pub use scene_io::{parse_scene, SceneParseError};
pub use slot::{parse_slot, slot_at};

#[cfg(target_arch = "wasm32")]
mod document;
#[cfg(target_arch = "wasm32")]
mod preview;

#[cfg(target_arch = "wasm32")]
pub use document::ProjectDocument;
#[cfg(target_arch = "wasm32")]
pub use preview::PreviewEngine;
