//! Inference seam — where an on-device model is actually run.
//!
//! There is one on-device engine: transcribe.cpp (ggml), which runs any
//! supported GGUF file. A model is a single `.gguf` under its model dir; the file
//! decides the architecture, so there's no per-architecture dispatch here. The
//! real work lives in `ggml.rs` (behind the `ggml` Cargo feature). Remote models
//! never reach this seam — `transcribe_project` posts them over HTTP first.

use std::path::Path;

use super::models::CaptionModel;
use super::Transcript;

/// Run a local model over 16 kHz mono f32 PCM. Resolves the model's single GGUF
/// file and hands it to the ggml engine. `language` is an optional ISO hint
/// (`None` = autodetect).
#[cfg(feature = "ggml")]
pub fn transcribe(
    model: &CaptionModel,
    model_dir: &Path,
    samples: &[f32],
    language: Option<&str>,
) -> Result<Transcript, String> {
    let file = model
        .files
        .first()
        .ok_or_else(|| format!("model '{}' has no GGUF file defined", model.id))?;
    let path = model_dir.join(&file.rel_path);
    transcribe_at_path(&path, &model.id, samples, language)
}

/// Path-keyed sibling of [`transcribe`]. Skips the registry lookup + dir join —
/// the caller already has the GGUF file in hand. Used by `transcribe_for_paths`
/// in `mod.rs` and reaches `ggml::transcribe_gguf` directly. `language` is an
/// optional ISO source-language hint (`None` = autodetect).
#[cfg(feature = "ggml")]
pub fn transcribe_at_path(
    model_path: &Path,
    model_id: &str,
    samples: &[f32],
    language: Option<&str>,
) -> Result<Transcript, String> {
    super::ggml::transcribe_gguf(model_path, model_id, samples, language)
}

/// Fallback for a build without the on-device engine (`--no-default-features`):
/// on-device transcription is unavailable, so only remote endpoints can run.
#[cfg(not(feature = "ggml"))]
pub fn transcribe(
    _model: &CaptionModel,
    _model_dir: &Path,
    _samples: &[f32],
    _language: Option<&str>,
) -> Result<Transcript, String> {
    Err("On-device transcription isn't available in this build.".into())
}

#[cfg(not(feature = "ggml"))]
pub fn transcribe_at_path(
    _model_path: &Path,
    _model_id: &str,
    _samples: &[f32],
    _language: Option<&str>,
) -> Result<Transcript, String> {
    Err("On-device transcription isn't available in this build.".into())
}
