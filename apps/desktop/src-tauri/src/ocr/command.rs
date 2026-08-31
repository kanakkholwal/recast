//! Reads a video into a structured text timeline; shared by the `screen.read` control method and the `read_video_text` command.
//! Compiles without the `ocr` feature, reporting the engine as absent so the command surface degrades gracefully.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::AppHandle;

use crate::commands::error::AppResult;

use super::frames::{probe_dims, sample_frames, SampleOpts};
use super::models;
use super::timeline::{build_timeline, OcrStats, TimelineOpts, VideoTextTimeline};

/// Progress of a read as counted work, the units being whatever the phase counts: bytes downloading, coarse frames sampling, OCR'd frames reading.
/// A `total` of 0 means the phase cannot be counted yet, so the UI stays indeterminate instead of dividing by it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrProgress {
    /// `"downloading"` | `"sampling"` | `"reading"` | `"done"`.
    pub phase: String,
    /// Units of this phase completed.
    pub done: u64,
    /// Units this phase expects, or 0 when not yet known.
    pub total: u64,
    /// The result so far: frames kept while sampling, screen states found while reading. Carried so a long read shows what it is producing, not just how far along it is.
    pub found: u64,
}

impl OcrProgress {
    fn new(phase: &str, done: u64, total: u64, found: u64) -> Self {
        Self {
            phase: phase.to_string(),
            done,
            total,
            found,
        }
    }
}

/// Smallest gap between two progress messages. The sampler ticks per decoded
/// frame (thousands on a long clip), which would flood the IPC channel and give
/// the UI nothing a human can read anyway. The final tick of each phase is always
/// sent regardless, so a bar never freezes short of its total.
const TICK_INTERVAL: Duration = Duration::from_millis(80);

/// Rate-limits progress messages to `TICK_INTERVAL`, except for ones marked final.
struct Throttle<F: Fn(OcrProgress)> {
    sink: F,
    last: Option<Instant>,
}

impl<F: Fn(OcrProgress)> Throttle<F> {
    fn new(sink: F) -> Self {
        Self { sink, last: None }
    }

    fn send(&mut self, p: OcrProgress, force: bool) {
        let now = Instant::now();
        let due = self
            .last
            .is_none_or(|t| now.duration_since(t) >= TICK_INTERVAL);
        if force || due {
            self.last = Some(now);
            (self.sink)(p);
        }
    }
}

/// Reads `video_path` into a timeline; `on_progress` receives counted progress and `previews` attaches a small JPEG per span.
/// `include_ranges` are the source ranges the edit keeps, so removed footage is never read; an empty list reads the whole file.
pub async fn run(
    app: &AppHandle,
    video_path: &str,
    previews: bool,
    include_ranges: Vec<(f64, f64)>,
    on_progress: impl Fn(OcrProgress) + Send + Sync + 'static,
) -> Result<VideoTextTimeline, String> {
    let media = std::path::PathBuf::from(video_path);
    if !media.exists() {
        return Err(format!("video not found: {video_path}"));
    }

    // Only announce a download when there is one: a phase that flashes on every run trains the reader to ignore it.
    let progress = Arc::new(on_progress);
    let paths = if models::models_present(app) {
        models::model_paths(app)?
    } else {
        let sink = Arc::clone(&progress);
        let mut throttle = Throttle::new(move |p| sink(p));
        throttle.send(OcrProgress::new("downloading", 0, 0, 0), true);
        models::ensure_models(app, |done, total| {
            throttle.send(
                OcrProgress::new("downloading", done, total, 0),
                total > 0 && done >= total,
            );
        })
        .await?
    };

    let finished = Arc::clone(&progress);
    let det = paths.detection;
    let rec = paths.recognition;
    let timeline = tokio::task::spawn_blocking(move || -> Result<VideoTextTimeline, String> {
        let (_, _, duration) = probe_dims(&media)?;

        // OCR dominates, and debug-build rten inference is orders of magnitude slower, so these timings ride out on the result.
        let sink = Arc::clone(&progress);
        let mut throttle = Throttle::new(move |p| sink(p));

        let t0 = Instant::now();
        let opts = SampleOpts {
            include_ranges,
            ..Default::default()
        };
        let mut scanned = 0u64;
        let frames = sample_frames(&media, &opts, &mut |tick| {
            scanned = tick.scanned;
            throttle.send(
                OcrProgress::new("sampling", tick.scanned, tick.total, tick.kept),
                tick.total > 0 && tick.scanned >= tick.total,
            );
        })?;
        let sample_ms = t0.elapsed().as_millis() as u64;

        // Close the bar at what it actually walked: the duration-derived estimate can undershoot and park it at 97%.
        throttle.send(
            OcrProgress::new("sampling", scanned, scanned, frames.len() as u64),
            true,
        );

        let t1 = Instant::now();
        let engine = build_engine(&det, &rec)?;
        let model_load_ms = t1.elapsed().as_millis() as u64;

        let t2 = Instant::now();
        throttle.send(OcrProgress::new("reading", 0, frames.len() as u64, 0), true);
        let mut timeline = build_timeline(
            &frames,
            duration,
            engine.as_ref(),
            &TimelineOpts { previews },
            &mut |tick| {
                throttle.send(
                    OcrProgress::new("reading", tick.done, tick.total, tick.spans),
                    tick.done >= tick.total,
                );
            },
        )?;
        let ocr_ms = t2.elapsed().as_millis() as u64;

        timeline.stats = OcrStats {
            duration_secs: duration,
            frames_scanned: scanned as u32,
            sample_ms,
            model_load_ms,
            ocr_ms,
            ..timeline.stats
        };

        let per_frame = if frames.is_empty() {
            0
        } else {
            ocr_ms / frames.len() as u64
        };
        log::info!(
            "ocr: {:.1}s video -> {scanned} frames scanned, {} kept in {sample_ms}ms, models \
             loaded in {model_load_ms}ms, OCR {ocr_ms}ms ({per_frame}ms/frame) -> {} spans",
            duration,
            frames.len(),
            timeline.spans.len()
        );
        Ok(timeline)
    })
    .await
    .map_err(|e| format!("ocr task join: {e}"))??;

    // Terminal phase, sent only with the result in hand, so a consumer's bar and summary can't disagree.
    finished(OcrProgress::new(
        "done",
        timeline.stats.frames_read as u64,
        timeline.stats.frames_read as u64,
        timeline.spans.len() as u64,
    ));
    Ok(timeline)
}

/// The engine seam. Present only when the `ocr` feature is compiled in; otherwise
/// the whole surface still exists and reports why it cannot run.
#[cfg(feature = "ocr")]
fn build_engine(
    det: &std::path::Path,
    rec: &std::path::Path,
) -> Result<Box<dyn super::engine::OcrEngine>, String> {
    Ok(Box::new(super::engine::OcrsEngine::new(det, rec)?))
}

#[cfg(not(feature = "ocr"))]
fn build_engine(
    _det: &std::path::Path,
    _rec: &std::path::Path,
) -> Result<Box<dyn super::engine::OcrEngine>, String> {
    Err("on-device OCR is not available in this build".into())
}

/// Reads a recording into a timestamped, structured text timeline. Experimental: surfaced only by the editor's dev-only OCR tab today.
/// `includeRanges` are the source-second pairs the edit keeps, passed in because the caller owns the edit state rather than the backend re-deriving it.
#[tauri::command]
pub async fn read_video_text(
    app: AppHandle,
    video_path: String,
    previews: bool,
    include_ranges: Vec<[f64; 2]>,
    on_phase: Channel<OcrProgress>,
) -> AppResult<VideoTextTimeline> {
    let ranges = include_ranges.into_iter().map(|r| (r[0], r[1])).collect();
    let timeline = run(&app, &video_path, previews, ranges, move |p| {
        let _ = on_phase.send(p);
    })
    .await?;
    Ok(timeline)
}

/// Writes an already-serialized read to a caller-chosen `dest_path`; the caller owns the format, since the timeline is a plain frontend object that serializes there.
/// This command owns only the disk write, so it needs no `ocr` feature.
#[tauri::command]
pub async fn export_screen_text(body: String, dest_path: String) -> AppResult<()> {
    tokio::fs::write(&dest_path, body)
        .await
        .map_err(|e| crate::commands::error::AppError::msg(format!("write screen text: {e}")))
}
