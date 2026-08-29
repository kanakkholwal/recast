use std::fs;
use std::path::PathBuf;
use tauri::ipc::Channel;

use chrono::{Local, TimeZone};
use tauri::{Emitter, State};

use super::error::{AppError, AppResult};
use super::system::get_active_output_dir;
use super::types::{AppState, RecordingEntry, RecordingStartResult};
use crate::capture::{CaptureTarget, RegionRect};
use crate::project::writer::{write_project, ProjectWriteRequest};
use crate::project::{ProjectMediaMetadata, ProjectMetadata, ProjectVideoMetadata};
use crate::recording::{CameraPreviewUpdate, RecordingOptions};
use crate::render::graph::RenderState;

fn recasts_dir(state: &State<'_, AppState>) -> PathBuf {
    let dir = get_active_output_dir(state).join("recasts");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn exports_dir(state: &State<'_, AppState>) -> PathBuf {
    let dir = get_active_output_dir(state).join("exports");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// A capture notice on its way to the UI.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptureNoticePayload {
    message: String,
    /// True when nothing more can be recorded, so the UI stops rather than warns.
    terminal: bool,
}

#[tauri::command]
pub async fn start_recording(
    app: tauri::AppHandle,
    target_type: String,
    target_id: u64,
    region: Option<RegionRect>,
    options: Option<RecordingOptions>,
    state: State<'_, AppState>,
) -> AppResult<RecordingStartResult> {
    // Resolving the capture target enumerates monitors/windows (xcap's
    // `Monitor::all`/`Window::all` can stall hundreds of ms), on Wayland
    // negotiates the xdg-desktop-portal dialog, and `start()` then spawns the
    // capture/encoder/audio/camera processes — all blocking. Tauri runs sync
    // commands on the main thread, which on macOS (WKWebView) and Linux
    // (WebKitGTK) also renders the UI, so doing this inline froze the window
    // while "Start" was pressed (Windows' out-of-process WebView2 masked it).
    // Mirror `stop_recording`: push the whole blocking body onto a worker so
    // the UI thread — including the Wayland portal dialog — stays responsive.
    let manager = state.recording_manager.clone();
    let output_dir = get_active_output_dir(&state);
    // A recording repeating one frame looks exactly like a working one.
    let notifier = app.clone();
    let notify = move |notice: crate::capture::CaptureNotice| {
        let payload = CaptureNoticePayload {
            message: notice.message().to_string(),
            terminal: notice.is_terminal(),
        };
        if let Err(e) = notifier.emit("recording:capture-notice", payload) {
            log::warn!("could not deliver a capture notice to the UI: {e}");
        }
    };

    let outcome =
        tauri::async_runtime::spawn_blocking(move || -> AppResult<RecordingStartResult> {
            // Advisory under the Wayland portal, where the user picks the real surface.
            let target = if target_type == "region" {
                let rect = region.ok_or_else(|| AppError::from("region target requires a rect"))?;
                CaptureTarget::resolve_region(rect)?
            } else {
                CaptureTarget::resolve(&target_type, target_id)?
            };
            let warnings = manager
                .start(target, output_dir, options.unwrap_or_default(), notify)
                .inspect_err(|e| log::error!("start_recording failed: {e:#}"))?;
            Ok(RecordingStartResult { warnings })
        })
        .await
        .map_err(|e| AppError::msg(format!("start_recording worker panicked: {e}")))?;

    // Keep display + system awake for the capture (released in stop_recording).
    // Only on success so a failed start doesn't leak a hold.
    if outcome.is_ok() {
        state.power.acquire();
        // Broadcast so observers (panel transport, tray, `recast watch`) reflect
        // the recording regardless of who started it (UI button or CLI).
        let _ = app.emit(
            "recording:started",
            serde_json::json!({ "startedAtUnixMs": Local::now().timestamp_millis() }),
        );
    }
    outcome
}

#[tauri::command]
pub async fn stop_recording(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<String> {
    // `stop()` joins the capture/cursor/encoder threads, finalizes the muxer,
    // stops the audio/mic sessions, and `write_project` zips the media to disk.
    // All of it is blocking CPU/IO.
    //
    // Tauri runs *synchronous* commands directly on the main thread. On macOS
    // the WebView renders on that same thread, so running this inline froze the
    // entire window after every recording — clicks/drag stopped landing until
    // the work finished. Windows' out-of-process WebView2 kept painting, which
    // is why the hang was macOS-only. Mirror `get_displays`/`export_video`: make
    // the command `async` and push the whole blocking body onto a worker so the
    // UI thread stays free to paint the "Saving…" transition. See the matching
    // note on `AppState::recording_manager` (it's an `Arc` for this reason).
    let manager = state.recording_manager.clone();
    let dest = recasts_dir(&state);

    let (project_path, warnings) =
        tauri::async_runtime::spawn_blocking(move || -> AppResult<(PathBuf, Vec<String>)> {
            // `{:#}` formats the full anyhow chain (top message + every `.context()`
            // below it), so the JS-side alert sees the real cause instead of just
            // the outermost label. Without this, errors like "encoder thread
            // panicked" hid the underlying FFmpeg-process exit code.
            let artifacts = manager
                .stop()
                .inspect_err(|e| log::error!("stop_recording failed: {e:#}"))?;
            // Non-fatal capture issues (e.g. mic/camera failed) to toast after save.
            let warnings = artifacts.warnings.clone();
            // Human-readable, sortable, searchable name (local time of capture) —
            // e.g. `Recast_2026-05-16_14-30-22.recast`.
            let stamp = Local
                .timestamp_millis_opt(artifacts.started_at_unix_ms as i64)
                .single()
                .unwrap_or_else(Local::now)
                .format("%Y-%m-%d_%H-%M-%S");
            let final_path = super::unique_path(&dest, &format!("Recast_{stamp}"), "recast");
            // The recording pipeline is the authoritative source for these values
            // (crop dimensions from `CaptureTarget`, FPS pinned by the pacer at 60).
            // Spawning ffprobe here just to confirm what we already know was
            // adding 100–300ms to every stop, right when the UI wants to transition.
            let media_duration_ms =
                if artifacts.stats.encoded_frames > 0 && artifacts.stats.nominal_fps > 0 {
                    (artifacts.stats.encoded_frames as f64 / artifacts.stats.nominal_fps as f64
                        * 1000.0)
                        .round() as u64
                } else {
                    artifacts.stats.duration_ms
                };
            let metadata = ProjectMetadata {
                schema_version: 1,
                created_at_unix_ms: artifacts.started_at_unix_ms,
                capture_target: artifacts.capture_target.clone(),
                stats: artifacts.stats.clone(),
                video: ProjectVideoMetadata {
                    width: artifacts.capture_target.crop.width,
                    height: artifacts.capture_target.crop.height,
                    // The pacer + encoder ran at the session's chosen capture rate
                    // (default 60); persist that, not a hard-coded const, so the
                    // editor and export source-fps detection are correct for
                    // high-refresh recordings.
                    fps: artifacts.stats.nominal_fps,
                    // The MEDIA's length, which is the encoded frame count at
                    // the CFR the encoder wrote — not `stats.duration_ms`, the
                    // wall clock of the session. They diverge exactly by the
                    // dropped frames, and the wall clock is always the longer
                    // of the two. `stats` keeps the wall clock; this is the
                    // number the editor and the export validator agree on.
                    duration_ms: media_duration_ms,
                },
                media: Some(ProjectMediaMetadata {
                    has_system_audio: artifacts.has_system_audio,
                    has_microphone: artifacts.microphone_path.is_some(),
                    has_camera: artifacts.camera_path.is_some(),
                    camera_requested: artifacts.camera_requested,
                    track_offsets: artifacts.track_offsets,
                }),
            };
            let default_render_state = RenderState {
                trim_end: media_duration_ms as f64 / 1000.0,
                camera_overlay: artifacts.camera_overlay.clone(),
                ..RenderState::default()
            };
            let project_path = write_project(ProjectWriteRequest {
                output_path: final_path.clone(),
                metadata,
                recording_path: artifacts.recording_path.clone(),
                cursor_path: artifacts.cursor_path.clone(),
                audio_path: Some(artifacts.audio_path.clone()),
                microphone_path: artifacts.microphone_path.clone(),
                camera_path: artifacts.camera_path.clone(),
                edits_json: serde_json::to_string_pretty(&default_render_state)
                    .unwrap_or_else(|_| "{}".into()),
            })
            .inspect_err(|e| log::error!("write_project failed: {e:#}"))?;

            // Clean up temporary session files.
            let _ = fs::remove_file(&artifacts.recording_path);
            let _ = fs::remove_file(&artifacts.cursor_path);
            let _ = fs::remove_file(&artifacts.audio_path);
            if let Some(ref mic_path) = artifacts.microphone_path {
                let _ = fs::remove_file(mic_path);
            }
            if let Some(ref cam_path) = artifacts.camera_path {
                let _ = fs::remove_file(cam_path);
            }

            Ok((project_path, warnings))
        })
        .await
        .map_err(|e| AppError::msg(format!("stop_recording worker panicked: {e}")))??;

    // Finalized: release the sleep inhibitor from start_recording (success path).
    state.power.release();

    // Surface non-fatal capture issues (mic/camera failed to record). The
    // recording still saved; the frontend shows these as a warning toast.
    if !warnings.is_empty() {
        let _ = app.emit("recording:warnings", &warnings);
    }

    *state.last_file_path.lock() = Some(project_path.to_string_lossy().to_string());
    let path_str = project_path.to_string_lossy().to_string();
    // Broadcast so the panel/tray return to idle even for a CLI- or timeout-
    // driven stop.
    let _ = app.emit(
        "recording:stopped",
        serde_json::json!({ "projectPath": path_str }),
    );
    Ok(path_str)
}

#[tauri::command]
pub fn pause_recording(state: State<'_, AppState>) -> AppResult<()> {
    Ok(state.recording_manager.pause()?)
}

#[tauri::command]
pub fn resume_recording(state: State<'_, AppState>) -> AppResult<()> {
    Ok(state.recording_manager.resume()?)
}

#[tauri::command]
pub fn is_recording_paused(state: State<'_, AppState>) -> AppResult<bool> {
    Ok(state.recording_manager.is_paused())
}

#[tauri::command]
pub fn update_camera_preview_state(
    state: CameraPreviewUpdate,
    app_state: State<'_, AppState>,
) -> AppResult<()> {
    Ok(app_state
        .recording_manager
        .update_camera_preview_state(state)?)
}

/// Open the camera and stream preview frames to the caller.
///
/// Cameras are exclusive, so this is also what takes the device away from the
/// WebView: nothing else may hold it while a recording is running. Each frame is
/// `width: u32le, height: u32le` then BGRA rows, reduced to preview size.
#[tauri::command]
pub async fn start_camera_preview(
    device: String,
    on_frame: Channel<tauri::ipc::InvokeResponseBody>,
) -> AppResult<crate::camera::session::CameraGeometry> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::camera::session::start(
            &device,
            Box::new(move |frame| {
                let _ = on_frame.send(tauri::ipc::InvokeResponseBody::Raw(frame));
            }),
        )
    })
    .await
    .map_err(|e| AppError::msg(format!("start_camera_preview join error: {e}")))?
    // `{:#}` for the whole chain: the outer message hides why the open failed.
    .map_err(|e| AppError::msg(format!("{e:#}")))
}

/// Release the camera held by `session`. Ignores a stale token, so the preview
/// window being replaced cannot close the camera its replacement just opened.
#[tauri::command]
pub async fn stop_camera_preview(session: u64) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || crate::camera::session::stop(session))
        .await
        .map_err(|e| AppError::msg(format!("stop_camera_preview join error: {e}")))
}

// `list_recasts`/`list_exports` are async + spawn_blocking: the scan does a
// `read_dir` plus a `metadata()` stat per file, which adds up to hundreds of ms
// on a library with many recordings — and on macOS/Linux a sync command runs on
// the UI thread. Resolve the dir up front (cheap config read), then scan off the
// main thread.
#[tauri::command]
pub async fn list_recasts(state: State<'_, AppState>) -> AppResult<Vec<RecordingEntry>> {
    let dir = recasts_dir(&state);
    tauri::async_runtime::spawn_blocking(move || list_files_by_ext(&dir, &["recast"]))
        .await
        .map_err(|e| AppError::msg(format!("list_recasts join error: {e}")))?
}

#[tauri::command]
pub async fn list_exports(state: State<'_, AppState>) -> AppResult<Vec<RecordingEntry>> {
    let dir = exports_dir(&state);
    tauri::async_runtime::spawn_blocking(move || list_files_by_ext(&dir, &["mp4", "webm", "gif"]))
        .await
        .map_err(|e| AppError::msg(format!("list_exports join error: {e}")))?
}

/// WebVTT for the caption sidecar next to `media_path` (e.g. `foo.mp4` →
/// `foo.vtt`/`foo.srt`), or `None` when neither exists. Lets the player show a
/// file's captions with no loaded project. Off the main thread (a sync command
/// would freeze the macOS WebView).
#[tauri::command]
pub async fn caption_sidecar_vtt(media_path: String) -> AppResult<Option<String>> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::transcription::subtitles::read_caption_sidecar(std::path::Path::new(&media_path))
    })
    .await
    .map_err(|e| AppError::msg(format!("caption_sidecar_vtt join error: {e}")))
}

/// One pass over `dir`, collecting any file whose extension is in `exts`.
/// Sorts newest-first by mtime.
fn list_files_by_ext(dir: &PathBuf, exts: &[&str]) -> AppResult<Vec<RecordingEntry>> {
    let mut entries = Vec::new();
    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return Ok(entries),
    };

    for entry in read.flatten() {
        let path = entry.path();
        let file_ext = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or_default();
        if !exts.contains(&file_ext) {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            let modified = meta
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            // Prefer birth time so the "Recorded Jul 18" label matches when the
            // user took the recording, not when a background job last touched
            // the file. Fall back to mtime on filesystems (most Linux) where
            // birth time isn't exposed; behavior is unchanged there.
            let created = meta
                .created()
                .ok()
                .and_then(|t| t.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(modified);
            // Only `.recast` carries a format; the probe reads just the zip
            // central directory, and this whole scan is already off the main
            // thread (spawn_blocking).
            let needs_migration = file_ext == "recast" && crate::project::is_legacy_project(&path);
            entries.push(RecordingEntry {
                filename: entry.file_name().to_string_lossy().to_string(),
                path: path.to_string_lossy().to_string(),
                size_bytes: meta.len(),
                created,
                modified,
                needs_migration,
            });
        }
    }
    entries.sort_by_key(|e| std::cmp::Reverse(e.modified));
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression guard for the macOS "app freezes after recording completes"
    /// bug. `stop_recording` MUST stay `async` so its blocking body — joining
    /// the capture/encoder threads, the camera-trim FFmpeg re-encode (30s+ on a
    /// slow CPU), and zipping the `.recast` to disk — runs on a `spawn_blocking`
    /// worker rather than Tauri's main thread. macOS renders the WebView on that
    /// same main thread, so a *synchronous* `stop_recording` froze the entire
    /// window until the work finished (Windows' out-of-process WebView2 kept
    /// painting, which is why the hang was macOS-only).
    ///
    /// The closures below are type-checked but never executed (no real `State`
    /// exists in a unit test). If either command is reverted to a plain `fn`,
    /// its call yields a `Result<..>` instead of a `Future`, `drive` rejects
    /// it, and the crate stops compiling here.
    ///
    /// `start_recording` is guarded too: it enumerates monitors/windows and
    /// spawns the capture pipeline, so it must also stay off the UI thread.
    #[test]
    fn recording_commands_stay_async_off_the_ui_thread() {
        fn drive<F: std::future::Future>(_: F) {}
        let _assert_stop =
            |app: tauri::AppHandle, state: State<'_, AppState>| drive(stop_recording(app, state));
        let _assert_start = |app: tauri::AppHandle, state: State<'_, AppState>| {
            drive(start_recording(app, String::new(), 0, None, None, state))
        };
    }
}
