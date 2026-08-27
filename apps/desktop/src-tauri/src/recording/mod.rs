pub mod clock;
pub mod pipeline;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

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
use crate::encoder::{spawn_encoder_loop, EncoderConfig};
use crate::render::node_types::{CameraMotionSegment, CameraOverlaySettings, CameraPlacement};
pub use clock::{offset_ms_from_video, RecordingClock, TrackStart};
use pipeline::{spawn_capture_loop, PipelineSnapshot, RecordingPipeline};

/// Frames per second emitted by the capture pacer and declared to the encoder.
/// The pacer emits exactly this many frames per real-time second, and the
/// encoder hands FFmpeg the same number as `-framerate`. Together they
/// guarantee 1 second of wall-clock recording → 1 second of video PTS — the
/// invariant the cursor track (timestamped in wall-clock μs) relies on for
/// sync.
pub const RECORDING_FPS: u32 = 60;

//  Shared types

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

    // True per-window capture (Windows Graphics Capture) records only the
    // window's own surface, so the "source" IS the window — no monitor, no
    // crop. This isolates a maximized or overlapped window, which the
    // monitor-plus-crop path below cannot. `create_source` selects the WGC
    // backend on the same predicate, so the two stay consistent. Other
    // platforms (and older Windows) fall through to monitor-plus-crop.
    #[cfg(windows)]
    if crate::capture::platform::windows::wgc_window_capture_supported() {
        // Even dims for libx264/NVENC; the WGC source clamps its copy to match.
        let win = CaptureArea {
            x: crop.x,
            y: crop.y,
            width: crop.width & !1,
            height: crop.height & !1,
        };
        // display_id is the monitor the window sits on, used only to re-base the
        // cursor track onto the window's origin.
        let display_id = Monitor::all()
            .ok()
            .and_then(|monitors| {
                monitors.into_iter().find(|monitor| {
                    let x = monitor.x().unwrap_or_default();
                    let y = monitor.y().unwrap_or_default();
                    let width = monitor.width().unwrap_or_default() as i32;
                    let height = monitor.height().unwrap_or_default() as i32;
                    center_x >= x && center_x < x + width && center_y >= y && center_y < y + height
                })
            })
            .and_then(|monitor| monitor.id().ok())
            .unwrap_or_default();

        return Ok(CaptureTarget {
            kind: CaptureKind::Window,
            id: target_id,
            display_id,
            label: window.title().unwrap_or_else(|_| "Window".into()),
            source: win,
            crop: win,
            scale_factor: 1.0,
        });
    }

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
            let scale = display_scale_factor(monitor);
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
    let scale_f32 = scale;

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

/// Signed millisecond offsets of each companion track relative to video frame
/// 0. Positive means the track started late and needs padding at its head;
/// negative means it started early and its head must be trimmed. `None` means
/// the track produced nothing, so consumers treat it as aligned.
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
    /// Reap a still-live session.
    ///
    /// Must be called explicitly from the app's exit handler: quitting from the
    /// tray goes through `app.exit(0)`, which ends in `std::process::exit` and
    /// therefore never runs destructors — so `Drop` alone could not save us. The
    /// encoder survived by luck (its stdin closes, so it sees EOF), but the
    /// audio, mic and camera children read from devices and kept running with
    /// the mic and webcam held after Recast was gone.
    pub fn abort_for_shutdown(&self) {
        if let Some(session) = self.session.lock().take() {
            log::warn!("aborting live recording session on shutdown");
            session.abort();
        }
    }
}

impl Drop for RecordingManager {
    fn drop(&mut self) {
        // A session still present at manager-drop time means the recording never
        // went through `stop()` (a panic unwound the owner). Reap it so we don't
        // orphan the capture/audio/mic FFmpeg children, which on macOS/Linux are
        // subprocesses that keep recording and hold the device (a stuck mic /
        // screen grab), and so no capture thread spins forever. The normal path
        // already took the session out, so this usually finds `None`.
        self.abort_for_shutdown();
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
    /// Best-effort teardown for an abnormal shutdown (see `Drop for
    /// RecordingManager`). Mirrors the reaping half of `stop()` without
    /// assembling artifacts. `RecordingSession` deliberately does NOT implement
    /// `Drop` so `stop()` can still move fields out of it; this consuming helper
    /// is the abnormal-path equivalent.
    fn abort(self) {
        self.stop_flag.store(true, Ordering::Release);
        // Joining lets each thread run its own cleanup: the capture thread drops
        // its `CaptureSource` (which kills the screen-capture FFmpeg child), and
        // the encoder/cursor threads finalize and exit.
        let _ = self.capture_handle.join();
        let _ = self.cursor_handle.join();
        let _ = self.encoder_handle.join();
        // Each OS session reaps its own FFmpeg child / releases its device.
        if let Some(session) = self.audio_session {
            let _ = session.stop();
        }
        if let Some(session) = self.microphone_session {
            let _ = session.stop();
        }
        // The camera is owned by the preview WebView, not a Rust child, so there
        // is nothing to reap here — the webview is torn down with the app.
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
                if camera_moved(&last, &placement) {
                    let can_extend = tracker
                        .overlay
                        .motion_segments
                        .last()
                        .map(|segment| extends_current_move(segment, last_at, now_secs))
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
                    // Only an accepted move advances the reference, so a drag
                    // slower than the dead zone accumulates rather than being
                    // filtered away one tick at a time.
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
        // Frame queue is capped by memory, not frame count: 180 BGRA frames is
        // ~6 GB at 4K and OOM'd low-end machines when the encoder fell behind.
        let frame_bytes = frame_bytes_bgra(target.source.width, target.source.height);
        let queue_capacity = queue_capacity_for(target.source.width, target.source.height);
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
        // Each track marks its own first sample against `started_at`; stop()
        // turns the differences into the offsets the muxer aligns by.
        let audio_start = TrackStart::new(started_at);
        let microphone_start = TrackStart::new(started_at);
        let camera_start = TrackStart::new(started_at);
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
                start: audio_start.clone(),
            }) {
                Ok(session) => {
                    // System audio was requested, but on macOS without
                    // ScreenCaptureKit or a virtual driver (and Linux without a
                    // PulseAudio monitor) no loopback source is reachable, so
                    // the session falls back to writing silence. Tell the user
                    // rather than delivering a mute track that looks captured.
                    if !session.is_capturing() {
                        warnings.push(
                            "System audio could not be captured on this device, \
                             so the recording will have no system sound. Your \
                             microphone and video are not affected."
                                .to_string(),
                        );
                    }
                    Some(session)
                }
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
            if let Some(warning) =
                crate::audio::microphone_quality_warning(options.microphone_device_id.as_deref())
            {
                warnings.push(warning);
            }
            match MicrophoneCaptureSession::start(MicrophoneCaptureConfig {
                output_path: microphone_path.clone(),
                device_id: options.microphone_device_id.clone(),
                pause_flag: pause_flag.clone(),
                start: microphone_start.clone(),
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

        // Attach a sink to the preview's reader: a second open fails, cameras are exclusive.
        let mut camera_requested = options.camera;
        if camera_requested {
            if let Err(e) = crate::camera::session::attach_recorder(
                camera_path.clone(),
                recording_fps,
                clock.clone(),
                camera_start.clone(),
                pause_flag.clone(),
            ) {
                log::warn!("camera recording unavailable: {e:#}");
                warnings.push(format!("Camera could not be recorded: {e}"));
                camera_requested = false;
            }
        }

        let mut camera_overlay = self.pending_camera_overlay.lock().clone();
        // The authoritative value (a delivered track) is set at stop; enable on
        // intent here so the editor shows the overlay while the file lands.
        camera_overlay.enabled = camera_requested;

        *guard = Some(RecordingSession {
            stop_flag,
            pause_flag,
            capture_handle,
            encoder_handle,
            cursor_handle,
            first_frame_offset_us,
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
        // Report system audio honestly before consuming the session: it counts
        // only when a real track was captured. A disabled toggle (no session) and
        // the silence fallback (session present but no reachable loopback source,
        // e.g. macOS without SCKit/BlackHole) both write silence below purely to
        // give the muxer a track, and neither must claim captured system audio.
        let has_system_audio = session
            .audio_session
            .as_ref()
            .is_some_and(|s| s.is_capturing());
        let audio_stop = session.audio_session.take().map(|s| s.stop());
        let microphone_stop = session.microphone_session.take().map(|s| s.stop());
        // Finish the file but leave the preview live; the device closes with the bubble.
        let camera_stop = session
            .camera_requested
            .then(crate::camera::session::detach_recorder);

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

        let track_offsets = TrackOffsets {
            audio_ms: offset_ms_from_video(cursor_offset_us, &session.audio_start),
            microphone_ms: offset_ms_from_video(cursor_offset_us, &session.microphone_start),
            camera_ms: offset_ms_from_video(cursor_offset_us, &session.camera_start),
        };
        log::info!(
            "track offsets vs video t0 ({}ms warmup): {:?}",
            cursor_offset_us / 1000,
            track_offsets
        );

        // Resolve the system-audio path: the captured file, else a silence
        // fallback so downstream always has a track to mux.
        let mut has_system_audio = has_system_audio;
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

        // WASAPI loopback delivers NOTHING while no application is rendering
        // audio, so a session where nothing played leaves a valid but
        // header-only WAV. That is not a captured track, and an empty input
        // breaks the export's filter graph outright, so replace it with real
        // silence of the right length and stop claiming system audio.
        if !crate::audio::wav::wav_has_samples(&audio_path) {
            let duration = session.clock.effective_elapsed().as_secs_f64();
            log::info!(
                "system audio captured no samples ({}s of silence written instead)",
                duration.round()
            );
            crate::audio::wav::write_silence_wav(&audio_path, 48_000, 2, duration)?;
            has_system_audio = false;
        }

        // Non-fatal capture issues to surface to the user after the save. A
        // requested mic/camera track that failed (device in use, or on macOS
        // Camera/Microphone permission denied so the device produced no frames)
        // otherwise vanished silently: the recording succeeds minus that track.
        let mut warnings: Vec<String> = Vec::new();

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

/// Cross-platform (runs on every CI leg AND locally on Windows).
#[cfg(test)]
mod session_tests {
    use super::*;

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

    /// The CaptureSource contract every backend must honour: the encoder is
    /// configured for `source` dimensions and crops with THIS rectangle, so a
    /// backend must emit full-`source`-sized frames. The X11 backend used to
    /// pre-crop instead, so the encoder cropped an already-cropped buffer and
    /// corrupted region/window recordings. Offsets are relative to the source
    /// origin, not the virtual desktop.
    #[test]
    fn crop_is_reported_relative_to_the_captured_source() {
        // Source is the second monitor, so the crop's desktop-space x/y (2000,
        // 50) must become source-relative (80, 50).
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
