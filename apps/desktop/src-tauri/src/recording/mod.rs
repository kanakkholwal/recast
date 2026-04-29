pub mod pipeline;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
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
use crate::cursor::{spawn_cursor_capture, write_cursor_track, CursorTrack};
use crate::encoder::{spawn_encoder_loop, EncoderConfig};
use pipeline::{spawn_capture_loop, PipelineSnapshot, RecordingPipeline};

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

    Ok(CaptureTarget {
        kind: CaptureKind::Display,
        id: target_id,
        label: display.name().unwrap_or_else(|_| "Display".into()),
        source: area,
        crop: area,
    })
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

    Ok(CaptureTarget {
        kind: CaptureKind::Window,
        id: target_id,
        label: window.title().unwrap_or_else(|_| "Window".into()),
        source,
        crop,
    })
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
            let x = monitor.x().unwrap_or_default();
            let y = monitor.y().unwrap_or_default();
            let width = monitor.width().unwrap_or_default() as i32;
            let height = monitor.height().unwrap_or_default() as i32;
            center_x >= x && center_x < x + width && center_y >= y && center_y < y + height
        })
        .context("unable to locate the display containing the selected region")?;

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

    // Clamp the requested region to the source monitor's bounds so that the
    // encoder crop is never outside the captured frame.
    let clamped_x = rect.x.max(mon_x).min(mon_x + mon_w as i32);
    let clamped_y = rect.y.max(mon_y).min(mon_y + mon_h as i32);
    let max_w = (mon_x + mon_w as i32 - clamped_x).max(0) as u32;
    let max_h = (mon_y + mon_h as i32 - clamped_y).max(0) as u32;
    // Encoder libx264 requires even dimensions.
    let crop_w = (rect.width.min(max_w)) & !1u32;
    let crop_h = (rect.height.min(max_h)) & !1u32;
    if crop_w == 0 || crop_h == 0 {
        return Err(anyhow!("region collapsed to zero after clamping"));
    }

    let crop = CaptureArea {
        x: clamped_x,
        y: clamped_y,
        width: crop_w,
        height: crop_h,
    };

    Ok(CaptureTarget {
        kind: CaptureKind::Region,
        id: monitor_id,
        label: format!("Area {crop_w}×{crop_h}"),
        source,
        crop,
    })
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
    pub microphone_path: Option<PathBuf>,
    pub camera_path: Option<PathBuf>,
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
        }
    }
}

//  Recording session orchestration

pub struct RecordingManager {
    session: Mutex<Option<RecordingSession>>,
}

impl Default for RecordingManager {
    fn default() -> Self {
        Self {
            session: Mutex::new(None),
        }
    }
}

struct RecordingSession {
    stop_flag: Arc<AtomicBool>,
    capture_handle: JoinHandle<Result<()>>,
    encoder_handle: JoinHandle<Result<()>>,
    cursor_handle: JoinHandle<CursorTrack>,
    audio_session: Option<AudioCaptureSession>,
    audio_path: PathBuf,
    microphone_session: Option<MicrophoneCaptureSession>,
    camera_session: Option<crate::camera::CameraCaptureSession>,
    pipeline: RecordingPipeline,
    target: CaptureTarget,
    recording_path: PathBuf,
    cursor_path: PathBuf,
    started_at: Instant,
    started_at_unix_ms: u64,
}

impl RecordingManager {
    pub fn start(
        &self,
        target: CaptureTarget,
        output_dir: PathBuf,
        options: RecordingOptions,
    ) -> Result<()> {
        let mut guard = self.session.lock();
        if guard.is_some() {
            return Err(anyhow!("recording is already running"));
        }

        std::fs::create_dir_all(&output_dir)?;
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
        let stop_flag = Arc::new(AtomicBool::new(false));
        let pipeline = RecordingPipeline::new(180);

        let capture_handle = spawn_capture_loop(
            target.clone(),
            stop_flag.clone(),
            pipeline.clone(),
            started_at,
        )?;

        let encoder_handle = spawn_encoder_loop(
            EncoderConfig {
                width: target.source.width,
                height: target.source.height,
                fps: 60,
                crop: target.crop_relative_to_source(),
                output_path: recording_path.clone(),
            },
            stop_flag.clone(),
            pipeline.clone(),
        )?;

        let cursor_handle = spawn_cursor_capture(stop_flag.clone(), started_at)?;

        // Start system audio capture. If it fails, log and continue.
        let audio_session = match AudioCaptureSession::start(AudioCaptureConfig {
            output_path: audio_path.clone(),
        }) {
            Ok(session) => Some(session),
            Err(e) => {
                log::warn!("audio capture unavailable, recording without audio: {e}");
                None
            }
        };

        // Start microphone capture as a separate track.
        let microphone_session = if options.microphone {
            match MicrophoneCaptureSession::start(MicrophoneCaptureConfig {
                output_path: microphone_path.clone(),
                device_id: options.microphone_device_id.clone(),
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
                    None
                }
            }
        } else {
            None
        };

        *guard = Some(RecordingSession {
            stop_flag,
            capture_handle,
            encoder_handle,
            cursor_handle,
            audio_session,
            audio_path,
            microphone_session,
            camera_session,
            pipeline,
            target,
            recording_path,
            cursor_path,
            started_at,
            started_at_unix_ms,
        });
        Ok(())
    }

    pub fn stop(&self) -> Result<RecordingArtifacts> {
        let mut guard = self.session.lock();
        let session = guard.take().context("recording is not running")?;
        drop(guard);

        session.stop_flag.store(true, Ordering::Release);

        session
            .capture_handle
            .join()
            .map_err(|_| anyhow!("capture thread panicked"))??;

        let cursor_track = session
            .cursor_handle
            .join()
            .map_err(|_| anyhow!("cursor thread panicked"))?;
        write_cursor_track(&session.cursor_path, &cursor_track)?;

        session
            .encoder_handle
            .join()
            .map_err(|_| anyhow!("encoder thread panicked"))??;

        // Stop system audio capture. Write silence fallback if unavailable.
        let audio_path = if let Some(audio_session) = session.audio_session {
            match audio_session.stop() {
                Ok(path) => path,
                Err(e) => {
                    log::warn!("audio capture stop failed, writing silence: {e}");
                    let duration = session.started_at.elapsed().as_secs_f64();
                    crate::audio::wav::write_silence_wav(&session.audio_path, 48_000, 2, duration)?;
                    session.audio_path
                }
            }
        } else {
            let duration = session.started_at.elapsed().as_secs_f64();
            crate::audio::wav::write_silence_wav(&session.audio_path, 48_000, 2, duration)?;
            session.audio_path
        };

        // Stop microphone capture if it was running.
        let microphone_path = if let Some(mic_session) = session.microphone_session {
            match mic_session.stop() {
                Ok(path) => Some(path),
                Err(e) => {
                    log::warn!("microphone capture stop failed: {e}");
                    None
                }
            }
        } else {
            None
        };

        // Stop camera capture if it was running.
        let camera_path = if let Some(cam_session) = session.camera_session {
            match cam_session.stop() {
                Ok(path) => Some(path),
                Err(e) => {
                    log::warn!("camera capture stop failed: {e}");
                    None
                }
            }
        } else {
            None
        };

        let stats = build_stats(
            &session.pipeline,
            session.started_at.elapsed().as_millis() as u64,
        );

        Ok(RecordingArtifacts {
            capture_target: session.target,
            recording_path: session.recording_path,
            cursor_path: session.cursor_path,
            audio_path,
            microphone_path,
            camera_path,
            started_at_unix_ms: session.started_at_unix_ms,
            stats,
        })
    }
}

fn build_stats(pipeline: &RecordingPipeline, duration_ms: u64) -> RecordingStats {
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
        nominal_fps: 60,
    }
}
