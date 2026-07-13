//! Entry point: read a video file into a structured text timeline.
//!
//! `run` is the shared core (used by the `screen.read` control method and the
//! `screen-read` CLI verb). It ensures the models are present, then does the
//! CPU-heavy sampling + OCR on a blocking thread so it never stalls the app.

use tauri::AppHandle;

use super::engine::OcrsEngine;
use super::frames::{probe_dims, sample_frames, SampleOpts};
use super::models;
use super::timeline::{build_timeline, VideoTextTimeline};

/// Read `video_path` into a timeline. `on_phase` receives coarse phase labels
/// (`"downloading"`, `"reading"`, `"done"`) for progress reporting.
pub async fn run(
    app: &AppHandle,
    video_path: &str,
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
        let frames = sample_frames(&media, &SampleOpts::default())?;
        let engine = OcrsEngine::new(&det, &rec)?;
        build_timeline(&frames, duration, &engine)
    })
    .await
    .map_err(|e| format!("ocr task join: {e}"))??;

    on_phase("done");
    Ok(timeline)
}
