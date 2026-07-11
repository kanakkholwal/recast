pub mod pipeline;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use xcap::{Monitor, Window};

use crate::audio::{
    AudioCaptureConfig, AudioCaptureSession, MicrophoneCaptureConfig, MicrophoneCaptureSession,
};
use crate::cursor::{
    shift_cursor_track, spawn_cursor_capture, write_cursor_track, CursorCaptureFrame, CursorTrack,
};
use crate::encoder::h264::{self, EncodePurpose, H264Encoder};
use crate::encoder::{spawn_encoder_loop, EncoderConfig};
use crate::render::node_types::{CameraMotionSegment, CameraOverlaySettings, CameraPlacement};
use pipeline::{spawn_capture_loop, PipelineSnapshot, RecordingPipeline};

/// Frames per second emitted by the capture pacer and declared to the encoder.
/// The pacer emits exactly this many frames per real-time second, and the
/// encoder hands FFmpeg the same number as `-framerate`. Together they
/// guarantee 1 second of wall-clock recording → 1 second of video PTS — the
/// invariant the cursor track (timestamped in wall-clock μs) relies on for
/// sync.
pub const RECORDING_FPS: u32 = 60;

//  Pause-aware recording clock

/// A wall-clock timer that can be paused. `effective_elapsed` reports elapsed
/// time *minus* every interval spent paused, so all capture tracks (video
/// pacer, cursor, audio) stay on one gap-free timeline across pause/resume.
#[derive(Clone)]
pub struct RecordingClock {
    start: Instant,
    /// Total time (µs) spent in completed pause intervals.
    paused_total_us: Arc<AtomicU64>,
    /// `Some(instant)` while a pause is currently in progress.
    paused_since: Arc<Mutex<Option<Instant>>>,
    /// Completed pause intervals as `(start_us, end_us)` offsets from
    /// `start`, in *real* wall-clock time — used to cut paused spans out
    /// of the continuously-recorded camera video.
    pause_intervals: Arc<Mutex<Vec<(u64, u64)>>>,
}

impl RecordingClock {
    fn new(start: Instant) -> Self {
        Self {
            start,
            paused_total_us: Arc::new(AtomicU64::new(0)),
            paused_since: Arc::new(Mutex::new(None)),
            pause_intervals: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Wall-clock time since start, excluding all paused intervals.
    pub fn effective_elapsed(&self) -> Duration {
        let raw = self.start.elapsed();
        let banked = Duration::from_micros(self.paused_total_us.load(Ordering::Acquire));
        let live = self
            .paused_since
            .lock()
            .map(|since| since.elapsed())
            .unwrap_or_default();
        raw.saturating_sub(banked).saturating_sub(live)
    }

    pub fn is_paused(&self) -> bool {
        self.paused_since.lock().is_some()
    }

    /// Begin a pause interval. Idempotent — a second call while already
    /// paused is a no-op.
    fn pause(&self) {
        let mut slot = self.paused_since.lock();
        if slot.is_none() {
            *slot = Some(Instant::now());
        }
    }

    /// End the current pause interval, banking its duration and recording
    /// it in `pause_intervals`. No-op if not currently paused.
    fn resume(&self) {
        let mut slot = self.paused_since.lock();
        if let Some(since) = slot.take() {
            let dur = since.elapsed();
            let start_us = since.duration_since(self.start).as_micros() as u64;
            let end_us = start_us + dur.as_micros() as u64;
            self.paused_total_us
                .fetch_add(dur.as_micros() as u64, Ordering::AcqRel);
            self.pause_intervals.lock().push((start_us, end_us));
        }
    }

    /// All pause intervals as real-time `(start_us, end_us)` offsets from
    /// recording start. Includes an in-progress pause (closed at "now"),
    /// so it's correct even when called after a stop-while-paused.
    pub fn pause_intervals(&self) -> Vec<(u64, u64)> {
        let in_progress = *self.paused_since.lock();
        let stored = self.pause_intervals.lock();
        let mut list = Vec::with_capacity(stored.len() + in_progress.is_some() as usize);
        list.extend_from_slice(&stored);
        drop(stored);
        if let Some(since) = in_progress {
            let start_us = since.duration_since(self.start).as_micros() as u64;
            let end_us = start_us + since.elapsed().as_micros() as u64;
            list.push((start_us, end_us));
        }
        list
    }
}

//  Shared types

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureKind {
    Display,
    Window,
    Region,
}

/// Pixel-space rectangle in virtual desktop coordinates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureTarget {
    pub kind: CaptureKind,
    pub id: u32,
    pub label: String,
    pub source: CaptureArea,
    pub crop: CaptureArea,
    /// CGDirectDisplayID / xcap monitor id of the display being captured. For
    /// `Window` targets this is the display the window sits on (distinct from
    /// `id`, which is the window id). macOS uses it to pick the matching
    /// AVFoundation "Capture screen N"; other platforms ignore it.
    #[serde(default)]
    pub display_id: u32,
    /// Backing scale factor (physical ÷ logical) of `display_id`. `source` and
    /// `crop` are stored in *physical device pixels* — on macOS that means the
    /// xcap-logical values were multiplied by this; on Windows/Linux xcap
    /// already reports physical, so this is 1.0. The cursor track uses it to
    /// lift its logical samples into the same physical space as the video.
    #[serde(default = "default_scale_factor")]
    pub scale_factor: f32,
}

fn default_scale_factor() -> f32 {
    1.0
}

/// Backing scale factor of a monitor — physical pixels per logical point.
/// Only macOS needs it (AVFoundation captures physical while xcap reports
/// logical); elsewhere xcap dimensions are already physical, so 1.0.
#[cfg(target_os = "macos")]
fn display_scale_factor(monitor: &Monitor) -> f32 {
    monitor.scale_factor().unwrap_or(1.0).max(1.0)
}
#[cfg(not(target_os = "macos"))]
fn display_scale_factor(_monitor: &Monitor) -> f32 {
    1.0
}

/// Scale a rectangle by `scale`, keeping width/height even (libx264 requires
/// it) and at least 2px.
fn scale_area(a: CaptureArea, scale: f64) -> CaptureArea {
    CaptureArea {
        x: (a.x as f64 * scale).round() as i32,
        y: (a.y as f64 * scale).round() as i32,
        width: (((a.width as f64 * scale).round() as u32) & !1).max(2),
        height: (((a.height as f64 * scale).round() as u32) & !1).max(2),
    }
}

/// Lift a freshly-resolved target's xcap-logical `source`/`crop` into the
/// physical device pixels AVFoundation actually delivers, and record the
/// factor for the cursor track. A no-op at scale 1.0 (Windows/Linux, where
/// xcap already reports physical), so those platforms stay byte-for-byte
/// unchanged.
fn apply_device_scale(target: &mut CaptureTarget, scale: f32) {
    target.scale_factor = scale;
    if (scale - 1.0).abs() < 1e-3 {
        return;
    }
    let s = scale as f64;
    target.source = scale_area(target.source, s);
    let mut crop = scale_area(target.crop, s);
    // Rounding can nudge the scaled crop a pixel past the scaled source; clamp
    // so the encoder's crop filter never exceeds the captured frame.
    let max_x = target.source.x + target.source.width as i32;
    let max_y = target.source.y + target.source.height as i32;
    crop.x = crop.x.clamp(target.source.x, max_x);
    crop.y = crop.y.clamp(target.source.y, max_y);
    let avail_w = (max_x - crop.x).max(2) as u32;
    let avail_h = (max_y - crop.y).max(2) as u32;
    crop.width = (crop.width.min(avail_w)) & !1;
    crop.height = (crop.height.min(avail_h)) & !1;
    target.crop = crop;
}

impl CaptureTarget {
    pub fn resolve(target_type: &str, target_id: u32) -> Result<Self> {
        match target_type {
            "window" => resolve_window_target(target_id),
            _ => resolve_display_target(target_id),
        }
    }

    pub fn resolve_region(rect: RegionRect) -> Result<Self> {
        resolve_region_target(rect)
    }

    pub fn crop_relative_to_source(&self) -> Option<CaptureArea> {
        if self.crop.x == self.source.x
            && self.crop.y == self.source.y
            && self.crop.width == self.source.width
            && self.crop.height == self.source.height
        {
            None
        } else {
            Some(CaptureArea {
                x: self.crop.x - self.source.x,
                y: self.crop.y - self.source.y,
                width: self.crop.width,
                height: self.crop.height,
            })
        }
    }
}

fn resolve_display_target(target_id: u32) -> Result<CaptureTarget> {
    let display = Monitor::all()?
        .into_iter()
        .find(|monitor| monitor.id().ok() == Some(target_id))
        .context("display target not found")?;

    let area = CaptureArea {
        x: display.x().unwrap_or_default(),
        y: display.y().unwrap_or_default(),
        width: display.width().unwrap_or_default(),
        height: display.height().unwrap_or_default(),
    };

    let mut target = CaptureTarget {
        kind: CaptureKind::Display,
        id: target_id,
        display_id: target_id,
        label: display.name().unwrap_or_else(|_| "Display".into()),
        source: area,
        crop: area,
        scale_factor: 1.0,
    };
    apply_device_scale(&mut target, display_scale_factor(&display));
    Ok(target)
}

fn resolve_window_target(target_id: u32) -> Result<CaptureTarget> {
    let window = Window::all()?
        .into_iter()
        .find(|candidate| candidate.id().ok() == Some(target_id))
        .context("window target not found")?;

    let crop = CaptureArea {
        x: window.x().unwrap_or_default(),
        y: window.y().unwrap_or_default(),
        width: window.width().unwrap_or_default(),
        height: window.height().unwrap_or_default(),
    };
    let center_x = crop.x + (crop.width as i32 / 2);
    let center_y = crop.y + (crop.height as i32 / 2);

    let source_monitor = Monitor::all()?
        .into_iter()
        .find(|monitor| {
            let x = monitor.x().unwrap_or_default();
            let y = monitor.y().unwrap_or_default();
            let width = monitor.width().unwrap_or_default() as i32;
            let height = monitor.height().unwrap_or_default() as i32;
            center_x >= x && center_x < x + width && center_y >= y && center_y < y + height
        })
        .context("unable to locate the display containing the selected window")?;

    let source = CaptureArea {
        x: source_monitor.x().unwrap_or_default(),
        y: source_monitor.y().unwrap_or_default(),
        width: source_monitor.width().unwrap_or_default(),
        height: source_monitor.height().unwrap_or_default(),
    };

    let mut target = CaptureTarget {
        kind: CaptureKind::Window,
        id: target_id,
        display_id: source_monitor.id().unwrap_or_default(),
        label: window.title().unwrap_or_else(|_| "Window".into()),
        source,
        crop,
        scale_factor: 1.0,
    };
    apply_device_scale(&mut target, display_scale_factor(&source_monitor));
    Ok(target)
}

fn resolve_region_target(rect: RegionRect) -> Result<CaptureTarget> {
    if rect.width == 0 || rect.height == 0 {
        return Err(anyhow!("region must have non-zero width and height"));
    }

    let center_x = rect.x + (rect.width as i32 / 2);
    let center_y = rect.y + (rect.height as i32 / 2);

    let monitor = Monitor::all()?
        .into_iter()
        .find(|monitor| {
            // The frontend Region Picker passes coordinates in PHYSICAL pixels,
            // but xcap's Monitor bounds are in LOGICAL pixels.
            // We must un-scale the physical center point to find the matching monitor.
            let scale = display_scale_factor(monitor) as f32;
            let cx = (center_x as f32 / scale).round() as i32;
            let cy = (center_y as f32 / scale).round() as i32;

            let x = monitor.x().unwrap_or_default();
            let y = monitor.y().unwrap_or_default();
            let width = monitor.width().unwrap_or_default() as i32;
            let height = monitor.height().unwrap_or_default() as i32;
            cx >= x && cx < x + width && cy >= y && cy < y + height
        })
        .context("unable to locate the display containing the selected region")?;

    let scale = display_scale_factor(&monitor);
    let scale_f32 = scale as f32;

    // Convert the physical rect back to logical pixels so it shares the same
    // coordinate space as the monitor for clamping.
    let logical_x = (rect.x as f32 / scale_f32).round() as i32;
    let logical_y = (rect.y as f32 / scale_f32).round() as i32;
    let logical_w = (rect.width as f32 / scale_f32).round() as u32;
    let logical_h = (rect.height as f32 / scale_f32).round() as u32;

    let monitor_id = monitor.id().unwrap_or_default();
    let mon_x = monitor.x().unwrap_or_default();
    let mon_y = monitor.y().unwrap_or_default();
    let mon_w = monitor.width().unwrap_or_default();
    let mon_h = monitor.height().unwrap_or_default();

    let source = CaptureArea {
        x: mon_x,
        y: mon_y,
        width: mon_w,
        height: mon_h,
    };

    // Clamp the requested logical region to the source monitor's bounds
    let clamped_x = logical_x.max(mon_x).min(mon_x + mon_w as i32);
    let clamped_y = logical_y.max(mon_y).min(mon_y + mon_h as i32);
    let max_w = (mon_x + mon_w as i32 - clamped_x).max(0) as u32;
    let max_h = (mon_y + mon_h as i32 - clamped_y).max(0) as u32;

    // Encoder libx264 requires even dimensions.
    let crop_w = (logical_w.min(max_w)) & !1u32;
    let crop_h = (logical_h.min(max_h)) & !1u32;
    if crop_w == 0 || crop_h == 0 {
        return Err(anyhow!("region collapsed to zero after clamping"));
    }

    let crop = CaptureArea {
        x: clamped_x,
        y: clamped_y,
        width: crop_w,
        height: crop_h,
    };

    let mut target = CaptureTarget {
        kind: CaptureKind::Region,
        id: monitor_id,
        display_id: monitor_id,
        // Use the original physical dimensions for the display label, ensuring they are even
        label: format!("Area {}×{}", rect.width & !1u32, rect.height & !1u32),
        source,
        crop,
        scale_factor: 1.0,
    };

    // This scales both source and crop back up to physical pixels for the encoder!
    apply_device_scale(&mut target, scale);
    Ok(target)
}
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
    pub camera_overlay: CameraOverlaySettings,
    pub started_at_unix_ms: u64,
    pub stats: RecordingStats,
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
    #[serde(default)]
    pub microphone_device_id: Option<String>,
    /// Capture camera video.
    #[serde(default)]
    pub camera: bool,
    /// Camera device ID / DirectShow device name (None = first available).
    #[serde(default)]
    pub camera_device_id: Option<String>,
    /// Capture frame rate. `None` (or out of the supported 24..=240 range)
    /// falls back to [`RECORDING_FPS`]. The pacer and encoder both run at this
    /// rate; values above the monitor's refresh just duplicate frames, so the
    /// UI gates the offered options by the detected display refresh.
    #[serde(default)]
    pub fps: Option<u32>,
    /// Capture quality tier — `"auto"` (default), `"balanced"`, `"high"`, or
    /// `"pristine"`. `"auto"`/unknown resolve against the detected encoder
    /// (hardware → high, software → balanced). See
    /// [`crate::encoder::RecordingQuality::resolve`].
    #[serde(default)]
    pub quality: Option<String>,
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
        }
    }
}

/// Clamp a requested capture frame rate to a sane range, falling back to the
/// default when unset or out of range. The lower bound matches the lowest
/// cinematic rate; the upper bound covers high-refresh panels (240 Hz) while
/// rejecting absurd values that would blow the queue budget.
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

#[derive(Clone)]
struct CameraOverlayTracker {
    overlay: CameraOverlaySettings,
    last_placement: Option<CameraPlacement>,
    last_at_secs: Option<f64>,
}

struct RecordingSession {
    stop_flag: Arc<AtomicBool>,
    /// Set while the recording is paused — capture/audio threads skip work.
    pause_flag: Arc<AtomicBool>,
    capture_handle: JoinHandle<Result<()>>,
    encoder_handle: JoinHandle<Result<()>>,
    cursor_handle: JoinHandle<CursorTrack>,
    /// Wall-clock μs from recording start to the first encoded video frame
    /// (capture-source warmup). Subtracted from the cursor track at `stop()`
    /// so cursor t=0 aligns with video frame 0.
    first_frame_offset_us: Arc<AtomicU64>,
    audio_session: Option<AudioCaptureSession>,
    audio_path: PathBuf,
    microphone_session: Option<MicrophoneCaptureSession>,
    camera_session: Option<crate::camera::CameraCaptureSession>,
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

impl RecordingManager {
    pub fn update_camera_preview_state(&self, update: CameraPreviewUpdate) -> Result<()> {
        let placement = CameraPlacement {
            x: update.window_x.clamp(0.0, 1.0),
            y: update.window_y.clamp(0.0, 1.0),
            width: update.window_width.clamp(0.05, 1.0),
            height: update.window_height.clamp(0.05, 1.0),
        };
        let corner_radius = update.corner_radius.clamp(0.0, 0.5);

        // Active session is the source of truth during recording. Pending is
        // snapshotted into the session at `start()` and persisted back at
        // `stop()`, so we don't double-write on every preview tick.
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
                if placement != last {
                    let can_extend = tracker
                        .overlay
                        .motion_segments
                        .last()
                        .map(|segment| {
                            segment.source == "live-recorded"
                                && (segment.end - last_at).abs() < 0.01
                                && now_secs - last_at <= 0.45
                        })
                        .unwrap_or(false);

                    // Bound memory + serialized project size on long sessions
                    // with sustained camera movement: once the segment list hits
                    // the cap, fold further moves into the last segment (same as
                    // the extend path) instead of growing the Vec without limit.
                    // 4096 deliberate moves is far beyond any real session given
                    // the 0.45 s drag-coalescing window above.
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
                }
            } else {
                tracker.overlay.default_placement = placement.clone();
            }

            tracker.last_placement = Some(placement);
            tracker.last_at_secs = Some(now_secs);
            return Ok(());
        }
        drop(guard);

        // Pre-recording: keep pending in sync so `start()` snapshots the
        // user's latest preview state into the new session.
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
        target: CaptureTarget,
        output_dir: PathBuf,
        options: RecordingOptions,
    ) -> Result<Vec<String>> {
        let mut guard = self.session.lock();
        if guard.is_some() {
            return Err(anyhow!("recording is already running"));
        }

        // macOS gates screen capture behind the Screen Recording TCC permission,
        // which is SEPARATE from the Accessibility grant the cursor tracker
        // needs. Without it FFmpeg avfoundation spawns but yields zero frames —
        // the old behaviour was a silently-empty recording the user only
        // discovered at stop(), with the UI timer ticking the whole time. Fail
        // fast here (and trigger the system prompt) so the timer never starts on
        // a dead capture. No-op on Windows/Linux.
        crate::permissions::ensure_screen_recording()?;

        std::fs::create_dir_all(&output_dir)?;
        // Resolve the capture rate + quality tier up front. Both the pacer and
        // the encoder must agree on `recording_fps` (the encoder declares it as
        // `-framerate`, the pacer emits exactly that many frames/sec), and the
        // chosen rate is persisted into the project metadata at stop().
        let recording_fps = resolve_recording_fps(options.fps);
        // `"auto"` (the default) resolves against the probed encoder: hardware
        // → High, software → Balanced. Explicit tiers pass through unchanged.
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
        // Cap the frame queue by *memory*, not frame count. The previous
        // hard-coded 180 was fine at 720p (~640 MB worst case) but
        // OOM'd low-end machines at 1080p (~1.5 GB) and 4K (~6 GB) when
        // the encoder fell behind. Target ~256 MB of BGRA backing buffers
        // — that's a 3 s buffer at 1080p60 and ~8 frames at 4K, with a
        // hard floor of 30 frames (0.5 s @ 60 fps) so even a single
        // 4K monitor still gets enough headroom to ride out a hitch.
        const QUEUE_BUDGET_BYTES: u64 = 256 * 1024 * 1024;
        let frame_bytes = (target.source.width as u64)
            .saturating_mul(target.source.height as u64)
            .saturating_mul(4)
            .max(1);
        let queue_capacity = (QUEUE_BUDGET_BYTES / frame_bytes).clamp(30, 180) as usize;
        log::info!(
            "recording pipeline queue: {queue_capacity} frames ({} MB at {}x{} BGRA)",
            (frame_bytes * queue_capacity as u64) / (1024 * 1024),
            target.source.width,
            target.source.height,
        );
        let pipeline = RecordingPipeline::new(queue_capacity);
        let mut warnings = Vec::new();

        // Cursor sampling needs Accessibility on macOS; recording works without
        // it, so warn rather than block — the track just has gaps until granted.
        if !crate::permissions::cursor_tracking_authorized() {
            warnings.push(
                "Cursor tracking is off — grant Recast in System Settings → \
                 Privacy & Security → Accessibility to capture cursor movement \
                 and clicks."
                    .to_string(),
            );
        }

        let first_frame_offset_us = Arc::new(AtomicU64::new(0));
        let capture_handle = spawn_capture_loop(
            target.clone(),
            stop_flag.clone(),
            pause_flag.clone(),
            pipeline.clone(),
            started_at,
            recording_fps,
            first_frame_offset_us.clone(),
        )?;

        let encoder_handle = match spawn_encoder_loop(
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
            Ok(handle) => handle,
            Err(e) => {
                // Capture thread is already live; signal + join it so a failed
                // start doesn't leave an orphaned capture loop (and its FFmpeg
                // child) running forever.
                stop_flag.store(true, Ordering::Release);
                let _ = capture_handle.join();
                return Err(e);
            }
        };

        // Cursor coordinates need to be remapped from virtual-desktop space
        // (where `GetCursorPos` returns them) to the recorded frame's
        // pixel space. The encoder crops the captured DXGI texture to the
        // `crop` rectangle, so the recorded video's (0, 0) corresponds to
        // virtual-desktop (`crop.x`, `crop.y`). Without this remap, every
        // sample lives outside the [0..frame] range whenever the user
        // records a secondary monitor or a region.
        // Only sample the cursor when the OS actually permits it. On macOS,
        // `device_query`'s CoreGraphics access re-triggers the "control this
        // computer using accessibility features" prompt EVERY recording when
        // Accessibility isn't granted — the repeated-prompt complaint. Gating
        // on the (non-prompting) trust check means we never poke that API
        // unless it'll succeed; otherwise we substitute an empty track (the
        // user already got the warning above). Always-true off macOS.
        let cursor_handle = if crate::permissions::cursor_tracking_authorized() {
            match spawn_cursor_capture(
                stop_flag.clone(),
                clock.clone(),
                CursorCaptureFrame {
                    origin_x: target.crop.x,
                    origin_y: target.crop.y,
                    width: target.crop.width,
                    height: target.crop.height,
                    // macOS samples the cursor in logical points but the video is
                    // physical pixels; lift samples by the display's scale so they
                    // line up. 1.0 on Windows/Linux (already physical) → unchanged.
                    scale: target.scale_factor,
                },
            ) {
                Ok(handle) => handle,
                Err(e) => {
                    // Capture + encoder are already live; tear both down so a failed
                    // start doesn't orphan them.
                    stop_flag.store(true, Ordering::Release);
                    let _ = capture_handle.join();
                    let _ = encoder_handle.join();
                    return Err(e);
                }
            }
        } else {
            // No-op placeholder so the session shape (and `stop()`'s join) is
            // unchanged; it yields an empty cursor track immediately.
            match std::thread::Builder::new()
                .name("recast-cursor-disabled".into())
                .spawn(CursorTrack::default)
            {
                Ok(handle) => handle,
                Err(e) => {
                    stop_flag.store(true, Ordering::Release);
                    let _ = capture_handle.join();
                    let _ = encoder_handle.join();
                    return Err(anyhow!("failed to spawn cursor placeholder thread: {e}"));
                }
            }
        };

        // Start system/loopback audio capture — but only when the user asked
        // for it. Gating here (mirroring the microphone/camera blocks below) is
        // what makes the "System audio" toggle real: loopback used to run
        // unconditionally, so it recorded *everything* on the default output —
        // including Recast's own editor playback — which is the record-while-
        // previewing echo. When off, `stop()` falls back to a silent WAV so
        // downstream muxing still has a track.
        let audio_session = if options.system_audio {
            match AudioCaptureSession::start(AudioCaptureConfig {
                output_path: audio_path.clone(),
                pause_flag: pause_flag.clone(),
            }) {
                Ok(session) => Some(session),
                Err(e) => {
                    log::warn!("audio capture unavailable, recording without audio: {e}");
                    None
                }
            }
        } else {
            None
        };

        // Start microphone capture as a separate track.
        let microphone_session = if options.microphone {
            match MicrophoneCaptureSession::start(MicrophoneCaptureConfig {
                output_path: microphone_path.clone(),
                device_id: options.microphone_device_id.clone(),
                pause_flag: pause_flag.clone(),
            }) {
                Ok(session) => Some(session),
                Err(e) => {
                    log::warn!("microphone capture unavailable: {e}");
                    None
                }
            }
        } else {
            None
        };

        // Start camera capture as a separate track.
        let camera_session = if options.camera {
            match crate::camera::CameraCaptureSession::start(crate::camera::CameraCaptureConfig {
                output_path: camera_path.clone(),
                device_name: options.camera_device_id.clone(),
            }) {
                Ok(session) => Some(session),
                Err(e) => {
                    log::warn!("camera capture unavailable: {e}");
                    warnings.push(format!("Camera capture unavailable: {e}"));
                    None
                }
            }
        } else {
            None
        };

        let mut camera_overlay = self.pending_camera_overlay.lock().clone();
        camera_overlay.enabled = options.camera && camera_session.is_some();

        *guard = Some(RecordingSession {
            stop_flag,
            pause_flag,
            capture_handle,
            encoder_handle,
            cursor_handle,
            first_frame_offset_us,
            audio_session,
            audio_path,
            microphone_session,
            camera_session,
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
        let session = guard.take().context("recording is not running")?;
        drop(guard);

        session.stop_flag.store(true, Ordering::Release);

        // Reap ALL capture threads and OS sessions before propagating any error.
        // `session` is already out of the session mutex, so anything we skip
        // tearing down here (because an earlier `?` returned) is orphaned for the
        // process lifetime — a spinning thread plus, for audio/camera, a live
        // FFmpeg child or capture device. So join every thread and stop every
        // session first, then surface the first failure.
        let capture_join = session.capture_handle.join();
        let cursor_join = session.cursor_handle.join();
        let encoder_join = session.encoder_handle.join();

        // Stop the system-audio / mic / camera OS sessions regardless of how the
        // threads fared — each reaps its own FFmpeg child / releases its device.
        // Capture whether a loopback session actually ran before consuming it, so
        // the project metadata can report system audio honestly (a disabled
        // toggle writes silence below but must not claim a real track).
        let has_system_audio = session.audio_session.is_some();
        let audio_stop = session.audio_session.map(|s| s.stop());
        let microphone_stop = session.microphone_session.map(|s| s.stop());
        let camera_stop = session.camera_session.map(|s| s.stop());

        // Everything is reaped — now surface fatal thread failures.
        capture_join.map_err(|_| anyhow!("capture thread panicked"))??;
        let mut cursor_track = cursor_join.map_err(|_| anyhow!("cursor thread panicked"))?;
        encoder_join.map_err(|_| anyhow!("encoder thread panicked"))??;

        // Re-base the cursor track onto the video clock: the capture-source
        // warmup means video frame 0 is wall-clock `first_frame_offset_us`, not
        // 0, while the cursor track has been ticking since recording start.
        // Without this the cursor (and its clicks / highlight) runs ahead of
        // the on-screen action by the warmup — the reported ~half-second delay.
        let cursor_offset_us = session.first_frame_offset_us.load(Ordering::Acquire);
        shift_cursor_track(&mut cursor_track, cursor_offset_us);
        write_cursor_track(&session.cursor_path, &cursor_track)?;

        // Resolve the system-audio path: the captured file, else a silence
        // fallback so downstream always has a track to mux.
        let audio_path = match audio_stop {
            Some(Ok(path)) => path,
            Some(Err(e)) => {
                log::warn!("audio capture stop failed, writing silence: {e}");
                let duration = session.clock.effective_elapsed().as_secs_f64();
                crate::audio::wav::write_silence_wav(&session.audio_path, 48_000, 2, duration)?;
                session.audio_path.clone()
            }
            None => {
                let duration = session.clock.effective_elapsed().as_secs_f64();
                crate::audio::wav::write_silence_wav(&session.audio_path, 48_000, 2, duration)?;
                session.audio_path.clone()
            }
        };

        // Microphone path if its capture succeeded.
        let microphone_path = match microphone_stop {
            Some(Ok(path)) => Some(path),
            Some(Err(e)) => {
                log::warn!("microphone capture stop failed: {e}");
                None
            }
            None => None,
        };

        // Camera path if its capture succeeded.
        let camera_path = match camera_stop {
            Some(Ok(path)) => Some(path),
            Some(Err(e)) => {
                log::warn!("camera capture stop failed: {e}");
                None
            }
            None => None,
        };

        // The camera records continuously through pauses; cut the paused
        // spans out so the camera video matches the pause-free screen
        // timeline. Best-effort — on failure keep the untrimmed file.
        let camera_path = match camera_path {
            Some(path) => {
                let intervals = session.clock.pause_intervals();
                if !intervals.is_empty() {
                    if let Err(e) = trim_video_pause_intervals(&path, &intervals) {
                        log::warn!("camera pause-trim failed, keeping untrimmed file: {e}");
                    }
                }
                Some(path)
            }
            None => None,
        };

        let stats = build_stats(
            &session.pipeline,
            session.clock.effective_elapsed().as_millis() as u64,
            session.recording_fps,
        );

        // Persist the user's latest overlay settings (mirror, shape, corner
        // radius, etc.) back to pending so the next recording inherits them.
        // Don't copy motion_segments — those are session-local.
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
            camera_overlay: session.camera_overlay.overlay,
            started_at_unix_ms: session.started_at_unix_ms,
            stats,
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
        // Bank the pause duration before letting threads run again so they
        // wake into a correct clock.
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

/// Re-encode `path` in place, dropping every frame inside one of the
/// `intervals` (real-time `(start_us, end_us)` offsets from recording start)
/// and re-stamping the survivors onto a gap-free timeline. Used to cut
/// recording-pause spans out of the continuously-captured camera video.
fn trim_video_pause_intervals(path: &Path, intervals: &[(u64, u64)]) -> Result<()> {
    let keep = {
        let terms: Vec<String> = intervals
            .iter()
            .map(|(s, e)| {
                format!(
                    "between(t,{:.3},{:.3})",
                    *s as f64 / 1_000_000.0,
                    *e as f64 / 1_000_000.0
                )
            })
            .collect();
        format!("not({})", terms.join("+"))
    };
    // `select` drops paused frames; `setpts=N/FRAME_RATE/TB` re-times the
    // survivors so the gaps close instead of becoming frozen frames.
    let vf = format!("select='{keep}',setpts=N/FRAME_RATE/TB");
    let tmp = path.with_extension("trim.mp4");
    let in_path = path.to_string_lossy().to_string();
    let out_path = tmp.to_string_lossy().to_string();

    // Camera trim runs synchronously at stop() — on a low-end CPU
    // recording 10 min of camera, libx264-veryfast can take 30+ seconds
    // while the user stares at a stuck "Saving recording…" UI. Route
    // through the same hardware encoder the rest of the pipeline
    // already probed (NVENC/AMF/QSV) so trim time scales with the GPU
    // instead of the CPU. The CPU path drops to libx264 ultrafast for
    // the same reason — quality is fine for a 720p camera bubble.
    let mut command = std::process::Command::new(crate::ffmpeg::ffmpeg_path());
    // Route through the same probed encoder the recorder uses (NVENC/AMF/QSV →
    // GPU, else libx264 ultrafast) so trim time scales with the GPU, not the CPU.
    let codec_args = h264::codec_args(
        H264Encoder::from_ffmpeg_name(crate::ffmpeg::preferred_h264_encoder()),
        EncodePurpose::QuickTrim,
    );
    command.args(["-y", "-i", in_path.as_str(), "-vf", vf.as_str(), "-an"]);
    command.args(&codec_args);
    command.arg(out_path.as_str());
    crate::ffmpeg::configure_silent_command(&mut command);
    let output = command
        .output()
        .context("failed to run ffmpeg for camera pause-trim")?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&tmp);
        return Err(anyhow!(
            "ffmpeg camera trim failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    std::fs::rename(&tmp, path).context("failed to swap in the trimmed camera file")?;
    Ok(())
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
mod scale_tests {
    use super::*;

    fn target(source: CaptureArea, crop: CaptureArea) -> CaptureTarget {
        CaptureTarget {
            kind: CaptureKind::Region,
            id: 1,
            display_id: 1,
            label: "t".into(),
            source,
            crop,
            scale_factor: 1.0,
        }
    }

    #[test]
    fn scale_area_scales_origin_and_keeps_even_dims() {
        let a = CaptureArea {
            x: 10,
            y: 20,
            width: 101,
            height: 51,
        };
        let s = scale_area(a, 2.0);
        assert_eq!((s.x, s.y), (20, 40));
        // 101*2 = 202, 51*2 = 102 — both already even.
        assert_eq!((s.width, s.height), (202, 102));
    }

    #[test]
    fn scale_area_forces_even_dimensions() {
        // 75 → round → 75 → & !1 → 74; libx264 needs even dims.
        let a = CaptureArea {
            x: 0,
            y: 0,
            width: 75,
            height: 75,
        };
        let s = scale_area(a, 1.0);
        assert_eq!((s.width, s.height), (74, 74));
    }

    #[test]
    fn apply_device_scale_is_noop_at_one() {
        let area = CaptureArea {
            x: 5,
            y: 7,
            width: 100,
            height: 80,
        };
        let mut t = target(area, area);
        apply_device_scale(&mut t, 1.0);
        assert_eq!(t.scale_factor, 1.0);
        assert_eq!(t.source.width, 100);
        assert_eq!((t.crop.x, t.crop.y), (5, 7));
        assert_eq!((t.crop.width, t.crop.height), (100, 80));
    }

    #[test]
    fn apply_device_scale_lifts_source_and_crop_to_physical() {
        let source = CaptureArea {
            x: 0,
            y: 0,
            width: 200,
            height: 100,
        };
        let crop = CaptureArea {
            x: 50,
            y: 25,
            width: 100,
            height: 50,
        };
        let mut t = target(source, crop);
        apply_device_scale(&mut t, 2.0);
        assert_eq!(t.scale_factor, 2.0);
        assert_eq!((t.source.width, t.source.height), (400, 200));
        assert_eq!((t.crop.x, t.crop.y), (100, 50));
        assert_eq!((t.crop.width, t.crop.height), (200, 100));
    }

    #[test]
    fn apply_device_scale_clamps_scaled_crop_within_scaled_source() {
        // A crop that runs off the display (e.g. a window pulled past the
        // screen edge): after scaling it must be clamped so the encoder's
        // crop filter never exceeds the captured frame.
        let source = CaptureArea {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let crop = CaptureArea {
            x: 60,
            y: 60,
            width: 80,
            height: 80,
        };
        let mut t = target(source, crop);
        apply_device_scale(&mut t, 2.0);
        // source → 200×200; crop origin → (120,120); available = 80 each way.
        assert_eq!((t.source.width, t.source.height), (200, 200));
        assert_eq!((t.crop.x, t.crop.y), (120, 120));
        assert_eq!((t.crop.width, t.crop.height), (80, 80));
        // Crop stays inside the captured frame.
        assert!(t.crop.x + t.crop.width as i32 <= t.source.x + t.source.width as i32);
        assert!(t.crop.y + t.crop.height as i32 <= t.source.y + t.source.height as i32);
    }
}

#[cfg(test)]
mod options_tests {
    use super::*;

    // The panel sends `systemAudio` (camelCase); it must land on `system_audio`
    // and actually gate loopback in `start()`. This guards the serde bridge that
    // the record-while-previewing echo fix depends on — if the rename or default
    // regresses, the toggle silently goes dead again.
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
        // A profile/older client that omits the field keeps the historical
        // capture-by-default behaviour.
        let opts: RecordingOptions = serde_json::from_str("{}").unwrap();
        assert!(opts.system_audio);
        assert!(RecordingOptions::default().system_audio);
    }
}
