use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use zip::ZipArchive;

use crate::project::format;
use crate::project::ProjectMetadata;

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

fn cache_dir_for(project_path: &Path) -> Result<PathBuf> {
    let metadata = fs::metadata(project_path)?;
    let stem = project_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("project");
    Ok(env::temp_dir()
        .join("recast-cache")
        .join(format!("{stem}-{}", metadata.len())))
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
    // The cache dir is keyed by project size, so a full-size file here is this
    // project's asset. Re-extracting would TRUNCATE it — and `File::create`
    // does that instantly while the rewrite takes seconds on a large recording,
    // so any reader mid-window sees a headerless file ("no video track").
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
    // Rename is atomic within a directory: readers see the old file or the new
    // one, never a partial.
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
    // Same reuse + atomic-publish rules as `extract_entry`; audio.wav is large
    // enough to hit the same truncation window.
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

    fn scratch() -> PathBuf {
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = env::temp_dir().join(format!("recast-extract-{}-{}", std::process::id(), n));
        fs::create_dir_all(&dir).expect("create scratch");
        dir
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
    fn extracts_then_reuses_without_rewriting() {
        let dir = scratch();
        let mut archive = archive_with(&dir, b"video-bytes");
        let target = dir.join("recording.mp4");

        extract_entry(&mut archive, "assets/recording.mp4", &target).expect("first extract");
        assert_eq!(fs::read(&target).expect("read"), b"video-bytes");

        // Same length => treated as already extracted. Sentinel content proves
        // the second call didn't truncate and rewrite: re-extracting a 637MB
        // recording blanks it for seconds, and readers see a headerless file.
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

    fn workspace() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("recast-fmt-{}-{}", std::process::id(), n));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
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
