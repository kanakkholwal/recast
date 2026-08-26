use recast_scene::v1::nodes::AudioClip;
use recast_scene::AudioGraph;

use crate::mix::{Master, Mixer, Placement, Track};
use crate::source::SampleSource;

/// Volumes are stored as percentages. Four is the same ceiling the panel offers,
/// and it stops a corrupt project asking for a gain of a thousand.
fn percent(value: f64) -> f32 {
    (value / 100.0).clamp(0.0, 4.0) as f32
}

/// Which recorded stream a source is. The three are separate tracks because
/// each has its own gain and mute in the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingKind {
    /// Audio muxed into the screen recording itself.
    Source,
    System,
    Mic,
}

/// The decoded audio a scene refers to. Decoding belongs to the caller, which
/// is the only part of this that is platform-specific.
#[derive(Default)]
pub struct SceneSources {
    recordings: Vec<(RecordingKind, Box<dyn SampleSource>)>,
    clips: Vec<(String, Box<dyn SampleSource>)>,
}

impl SceneSources {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn recording(mut self, kind: RecordingKind, source: Box<dyn SampleSource>) -> Self {
        self.recordings.push((kind, source));
        self
    }

    /// `id` is the [`AudioClip`] id the source belongs to.
    pub fn clip(mut self, id: impl Into<String>, source: Box<dyn SampleSource>) -> Self {
        self.clips.push((id.into(), source));
        self
    }
}

/// Builds the mixer a scene describes. Clips with no decoded source are left
/// out rather than filled with silence, so a missing file is visible as a
/// missing clip instead of a mix that quietly lost a stem.
pub fn mixer_for(graph: &AudioGraph, duration_sec: f64, sources: SceneSources) -> Mixer {
    let settings = &graph.settings;
    let master = Master {
        gain: percent(settings.volume),
        muted: settings.muted,
        fade_in: settings.fade_in,
        fade_out: settings.fade_out,
        normalize: settings.normalize_loudness,
        ..Master::new(duration_sec)
    };
    let mut mixer = Mixer::new(master);

    for (kind, source) in sources.recordings {
        // The master gain is the master node's now, so each recording carries
        // only its own trim. The FFmpeg graph had to fold both into one filter
        // because it had nowhere else to put the master.
        let (gain, muted) = match kind {
            RecordingKind::Source => (1.0, false),
            RecordingKind::System => (percent(settings.system_volume), settings.system_muted),
            RecordingKind::Mic => (percent(settings.mic_volume), settings.mic_muted),
        };
        if muted || gain == 0.0 {
            continue;
        }
        let mut track = Track::new(source);
        track.gain = gain;
        mixer.push(track);
    }

    // Taken out as they are matched, so two clips cannot claim one source.
    let mut remaining = sources.clips;
    for clip in &graph.clips {
        if clip.muted || clip.gain <= 0.0 {
            continue;
        }
        let Some(index) = remaining.iter().position(|(id, _)| *id == clip.id) else {
            continue;
        };
        let (_, source) = remaining.remove(index);
        mixer.push(track_for(clip, source));
    }
    mixer
}

fn track_for(clip: &AudioClip, source: Box<dyn SampleSource>) -> Track {
    let mut track = Track::new(source);
    track.placement = Placement {
        start_sec: clip.start_output_sec.max(0.0),
        offset_sec: clip.offset_sec.max(0.0),
        duration_sec: clip.duration_sec.max(0.0),
    };
    track.gain = percent(clip.gain);
    track.fade_in = clip.fade_in.max(0.0);
    track.fade_out = clip.fade_out.max(0.0);
    track.looping = clip.looping;
    track.ducked = clip.ducking;
    track
}
