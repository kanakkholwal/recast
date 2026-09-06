pub mod clock;
pub mod pipeline;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::audio::{
    AudioCaptureConfig, AudioCaptureSession, MicrophoneCaptureConfig, MicrophoneCaptureSession,
};
use crate::capture::{CaptureNotice, CaptureTarget};
use crate::cursor::{spawn_cursor_capture, write_cursor_track, CursorCaptureFrame, CursorTrack};
use crate::encoder::{spawn_encoder_loop, EncoderConfig};
use crate::render::node_types::{CameraMotionSegment, CameraOverlaySettings, CameraPlacement};
pub use clock::{offset_ms_from_video, RecordingClock, TrackStart};
use pipeline::{
    spawn_capture_loop, Cadence, CaptureLoop, PipelineSnapshot, QueueSink, RecordingPipeline,
};

/// Frames per second emitted by the pacer and declared to the encoder as `-framerate`.
/// Together they guarantee one wall-clock second becomes one second of video PTS, the invariant the wall-clock-stamped cursor track relies on for sync.
pub const RECORDING_FPS: u32 = 60;

//  Shared types

//  Recording stats and artifacts

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStats {
    pub captured_frames: u64,
    pub encoded_frames: u64,
    pub dropped_frames: u64,
    pub duration_ms: u64,
    pub nominal_fps: u32,
}

/// Signed millisecond offsets of each companion track against video frame 0: positive started late and needs head padding, negative started early and must be trimmed.
/// `None` means the track produced nothing, so consumers treat it as aligned.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackOffsets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub microphone_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct RecordingArtifacts {
    pub capture_target: CaptureTarget,
    pub recording_path: PathBuf,
    pub cursor_path: PathBuf,
    pub audio_path: PathBuf,
    /// Whether a real system-audio (loopback) track was captured. False when
    /// the user disabled system audio — `audio_path` then points at a silent
    /// fallback WAV, so downstream muxing is unaffected but the project
    /// metadata reports the track as absent.
    pub has_system_audio: bool,
    pub microphone_path: Option<PathBuf>,
    pub camera_path: Option<PathBuf>,
    /// Whether the session asked for a camera. Paired with `camera_path` this
    /// separates "camera was off" from "camera was on but didn't arrive"; the
    /// latter also pushes a warning, but the project has to remember it.
    pub camera_requested: bool,
    pub camera_overlay: CameraOverlaySettings,
    /// Signed milliseconds each companion track starts after video frame 0.
    /// Every capture device comes up at its own instant, so without these the
    /// muxer lines all tracks up at 0 and bakes the skew into the export.
    pub track_offsets: TrackOffsets,
    pub started_at_unix_ms: u64,
    pub stats: RecordingStats,
    /// Non-fatal issues to surface to the user after a successful stop, e.g. a
    /// requested camera or microphone track that failed to capture (device in
    /// use, or on macOS Camera/Microphone permission denied). The recording
    /// still succeeds without those tracks; the caller shows these as a toast.
    pub warnings: Vec<String>,
}

/// Options controlling what gets captured in a recording session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingOptions {
    /// Capture system/loopback audio (what you hear).
    #[serde(default = "default_true")]
    pub system_audio: bool,
    /// Capture microphone input.
    #[serde(default)]
    pub microphone: bool,
    /// Microphone device ID (None = default device).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub microphone_device_id: Option<String>,
    /// Capture camera video.
    #[serde(default)]
    pub camera: bool,
    /// Camera device ID / DirectShow device name (None = first available).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_device_id: Option<String>,
    /// Capture frame rate. `None` (or out of the supported 24..=240 range)
    /// falls back to [`RECORDING_FPS`]. The pacer and encoder both run at this
    /// rate; values above the monitor's refresh just duplicate frames, so the
    /// UI gates the offered options by the detected display refresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<u32>,
    /// Capture quality tier — `"auto"` (default), `"balanced"`, `"high"`, or
    /// `"pristine"`. `"auto"`/unknown resolve against the detected encoder
    /// (hardware → high, software → balanced). See
    /// [`crate::encoder::RecordingQuality::resolve`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    /// Prefer the FFmpeg-free GPU writer. Mirrored from `AppConfig`, which the
    /// command layer reads; the env override is applied further down so a
    /// developer can force it without touching settings.
    #[serde(default)]
    pub native_encoder: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraPreviewUpdate {
    pub mirror: bool,
    pub shape: String,
    pub corner_radius: f64,
    pub animation_preset: String,
    pub window_x: f64,
    pub window_y: f64,
    pub window_width: f64,
    pub window_height: f64,
}

fn default_true() -> bool {
    true
}

impl Default for RecordingOptions {
    fn default() -> Self {
        Self {
            system_audio: true,
            microphone: false,
            microphone_device_id: None,
            camera: false,
            camera_device_id: None,
            fps: None,
            quality: None,
            native_encoder: false,
        }
    }
}

const QUEUE_BUDGET_BYTES: u64 = 256 * 1024 * 1024;
/// Enough to keep the encoder fed while the pacer writes: one in flight each
/// way plus two spare. Only binds past 4K, where the budget alone yields fewer.
const QUEUE_MIN_FRAMES: u64 = 4;
const QUEUE_MAX_FRAMES: u64 = 180;

fn frame_bytes_bgra(width: u32, height: u32) -> u64 {
    (width as u64)
        .saturating_mul(height as u64)
        .saturating_mul(4)
        .max(1)
}

fn queue_capacity_for(width: u32, height: u32) -> usize {
    (QUEUE_BUDGET_BYTES / frame_bytes_bgra(width, height)).clamp(QUEUE_MIN_FRAMES, QUEUE_MAX_FRAMES)
        as usize
}

#[cfg(test)]
mod queue_budget_tests {
    use super::*;

    fn queue_bytes(width: u32, height: u32) -> u64 {
        frame_bytes_bgra(width, height) * queue_capacity_for(width, height) as u64
    }

    #[test]
    fn queue_never_exceeds_its_memory_budget_up_to_4k() {
        for &(w, h, label) in &[
            (1280u32, 720u32, "720p"),
            (1920, 1080, "1080p"),
            (2560, 1440, "1440p"),
            (3440, 1440, "ultrawide 1440p"),
            (3840, 2160, "4K"),
        ] {
            let bytes = queue_bytes(w, h);
            assert!(
                bytes <= QUEUE_BUDGET_BYTES,
                "{label} queue is {} MB, over the {} MB budget",
                bytes / (1024 * 1024),
                QUEUE_BUDGET_BYTES / (1024 * 1024)
            );
        }
    }

    #[test]
    fn past_4k_the_floor_wins_but_stays_bounded() {
        for &(w, h, label) in &[(7680u32, 2160u32, "dual 4K span"), (7680, 4320, "8K")] {
            assert_eq!(
                queue_capacity_for(w, h),
                QUEUE_MIN_FRAMES as usize,
                "{label} should sit on the floor"
            );
            assert!(
                queue_bytes(w, h) <= 2 * QUEUE_BUDGET_BYTES,
                "{label} floor allocation is unbounded"
            );
        }
    }

    #[test]
    fn small_captures_are_capped_by_frame_count_not_memory() {
        assert_eq!(queue_capacity_for(640, 360), QUEUE_MAX_FRAMES as usize);
    }

    #[test]
    fn every_capture_keeps_enough_frames_to_pipeline() {
        for &(w, h) in &[(1920u32, 1080u32), (3840, 2160), (7680, 4320)] {
            assert!(queue_capacity_for(w, h) >= QUEUE_MIN_FRAMES as usize);
        }
    }

    #[test]
    fn a_1440p_queue_is_not_the_pre_fix_421mb() {
        assert!(queue_bytes(2560, 1440) < 421 * 1024 * 1024);
    }

    #[test]
    fn a_degenerate_target_does_not_divide_by_zero() {
        assert!(queue_capacity_for(0, 0) > 0);
    }
}

/// Clamp a requested capture frame rate to a sane range, falling back to the
/// default when unset or out of range. The lower bound matches the lowest
/// cinematic rate; the upper bound covers high-refresh panels (240 Hz) while
/// rejecting absurd values that would blow the queue budget.
/// Whether the project really has a captured system-audio track, and what the
/// user has to be told when it does not.
///
/// Silence is written in every failing case so the mux always has a track, and
/// that file HAS samples: the samples guard cannot tell it apart from a real
/// capture, so the outcome has to be decided here rather than inferred later.
fn system_audio_outcome(
    requested: bool,
    stopped_ok: bool,
    has_samples: bool,
) -> (bool, Option<&'static str>) {
    if !requested {
        return (false, None);
    }
    if !stopped_ok {
        return (false, Some("System audio could not be recorded."));
    }
    (has_samples, None)
}

fn resolve_recording_fps(requested: Option<u32>) -> u32 {
    match requested {
        Some(fps) if (24..=240).contains(&fps) => fps,
        _ => RECORDING_FPS,
    }
}

//  Recording session orchestration

pub struct RecordingManager {
    session: Mutex<Option<RecordingSession>>,
    pending_camera_overlay: Mutex<CameraOverlaySettings>,
}

impl Default for RecordingManager {
    fn default() -> Self {
        Self {
            session: Mutex::new(None),
            pending_camera_overlay: Mutex::new(CameraOverlaySettings::default()),
        }
    }
}

impl RecordingManager {
    /// Reaps a still-live session; must be called from the exit handler, since quitting via `app.exit(0)` ends in `std::process::exit` and runs no destructors.
    /// The encoder survived on luck (its stdin closes), but the audio, mic and camera children held the devices after Recast was gone.
    pub fn abort_for_shutdown(&self) {
        if let Some(session) = self.session.lock().take() {
            log::warn!("aborting live recording session on shutdown");
            session.abort();
        }
    }
}

impl Drop for RecordingManager {
    fn drop(&mut self) {
        // A session left at drop means `stop()` never ran: reap it, or the capture, audio and mic children keep holding devices.
        self.abort_for_shutdown();
    }
}

#[derive(Clone)]
struct CameraOverlayTracker {
    overlay: CameraOverlaySettings,
    last_placement: Option<CameraPlacement>,
    last_at_secs: Option<f64>,
}

/// Forces the FFmpeg-free writer on or off regardless of the setting: `1` on, `0` off. Unset defers to `AppConfig::native_encoder`.
/// The native path is complete but has only run on this machine; until it has recorded on other people's hardware, the setting defaults off.
const NATIVE_ENCODER_ENV: &str = "RECAST_NATIVE_ENCODER";

/// Whether this recording should try the native writer: the env override if one is set, otherwise the user's setting.
/// Pure, so the precedence is testable without an app handle or a registry.
fn native_opt_in(setting: bool, env: Option<&str>) -> bool {
    match env {
        Some("1") => true,
        Some("0") => false,
        _ => setting,
    }
}

/// Whether this recording writes through the GPU encoder, and why not if not.
struct NativeChoice {
    chosen: bool,
    refused: Option<&'static str>,
}

/// The reason the native writer cannot take this recording, or `None`.
/// Pure so the policy is testable without a display or an encoder: every input here is something the caller has already looked up.
const fn native_refusal(
    opted_in: bool,
    platform_supported: bool,
    encoder_available: bool,
    cropped: bool,
) -> Option<&'static str> {
    if !platform_supported {
        return Some("the GPU writer is Windows-only so far");
    }
    if !opted_in {
        return Some("not opted in");
    }
    if !encoder_available {
        return Some("this machine has no Media Foundation H.264 encoder");
    }
    if cropped {
        // The metadata still describes a full-size source, so the editor would disagree.
        return Some("a cropped recording still goes through the FFmpeg crop filter");
    }
    None
}

impl NativeChoice {
    /// Where the source must leave its pixels for the chosen writer.
    const fn frame_mode(&self) -> crate::capture::FrameMode {
        if self.chosen {
            crate::capture::FrameMode::Gpu
        } else {
            crate::capture::FrameMode::Host
        }
    }
}

/// The writer for this recording, and the cadence it needs.
/// The native writer stamps every sample so it takes only real frames, while FFmpeg reads a timestamp-less pipe and needs one frame per slot whether the desktop changed or not.
fn writer_for(
    native: &NativeChoice,
    path: &std::path::Path,
    fps: u32,
    pipeline: &RecordingPipeline,
) -> (Box<dyn pipeline::FrameSink>, Cadence) {
    #[cfg(windows)]
    if native.chosen {
        return (
            Box::new(crate::encoder::native::NativeSink::new(
                path.to_path_buf(),
                fps,
                pipeline.stats(),
            )),
            Cadence::OnChange {
                keepalive: crate::encoder::native::KEEPALIVE,
            },
        );
    }
    #[cfg(not(windows))]
    let _ = (native, path, fps);
    (Box::new(QueueSink::new(pipeline.clone())), Cadence::Fixed)
}

fn native_encoding_choice(target: &CaptureTarget, setting: bool) -> NativeChoice {
    let env = std::env::var(NATIVE_ENCODER_ENV).ok();
    let opted_in = native_opt_in(setting, env.as_deref());
    #[cfg(windows)]
    let (platform_supported, encoder_available) = (true, crate::encoder::native::available());
    #[cfg(not(windows))]
    let (platform_supported, encoder_available) = (false, false);
    let refused = native_refusal(
        opted_in,
        platform_supported,
        encoder_available,
        target.crop_relative_to_source().is_some(),
    );
    NativeChoice {
        chosen: refused.is_none(),
        refused,
    }
}

struct RecordingSession {
    stop_flag: Arc<AtomicBool>,
    /// Set while the recording is paused — capture/audio threads skip work.
    pause_flag: Arc<AtomicBool>,
    capture_handle: JoinHandle<Result<()>>,
    /// `None` on the native path, where the capture thread encodes as it goes.
    encoder_handle: Option<JoinHandle<Result<()>>>,
    cursor_handle: JoinHandle<CursorTrack>,
    /// Wall-clock μs from recording start to the first encoded video frame (capture-source warmup). Subtracted from the cursor track at `stop()` so cursor t=0 aligns with video frame 0.
    video_start: TrackStart,
    audio_session: Option<AudioCaptureSession>,
    audio_path: PathBuf,
    audio_start: TrackStart,
    microphone_session: Option<MicrophoneCaptureSession>,
    microphone_start: TrackStart,
    /// Camera was requested this session. `camera::session` owns the device and
    /// writes the track directly, so the offset below is measured against this
    /// session's own clock instead of being reported over IPC.
    camera_requested: bool,
    camera_path: PathBuf,
    camera_start: TrackStart,
    pipeline: RecordingPipeline,
    target: CaptureTarget,
    recording_path: PathBuf,
    cursor_path: PathBuf,
    /// Pause-aware clock — source of truth for all sync-relevant timing.
    clock: RecordingClock,
    started_at_unix_ms: u64,
    camera_overlay: CameraOverlayTracker,
    /// Capture rate this session was started at (pacer + encoder + metadata).
    recording_fps: u32,
}

impl RecordingSession {
    /// Best-effort teardown for an abnormal shutdown, mirroring the reaping half of `stop()` without assembling artifacts.
    /// `RecordingSession` deliberately does not implement `Drop`, so `stop()` can still move fields out of it; this consuming helper is the abnormal-path equivalent.
    fn abort(self) {
        self.stop_flag.store(true, Ordering::Release);
        // Joining lets each thread run its own cleanup: the capture thread drops its `CaptureSource`, killing the FFmpeg child.
        let _ = self.capture_handle.join();
        let _ = self.cursor_handle.join();
        if let Some(encoder) = self.encoder_handle {
            let _ = encoder.join();
        }
        // Each OS session reaps its own FFmpeg child / releases its device.
        if let Some(session) = self.audio_session {
            let _ = session.stop();
        }
        if let Some(session) = self.microphone_session {
            let _ = session.stop();
        }
        // The camera lives in camera::session's own slot, not in this session.
    }
}

/// Sub-pixel jitter in the reported preview geometry is not a camera move.
/// About two pixels on a 1080p frame.
const CAMERA_MOVE_EPSILON: f64 = 0.002;

/// One drag stays one segment; a stream of moves that never stops does not.
/// A recorded segment replays as an eased glide between its endpoints, so an
/// over-long one invents a movement that never happened.
const MAX_MOTION_SEGMENT_SECS: f64 = 10.0;

/// Compared against the last ACCEPTED position, so a drag slower than the dead
/// zone accumulates instead of being filtered away tick by tick.
fn camera_moved(from: &CameraPlacement, to: &CameraPlacement) -> bool {
    (to.x - from.x).abs() > CAMERA_MOVE_EPSILON
        || (to.y - from.y).abs() > CAMERA_MOVE_EPSILON
        || (to.width - from.width).abs() > CAMERA_MOVE_EPSILON
        || (to.height - from.height).abs() > CAMERA_MOVE_EPSILON
}

fn extends_current_move(segment: &CameraMotionSegment, last_at: f64, now: f64) -> bool {
    segment.source == "live-recorded"
        && (segment.end - last_at).abs() < 0.01
        && now - last_at <= 0.45
        && now - segment.start <= MAX_MOTION_SEGMENT_SECS
}

impl RecordingManager {
    pub fn update_camera_preview_state(&self, update: CameraPreviewUpdate) -> Result<()> {
        let placement = CameraPlacement {
            x: update.window_x.clamp(0.0, 1.0),
            y: update.window_y.clamp(0.0, 1.0),
            width: update.window_width.clamp(0.05, 1.0),
            height: update.window_height.clamp(0.05, 1.0),
        };
        let corner_radius = update.corner_radius.clamp(0.0, 0.5);

        // The active session is the source of truth while recording; pending is snapshotted at start and written back at stop.
        let mut guard = self.session.lock();
        if let Some(session) = guard.as_mut() {
            let tracker = &mut session.camera_overlay;
            tracker.overlay.enabled = true;
            tracker.overlay.mirror = update.mirror;
            tracker.overlay.shape = update.shape;
            tracker.overlay.corner_radius = corner_radius;
            tracker.overlay.animation_preset = update.animation_preset;

            let now_secs = session.clock.effective_elapsed().as_secs_f64();
            if let (Some(last), Some(last_at)) =
                (tracker.last_placement.clone(), tracker.last_at_secs)
            {
                if camera_moved(&last, &placement) {
                    let can_extend = tracker
                        .overlay
                        .motion_segments
                        .last()
                        .map(|segment| extends_current_move(segment, last_at, now_secs))
                        .unwrap_or(false);

                    // Cap the segment list so a long session with sustained camera movement can't grow the Vec without limit.
                    const MAX_MOTION_SEGMENTS: usize = 4096;
                    let at_cap = tracker.overlay.motion_segments.len() >= MAX_MOTION_SEGMENTS;

                    if can_extend || at_cap {
                        if let Some(segment) = tracker.overlay.motion_segments.last_mut() {
                            segment.end = now_secs.max(segment.start + 0.001);
                            segment.to_x = placement.x;
                            segment.to_y = placement.y;
                            segment.to_width = placement.width;
                            segment.to_height = placement.height;
                        }
                    } else {
                        tracker.overlay.motion_segments.push(CameraMotionSegment {
                            start: last_at,
                            end: now_secs.max(last_at + 0.001),
                            from_x: last.x,
                            from_y: last.y,
                            from_width: last.width,
                            from_height: last.height,
                            to_x: placement.x,
                            to_y: placement.y,
                            to_width: placement.width,
                            to_height: placement.height,
                            ease_in: Default::default(),
                            ease_out: Default::default(),
                            source: "live-recorded".into(),
                        });
                    }
                    // Only an accepted move advances the reference, so a drag slower than the dead zone accumulates instead of being filtered away.
                    tracker.last_placement = Some(placement);
                }
            } else {
                tracker.overlay.default_placement = placement.clone();
                tracker.last_placement = Some(placement);
            }

            tracker.last_at_secs = Some(now_secs);
            return Ok(());
        }
        drop(guard);

        // Pre-recording: keep pending in sync so `start()` snapshots the user's latest preview state.
        let mut pending = self.pending_camera_overlay.lock();
        pending.enabled = true;
        pending.mirror = update.mirror;
        pending.shape = update.shape;
        pending.corner_radius = corner_radius;
        pending.animation_preset = update.animation_preset;
        pending.default_placement = placement;
        Ok(())
    }

    pub fn start(
        &self,
        mut target: CaptureTarget,
        output_dir: PathBuf,
        options: RecordingOptions,
        notify: impl Fn(CaptureNotice) + Send + Clone + 'static,
        mic_level: crate::audio::MicLevelSink,
    ) -> Result<Vec<String>> {
        let mut guard = self.session.lock();
        if guard.is_some() {
            return Err(anyhow!("recording is already running"));
        }

        // macOS Screen Recording TCC is separate from Accessibility; without it avfoundation yields zero frames and the take is silently empty.
        crate::permissions::ensure_screen_recording()?;

        std::fs::create_dir_all(&output_dir)?;
        // The pacer and encoder must agree on `recording_fps`, and the chosen rate is persisted into project metadata at stop().
        let recording_fps = resolve_recording_fps(options.fps);
        // 'auto' resolves against the probed encoder: hardware to High, software to Balanced. Explicit tiers pass through.
        let recording_quality = crate::encoder::RecordingQuality::resolve(
            options.quality.as_deref(),
            crate::ffmpeg::preferred_h264_encoder(),
        );
        log::info!("recording config: {recording_fps} fps, quality={recording_quality:?}");
        let started_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let stem = format!("recast-session-{started_at_unix_ms}");
        let recording_path = output_dir.join(format!("{stem}.recording.mp4"));
        let cursor_path = output_dir.join(format!("{stem}.cursor.json"));
        let audio_path = output_dir.join(format!("{stem}.audio.wav"));
        let microphone_path = output_dir.join(format!("{stem}.microphone.wav"));
        let camera_path = output_dir.join(format!("{stem}.camera.mp4"));
        let started_at = Instant::now();
        let clock = RecordingClock::new(started_at);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let pause_flag = Arc::new(AtomicBool::new(false));
        // Settled before the source opens: the writer decides where frames must live.
        let native = native_encoding_choice(&target, options.native_encoder);
        if let Some(reason) = native.refused {
            log::info!("native encoder not used: {reason}");
        }
        // Opened first: the backend is the authority on the frame size below.
        let source =
            crate::capture::create_capture_source(&target, recording_fps, native.frame_mode())?;
        target.adopt_source_size(source.width(), source.height());

        // Capped by memory, not frame count: 180 BGRA frames is ~6 GB at 4K and OOM'd low-end machines.
        let queue_capacity = queue_capacity_for(target.source.width, target.source.height);
        if !native.chosen {
            let frame_bytes = frame_bytes_bgra(target.source.width, target.source.height);
            log::info!(
                "recording pipeline queue: {queue_capacity} frames ({} MB at {}x{} BGRA)",
                (frame_bytes * queue_capacity as u64) / (1024 * 1024),
                target.source.width,
                target.source.height,
            );
        }
        let pipeline = RecordingPipeline::new(queue_capacity);
        let mut warnings = Vec::new();

        // Cursor sampling needs macOS Accessibility, but recording works without it, so the track just has gaps.
        if !crate::permissions::cursor_tracking_authorized() {
            warnings.push(
                "Cursor tracking is off — grant Recast in System Settings → \
                 Privacy & Security → Accessibility to capture cursor movement \
                 and clicks."
                    .to_string(),
            );
        }

        // Each track marks its first sample against `started_at`; stop() turns the differences into the muxer's offsets.
        let video_start = TrackStart::new(started_at);
        let audio_start = TrackStart::new(started_at);
        let microphone_start = TrackStart::new(started_at);
        let camera_start = TrackStart::new(started_at);
        let (sink, cadence) = writer_for(&native, &recording_path, recording_fps, &pipeline);
        let capture_handle = spawn_capture_loop(
            source,
            CaptureLoop {
                stop_flag: stop_flag.clone(),
                pause_flag: pause_flag.clone(),
                sink,
                cadence,
                timeline: clock.clone(),
                stats: pipeline.stats(),
                target_fps: recording_fps,
                video_start: video_start.clone(),
            },
            notify,
        )?;

        let encoder_handle = if native.chosen {
            None
        } else {
            match spawn_encoder_loop(
                EncoderConfig {
                    width: target.source.width,
                    height: target.source.height,
                    fps: recording_fps,
                    crop: target.crop_relative_to_source(),
                    output_path: recording_path.clone(),
                    quality: recording_quality,
                },
                stop_flag.clone(),
                pipeline.clone(),
            ) {
                Ok(handle) => Some(handle),
                Err(e) => {
                    // Signal + join the live capture thread so a failed start orphans nothing.
                    stop_flag.store(true, Ordering::Release);
                    let _ = capture_handle.join();
                    return Err(e);
                }
            }
        };

        // macOS re-prompts for Accessibility on an unguarded CoreGraphics read.
        let cursor_handle = if crate::permissions::cursor_tracking_authorized() {
            match spawn_cursor_capture(
                stop_flag.clone(),
                clock.clone(),
                video_start.clone(),
                CursorCaptureFrame {
                    origin_x: target.crop.x,
                    origin_y: target.crop.y,
                    width: target.crop.width,
                    height: target.crop.height,
                    // macOS samples the cursor in logical points while the video is physical pixels; 1.0 elsewhere leaves it unchanged.
                    scale: target.scale_factor,
                },
            ) {
                Ok(handle) => handle,
                Err(e) => {
                    // Capture and encoder are already live, so tear both down or a failed start orphans them.
                    stop_flag.store(true, Ordering::Release);
                    let _ = capture_handle.join();
                    if let Some(encoder) = encoder_handle {
                        let _ = encoder.join();
                    }
                    return Err(e);
                }
            }
        } else {
            // No-op placeholder keeping the session shape and `stop()`'s join unchanged; yields an empty cursor track.
            match std::thread::Builder::new()
                .name("recast-cursor-disabled".into())
                .spawn(CursorTrack::default)
            {
                Ok(handle) => handle,
                Err(e) => {
                    stop_flag.store(true, Ordering::Release);
                    let _ = capture_handle.join();
                    if let Some(encoder) = encoder_handle {
                        let _ = encoder.join();
                    }
                    return Err(anyhow!("failed to spawn cursor placeholder thread: {e}"));
                }
            }
        };

        // Gated on the toggle: loopback used to run unconditionally and recorded Recast's own playback, which is the preview echo.
        let audio_session = if options.system_audio {
            let session = AudioCaptureSession::start(AudioCaptureConfig {
                output_path: audio_path.clone(),
                pause_flag: pause_flag.clone(),
                start: audio_start.clone(),
            });
            // Asked for but unreachable; `stop` writes silence, so say so rather than deliver a mute track that looks captured.
            if session.is_none() {
                warnings.push(
                    "System audio could not be captured on this device, \
                     so the recording will have no system sound. Your \
                     microphone and video are not affected."
                        .to_string(),
                );
            }
            session
        } else {
            None
        };

        // Start microphone capture as a separate track.
        let microphone_session = if options.microphone {
            match MicrophoneCaptureSession::start(
                MicrophoneCaptureConfig {
                    output_path: microphone_path.clone(),
                    device_id: options.microphone_device_id.clone(),
                    pause_flag: pause_flag.clone(),
                    start: microphone_start.clone(),
                },
                Some(mic_level),
            ) {
                Ok(session) => {
                    warnings.extend(session.warnings());
                    Some(session)
                }
                Err(e) => {
                    log::warn!("microphone capture unavailable: {e}");
                    None
                }
            }
        } else {
            None
        };

        // Attach a sink to the preview's reader: a second open fails, cameras are exclusive.
        let mut camera_requested = options.camera;
        if camera_requested {
            if let Err(e) = crate::camera::session::attach_recorder(
                camera_path.clone(),
                camera_start.clone(),
                pause_flag.clone(),
            ) {
                log::warn!("camera recording unavailable: {e:#}");
                warnings.push(format!("Camera could not be recorded: {e}"));
                camera_requested = false;
            }
        }

        let mut camera_overlay = self.pending_camera_overlay.lock().clone();
        // Enable on intent so the editor shows the overlay while the file lands; stop() sets the authoritative value.
        camera_overlay.enabled = camera_requested;

        *guard = Some(RecordingSession {
            stop_flag,
            pause_flag,
            capture_handle,
            encoder_handle,
            cursor_handle,
            video_start,
            audio_session,
            audio_path,
            audio_start,
            microphone_session,
            microphone_start,
            camera_requested,
            camera_path,
            camera_start,
            pipeline,
            target,
            recording_path,
            cursor_path,
            clock,
            started_at_unix_ms,
            camera_overlay: CameraOverlayTracker {
                last_placement: Some(camera_overlay.default_placement.clone()),
                last_at_secs: Some(0.0),
                overlay: camera_overlay,
            },
            recording_fps,
        });
        Ok(warnings)
    }

    pub fn stop(&self) -> Result<RecordingArtifacts> {
        let mut guard = self.session.lock();
        let mut session = guard.take().context("recording is not running")?;
        drop(guard);

        session.stop_flag.store(true, Ordering::Release);

        // Join every thread and stop every session before propagating: an early `?` would orphan a thread plus a live FFmpeg child.
        let capture_join = session.capture_handle.join();
        let cursor_join = session.cursor_handle.join();
        let encoder_join = session.encoder_handle.take().map(JoinHandle::join);

        // No session means the toggle was off or no loopback was reachable; both write silence below, and neither is a captured track.
        let has_system_audio = session.audio_session.is_some();
        // Stopped however the threads fared: each session releases its own device.
        let audio_stop = session.audio_session.take().map(|s| s.stop());
        let microphone_stop = session.microphone_session.take().map(|s| s.stop());
        // Finish the file but leave the preview live; the device closes with the bubble.
        let camera_stop = session
            .camera_requested
            .then(crate::camera::session::detach_recorder);

        // Everything is reaped — now surface fatal thread failures.
        capture_join.map_err(|_| anyhow!("capture thread panicked"))??;
        let cursor_track = cursor_join.map_err(|_| anyhow!("cursor thread panicked"))?;
        if let Some(joined) = encoder_join {
            joined.map_err(|_| anyhow!("encoder thread panicked"))??;
        }

        // The cursor thread stamped in video time, so there is nothing to re-base.
        let video_zero_us = session.video_start.elapsed_us().unwrap_or(0);
        write_cursor_track(&session.cursor_path, &cursor_track)?;

        let track_offsets = TrackOffsets {
            audio_ms: offset_ms_from_video(video_zero_us, &session.audio_start),
            microphone_ms: offset_ms_from_video(video_zero_us, &session.microphone_start),
            camera_ms: offset_ms_from_video(video_zero_us, &session.camera_start),
        };
        log::info!(
            "track offsets vs video t0 ({}ms warmup): {:?}",
            video_zero_us / 1000,
            track_offsets
        );

        // A requested track that failed otherwise vanished silently: the recording succeeds minus that track.
        let mut warnings: Vec<String> = Vec::new();

        // The captured file, else a silence fallback so downstream always has a track to mux.
        let stopped_ok = matches!(audio_stop, Some(Ok(_)));
        let audio_path = match audio_stop {
            Some(Ok(path)) => path,
            other => {
                if let Some(Err(e)) = other {
                    log::warn!("audio capture stop failed, writing silence: {e}");
                }
                let duration = session.clock.effective_elapsed().as_secs_f64();
                crate::audio::wav::write_track_silence(&session.audio_path, duration)?;
                session.audio_path.clone()
            }
        };

        // WASAPI loopback delivers nothing while no app renders audio, and that header-only WAV breaks the export's filter graph.
        let mut has_samples = crate::audio::wav::wav_has_samples(&audio_path);
        if !has_samples {
            let duration = session.clock.effective_elapsed().as_secs_f64();
            log::info!(
                "system audio captured no samples ({}s of silence written instead)",
                duration.round()
            );
            crate::audio::wav::write_track_silence(&audio_path, duration)?;
            has_samples = false;
        }
        let (has_system_audio, system_warning) =
            system_audio_outcome(has_system_audio, stopped_ok, has_samples);
        if let Some(warning) = system_warning {
            warnings.push(warning.into());
        }

        // Microphone path if its capture succeeded.
        let microphone_path = match microphone_stop {
            Some(Ok(path)) if crate::audio::wav::wav_has_samples(&path) => Some(path),
            Some(Ok(path)) => {
                log::warn!("microphone produced no samples: {}", path.display());
                warnings.push(
                    "The microphone was selected but recorded no sound, so the                      recording has no mic track. Check it isn't muted or in use                      by another app."
                        .to_string(),
                );
                None
            }
            Some(Err(e)) => {
                log::warn!("microphone capture stop failed: {e}");
                warnings.push(
                    "Microphone could not be captured, so the recording has no mic \
                     track. Check the microphone is connected and not in use (on \
                     macOS, that Recast has Microphone permission)."
                        .to_string(),
                );
                None
            }
            None => None,
        };

        // Joined above, so complete; size-checked because a dead camera leaves a header.
        if let Some(Err(e)) = camera_stop {
            log::warn!("camera recording failed: {e:#}");
        }
        let camera_path = if session.camera_requested {
            match std::fs::metadata(&session.camera_path) {
                Ok(m) if m.len() >= 1024 => Some(session.camera_path.clone()),
                _ => {
                    warnings.push(
                        "Camera could not be captured, so the recording has no camera \
                         track. Check the webcam is connected and not in use by another \
                         app (on macOS, that Recast has Camera permission)."
                            .to_string(),
                    );
                    None
                }
            }
        } else {
            None
        };
        // A delivered track is the authoritative signal the overlay should paint.
        session.camera_overlay.overlay.enabled = camera_path.is_some();

        let stats = build_stats(
            &session.pipeline,
            session.clock.effective_elapsed().as_millis() as u64,
            session.recording_fps,
        );

        // Persist overlay settings back to pending so the next recording inherits them; motion_segments stay session-local.
        {
            let final_overlay = &session.camera_overlay.overlay;
            let mut pending = self.pending_camera_overlay.lock();
            pending.mirror = final_overlay.mirror;
            pending.shape = final_overlay.shape.clone();
            pending.corner_radius = final_overlay.corner_radius;
            pending.animation_preset = final_overlay.animation_preset.clone();
            pending.default_placement = final_overlay.default_placement.clone();
        }

        Ok(RecordingArtifacts {
            capture_target: session.target,
            recording_path: session.recording_path,
            cursor_path: session.cursor_path,
            audio_path,
            has_system_audio,
            microphone_path,
            camera_path,
            camera_requested: session.camera_requested,
            camera_overlay: session.camera_overlay.overlay,
            track_offsets,
            started_at_unix_ms: session.started_at_unix_ms,
            stats,
            warnings,
        })
    }

    /// Pause the active recording. Capture, cursor, and audio threads stop
    /// producing samples; the pause-aware clock freezes. Idempotent.
    pub fn pause(&self) -> Result<()> {
        let guard = self.session.lock();
        let session = guard.as_ref().context("recording is not running")?;
        if session.clock.is_paused() {
            return Ok(());
        }
        session.pause_flag.store(true, Ordering::Release);
        session.clock.pause();
        Ok(())
    }

    /// Resume a paused recording. Idempotent.
    pub fn resume(&self) -> Result<()> {
        let guard = self.session.lock();
        let session = guard.as_ref().context("recording is not running")?;
        if !session.clock.is_paused() {
            return Ok(());
        }
        // Bank the pause duration before letting threads run again, so they wake into a correct clock.
        session.clock.resume();
        session.pause_flag.store(false, Ordering::Release);
        Ok(())
    }

    /// Whether a recording is currently active and paused.
    pub fn is_paused(&self) -> bool {
        self.session
            .lock()
            .as_ref()
            .map(|s| s.clock.is_paused())
            .unwrap_or(false)
    }

    /// Whether a capture session is live (between a successful start and stop).
    /// Authoritative engine state, unlike the frontend-mirrored tray flag.
    pub fn is_recording(&self) -> bool {
        self.session.lock().is_some()
    }
}

fn build_stats(pipeline: &RecordingPipeline, duration_ms: u64, nominal_fps: u32) -> RecordingStats {
    let PipelineSnapshot {
        captured_frames,
        dropped_frames,
        encoded_frames,
    } = pipeline.stats().snapshot();

    RecordingStats {
        captured_frames,
        encoded_frames,
        dropped_frames,
        duration_ms,
        nominal_fps,
    }
}

#[cfg(test)]
mod system_audio_tests {
    use super::*;

    /// The failing arms all write silence, and that file has samples, so the
    /// metadata used to claim a captured system-audio track that was silent and
    /// warn about nothing.
    #[test]
    fn a_capture_that_failed_is_not_reported_as_a_track() {
        assert_eq!(
            system_audio_outcome(true, false, true),
            (false, Some("System audio could not be recorded."))
        );
    }

    /// Loopback delivers nothing while no app renders sound. That is not a
    /// failure worth warning about, but it is not a track either.
    #[test]
    fn a_capture_with_no_sound_in_it_is_not_a_track_and_is_not_a_warning() {
        assert_eq!(system_audio_outcome(true, true, false), (false, None));
    }

    #[test]
    fn a_capture_that_worked_is_a_track() {
        assert_eq!(system_audio_outcome(true, true, true), (true, None));
    }

    /// The toggle was off or no loopback was reachable; the user was already
    /// told at start, so the stop must not tell them again.
    #[test]
    fn a_track_that_was_never_requested_warns_about_nothing() {
        assert_eq!(system_audio_outcome(false, false, false), (false, None));
        assert_eq!(system_audio_outcome(false, true, true), (false, None));
    }
}

#[cfg(test)]
mod camera_motion_tests {
    use super::*;

    fn place(x: f64, y: f64) -> CameraPlacement {
        CameraPlacement {
            x,
            y,
            width: 0.16,
            height: 0.29,
        }
    }

    fn segment(start: f64, end: f64) -> CameraMotionSegment {
        CameraMotionSegment {
            start,
            end,
            from_x: 1.0,
            from_y: 0.86,
            from_width: 0.16,
            from_height: 0.29,
            to_x: 0.79,
            to_y: 0.5,
            to_width: 0.16,
            to_height: 0.29,
            ease_in: Default::default(),
            ease_out: Default::default(),
            source: "live-recorded".into(),
        }
    }

    /// The preview geometry jitters in the last few bits. Treating that as a
    /// move let the coalescing window fold a whole take into ONE segment, which
    /// replays as an eased glide across the entire recording.
    #[test]
    fn sub_pixel_jitter_is_not_a_move() {
        assert!(!camera_moved(&place(1.0, 0.86), &place(1.0004, 0.8603)));
        assert!(!camera_moved(&place(0.5, 0.5), &place(0.5, 0.5)));
    }

    #[test]
    fn a_real_drag_is_a_move() {
        assert!(camera_moved(&place(0.5, 0.5), &place(0.52, 0.5)));
        assert!(camera_moved(&place(0.5, 0.5), &place(0.5, 0.52)));
    }

    /// Resizing counts too, or a pinch would record as no movement at all.
    #[test]
    fn a_size_change_alone_is_a_move() {
        let mut bigger = place(0.5, 0.5);
        bigger.width += 0.05;
        assert!(camera_moved(&place(0.5, 0.5), &bigger));
    }

    #[test]
    fn a_continuing_drag_extends_the_open_segment() {
        assert!(extends_current_move(&segment(1.0, 1.4), 1.4, 1.5));
    }

    /// A pause between moves starts a new segment rather than gliding the
    /// bubble across the gap.
    #[test]
    fn a_gap_longer_than_the_drag_window_starts_a_new_segment() {
        assert!(!extends_current_move(&segment(1.0, 1.4), 1.4, 2.5));
    }

    /// The backstop for the whole-take segment: even an unbroken stream of
    /// moves stops extending one segment eventually.
    #[test]
    fn a_segment_stops_growing_once_it_is_long_enough() {
        let long = segment(0.15, 0.15 + MAX_MOTION_SEGMENT_SECS + 0.1);
        let last_at = long.end;
        assert!(!extends_current_move(&long, last_at, last_at + 0.1));
    }

    #[test]
    fn an_authored_segment_is_never_extended_by_a_live_move() {
        let mut authored = segment(1.0, 1.4);
        authored.source = "manual".into();
        assert!(!extends_current_move(&authored, 1.4, 1.5));
    }
}

#[cfg(test)]
mod options_tests {
    use super::*;

    // Guards the serde bridge the echo fix depends on: `systemAudio` must land on `system_audio` and gate loopback.
    #[test]
    fn system_audio_toggle_deserializes_from_camel_case() {
        let off: RecordingOptions = serde_json::from_str(r#"{"systemAudio": false}"#).unwrap();
        assert!(
            !off.system_audio,
            "systemAudio:false must disable system audio"
        );

        let on: RecordingOptions = serde_json::from_str(r#"{"systemAudio": true}"#).unwrap();
        assert!(on.system_audio);
    }

    #[test]
    fn system_audio_defaults_on_when_omitted() {
        // A profile or older client that omits the field keeps the historical capture-by-default behaviour.
        let opts: RecordingOptions = serde_json::from_str("{}").unwrap();
        assert!(opts.system_audio);
        assert!(RecordingOptions::default().system_audio);
    }
}

/// Cross-platform (runs on every CI leg AND locally on Windows).
#[cfg(test)]
mod session_tests {
    use super::*;
    use crate::capture::{CaptureArea, CaptureKind};

    fn area(x: i32, y: i32, width: u32, height: u32) -> CaptureArea {
        CaptureArea {
            x,
            y,
            width,
            height,
        }
    }

    fn target(kind: CaptureKind, source: CaptureArea, crop: CaptureArea) -> CaptureTarget {
        CaptureTarget {
            kind,
            id: 1,
            label: "test".into(),
            source,
            crop,
            display_id: 1,
            scale_factor: 1.0,
        }
    }

    /// The CaptureSource contract: the encoder is configured for `source` dimensions and crops with THIS rectangle, so backends must emit full-source frames.
    /// X11 used to pre-crop, so the encoder cropped an already-cropped buffer and corrupted region recordings. Offsets are source-relative, not virtual-desktop.
    #[test]
    fn crop_is_reported_relative_to_the_captured_source() {
        // The source is the second monitor, so desktop-space (2000, 50) must become source-relative (80, 50).
        let t = target(
            CaptureKind::Region,
            area(1920, 0, 1920, 1080),
            area(2000, 50, 800, 600),
        );
        let crop = t.crop_relative_to_source().expect("region must crop");
        assert_eq!((crop.x, crop.y), (80, 50));
        assert_eq!((crop.width, crop.height), (800, 600));
    }

    /// Full-display capture needs no crop filter at all.
    #[test]
    fn full_display_capture_needs_no_crop() {
        let full = area(0, 0, 1920, 1080);
        let t = target(CaptureKind::Display, full, full);
        assert!(t.crop_relative_to_source().is_none());
    }

    /// `Drop for RecordingManager` takes the session lock to reap a live
    /// recording. Dropping an idle manager must be a silent no-op: this pins
    /// that it neither panics nor deadlocks on its own mutex (a deadlock would
    /// hang this test rather than fail it).
    #[test]
    fn dropping_an_idle_manager_is_a_noop() {
        let manager = RecordingManager::default();
        assert!(!manager.is_recording());
        drop(manager);
    }
}

/// A/V sync against the real capture stack with every source enabled, Phase 5's exit criterion.
/// The per-track unit tests cover the arithmetic; only this catches the tracks disagreeing about when zero is, since each is opened by a different OS API on its own schedule.
#[cfg(test)]
mod sync_live_tests {
    use super::*;

    /// Opening a capture device legitimately takes a few hundred milliseconds (~364ms for WASAPI loopback, ~739ms for the microphone), and that latency is corrected downstream.
    /// So this bound catches a garbage clock rather than the latency itself: past a second and a half, nothing plausible is being measured.
    const PLAUSIBLE_OPEN_MS: i64 = 1_500;

    /// The zero-copy writer driven by the REAL recorder: nothing else proves the capture loop hands it GPU handles, that `stop()` closes the file, or that the result plays.
    /// Sets a process-wide opt-in, so it must not run beside another recording test.
    #[cfg(windows)]
    #[test]
    #[ignore = "live: records the real screen through the GPU encoder"]
    fn the_native_writer_records_a_playable_variable_rate_file() {
        if !capturekit::capabilities().display_enumeration || !crate::encoder::native::available() {
            return;
        }
        let displays = capturekit::displays().expect("displays enumerate");
        let Some(display) = displays.iter().find(|d| d.is_primary).or(displays.first()) else {
            return;
        };
        let target = CaptureTarget::resolve("screen", display.id.0).expect("the display resolves");
        let out = std::env::temp_dir().join("recast-native-live");
        let _ = std::fs::create_dir_all(&out);

        std::env::set_var(NATIVE_ENCODER_ENV, "1");
        let manager = RecordingManager::default();
        let started = manager.start(
            target,
            out,
            RecordingOptions::default(),
            |_| {},
            std::sync::Arc::new(|_: f32| {}),
        );
        let artifacts = started.and_then(|_| {
            std::thread::sleep(std::time::Duration::from_secs(4));
            manager.stop()
        });
        std::env::remove_var(NATIVE_ENCODER_ENV);
        let artifacts = artifacts.expect("the recording runs");

        let probe = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join("ffprobe-x86_64-pc-windows-msvc.exe");
        assert!(probe.exists(), "no ffprobe at {}", probe.display());
        let out = std::process::Command::new(&probe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "packet=pts_time",
                "-of",
                "csv=p=0",
            ])
            .arg(&artifacts.recording_path)
            .stdin(std::process::Stdio::null())
            .output()
            .expect("ffprobe runs");
        let times: Vec<f64> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|line| line.trim().trim_end_matches(',').parse().ok())
            .collect();
        assert!(
            times.len() > 10,
            "4 seconds of recording produced {} packets",
            times.len()
        );

        let span = times.last().expect("packets") - times[0];
        assert!(
            (2.5..5.5).contains(&span),
            "a 4s recording spans {span}s of presentation time"
        );
        // What separates this writer: gaps are kept, not rounded to a slot.
        let gaps: Vec<i64> = times
            .windows(2)
            .map(|pair| ((pair[1] - pair[0]) * 1_000_000.0).round() as i64)
            .collect();
        let distinct: std::collections::HashSet<i64> = gaps.iter().copied().collect();
        assert!(
            distinct.len() > 1,
            "every gap is identical ({distinct:?}), which is a fixed rate"
        );

        // NVIDIA's default GOP is infinite: one keyframe for a whole recording.
        let keyed = std::process::Command::new(&probe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-skip_frame",
                "nokey",
                "-show_entries",
                "frame=pts_time",
                "-of",
                "csv=p=0",
            ])
            .arg(&artifacts.recording_path)
            .stdin(std::process::Stdio::null())
            .output()
            .expect("ffprobe runs");
        let keyframes = String::from_utf8_lossy(&keyed.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count();
        assert!(
            keyframes >= 4,
            "a {span:.1}s recording holds {keyframes} keyframes, so seeking decodes from far back"
        );
    }

    #[test]
    #[ignore = "live: records the real screen and opens the mic and camera"]
    fn every_enabled_track_lands_on_the_video_clock() {
        if !capturekit::capabilities().display_enumeration {
            return;
        }
        let displays = capturekit::displays().expect("displays enumerate");
        let Some(display) = displays.iter().find(|d| d.is_primary).or(displays.first()) else {
            return;
        };
        // The real id: a made-up one failed silently and passed this in 60ms.
        let target = CaptureTarget::resolve("screen", display.id.0).expect("the display resolves");
        let out = std::env::temp_dir().join("recast-sync-live");
        let _ = std::fs::create_dir_all(&out);

        let options = RecordingOptions {
            system_audio: true,
            microphone: true,
            camera: true,
            ..RecordingOptions::default()
        };
        let manager = RecordingManager::default();
        let warnings = manager
            .start(
                target,
                out,
                options,
                |_| {},
                std::sync::Arc::new(|_: f32| {}),
            )
            .expect("the recording starts");
        std::thread::sleep(std::time::Duration::from_secs(4));
        let artifacts = manager.stop().expect("the recording stops");

        // A source this machine lacks is reported, never silently dropped.
        for warning in &warnings {
            eprintln!("source warning: {warning}");
        }
        // Without real video there is no clock to measure the other tracks against.
        let recorded = std::fs::metadata(&artifacts.recording_path)
            .expect("the recording file exists")
            .len();
        assert!(
            recorded > 0,
            "the recording is empty, so nothing was captured"
        );

        let offsets = &artifacts.track_offsets;
        eprintln!("recorded {recorded} bytes; track offsets vs video t0: {offsets:?}");

        let mut measured = 0;
        for (name, offset) in [
            ("system audio", offsets.audio_ms),
            ("microphone", offsets.microphone_ms),
            ("camera", offsets.camera_ms),
        ] {
            // A real absence, named in the warnings above, not a code failure.
            let Some(offset) = offset else {
                eprintln!("{name}: no samples, skipped");
                continue;
            };
            measured += 1;
            // An unmeasured track is assumed aligned and drifts by its open time.
            assert!(
                offset.abs() <= PLAUSIBLE_OPEN_MS,
                "{name} reports {offset}ms from video t0, which is not a device open latency"
            );
        }
        assert!(
            measured >= 2,
            "only {measured} track(s) reported an offset; the harness proves nothing about sync"
        );
    }
}

#[cfg(test)]
mod native_choice_tests {
    use super::{native_opt_in, native_refusal};

    #[test]
    fn every_condition_met_takes_the_native_path() {
        assert_eq!(native_refusal(true, true, true, false), None);
    }

    /// Opting in cannot conjure an encoder, a platform or a crop-free capture.
    #[test]
    fn each_missing_condition_refuses_with_its_own_reason() {
        let reasons = [
            native_refusal(false, true, true, false),
            native_refusal(true, false, true, false),
            native_refusal(true, true, false, false),
            native_refusal(true, true, true, true),
        ];
        assert!(reasons.iter().all(Option::is_some), "{reasons:?}");
        let distinct: std::collections::HashSet<_> = reasons.iter().collect();
        assert_eq!(distinct.len(), 4, "each refusal owes its own reason");
    }

    /// Unset, the setting decides. This is the shipping path: a user who has
    /// never heard of the env var gets exactly what Settings says.
    #[test]
    fn without_the_override_the_setting_decides() {
        assert!(native_opt_in(true, None));
        assert!(!native_opt_in(false, None));
    }

    /// The override forces BOTH ways. An override that could only turn the writer on would leave no way to reproduce an FFmpeg-path bug on a machine whose settings have it enabled.
    #[test]
    fn the_env_override_wins_in_both_directions() {
        assert!(native_opt_in(false, Some("1")), "1 did not force it on");
        assert!(!native_opt_in(true, Some("0")), "0 did not force it off");
    }

    /// A typo must not silently flip the encoder. Anything that is not an
    /// explicit 1 or 0 defers to the setting rather than guessing.
    #[test]
    fn an_unrecognised_override_defers_to_the_setting() {
        for value in ["", "true", "yes", "2", "on"] {
            assert!(
                native_opt_in(true, Some(value)),
                "{value:?} overrode a true setting"
            );
            assert!(
                !native_opt_in(false, Some(value)),
                "{value:?} overrode a false setting"
            );
        }
    }

    /// An unsupported platform is reported as such even when nothing else is
    /// set up either, because that is the fact the user can do nothing about.
    #[test]
    fn the_platform_is_reported_before_anything_the_user_could_change() {
        assert_eq!(
            native_refusal(false, false, false, true),
            Some("the GPU writer is Windows-only so far")
        );
    }
}
