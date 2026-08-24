use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::recording::{CaptureTarget, RecordingStats};

pub mod autosave;
pub mod format;
pub mod journal;
pub mod reader;
pub mod writer;

/// Cheap format probe: reads only the ZIP central directory (entry names) — no
/// extraction or media decompression — to decide whether `path` is a legacy v1
/// bundle. Returns false for unreadable or non-archive files.
pub fn is_legacy_project(path: &Path) -> bool {
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(archive) = zip::ZipArchive::new(file) else {
        return false;
    };
    let names: Vec<String> = archive.file_names().map(str::to_string).collect();
    !format::is_v2(&names)
}

/// Re-pack a legacy v1 `.recast` as v2 in place, keeping a one-time
/// `*.recast.bak` of the original first (recordings can be irreplaceable).
/// No-op if the project is already v2. The atomic rename inside `write_project`
/// means a crash mid-migration leaves the backup and the untouched original.
pub fn migrate_project(path: &Path) -> Result<()> {
    let opened = reader::open_project(path).context("failed to open project for migration")?;
    if !opened.needs_migration {
        return Ok(());
    }

    let edits_json =
        std::fs::read_to_string(&opened.edits_path).context("failed to read edits to migrate")?;

    let backup = path.with_extension("recast.bak");
    std::fs::copy(path, &backup).context("failed to write migration backup")?;

    writer::write_project(writer::ProjectWriteRequest {
        output_path: path.to_path_buf(),
        metadata: opened.metadata,
        recording_path: opened.recording_path,
        cursor_path: opened.cursor_path,
        audio_path: opened.audio_path,
        microphone_path: opened.microphone_path,
        camera_path: opened.camera_path,
        edits_json,
    })
    .context("failed to write migrated project")?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMetadata {
    pub schema_version: u32,
    pub created_at_unix_ms: u64,
    pub capture_target: CaptureTarget,
    pub stats: RecordingStats,
    pub video: ProjectVideoMetadata,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<ProjectMediaMetadata>,
}

impl ProjectMetadata {
    /// Duration of the recorded *media*, in seconds — not of the capture
    /// session.
    ///
    /// `stats.duration_ms` (and, for projects written before this split,
    /// `video.duration_ms`) is wall-clock elapsed time. The encoder writes CFR
    /// at `nominal_fps`, so whenever the capture dropped frames the file is
    /// SHORTER than the wall clock: a 27.102 s session that encoded 3195 frames
    /// at 120 fps produces exactly 26.625 s of video. Seeding `trim_end` from
    /// the wall clock therefore wrote a render state the export validator
    /// (which probes the real file) rejects as `trim_end_exceeds_source`.
    ///
    /// `encoded_frames / nominal_fps` is that CFR identity, so this matches
    /// ffprobe to the microsecond without paying for a probe spawn. Falls back
    /// to the stored duration when the frame count is unavailable.
    pub fn media_duration_secs(&self) -> f64 {
        if self.stats.encoded_frames > 0 && self.stats.nominal_fps > 0 {
            self.stats.encoded_frames as f64 / self.stats.nominal_fps as f64
        } else {
            self.video.duration_ms as f64 / 1000.0
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectVideoMetadata {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMediaMetadata {
    pub has_system_audio: bool,
    pub has_microphone: bool,
    pub has_camera: bool,
    /// Whether the camera was ASKED for, regardless of whether a track arrived.
    ///
    /// `has_camera` alone can't tell a camera that was switched off from one
    /// that was requested and failed (device busy, permission denied) — and the
    /// editor otherwise tells someone to "turn the camera on" when they did.
    /// Defaulted so bundles written before this field read as "not requested",
    /// which for them is indistinguishable from off anyway.
    #[serde(default)]
    pub camera_requested: bool,
    /// Signed millisecond offsets of the audio / mic / camera tracks relative
    /// to video frame 0. Absent on bundles written before offsets were
    /// measured, which read as "assume aligned" (the old behaviour).
    #[serde(default)]
    pub track_offsets: crate::recording::TrackOffsets,
}

#[cfg(test)]
mod duration_tests {
    use super::*;
    use serde_json::json;

    fn metadata(encoded_frames: u64, nominal_fps: u32, wall_ms: u64) -> ProjectMetadata {
        serde_json::from_value(json!({
            "schemaVersion": 1,
            "createdAtUnixMs": 1_700_000_000_000u64,
            "captureTarget": {
                "kind": "display", "id": 1, "label": "Display 1",
                "source": { "x": 0, "y": 0, "width": 1920, "height": 1080 },
                "crop": { "x": 0, "y": 0, "width": 1920, "height": 1080 },
                "displayId": 1, "scaleFactor": 1.0
            },
            "stats": {
                "capturedFrames": encoded_frames, "encodedFrames": encoded_frames,
                "droppedFrames": 0, "durationMs": wall_ms, "nominalFps": nominal_fps
            },
            "video": { "width": 1920, "height": 1080, "fps": nominal_fps, "durationMs": wall_ms }
        }))
        .expect("fixture metadata")
    }

    /// Frame counts, fps and the ffprobe `format.duration` of real recordings on
    /// disk. Every one of these had a wall clock LONGER than the file, which is
    /// what made `enqueue_export` reject the app's own render state.
    #[test]
    fn media_duration_matches_probed_file_not_wall_clock() {
        for (frames, fps, wall_ms, probed) in [
            (3195u64, 120u32, 27_102u64, 26.625f64),
            (5529, 120, 46_607, 46.075),
            (1010, 60, 17_314, 16.833_333_333_333_332),
            (2132, 120, 18_254, 17.766_666_666_666_666),
            (13822, 60, 231_083, 230.366_666_666_666_67),
        ] {
            let m = metadata(frames, fps, wall_ms);
            assert!(
                (m.media_duration_secs() - probed).abs() < 1e-6,
                "{frames}@{fps}: got {} want {probed}",
                m.media_duration_secs(),
            );
            assert!(
                m.media_duration_secs() < wall_ms as f64 / 1000.0,
                "wall clock must be the longer of the two",
            );
        }
    }

    #[test]
    fn falls_back_to_stored_duration_without_a_frame_count() {
        assert_eq!(metadata(0, 60, 10_000).media_duration_secs(), 10.0);
        assert_eq!(metadata(600, 0, 10_000).media_duration_secs(), 10.0);
    }

    /// `cameraRequested` is additive: bundles written before it must keep
    /// deserializing, and must not claim the camera was asked for.
    #[test]
    fn media_metadata_without_camera_requested_reads_as_not_requested() {
        let media: ProjectMediaMetadata = serde_json::from_value(json!({
            "hasSystemAudio": true, "hasMicrophone": true, "hasCamera": false
        }))
        .expect("pre-field media metadata must still parse");
        assert!(!media.camera_requested);
    }

    #[test]
    fn media_metadata_round_trips_camera_requested() {
        let media: ProjectMediaMetadata = serde_json::from_value(json!({
            "hasSystemAudio": false, "hasMicrophone": false,
            "hasCamera": false, "cameraRequested": true
        }))
        .expect("fixture media metadata");
        assert!(media.camera_requested);
        // The pair that means "asked for it, never arrived".
        assert!(!media.has_camera);
        let back = serde_json::to_value(&media).expect("serialize");
        assert_eq!(back["cameraRequested"], json!(true));
    }
}
