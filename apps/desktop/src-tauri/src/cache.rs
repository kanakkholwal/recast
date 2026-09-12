//! Best-effort disk cache for thumbnails, ffprobe metadata and waveforms, keyed on source path plus mtime and size.
//! Purely an optimization: any read or write failure falls back to recomputation, and it lives under the OS temp dir.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

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

/// Age after which an untouched entry is dropped.
const TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// Ceiling on the whole cache. These are thumbnails, waveforms and probe results,
/// not media, so the old 8 GB ceiling was sized for the extraction cache that is gone.
/// Age is time since WRITE, not since read: `get` never touches the file, so a hot
/// entry still ages out. Costs a recompute, which is what a cache miss costs anyway.
const MAX_BYTES: u64 = 1024 * 1024 * 1024;

/// Evicts stale and excess entries. STARTUP only: nothing is open yet, so no live
/// editor loses an artifact under it. Best-effort; a failure just leaves the cache large.
pub fn sweep() {
    sweep_in(&cache_dir(), TTL, MAX_BYTES);
}

/// `sweep` with the root and limits injected, so the policy is testable without
/// touching the real temp dir or waiting out a seven-day TTL.
fn sweep_in(root: &Path, ttl: Duration, max_bytes: u64) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let now = SystemTime::now();
    let mut surviving: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let (used, size) = match entry.metadata() {
            // Unstattable counts as maximally stale, so junk goes before anything real.
            Err(_) => (SystemTime::UNIX_EPOCH, 0),
            Ok(meta) => (
                meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                meta.len(),
            ),
        };
        if now.duration_since(used).is_ok_and(|age| age > ttl) {
            remove(&path);
            continue;
        }
        surviving.push((used, size, path));
    }

    let mut total: u64 = surviving.iter().map(|(_, size, _)| size).sum();
    if total <= max_bytes {
        return;
    }
    // Least recently used first, so what is still in rotation survives.
    surviving.sort_by_key(|(used, _, _)| *used);
    for (_, size, path) in surviving {
        if total <= max_bytes {
            break;
        }
        // Only count what actually went, or one undeletable entry ends the sweep early.
        if remove(&path) {
            total = total.saturating_sub(size);
        }
    }
}

fn remove(path: &Path) -> bool {
    let removed = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    if let Err(err) = &removed {
        log::debug!("cache sweep could not remove {}: {err}", path.display());
    }
    removed.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("recast-cache-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    #[test]
    fn an_untouched_entry_expires_and_a_recent_one_stays() {
        let root = scratch("ttl");
        let entry = root.join("thumb.json");
        fs::write(&entry, b"{}").expect("write");

        sweep_in(&root, Duration::from_secs(3600), u64::MAX);
        assert!(entry.exists(), "a fresh entry survives");

        sweep_in(&root, Duration::ZERO, u64::MAX);
        assert!(!entry.exists(), "an expired entry goes");
        let _ = fs::remove_dir_all(&root);
    }

    /// Without this the cache grows without bound, which is what the extraction
    /// cache did before it was swept.
    #[test]
    fn the_least_recently_used_entries_go_until_the_cache_fits() {
        let root = scratch("lru");
        let old = root.join("old.bin");
        let recent = root.join("recent.bin");
        fs::write(&old, vec![0u8; 4096]).expect("old");
        std::thread::sleep(Duration::from_millis(20));
        fs::write(&recent, vec![0u8; 4096]).expect("recent");

        sweep_in(&root, Duration::from_secs(3600), 6000);

        assert!(!old.exists(), "the older entry is evicted under the cap");
        assert!(recent.exists(), "the newer one survives");
        let _ = fs::remove_dir_all(&root);
    }
}
