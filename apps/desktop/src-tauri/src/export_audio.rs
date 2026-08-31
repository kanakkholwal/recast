//! Decoding a scene's audio into the sources `recast-audio` mixes. Decoding is
//! the only platform-specific part, which is why `SceneSources` takes samples.

use std::path::{Path, PathBuf};

use recast_audio::SceneSources;
// Only the native decode path builds Samples; off Windows the stub returns an empty SceneSources.
#[cfg(windows)]
use recast_audio::{Samples, MASTER_CHANNELS, MASTER_RATE};
use recast_scene::v1::nodes::{AudioClipRole, AudioClipSource};
use recast_scene::AudioGraph;

/// Where a clip's media lives, or `None` when unreadable. A provider clip is
/// cached under `asset_path`; a local one names its file.
#[must_use]
pub fn clip_path(source: &AudioClipSource) -> Option<PathBuf> {
    match source {
        AudioClipSource::Local { path } => Some(PathBuf::from(path)),
        AudioClipSource::Provider { asset_path, .. } if !asset_path.is_empty() => {
            Some(PathBuf::from(asset_path))
        }
        AudioClipSource::Provider { .. } => None,
    }
}

/// Decodes `path` to the master rate and channels. `Ok(None)` when the file has
/// no audio track, which a recording made with no microphone legitimately has.
#[cfg(windows)]
pub fn decode(path: &Path) -> Result<Option<Samples>, String> {
    use recast_codec_mf::{AudioFormat, AudioReader};

    let format = AudioFormat {
        sample_rate: MASTER_RATE,
        channels: MASTER_CHANNELS as u16,
    };
    let Some(mut reader) =
        AudioReader::open(path, format).map_err(|e| format!("{}: {e}", path.display()))?
    else {
        return Ok(None);
    };
    let data = reader
        .read_all()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if data.is_empty() {
        return Ok(None);
    }
    Ok(Some(Samples::new(
        data,
        MASTER_RATE,
        MASTER_CHANNELS as u16,
    )))
}

/// The recording's own tracks. A project captures the microphone and system
/// audio to their own files, and an export that reads only the video loses both.
#[derive(Debug, Clone, Default)]
pub struct RecordingAudio<'a> {
    pub video: Option<&'a Path>,
    pub system: Option<&'a Path>,
    pub microphone: Option<&'a Path>,
}

/// A voice-over replaces the recording's own audio rather than layering over
/// it, which is what the FFmpeg mux calls detaching.
#[must_use]
pub fn voice_detached(graph: &AudioGraph) -> bool {
    graph
        .clips
        .iter()
        .any(|clip| clip.role == AudioClipRole::Voice && !clip.muted && clip.gain > 0.0)
}

/// Everything `graph` refers to. A clip that will not decode is left out, not
/// silenced, and never fatal: one bad music file must not fail a good export.
#[cfg(windows)]
#[must_use]
pub fn sources_for(graph: &AudioGraph, recording: &RecordingAudio<'_>) -> SceneSources {
    use recast_audio::RecordingKind;

    let mut sources = SceneSources::new();
    // A voice-over takes the recording's place, so its captured tracks are dropped.
    let captured: &[(Option<&Path>, RecordingKind)] = &match voice_detached(graph) {
        true => [(None, RecordingKind::Source); 3],
        false => [
            (recording.video, RecordingKind::Source),
            (recording.system, RecordingKind::System),
            (recording.microphone, RecordingKind::Mic),
        ],
    };
    // A capture that never opened leaves a header with no samples, which decodes to `Ok(None)` and needs no guard of its own.
    for (path, kind) in captured.iter().filter_map(|(p, k)| p.map(|p| (p, *k))) {
        match decode(path) {
            Ok(Some(samples)) => sources = sources.recording(kind, Box::new(samples)),
            Ok(None) => log::info!("export: {} has no audio to mix", path.display()),
            Err(error) => log::warn!("export: {} did not decode: {error}", path.display()),
        }
    }

    for clip in &graph.clips {
        if clip.muted || clip.gain <= 0.0 {
            continue;
        }
        let Some(path) = clip_path(&clip.source) else {
            log::warn!("export: audio clip {} names no readable source", clip.id);
            continue;
        };
        match decode(&path) {
            Ok(Some(samples)) => sources = sources.clip(clip.id.clone(), Box::new(samples)),
            Ok(None) => log::warn!("export: audio clip {} decoded to nothing", clip.id),
            Err(error) => log::warn!("export: audio clip {} did not decode: {error}", clip.id),
        }
    }
    sources
}

/// No in-process decoder here, and none needed: the platforms without one take
/// the mux pass, which builds the audio track from the render state itself.
#[cfg(not(windows))]
#[must_use]
pub fn sources_for(_graph: &AudioGraph, _recording: &RecordingAudio<'_>) -> SceneSources {
    SceneSources::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(role: AudioClipRole, muted: bool, gain: f64) -> recast_scene::v1::nodes::AudioClip {
        recast_scene::v1::nodes::AudioClip {
            id: "c1".into(),
            source: AudioClipSource::Local {
                path: "music.mp3".into(),
            },
            role,
            gain,
            muted,
            ..serde_json::from_value(serde_json::json!({
                "id": "c1",
                "source": { "kind": "local", "path": "music.mp3" }
            }))
            .expect("clip defaults")
        }
    }

    fn graph_with(clips: Vec<recast_scene::v1::nodes::AudioClip>) -> AudioGraph {
        AudioGraph {
            settings: Default::default(),
            clips,
        }
    }

    /// A voice-over replaces the recording rather than layering over it, which
    /// is what the FFmpeg mux does by dropping the captured inputs.
    #[test]
    fn an_audible_voice_over_detaches_the_recording() {
        assert!(voice_detached(&graph_with(vec![clip(
            AudioClipRole::Voice,
            false,
            100.0
        )])));
    }

    /// Every reason a voice clip does not take over. Each used to be the same
    /// silent nothing, so they are asserted together.
    #[test]
    fn a_voice_over_that_cannot_be_heard_leaves_the_recording_alone() {
        for (name, clips) in [
            ("no clips at all", vec![]),
            (
                "music, not voice",
                vec![clip(AudioClipRole::Music, false, 100.0)],
            ),
            (
                "a muted voice clip",
                vec![clip(AudioClipRole::Voice, true, 100.0)],
            ),
            (
                "a silent voice clip",
                vec![clip(AudioClipRole::Voice, false, 0.0)],
            ),
        ] {
            assert!(!voice_detached(&graph_with(clips)), "detached on {name}");
        }
    }

    #[test]
    fn a_local_clip_names_its_own_file() {
        let source = AudioClipSource::Local {
            path: "C:/music/loop.mp3".into(),
        };
        assert_eq!(clip_path(&source), Some(PathBuf::from("C:/music/loop.mp3")));
    }

    #[test]
    fn a_provider_clip_reads_from_its_cached_asset() {
        let source = AudioClipSource::Provider {
            provider_id: "epidemic".into(),
            track_id: "abc".into(),
            asset_path: "C:/cache/abc.mp3".into(),
            attribution: None,
            license: None,
        };
        assert_eq!(clip_path(&source), Some(PathBuf::from("C:/cache/abc.mp3")));
    }

    /// A provider clip whose asset never downloaded has nothing to read. Naming
    /// the empty string as a path would decode the working directory.
    #[test]
    fn a_provider_clip_with_no_cached_asset_names_nothing() {
        let source = AudioClipSource::Provider {
            provider_id: "epidemic".into(),
            track_id: "abc".into(),
            asset_path: String::new(),
            attribution: None,
            license: None,
        };
        assert_eq!(clip_path(&source), None);
    }
}
