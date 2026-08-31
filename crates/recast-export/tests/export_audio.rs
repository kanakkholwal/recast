#![cfg(windows)]

use recast_audio::{Master, Mixer, Samples, Track, MASTER_CHANNELS, MASTER_RATE};
use recast_compositor::{
    PlaneData, PlaneLayout, RenderSource, Session, SourceColor, SourceGeometry, SourcePlanes,
};
use recast_export::{FrameLoop, FrameWalk, Mp4Sink, PictureSource};
use recast_gpu::{GpuContext, GpuOptions};
use recast_scene::migrate::to_scene;
use recast_scene::Scene;

/// AAC-LC frame size. The muxer counts audio duration in samples.
const AAC_FRAME_SAMPLES: u32 = 1024;

const SRC_W: u32 = 640;
const SRC_H: u32 = 360;

fn context() -> Option<&'static GpuContext> {
    static SHARED: std::sync::OnceLock<Option<GpuContext>> = std::sync::OnceLock::new();
    SHARED
        .get_or_init(|| {
            GpuContext::new_blocking(GpuOptions {
                require_hardware: false,
                ..Default::default()
            })
            .map_err(|e| eprintln!("skipping: no GPU adapter ({e})"))
            .ok()
        })
        .as_ref()
}

const BASE: &str = r##"{
    "trimStart": 0.0, "trimEnd": 2.0,
    "backgroundType": "color", "backgroundValue": "#2200ff", "backgroundBlur": 0.0,
    "padding": 0.0, "cursorEnabled": false, "cursorSize": 3.0, "cursorSmoothing": 50.0,
    "cursorHighlightClicks": true, "cursorHighlightColor": "#3b82f6",
    "cursorHighlightOpacity": 40.0, "cursorHideWhenIdle": false, "cursorIdleTimeout": 3.0,
    "zoomRegions": []
}"##;

fn session(ctx: &GpuContext) -> Session {
    let state = serde_json::from_str(BASE).expect("fixture parses");
    let scene: Scene = to_scene(&state);
    Session::new(
        ctx,
        scene,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    )
    .expect("session")
}

struct Flat(Vec<u8>);

impl Flat {
    fn new() -> Self {
        let mut bytes = vec![200; (SRC_W * SRC_H) as usize];
        bytes.resize(PlaneLayout::Nv12.packed_len(SRC_W, SRC_H), 128);
        Self(bytes)
    }
}

impl PictureSource for Flat {
    type Error = std::convert::Infallible;

    fn picture_at(&mut self, _source_time: f64) -> Result<Option<SourcePlanes<'_>>, Self::Error> {
        Ok(Some(SourcePlanes {
            width: SRC_W,
            height: SRC_H,
            layout: PlaneLayout::Nv12,
            color: SourceColor::default(),
            data: PlaneData::Packed(&self.0),
        }))
    }
}

/// A 440 Hz tone, so the track carries something a decoder can be asked about.
fn tone(seconds: f64) -> Samples {
    let frames = (f64::from(MASTER_RATE) * seconds) as usize;
    let mut data = vec![0.0f32; frames * MASTER_CHANNELS];
    for frame in 0..frames {
        let t = frame as f64 / f64::from(MASTER_RATE);
        let value = (t * 440.0 * std::f64::consts::TAU).sin() as f32 * 0.5;
        for channel in 0..MASTER_CHANNELS {
            data[frame * MASTER_CHANNELS + channel] = value;
        }
    }
    Samples::new(data, MASTER_RATE, MASTER_CHANNELS as u16)
}

fn mixer(seconds: f64) -> Mixer {
    let mut mixer = Mixer::new(Master::new(seconds));
    mixer.push(Track::new(Box::new(tone(seconds))));
    mixer
}

/// Renders video and audio into one file and returns its bytes.
fn export(ctx: &GpuContext, walk: FrameWalk, seconds: f64, with_audio: bool) -> (Vec<u8>, u64) {
    let mut session = session(ctx);
    let size = RenderSource::output_size(&session);
    let mut sink = Mp4Sink::new(
        size.width,
        size.height,
        walk,
        4_000_000,
        SourceColor::default(),
    )
    .expect("an H.264 encoder");

    FrameLoop::new()
        .run(
            &mut session,
            &mut Flat::new(),
            walk,
            ctx.device(),
            ctx.queue(),
            |index, rgba| sink.push(index, rgba),
        )
        .expect("rendered");

    if with_audio {
        sink.push_audio(&mut mixer(seconds), 128_000)
            .expect("audio encodes");
    }
    let audio = sink.audio_sample_count();
    (sink.finish().expect("a finished file"), audio)
}

/// Exports with `mix` as the only audio, returning the file bytes.
fn export_with(ctx: &GpuContext, walk: FrameWalk, mut mix: Mixer) -> Vec<u8> {
    let mut session = session(ctx);
    let size = RenderSource::output_size(&session);
    let mut sink = Mp4Sink::new(
        size.width,
        size.height,
        walk,
        4_000_000,
        SourceColor::default(),
    )
    .expect("an H.264 encoder");
    FrameLoop::new()
        .run(
            &mut session,
            &mut Flat::new(),
            walk,
            ctx.device(),
            ctx.queue(),
            |index, rgba| sink.push(index, rgba),
        )
        .expect("rendered");
    sink.push_audio(&mut mix, 128_000).expect("audio encodes");
    sink.finish().expect("a finished file")
}

#[test]
fn an_export_with_sound_carries_an_audio_track() {
    let Some(ctx) = context() else { return };
    let (bytes, audio) = export(ctx, FrameWalk::new(0.5, (30, 1)), 0.5, true);
    assert!(audio > 0, "no AAC frames were written");
    assert!(!bytes.is_empty());
}

/// Coverage, not a count: a dropped encoder tail ends the sound before the
/// picture, which a frame-count tolerance hides.
#[test]
fn the_audio_track_covers_the_whole_mix_including_the_encoder_tail() {
    let Some(ctx) = context() else { return };
    let seconds = 0.5;
    let (_, audio) = export(ctx, FrameWalk::new(seconds, (30, 1)), seconds, true);

    let mix_samples = (f64::from(MASTER_RATE) * seconds) as u64;
    let written = audio * u64::from(AAC_FRAME_SAMPLES);
    assert!(
        written >= mix_samples,
        "the track covers {written} samples of a {mix_samples}-sample mix"
    );
    assert!(
        written < mix_samples + u64::from(AAC_FRAME_SAMPLES) * 4,
        "the track ran {written} samples past a {mix_samples}-sample mix"
    );
}

/// A silent project must not gain an empty track: the writer drops a track with
/// no config rather than emit one that plays as silence.
#[test]
fn an_export_without_sound_stays_video_only() {
    let Some(ctx) = context() else { return };
    let (bytes, audio) = export(ctx, FrameWalk::new(0.2, (30, 1)), 0.0, false);
    assert_eq!(audio, 0);
    assert!(!bytes.is_empty(), "the video track still has to be written");
}

/// The file has to name an audio track, not just carry frames: without `esds`
/// the config no decoder can start it.
#[test]
fn the_finished_file_declares_its_audio_track() {
    let Some(ctx) = context() else { return };
    let (with_sound, _) = export(ctx, FrameWalk::new(0.5, (30, 1)), 0.5, true);
    let (silent, _) = export(ctx, FrameWalk::new(0.5, (30, 1)), 0.0, false);

    assert!(
        contains(&with_sound, b"mp4a"),
        "no audio sample entry in the file"
    );
    assert!(contains(&with_sound, b"esds"), "no decoder config");
    assert!(
        !contains(&silent, b"mp4a"),
        "a silent export grew an audio track"
    );
    assert!(with_sound.len() > silent.len(), "the sound added no bytes");
}

/// `Mixer::render_into` continues from its own cursor, so a mixer that has
/// already been read (a loudness pass, a preview) would encode from the middle.
#[test]
fn a_mixer_that_was_already_rendered_still_encodes_from_the_start() {
    let Some(ctx) = context() else { return };
    let seconds = 0.4;
    let walk = FrameWalk::new(seconds, (30, 1));

    let fresh = export_with(ctx, walk, mixer(seconds));
    let mut used = mixer(seconds);
    let mut scratch = vec![0.0f32; MASTER_RATE as usize / 4 * MASTER_CHANNELS];
    used.render_into(&mut scratch);
    let after_use = export_with(ctx, walk, used);

    assert_eq!(
        fresh, after_use,
        "the encoded audio depends on where the mixer had been read to"
    );
}

/// A second pass would append a second run of AAC frames into the same track,
/// so the file would carry the mix twice at the wrong offsets.
#[test]
fn writing_the_audio_track_twice_is_refused() {
    let Some(ctx) = context() else { return };
    let walk = FrameWalk::new(0.2, (30, 1));
    let mut session = session(ctx);
    let size = RenderSource::output_size(&session);
    let mut sink = Mp4Sink::new(
        size.width,
        size.height,
        walk,
        4_000_000,
        SourceColor::default(),
    )
    .expect("an H.264 encoder");
    FrameLoop::new()
        .run(
            &mut session,
            &mut Flat::new(),
            walk,
            ctx.device(),
            ctx.queue(),
            |index, rgba| sink.push(index, rgba),
        )
        .expect("rendered");

    sink.push_audio(&mut mixer(0.2), 128_000)
        .expect("first pass");
    let again = sink.push_audio(&mut mixer(0.2), 128_000);
    assert!(again.is_err(), "a second audio pass was accepted");
}

/// D-7: nothing else compares the two tracks, so a file whose sound is half the
/// length of its picture passes every other test here.
#[test]
fn the_audio_and_the_video_cover_the_same_span() {
    let Some(ctx) = context() else { return };
    let seconds = 0.5;
    let walk = FrameWalk::new(seconds, (30, 1));
    let (_, audio) = export(ctx, walk, seconds, true);

    let audio_seconds = audio as f64 * f64::from(AAC_FRAME_SAMPLES) / f64::from(MASTER_RATE);
    let video_seconds = walk.len() as f64 * f64::from(walk.fps().1) / f64::from(walk.fps().0);
    assert!(
        (audio_seconds - video_seconds).abs() < 0.1,
        "sound runs {audio_seconds:.3}s against {video_seconds:.3}s of picture"
    );
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}
