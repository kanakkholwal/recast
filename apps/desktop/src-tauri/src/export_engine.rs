//! Export through the engine rather than the FFmpeg filter graph. Off by
//! default: the graph runs every export until this one has proven parity.

use std::path::{Path, PathBuf};

/// Forces the engine path on or off regardless of the setting: `1` on, `0` off.
/// Unset defers to the experimental flag the request carries.
const ENGINE_EXPORT_ENV: &str = "RECAST_ENGINE_EXPORT";

/// Whether this export renders through the engine: the env override if one is
/// set, otherwise the request's flag.
///
/// Pure, so the precedence is testable without an app handle or a registry.
#[must_use]
pub fn engine_opt_in(requested: bool, env: Option<&str>) -> bool {
    match env {
        Some("1") => true,
        Some("0") => false,
        _ => requested,
    }
}

/// Whether an export should render through the engine.
#[must_use]
pub fn enabled(requested: bool) -> bool {
    engine_opt_in(requested, std::env::var(ENGINE_EXPORT_ENV).ok().as_deref())
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
use recast_export::{
    FfmpegPictures, FfmpegSink, Frame, FrameLoop, FrameWalk, PixelLayout, SourceInfo,
};
#[cfg(windows)]
use recast_export::{Mp4Error, Mp4Sink, VideoPictures};
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
    /// The scene needs something this path cannot do. The caller falls back to
    /// FFmpeg rather than shipping output that disagrees with the preview.
    #[error("the engine cannot export this scene: {0}")]
    Unsupported(String),
}

/// What an export actually did, so a slow or wrong result on a machine nobody
/// here owns can be diagnosed from its log line rather than guessed at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportReport {
    pub frames: u64,
    /// The codec backend that encoded: in-process, or the bundled FFmpeg.
    pub codec: &'static str,
    /// Where RGBA became NV12. The CPU path is nine times slower at 1080p.
    pub pixels: &'static str,
    pub width: u32,
    pub height: u32,
    /// The recording's own size, which differs when a quality cap shrank it.
    pub source_width: u32,
    pub source_height: u32,
    /// Whether this file carries its audio, or an intermediate the mux completes.
    pub audio: bool,
    pub captions: bool,
}

impl std::fmt::Display for ExportReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} frames {}x{} (source {}x{}) codec={} pixels={} audio={} captions={}",
            self.frames,
            self.width,
            self.height,
            self.source_width,
            self.source_height,
            self.codec,
            self.pixels,
            self.audio,
            self.captions,
        )
    }
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
    #[cfg(windows)]
    #[error(transparent)]
    Encode(#[from] Mp4Error),
    #[error(transparent)]
    Ffmpeg(#[from] recast_export::FfmpegError),
    #[error("cancelled")]
    Cancelled,
}

/// The recording, decoded by whichever backend this export chose.
enum Pictures {
    #[cfg(windows)]
    Native(Box<VideoPictures>),
    Ffmpeg(Box<FfmpegPictures>),
}

impl Pictures {
    fn size(&self) -> SourceGeometry {
        match self {
            #[cfg(windows)]
            Self::Native(p) => SourceGeometry {
                width: p.width(),
                height: p.height(),
            },
            Self::Ffmpeg(p) => SourceGeometry {
                width: p.width(),
                height: p.height(),
            },
        }
    }
}

impl recast_export::PictureSource for Pictures {
    type Error = EngineExportError;

    fn picture_at(
        &mut self,
        source_time: f64,
    ) -> Result<Option<recast_compositor::SourcePlanes<'_>>, Self::Error> {
        match self {
            #[cfg(windows)]
            Self::Native(p) => p
                .picture_at(source_time)
                .map_err(|e| EngineExportError::Render(e.to_string())),
            Self::Ffmpeg(p) => p
                .picture_at(source_time)
                .map_err(|e| EngineExportError::Render(e.to_string())),
        }
    }
}

/// The codec backend writing the rendered frames. Media Foundation encodes in
/// process where it exists; everywhere else the bundled FFmpeg does, which is
/// the same split the browser renderer already ships on.
enum Sink {
    #[cfg(windows)]
    Native(Box<Mp4Sink>),
    Ffmpeg(Box<FfmpegSink>),
}

// The FFmpeg arms ignore what the native encoder needs: a frame index, a mixer and a destination path.
#[cfg_attr(not(windows), allow(unused_variables))]
impl Sink {
    fn push(&mut self, index: u64, frame: Frame<'_>) -> Result<(), SinkStop> {
        match self {
            #[cfg(windows)]
            Self::Native(sink) => Ok(sink.push(index, frame)?),
            Self::Ffmpeg(sink) => Ok(sink.push(frame)?),
        }
    }

    /// Which backend this is, for the export's own log line.
    const fn name(&self) -> &'static str {
        match self {
            #[cfg(windows)]
            Self::Native(_) => "media-foundation",
            Self::Ffmpeg(_) => "ffmpeg",
        }
    }

    /// Whether this backend can write the audio track itself. FFmpeg cannot
    /// here: its input pipe is already carrying the video.
    const fn writes_audio(&self) -> bool {
        match self {
            #[cfg(windows)]
            Self::Native(_) => true,
            Self::Ffmpeg(_) => false,
        }
    }

    /// Writes the mixed track. Only reached on a backend that answers
    /// `writes_audio`, so the other arm is unreachable rather than silent.
    fn push_audio(
        &mut self,
        mixer: &mut recast_audio::Mixer,
        bitrate: u32,
    ) -> Result<(), EngineExportError> {
        match self {
            #[cfg(windows)]
            Self::Native(sink) => sink
                .push_audio(mixer, bitrate)
                .map_err(|e| EngineExportError::Audio(e.to_string())),
            Self::Ffmpeg(_) => Err(EngineExportError::Audio(
                "the FFmpeg backend writes video only; the mux pass owns the audio".into(),
            )),
        }
    }

    fn finish(self, output: &Path) -> Result<(), EngineExportError> {
        match self {
            #[cfg(windows)]
            Self::Native(sink) => {
                // Written through a `.part` and renamed: a plain write leaves a truncated mp4 under the user's chosen name if the disk fills or the app dies mid-write.
                let tmp = output.with_extension("mp4.part");
                let failed = |error: std::io::Error| EngineExportError::Write {
                    path: output.to_path_buf(),
                    error,
                };
                let file = std::fs::File::create(&tmp).map_err(failed)?;
                let mut out = std::io::BufWriter::new(file);
                // Streamed rather than returned: a 30-minute export is gigabytes, and holding the finished file alongside its own samples doubled that.
                sink.finish_into(&mut out)
                    .map_err(|e| EngineExportError::Encode(e.to_string()))?;
                std::io::Write::flush(&mut out).map_err(failed)?;
                drop(out);
                std::fs::rename(&tmp, output).map_err(failed)
            }
            Self::Ffmpeg(sink) => sink
                .finish()
                .map_err(|e| EngineExportError::Encode(e.to_string())),
        }
    }
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

/// Every image annotation this scene names, decoded and uploaded once.
///
/// One that will not decode is skipped rather than fatal, which is what the
/// FFmpeg graph does: a single unreadable overlay must not lose a good export.
fn upload_annotation_images(
    ctx: &recast_gpu::GpuContext,
    state: &RenderState,
) -> Vec<(String, wgpu::TextureView)> {
    let mut uploaded: Vec<(String, wgpu::TextureView)> = Vec::new();
    for annotation in &state.annotations {
        let recast_scene::v1::nodes::AnnotationKind::Image { path, .. } = &annotation.kind else {
            continue;
        };
        if path.is_empty() || uploaded.iter().any(|(seen, _)| seen == path) {
            continue;
        }
        match upload_background(ctx, Path::new(path)) {
            Ok((texture, _, _)) => uploaded.push((
                path.clone(),
                texture.create_view(&wgpu::TextureViewDescriptor::default()),
            )),
            Err(e) => log::warn!("engine export: image annotation {path}: {e}"),
        }
    }
    uploaded
}

/// The wallpaper or image file this scene's background names, if it names one.
fn background_image(state: &RenderState) -> Option<PathBuf> {
    matches!(state.background_type.as_str(), "image" | "wallpaper")
        .then(|| PathBuf::from(&state.background_value))
        .filter(|path| path.exists())
}

/// Decodes the background once and uploads it. Static, so it is bound to every
/// frame rather than re-uploaded per frame.
fn upload_background(
    ctx: &recast_gpu::GpuContext,
    path: &Path,
) -> Result<(wgpu::Texture, u32, u32), EngineExportError> {
    let decoded = image::open(path)
        .map_err(|e| EngineExportError::Unsupported(format!("{}: {e}", path.display())))?
        .to_rgba8();
    let (width, height) = (decoded.width().max(1), decoded.height().max(1));
    let texture = ctx.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("engine-background"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    ctx.queue().write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &decoded,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    Ok((texture, width, height))
}

/// The recording's cursor samples, as `write_cursor_track` wrote them.
fn load_cursor_track(path: &Path) -> Result<recast_cursor::CursorTrack, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
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
    /// for the mux pass, which owns the music clips and the voice detach, and
    /// ignored by a backend that cannot write audio at all.
    pub audio: bool,
    /// The recording as the caller already probed it. Used where the decoder
    /// cannot report its own geometry, which is every platform but Windows.
    pub source: SourceInfo,
    /// The bundled FFmpeg, required wherever it is the codec backend.
    pub ffmpeg: Option<&'a Path>,
    /// The recording's captured tracks. A project keeps the microphone and
    /// system audio in their own files, which the video alone does not carry.
    pub audio_sources: crate::export_audio::RecordingAudio<'a>,
    /// Take the FFmpeg backend even where an in-process one exists. Platforms
    /// without one always take it; this is how it is exercised on the one that
    /// has one, and it is the fallback when that encoder refuses a size.
    pub force_ffmpeg: bool,
    /// The editor's own time map. `None` derives it from the scene, which the
    /// preview does too; passing the editor's keeps video and audio on the one
    /// timeline instead of two independently derived ones.
    pub time_map: Option<recast_time::TimeMap>,
    /// The recording's cursor samples. The compositor draws the pointer from
    /// these alone, so without them a cursor-enabled export renders none.
    pub cursor_track: Option<&'a Path>,
    /// The camera recording and how far it lags the screen, in seconds. The
    /// bubble is a separate file composited at export, not part of the capture.
    pub camera: Option<(&'a Path, f64)>,
}

/// Whether the engine writes a finished file on this platform, or an
/// intermediate the mux pass still has to give an audio track and a container.
#[must_use]
pub const fn writes_finished_files() -> bool {
    cfg!(windows)
}

/// The scale that fits `canvas` inside `max`, never enlarging.
#[must_use]
pub fn fit_scale(canvas: (u32, u32), max: (u32, u32)) -> f64 {
    let x = f64::from(max.0) / f64::from(canvas.0.max(1));
    let y = f64::from(max.1) / f64::from(canvas.1.max(1));
    x.min(y).min(1.0)
}

/// `source` scaled by `k`, snapped to even dimensions.
/// The canvas derives from the source and padding is a percentage of it, so scaling the source scales the whole composition, drawn AT output size rather than downsampled after.
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

/// What the loop will hand the sink. NV12 wherever the GPU pass can pack the
/// shape, which is nine times faster than converting the readback on the CPU.
fn pixel_layout(width: u32, height: u32) -> PixelLayout {
    match recast_export::GpuNv12::handles(width, height) {
        true => PixelLayout::Nv12,
        false => PixelLayout::Rgba,
    }
}

/// The recording, opened through the backend this export chose.
fn open_pictures(spec: &ExportSpec<'_>) -> Result<Pictures, EngineExportError> {
    open_video(spec, spec.input)
}

/// `path` through the same backend, so the camera decodes the way the screen does.
fn open_video(spec: &ExportSpec<'_>, path: &Path) -> Result<Pictures, EngineExportError> {
    let refuse = |message: String| EngineExportError::OpenInput {
        path: path.to_path_buf(),
        message,
    };
    #[cfg(windows)]
    if !spec.force_ffmpeg {
        let reader =
            VideoPictures::open(path, SourceColor::default()).map_err(|e| refuse(e.to_string()))?;
        return Ok(Pictures::Native(Box::new(reader)));
    }
    let program = spec.ffmpeg.ok_or_else(|| {
        refuse("no FFmpeg was given, and this platform has no in-process decoder".into())
    })?;
    let reader = FfmpegPictures::open(program, path, spec.source, SourceColor::default())
        .map_err(|e| refuse(e.to_string()))?;
    Ok(Pictures::Ffmpeg(Box::new(reader)))
}

/// The encoder for a canvas of `width` by `height`.
fn open_sink(
    spec: &ExportSpec<'_>,
    width: u32,
    height: u32,
    walk: FrameWalk,
    bitrate: u32,
) -> Result<Sink, EngineExportError> {
    #[cfg(windows)]
    if !spec.force_ffmpeg {
        let mut sink = Mp4Sink::new(width, height, walk, bitrate, SourceColor::default())
            .map_err(|e| EngineExportError::Encode(e.to_string()))?;
        // Samples wait on disk, not in RAM: 30 minutes of 1080p is gigabytes of them, and they are only read back once, in order.
        if let Err(e) = sink.spill_to(&std::env::temp_dir()) {
            log::warn!("engine export: samples stay in memory ({e})");
        }
        return Ok(Sink::Native(Box::new(sink)));
    }
    let _ = walk;
    let program = spec.ffmpeg.ok_or_else(|| {
        EngineExportError::Encode(
            "no FFmpeg was given, and this platform has no in-process encoder".into(),
        )
    })?;
    FfmpegSink::new(
        program,
        spec.output,
        width,
        height,
        spec.fps,
        bitrate,
        pixel_layout(width, height),
    )
    .map(|sink| Sink::Ffmpeg(Box::new(sink)))
    .map_err(|e| EngineExportError::Encode(e.to_string()))
}

/// The source geometry an export will render from: `native`, shrunk until the
/// canvas `state` builds from it fits `max`.
#[must_use]
pub fn capped_source(
    native: SourceGeometry,
    state: &RenderState,
    max: Option<(u32, u32)>,
) -> SourceGeometry {
    let Some(max) = max else { return native };
    let natural = canvas_geometry(
        native.width,
        native.height,
        state.padding,
        state.output_aspect.as_deref(),
    );
    scaled_source(native, fit_scale((natural.canvas_w, natural.canvas_h), max))
}

/// Renders `state` through the engine, returning the frames written. Writes the
/// scene's audio too, where the backend can and the spec asks.
pub fn export_video(
    state: &RenderState,
    spec: &ExportSpec<'_>,
    progress: &mut dyn FnMut(u64, u64) -> Flow,
) -> Result<ExportReport, EngineExportError> {
    let (output, fps) = (spec.output, spec.fps);
    let captions = spec.captions;
    let ctx = shared_context()?;

    // The recording decides the geometry; the state says what to do with it.
    let mut pictures = open_pictures(spec)?;
    let native = pictures.size();
    let source = capped_source(native, state, spec.max_size);

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
    if state.cursor_enabled {
        match spec.cursor_track.map(load_cursor_track) {
            Some(Ok(track)) => session.set_cursor_track(Some(track)),
            // Refused rather than rendered bare: a cursor the user asked for and cannot see is a wrong file, not a degraded one.
            Some(Err(e)) => {
                return Err(EngineExportError::Unsupported(format!(
                    "the cursor track did not load: {e}"
                )))
            }
            None => {
                return Err(EngineExportError::Unsupported(
                    "the cursor is enabled but no track was given".into(),
                ))
            }
        }
    }
    // Opened through the same backend as the screen, so the two decode alike.
    let mut camera = match (state.camera_overlay.enabled, spec.camera) {
        (true, Some((path, offset))) => Some((open_video(spec, path)?, offset)),
        // A camera the user enabled and cannot see is a wrong file, not a degraded one.
        (true, None) => {
            return Err(EngineExportError::Unsupported(
                "the camera overlay is enabled but no recording was given".into(),
            ))
        }
        (false, _) => None,
    };
    let background = match background_image(state) {
        Some(path) => Some(upload_background(ctx, &path)?),
        None => None,
    };
    let annotation_images = upload_annotation_images(ctx, state);
    // The view outlives the borrow `Extras` takes, so it is built before the loop.
    let background_view = background.as_ref().map(|(texture, w, h)| {
        (
            texture.create_view(&wgpu::TextureViewDescriptor::default()),
            *w,
            *h,
        )
    });
    // The editor's map wins over the scene-derived one: they can differ, and the muxed audio follows the editor's.
    if let Some(map) = spec.time_map.clone() {
        session.set_time_map(Some(map));
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
    let mut sink = open_sink(spec, size.width, size.height, walk, bitrate)?;
    let mut report = ExportReport {
        frames: walk.len(),
        codec: sink.name(),
        pixels: match pixel_layout(size.width, size.height) {
            PixelLayout::Nv12 => "nv12-gpu",
            PixelLayout::Rgba => "rgba-cpu",
        },
        width: size.width,
        height: size.height,
        source_width: native.width,
        source_height: native.height,
        audio: false,
        captions: captions.is_some(),
    };

    let total = walk.len();
    // The colour the sink encodes in, so both halves agree on the matrix.
    let mut loop_ = match pixel_layout(size.width, size.height) {
        PixelLayout::Nv12 => FrameLoop::with_nv12(SourceColor::default()),
        PixelLayout::Rgba => FrameLoop::new(),
    };
    loop_
        .run(
            &mut session,
            &mut pictures,
            walk,
            ctx,
            recast_export::Extras {
                background: background_view.as_ref().map(|(view, w, h)| {
                    recast_compositor::BackgroundImage {
                        view,
                        width: *w,
                        height: *h,
                        needs_srgb_decode: true,
                    }
                }),
                camera: camera
                    .as_mut()
                    .map(|(pictures, offset)| (pictures, *offset)),
                annotations: &annotation_images,
            },
            |index, frame| {
                sink.push(index, frame)?;
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
    if spec.audio && sink.writes_audio() {
        let scene = to_scene(state);
        // The same timeline the video walked: audio placed at output zero while the video started at the trim was the whole file out of sync.
        let map = spec
            .time_map
            .clone()
            .unwrap_or_else(|| scene.timeline.time_map());
        if !crate::export_audio::spans_are_conformable(&map.spans) {
            // A resample here would shift pitch where FFmpeg's `atempo` does not, so a sped-up project keeps the path whose audio matches the preview.
            return Err(EngineExportError::Unsupported(
                "a speed change needs the FFmpeg audio path".into(),
            ));
        }
        let mut mixer = recast_audio::mixer_for(
            &scene.audio,
            RenderSource::output_duration(&session),
            crate::export_audio::sources_for(&scene.audio, &spec.audio_sources, &map.spans),
        );
        if mixer.total_frames() > 0 {
            sink.push_audio(&mut mixer, AUDIO_BITRATE)?;
            report.audio = true;
        }
    }

    sink.finish(output)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The compositor draws the pointer from the track alone, so the only way
    /// it reaches an engine export is this file. Written and read back rather
    /// than hand-built: the desktop and the crate have separate `CursorTrack`
    /// types, and a field rename on either side would silently export no cursor.
    #[test]
    fn a_written_cursor_track_loads_into_the_shape_the_engine_draws_from() {
        let dir = std::env::temp_dir().join(format!("recast-cursor-track-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch");
        let path = dir.join("cursor.json");
        let track = crate::cursor::CursorTrack {
            samples: vec![crate::cursor::CursorSample {
                timestamp_us: 500_000,
                x: 640,
                y: 360,
                velocity_x: 0.0,
                velocity_y: 0.0,
                visible: true,
                left_down: true,
                right_down: false,
            }],
            ..Default::default()
        };
        crate::cursor::write_cursor_track(&path, &track).expect("the track writes");

        let loaded = load_cursor_track(&path).expect("the engine reads it back");
        assert_eq!(loaded.samples.len(), 1, "the samples did not survive");
        let sample = loaded.samples[0];
        assert_eq!(sample.timestamp_us, 500_000);
        assert!((sample.x - 640.0).abs() < 1e-9 && (sample.y - 360.0).abs() < 1e-9);
        assert!(sample.visible && sample.left_down);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The flag crosses the bridge as `engineExport`, and a rename on either
    /// side would silently leave every export on the graph.
    #[test]
    fn the_flag_the_editor_sends_arrives_as_the_request_field() {
        let sent = serde_json::json!({
            "exportId": "e1",
            "inputPath": "in.mp4",
            "format": "mp4",
            "quality": "source",
            "renderState": RenderState::default(),
            "engineExport": true,
        });
        let request: crate::commands::types::ExportRequest =
            serde_json::from_value(sent).expect("the editor's payload");
        assert!(request.engine_export);
    }

    /// A payload queued before the flag existed must not start rendering through
    /// the engine when the app is updated under it.
    #[test]
    fn an_older_queued_payload_stays_on_the_graph() {
        let sent = serde_json::json!({
            "exportId": "e1",
            "inputPath": "in.mp4",
            "format": "mp4",
            "quality": "source",
            "renderState": RenderState::default(),
        });
        let request: crate::commands::types::ExportRequest =
            serde_json::from_value(sent).expect("an older payload");
        assert!(!request.engine_export);
    }

    /// The line is what a report from a machine nobody here owns will contain,
    /// so it has to name every choice an export made, not just its size.
    #[test]
    fn the_report_names_every_choice_the_export_made() {
        let report = ExportReport {
            frames: 90,
            codec: "media-foundation",
            pixels: "nv12-gpu",
            width: 1280,
            height: 720,
            source_width: 1920,
            source_height: 1080,
            audio: true,
            captions: false,
        };
        let line = report.to_string();
        for part in [
            "90 frames",
            "1280x720",
            "source 1920x1080",
            "codec=media-foundation",
            "pixels=nv12-gpu",
            "audio=true",
            "captions=false",
        ] {
            assert!(line.contains(part), "the report omits {part}: {line}");
        }
    }

    /// The env wins both ways, so a developer can force the graph back on a
    /// machine whose user turned the flag on, and the reverse.
    #[test]
    fn the_environment_overrides_the_flag_in_both_directions() {
        assert!(engine_opt_in(false, Some("1")));
        assert!(!engine_opt_in(true, Some("0")));
    }

    #[test]
    fn without_an_override_the_flag_decides() {
        assert!(engine_opt_in(true, None));
        assert!(!engine_opt_in(false, None));
        // Anything but 1 or 0 is not an answer, so the flag still decides.
        assert!(engine_opt_in(true, Some("yes")));
        assert!(!engine_opt_in(false, Some("yes")));
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

    /// The rate has to follow the cap. A file-size check cannot see this: a
    /// static frame compresses to nearly nothing whatever rate it is given.
    #[test]
    fn a_capped_export_asks_for_a_rate_matching_what_it_renders() {
        let state = RenderState::default();
        let native = SourceGeometry {
            width: 1920,
            height: 1080,
        };
        let capped = capped_source(native, &state, Some((1280, 720)));
        assert_eq!((capped.width, capped.height), (1280, 720));
        assert!(
            bitrate_for(capped.width, capped.height, 30.0)
                < bitrate_for(native.width, native.height, 30.0)
        );
    }

    /// Padding is a percentage of the source, so the canvas is larger than the
    /// source and the cap has to be measured against the canvas.
    #[test]
    fn the_cap_is_measured_against_the_padded_canvas() {
        let padded = RenderState {
            padding: 20.0,
            ..Default::default()
        };
        let native = SourceGeometry {
            width: 1920,
            height: 1080,
        };
        let capped = capped_source(native, &padded, Some((1280, 720)));
        let plain = capped_source(native, &RenderState::default(), Some((1280, 720)));
        assert!(
            capped.width < plain.width,
            "padding did not tighten the cap: {} against {}",
            capped.width,
            plain.width
        );
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

    /// Serialises the exports below. They share one GPU context and one Media
    /// Foundation encoder, and running them at once made three of them flake.
    static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// The lock, taken even when a previous test panicked holding it: poisoning
    /// would turn one failure into a cascade of unrelated ones.
    fn exclusive() -> std::sync::MutexGuard<'static, ()> {
        ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner())
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
            source: SourceInfo {
                width: SRC_W,
                height: SRC_H,
                fps: 30.0,
            },
            ffmpeg: None,
            force_ffmpeg: false,
            time_map: None,
            cursor_track: None,
            camera: None,
            audio_sources: crate::export_audio::RecordingAudio {
                video: Some(input),
                ..Default::default()
            },
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
                ctx,
                recast_export::Extras::default(),
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
                ctx,
                recast_export::Extras::default(),
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
        let _serial = exclusive();
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
        let report = export_video(&state, &spec(&input, &output), &mut never_cancels)
            .expect("the export runs");
        let frames = report.frames;
        assert_eq!(frames, FrameWalk::new(0.5, (30, 1)).len());

        let mut reader = recast_codec_mf::VideoReader::open(&output).expect("the export opens");
        let mut decoded = 0u64;
        while reader.next_frame().expect("decode").is_some() {
            decoded += 1;
        }
        assert_eq!(decoded, frames, "frames went missing between the two files");
    }

    /// The pointer is drawn from the track and nothing else, so a track that
    /// never reaches the session exports a file with no cursor in it. Compared
    /// against the same export with the cursor off: only the pointer differs.
    #[test]
    fn the_cursor_track_reaches_the_exported_picture() {
        let Some(ctx) = context() else { return };
        let _serial = exclusive();
        let scratch = Scratch::new("cursor");
        let input = scratch.0.join("in.mp4");
        record(&ctx, &input, 0.3);

        let track_path = scratch.0.join("cursor.json");
        let mut track = crate::cursor::CursorTrack::default();
        // Still, centred and visible for the whole clip, so every frame carries it.
        for step in 0..10 {
            track.samples.push(crate::cursor::CursorSample {
                timestamp_us: step * 30_000,
                x: (SRC_W / 2) as i32,
                y: (SRC_H / 2) as i32,
                velocity_x: 0.0,
                velocity_y: 0.0,
                visible: true,
                left_down: false,
                right_down: false,
            });
        }
        crate::cursor::write_cursor_track(&track_path, &track).expect("the track writes");

        let base = RenderState {
            trim_start: 0.0,
            trim_end: 0.3,
            cursor_enabled: false,
            ..Default::default()
        };
        let plain_out = scratch.0.join("plain.mp4");
        export_video(&base, &spec(&input, &plain_out), &mut never_cancels).expect("plain export");

        let with_cursor = RenderState {
            cursor_enabled: true,
            ..base.clone()
        };
        let cursor_out = scratch.0.join("cursor.mp4");
        let mut cursor_spec = spec(&input, &cursor_out);
        cursor_spec.cursor_track = Some(&track_path);
        export_video(&with_cursor, &cursor_spec, &mut never_cancels).expect("cursor export");

        let plain = std::fs::read(&plain_out).expect("plain file");
        let drawn = std::fs::read(&cursor_out).expect("cursor file");
        assert_ne!(
            plain, drawn,
            "the cursor track changed nothing, so it never reached the session"
        );
    }

    /// The wallpaper is a static asset the frame loop has to be handed; without
    /// it the background fell back to flat grey and the export looked finished.
    #[test]
    fn an_image_background_reaches_the_exported_picture() {
        let Some(ctx) = context() else { return };
        let _serial = exclusive();
        let scratch = Scratch::new("wallpaper");
        let input = scratch.0.join("in.mp4");
        record(&ctx, &input, 0.2);

        // Padding, so the background is visible around the card rather than covered.
        let base = RenderState {
            trim_start: 0.0,
            trim_end: 0.2,
            padding: 15.0,
            cursor_enabled: false,
            background_type: "color".into(),
            background_value: "#111111".into(),
            ..Default::default()
        };
        let plain_out = scratch.0.join("plain.mp4");
        export_video(&base, &spec(&input, &plain_out), &mut never_cancels).expect("plain export");

        // A saturated wallpaper, so it cannot be mistaken for the flat fallback.
        let wallpaper = scratch.0.join("wall.png");
        let mut pixels = image::RgbaImage::new(64, 64);
        for pixel in pixels.pixels_mut() {
            *pixel = image::Rgba([255, 0, 255, 255]);
        }
        pixels.save(&wallpaper).expect("the wallpaper writes");

        let with_image = RenderState {
            background_type: "wallpaper".into(),
            background_value: wallpaper.to_string_lossy().into_owned(),
            ..base.clone()
        };
        let image_out = scratch.0.join("image.mp4");
        export_video(&with_image, &spec(&input, &image_out), &mut never_cancels)
            .expect("wallpaper export");

        let plain = std::fs::read(&plain_out).expect("plain file");
        let drawn = std::fs::read(&image_out).expect("wallpaper file");
        assert_ne!(
            plain, drawn,
            "the wallpaper changed nothing, so it never reached the frame loop"
        );
    }

    /// Image annotations are also how a text annotation reaches an export: the
    /// WebView rasterises it to a PNG first. Missing them lost the user's text.
    #[test]
    fn an_image_annotation_reaches_the_exported_picture() {
        let Some(ctx) = context() else { return };
        let _serial = exclusive();
        let scratch = Scratch::new("annotation");
        let input = scratch.0.join("in.mp4");
        record(&ctx, &input, 0.2);

        let overlay = scratch.0.join("mark.png");
        let mut pixels = image::RgbaImage::new(32, 32);
        for pixel in pixels.pixels_mut() {
            *pixel = image::Rgba([0, 255, 0, 255]);
        }
        pixels.save(&overlay).expect("the overlay writes");

        let base = RenderState {
            trim_start: 0.0,
            trim_end: 0.2,
            cursor_enabled: false,
            ..Default::default()
        };
        let plain_out = scratch.0.join("plain.mp4");
        export_video(&base, &spec(&input, &plain_out), &mut never_cancels).expect("plain export");

        let mut marked = base.clone();
        marked.annotations = vec![serde_json::from_value(serde_json::json!({
            "id": "i1", "start": 0.0, "end": 10.0,
            "kind": {"kind": "image", "x": 0.1, "y": 0.1, "w": 0.5, "h": 0.5,
                     "path": overlay.to_string_lossy()}
        }))
        .expect("annotation")];
        let marked_out = scratch.0.join("marked.mp4");
        export_video(&marked, &spec(&input, &marked_out), &mut never_cancels)
            .expect("annotated export");

        let plain = std::fs::read(&plain_out).expect("plain file");
        let drawn = std::fs::read(&marked_out).expect("annotated file");
        assert_ne!(
            plain, drawn,
            "the image annotation changed nothing, so it never reached the frame loop"
        );
    }

    /// The bubble is a separate recording composited at export, so without the
    /// second picture source the camera layer rendered as nothing at all.
    #[test]
    fn the_camera_recording_reaches_the_exported_picture() {
        let Some(ctx) = context() else { return };
        let _serial = exclusive();
        let scratch = Scratch::new("camera");
        let input = scratch.0.join("in.mp4");
        let camera = scratch.0.join("cam.mp4");
        record(&ctx, &input, 0.2);
        record(&ctx, &camera, 0.2);

        let base = RenderState {
            trim_start: 0.0,
            trim_end: 0.2,
            cursor_enabled: false,
            ..Default::default()
        };
        let plain_out = scratch.0.join("plain.mp4");
        export_video(&base, &spec(&input, &plain_out), &mut never_cancels).expect("plain export");

        let mut with_camera = base.clone();
        with_camera.camera_overlay.enabled = true;
        let camera_out = scratch.0.join("camera.mp4");
        let mut camera_spec = spec(&input, &camera_out);
        camera_spec.camera = Some((&camera, 0.0));
        export_video(&with_camera, &camera_spec, &mut never_cancels).expect("camera export");

        let plain = std::fs::read(&plain_out).expect("plain file");
        let bubble = std::fs::read(&camera_out).expect("camera file");
        assert_ne!(
            plain, bubble,
            "the camera recording changed nothing, so it never reached the frame loop"
        );
    }

    /// A camera the user enabled and cannot see is a wrong file, not a
    /// degraded one, so a missing recording declines to the FFmpeg graph.
    #[test]
    fn a_camera_with_no_recording_declines_instead_of_rendering_none() {
        let Some(ctx) = context() else { return };
        let _serial = exclusive();
        let scratch = Scratch::new("camera-missing");
        let input = scratch.0.join("in.mp4");
        let output = scratch.0.join("out.mp4");
        record(&ctx, &input, 0.2);

        let mut state = RenderState {
            trim_start: 0.0,
            trim_end: 0.2,
            cursor_enabled: false,
            ..Default::default()
        };
        state.camera_overlay.enabled = true;
        let failed = export_video(&state, &spec(&input, &output), &mut never_cancels)
            .expect_err("an enabled camera with no recording cannot be rendered");
        assert!(matches!(failed, EngineExportError::Unsupported(_)));
    }

    /// A cursor the user asked for and cannot see is a wrong file, not a
    /// degraded one, so a missing track declines to the FFmpeg graph.
    #[test]
    fn a_cursor_with_no_track_declines_instead_of_rendering_none() {
        let Some(ctx) = context() else { return };
        let _serial = exclusive();
        let scratch = Scratch::new("cursor-missing");
        let input = scratch.0.join("in.mp4");
        let output = scratch.0.join("out.mp4");
        record(&ctx, &input, 0.2);

        let state = RenderState {
            trim_start: 0.0,
            trim_end: 0.2,
            cursor_enabled: true,
            ..Default::default()
        };
        let failed = export_video(&state, &spec(&input, &output), &mut never_cancels)
            .expect_err("an enabled cursor with no track cannot be rendered");
        assert!(matches!(failed, EngineExportError::Unsupported(_)));
    }

    /// The recording has to reach the canvas, not just the frame count.
    #[test]
    fn the_recording_reaches_the_exported_picture() {
        let Some(ctx) = context() else { return };
        let _serial = exclusive();
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
        let _serial = exclusive();
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
        let report = export_video(&state, &spec(&input, &output), &mut never_cancels)
            .expect("the export runs");

        let bytes = std::fs::read(&output).expect("read back");
        assert!(
            contains(&bytes, b"mp4a") && contains(&bytes, b"esds"),
            "the export has no audio track"
        );
        assert!(report.audio, "the report denies the audio track it wrote");
    }

    /// The most pixels darker than the pill's threshold that any one frame shows in its bottom quarter, where a bottom caption sits.
    /// A count over every frame, not a mean at one index: the pill's effect on a mean is inside rate-control noise, and an index assumes a decode order and clearing the entrance.
    fn darkest_caption_band(path: &Path) -> usize {
        // The pill is #0b0b12 at 61% over a luma-220 source, so it lands near 93.
        const PILL_MAX_LUMA: u8 = 140;
        let mut reader = recast_codec_mf::VideoReader::open(path).expect("opens");
        let (w, h) = {
            let info = reader.info();
            (info.width as usize, info.height as usize)
        };
        let mut worst = 0usize;
        while let Some(frame) = reader.next_frame().expect("decode") {
            let band = &frame.data[w * (h - h / 4)..w * h];
            worst = worst.max(band.iter().filter(|&&b| b < PILL_MAX_LUMA).count());
        }
        worst
    }

    /// B-3: the engine export ignored the transcript entirely, so every export
    /// through it came out caption-less while the graph burned them in.
    #[test]
    fn a_burned_caption_reaches_the_exported_pixels() {
        let Some(ctx) = context() else { return };
        let _serial = exclusive();
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

        // A delta, not a floor: the decoder pads 360 rows to 368 and those rows are dark in both.
        let (before, after) = (
            darkest_caption_band(&plain),
            darkest_caption_band(&captioned),
        );
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
        let _serial = exclusive();
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
            .expect("the export runs")
            .frames;

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
        let _serial = exclusive();
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
        let _serial = exclusive();
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
    }

    /// The intermediate handed to the mux pass must carry no audio: the mux owns
    /// the music clips and the voice detach, and a second track would double up.
    #[test]
    fn an_intermediate_for_the_mux_pass_carries_no_audio_track() {
        let Some(ctx) = context() else { return };
        let _serial = exclusive();
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

    /// The sidecar this machine ships, or `None`, which skips.
    fn sidecar() -> Option<std::path::PathBuf> {
        recast_testkit::ffmpeg_path()
    }

    /// The macOS and Linux export path, run here. Those platforms have no
    /// in-process codec, so the whole path would otherwise ship unexecuted: a
    /// Windows-only CI never compiles a `#[cfg(unix)]` line, let alone runs it.
    #[test]
    fn the_ffmpeg_backend_exports_a_playable_file() {
        let Some(ctx) = context() else { return };
        let _serial = exclusive();
        let Some(ffmpeg) = sidecar() else { return };
        let scratch = Scratch::new("ffmpeg-backend");
        let input = scratch.0.join("in.mp4");
        let output = scratch.0.join("out.mp4");
        record(&ctx, &input, 0.3);

        let state = RenderState {
            trim_start: 0.0,
            trim_end: 0.3,
            cursor_enabled: false,
            ..Default::default()
        };
        let report = export_video(
            &state,
            &ExportSpec {
                force_ffmpeg: true,
                time_map: None,
                cursor_track: None,
                camera: None,
                ffmpeg: Some(&ffmpeg),
                // Video only, as it is on the platforms that take this path.
                audio: false,
                ..spec(&input, &output)
            },
            &mut never_cancels,
        )
        .expect("the export runs");
        let frames = report.frames;
        assert_eq!(frames, FrameWalk::new(0.3, (30, 1)).len());
        // The report is what a machine nobody here owns will be diagnosed from.
        assert_eq!(report.codec, "ffmpeg");
        assert_eq!(report.pixels, "nv12-gpu");
        assert!(
            !report.audio,
            "a video-only backend reported an audio track"
        );

        let mut reader = recast_codec_mf::VideoReader::open(&output).expect("the export opens");
        let mut decoded = 0u64;
        let mut first_luma = None;
        while let Some(frame) = reader.next_frame().expect("decode") {
            if first_luma.is_none() {
                let info = reader.info();
                let luma = (info.width * info.height) as usize;
                let plane = &frame.data[..luma.min(frame.data.len())];
                first_luma =
                    Some(plane.iter().map(|&b| f64::from(b)).sum::<f64>() / plane.len() as f64);
            }
            decoded += 1;
        }
        assert_eq!(decoded, frames, "frames went missing between the two files");
        // The recording has to reach the canvas, not just the frame count.
        assert!(
            first_luma.is_some_and(|mean| mean > 60.0),
            "the FFmpeg backend exported a nearly black frame: {first_luma:?}"
        );
    }

    /// The same composition through both backends must land on the same canvas,
    /// or an export looks different depending on which OS ran it.
    #[test]
    fn both_backends_render_the_same_geometry() {
        let Some(ctx) = context() else { return };
        let _serial = exclusive();
        let Some(ffmpeg) = sidecar() else { return };
        let scratch = Scratch::new("both-backends");
        let input = scratch.0.join("in.mp4");
        let native_out = scratch.0.join("native.mp4");
        let piped_out = scratch.0.join("piped.mp4");
        record(&ctx, &input, 0.2);

        let state = RenderState {
            trim_start: 0.0,
            trim_end: 0.2,
            cursor_enabled: false,
            padding: 6.0,
            ..Default::default()
        };
        export_video(
            &state,
            &ExportSpec {
                audio: false,
                ..spec(&input, &native_out)
            },
            &mut never_cancels,
        )
        .expect("the native export runs");
        export_video(
            &state,
            &ExportSpec {
                force_ffmpeg: true,
                time_map: None,
                cursor_track: None,
                camera: None,
                ffmpeg: Some(&ffmpeg),
                audio: false,
                ..spec(&input, &piped_out)
            },
            &mut never_cancels,
        )
        .expect("the piped export runs");

        let decoded = |path: &Path| {
            let mut reader = recast_codec_mf::VideoReader::open(path).expect("opens");
            let frame = reader.next_frame().expect("decode").expect("a frame");
            let info = reader.info();
            let (w, h) = (info.width, info.height);
            let luma = (w * h) as usize;
            let plane = &frame.data[..luma.min(frame.data.len())];
            let mean = plane.iter().map(|&b| f64::from(b)).sum::<f64>() / plane.len() as f64;
            ((w, h), mean)
        };
        let (native_size, native_luma) = decoded(&native_out);
        let (piped_size, piped_luma) = decoded(&piped_out);
        assert_eq!(
            native_size, piped_size,
            "the two backends disagree on the canvas"
        );
        // The piped backend is fed GPU NV12 on a packable shape; a pixel format the encoder read differently would land here, not in the geometry.
        assert!(
            (native_luma - piped_luma).abs() < 6.0,
            "the two backends drew different pictures: {native_luma:.2} native, {piped_luma:.2} piped"
        );
    }

    /// The FFmpeg backend cannot write audio: its input pipe already carries the
    /// video. Asking must fail loudly rather than drop the track.
    #[test]
    fn the_ffmpeg_backend_refuses_to_write_audio() {
        let Some(ctx) = context() else { return };
        let _serial = exclusive();
        let Some(ffmpeg) = sidecar() else { return };
        let scratch = Scratch::new("ffmpeg-audio");
        let input = scratch.0.join("in.mp4");
        let output = scratch.0.join("out.mp4");
        record_with_sound(&ctx, &input, 0.2);

        let state = RenderState {
            trim_start: 0.0,
            trim_end: 0.2,
            cursor_enabled: false,
            ..Default::default()
        };
        let error = export_video(
            &state,
            &ExportSpec {
                force_ffmpeg: true,
                time_map: None,
                cursor_track: None,
                camera: None,
                ffmpeg: Some(&ffmpeg),
                audio: true,
                ..spec(&input, &output)
            },
            &mut never_cancels,
        );
        assert!(
            error.is_ok(),
            "asking a video-only backend for audio must not fail the export"
        );
        let bytes = std::fs::read(&output).expect("read back");
        assert!(
            !contains(&bytes, b"mp4a"),
            "the FFmpeg backend wrote an audio track it cannot write"
        );
    }

    #[test]
    fn the_ffmpeg_backend_is_refused_without_a_binary_to_run() {
        let Some(_ctx) = context() else { return };
        let scratch = Scratch::new("no-ffmpeg");
        let input = scratch.0.join("in.mp4");
        let output = scratch.0.join("out.mp4");
        let state = RenderState::default();
        let error = export_video(
            &state,
            &ExportSpec {
                force_ffmpeg: true,
                time_map: None,
                cursor_track: None,
                camera: None,
                ffmpeg: None,
                ..spec(&input, &output)
            },
            &mut never_cancels,
        )
        .expect_err("no binary means no export");
        assert!(
            matches!(error, EngineExportError::OpenInput { .. }),
            "{error}"
        );
    }

    /// A WAV of audible tone, standing in for the microphone track a project
    /// captures to its own file.
    fn tone_wav(path: &Path, seconds: f64) {
        use crate::audio::wav::{WavFormat, WavWriter};

        let format = WavFormat::pcm16(48_000, 1);
        let mut writer = WavWriter::new(path, format).expect("a wav writer");
        let frames = (seconds * 48_000.0) as usize;
        let mut bytes = Vec::with_capacity(frames * 2);
        for frame in 0..frames {
            let t = frame as f64 / 48_000.0;
            let value = (t * 440.0 * std::f64::consts::TAU).sin() * 0.6;
            bytes.extend_from_slice(&((value * 32767.0) as i16).to_le_bytes());
        }
        writer.write_samples(&bytes).expect("samples");
        writer.finish().expect("finish");
    }

    /// Phase 8: a project keeps the microphone in its own file, and the engine
    /// export read only the video, so the exported audio lost it entirely.
    #[test]
    fn a_separate_microphone_track_reaches_the_exported_audio() {
        let Some(ctx) = context() else { return };
        let _serial = exclusive();
        let scratch = Scratch::new("mic-track");
        let input = scratch.0.join("in.mp4");
        let mic = scratch.0.join("mic.wav");
        let without = scratch.0.join("without.mp4");
        let with = scratch.0.join("with.mp4");
        record(&ctx, &input, 0.4);
        tone_wav(&mic, 0.4);

        let state = RenderState {
            trim_start: 0.0,
            trim_end: 0.4,
            cursor_enabled: false,
            ..Default::default()
        };
        export_video(&state, &spec(&input, &without), &mut never_cancels).expect("runs");
        export_video(
            &state,
            &ExportSpec {
                audio_sources: crate::export_audio::RecordingAudio {
                    video: Some(&input),
                    microphone: Some(&mic),
                    ..Default::default()
                },
                ..spec(&input, &with)
            },
            &mut never_cancels,
        )
        .expect("runs");

        // Decoded loudness, not file size: a static tone compresses to whatever the rate controller feels like.
        let loudness = |path: &Path| {
            let samples =
                crate::audio_decode::decode_mono(&[path], 16_000).expect("the export decodes");
            match samples.is_empty() {
                true => 0.0,
                false => (samples
                    .iter()
                    .map(|s| f64::from(*s) * f64::from(*s))
                    .sum::<f64>()
                    / samples.len() as f64)
                    .sqrt(),
            }
        };
        let bytes = std::fs::read(&with).expect("read back");
        assert!(
            contains(&bytes, b"mp4a"),
            "the export carries no audio track at all"
        );
        assert!(
            loudness(&with) > loudness(&without) + 0.01,
            "the microphone track added nothing audible: {:.4} without, {:.4} with",
            loudness(&without),
            loudness(&with)
        );
    }

    /// A canvas the shader cannot pack still has to export, and the report has
    /// to say the pixels went the slow way rather than claim the GPU path.
    #[test]
    fn an_unpackable_canvas_falls_back_and_says_so() {
        let Some(ctx) = context() else { return };
        let _serial = exclusive();
        let scratch = Scratch::new("unpackable");
        let input = scratch.0.join("in.mp4");
        let output = scratch.0.join("out.mp4");
        record(&ctx, &input, 0.2);

        let state = RenderState {
            trim_start: 0.0,
            trim_end: 0.2,
            cursor_enabled: false,
            ..Default::default()
        };
        // 640 scaled by 302/640 lands on 302, which is even but not a multiple of four.
        let report = export_video(
            &state,
            &ExportSpec {
                max_size: Some((302, 400)),
                bitrate: None,
                ..spec(&input, &output)
            },
            &mut never_cancels,
        )
        .expect("the export runs");

        assert!(
            !recast_export::GpuNv12::handles(report.width, report.height),
            "the fixture canvas {}x{} is packable after all",
            report.width,
            report.height
        );
        assert_eq!(report.pixels, "rgba-cpu");
        assert!(output.exists(), "an unpackable canvas produced no file");
    }

    /// A voice-over replaces the recording, so its captured tracks must not be
    /// gathered at all: layering both plays the original under the voice.
    #[test]
    fn a_voice_over_keeps_the_recordings_own_tracks_out_of_the_mix() {
        let Some(ctx) = context() else { return };
        let _serial = exclusive();
        let scratch = Scratch::new("voice-over");
        let input = scratch.0.join("in.mp4");
        let mic = scratch.0.join("mic.wav");
        record_with_sound(&ctx, &input, 0.2);
        tone_wav(&mic, 0.2);

        let sources = crate::export_audio::RecordingAudio {
            video: Some(&input),
            microphone: Some(&mic),
            ..Default::default()
        };
        let quiet = recast_scene::AudioGraph {
            settings: Default::default(),
            clips: vec![voice_clip(&mic)],
        };
        assert!(
            crate::export_audio::sources_for(&quiet, &sources, &[])
                .recording_kinds()
                .is_empty(),
            "the recording was mixed under the voice-over"
        );

        let no_voice = recast_scene::AudioGraph::default();
        assert_eq!(
            crate::export_audio::sources_for(&no_voice, &sources, &[])
                .recording_kinds()
                .len(),
            2,
            "without a voice-over both captured tracks belong in the mix"
        );
    }

    /// A capture that never opened leaves a header with no samples, and
    /// gathering it would put an empty source into the mix.
    #[test]
    fn an_empty_capture_never_reaches_the_sources() {
        let Some(ctx) = context() else { return };
        let _serial = exclusive();
        let scratch = Scratch::new("empty-capture");
        let input = scratch.0.join("in.mp4");
        let mic = scratch.0.join("mic.wav");
        record_with_sound(&ctx, &input, 0.2);
        crate::audio::wav::write_silence_wav(&mic, 48_000, 1, 0.0).expect("an empty wav");

        let gathered = crate::export_audio::sources_for(
            &recast_scene::AudioGraph::default(),
            &crate::export_audio::RecordingAudio {
                video: Some(&input),
                microphone: Some(&mic),
                ..Default::default()
            },
            &[],
        );
        assert_eq!(
            gathered.recording_kinds().len(),
            1,
            "the empty capture was gathered alongside the recording"
        );
    }

    /// An audible voice-over clip pointing at a real file.
    fn voice_clip(path: &Path) -> recast_scene::v1::nodes::AudioClip {
        serde_json::from_value(serde_json::json!({
            "id": "voice",
            "source": { "kind": "local", "path": path.to_string_lossy() },
            "role": "voice",
            "gain": 100.0,
        }))
        .expect("voice clip")
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
