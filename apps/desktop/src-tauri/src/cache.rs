//! Best-effort disk cache for thumbnails, ffprobe metadata and waveforms, keyed on source path plus mtime and size.
//! Purely an optimization: any read or write failure falls back to recomputation, and it lives under the OS temp dir.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{de::DeserializeOwned, Serialize};

/// Stable cache directory. Best-effort: if the OS clears temp, entries rebuild
/// on next use.
fn cache_dir() -> PathBuf {
    std::env::temp_dir().join("recast-cache")
}

/// Fold one source file's identity (path + mtime + size) into `hasher`.
/// Returns `false` if the file can't be stat'd — the caller then skips the
/// cache entirely rather than risk a stale hit.
fn hash_source(hasher: &mut DefaultHasher, source: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(source) else {
        return false;
    };
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    source.to_string_lossy().hash(hasher);
    mtime.hash(hasher);
    meta.len().hash(hasher);
    true
}

/// Resolve the on-disk path of the cache entry for the given sources +
/// discriminator under namespace `kind`. `None` when any source is unstattable.
fn entry_path(kind: &str, sources: &[&Path], extra: u64) -> Option<PathBuf> {
    let mut hasher = DefaultHasher::new();
    for source in sources {
        if !hash_source(&mut hasher, source) {
            return None;
        }
    }
    extra.hash(&mut hasher);
    let h = hasher.finish();
    Some(cache_dir().join(format!("{kind}-{h:016x}.json")))
}

/// Look up a cached value derived from `sources`. Returns `None` on any miss,
/// stale source, or decode error.
pub fn get<T: DeserializeOwned>(kind: &str, sources: &[&Path], extra: u64) -> Option<T> {
    let path = entry_path(kind, sources, extra)?;
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Store `value` keyed to `sources`. Best-effort — failures are swallowed.
pub fn put<T: Serialize>(kind: &str, sources: &[&Path], extra: u64, value: &T) {
    let Some(path) = entry_path(kind, sources, extra) else {
        return;
    };
    let _ = std::fs::create_dir_all(cache_dir());
    if let Ok(bytes) = serde_json::to_vec(value) {
        let _ = std::fs::write(&path, bytes);
    }
}
