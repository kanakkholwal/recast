//! Export through the engine rather than the FFmpeg filter graph. Off by
//! default: the graph runs every export until this one has proven parity.

use std::path::{Path, PathBuf};

/// Env switch for the engine export path.
const ENGINE_EXPORT_ENV: &str = "RECAST_ENGINE_EXPORT";

/// Whether an export should render through the engine. Only an explicit `1`:
/// with no proven parity this is a developer switch, not a user's choice.
#[must_use]
pub fn enabled() -> bool {
    matches!(std::env::var(ENGINE_EXPORT_ENV).as_deref(), Ok("1"))
}

/// Bits per pixel per frame for H.264. 0.08 is the usual "visually clean screen
/// content" figure; the engine encoder takes a rate where FFmpeg took a CRF.
const BITS_PER_PIXEL: f64 = 0.08;

/// A video bitrate for `width` x `height` at `fps`, clamped to a sane band.
#[must_use]
pub fn bitrate_for(width: u32, height: u32, fps: f64) -> u32 {
    let pixels = f64::from(width) * f64::from(height);
    let rate = pixels * fps.max(1.0) * BITS_PER_PIXEL;
    // Floors a thumbnail off 0 and stops a 4K120 request asking for 200 Mbps.
    rate.clamp(1_000_000.0, 60_000_000.0) as u32
}

/// AAC bitrate for the exported track. 128 kbps stereo is transparent for
/// screen-recording audio and is a rate the Microsoft encoder actually accepts.
const AUDIO_BITRATE: u32 = 128_000;

/// The export worker's device, built once: enumeration plus device creation is
/// tens of milliseconds a job, and a queue must not pay it per job.
fn shared_context() -> Result<&'static GpuContext, EngineExportError> {
    static CONTEXT: std::sync::OnceLock<Result<GpuContext, String>> = std::sync::OnceLock::new();
    CONTEXT
        .get_or_init(|| {
            GpuContext::new_blocking(GpuOptions {
                require_hardware: false,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| EngineExportError::NoAdapter(e.clone()))
}

use recast_captions::CaptionTrack;
use recast_compositor::{canvas_geometry, RenderSource, Session, SourceColor, SourceGeometry};
use recast_export::{FrameLoop, FrameWalk, Mp4Error, Mp4Sink, VideoPictures};
use recast_gpu::{GpuContext, GpuOptions};
use recast_scene::migrate::to_scene;
use recast_scene::v1::RenderState;

#[derive(Debug, thiserror::Error)]
pub enum EngineExportError {
    #[error("no GPU adapter for the export engine: {0}")]
    NoAdapter(String),
    #[error("building the render session: {0}")]
    Session(String),
    #[error("the scene has no duration to render")]
    Empty,
    #[error("rendering: {0}")]
    Render(String),
    #[error("writing {path}: {error}")]
    Write {
        path: PathBuf,
        error: std::io::Error,
    },
    #[error("{0}")]
    Encode(String),
    #[error("opening {path} to read from: {message}")]
    OpenInput { path: PathBuf, message: String },
    #[error("encoding the audio track: {0}")]
    Audio(String),
    #[error("the export was cancelled")]
    Cancelled,
}

/// What the caller wants after a progress tick. The engine export has no
/// process to kill, so cancelling is an answer rather than a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Cancel,
}

/// Why the frame loop stopped writing. Cancellation travels as a sink error
/// because that is the only way out of the loop that unwinds nothing.
#[derive(Debug, thiserror::Error)]
enum SinkStop {
    #[error(transparent)]
    Encode(#[from] Mp4Error),
    #[error("cancelled")]
    Cancelled,
}

/// What burning captions needs: the words, and a face for a family the system
/// cannot resolve on its own (a downloaded Google font).
#[derive(Debug, Clone, Default)]
pub struct CaptionBurnIn {
    pub track: CaptionTrack,
    pub font: Option<PathBuf>,
}

/// The track the export should burn, or `None` when captions are off, the
/// project has no style, or the transcript is absent or empty.
#[must_use]
pub fn burn_in_for(state: &RenderState, burn: bool) -> Option<CaptionTrack> {
    if !burn || !state.caption_style.as_ref().is_some_and(|s| s.enabled) {
        return None;
    }
    let value = state
        .passthrough
        .get("transcript")
        .filter(|v| !v.is_null())?;
    let track: CaptionTrack = serde_json::from_value(value.clone()).ok()?;
    (!track.is_empty()).then_some(track)
}

/// One engine export, beyond the scene itself.
pub struct ExportSpec<'a> {
    pub input: &'a Path,
    pub output: &'a Path,
    pub fps: (u32, u32),
    /// `None` derives it from the size actually rendered, which is the only
    /// place that knows what the quality cap did.
    pub bitrate: Option<u32>,
    /// The quality profile's ceiling on the output canvas. Only ever shrinks.
    pub max_size: Option<(u32, u32)>,
    pub captions: Option<&'a CaptionBurnIn>,
    /// Whether to write the scene's audio. Off when the file is an intermediate
    /// for the mux pass, which owns the music clips and the voice detach.
    pub audio: bool,
}

/// The scale that fits `canvas` inside `max`, never enlarging.
#[must_use]
pub fn fit_scale(canvas: (u32, u32), max: (u32, u32)) -> f64 {
    let x = f64::from(max.0) / f64::from(canvas.0.max(1));
    let y = f64::from(max.1) / f64::from(canvas.1.max(1));
    x.min(y).min(1.0)
}

/// `source` scaled by `k`, snapped to even dimensions.
///
/// The canvas is derived from the source, and padding is a percentage of it, so
/// scaling the source scales the whole composition. Everything is then drawn AT
/// the output size rather than downsampled after the fact.
#[must_use]
pub fn scaled_source(source: SourceGeometry, k: f64) -> SourceGeometry {
    let even = |v: u32| -> u32 {
        let scaled = (f64::from(v) * k).round() as u32;
        (scaled / 2 * 2).max(2)
    };
    if k >= 1.0 {
        return source;
    }
    SourceGeometry {
        width: even(source.width),
        height: even(source.height),
    }
}

/// Renders `state` through the engine, returning the frames written. Writes the
/// scene's audio too, when it has any.
#[cfg(windows)]
pub fn export_video(
    state: &RenderState,
    spec: &ExportSpec<'_>,
    progress: &mut dyn FnMut(u64, u64) -> Flow,
) -> Result<u64, EngineExportError> {
    let (input, output, fps) = (spec.input, spec.output, spec.fps);
    let captions = spec.captions;
    let ctx = shared_context()?;

    // The recording decides the geometry; the state says what to do with it.
    let mut pictures = VideoPictures::open(input, SourceColor::default()).map_err(|e| {
        EngineExportError::OpenInput {
            path: input.to_path_buf(),
            message: e.to_string(),
        }
    })?;
    let native = SourceGeometry {
        width: pictures.width(),
        height: pictures.height(),
    };
    let natural = canvas_geometry(
        native.width,
        native.height,
        state.padding,
        state.output_aspect.as_deref(),
    );
    let source = match spec.max_size {
        Some(max) => scaled_source(native, fit_scale((natural.canvas_w, natural.canvas_h), max)),
        None => native,
    };

    let mut session = Session::new(ctx, to_scene(state), source)
        .map_err(|e| EngineExportError::Session(e.to_string()))?;
    if let Some(captions) = captions {
        // A caption-less export is worth more than none: a missing face degrades to the system match.
        if let Some(path) = &captions.font {
            match std::fs::read(path) {
                Ok(bytes) => {
                    if !session.set_caption_font(bytes, 0) {
                        log::warn!("engine export: {} is not a readable face", path.display());
                    }
                }
                Err(e) => log::warn!("engine export: reading {}: {e}", path.display()),
            }
        }
        session.set_caption_track(Some(captions.track.clone()));
    }
    let walk = FrameWalk::new(RenderSource::output_duration(&session), fps);
    if walk.is_empty() {
        return Err(EngineExportError::Empty);
    }

    let size = RenderSource::output_size(&session);
    let bitrate = spec.bitrate.unwrap_or_else(|| {
        bitrate_for(
            size.width,
            size.height,
            f64::from(fps.0) / f64::from(fps.1.max(1)),
        )
    });
    let mut sink = Mp4Sink::new(
        size.width,
        size.height,
        walk,
        bitrate,
        SourceColor::default(),
    )
    .map_err(|e| EngineExportError::Encode(e.to_string()))?;

    let total = walk.len();
    FrameLoop::new()
        .run(
            &mut session,
            &mut pictures,
            walk,
            ctx.device(),
            ctx.queue(),
            |index, rgba| {
                sink.push(index, rgba)?;
                match progress(index + 1, total) {
                    Flow::Continue => Ok(()),
                    Flow::Cancel => Err(SinkStop::Cancelled),
                }
            },
        )
        .map_err(|e| match e {
            recast_export::RenderError::Sink {
                error: SinkStop::Cancelled,
                ..
            } => EngineExportError::Cancelled,
            other => EngineExportError::Render(other.to_string()),
        })?;

    // Skipped whole when off: decoding the sources is most of an audio pass's cost.
    if spec.audio {
        let scene = to_scene(state);
        let mut mixer = recast_audio::mixer_for(
            &scene.audio,
            RenderSource::output_duration(&session),
            crate::export_audio::sources_for(&scene.audio, input),
        );
        if mixer.total_frames() > 0 {
            sink.push_audio(&mut mixer, AUDIO_BITRATE)
                .map_err(|e| EngineExportError::Audio(e.to_string()))?;
        }
    }

    let bytes = sink
        .finish()
        .map_err(|e| EngineExportError::Encode(e.to_string()))?;
    std::fs::write(output, &bytes).map_err(|error| EngineExportError::Write {
        path: output.to_path_buf(),
        error,
    })?;
    Ok(walk.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_engine_path_is_off_unless_explicitly_asked_for() {
        // Anything but "1" leaves the graph in charge.
        assert!(!enabled() || std::env::var(ENGINE_EXPORT_ENV).as_deref() == Ok("1"));
    }

    fn caption_state(
        style: Option<recast_captions::CaptionStyle>,
        transcript: &str,
    ) -> RenderState {
        let mut state = RenderState {
            caption_style: style,
            ..Default::default()
        };
        if !transcript.is_empty() {
            state.passthrough.insert(
                "transcript".into(),
                serde_json::from_str(transcript).expect("transcript"),
            );
        }
        state
    }

    const TRANSCRIPT: &str = r#"{"segments":[{"id":"s0","start":0.0,"end":1.0,"text":"hi",
        "words":[{"start":0.0,"end":0.5,"text":"hi"}]}]}"#;

    #[test]
    fn a_project_with_captions_burns_them_when_asked() {
        let state = caption_state(Some(recast_captions::CaptionStyle::default()), TRANSCRIPT);
        let track = burn_in_for(&state, true).expect("a track to burn");
        assert_eq!(track.segments.len(), 1);
    }

    /// Every reason not to burn, each of which used to be the same silent
    /// nothing because the engine never read the transcript at all.
    #[test]
    fn nothing_is_burned_when_the_export_did_not_ask_or_the_project_cannot() {
        let style = recast_captions::CaptionStyle::default();
        let disabled = recast_captions::CaptionStyle {
            enabled: false,
            ..style.clone()
        };
        for (name, state, burn) in [
            (
                "the export did not ask",
                caption_state(Some(style.clone()), TRANSCRIPT),
                false,
            ),
            (
                "the project has no caption style",
                caption_state(None, TRANSCRIPT),
                true,
            ),
            (
                "the style is switched off",
                caption_state(Some(disabled), TRANSCRIPT),
                true,
            ),
            (
                "there is no transcript",
                caption_state(Some(style.clone()), ""),
                true,
            ),
            (
                "the transcript has no words",
                caption_state(Some(style), r#"{"segments":[]}"#),
                true,
            ),
        ] {
            assert!(burn_in_for(&state, burn).is_none(), "burned anyway: {name}");
        }
    }

    #[test]
    fn a_quality_cap_shrinks_the_composition_and_never_enlarges_it() {
        assert_eq!(fit_scale((1920, 1080), (1280, 720)), 720.0 / 1080.0);
        // The tighter of the two axes wins, so a tall canvas is not stretched sideways.
        assert_eq!(fit_scale((1080, 1920), (1280, 720)), 720.0 / 1920.0);
        assert_eq!(fit_scale((640, 360), (1280, 720)), 1.0);
    }

    #[test]
    fn a_capped_source_stays_even_and_keeps_its_shape() {
        let source = SourceGeometry {
            width: 1920,
            height: 1080,
        };
        let scaled = scaled_source(source, 720.0 / 1080.0);
        assert_eq!((scaled.width, scaled.height), (1280, 720));
        assert_eq!(scaled.width % 2, 0);
        assert_eq!(scaled.height % 2, 0);
    }

    /// A cap wider than the composition must leave the source untouched, or
    /// every "Source quality" export would be resampled for nothing.
    #[test]
    fn an_uncapped_export_renders_at_its_own_size() {
        let source = SourceGeometry {
            width: 1921,
            height: 1081,
        };
        let scaled = scaled_source(source, 1.0);
        assert_eq!((scaled.width, scaled.height), (1921, 1081));
    }

    /// A cap so small it would round a dimension to zero still has to produce a
    /// frame an encoder will take.
    #[test]
    fn an_absurd_cap_still_leaves_an_encodable_frame() {
        let scaled = scaled_source(
            SourceGeometry {
                width: 1920,
                height: 1080,
            },
            0.0001,
        );
        assert!(scaled.width >= 2 && scaled.height >= 2);
    }

    #[test]
    fn the_bitrate_scales_with_pixels_and_rate() {
        let hd = bitrate_for(1920, 1080, 30.0);
        assert!(bitrate_for(1280, 720, 30.0) < hd);
        assert!(hd < bitrate_for(1920, 1080, 60.0));
    }

    /// A thumbnail-sized canvas must not ask for a bitrate no encoder accepts,
    /// and a 4K120 request must not ask for hundreds of megabits.
    #[test]
    fn the_bitrate_stays_inside_a_band_an_encoder_will_take() {
        assert_eq!(bitrate_for(16, 16, 1.0), 1_000_000);
        assert_eq!(bitrate_for(7680, 4320, 120.0), 60_000_000);
    }

    #[test]
    fn a_zero_frame_rate_does_not_collapse_the_bitrate() {
        assert!(bitrate_for(1920, 1080, 0.0) >= 1_000_000);
    }
}

#[cfg(all(test, windows))]
mod live {
    use recast_compositor::{PlaneData, PlaneLayout, SourcePlanes};
    use recast_export::{FrameLoop, FrameWalk, Mp4Sink, PictureSource};
    use recast_gpu::{GpuContext, GpuOptions};

    use super::*;

    const SRC_W: u32 = 640;
    const SRC_H: u32 = 360;

    /// Removes itself. The audit found 164 MB of directories left by tests that
    /// skipped this.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("recast-engine-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("scratch dir");
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The plain 30 fps export every live test runs, at a fixed bitrate so the
    /// encoder is not a variable.
    fn spec<'a>(input: &'a Path, output: &'a Path) -> ExportSpec<'a> {
        ExportSpec {
            input,
            output,
            fps: (30, 1),
            bitrate: Some(4_000_000),
            max_size: None,
            captions: None,
            audio: true,
        }
    }

    /// Progress a test does not care about; never asks to stop.
    fn never_cancels(_done: u64, _total: u64) -> Flow {
        Flow::Continue
    }

    struct Bright(Vec<u8>);

    impl PictureSource for Bright {
        type Error = std::convert::Infallible;

        fn picture_at(&mut self, _t: f64) -> Result<Option<SourcePlanes<'_>>, Self::Error> {
            Ok(Some(SourcePlanes {
                width: SRC_W,
                height: SRC_H,
                layout: PlaneLayout::Nv12,
                color: SourceColor::default(),
                data: PlaneData::Packed(&self.0),
            }))
        }
    }

    /// Writes a recording for the export to consume, so the test drives the
    /// real decoder rather than a synthetic source.
    fn record(ctx: &GpuContext, path: &Path, seconds: f64) {
        let state = RenderState {
            trim_start: 0.0,
            trim_end: seconds,
            cursor_enabled: false,
            ..Default::default()
        };
        let mut session = Session::new(
            ctx,
            to_scene(&state),
            SourceGeometry {
                width: SRC_W,
                height: SRC_H,
            },
        )
        .expect("session");
        let size = RenderSource::output_size(&session);
        let walk = FrameWalk::new(seconds, (30, 1));
        let mut sink = Mp4Sink::new(
            size.width,
            size.height,
            walk,
            6_000_000,
            SourceColor::default(),
        )
        .expect("an H.264 encoder");

        let mut bytes = vec![220u8; (SRC_W * SRC_H) as usize];
        bytes.resize(PlaneLayout::Nv12.packed_len(SRC_W, SRC_H), 128);
        FrameLoop::new()
            .run(
                &mut session,
                &mut Bright(bytes),
                walk,
                ctx.device(),
                ctx.queue(),
                |index, rgba| sink.push(index, rgba),
            )
            .expect("rendered");
        std::fs::write(path, sink.finish().expect("finished")).expect("write");
    }

    /// A recording that carries sound, so the export has audio to preserve.
    fn record_with_sound(ctx: &GpuContext, path: &Path, seconds: f64) {
        use recast_audio::{Master, Mixer, Samples, Track, MASTER_CHANNELS, MASTER_RATE};

        let state = RenderState {
            trim_start: 0.0,
            trim_end: seconds,
            cursor_enabled: false,
            ..Default::default()
        };
        let mut session = Session::new(
            ctx,
            to_scene(&state),
            SourceGeometry {
                width: SRC_W,
                height: SRC_H,
            },
        )
        .expect("session");
        let size = RenderSource::output_size(&session);
        let walk = FrameWalk::new(seconds, (30, 1));
        let mut sink = Mp4Sink::new(
            size.width,
            size.height,
            walk,
            6_000_000,
            SourceColor::default(),
        )
        .expect("an H.264 encoder");

        let mut bytes = vec![220u8; (SRC_W * SRC_H) as usize];
        bytes.resize(PlaneLayout::Nv12.packed_len(SRC_W, SRC_H), 128);
        FrameLoop::new()
            .run(
                &mut session,
                &mut Bright(bytes),
                walk,
                ctx.device(),
                ctx.queue(),
                |index, rgba| sink.push(index, rgba),
            )
            .expect("rendered");

        let frames = (f64::from(MASTER_RATE) * seconds) as usize;
        let mut pcm = vec![0.0f32; frames * MASTER_CHANNELS];
        for frame in 0..frames {
            let t = frame as f64 / f64::from(MASTER_RATE);
            let value = (t * 440.0 * std::f64::consts::TAU).sin() as f32 * 0.5;
            for channel in 0..MASTER_CHANNELS {
                pcm[frame * MASTER_CHANNELS + channel] = value;
            }
        }
        let mut mixer = Mixer::new(Master::new(seconds));
        mixer.push(Track::new(Box::new(Samples::new(
            pcm,
            MASTER_RATE,
            MASTER_CHANNELS as u16,
        ))));
        sink.push_audio(&mut mixer, 128_000).expect("audio encodes");
        std::fs::write(path, sink.finish().expect("finished")).expect("write");
    }

    fn context() -> Option<GpuContext> {
        GpuContext::new_blocking(GpuOptions {
            require_hardware: false,
            ..Default::default()
        })
        .ok()
    }

    /// End to end through the shipping binary's own module: a real recording in,
    /// a real file out, decoded by a reader that never saw the render path.
    #[test]
    fn an_engine_export_reads_a_recording_and_writes_a_playable_file() {
        let Some(ctx) = context() else { return };
        let scratch = Scratch::new("roundtrip");
        let input = scratch.0.join("in.mp4");
        let output = scratch.0.join("out.mp4");
        record(&ctx, &input, 0.5);

        let state = RenderState {
            trim_start: 0.0,
            trim_end: 0.5,
            cursor_enabled: false,
            ..Default::default()
        };
        let frames = export_video(&state, &spec(&input, &output), &mut never_cancels)
            .expect("the export runs");
        assert_eq!(frames, FrameWalk::new(0.5, (30, 1)).len());

        let mut reader = recast_codec_mf::VideoReader::open(&output).expect("the export opens");
        let mut decoded = 0u64;
        while reader.next_frame().expect("decode").is_some() {
            decoded += 1;
        }
        assert_eq!(decoded, frames, "frames went missing between the two files");
    }

    /// The recording has to reach the canvas, not just the frame count.
    #[test]
    fn the_recording_reaches_the_exported_picture() {
        let Some(ctx) = context() else { return };
        let scratch = Scratch::new("pixels");
        let input = scratch.0.join("in.mp4");
        let output = scratch.0.join("out.mp4");
        record(&ctx, &input, 0.3);

        let state = RenderState {
            trim_start: 0.0,
            trim_end: 0.3,
            cursor_enabled: false,
            ..Default::default()
        };
        export_video(&state, &spec(&input, &output), &mut never_cancels).expect("the export runs");

        let mut reader = recast_codec_mf::VideoReader::open(&output).expect("opens");
        let frame = reader.next_frame().expect("decode").expect("a frame");
        let info = reader.info();
        let luma = (info.width * info.height) as usize;
        let plane = &frame.data[..luma.min(frame.data.len())];
        let mean = plane.iter().map(|&b| f64::from(b)).sum::<f64>() / plane.len() as f64;
        assert!(mean > 60.0, "the exported frame is nearly black: {mean}");
    }

    /// B-2: an engine export used to produce a silent file. The recording's own
    /// audio has to survive into the export.
    #[test]
    fn the_recordings_audio_survives_into_the_exported_file() {
        let Some(ctx) = context() else { return };
        let scratch = Scratch::new("audio");
        let input = scratch.0.join("in.mp4");
        let output = scratch.0.join("out.mp4");
        record_with_sound(&ctx, &input, 0.5);

        let state = RenderState {
            trim_start: 0.0,
            trim_end: 0.5,
            cursor_enabled: false,
            ..Default::default()
        };
        export_video(&state, &spec(&input, &output), &mut never_cancels).expect("the export runs");

        let bytes = std::fs::read(&output).expect("read back");
        assert!(
            contains(&bytes, b"mp4a") && contains(&bytes, b"esds"),
            "the export has no audio track"
        );
    }

    /// Pixels darker than the pill's threshold in the bottom quarter of frame
    /// `index`, where a bottom caption sits. A count, not a mean: the pill
    /// covers a few percent of the band, so its effect on the mean is within
    /// the encoder's rate-control noise between two runs.
    ///
    /// Frame 0 is the wrong frame to ask: the preset's slide entrance starts at
    /// alpha 0, so it carries no caption by design.
    fn dark_pixels_in_caption_band(path: &Path, index: usize) -> usize {
        // The pill is #0b0b12 at 61% over a luma-220 source, so it lands near 93.
        const PILL_MAX_LUMA: u8 = 140;
        let mut reader = recast_codec_mf::VideoReader::open(path).expect("opens");
        let mut frame = reader.next_frame().expect("decode").expect("a frame");
        for _ in 0..index {
            frame = reader.next_frame().expect("decode").expect("a frame");
        }
        let info = reader.info();
        let (w, h) = (info.width as usize, info.height as usize);
        let band = &frame.data[w * (h - h / 4)..w * h];
        band.iter().filter(|&&b| b < PILL_MAX_LUMA).count()
    }

    /// A frame past the caption entrance at 30 fps.
    const ENTRANCE_SETTLED: usize = 6;

    /// B-3: the engine export ignored the transcript entirely, so every export
    /// through it came out caption-less while the graph burned them in.
    #[test]
    fn a_burned_caption_reaches_the_exported_pixels() {
        let Some(ctx) = context() else { return };
        let scratch = Scratch::new("captions");
        let input = scratch.0.join("in.mp4");
        record(&ctx, &input, 0.3);

        // Arial rather than the preset's Inter: a face every test machine resolves without a download.
        let style = recast_captions::CaptionStyle {
            font_family: "Arial".into(),
            font_weight: 400,
            ..recast_captions::CaptionStyle::default()
        };
        let state = RenderState {
            trim_start: 0.0,
            trim_end: 0.3,
            cursor_enabled: false,
            caption_style: Some(style),
            ..Default::default()
        };
        let words = r#"{"segments":[{"id":"s0","start":0.0,"end":0.3,"text":"burned in",
            "words":[{"start":0.0,"end":0.15,"text":"burned"},
                     {"start":0.15,"end":0.3,"text":"in"}]}]}"#;
        let burn = CaptionBurnIn {
            track: serde_json::from_str(words).expect("track"),
            font: None,
        };

        let plain = scratch.0.join("plain.mp4");
        let captioned = scratch.0.join("captioned.mp4");
        export_video(&state, &spec(&input, &plain), &mut never_cancels).expect("the export runs");
        export_video(
            &state,
            &ExportSpec {
                captions: Some(&burn),
                ..spec(&input, &captioned)
            },
            &mut never_cancels,
        )
        .expect("the export runs");

        // Past the 125 ms entrance, so the pill is at full opacity.
        let (before, after) = (
            dark_pixels_in_caption_band(&plain, ENTRANCE_SETTLED),
            dark_pixels_in_caption_band(&captioned, ENTRANCE_SETTLED),
        );
        // A delta, not a floor: the decoder pads 360 rows to 368 and those rows are dark in both.
        assert!(
            after > before + 500,
            "no pill reached the exported pixels: {before} dark without, {after} with"
        );
    }

    /// The engine path had no progress and no cancel, which is why it could not
    /// be a setting: an export ran to completion with a frozen bar.
    #[test]
    fn every_frame_is_reported_and_the_last_tick_is_the_whole_export() {
        let Some(ctx) = context() else { return };
        let scratch = Scratch::new("progress");
        let input = scratch.0.join("in.mp4");
        let output = scratch.0.join("out.mp4");
        record(&ctx, &input, 0.3);

        let state = RenderState {
            trim_start: 0.0,
            trim_end: 0.3,
            cursor_enabled: false,
            ..Default::default()
        };
        let mut ticks: Vec<(u64, u64)> = Vec::new();
        let mut record_tick = |done, total| {
            ticks.push((done, total));
            Flow::Continue
        };
        let frames = export_video(&state, &spec(&input, &output), &mut record_tick)
            .expect("the export runs");

        assert_eq!(ticks.len() as u64, frames, "a frame went unreported");
        assert_eq!(ticks.first(), Some(&(1, frames)), "the first tick is not 1");
        assert_eq!(
            ticks.last(),
            Some(&(frames, frames)),
            "the bar never reaches the end"
        );
    }

    #[test]
    fn cancelling_stops_the_render_and_writes_no_file() {
        let Some(ctx) = context() else { return };
        let scratch = Scratch::new("cancel");
        let input = scratch.0.join("in.mp4");
        let output = scratch.0.join("out.mp4");
        record(&ctx, &input, 0.5);

        let state = RenderState {
            trim_start: 0.0,
            trim_end: 0.5,
            cursor_enabled: false,
            ..Default::default()
        };
        let mut seen = 0u64;
        let mut stop_after_three = |done, _total| {
            seen = done;
            match done >= 3 {
                true => Flow::Cancel,
                false => Flow::Continue,
            }
        };
        let error = export_video(&state, &spec(&input, &output), &mut stop_after_three)
            .expect_err("a cancelled export cannot succeed");

        assert!(matches!(error, EngineExportError::Cancelled), "{error}");
        assert_eq!(seen, 3, "the loop kept rendering past the cancel");
        assert!(!output.exists(), "a cancelled export left a file behind");
    }

    /// B-3: the engine rendered at the composition's own size whatever quality
    /// was asked for, so a "Small" export came out full size at a bitrate
    /// computed for a frame that was never encoded.
    #[test]
    fn the_quality_cap_reaches_the_exported_file() {
        let Some(ctx) = context() else { return };
        let scratch = Scratch::new("quality");
        let input = scratch.0.join("in.mp4");
        let full = scratch.0.join("full.mp4");
        let small = scratch.0.join("small.mp4");
        record(&ctx, &input, 0.2);

        let state = RenderState {
            trim_start: 0.0,
            trim_end: 0.2,
            cursor_enabled: false,
            ..Default::default()
        };
        export_video(&state, &spec(&input, &full), &mut never_cancels).expect("the export runs");
        export_video(
            &state,
            &ExportSpec {
                max_size: Some((320, 180)),
                // Derived, so a cap that shrinks the frame also lowers the rate.
                bitrate: None,
                ..spec(&input, &small)
            },
            &mut never_cancels,
        )
        .expect("the export runs");

        let dims = |path: &Path| {
            let reader = recast_codec_mf::VideoReader::open(path).expect("opens");
            (reader.info().width, reader.info().height)
        };
        let (full_w, _) = dims(&full);
        let (small_w, small_h) = dims(&small);
        assert!(small_w <= 320, "the cap was ignored: {small_w} wide");
        assert!(small_h <= 192, "the cap was ignored: {small_h} tall");
        assert!(
            small_w < full_w,
            "capped and uncapped came out the same size"
        );
        assert!(
            std::fs::metadata(&small).expect("small").len()
                < std::fs::metadata(&full).expect("full").len(),
            "the capped export is not smaller on disk"
        );
    }

    /// The intermediate handed to the mux pass must carry no audio: the mux owns
    /// the music clips and the voice detach, and a second track would double up.
    #[test]
    fn an_intermediate_for_the_mux_pass_carries_no_audio_track() {
        let Some(ctx) = context() else { return };
        let scratch = Scratch::new("silent");
        let input = scratch.0.join("in.mp4");
        let output = scratch.0.join("out.mp4");
        record_with_sound(&ctx, &input, 0.3);

        let state = RenderState {
            trim_start: 0.0,
            trim_end: 0.3,
            cursor_enabled: false,
            ..Default::default()
        };
        export_video(
            &state,
            &ExportSpec {
                audio: false,
                ..spec(&input, &output)
            },
            &mut never_cancels,
        )
        .expect("the export runs");

        let bytes = std::fs::read(&output).expect("read back");
        assert!(
            !contains(&bytes, b"mp4a"),
            "the intermediate carries an audio track the mux would duplicate"
        );
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn a_missing_recording_is_refused_before_the_gpu_is_touched() {
        let state = RenderState::default();
        let missing = std::env::temp_dir().join("recast-engine-does-not-exist.mp4");
        let error = export_video(&state, &spec(&missing, &missing), &mut never_cancels)
            .expect_err("a missing input cannot export");
        assert!(
            matches!(error, EngineExportError::OpenInput { .. })
                || matches!(error, EngineExportError::NoAdapter(_)),
            "{error}"
        );
    }
}
