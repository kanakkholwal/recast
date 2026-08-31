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

use recast_compositor::{RenderSource, Session, SourceColor, SourceGeometry};
use recast_export::{FrameLoop, FrameWalk, Mp4Sink, VideoPictures};
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
}

/// Renders `input` under `state` to `output` at `fps`, returning the frames
/// written. Writes the scene's audio too, when it has any.
#[cfg(windows)]
pub fn export_video(
    state: &RenderState,
    input: &Path,
    output: &Path,
    fps: (u32, u32),
    bitrate: u32,
) -> Result<u64, EngineExportError> {
    let ctx = shared_context()?;

    // The recording decides the geometry; the state says what to do with it.
    let mut pictures = VideoPictures::open(input, SourceColor::default()).map_err(|e| {
        EngineExportError::OpenInput {
            path: input.to_path_buf(),
            message: e.to_string(),
        }
    })?;
    let source = SourceGeometry {
        width: pictures.width(),
        height: pictures.height(),
    };

    let mut session = Session::new(ctx, to_scene(state), source)
        .map_err(|e| EngineExportError::Session(e.to_string()))?;
    let walk = FrameWalk::new(RenderSource::output_duration(&session), fps);
    if walk.is_empty() {
        return Err(EngineExportError::Empty);
    }

    let size = RenderSource::output_size(&session);
    let mut sink = Mp4Sink::new(
        size.width,
        size.height,
        walk,
        bitrate,
        SourceColor::default(),
    )
    .map_err(|e| EngineExportError::Encode(e.to_string()))?;

    FrameLoop::new()
        .run(
            &mut session,
            &mut pictures,
            walk,
            ctx.device(),
            ctx.queue(),
            |index, rgba| sink.push(index, rgba),
        )
        .map_err(|e| EngineExportError::Render(e.to_string()))?;

    // After the video: the writer needs the track format before `finish`.
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
        let frames =
            export_video(&state, &input, &output, (30, 1), 4_000_000).expect("the export runs");
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
        export_video(&state, &input, &output, (30, 1), 4_000_000).expect("the export runs");

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
        export_video(&state, &input, &output, (30, 1), 4_000_000).expect("the export runs");

        let bytes = std::fs::read(&output).expect("read back");
        assert!(
            contains(&bytes, b"mp4a") && contains(&bytes, b"esds"),
            "the export has no audio track"
        );
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn a_missing_recording_is_refused_before_the_gpu_is_touched() {
        let state = RenderState::default();
        let missing = std::env::temp_dir().join("recast-engine-does-not-exist.mp4");
        let error = export_video(&state, &missing, &missing, (30, 1), 1_000_000)
            .err()
            .expect("a missing input cannot export");
        assert!(
            matches!(error, EngineExportError::OpenInput { .. })
                || matches!(error, EngineExportError::NoAdapter(_)),
            "{error}"
        );
    }
}
