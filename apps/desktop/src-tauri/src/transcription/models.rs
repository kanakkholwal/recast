//! Caption model registry + verified, resumable-friendly download.
//!
//! Models are fetched **directly from HuggingFace** (decided 2026-06-29) into
//! `app_data_dir/models/<id>/`, sha256-verified when a hash is known. The
//! download/verify path mirrors `commands/assets.rs` (streamed `.tmp` + atomic
//! rename) but emits per-byte progress so the UI can show a real bar.
//!
//! NOTE ON DATA: the Whisper entries use the canonical `ggerganov/whisper.cpp`
//! GGML files (stable URLs). Their `sha256` is left `None` for now (skip-verify
//! with a warning) — fill them in when locking exact revisions. The **Parakeet
//! V3** entry has no `files` yet: the exact ONNX file set `transcribe-rs`
//! expects must be confirmed against its loader before we can pin URLs/hashes.
//! `download`/`transcribe` guard against the empty file list.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tokio::fs;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Parakeet,
    Canary,
    GigaAM,
    Cohere,
    Whisper,
    /// Not a local architecture: transcription runs on a remote
    /// OpenAI-compatible endpoint. The server owns the real model.
    Remote,
}

/// The inference backend a model runs on. Independent of `Engine` (the model
/// architecture): several architectures share one runtime. This is the axis the
/// UI gates availability on, since a build may ship one runtime and not another.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Runtime {
    /// transcribe-rs over `ort` (ONNX). Shipped today.
    Onnx,
    /// whisper.cpp. Not built yet (needs the `whisper-cpp` LLVM+CMake toolchain);
    /// models may install but can't run until then.
    WhisperCpp,
    /// OpenAI-compatible `/audio/transcriptions` endpoint (on-device, self-hosted,
    /// or third-party). No local files; config + a keyring-held key.
    Remote,
}

impl Engine {
    /// The runtime this architecture runs on. Keeps the two axes in sync from one
    /// place, so a new `Engine` arm can't forget to declare its backend.
    pub fn runtime(self) -> Runtime {
        match self {
            Engine::Parakeet | Engine::Canary | Engine::GigaAM | Engine::Cohere => Runtime::Onnx,
            Engine::Whisper => Runtime::WhisperCpp,
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
        // ONNX needs the `captions` feature (the `ort` runtime). Absent on the
        // Intel-Mac build, where on-device models must report as unavailable.
        #[cfg(feature = "captions")]
        Runtime::Onnx => (true, None),
        #[cfg(not(feature = "captions"))]
        Runtime::Onnx => (
            false,
            Some("On-device transcription isn't available in this build.".into()),
        ),
        // Available only in a build compiled with the `whisper` feature (which
        // pulls the LLVM/CMake toolchain). Default builds report it as pending.
        #[cfg(feature = "whisper")]
        Runtime::WhisperCpp => (true, None),
        #[cfg(not(feature = "whisper"))]
        Runtime::WhisperCpp => (
            false,
            Some("The Whisper runtime arrives in a later build.".into()),
        ),
        // No remote endpoints are configured yet (added in a later phase); until
        // then a remote model is present in the catalog but not runnable.
        Runtime::Remote => (
            false,
            Some("Configure a remote transcription endpoint to use this model.".into()),
        ),
    }
}

/// One file that makes up a model. Whisper is a single `.bin`; Parakeet is a
/// directory of ONNX files (hence `rel_path`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFile {
    /// Path under the model dir, e.g. `ggml-small.bin` or `encoder.onnx`.
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
    /// `None` for every local (onnx/whisperCpp) model.
    #[serde(default)]
    pub remote: Option<super::remote::RemoteEndpoint>,
}

fn source_builtin() -> ModelSource {
    ModelSource::Builtin
}

/// The int8 ONNX file set `transcribe-rs`'s `ParakeetModel::load(dir, Int8)`
/// expects in the model directory.
const PARAKEET_FILES: [&str; 4] = [
    "encoder-model.int8.onnx",
    "decoder_joint-model.int8.onnx",
    "nemo128.onnx",
    "vocab.txt",
];

/// Parakeet (NVIDIA, ONNX via `transcribe-rs`, CPU-optimized). Downloaded from
/// the `istupakov/parakeet-tdt-0.6b-*-onnx` HuggingFace repos.
fn parakeet(
    id: &str,
    name: &str,
    hf_repo: &str,
    multilingual: bool,
    is_default: bool,
) -> CaptionModel {
    let base = format!("https://huggingface.co/{hf_repo}/resolve/main");
    let files = PARAKEET_FILES
        .iter()
        .map(|f| ModelFile {
            rel_path: (*f).into(),
            url: format!("{base}/{f}"),
            sha256: None, // TODO: pin once we lock a revision
        })
        .collect();
    CaptionModel {
        id: id.into(),
        display_name: name.into(),
        engine: Engine::Parakeet,
        family: "Parakeet".into(),
        languages: vec![if multilingual { "multi" } else { "en" }.into()],
        approx_size_bytes: Some(660_000_000),
        is_default,
        files,
        requires_gpu: false, // Parakeet is CPU-optimized
        prefers_gpu: false,
        min_ram_bytes: Some(2_000_000_000),
        source: ModelSource::Builtin,
        remote: None,
    }
}

// File sets per engine (int8), matching what each transcribe-rs ONNX loader
// resolves. Repos per transcribe-rs README's model table.
const CANARY_FILES: [&str; 4] = [
    "encoder-model.int8.onnx",
    "decoder-model.int8.onnx",
    "nemo128.onnx",
    "vocab.txt",
];
const GIGAAM_FILES: [&str; 2] = ["model.int8.onnx", "vocab.txt"];
const COHERE_FILES: [&str; 3] = [
    "cohere-encoder.int4.onnx",
    "cohere-decoder.int4.onnx",
    "tokens.txt",
];
/// Generic ONNX model entry (Canary / GigaAM / …) downloaded from a HuggingFace
/// repo. `files` are stored flat in the model dir, where transcribe-rs loads them.
#[allow(clippy::too_many_arguments)]
fn onnx(
    id: &str,
    name: &str,
    family: &str,
    engine: Engine,
    hf_repo: &str,
    files: &[&str],
    languages: Vec<String>,
    size: u64,
) -> CaptionModel {
    let base = format!("https://huggingface.co/{hf_repo}/resolve/main");
    let files = files
        .iter()
        .map(|f| ModelFile {
            rel_path: (*f).into(),
            url: format!("{base}/{f}"),
            sha256: None, // TODO: pin once we lock a revision
        })
        .collect();
    CaptionModel {
        id: id.into(),
        display_name: name.into(),
        engine,
        family: family.into(),
        languages,
        approx_size_bytes: Some(size),
        is_default: false,
        files,
        requires_gpu: false,
        prefers_gpu: false,
        min_ram_bytes: Some(2_000_000_000),
        source: ModelSource::Builtin,
        remote: None,
    }
}

/// The model catalog. Currently the Parakeet ONNX models (run via the
/// `transcribe-rs` `onnx` engine — no extra toolchain). Parakeet V3 is the
/// default. The broader Handy-style ONNX catalog (Moonshine / Canary /
/// SenseVoice / GigaAM / Cohere) is the next addition — each needs its own HF
/// repo + file set wired here and an engine arm in `engine.rs`. Whisper models
/// wait on the `whisper-cpp` build (LLVM + CMake).
pub fn registry() -> Vec<CaptionModel> {
    #[allow(unused_mut)]
    let mut models = vec![
        parakeet(
            "parakeet-v3",
            "Parakeet V3 (0.6B)",
            "istupakov/parakeet-tdt-0.6b-v3-onnx",
            true,
            true,
        ),
        // TODO: confirm the v2 (English) repo id / file names before shipping.
        parakeet(
            "parakeet-v2",
            "Parakeet V2 (0.6B, English)",
            "istupakov/parakeet-tdt-0.6b-v2-onnx",
            false,
            false,
        ),
        // - Canary (multilingual + translation) -
        onnx(
            "canary-180m-flash",
            "Canary 180M Flash",
            "Canary",
            Engine::Canary,
            "istupakov/canary-180m-flash-onnx",
            &CANARY_FILES,
            vec!["multi".into()],
            146_000_000,
        ),
        onnx(
            "canary-1b-v2",
            "Canary 1B v2",
            "Canary",
            Engine::Canary,
            "istupakov/canary-1b-v2-onnx",
            &CANARY_FILES,
            vec!["multi".into()],
            691_000_000,
        ),
        // - GigaAM (Russian) -
        onnx(
            "gigaam-v3",
            "GigaAM v3 (Russian)",
            "GigaAM",
            Engine::GigaAM,
            "istupakov/gigaam-v3-onnx",
            &GIGAAM_FILES,
            vec!["ru".into()],
            151_000_000,
        ),
        // - Cohere (large, multilingual) -
        onnx(
            "cohere",
            "Cohere",
            "Cohere",
            Engine::Cohere,
            "cstr/cohere-transcribe-onnx-int4",
            &COHERE_FILES,
            vec!["multi".into()],
            1_700_000_000,
        ),
    ];
    // Built-in Whisper (whisper.cpp) models only exist in a `whisper`-feature
    // build; otherwise they'd be perpetually un-runnable downloads in the picker.
    #[cfg(feature = "whisper")]
    models.extend(whisper_models());
    models
}

/// GGML Whisper model file entry. whisper.cpp loads a single `.bin`; the file
/// name doubles as the on-disk name under the model dir. Files come from the
/// canonical `ggerganov/whisper.cpp` HuggingFace repo.
#[cfg(feature = "whisper")]
fn whisper(id: &str, name: &str, ggml_file: &str, multilingual: bool, size: u64) -> CaptionModel {
    let url = format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{ggml_file}");
    CaptionModel {
        id: id.into(),
        display_name: name.into(),
        engine: Engine::Whisper,
        family: "Whisper".into(),
        languages: vec![if multilingual { "multi" } else { "en" }.into()],
        approx_size_bytes: Some(size),
        is_default: false,
        files: vec![ModelFile {
            rel_path: ggml_file.into(),
            url,
            sha256: None, // TODO: pin once we lock a revision
        }],
        requires_gpu: false, // whisper.cpp runs on CPU (GPU accel is opt-in)
        prefers_gpu: false,
        min_ram_bytes: Some(2_000_000_000),
        source: ModelSource::Builtin,
        remote: None,
    }
}

#[cfg(feature = "whisper")]
fn whisper_models() -> Vec<CaptionModel> {
    vec![
        whisper(
            "whisper-base",
            "Whisper Base",
            "ggml-base.bin",
            true,
            148_000_000,
        ),
        whisper(
            "whisper-small",
            "Whisper Small",
            "ggml-small.bin",
            true,
            488_000_000,
        ),
        whisper(
            "whisper-medium",
            "Whisper Medium",
            "ggml-medium.bin",
            true,
            1_530_000_000,
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
        assert_eq!(Engine::Parakeet.runtime(), Runtime::Onnx);
        assert_eq!(Engine::Canary.runtime(), Runtime::Onnx);
        assert_eq!(Engine::GigaAM.runtime(), Runtime::Onnx);
        assert_eq!(Engine::Cohere.runtime(), Runtime::Onnx);
        assert_eq!(Engine::Whisper.runtime(), Runtime::WhisperCpp);
        assert_eq!(Engine::Remote.runtime(), Runtime::Remote);
    }

    #[test]
    fn onnx_availability_tracks_the_captions_feature() {
        // ONNX (Parakeet/Canary/…) needs the `ort` runtime the `captions` feature
        // pulls; the Intel-Mac build (captions off) must report it unavailable.
        let (available, reason) = runtime_status(Runtime::Onnx);
        assert_eq!(available, cfg!(feature = "captions"));
        assert_eq!(reason.is_none(), cfg!(feature = "captions"));
    }

    #[test]
    fn whisper_availability_tracks_the_feature() {
        let (available, reason) = runtime_status(Runtime::WhisperCpp);
        // The `whisper` build activates the runtime; every other build reports it
        // as pending with a reason.
        assert_eq!(available, cfg!(feature = "whisper"));
        assert_eq!(reason.is_none(), cfg!(feature = "whisper"));
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
    fn whisper_models_present_only_in_whisper_build() {
        let has_whisper = registry()
            .iter()
            .any(|m| matches!(m.engine, Engine::Whisper));
        assert_eq!(has_whisper, cfg!(feature = "whisper"));
    }
}
