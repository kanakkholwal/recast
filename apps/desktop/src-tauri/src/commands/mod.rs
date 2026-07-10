pub(crate) mod assets;
pub(crate) mod auth;
pub(crate) mod cloud;
mod editor;
pub(crate) mod error;
pub(crate) mod export;
pub(crate) mod extensions;
pub(crate) mod ffmpeg;
pub(crate) mod files;
pub(crate) mod gdrive;
mod recording;
pub(crate) mod system;
pub(crate) mod types;

pub use assets::*;
pub use auth::*;
pub use cloud::*;
pub use editor::*;
pub use extensions::*;
pub use files::*;
pub use gdrive::*;
pub use recording::*;
pub use system::*;

use std::path::{Path, PathBuf};

/// Build a collision-free path inside `dir` for `<stem>.<ext>`.
///
/// If the plain name is free it's used as-is; otherwise a counter is appended
/// the way Explorer/Finder disambiguate duplicates — `<stem> (1).<ext>`,
/// `<stem> (2).<ext>`, and so on.
pub(crate) fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let plain = dir.join(format!("{stem}.{ext}"));
    if !plain.exists() {
        return plain;
    }
    let mut counter = 1u32;
    loop {
        let candidate = dir.join(format!("{stem} ({counter}).{ext}"));
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}
