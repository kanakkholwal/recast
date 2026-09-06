//! Caption model registry and verified download from HuggingFace into `app_data_dir/models/<id>/`.
//! Every built-in is sha256-pinned and a test keeps it so: an unpinned GGUF is arbitrary bytes handed to a native parser.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// How a model runs. There are exactly two engines: `ggml` on-device and `remote` against an OpenAI-compatible endpoint.
/// The GGUF file decides the architecture for ggml, so Parakeet or Whisper is display-only metadata rather than a separate code path.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    /// On-device, transcribe.cpp over ggml. Runs any supported GGUF file.
    Ggml,
    /// Not a local architecture: transcription runs on a remote
    /// OpenAI-compatible endpoint. The server owns the real model.
    Remote,
}

/// The inference backend a model runs on, and the axis the UI gates availability on: a `ggml` build ships the on-device engine, a `--no-default-features` one does not.
/// Kept as its own enum rather than folded into `Engine`, so the UI's `runtime` and `runtimeAvailable` contract stays stable.
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

/// Whether a runtime can actually run in this build, plus a user-facing reason when it can't. Composed with the device-capability check (`evaluate`) to decide if a model is offerable.
pub fn runtime_status(runtime: Runtime) -> (bool, Option<String>) {
    match runtime {
        // The on-device engine needs the `ggml` feature, so a no-default-features build reports it unavailable.
        #[cfg(feature = "ggml")]
        Runtime::Ggml => (true, None),
        #[cfg(not(feature = "ggml"))]
        Runtime::Ggml => (
            false,
            Some("On-device transcription isn't available in this build.".into()),
        ),
        // Remote availability is per-endpoint (is a key stored?), so the global gate is always unavailable with guidance.
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
    // - catalog metadata (picker presentation only; never gates execution) -
    /// What the model can do beyond plain transcription.
    #[serde(default)]
    pub capabilities: ModelCapabilities,
    /// How many languages the model covers. `languages` carries `["multi"]` for multilingual models rather than 99 entries, so the count is stored separately instead of derived from it.
    #[serde(default)]
    pub language_count: Option<u32>,
    /// Relative speed / accuracy, 0-100, for the picker's comparison bars.
    /// Editorial values from the upstream catalog — useful for ranking models
    /// against each other, not as absolute benchmarks.
    #[serde(default)]
    pub speed_score: Option<u8>,
    #[serde(default)]
    pub accuracy_score: Option<u8>,
    /// Surfaced with a "Recommended" tag in the picker.
    #[serde(default)]
    pub recommended: bool,
}

/// Model abilities beyond plain same-language transcription. `streaming` / `translate` / `lang_detect` are presentation only. `timestamps` is NOT: see `TimestampGranularity`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCapabilities {
    /// Emits partial results as audio arrives (vs. one result at the end).
    #[serde(default)]
    pub streaming: bool,
    /// Can transcribe speech into a different language.
    #[serde(default)]
    pub translate: bool,
    /// Detects the spoken language rather than needing it declared.
    #[serde(default)]
    pub lang_detect: bool,
    /// How precisely the model locates its text in time.
    #[serde(default)]
    pub timestamps: TimestampGranularity,
}

/// How precisely a model reports WHEN each piece of text was said: a hard requirement, since untimed captions cannot be placed, clipped at a cut, or highlighted.
/// A `None` model returns bare text that `words.rs` spreads evenly, drifting further out of sync the longer you talk. 34 of the upstream catalog's 65 are `None`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimestampGranularity {
    /// No timing at all — unusable for captions.
    #[default]
    None,
    /// Per phrase/sentence. Enough for captions; per-word highlight is
    /// synthesized within each segment.
    Segment,
    /// Per token (sub-word). Word timings are derived by grouping.
    Token,
    /// Per word, directly.
    Word,
}

impl TimestampGranularity {
    /// Whether this model can drive captions at all. Consumed by the registry
    /// guard test, which enforces it across every built-in.
    #[allow(dead_code)]
    pub fn usable_for_captions(self) -> bool {
        !matches!(self, TimestampGranularity::None)
    }
}

fn source_builtin() -> ModelSource {
    ModelSource::Builtin
}

/// Catalog-metadata setters, chained onto `ggml_model` at the registry entry so
/// the presentation data reads next to the model it describes instead of
/// stretching the constructor to a dozen positional arguments.
impl CaptionModel {
    /// Relative speed / accuracy (0-100) for the picker's comparison bars.
    fn scored(mut self, speed: u8, accuracy: u8) -> Self {
        self.speed_score = Some(speed);
        self.accuracy_score = Some(accuracy);
        self
    }

    /// Number of languages covered (see `language_count`).
    fn langs(mut self, count: u32) -> Self {
        self.language_count = Some(count);
        self
    }

    fn caps(
        mut self,
        streaming: bool,
        translate: bool,
        lang_detect: bool,
        timestamps: TimestampGranularity,
    ) -> Self {
        self.capabilities = ModelCapabilities {
            streaming,
            translate,
            lang_detect,
            timestamps,
        };
        self
    }

    fn recommend(mut self) -> Self {
        self.recommended = true;
        self
    }
}

/// A built-in ggml model: one GGUF from a `handy-computer` repo, with `family` only the picker's display group since the file itself picks the architecture.
/// `sha256` is exact-byte and pinned, rejecting a corrupted or upstream-replaced download at the `is_installed` gate; compute it with `tools/dev/pin-model-sha256.ps1`.
#[expect(
    clippy::too_many_arguments,
    reason = "a registry row: every column is independent data"
)]
fn ggml_model(
    id: &str,
    name: &str,
    family: &str,
    hf_repo: &str,
    file: &str,
    languages: Vec<String>,
    size: u64,
    is_default: bool,
    sha256: Option<&str>,
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
            sha256: sha256.map(str::to_string),
        }],
        // ggml runs on CPU (tinyBLAS); GPU is opt-in and only speeds it up.
        requires_gpu: false,
        prefers_gpu: false,
        min_ram_bytes: Some(2_000_000_000),
        source: ModelSource::Builtin,
        remote: None,
        capabilities: ModelCapabilities::default(),
        language_count: None,
        speed_score: None,
        accuracy_score: None,
        recommended: false,
    }
}

/// The built-in catalog of single-file GGUF models run by the ggml engine, with multilingual word-timestamped Parakeet V3 as the default and Whisper as an alternative.
/// All entries are plain data and compile in every build; whether they can RUN is gated at runtime by `runtime_status`.
pub fn registry() -> Vec<CaptionModel> {
    vec![
        ggml_model(
            "parakeet-v3",
            "Parakeet V3 (0.6B)",
            "Parakeet",
            "handy-computer/parakeet-tdt-0.6b-v3-gguf",
            "parakeet-tdt-0.6b-v3-Q8_0.gguf",
            vec!["multi".into()],
            739_508_576,
            true,
            Some("5859f77944efcd8eafa23a6350731960b2b55b2203df51f319665c807d802cc7"),
        )
        .scored(79, 88)
        .langs(25)
        .caps(false, false, true, TimestampGranularity::Token),
        ggml_model(
            "parakeet-v2",
            "Parakeet V2 (0.6B, English)",
            "Parakeet",
            "handy-computer/parakeet-tdt-0.6b-v2-gguf",
            "parakeet-tdt-0.6b-v2-Q8_0.gguf",
            vec!["en".into()],
            729_574_912,
            false,
            Some("f0d0e99cebb6d3b83f1f7069b82b5d3c2e39a54545b0da039cb4bafd9c4e5caa"),
        )
        .scored(85, 89)
        .langs(1)
        .caps(false, false, false, TimestampGranularity::Token),
        // transcribe.cpp runs Nemotron under its `parakeet` architecture (same encoder family), so no engine work.
        ggml_model(
            "nemotron-streaming-3.5",
            "Nemotron Streaming 3.5",
            "Nemotron",
            "handy-computer/nemotron-3.5-asr-streaming-0.6b-gguf",
            "nemotron-3.5-asr-streaming-0.6b-Q8_0.gguf",
            vec!["multi".into()],
            751_094_240,
            false,
            Some("b94545b313b3223fda7b2857a52681da813935c2127643d1e9ff0c23d988089c"),
        )
        .scored(84, 82)
        .langs(28)
        .caps(true, false, true, TimestampGranularity::Token)
        .recommend(),
        // Highest accuracy in the catalog, and the heaviest. Q5_K_M (not Q8_0)
        ggml_model(
            "whisper-base",
            "Whisper Base",
            "Whisper",
            "handy-computer/whisper-base-gguf",
            "whisper-base-Q5_K_M.gguf",
            vec!["multi".into()],
            60_000_000,
            false,
            // SHA-256 of the GGUF above, pinned 2026-07-15; `download_file` detects mismatches, so re-pin when the URL changes.
            Some("8E0FEB7BC35780353CF31821018E601BB7B7CFF6C9A0E17ADA5A5DB23F4DB867"),
        )
        .scored(99, 71)
        .langs(99)
        .caps(false, true, true, TimestampGranularity::Segment),
        ggml_model(
            "whisper-small",
            "Whisper Small",
            "Whisper",
            "handy-computer/whisper-small-gguf",
            "whisper-small-Q5_K_M.gguf",
            vec!["multi".into()],
            193_749_056,
            false,
            Some("326cd00c3e7217c751667c7c1600eaf7e0de174e186ca2c16b4bf590251c3c3b"),
        )
        .scored(78, 80)
        .langs(99)
        .caps(false, true, true, TimestampGranularity::Segment),
        // Broadest language coverage (99), at the cost of speed.
        ggml_model(
            "whisper-medium",
            "Whisper Medium",
            "Whisper",
            "handy-computer/whisper-medium-gguf",
            "whisper-medium-Q8_0.gguf",
            vec!["multi".into()],
            831_538_144,
            false,
            Some("09e6a65e7de377aa5b10bae24608bc6f8ca2ed04b3993ef10d4a02bcd9a82adf"),
        )
        .scored(42, 84)
        .langs(99)
        .caps(false, true, true, TimestampGranularity::Segment)
        .recommend(),
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
    // Remote endpoint ids are namespaced `remote:<id>` so they can't collide with built-ins or packs.
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
    Ok(model_dir_at(&models_dir(app)?, id))
}

/// Path-keyed sibling of [`model_dir`]. Lets non-Tauri callers (CLI, smoke
/// tests, automation) reach a model's on-disk directory without an
/// `AppHandle`. The Tauri-cmd path delegates through here so there's one
/// source of truth for the join math.
pub fn model_dir_at(models_root: &Path, id: &str) -> PathBuf {
    models_root.join(id)
}

/// A model is installed when every declared file is present (and matches its
/// sha256 if one is known). A model with no files defined is never "installed".
pub fn is_installed(app: &AppHandle, model: &CaptionModel) -> Result<bool, String> {
    let dir = model_dir(app, &model.id)?;
    is_installed_at(&dir, model)
}

/// Path-keyed sibling of [`is_installed`]. Same checks (file exists + sha256
/// match when one is pinned) against a caller-supplied model dir. Used by the
/// CLI `transcribe` verb and the smoke test; `is_installed(app, ...)` delegates
/// here so the criteria live in one place.
pub fn is_installed_at(model_dir: &Path, model: &CaptionModel) -> Result<bool, String> {
    if model.files.is_empty() {
        return Ok(false);
    }
    for f in &model.files {
        let path = model_dir.join(&f.rel_path);
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
        log::warn!("caption model file {url} downloaded without sha256 verification");
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
        // The on-device engine needs the `ggml` feature, so a no-default-features build must report it unavailable.
        let (available, reason) = runtime_status(Runtime::Ggml);
        assert_eq!(available, cfg!(feature = "ggml"));
        assert_eq!(reason.is_none(), cfg!(feature = "ggml"));
    }

    #[test]
    fn remote_runtime_is_never_globally_available() {
        // Remote availability is per-endpoint, so the global gate is always unavailable with a reason.
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

    /// An unpinned entry downloads over the network and goes straight into a
    /// native GGUF parser with nothing checking the bytes.
    #[test]
    fn every_builtin_model_is_hash_pinned() {
        for m in registry() {
            for f in &m.files {
                let pinned = f.sha256.as_deref().unwrap_or_default();
                assert_eq!(pinned.len(), 64, "{} has no sha256 pin", m.id);
                assert!(
                    pinned.chars().all(|c| c.is_ascii_hexdigit()),
                    "{} sha256 is not hex",
                    m.id
                );
            }
        }
    }

    /// The URL is built from the repo + filename, so a typo in either yields a 404 only at download time — on the user's machine, after they clicked. Assert the shape here instead.
    #[test]
    fn every_builtin_url_points_at_its_own_gguf_in_the_handy_org() {
        for m in registry() {
            let f = &m.files[0];
            assert!(
                f.url.starts_with("https://huggingface.co/handy-computer/"),
                "{}: unexpected host/org in {}",
                m.id,
                f.url
            );
            assert!(
                f.url.ends_with(&format!("/resolve/main/{}", f.rel_path)),
                "{}: url {} does not resolve its own file {}",
                m.id,
                f.url,
                f.rel_path
            );
        }
    }

    /// Presentation metadata is what the picker ranks and badges models by, so a
    /// missing score silently renders an empty bar rather than failing loudly.
    #[test]
    fn every_builtin_carries_complete_picker_metadata() {
        for m in registry() {
            assert!(m.speed_score.is_some(), "{} has no speed score", m.id);
            assert!(m.accuracy_score.is_some(), "{} has no accuracy score", m.id);
            assert!(m.language_count.is_some(), "{} has no language count", m.id);
            assert!(
                m.approx_size_bytes.is_some_and(|b| b > 0),
                "{} has no size",
                m.id
            );
            for (label, score) in [("speed", m.speed_score), ("accuracy", m.accuracy_score)] {
                let score = score.unwrap();
                assert!(
                    score <= 100,
                    "{}: {label} score {score} is out of 0-100",
                    m.id
                );
            }
        }
    }

    /// Captions are a timeline feature: without timing there is nothing to place, clip at a cut, or highlight.
    /// Canary 180M Flash and Cohere Transcribe both shipped and were pulled for emitting no timestamps. 34 of the catalog's 65 are `none`, so check rather than assume.
    #[test]
    fn every_builtin_can_actually_time_its_captions() {
        for m in registry() {
            assert!(
                m.capabilities.timestamps.usable_for_captions(),
                "{} reports no timestamps — it cannot drive captions, however good its \
                 transcription is. Verify `timestamps` in the upstream catalog before adding \
                 a model.",
                m.id
            );
        }
    }
}
