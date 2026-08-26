#![cfg(feature = "scene")]

use recast_audio::{
    mixer_for, RecordingKind, SampleSource, Samples, SceneSources, MASTER_RATE,
};
use recast_scene::v1::nodes::{AudioClip, AudioClipSource, AudioSettings};
use recast_scene::AudioGraph;

/// A constant, so a level reads straight off any sample.
fn flat(level: f32, seconds: f64) -> Box<dyn SampleSource> {
    let frames = (seconds * MASTER_RATE as f64) as usize;
    Box::new(Samples::mono(vec![level; frames], MASTER_RATE))
}

fn peak_between(mix: &[f32], from: f64, to: f64) -> f32 {
    let first = (from * MASTER_RATE as f64) as usize;
    let last = ((to * MASTER_RATE as f64) as usize).min(mix.len() / 2);
    mix[first * 2..last * 2]
        .iter()
        .fold(0.0f32, |m, v| m.max(v.abs()))
}

fn clip(id: &str) -> AudioClip {
    AudioClip {
        id: id.into(),
        source: AudioClipSource::Local { path: id.into() },
        role: Default::default(),
        start_output_sec: 0.0,
        offset_sec: 0.0,
        duration_sec: 0.0,
        gain: 100.0,
        muted: false,
        fade_in: 0.0,
        fade_out: 0.0,
        looping: false,
        ducking: false,
    }
}

#[test]
fn percentages_become_linear_gain() {
    let graph = AudioGraph {
        settings: AudioSettings {
            volume: 50.0,
            ..AudioSettings::default()
        },
        clips: Vec::new(),
    };
    let sources = SceneSources::new().recording(RecordingKind::Source, flat(0.8, 1.0));
    let mix = mixer_for(&graph, 1.0, sources).render_all();
    let level = peak_between(&mix, 0.1, 0.9);
    assert!((level - 0.4).abs() < 0.001, "read {level}");
}

/// The master volume is the master node's, and each recording keeps only its
/// own trim. Both used to be folded into one filter because there was no master.
#[test]
fn a_source_trim_and_the_master_multiply_once_each() {
    let graph = AudioGraph {
        settings: AudioSettings {
            volume: 50.0,
            mic_volume: 50.0,
            ..AudioSettings::default()
        },
        clips: Vec::new(),
    };
    let sources = SceneSources::new().recording(RecordingKind::Mic, flat(0.8, 1.0));
    let mix = mixer_for(&graph, 1.0, sources).render_all();
    let level = peak_between(&mix, 0.1, 0.9);
    assert!((level - 0.2).abs() < 0.001, "read {level}");
}

/// The screen recording's own audio is already muxed into the video, so the
/// microphone slider has no business touching it. Only the master does.
#[test]
fn the_source_stream_ignores_the_microphone_trim() {
    let graph = AudioGraph {
        settings: AudioSettings {
            mic_volume: 10.0,
            mic_muted: true,
            ..AudioSettings::default()
        },
        clips: Vec::new(),
    };
    let sources = SceneSources::new().recording(RecordingKind::Source, flat(0.5, 1.0));
    let mix = mixer_for(&graph, 1.0, sources).render_all();
    let level = peak_between(&mix, 0.1, 0.9);
    assert!((level - 0.5).abs() < 0.001, "read {level}");
}

#[test]
fn a_muted_stream_is_left_out_of_the_mix() {
    let graph = AudioGraph {
        settings: AudioSettings {
            system_muted: true,
            ..AudioSettings::default()
        },
        clips: Vec::new(),
    };
    let sources = SceneSources::new()
        .recording(RecordingKind::System, flat(0.5, 1.0))
        .recording(RecordingKind::Mic, flat(0.25, 1.0));
    let mix = mixer_for(&graph, 1.0, sources).render_all();
    let level = peak_between(&mix, 0.1, 0.9);
    assert!((level - 0.25).abs() < 0.001, "read {level}");
}

/// A volume above four hundred percent is a corrupt project, not a request.
#[test]
fn an_absurd_volume_is_clamped_rather_than_obeyed() {
    let graph = AudioGraph {
        settings: AudioSettings {
            volume: 10_000.0,
            ..AudioSettings::default()
        },
        clips: Vec::new(),
    };
    let sources = SceneSources::new().recording(RecordingKind::Source, flat(0.1, 1.0));
    let mix = mixer_for(&graph, 1.0, sources).render_all();
    let level = peak_between(&mix, 0.1, 0.9);
    assert!((level - 0.4).abs() < 0.001, "read {level}");
}

#[test]
fn a_clip_lands_where_the_project_put_it() {
    let mut music = clip("bed");
    music.start_output_sec = 1.0;
    music.gain = 25.0;
    let graph = AudioGraph {
        settings: AudioSettings::default(),
        clips: vec![music],
    };
    let sources = SceneSources::new().clip("bed", flat(0.8, 1.0));
    let mix = mixer_for(&graph, 3.0, sources).render_all();
    assert_eq!(peak_between(&mix, 0.0, 0.9), 0.0);
    let level = peak_between(&mix, 1.1, 1.9);
    assert!((level - 0.2).abs() < 0.001, "read {level}");
}

/// A clip whose file never decoded is dropped. Filling it with silence would
/// leave a mix that is quietly missing a stem and looks complete.
#[test]
fn a_clip_with_no_decoded_source_is_left_out() {
    let graph = AudioGraph {
        settings: AudioSettings::default(),
        clips: vec![clip("missing")],
    };
    let mix = mixer_for(&graph, 1.0, SceneSources::new()).render_all();
    assert_eq!(peak_between(&mix, 0.0, 1.0), 0.0);
}

#[test]
fn two_clips_take_their_own_sources_rather_than_sharing_one() {
    let mut first = clip("a");
    first.gain = 50.0;
    let mut second = clip("b");
    second.gain = 50.0;
    let graph = AudioGraph {
        settings: AudioSettings::default(),
        clips: vec![first, second],
    };
    let sources = SceneSources::new()
        .clip("a", flat(0.2, 1.0))
        .clip("b", flat(0.6, 1.0));
    let mix = mixer_for(&graph, 1.0, sources).render_all();
    let level = peak_between(&mix, 0.1, 0.9);
    assert!((level - 0.4).abs() < 0.001, "read {level}");
}

/// A clip is matched by id, not by the order it happens to arrive in.
#[test]
fn clips_are_matched_by_id_not_by_position() {
    let mut first = clip("a");
    first.gain = 50.0;
    let mut second = clip("b");
    second.muted = true;
    let graph = AudioGraph {
        settings: AudioSettings::default(),
        clips: vec![first, second],
    };
    let sources = SceneSources::new()
        .clip("b", flat(0.9, 1.0))
        .clip("a", flat(0.2, 1.0));
    let mix = mixer_for(&graph, 1.0, sources).render_all();
    let level = peak_between(&mix, 0.1, 0.9);
    assert!((level - 0.1).abs() < 0.001, "read {level}");
}

#[test]
fn a_ducking_clip_becomes_a_ducked_track() {
    let mut voice = clip("voice");
    voice.start_output_sec = 1.0;
    voice.duration_sec = 1.0;
    let mut bed = clip("bed");
    bed.ducking = true;
    bed.gain = 25.0;
    let graph = AudioGraph {
        settings: AudioSettings::default(),
        clips: vec![voice, bed],
    };
    let sources = SceneSources::new()
        .clip("voice", flat(0.5, 4.0))
        .clip("bed", flat(0.8, 4.0));
    let mix = mixer_for(&graph, 4.0, sources).render_all();

    let before = peak_between(&mix, 0.5, 0.9);
    assert!((before - 0.2).abs() < 0.001, "bed read {before} before the key");
    let under = mix[(1.8 * MASTER_RATE as f64) as usize * 2
        ..(1.95 * MASTER_RATE as f64) as usize * 2]
        .iter()
        .step_by(2)
        .fold(0.0f32, |m, v| m.max((v - 0.5).abs()));
    assert!(under < 0.07, "bed held {under} under the key");
}

#[test]
fn the_normalize_flag_reaches_the_master() {
    let graph = AudioGraph {
        settings: AudioSettings {
            normalize_loudness: true,
            ..AudioSettings::default()
        },
        clips: Vec::new(),
    };
    assert!(mixer_for(&graph, 1.0, SceneSources::new()).master().normalize);

    let off = AudioGraph::default();
    assert!(!mixer_for(&off, 1.0, SceneSources::new()).master().normalize);
}

#[test]
fn the_master_fades_come_from_the_settings() {
    let graph = AudioGraph {
        settings: AudioSettings {
            fade_in: 0.5,
            fade_out: 0.25,
            ..AudioSettings::default()
        },
        clips: Vec::new(),
    };
    let master = *mixer_for(&graph, 2.0, SceneSources::new()).master();
    assert_eq!((master.fade_in, master.fade_out), (0.5, 0.25));
    assert_eq!(master.duration_sec, 2.0);
}
