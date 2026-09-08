//! The v3 directory behind the doors the zip uses: `open_project` and `update_project_edits` route here for a directory.
//! The document is the truth; `.cache/edits.json` is a derived `RenderState` regenerated on open so every consumer of `edits_path` stays untouched.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use recast_project::layout::{self, locate, Layout, Located};
use recast_project::scene::{self, MediaFile, MediaRefs, TrackFile};
use recast_project::{parse, serialize, IdGen};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::reader::ProjectOpenResult;
use super::writer::ProjectWriteRequest;
use super::ProjectMetadata;
use crate::render::graph::RenderState;

/// Whether `path` is a v3 project directory. A bundle, a plain video, or a missing path is not.
pub fn is_project_dir(path: &Path) -> bool {
    matches!(locate(path), Ok(Located::V3Dir(_)))
}

/// Opens a v3 directory as the same `ProjectOpenResult` a bundle produces, writing the derived state under `.cache/`.
/// Upgrades the bundle at `path` into a v3 directory at the same path, keeping the bundle as `Name.recast.bak`.
pub fn migrate(path: &Path) -> Result<recast_project::migrate::Report> {
    recast_project::migrate::migrate(path, path, recast_project::migrate::Options::default())
        .with_context(|| format!("failed to migrate {} to v3", path.display()))
}

/// Bytes a project directory occupies on disk, minus `.cache` (rebuildable, and the library card should say what a copy costs).
pub fn size_bytes(root: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|e| e.file_name() != layout::CACHE_DIR)
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => size_bytes(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

pub fn open(path: &Path) -> Result<ProjectOpenResult> {
    let layout = Layout::new(path);
    let doc = read_document(&layout)?;
    let mut state =
        scene::to_render_state(&doc).context("mapping the document to a render state")?;
    if let Some(transcript) = read_words(&layout) {
        state.passthrough.insert("transcript".into(), transcript);
    }
    let metadata = read_capture(&layout)?;
    let edits_path = write_derived_state(&layout, &state)?;
    let present = |relative: &str| -> Option<PathBuf> {
        let p = layout.resolve(relative);
        p.is_file().then_some(p)
    };
    Ok(ProjectOpenResult {
        metadata,
        format: super::Format::V3,
        needs_migration: false,
        recording_path: layout.resolve(layout::RECORDING),
        cursor_path: layout.resolve(layout::CURSOR_TRACK),
        edits_path,
        audio_path: present(layout::SYSTEM),
        microphone_path: present(layout::MIC),
        camera_path: present(layout::CAMERA),
    })
}

/// Saves the editor's whole state into the document, lifting the transcript into `tracks/`. Ids are recycled from the current document.
pub fn save_edits(path: &Path, edits_json: &str) -> Result<()> {
    let layout = Layout::new(path);
    let current = read_document(&layout)?;
    let mut state: RenderState = serde_json::from_str(edits_json).context("parsing the edits")?;
    let words = state
        .passthrough
        .remove("transcript")
        .filter(|t| !t.is_null());
    if let Some(words) = &words {
        write_words(&layout, words)?;
    }
    let metadata = read_capture(&layout)?;
    let refs = media_refs(&layout, &metadata, &current, words.as_ref());
    let mut ids = IdGen::seeded(seed_of(edits_json));
    let doc = scene::from_render_state_public(&state, &refs, Some(&current), &mut ids);
    atomic(&layout.document(), serialize(&doc).as_bytes())?;
    write_derived_state(&layout, &state)?;
    Ok(())
}

/// Writes a fresh recording as a directory: media is MOVED into place, so a stop costs a rename per file instead of a copy into a zip.
pub fn write_project(request: ProjectWriteRequest) -> Result<PathBuf> {
    let root = &request.output_path;
    let layout = Layout::new(root);
    for dir in [
        layout::MEDIA_DIR,
        layout::TRACKS_DIR,
        layout::BRANCHES_DIR,
        layout::CACHE_DIR,
    ] {
        fs::create_dir_all(root.join(dir)).context("creating the project directories")?;
    }
    move_into(&request.recording_path, &layout.resolve(layout::RECORDING))?;
    move_into(&request.cursor_path, &layout.resolve(layout::CURSOR_TRACK))?;
    for (source, target) in [
        (&request.audio_path, layout::SYSTEM),
        (&request.microphone_path, layout::MIC),
        (&request.camera_path, layout::CAMERA),
    ] {
        if let Some(source) = source {
            move_into(source, &layout.resolve(target))?;
        }
    }
    let capture = serde_json::to_string_pretty(&request.metadata)
        .context("serialising the capture record")?;
    atomic(&layout.capture(), capture.as_bytes())?;
    let state: RenderState =
        serde_json::from_str(&request.edits_json).context("parsing the initial edits")?;
    let refs = media_refs(
        &layout,
        &request.metadata,
        &recast_project::Document::empty(),
        None,
    );
    let mut ids = IdGen::seeded(seed_of(&request.edits_json));
    let doc = scene::from_render_state_public(&state, &refs, None, &mut ids);
    atomic(&layout.document(), serialize(&doc).as_bytes())?;
    write_derived_state(&layout, &state)?;
    Ok(root.clone())
}

fn read_document(layout: &Layout) -> Result<recast_project::Document> {
    let text = fs::read_to_string(layout.document())
        .with_context(|| format!("reading {}", layout.document().display()))?;
    parse(&text).context("parsing project.rcx")
}

fn read_capture(layout: &Layout) -> Result<ProjectMetadata> {
    let text = fs::read_to_string(layout.capture())
        .with_context(|| format!("reading {}", layout.capture().display()))?;
    serde_json::from_str(&text).context("parsing capture.json")
}

fn read_words(layout: &Layout) -> Option<Value> {
    let text = fs::read_to_string(layout.resolve(layout::WORDS_TRACK)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Written only when the content moved, so a save that did not retranscribe leaves the track's mtime alone.
fn write_words(layout: &Layout, words: &Value) -> Result<()> {
    let path = layout.resolve(layout::WORDS_TRACK);
    let text = words.to_string();
    if fs::read_to_string(&path).ok().as_deref() == Some(text.as_str()) {
        return Ok(());
    }
    atomic(&path, text.as_bytes())
}

fn write_derived_state(layout: &Layout, state: &RenderState) -> Result<PathBuf> {
    fs::create_dir_all(layout.cache()).context("creating .cache")?;
    let path = layout.cache().join("edits.json");
    let json = serde_json::to_string_pretty(state).context("serialising the render state")?;
    atomic(&path, json.as_bytes())?;
    Ok(path)
}

fn media_refs(
    layout: &Layout,
    meta: &ProjectMetadata,
    current: &recast_project::Document,
    words: Option<&Value>,
) -> MediaRefs {
    let exists = |relative: &str| layout.resolve(relative).is_file();
    let offsets = meta
        .media
        .as_ref()
        .map(|m| m.track_offsets)
        .unwrap_or_default();
    MediaRefs {
        recording: exists(layout::RECORDING).then(|| MediaFile {
            src: layout::RECORDING.into(),
            width: meta.video.width,
            height: meta.video.height,
            fps: f64::from(meta.video.fps),
            duration: meta.media_duration_secs(),
            offset: 0.0,
        }),
        camera: exists(layout::CAMERA).then(|| MediaFile {
            src: layout::CAMERA.into(),
            offset: offsets.camera_ms.unwrap_or(0) as f64 / 1000.0,
            ..Default::default()
        }),
        mic: exists(layout::MIC).then(|| layout::MIC.into()),
        system: exists(layout::SYSTEM).then(|| layout::SYSTEM.into()),
        cursor: exists(layout::CURSOR_TRACK)
            .then(|| track_from_current(current, scene::ids::CURSOR_TRACK, layout::CURSOR_TRACK)),
        words: words.map(words_track),
    }
}

/// The counts a track header carries come from the current document, so a save never re-reads a 500 KB cursor file to learn its length.
fn track_from_current(current: &recast_project::Document, id: &str, src: &str) -> TrackFile {
    let node = current.find(id);
    let num = |name: &str| {
        node.and_then(|n| n.attr(name))
            .and_then(|v| v.parse::<f64>().ok())
    };
    let span = node.and_then(|n| n.attr("span")).and_then(|s| {
        let mut parts = s.split_whitespace().map(|p| p.parse::<f64>().ok());
        Some((parts.next()??, parts.next()??))
    });
    TrackFile {
        src: src.into(),
        rows: num("n").unwrap_or(0.0) as usize,
        span,
        engine: None,
        model: None,
        lang: None,
    }
}

fn words_track(transcript: &Value) -> TrackFile {
    let words: Vec<(f64, f64)> = transcript
        .get("segments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|s| {
            s.get("words")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(|w| Some((w.get("start")?.as_f64()?, w.get("end")?.as_f64()?)))
        .collect();
    let text = |key: &str| {
        transcript
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    TrackFile {
        src: layout::WORDS_TRACK.into(),
        rows: words.len(),
        span: words.first().zip(words.last()).map(|(f, l)| (f.0, l.1)),
        engine: text("engine"),
        model: text("modelId"),
        lang: text("language"),
    }
}

/// Ids for rows the state carries none for come from the state's own bytes, so saving the same state twice mints the same ids.
fn seed_of(edits_json: &str) -> u64 {
    let digest = Sha256::digest(edits_json.as_bytes());
    u64::from_le_bytes(digest[..8].try_into().unwrap_or([0; 8]))
}

/// Rename where the volume allows, copy and delete otherwise.
fn move_into(source: &Path, target: &Path) -> Result<()> {
    if fs::rename(source, target).is_ok() {
        return Ok(());
    }
    fs::copy(source, target)
        .with_context(|| format!("copying {} into the project", source.display()))?;
    let _ = fs::remove_file(source);
    Ok(())
}

fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    crate::commands::system::write_atomic(&tmp, path, bytes)
        .with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn migrated_dir() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("Take.recast");
        std::fs::copy(
            "C:/Users/kanak/Videos/Recast/recasts/Recast_2026-08-29_20-30-54.recast",
            &src,
        )
        .unwrap();
        recast_project::migrate::migrate(
            &src,
            &src,
            recast_project::migrate::Options { keep_backup: false },
        )
        .unwrap();
        (tmp, src)
    }

    /// Needs the owner's real bundle; the shape of a fresh recording is covered by `write_project` below.
    #[test]
    #[ignore = "needs the owner's real recordings"]
    fn a_migrated_directory_opens_like_a_bundle_and_saves_back_into_the_document() {
        let (_tmp, dir) = migrated_dir();
        let opened = open(&dir).unwrap();
        assert!(!opened.needs_migration);
        assert!(opened.recording_path.is_file());
        let edits = fs::read_to_string(&opened.edits_path).unwrap();
        let mut state: RenderState = serde_json::from_str(&edits).unwrap();
        state.padding = 12.0;
        save_edits(&dir, &serde_json::to_string(&state).unwrap()).unwrap();
        let text = fs::read_to_string(dir.join(layout::DOCUMENT)).unwrap();
        assert!(text.contains("pad=\"12\""), "{text}");
        let again: RenderState =
            serde_json::from_str(&fs::read_to_string(open(&dir).unwrap().edits_path).unwrap())
                .unwrap();
        assert_eq!(again.padding, 12.0);
    }

    #[test]
    fn a_fresh_recording_lands_as_a_directory_with_its_media_moved_in() {
        let tmp = tempfile::tempdir().unwrap();
        let staged = tmp.path().join("staged");
        fs::create_dir_all(&staged).unwrap();
        let rec = staged.join("x.recording.mp4");
        let cur = staged.join("x.cursor.json");
        fs::write(&rec, b"MP4").unwrap();
        fs::write(&cur, r#"{"samples":[],"clicks":[]}"#).unwrap();
        let metadata: ProjectMetadata = serde_json::from_str(
            r#"{"schemaVersion":1,"createdAtUnixMs":1,"captureTarget":{"kind":"display","id":1,"label":"x","source":{"x":0,"y":0,"width":1920,"height":1080},"crop":{"x":0,"y":0,"width":1920,"height":1080},"displayId":1,"scaleFactor":1},"stats":{"capturedFrames":60,"encodedFrames":60,"droppedFrames":0,"durationMs":1000,"nominalFps":60},"video":{"width":1920,"height":1080,"fps":60,"durationMs":1000}}"#,
        )
        .unwrap();
        let out = tmp.path().join("Fresh.recast");
        let state = RenderState {
            trim_end: 1.0,
            ..RenderState::default()
        };
        let path = write_project(ProjectWriteRequest {
            output_path: out.clone(),
            metadata,
            recording_path: rec.clone(),
            cursor_path: cur.clone(),
            audio_path: None,
            microphone_path: None,
            camera_path: None,
            edits_json: serde_json::to_string(&state).unwrap(),
        })
        .unwrap();
        assert_eq!(path, out);
        assert!(!rec.exists(), "the recording was moved, not copied");
        assert!(out.join(layout::RECORDING).is_file());
        assert!(is_project_dir(&out));
        let opened = open(&out).unwrap();
        assert_eq!(opened.metadata.video.width, 1920);
        let text = fs::read_to_string(out.join(layout::DOCUMENT)).unwrap();
        assert!(text.contains("<media id=\"rec\""), "{text}");
        assert!(text.contains("out=\"1.000\""), "{text}");
    }
}
