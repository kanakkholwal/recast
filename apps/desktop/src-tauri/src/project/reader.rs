use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use zip::ZipArchive;

use crate::project::format;
use crate::project::ProjectMetadata;

/// Shared with `crate::cache`, which stores its own small artifacts here.
const CACHE_ROOT: &str = "recast-cache";
/// Marker file whose mtime records when a project was last opened.
const LAST_USED_MARKER: &str = ".lastused";
/// Age after which an untouched cache entry is dropped.
const CACHE_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// Ceiling on the whole cache. Beyond this the least-recently-used entries go,
/// so one week of large recordings can't fill the system drive.
const CACHE_MAX_BYTES: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ProjectOpenResult {
    pub metadata: ProjectMetadata,
    /// True for a v1 bundle — the editor migrates before loading it.
    pub needs_migration: bool,
    pub recording_path: PathBuf,
    pub cursor_path: PathBuf,
    pub edits_path: PathBuf,
    pub audio_path: Option<PathBuf>,
    pub microphone_path: Option<PathBuf>,
    pub camera_path: Option<PathBuf>,
}

/// Extract a `.recast` to the temp cache and report its layout. Both v1 and v2
/// extract to the same cache file names (`recording.mp4`, `cursor.json`,
/// `audio.wav`, `edits.json`) so the export/thumbnail pipeline is layout-blind;
/// v2 additionally fans its `edits/` sections back into one `edits.json`.
pub fn open_project(path: &Path) -> Result<ProjectOpenResult> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;

    let names: Vec<String> = archive.file_names().map(str::to_string).collect();
    let is_v2 = format::is_v2(&names);

    let metadata: ProjectMetadata = {
        let mut metadata_entry = archive.by_name(format::METADATA_NAME)?;
        let mut bytes = Vec::new();
        metadata_entry.read_to_end(&mut bytes)?;
        serde_json::from_slice(&bytes)?
    };

    let cache_dir = cache_dir_for(path)?;
    fs::create_dir_all(&cache_dir)?;
    touch_last_used(&cache_dir);

    if is_v2 {
        open_v2(&mut archive, metadata, &cache_dir)
    } else {
        open_v1(&mut archive, metadata, &cache_dir)
    }
}

fn open_v2(
    archive: &mut ZipArchive<File>,
    metadata: ProjectMetadata,
    cache_dir: &Path,
) -> Result<ProjectOpenResult> {
    let recording_path = extract_entry(
        archive,
        format::ASSET_VIDEO,
        &cache_dir.join("recording.mp4"),
    )?;
    let cursor_path = extract_entry(
        archive,
        format::ASSET_CURSOR_TRACK,
        &cache_dir.join("cursor.json"),
    )?;
    let audio_path = try_extract_entry(archive, format::ASSET_AUDIO, &cache_dir.join("audio.wav"));
    let microphone_path = try_extract_entry(
        archive,
        format::ASSET_MICROPHONE,
        &cache_dir.join("microphone.wav"),
    );
    let camera_path =
        try_extract_entry(archive, format::ASSET_CAMERA, &cache_dir.join("camera.mp4"));

    let edits_path = cache_dir.join("edits.json");
    let merged = merge_section_files(archive)?;
    fs::write(&edits_path, serde_json::to_string(&merged)?)
        .context("failed to write merged edits to cache")?;

    Ok(ProjectOpenResult {
        metadata,
        needs_migration: false,
        recording_path,
        cursor_path,
        edits_path,
        audio_path,
        microphone_path,
        camera_path,
    })
}

fn open_v1(
    archive: &mut ZipArchive<File>,
    metadata: ProjectMetadata,
    cache_dir: &Path,
) -> Result<ProjectOpenResult> {
    let recording_path = extract_entry(archive, "recording.mp4", &cache_dir.join("recording.mp4"))?;
    let cursor_path = extract_entry(archive, "cursor.json", &cache_dir.join("cursor.json"))?;
    let edits_path = extract_entry(archive, "edits.json", &cache_dir.join("edits.json"))?;
    let audio_path = try_extract_entry(archive, "audio.wav", &cache_dir.join("audio.wav"));
    let microphone_path =
        try_extract_entry(archive, "microphone.wav", &cache_dir.join("microphone.wav"));
    let camera_path = try_extract_entry(archive, "camera.mp4", &cache_dir.join("camera.mp4"));

    Ok(ProjectOpenResult {
        metadata,
        needs_migration: true,
        recording_path,
        cursor_path,
        edits_path,
        audio_path,
        microphone_path,
        camera_path,
    })
}

/// Read every present `edits/<section>.json` and fan them into one flat edits
/// object (inverse of the writer's split).
fn merge_section_files(archive: &mut ZipArchive<File>) -> Result<serde_json::Value> {
    let mut sections: Vec<(String, serde_json::Value)> = Vec::new();
    for section in format::SECTIONS {
        let entry_name = format::section_path(section);
        let mut entry = match archive.by_name(&entry_name) {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let mut raw = String::new();
        entry.read_to_string(&mut raw)?;
        let value: serde_json::Value = serde_json::from_str(&raw)
            .with_context(|| format!("section {entry_name} is not valid JSON"))?;
        sections.push((section.to_string(), value));
    }
    Ok(format::merge_sections(sections))
}

/// Stable extraction directory for a project.
///
/// Keyed on the project's PATH alone — never its length or mtime. Keying on
/// length meant every save minted a fresh directory and re-extracted the whole
/// recording on the next open: three revisions of one 700 MB project left 2.1 GB
/// of identical copies, and the re-extraction itself is a multi-hundred-MB
/// synchronous flush that stalls the machine mid-edit. Per-asset freshness is
/// `already_extracted`'s job, so a save that only rewrites `edits.json` reuses
/// the media untouched.
fn cache_dir_for(project_path: &Path) -> Result<PathBuf> {
    let stem = project_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("project");
    // Disambiguates same-named projects in different folders; a `DefaultHasher` bump costs one re-extraction.
    let mut hasher = DefaultHasher::new();
    project_path.to_string_lossy().hash(&mut hasher);
    Ok(env::temp_dir()
        .join(CACHE_ROOT)
        .join(format!("{stem}-{:016x}", hasher.finish())))
}

/// Record this project as recently used. Reuse skips extraction entirely, so
/// without the marker an actively-edited project looks stale to the sweeper and
/// gets evicted first. Best-effort: failing only costs eviction accuracy.
fn touch_last_used(cache_dir: &Path) {
    let _ = File::create(cache_dir.join(LAST_USED_MARKER));
}

/// When this entry was last opened: the marker's mtime, falling back to the
/// entry's own. `SystemTime::UNIX_EPOCH` for an unstattable entry makes it
/// maximally stale, so junk is evicted first rather than pinned forever.
fn last_used_at(path: &Path) -> SystemTime {
    let marker = fs::metadata(path.join(LAST_USED_MARKER)).and_then(|meta| meta.modified());
    marker
        .or_else(|_| fs::metadata(path).and_then(|meta| meta.modified()))
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

/// Recursive size of a cache entry. Unreadable subtrees count as 0 — the sweep
/// is advisory, and refusing to evict because one stat failed is worse.
fn entry_size(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if !meta.is_dir() {
        return meta.len();
    }
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| entry_size(&entry.path()))
        .sum()
}

fn remove_entry(path: &Path) {
    let removed = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    if let Err(err) = removed {
        log::debug!("cache sweep could not remove {}: {err}", path.display());
    }
}

/// Evict stale and excess extraction caches.
///
/// Call at STARTUP only. A time-based sweep during a session would delete assets
/// out from under an open editor; at startup nothing is open, so every entry is
/// safely evictable. Returns nothing because this is best-effort maintenance —
/// a failure means the cache stays larger than intended, never that opening
/// fails.
pub fn sweep_cache() {
    sweep_cache_in(
        &env::temp_dir().join(CACHE_ROOT),
        CACHE_TTL,
        CACHE_MAX_BYTES,
    );
}

/// `sweep_cache` with the root and limits injected, so the policy is testable
/// without touching the real temp dir or waiting out a 7-day TTL.
fn sweep_cache_in(root: &Path, ttl: Duration, max_bytes: u64) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    let now = SystemTime::now();
    let mut surviving: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let last_used = last_used_at(&path);
        let expired = now.duration_since(last_used).is_ok_and(|age| age > ttl);
        if expired {
            remove_entry(&path);
            continue;
        }
        surviving.push((last_used, entry_size(&path), path));
    }

    let mut total: u64 = surviving.iter().map(|(_, size, _)| size).sum();
    if total <= max_bytes {
        return;
    }
    // Least-recently-used first, so the projects still in rotation survive.
    surviving.sort_by_key(|(last_used, _, _)| *last_used);
    for (_, size, path) in surviving {
        if total <= max_bytes {
            break;
        }
        remove_entry(&path);
        total = total.saturating_sub(size);
    }
}

/// Temp name to write into before publishing, so a reader never observes a
/// half-written asset.
fn partial_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("asset");
    path.with_file_name(format!("{name}.partial"))
}

/// True when `path` already holds this entry in full.
fn already_extracted(path: &Path, expected_len: u64) -> bool {
    fs::metadata(path).is_ok_and(|meta| meta.len() == expected_len)
}

fn extract_entry(archive: &mut ZipArchive<File>, name: &str, path: &Path) -> Result<PathBuf> {
    let mut entry = archive
        .by_name(name)
        .with_context(|| format!("missing {name} in project"))?;
    // Size equality makes a save cheap, and re-extracting would TRUNCATE: `File::create` is instant while the rewrite takes seconds, so a reader mid-window sees a headerless file.
    if already_extracted(path, entry.size()) {
        return Ok(path.to_path_buf());
    }
    let partial = partial_path(path);
    {
        let mut output = File::create(&partial)?;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = entry.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            output.write_all(&buffer[..read])?;
        }
        output.sync_all()?;
    }
    // Rename is atomic within a directory: readers see the old file or the new one, never a partial.
    fs::rename(&partial, path)?;
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod backcompat_tests {
    use super::*;

    /// Load every real `.recast` in `$RECAST_BACKCOMPAT_DIR` through the current
    /// `open_project` and assert it parses. This is the concrete backward-
    /// compatibility check: pre-change recasts must still deserialize against
    /// the present `ProjectMetadata`/`RecordingStats` structs. Skips silently
    /// when the env var is unset so normal `cargo test` is unaffected.
    #[test]
    fn opens_existing_recasts_from_dir() {
        let Some(dir) = std::env::var_os("RECAST_BACKCOMPAT_DIR") else {
            eprintln!("RECAST_BACKCOMPAT_DIR unset — skipping backcompat check");
            return;
        };
        let dir = PathBuf::from(dir);
        let mut checked = 0usize;
        for entry in fs::read_dir(&dir).expect("read dir") {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("recast") {
                continue;
            }
            let result = open_project(&path)
                .unwrap_or_else(|e| panic!("FAILED to open {}: {e:#}", path.display()));
            // Sanity-check the fields the recording-fps changes touch.
            assert!(result.metadata.video.fps >= 1, "{}", path.display());
            assert!(result.metadata.stats.nominal_fps >= 1, "{}", path.display());
            eprintln!(
                "OK  {}  video.fps={} stats.nominalFps={} {}x{}",
                path.file_name().unwrap().to_string_lossy(),
                result.metadata.video.fps,
                result.metadata.stats.nominal_fps,
                result.metadata.video.width,
                result.metadata.video.height,
            );
            checked += 1;
        }
        assert!(checked > 0, "no .recast files found in {}", dir.display());
        eprintln!("Parsed {checked} existing recast(s) with the current schema.");
    }
}

/// Try to extract an optional entry from the archive. Returns None if the entry doesn't exist.
fn try_extract_entry(archive: &mut ZipArchive<File>, name: &str, path: &Path) -> Option<PathBuf> {
    let mut entry = archive.by_name(name).ok()?;
    // Same reuse and atomic-publish rules as `extract_entry`; audio.wav is large enough to hit the same window.
    if already_extracted(path, entry.size()) {
        return Some(path.to_path_buf());
    }
    let partial = partial_path(path);
    {
        let mut output = File::create(&partial).ok()?;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = entry.read(&mut buffer).ok()?;
            if read == 0 {
                break;
            }
            output.write_all(&buffer[..read]).ok()?;
        }
        output.sync_all().ok()?;
    }
    fs::rename(&partial, path).ok()?;
    Some(path.to_path_buf())
}

#[cfg(test)]
mod extract_tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    static N: AtomicU32 = AtomicU32::new(0);

    fn scratch() -> recast_testkit::Scratch {
        let n = N.fetch_add(1, Ordering::Relaxed);
        recast_testkit::Scratch::new(&format!("extract-{n}"))
    }

    fn archive_with(dir: &Path, body: &[u8]) -> ZipArchive<File> {
        let zip_path = dir.join("p.zip");
        let mut writer = ZipWriter::new(File::create(&zip_path).expect("create zip"));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        writer
            .start_file("assets/recording.mp4", options)
            .expect("start");
        writer.write_all(body).expect("write");
        writer.finish().expect("finish");
        ZipArchive::new(File::open(&zip_path).expect("open zip")).expect("read zip")
    }

    #[test]
    fn cache_dir_survives_a_save_that_changes_the_project_size() {
        let dir = scratch();
        let project = dir.join("Recast_2026-07-20.recast");
        fs::write(&project, b"original bundle").expect("write project");
        let before = cache_dir_for(&project).expect("cache dir");

        // A save grows `edits.json`, so keying on length minted a new directory and re-extracted the whole recording.
        fs::write(&project, b"bundle after a save, now longer").expect("rewrite project");
        let after = cache_dir_for(&project).expect("cache dir");

        assert_eq!(before, after);
    }

    #[test]
    fn cache_dir_differs_for_same_name_in_different_folders() {
        // Both guards held: a temporary would delete its directory before the write.
        let (dir_one, dir_two) = (scratch(), scratch());
        let one = dir_one.join("Recast.recast");
        let two = dir_two.join("Recast.recast");
        fs::write(&one, b"a").expect("write one");
        fs::write(&two, b"b").expect("write two");

        assert_ne!(
            cache_dir_for(&one).expect("one"),
            cache_dir_for(&two).expect("two")
        );
    }

    #[test]
    fn sweep_evicts_expired_entries_and_keeps_fresh_ones() {
        let root = scratch();
        let stale = root.join("stale");
        let fresh = root.join("fresh");
        fs::create_dir_all(&stale).expect("stale");
        fs::create_dir_all(&fresh).expect("fresh");
        touch_last_used(&fresh);

        // Zero TTL expires everything older than `now`; `fresh` was just touched, so only unused entries should go.
        sweep_cache_in(&root, Duration::from_secs(3600), u64::MAX);
        assert!(fresh.exists(), "recently used entry must survive");

        sweep_cache_in(&root, Duration::ZERO, u64::MAX);
        assert!(!stale.exists(), "expired entry must be removed");
    }

    #[test]
    fn sweep_evicts_least_recently_used_until_under_the_size_cap() {
        let root = scratch();
        let old = root.join("old");
        let recent = root.join("recent");
        fs::create_dir_all(&old).expect("old");
        fs::create_dir_all(&recent).expect("recent");
        fs::write(old.join("asset.bin"), vec![0u8; 4096]).expect("old asset");
        fs::write(recent.join("asset.bin"), vec![0u8; 4096]).expect("recent asset");
        touch_last_used(&old);
        // Ensure a strictly later marker; some filesystems have coarse mtimes.
        std::thread::sleep(std::time::Duration::from_millis(20));
        touch_last_used(&recent);

        // Cap fits one entry, so the older one is evicted and the newer kept.
        sweep_cache_in(&root, Duration::from_secs(3600), 6000);

        assert!(!old.exists(), "LRU entry must be evicted under the cap");
        assert!(recent.exists(), "most recent entry must survive");
    }

    #[test]
    fn extracts_then_reuses_without_rewriting() {
        let dir = scratch();
        let mut archive = archive_with(&dir, b"video-bytes");
        let target = dir.join("recording.mp4");

        extract_entry(&mut archive, "assets/recording.mp4", &target).expect("first extract");
        assert_eq!(fs::read(&target).expect("read"), b"video-bytes");

        // Same length means already extracted; the sentinel proves the second call didn't truncate and rewrite.
        fs::write(&target, b"SENTINEL-XX").expect("sentinel");
        extract_entry(&mut archive, "assets/recording.mp4", &target).expect("second extract");
        assert_eq!(fs::read(&target).expect("read"), b"SENTINEL-XX");
    }

    #[test]
    fn re_extracts_when_the_cached_file_is_the_wrong_size() {
        let dir = scratch();
        let mut archive = archive_with(&dir, b"video-bytes");
        let target = dir.join("recording.mp4");
        fs::write(&target, b"truncated").expect("short file");

        extract_entry(&mut archive, "assets/recording.mp4", &target).expect("extract");
        assert_eq!(fs::read(&target).expect("read"), b"video-bytes");
    }

    #[test]
    fn leaves_no_partial_file_behind() {
        let dir = scratch();
        let mut archive = archive_with(&dir, b"video-bytes");
        let target = dir.join("recording.mp4");

        extract_entry(&mut archive, "assets/recording.mp4", &target).expect("extract");
        assert!(
            !partial_path(&target).exists(),
            "partial file was not published"
        );
    }
}

#[cfg(test)]
mod roundtrip_tests {
    use super::*;
    use crate::project::writer::{self, ProjectWriteRequest};
    use crate::project::ProjectMetadata;
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicU32, Ordering};
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn workspace() -> recast_testkit::Scratch {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        recast_testkit::Scratch::new(&format!("fmt-{n}"))
    }

    fn fixture_metadata() -> ProjectMetadata {
        serde_json::from_value(json!({
            "schemaVersion": 1,
            "createdAtUnixMs": 1_700_000_000_000u64,
            "captureTarget": {
                "kind": "display",
                "id": 1,
                "label": "Display 1",
                "source": { "x": 0, "y": 0, "width": 1920, "height": 1080 },
                "crop": { "x": 0, "y": 0, "width": 1920, "height": 1080 },
                "displayId": 1,
                "scaleFactor": 1.0
            },
            "stats": {
                "capturedFrames": 600, "encodedFrames": 600, "droppedFrames": 0,
                "durationMs": 10000, "nominalFps": 60
            },
            "video": { "width": 1920, "height": 1080, "fps": 60, "durationMs": 10000 }
        }))
        .expect("fixture metadata")
    }

    #[test]
    fn write_v2_then_open_round_trips_edits() {
        let ws = workspace();
        let recording = ws.join("rec.mp4");
        let cursor = ws.join("cursor.json");
        let audio = ws.join("audio.wav");
        fs::write(&recording, b"video-bytes").unwrap();
        fs::write(&cursor, br#"{"samples":[]}"#).unwrap();
        fs::write(&audio, b"RIFFaudio").unwrap();

        let edits = r#"{"trimStart":0,"trimEnd":10,"padding":6,"cursorEnabled":true,"zoomRegions":[{"id":"z1","scale":2}],"annotations":[],"audioSettings":{"volume":1},"futureKey":42}"#;
        let out = ws.join("project.recast");
        writer::write_project(ProjectWriteRequest {
            output_path: out.clone(),
            metadata: fixture_metadata(),
            recording_path: recording,
            cursor_path: cursor,
            audio_path: Some(audio),
            microphone_path: None,
            camera_path: None,
            edits_json: edits.to_string(),
        })
        .expect("write v2");

        let opened = open_project(&out).expect("open v2");
        assert!(!opened.needs_migration);
        assert!(opened.audio_path.is_some());
        assert!(opened.microphone_path.is_none());
        assert_eq!(opened.metadata.video.width, 1920);

        let merged: Value =
            serde_json::from_str(&fs::read_to_string(&opened.edits_path).unwrap()).unwrap();
        assert_eq!(
            merged,
            serde_json::from_str::<Value>(edits).unwrap(),
            "split→merge is lossless, incl. the unmodelled futureKey"
        );
    }

    fn write_v1_archive(path: &Path, metadata: &[u8], edits: &[u8]) {
        let file = File::create(path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for (name, bytes) in [
            ("metadata.json", metadata),
            ("recording.mp4", b"video" as &[u8]),
            ("cursor.json", br#"{"samples":[]}"#),
            ("audio.wav", b"RIFF"),
            ("edits.json", edits),
        ] {
            zip.start_file(name, opts).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn migrate_v1_to_v2_backs_up_and_preserves_edits() {
        let ws = workspace();
        let proj = ws.join("legacy.recast");
        let edits = r#"{"trimStart":0,"trimEnd":10,"annotations":[{"id":"a1","kind":"rect"}],"legacyKey":true}"#;
        write_v1_archive(
            &proj,
            &serde_json::to_vec(&fixture_metadata()).unwrap(),
            edits.as_bytes(),
        );

        let before = open_project(&proj).expect("open v1");
        assert!(before.needs_migration);

        crate::project::migrate_project(&proj).expect("migrate");
        assert!(ws.join("legacy.recast.bak").exists(), "backup kept");

        let after = open_project(&proj).expect("open migrated");
        assert!(!after.needs_migration);
        let merged: Value =
            serde_json::from_str(&fs::read_to_string(&after.edits_path).unwrap()).unwrap();
        assert_eq!(merged, serde_json::from_str::<Value>(edits).unwrap());
    }

    #[test]
    fn migrate_is_noop_on_v2() {
        let ws = workspace();
        let recording = ws.join("rec.mp4");
        let cursor = ws.join("cursor.json");
        fs::write(&recording, b"v").unwrap();
        fs::write(&cursor, b"{}").unwrap();
        let out = ws.join("already.recast");
        writer::write_project(ProjectWriteRequest {
            output_path: out.clone(),
            metadata: fixture_metadata(),
            recording_path: recording,
            cursor_path: cursor,
            audio_path: None,
            microphone_path: None,
            camera_path: None,
            edits_json: "{}".to_string(),
        })
        .expect("write v2");

        crate::project::migrate_project(&out).expect("noop migrate");
        assert!(
            !out.with_extension("recast.bak").exists(),
            "no backup for v2"
        );
    }
}
