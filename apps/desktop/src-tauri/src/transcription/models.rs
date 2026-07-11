//! Caption model registry + verified, resumable-friendly download.
//!
//! Models are fetched **directly from HuggingFace** (decided 2026-06-29) into
//! `app_data_dir/models/<id>/`, sha256-verified when a hash is known. The
//! download/verify path mirrors `commands/assets.rs` (streamed `.tmp` + atomic
//! rename) but emits per-byte progress so the UI can show a real bar.
//!
//! NOTE ON DATA: every model is a single GGUF file hosted under the
//! `handy-computer` HuggingFace org (the canonical transcribe.cpp catalog).
//! `sha256` is left `None` for now (skip-verify with a warning) — pin the hashes
//! before release once the exact files are locked. `download`/`transcribe` guard
//! against an empty file list.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// How a model runs. There are exactly two engines: `ggml` (on-device,
/// transcribe.cpp) and `remote` (an OpenAI-compatible endpoint). The GGUF file
/// decides the model architecture for the ggml engine, so the architecture
/// (Parakeet / Whisper / ...) is display-only metadata (`CaptionModel::family`),
/// not a separate code path.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    /// On-device, transcribe.cpp over ggml. Runs any supported GGUF file.
    Ggml,
    /// Not a local architecture: transcription runs on a remote
    /// OpenAI-compatible endpoint. The server owns the real model.
    Remote,
}

/// The inference backend a model runs on. This is the axis the UI gates
/// availability on: a `ggml` build ships the on-device engine, a
/// `--no-default-features` build does not. Kept as its own enum (rather than
/// folded into `Engine`) so the UI's `runtime` / `runtimeAvailable` contract is
/// stable.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Runtime {
    /// transcribe.cpp / ggml (on-device). Present when built with `ggml`.
    Ggml,
    /// OpenAI-compatible `/audio/transcriptions` endpoint (on-device, self-hosted,
    /// or third-party). No local files; config + a keyring-held key.
    Remote,
}

impl Engine {
    /// The runtime this engine runs on. One-to-one today, but kept as a mapping so
    /// the two-axis UI contract (engine + runtime) has a single source of truth.
    pub fn runtime(self) -> Runtime {
        match self {
            Engine::Ggml => Runtime::Ggml,
            Engine::Remote => Runtime::Remote,
        }
    }
}

/// Where a catalog entry came from. Drives a provenance badge and tells the UI
/// a model is managed via the Extensions tab rather than the built-in catalog.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModelSource {
    Builtin,
    Extension,
    /// A user-configured remote OpenAI-compatible endpoint.
    Remote,
}

/// Whether a runtime can actually run in this build, plus a user-facing reason
/// when it can't. Composed with the device-capability check (`evaluate`) to
/// decide if a model is offerable.
pub fn runtime_status(runtime: Runtime) -> (bool, Option<String>) {
    match runtime {
        // The on-device engine needs the `ggml` feature (transcribe.cpp compiled
        // in). A `--no-default-features` build reports it unavailable, so the UI
        // falls back to remote endpoints.
        #[cfg(feature = "ggml")]
        Runtime::Ggml => (true, None),
        #[cfg(not(feature = "ggml"))]
        Runtime::Ggml => (
            false,
            Some("On-device transcription isn't available in this build.".into()),
        ),
        // Remote availability is decided per-endpoint (is a key stored?), so the
        // global gate is always "not available" with guidance.
        Runtime::Remote => (
            false,
            Some("Configure a remote transcription endpoint to use this model.".into()),
        ),
    }
}

/// One file that makes up a model. A ggml model is a single `.gguf` file; the
/// `rel_path` doubles as its on-disk name under the model dir.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFile {
    /// Path under the model dir, e.g. `parakeet-tdt-0.6b-v3-Q8_0.gguf`.
    pub rel_path: String,
    pub url: String,
    /// Expected sha256; `None`/empty skips verification (logged).
    #[serde(default)]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionModel {
    pub id: String,
    pub display_name: String,
    pub engine: Engine,
    /// Display group for the picker, e.g. "Parakeet" / "Whisper".
    pub family: String,
    /// BCP-47-ish language hints; `["multi"]` for multilingual.
    pub languages: Vec<String>,
    pub approx_size_bytes: Option<u64>,
    pub is_default: bool,
    pub files: Vec<ModelFile>,
    // - device requirements (drive UI gating; see capabilities.rs) -
    /// Hard requirement: no supported GPU → model is disabled.
    #[serde(default)]
    pub requires_gpu: bool,
    /// Soft: runs on CPU but is slow without a GPU → warning, not a block.
    #[serde(default)]
    pub prefers_gpu: bool,
    /// Soft: warn when the device has less than this much RAM.
    #[serde(default)]
    pub min_ram_bytes: Option<u64>,
    /// Built-in catalog entry vs. one contributed by an installed extension.
    #[serde(default = "source_builtin")]
    pub source: ModelSource,
    /// Present only for `Engine::Remote` models: the endpoint to POST audio to.
    /// `None` for every local (ggml) model.
    #[serde(default)]
    pub remote: Option<super::remote::RemoteEndpoint>,
}

fn source_builtin() -> ModelSource {
    ModelSource::Builtin
}

/// A built-in ggml model: one GGUF file from a `handy-computer` HuggingFace repo.
/// `family` is the display group for the picker; the GGUF itself tells
/// transcribe.cpp which architecture to run.
#[allow(clippy::too_many_arguments)]
fn ggml_model(
    id: &str,
    name: &str,
    family: &str,
    hf_repo: &str,
    file: &str,
    languages: Vec<String>,
    size: u64,
    is_default: bool,
) -> CaptionModel {
    let url = format!("https://huggingface.co/{hf_repo}/resolve/main/{file}");
    CaptionModel {
        id: id.into(),
        display_name: name.into(),
        engine: Engine::Ggml,
        family: family.into(),
        languages,
        approx_size_bytes: Some(size),
        is_default,
        files: vec![ModelFile {
            rel_path: file.into(),
            url,
            sha256: None, // TODO: pin before release once revisions are locked
        }],
        // ggml runs on CPU (tinyBLAS); GPU is opt-in and only speeds it up.
        requires_gpu: false,
        prefers_gpu: false,
        min_ram_bytes: Some(2_000_000_000),
        source: ModelSource::Builtin,
        remote: None,
    }
}

/// The built-in model catalog: single-file GGUF models run by the ggml
/// (transcribe.cpp) engine. Parakeet V3 (multilingual, word-timestamped) is the
/// default. Whisper is offered as an alternative family. All entries are plain
/// data and compile in every build; whether they can RUN is gated at runtime by
/// `runtime_status` (the `ggml` feature).
pub fn registry() -> Vec<CaptionModel> {
    vec![
        ggml_model(
            "parakeet-v3",
            "Parakeet V3 (0.6B)",
            "Parakeet",
            "handy-computer/parakeet-tdt-0.6b-v3-gguf",
            "parakeet-tdt-0.6b-v3-Q8_0.gguf",
            vec!["multi".into()],
            660_000_000,
            true,
        ),
        ggml_model(
            "parakeet-v2",
            "Parakeet V2 (0.6B, English)",
            "Parakeet",
            "handy-computer/parakeet-tdt-0.6b-v2-gguf",
            "parakeet-tdt-0.6b-v2-Q8_0.gguf",
            vec!["en".into()],
            660_000_000,
            false,
        ),
        ggml_model(
            "whisper-base",
            "Whisper Base",
            "Whisper",
            "handy-computer/whisper-base-gguf",
            "whisper-base-Q5_K_M.gguf",
            vec!["multi".into()],
            60_000_000,
            false,
        ),
        ggml_model(
            "whisper-small",
            "Whisper Small",
            "Whisper",
            "handy-computer/whisper-small-gguf",
            "whisper-small-Q5_K_M.gguf",
            vec!["multi".into()],
            190_000_000,
            false,
        ),
    ]
}

/// The full catalog this build offers: built-ins plus caption models
/// contributed by installed+enabled extensions. A pack model whose id collides
/// with a built-in is dropped (built-ins win), so a pack can't shadow a shipped
/// model.
pub fn all_models(app: &AppHandle) -> Vec<CaptionModel> {
    let mut models = registry();
    let builtin_ids: std::collections::HashSet<String> =
        models.iter().map(|m| m.id.clone()).collect();
    for m in super::packs::pack_models(app) {
        if !builtin_ids.contains(&m.id) {
            models.push(m);
        }
    }
    // User-configured remote endpoints. Their ids are namespaced (`remote:<id>`)
    // so they can't collide with built-ins or packs.
    models.extend(super::remote::remote_models(app));
    models
}

pub fn find(app: &AppHandle, id: &str) -> Option<CaptionModel> {
    all_models(app).into_iter().find(|m| m.id == id)
}

pub fn models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir unavailable: {e}"))?;
    Ok(base.join("models"))
}

pub fn model_dir(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    Ok(models_dir(app)?.join(id))
}

/// A model is installed when every declared file is present (and matches its
/// sha256 if one is known). A model with no files defined is never "installed".
pub fn is_installed(app: &AppHandle, model: &CaptionModel) -> Result<bool, String> {
    if model.files.is_empty() {
        return Ok(false);
    }
    let dir = model_dir(app, &model.id)?;
    for f in &model.files {
        let path = dir.join(&f.rel_path);
        if !path.exists() {
            return Ok(false);
        }
        if let Some(expected) = f.sha256.as_deref().filter(|s| !s.is_empty()) {
            match file_sha256(&path) {
                Ok(got) if got.eq_ignore_ascii_case(expected) => {}
                _ => return Ok(false),
            }
        }
    }
    Ok(true)
}

fn file_sha256(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Stream `url` into `dest` (via a sibling `.tmp`), hashing as we go and calling
/// `on_progress(downloaded, total)` per chunk. Verifies sha256 when known, then
/// atomically renames into place. `total` is 0 when the server omits a length.
pub async fn download_file(
    client: &reqwest::Client,
    url: &str,
    sha256: Option<&str>,
    dest: &Path,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("create dir: {e}"))?;
    }
    let tmp = dest.with_extension("tmp");
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("request: {e}"))?
        .error_for_status()
        .map_err(|e| format!("http: {e}"))?;
    let total = resp.content_length().unwrap_or(0);

    let mut hasher = Sha256::new();
    let mut file = fs::File::create(&tmp)
        .await
        .map_err(|e| format!("create tmp: {e}"))?;
    let mut downloaded = 0u64;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("stream: {e}"))?;
        hasher.update(&bytes);
        file.write_all(&bytes)
            .await
            .map_err(|e| format!("write: {e}"))?;
        downloaded += bytes.len() as u64;
        on_progress(downloaded, total);
    }
    file.flush().await.map_err(|e| format!("flush: {e}"))?;
    drop(file);

    if let Some(expected) = sha256.filter(|s| !s.is_empty()) {
        let got = hex::encode(hasher.finalize());
        if !got.eq_ignore_ascii_case(expected) {
            let _ = fs::remove_file(&tmp).await;
            return Err(format!("sha256 mismatch (expected {expected}, got {got})"));
        }
    } else {
        log::warn!(
            "caption model file {} downloaded without sha256 verification",
            url
        );
    }

    if dest.exists() {
        let _ = fs::remove_file(dest).await;
    }
    fs::rename(&tmp, dest)
        .await
        .map_err(|e| format!("rename: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_maps_to_its_runtime() {
        assert_eq!(Engine::Ggml.runtime(), Runtime::Ggml);
        assert_eq!(Engine::Remote.runtime(), Runtime::Remote);
    }

    #[test]
    fn ggml_availability_tracks_the_feature() {
        // The on-device engine needs the `ggml` feature (transcribe.cpp compiled
        // in); a `--no-default-features` build must report it unavailable so the
        // UI falls back to remote endpoints.
        let (available, reason) = runtime_status(Runtime::Ggml);
        assert_eq!(available, cfg!(feature = "ggml"));
        assert_eq!(reason.is_none(), cfg!(feature = "ggml"));
    }

    #[test]
    fn remote_runtime_is_never_globally_available() {
        // Remote availability is decided per-endpoint (key present), so the
        // global gate is always "not available" with a reason.
        let (available, reason) = runtime_status(Runtime::Remote);
        assert!(!available);
        assert!(reason.is_some());
    }

    #[test]
    fn registry_nominates_exactly_one_default() {
        assert_eq!(registry().iter().filter(|m| m.is_default).count(), 1);
    }

    #[test]
    fn every_builtin_is_a_single_gguf_ggml_model() {
        for m in registry() {
            assert!(matches!(m.engine, Engine::Ggml), "{} is not ggml", m.id);
            assert_eq!(m.files.len(), 1, "{} should be one GGUF file", m.id);
            assert!(
                m.files[0].rel_path.ends_with(".gguf"),
                "{} file is not a .gguf",
                m.id
            );
        }
    }
}
