use serde::{Deserialize, Serialize};

use crate::capture::CaptureTarget;
use crate::recording::RecordingStats;

pub mod autosave;
pub mod documents;
pub mod format;
pub mod journal;
pub mod journal_doc;
pub mod reader;
pub mod v3;
pub mod watch;
pub mod writer;

/// On-disk shape a project was opened from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    V1,
    V2,
    V3,
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
    /// Duration of the recorded media, not of the capture session: CFR encoding makes a dropped-frame recording SHORTER than the wall clock.
    /// `encoded_frames / nominal_fps` matches ffprobe without a probe spawn; seeding `trim_end` from wall clock produced `trim_end_exceeds_source`.
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
    /// Whether the camera was ASKED for, regardless of whether a track arrived: `has_camera` alone cannot separate switched-off from requested-and-failed.
    /// Defaulted, so bundles written before this field read as not requested, which for them is indistinguishable from off.
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
