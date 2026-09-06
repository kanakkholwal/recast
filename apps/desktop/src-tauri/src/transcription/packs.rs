//! Caption models contributed by installed extensions, merged into the built-in catalog.
//! `runtime` and `engine` are closed enums, so a pack selects an existing backend and can never introduce one.

use serde::Deserialize;
use serde_json::Value;
use tauri::AppHandle;

use crate::commands::extensions::{is_safe_ext_id, is_safe_filename};

use super::models::{CaptionModel, Engine, ModelFile, ModelSource, Runtime};

/// One weight file of a pack model. Unlike built-ins (whose sha256 is still
/// `None` pending a pinned revision), a pack file MUST pin its hash.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackFile {
    rel_path: String,
    url: String,
    sha256: String,
}

/// A `contributes.captionModels[]` entry. Deserialization alone enforces the
/// closed `runtime`/`engine` allowlists; [`to_caption_model`] adds the rest.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptionModelContribution {
    id: String,
    display_name: String,
    runtime: Runtime,
    engine: Engine,
    family: String,
    languages: Vec<String>,
    #[serde(default)]
    approx_size_bytes: Option<u64>,
    files: Vec<PackFile>,
    #[serde(default)]
    requires_gpu: bool,
    #[serde(default)]
    prefers_gpu: bool,
    #[serde(default)]
    min_ram_bytes: Option<u64>,
}

/// Validate a contribution and convert it into a catalog entry. Returns an
/// error string (logged + skipped by the caller) when a rule fails.
fn to_caption_model(c: CaptionModelContribution) -> Result<CaptionModel, String> {
    // The id doubles as the on-disk model dir name under `models/`.
    if !is_safe_ext_id(&c.id) {
        return Err(format!("unsafe model id '{}'", c.id));
    }
    // The engine must run on the declared runtime, or the availability gate and the transcribe arm disagree.
    if c.engine.runtime() != c.runtime {
        return Err(format!(
            "engine '{:?}' does not run on runtime '{:?}' (model '{}')",
            c.engine, c.runtime, c.id
        ));
    }
    if c.files.is_empty() {
        return Err(format!("model '{}' declares no files", c.id));
    }
    if c.languages.is_empty() {
        return Err(format!("model '{}' declares no languages", c.id));
    }
    let mut files = Vec::with_capacity(c.files.len());
    for f in c.files {
        if !is_safe_filename(&f.rel_path) {
            return Err(format!(
                "unsafe file path '{}' in model '{}'",
                f.rel_path, c.id
            ));
        }
        if f.sha256.trim().is_empty() {
            return Err(format!(
                "file '{}' in model '{}' must pin a sha256",
                f.rel_path, c.id
            ));
        }
        files.push(ModelFile {
            rel_path: f.rel_path,
            url: f.url,
            sha256: Some(f.sha256),
        });
    }
    Ok(CaptionModel {
        id: c.id,
        display_name: c.display_name,
        engine: c.engine,
        family: c.family,
        languages: c.languages,
        approx_size_bytes: c.approx_size_bytes,
        // Only the built-in catalog nominates a default; a pack can't seize it.
        is_default: false,
        files,
        requires_gpu: c.requires_gpu,
        prefers_gpu: c.prefers_gpu,
        min_ram_bytes: c.min_ram_bytes,
        source: ModelSource::Extension,
        remote: None,
        // A pack gets neutral defaults (no scores or badges) rather than a way to self-promote in the picker.
        capabilities: Default::default(),
        language_count: None,
        speed_score: None,
        accuracy_score: None,
        recommended: false,
    })
}

/// Parse the `captionModels` array out of one extension's `contributes` blob,
/// returning the valid models. Invalid entries are dropped with a warning
/// (a bad model in a pack must not break the whole catalog). Pure over its
/// input, so it's unit-testable without the filesystem.
pub(crate) fn models_from_contributes(pack_id: &str, contributes: &Value) -> Vec<CaptionModel> {
    let Some(arr) = contributes.get("captionModels").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in arr {
        match serde_json::from_value::<CaptionModelContribution>(entry.clone()) {
            Ok(c) => match to_caption_model(c) {
                Ok(m) => out.push(m),
                Err(e) => log::warn!("pack '{pack_id}': skipping caption model: {e}"),
            },
            Err(e) => log::warn!("pack '{pack_id}': invalid captionModel contribution: {e}"),
        }
    }
    out
}

/// Every valid caption model contributed by installed+enabled extensions. Deduplicated by id (first pack wins); collisions with built-ins are resolved later in `models::all_models`.
pub fn pack_models(app: &AppHandle) -> Vec<CaptionModel> {
    let mut out: Vec<CaptionModel> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for manifest in crate::commands::extensions::enabled_manifests(app) {
        for m in models_from_contributes(&manifest.id, &manifest.contributes) {
            if seen.insert(m.id.clone()) {
                out.push(m);
            } else {
                log::warn!(
                    "pack '{}': caption model '{}' shadows another pack's id; ignored",
                    manifest.id,
                    m.id
                );
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid_ggml() -> Value {
        json!({
            "id": "acme.parakeet-hi",
            "displayName": "Parakeet Hindi",
            "runtime": "ggml",
            "engine": "ggml",
            "family": "Parakeet",
            "languages": ["hi"],
            "files": [
                { "relPath": "parakeet-hi-Q8_0.gguf", "url": "https://x/model", "sha256": "aa" }
            ]
        })
    }

    fn one(contributes: Value) -> Vec<CaptionModel> {
        models_from_contributes("test-pack", &contributes)
    }

    #[test]
    fn accepts_a_valid_ggml_model() {
        let models = one(json!({ "captionModels": [valid_ggml()] }));
        assert_eq!(models.len(), 1);
        let m = &models[0];
        assert_eq!(m.id, "acme.parakeet-hi");
        assert_eq!(m.source, ModelSource::Extension);
        assert!(!m.is_default, "a pack must not seize the default slot");
        assert_eq!(m.files.len(), 1);
        assert_eq!(m.files[0].sha256.as_deref(), Some("aa"));
    }

    #[test]
    fn rejects_engine_runtime_mismatch() {
        let mut c = valid_ggml();
        c["runtime"] = json!("ggml");
        c["engine"] = json!("remote"); // remote runs on the remote runtime, not ggml
        assert!(one(json!({ "captionModels": [c] })).is_empty());
    }

    #[test]
    fn rejects_missing_sha256() {
        let mut c = valid_ggml();
        c["files"][0]["sha256"] = json!("");
        assert!(one(json!({ "captionModels": [c] })).is_empty());
    }

    #[test]
    fn rejects_unsafe_id_and_paths() {
        let mut bad_id = valid_ggml();
        bad_id["id"] = json!("../escape");
        assert!(one(json!({ "captionModels": [bad_id] })).is_empty());

        let mut bad_path = valid_ggml();
        bad_path["files"][0]["relPath"] = json!("../../etc/passwd");
        assert!(one(json!({ "captionModels": [bad_path] })).is_empty());
    }

    #[test]
    fn rejects_empty_files_or_languages() {
        let mut no_files = valid_ggml();
        no_files["files"] = json!([]);
        assert!(one(json!({ "captionModels": [no_files] })).is_empty());

        let mut no_langs = valid_ggml();
        no_langs["languages"] = json!([]);
        assert!(one(json!({ "captionModels": [no_langs] })).is_empty());
    }

    #[test]
    fn rejects_unknown_runtime_or_engine() {
        let mut bad_rt = valid_ggml();
        bad_rt["runtime"] = json!("tensorrt");
        assert!(one(json!({ "captionModels": [bad_rt] })).is_empty());

        let mut bad_eng = valid_ggml();
        bad_eng["engine"] = json!("moonshine");
        assert!(one(json!({ "captionModels": [bad_eng] })).is_empty());
    }

    #[test]
    fn keeps_valid_entries_and_drops_invalid_ones() {
        let mut bad = valid_ggml();
        bad["id"] = json!("acme.good-2");
        bad["files"] = json!([]); // invalid
        let mut good2 = valid_ggml();
        good2["id"] = json!("acme.good-2-ok");
        let models = one(json!({ "captionModels": [valid_ggml(), bad, good2] }));
        let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, vec!["acme.parakeet-hi", "acme.good-2-ok"]);
    }

    #[test]
    fn no_caption_models_key_is_empty() {
        assert!(one(json!({ "cursors": [] })).is_empty());
        assert!(one(json!({})).is_empty());
    }
}
