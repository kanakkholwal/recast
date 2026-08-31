//! Decoding a scene's audio into the sources `recast-audio` mixes. Decoding is
//! the only platform-specific part, which is why `SceneSources` takes samples.

use std::path::{Path, PathBuf};

use recast_audio::{Samples, SceneSources, MASTER_CHANNELS, MASTER_RATE};
use recast_scene::v1::nodes::AudioClipSource;
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

/// Everything `graph` refers to. A clip that will not decode is left out, not
/// silenced, and never fatal: one bad music file must not fail a good export.
#[cfg(windows)]
#[must_use]
pub fn sources_for(graph: &AudioGraph, recording: &Path) -> SceneSources {
    use recast_audio::RecordingKind;

    let mut sources = SceneSources::new();
    match decode(recording) {
        Ok(Some(samples)) => {
            sources = sources.recording(RecordingKind::Source, Box::new(samples));
        }
        Ok(None) => log::debug!("export: {} has no audio track", recording.display()),
        Err(error) => log::warn!("export: the recording's audio did not decode: {error}"),
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

#[cfg(test)]
mod tests {
    use super::*;

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
