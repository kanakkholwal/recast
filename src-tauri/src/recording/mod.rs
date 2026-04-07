pub mod pipeline;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use xcap::{Monitor, Window};

use crate::audio::{AudioCaptureConfig, AudioCaptureSession};
use crate::cursor::{CursorTrack, spawn_cursor_capture, write_cursor_track};
use crate::encoder::{EncoderConfig, spawn_encoder_loop};
use pipeline::{PipelineSnapshot, RecordingPipeline, spawn_capture_loop};

// ── Shared types ────────────────────────────────────────────────────────

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

// ── Recording stats and artifacts ───────────────────────────────────────

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
    pub started_at_unix_ms: u64,
    pub stats: RecordingStats,
}

// ── Recording session orchestration ─────────────────────────────────────

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
    pipeline: RecordingPipeline,
    target: CaptureTarget,
    recording_path: PathBuf,
    cursor_path: PathBuf,
    started_at: Instant,
    started_at_unix_ms: u64,
}

impl RecordingManager {
    pub fn start(&self, target: CaptureTarget, output_dir: PathBuf) -> Result<()> {
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

        // Start real audio capture. If it fails (e.g., no audio device), log
        // the error and continue without audio — the recording is still valid.
        let audio_session = match AudioCaptureSession::start(AudioCaptureConfig {
            output_path: audio_path.clone(),
            capture_loopback: true,
            capture_microphone: false,
        }) {
            Ok(session) => Some(session),
            Err(e) => {
                log::warn!("audio capture unavailable, recording without audio: {e}");
                None
            }
        };

        *guard = Some(RecordingSession {
            stop_flag,
            capture_handle,
            encoder_handle,
            cursor_handle,
            audio_session,
            audio_path,
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

        // Stop audio capture. Write silence fallback if audio was unavailable.
        let audio_path = if let Some(audio_session) = session.audio_session {
            match audio_session.stop() {
                Ok(path) => path,
                Err(e) => {
                    log::warn!("audio capture stop failed, writing silence: {e}");
                    let duration = session.started_at.elapsed().as_secs_f64();
                    crate::audio::wav::write_silence_wav(
                        &session.audio_path,
                        48_000,
                        2,
                        duration,
                    )?;
                    session.audio_path
                }
            }
        } else {
            let duration = session.started_at.elapsed().as_secs_f64();
            crate::audio::wav::write_silence_wav(&session.audio_path, 48_000, 2, duration)?;
            session.audio_path
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
