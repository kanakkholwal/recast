//! Decoding a scene's audio into the sources `recast-audio` mixes. Decoding is
//! the only platform-specific part, which is why `SceneSources` takes samples.

use std::path::{Path, PathBuf};

use recast_audio::SceneSources;
// Only the native decode path builds Samples; off Windows the stub returns an empty SceneSources.
#[cfg(windows)]
use recast_audio::{SampleSource, Samples, MASTER_CHANNELS, MASTER_RATE};
use recast_scene::v1::nodes::{AudioClipRole, AudioClipSource};
use recast_scene::AudioGraph;
use recast_time::MappedSpan;

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
    let opened = match AudioReader::open(path, format) {
        Ok(Some(reader)) => Some(reader),
        Ok(None) => return Ok(None),
        // A codec the in-process reader refuses is FFmpeg's job, not silence.
        Err(error) => {
            log::info!(
                "export: {} needs ffmpeg to decode ({error})",
                path.display()
            );
            None
        }
    };
    let data = match opened {
        Some(mut reader) => reader
            .read_all()
            .map_err(|e| format!("{}: {e}", path.display()))?,
        None => crate::audio_decode::decode_interleaved(path, MASTER_RATE, MASTER_CHANNELS as u16)?,
    };
    if data.is_empty() {
        return Ok(None);
    }
    Ok(Some(Samples::new(
        data,
        MASTER_RATE,
        MASTER_CHANNELS as u16,
    )))
}

/// Whether `spans` describe a timeline this can conform audio to.
///
/// Trim and cuts are a rearrangement of samples, which [`conform`] does exactly.
/// A speed change is a resample, and FFmpeg's `atempo` preserves pitch where a
/// plain resample does not, so a sped-up project must keep the FFmpeg path
/// rather than export audio that disagrees with the preview.
#[must_use]
pub fn spans_are_conformable(spans: &[MappedSpan]) -> bool {
    spans.iter().all(|s| (s.speed - 1.0).abs() < 1e-6)
}

/// Rewrites `data` onto the OUTPUT axis: the surviving stretches of the
/// recording, concatenated in the order the video plays them.
///
/// Without this the mixer places the whole undisturbed source at output zero,
/// so a project trimmed to start at 10s exported audio from 0s under video
/// from 10s: the entire file out of sync, not merely drifting.
#[cfg(windows)]
fn conform(data: &mut Vec<f32>, rate: u32, channels: u16, spans: &[MappedSpan]) {
    let frame = channels.max(1) as usize;
    let frames = data.len() / frame;
    let at = |sec: f64| ((sec.max(0.0) * f64::from(rate)).round() as usize).min(frames);
    // In place: spans only move samples earlier, so a forward copy cannot overwrite what it has yet to read, and a 30-minute track need not exist twice.
    let mut kept = 0usize;
    for span in spans {
        let (from, to) = (at(span.orig_start), at(span.orig_end));
        if to <= from {
            continue;
        }
        debug_assert!(kept <= from, "the spans ran backwards over one another");
        data.copy_within(from * frame..to * frame, kept * frame);
        kept += to - from;
    }
    data.truncate(kept * frame);
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
pub fn sources_for(
    graph: &AudioGraph,
    recording: &RecordingAudio<'_>,
    spans: &[MappedSpan],
) -> SceneSources {
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
            // Conformed here, not placed by the mixer: `Placement` describes ONE contiguous stretch and a cut list is many.
            Ok(Some(samples)) => {
                sources = sources.recording(kind, Box::new(conformed(samples, spans)))
            }
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

/// [`conform`] applied to decoded samples, or the samples unchanged when the
/// timeline is one uncut stretch starting at zero.
#[cfg(windows)]
fn conformed(samples: Samples, spans: &[MappedSpan]) -> Samples {
    if spans.is_empty() {
        return samples;
    }
    let (rate, channels) = (samples.sample_rate(), samples.channels());
    let mut data = samples.into_data();
    conform(&mut data, rate, channels, spans);
    Samples::new(data, rate, channels)
}

/// No in-process decoder here, and none needed: the platforms without one take
/// the mux pass, which builds the audio track from the render state itself.
#[cfg(not(windows))]
#[must_use]
pub fn sources_for(
    _graph: &AudioGraph,
    _recording: &RecordingAudio<'_>,
    _spans: &[MappedSpan],
) -> SceneSources {
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

    fn span(orig_start: f64, orig_end: f64, speed: f64) -> MappedSpan {
        MappedSpan {
            orig_start,
            orig_end,
            speed,
            out_start: 0.0,
            out_end: 0.0,
        }
    }

    /// The mixer places a recording at output zero with no offset, so a trimmed
    /// project exported audio from 0s under video from the trim: the whole file
    /// out of sync, not drifting.
    #[cfg(windows)]
    #[test]
    fn a_trim_drops_the_audio_before_it() {
        // One second of mono at 4 Hz, each sample naming its own second.
        let mut data: Vec<f32> = (0..8).map(|i| i as f32).collect();
        conform(&mut data, 4, 1, &[span(1.0, 2.0, 1.0)]);
        assert_eq!(data, vec![4.0, 5.0, 6.0, 7.0]);
    }

    /// Two surviving stretches are concatenated in play order, which is what a
    /// cut in the middle of a recording leaves behind.
    #[cfg(windows)]
    #[test]
    fn a_cut_joins_what_is_left_either_side_of_it() {
        let mut data: Vec<f32> = (0..8).map(|i| i as f32).collect();
        conform(&mut data, 4, 1, &[span(0.0, 0.5, 1.0), span(1.5, 2.0, 1.0)]);
        assert_eq!(data, vec![0.0, 1.0, 6.0, 7.0]);
    }

    /// Interleaved samples must be cut on FRAME boundaries, or the channels swap.
    #[cfg(windows)]
    #[test]
    fn a_stereo_source_is_cut_on_frame_boundaries() {
        let mut data: Vec<f32> = vec![0.0, 10.0, 1.0, 11.0, 2.0, 12.0, 3.0, 13.0];
        // 2 Hz, so 1.0s..2.0s is frames 2 and 3, each a (left, right) pair.
        conform(&mut data, 2, 2, &[span(1.0, 2.0, 1.0)]);
        assert_eq!(data, vec![2.0, 12.0, 3.0, 13.0]);
    }

    /// The wrapper is the wiring: `conform` being right is no use if the
    /// recording reaches the mixer unconformed.
    #[cfg(windows)]
    #[test]
    fn the_spans_reach_the_samples_the_mixer_will_place() {
        let samples = Samples::new((0..8).map(|i| i as f32).collect(), 4, 1);
        let out = conformed(samples, &[span(1.0, 2.0, 1.0)]);
        assert_eq!(out.data(), &[4.0, 5.0, 6.0, 7.0]);
        assert_eq!(out.sample_rate(), 4);
        assert_eq!(out.channels(), 1);
    }

    /// No spans is a project with no trim, cuts or splits, which must not be
    /// mistaken for "keep nothing".
    #[cfg(windows)]
    #[test]
    fn no_spans_leaves_the_recording_whole() {
        let samples = Samples::new(vec![1.0, 2.0], 4, 1);
        assert_eq!(conformed(samples, &[]).data(), &[1.0, 2.0]);
    }

    /// A resample would shift pitch where FFmpeg's `atempo` does not, so a
    /// sped-up project has to keep the path whose audio matches the preview.
    #[test]
    fn a_speed_change_is_not_something_this_can_conform() {
        assert!(spans_are_conformable(&[span(0.0, 1.0, 1.0)]));
        assert!(spans_are_conformable(&[]));
        assert!(!spans_are_conformable(&[
            span(0.0, 1.0, 1.0),
            span(1.0, 2.0, 2.0)
        ]));
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
