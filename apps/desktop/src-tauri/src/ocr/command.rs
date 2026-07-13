//! Entry point: read a video file into a structured text timeline.
//!
//! `run` is the shared core, used by the `screen.read` control method (CLI) and by
//! the `read_video_text` Tauri command (the editor's dev OCR tab). It ensures the
//! models are present, then does the CPU-heavy sampling + OCR on a blocking thread
//! so it never stalls the app.
//!
//! This module compiles with or without the `ocr` feature. Without it, `run`
//! reports that the engine is not in this build, exactly as the transcription seam
//! does for `ggml`, so the command surface stays present and degrades gracefully.

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::AppHandle;

use crate::commands::error::AppResult;

use super::frames::{probe_dims, sample_frames, SampleOpts};
use super::models;
use super::timeline::{build_timeline, TimelineOpts, VideoTextTimeline};

/// Coarse progress phases for a read.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrProgress {
    /// `"downloading"` | `"reading"` | `"done"`.
    pub phase: String,
}

/// Read `video_path` into a timeline. `on_phase` receives coarse phase labels.
/// `previews` attaches a small JPEG per span for review UIs.
///
/// `include_ranges` are the source-time ranges the edit actually keeps (the
/// segments left after trim and cuts). Pass them from the editor so removed
/// footage is never read; pass an empty list from a headless caller that has no
/// edit context, which reads the whole file.
pub async fn run(
    app: &AppHandle,
    video_path: &str,
    previews: bool,
    include_ranges: Vec<(f64, f64)>,
    on_phase: impl Fn(&str),
) -> Result<VideoTextTimeline, String> {
    let media = std::path::PathBuf::from(video_path);
    if !media.exists() {
        return Err(format!("video not found: {video_path}"));
    }

    on_phase("downloading");
    let paths = models::ensure_models(app, |_, _| {}).await?;

    on_phase("reading");
    let det = paths.detection;
    let rec = paths.recognition;
    let timeline = tokio::task::spawn_blocking(move || -> Result<VideoTextTimeline, String> {
        let (_, _, duration) = probe_dims(&media)?;

        // Per-stage timings. OCR dominates by a wide margin, and in a debug build
        // the rten inference is orders of magnitude slower than release, so these
        // numbers are the first thing to look at when a read feels slow.
        let t0 = std::time::Instant::now();
        let opts = SampleOpts {
            include_ranges,
            ..Default::default()
        };
        let frames = sample_frames(&media, &opts)?;
        let sampled_ms = t0.elapsed().as_millis();

        let t1 = std::time::Instant::now();
        let engine = build_engine(&det, &rec)?;
        let load_ms = t1.elapsed().as_millis();

        let t2 = std::time::Instant::now();
        let timeline = build_timeline(
            &frames,
            duration,
            engine.as_ref(),
            &TimelineOpts { previews },
        )?;
        let ocr_ms = t2.elapsed().as_millis();

        let per_frame = if frames.is_empty() {
            0
        } else {
            ocr_ms / frames.len() as u128
        };
        log::info!(
            "ocr: {:.1}s video -> {} frames kept in {sampled_ms}ms, models loaded in {load_ms}ms, \
             OCR {ocr_ms}ms ({per_frame}ms/frame) -> {} spans",
            duration,
            frames.len(),
            timeline.spans.len()
        );
        Ok(timeline)
    })
    .await
    .map_err(|e| format!("ocr task join: {e}"))??;

    on_phase("done");
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

/// Read a recording into a timestamped, structured text timeline.
///
/// `includeRanges` are the `[start, end]` source-second pairs the edit keeps, so
/// trimmed-off and cut-out footage is never read. The caller (the editor) owns the
/// edit state, so it passes them in rather than the backend re-deriving them.
///
/// Experimental: surfaced only by the editor's dev-only OCR tab today.
#[tauri::command]
pub async fn read_video_text(
    app: AppHandle,
    video_path: String,
    previews: bool,
    include_ranges: Vec<[f64; 2]>,
    on_phase: Channel<OcrProgress>,
) -> AppResult<VideoTextTimeline> {
    let ranges = include_ranges.into_iter().map(|r| (r[0], r[1])).collect();
    let timeline = run(&app, &video_path, previews, ranges, move |phase| {
        let _ = on_phase.send(OcrProgress {
            phase: phase.to_string(),
        });
    })
    .await?;
    Ok(timeline)
}
