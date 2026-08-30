//! Offline captions / transcription (M1 foundation).
//!
//! Transcribes a *recorded clip's* audio on-device — Recast doesn't capture a
//! live mic for this (that's dictation; out of scope). The flow is:
//!   model download (verified) → FFmpeg decode to 16 kHz mono f32 → engine.
//!
//! Everything here is async + `spawn_blocking` for CPU/FFmpeg work — sync Tauri
//! commands freeze the macOS WebView (see the recording-IPC hardening). On-device
//! inference runs through the ggml engine (see `engine.rs` / `ggml.rs`); remote
//! endpoints post audio over HTTP (see `remote.rs`).
//!
//! Full design: `apps/desktop/docs/captions-transcription-plan.md`.

mod audio;
mod cancel;
mod capabilities;
mod engine;
// On by default; absent in a `--no-default-features` build, which then transcribes via remote endpoints.
#[cfg(feature = "ggml")]
mod ggml;
mod models;
mod packs;
pub(crate) mod remote;
pub(crate) mod subtitles;
pub(crate) mod text_measure;
mod words;

use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{ipc::Channel, AppHandle};
use tokio::fs;

use capabilities::DeviceCapabilities;
use models::{CaptionModel, Engine as ModelEngine, ModelSource, Runtime};

use crate::commands::error::{AppError, AppResult};

// Reused by silence detection to fetch the Silero VAD model; the download and verify path lives in `models`.
pub(crate) use models::{download_file, models_dir};

// - Transcript data model (mirrors the planned project-format `transcript` section) -

// The model lives in `recast-captions`, shared with the compositor so burn-in and preview can't disagree.
pub use recast_captions::{CaptionAnimation, CaptionStyle, TranscriptWord};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub id: String,
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub words: Vec<TranscriptWord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub engine: String,
    pub model_id: String,
    pub language: Option<String>,
    pub segments: Vec<TranscriptSegment>,
}

/// Flattened model row for the UI (registry meta + on-disk install state).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionModelInfo {
    pub id: String,
    pub display_name: String,
    pub engine: ModelEngine,
    /// Inference backend (derived from `engine`). The axis the UI gates
    /// availability on; several engines share one runtime.
    pub runtime: Runtime,
    /// Built-in vs. contributed by an installed extension (provenance badge).
    pub source: ModelSource,
    pub family: String,
    pub languages: Vec<String>,
    pub approx_size_bytes: Option<u64>,
    pub is_default: bool,
    pub installed: bool,
    /// False for a model with no files defined (e.g. a remote endpoint).
    pub downloadable: bool,
    pub requires_gpu: bool,
    pub prefers_gpu: bool,
    pub min_ram_bytes: Option<u64>,
    /// Can this device run the model at all? (false → hard-disabled in the UI).
    pub runnable: bool,
    /// Is this model's runtime usable in this build? False when the on-device
    /// engine isn't compiled in (`--no-default-features`) or a remote endpoint has
    /// no key. Download is still allowed; only Generate is gated on this.
    pub runtime_available: bool,
    /// Non-blocking caveat for this device (slow on CPU, low RAM, …), or the
    /// reason the runtime is unavailable when that's the blocker.
    pub warning: Option<String>,
    // - catalog presentation (passed through from the registry) -
    pub capabilities: models::ModelCapabilities,
    pub language_count: Option<u32>,
    pub speed_score: Option<u8>,
    pub accuracy_score: Option<u8>,
    pub recommended: bool,
}

/// Decide whether a model can run on this device and what to warn about.
/// Hard requirement (GPU) disables; soft factors (CPU-slow, low RAM) warn.
fn evaluate(model: &CaptionModel, caps: &DeviceCapabilities) -> (bool, Option<String>) {
    if model.requires_gpu && !caps.gpu.available {
        return (
            false,
            Some("Requires a supported GPU on this device.".into()),
        );
    }
    let mut notes: Vec<String> = Vec::new();
    if model.prefers_gpu && !caps.gpu.available {
        notes.push("Runs on CPU here — expect slower transcription.".into());
    }
    if let (Some(min), Some(ram)) = (model.min_ram_bytes, caps.total_ram_bytes) {
        if ram < min {
            notes.push(format!(
                "Recommended {:.0} GB RAM; this device has {:.1} GB.",
                min as f64 / 1e9,
                ram as f64 / 1e9
            ));
        }
    }
    (true, (!notes.is_empty()).then(|| notes.join(" ")))
}

// - Channel payloads -

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadProgress {
    model_id: String,
    /// File currently downloading (empty on the final "complete" tick).
    file: String,
    downloaded: u64,
    /// 0 when the server didn't report a content length.
    total: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranscribeProgress {
    phase: String, // "extracting" | "transcribing" | "done"
}

// - Commands -

/// Catalog + per-model install state. Cheap disk checks; async to honour the
/// no-sync-commands rule.
#[tauri::command]
pub async fn list_caption_models(app: AppHandle) -> AppResult<Vec<CaptionModelInfo>> {
    let caps = capabilities::detect();
    let infos = models::all_models(&app)
        .into_iter()
        .map(|m| {
            let runtime = m.engine.runtime();
            let downloadable = !m.files.is_empty();
            // A remote endpoint is available when a key is configured; a local model needs runtime, device caps and files.
            let (installed, runnable, runtime_available, warning) = match m.remote.as_ref() {
                Some(ep) => {
                    let has_key = remote::has_key(&ep.id);
                    let warn = (!has_key)
                        .then(|| "Add an API key for this endpoint to use it.".to_string());
                    (has_key, true, has_key, warn)
                }
                None => {
                    let installed = models::is_installed(&app, &m).unwrap_or(false);
                    let (rt_available, rt_reason) = models::runtime_status(runtime);
                    let (runnable, device_warning) = evaluate(&m, &caps);
                    // An unavailable runtime is the dominant blocker, so its reason wins over a soft device caveat.
                    (
                        installed,
                        runnable,
                        rt_available,
                        rt_reason.or(device_warning),
                    )
                }
            };
            CaptionModelInfo {
                id: m.id,
                display_name: m.display_name,
                engine: m.engine,
                runtime,
                source: m.source,
                family: m.family,
                languages: m.languages,
                approx_size_bytes: m.approx_size_bytes,
                is_default: m.is_default,
                installed,
                downloadable,
                requires_gpu: m.requires_gpu,
                prefers_gpu: m.prefers_gpu,
                min_ram_bytes: m.min_ram_bytes,
                runnable,
                runtime_available,
                warning,
                capabilities: m.capabilities,
                language_count: m.language_count,
                speed_score: m.speed_score,
                accuracy_score: m.accuracy_score,
                recommended: m.recommended,
            }
        })
        .collect();
    Ok(infos)
}

/// Report this device's OS / arch / RAM / GPU so the UI can explain why a model
/// is disabled or warned.
#[tauri::command]
pub async fn caption_capabilities() -> AppResult<DeviceCapabilities> {
    Ok(capabilities::detect())
}

/// Download every file for a model, streaming progress on the `on_progress`
/// channel (request-scoped — one channel per download, so the caller never has
/// to correlate ticks to a model id).
#[tauri::command]
pub async fn download_caption_model(
    app: AppHandle,
    id: String,
    on_progress: Channel<DownloadProgress>,
) -> AppResult<()> {
    let model = models::find(&app, &id)
        .ok_or_else(|| AppError::msg(format!("unknown caption model: {id}")))?;
    if model.files.is_empty() {
        return Err(AppError::msg(format!(
            "model '{id}' has no downloadable files defined yet"
        )));
    }
    let dir = models::model_dir(&app, &id)?;
    fs::create_dir_all(&dir)
        .await
        .map_err(|e| AppError::msg(format!("create model dir: {e}")))?;

    let client = reqwest::Client::builder()
        .user_agent("recast-desktop")
        .build()
        .map_err(|e| AppError::msg(format!("client: {e}")))?;

    for f in &model.files {
        let dest = dir.join(&f.rel_path);
        let rel = f.rel_path.clone();
        models::download_file(
            &client,
            &f.url,
            f.sha256.as_deref(),
            &dest,
            |downloaded, total| {
                let _ = on_progress.send(DownloadProgress {
                    model_id: id.clone(),
                    file: rel.clone(),
                    downloaded,
                    total,
                });
            },
        )
        .await?;
    }

    let _ = on_progress.send(DownloadProgress {
        model_id: id.clone(),
        file: String::new(),
        downloaded: 1,
        total: 1,
    });
    Ok(())
}

/// Remove a downloaded model's files.
#[tauri::command]
pub async fn delete_caption_model(app: AppHandle, id: String) -> AppResult<()> {
    let dir = models::model_dir(&app, &id)?;
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .await
            .map_err(|e| AppError::msg(format!("delete model: {e}")))?;
    }
    Ok(())
}

/// Transcribe a recording's audio with the chosen model. Decode + inference run
/// on a blocking thread; phase events drive the UI.
#[tauri::command]
pub async fn transcribe_project(
    app: AppHandle,
    audio_path: Option<String>,
    microphone_path: Option<String>,
    model_id: String,
    language: Option<String>,
    on_phase: Channel<TranscribeProgress>,
) -> AppResult<Transcript> {
    cancel::begin();
    let model = models::find(&app, &model_id)
        .ok_or_else(|| AppError::msg(format!("unknown caption model: {model_id}")))?;

    // Availability gate: a remote model needs a stored key, a local one its runtime, device support and files.
    let remote_ep = model.remote.clone();
    match remote_ep.as_ref() {
        Some(ep) => {
            if !remote::has_key(&ep.id) {
                return Err(AppError::from("Add an API key for this endpoint first."));
            }
        }
        None => {
            let (runtime_available, runtime_reason) =
                models::runtime_status(model.engine.runtime());
            if !runtime_available {
                return Err(AppError::msg(runtime_reason.unwrap_or_else(|| {
                    format!("model '{model_id}' runtime is unavailable in this build")
                })));
            }
            let (runnable, _) = evaluate(&model, &capabilities::detect());
            if !runnable {
                return Err(AppError::msg(format!(
                    "model '{model_id}' can't run on this device"
                )));
            }
            if !models::is_installed(&app, &model)? {
                return Err(AppError::msg(format!(
                    "model '{model_id}' is not downloaded"
                )));
            }
        }
    }

    let _ = on_phase.send(TranscribeProgress {
        phase: "extracting".into(),
    });

    // Decode to 16 kHz mono f32 off the UI thread (CPU plus ffmpeg), for both paths.
    let sources: Vec<String> = [audio_path, microphone_path]
        .into_iter()
        .flatten()
        .collect();
    let samples: Vec<f32> = tokio::task::spawn_blocking(move || {
        let refs: Vec<&str> = sources.iter().map(|s| s.as_str()).collect();
        let samples = audio::extract_pcm_f32(&refs)?;
        if samples.is_empty() {
            return Err("no audio to transcribe".to_string());
        }
        Ok::<Vec<f32>, String>(samples)
    })
    .await
    .map_err(|e| AppError::msg(format!("audio extract task panicked: {e}")))??;

    // Extract runs as one ffmpeg call, so this is the first point it can stop.
    if cancel::is_requested() {
        return Err(AppError::from(cancel::CANCELLED_MSG));
    }

    let _ = on_phase.send(TranscribeProgress {
        phase: "transcribing".into(),
    });

    let transcript = match remote_ep.as_ref() {
        // Remote: the network call is async, so it must NOT run on a blocking thread. The key never crosses IPC.
        Some(ep) => {
            let key = remote::read_key(&ep.id)
                .ok_or_else(|| AppError::from("Add an API key for this endpoint first."))?;
            remote::transcribe_remote(ep, &key, &samples, language.as_deref())
                .await
                .map_err(AppError::msg)?
        }
        // Local: inference is CPU-bound, so run it on a blocking thread.
        None => {
            let model_dir = models::model_dir(&app, &model.id)?;
            let lang = language.clone();
            tokio::task::spawn_blocking(move || {
                engine::transcribe(&model, &model_dir, &samples, lang.as_deref())
            })
            .await
            .map_err(|e| AppError::msg(format!("transcription task panicked: {e}")))??
        }
    };

    // The remote path can't be interrupted mid-request, so drop its result rather than overwrite a stopped transcript.
    if cancel::is_requested() {
        return Err(AppError::from(cancel::CANCELLED_MSG));
    }

    let _ = on_phase.send(TranscribeProgress {
        phase: "done".into(),
    });
    Ok(transcript)
}

/// Ask the in-flight transcription to stop. No-op when nothing is running.
#[tauri::command]
pub fn cancel_transcription() {
    cancel::request();
}

/// Path-aware counterpart to [`transcribe_project`]. Identical pipeline
/// (`audio::extract_pcm_f32` → `engine::transcribe_at_path` →
/// `words::build_segments`) but no `AppHandle`, no `Channel<TranscribeProgress>`,
/// no model-registry lookup — the caller supplies the audio path, the GGUF
/// path, and the model id directly. Used by the CLI `transcribe` verb
/// (`apps/desktop/src-tauri/src/cli.rs`) and the CI / release smoke test
/// (`scripts/release/smoke-test-transcription.ps1`).
///
/// Streams the three phases to stderr (one line each) so a CLI observer sees
/// progress. The frontend's IPC `Channel` is a no-op equivalent for scripts.
pub async fn transcribe_for_paths(
    audio_path: &Path,
    model_path: &Path,
    model_id: &str,
    language: Option<&str>,
) -> AppResult<Transcript> {
    if !audio_path.exists() {
        return Err(AppError::msg(format!(
            "audio file not found: {}",
            audio_path.display()
        )));
    }
    if !model_path.exists() {
        return Err(AppError::msg(format!(
            "model file not found: {}",
            model_path.display()
        )));
    }

    eprintln!("phase: extracting");
    let audio_owned = audio_path.to_string_lossy().into_owned();
    let samples: Vec<f32> = tokio::task::spawn_blocking(move || {
        let sources = [audio_owned.as_str()];
        let samples = audio::extract_pcm_f32(&sources)?;
        if samples.is_empty() {
            return Err("no audio to transcribe".to_string());
        }
        Ok::<Vec<f32>, String>(samples)
    })
    .await
    .map_err(|e| AppError::msg(format!("audio extract task panicked: {e}")))??;

    eprintln!("phase: transcribing");
    let model_path_owned = model_path.to_path_buf();
    let model_id_owned = model_id.to_string();
    let language_owned = language.map(|s| s.to_string());
    let transcript = tokio::task::spawn_blocking(move || {
        engine::transcribe_at_path(
            &model_path_owned,
            &model_id_owned,
            &samples,
            language_owned.as_deref(),
        )
    })
    .await
    .map_err(|e| AppError::msg(format!("transcription task panicked: {e}")))??;

    eprintln!("phase: done");
    Ok(transcript)
}

/// True when at least one of the given media files actually carries an audio
/// stream. The caption tab gates its Generate UI on this: a recording can have
/// a video path but no audio track (recorded with mic + system audio off), and
/// there's nothing to transcribe then. ffprobe subprocess → async + blocking.
#[tauri::command]
pub async fn has_transcribable_audio(paths: Vec<String>) -> AppResult<bool> {
    tokio::task::spawn_blocking(move || {
        paths
            .iter()
            .filter(|p| !p.trim().is_empty())
            .any(|p| crate::commands::ffmpeg::has_audio(std::path::Path::new(p)))
    })
    .await
    .map_err(|e| AppError::msg(format!("audio probe task panicked: {e}")))
}

// - Remote transcription endpoints (OpenAI-compatible) -

/// A configured remote endpoint plus whether its API key is stored. Never
/// carries the key itself.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAsrEndpointInfo {
    #[serde(flatten)]
    endpoint: remote::RemoteEndpoint,
    has_key: bool,
}

/// List configured remote endpoints (with key-present flags, not the keys).
#[tauri::command]
pub async fn list_remote_asr_endpoints(app: AppHandle) -> AppResult<Vec<RemoteAsrEndpointInfo>> {
    Ok(remote::read_endpoints(&app)
        .into_iter()
        .map(|ep| {
            let has_key = remote::has_key(&ep.id);
            RemoteAsrEndpointInfo {
                endpoint: ep,
                has_key,
            }
        })
        .collect())
}

/// Add or update a remote endpoint's (non-secret) config. Returns the stored,
/// normalized form.
#[tauri::command]
pub async fn set_remote_asr_endpoint(
    app: AppHandle,
    endpoint: remote::RemoteEndpoint,
) -> AppResult<remote::RemoteEndpoint> {
    remote::upsert_endpoint(&app, endpoint).map_err(AppError::msg)
}

/// Remove a remote endpoint and its stored key.
#[tauri::command]
pub async fn delete_remote_asr_endpoint(app: AppHandle, id: String) -> AppResult<()> {
    remote::delete_endpoint(&app, &id).map_err(AppError::msg)
}

/// Store (or, with an empty value, clear) a remote endpoint's API key in the OS
/// keyring. Write-only by design: there is no command that returns the key.
#[tauri::command]
pub async fn set_remote_asr_key(app: AppHandle, id: String, key: String) -> AppResult<()> {
    if !remote::read_endpoints(&app).iter().any(|e| e.id == id) {
        return Err(AppError::from("unknown remote endpoint"));
    }
    if key.trim().is_empty() {
        return remote::delete_key(&id).map_err(AppError::msg);
    }
    remote::store_key(&id, &key).map_err(AppError::msg)
}

/// Serialize a transcript to a subtitle sidecar (`srt` | `vtt`) and write it to
/// `dest_path` (chosen by the caller via the save dialog).
#[tauri::command]
pub async fn export_captions(
    transcript: Transcript,
    format: String,
    dest_path: String,
) -> AppResult<()> {
    let body = match format.as_str() {
        "srt" => subtitles::to_srt(&transcript),
        "vtt" => subtitles::to_vtt(&transcript),
        other => {
            return Err(AppError::msg(format!(
                "unsupported subtitle format: {other}"
            )))
        }
    };
    fs::write(&dest_path, body)
        .await
        .map_err(|e| AppError::msg(format!("write subtitles: {e}")))
}

#[cfg(test)]
mod tests {
    use super::{CaptionAnimation, CaptionStyle};

    #[test]
    fn default_animation_is_static() {
        // The default spec (line chunks, no emphasis, no entrance) must take the static one-Dialogue-per-line path.
        assert!(CaptionAnimation::default().is_static());
    }

    #[test]
    fn any_visible_effect_makes_it_non_static() {
        let word_chunked = CaptionAnimation {
            chunk: "word".into(),
            ..Default::default()
        };
        assert!(!word_chunked.is_static());

        let emphasized = CaptionAnimation {
            emphasis: "color".into(),
            ..Default::default()
        };
        assert!(!emphasized.is_static());

        let with_entrance = CaptionAnimation {
            entrance: "fade".into(),
            ..Default::default()
        };
        assert!(!with_entrance.is_static());
    }

    #[test]
    fn highlight_resolves_absent_to_active() {
        // A pre-highlight project keeps the legacy per-word behaviour, while a fresh default is static.
        let legacy = CaptionAnimation {
            highlight: None,
            ..Default::default()
        };
        assert_eq!(legacy.highlight(), "active");
        assert_eq!(CaptionAnimation::default().highlight(), "none");
    }

    #[test]
    fn caption_style_default_mirrors_loom_preset() {
        // Guards Rust/TS default drift: these must equal DEFAULT_CAPTION_STYLE in @recast/captions. Update both together.
        let d = CaptionStyle::default();
        assert_eq!(d.font_weight, 600);
        assert_eq!(d.font_size_pct, 3.8);
        assert_eq!(d.background, "box");
        assert_eq!(d.muted_color, "#a1a1aa");
        assert_eq!(d.max_chars_per_line, 42);
        assert_eq!(d.animation.as_ref().unwrap().highlight(), "progressive");
    }
}
