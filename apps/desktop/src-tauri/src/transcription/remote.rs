//! Remote transcription over an OpenAI-compatible `/audio/transcriptions` endpoint; plain HTTP, so it needs no model files or Cargo feature.
//! The API key lives in the OS keyring and is read only here, never over IPC, matching the Cloud and Drive tokens.

use std::io::Write;
use std::path::PathBuf;

use keyring::Entry;
use reqwest::multipart;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::commands::extensions::is_safe_ext_id;

use super::models::{CaptionModel, Engine, ModelSource};
use super::words::{RawSeg, RawWord};
use super::{Transcript, TranscriptSegment};

const KEYRING_SERVICE: &str = "com.kanakkholwal.recast";
/// Long clips over a slow endpoint can take a while; err on generous.
const REQUEST_TIMEOUT_SECS: u64 = 300;

/// A user-configured remote transcription endpoint. Non-secret: the API key is NOT here (it lives in the keyring), so this is safe to persist and to return over IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteEndpoint {
    /// Stable slug; doubles as the keyring entry suffix and the catalog id.
    pub id: String,
    pub display_name: String,
    /// Base URL up to (not including) `/audio/transcriptions`, e.g.
    /// `http://127.0.0.1:1234/v1`. Trailing slash stripped on save.
    pub base_url: String,
    /// The model name the endpoint expects, e.g. `whisper-large-v3`.
    pub model: String,
    /// BCP-47-ish language hints for the picker badge; `["multi"]` when unset.
    #[serde(default)]
    pub languages: Vec<String>,
}

// ── Config persistence (non-secret) ─────────────────────────────────────────

fn endpoints_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join("remote-asr.json"))
}

pub(crate) fn read_endpoints(app: &AppHandle) -> Vec<RemoteEndpoint> {
    let Some(path) = endpoints_path(app) else {
        return Vec::new();
    };
    crate::commands::system::read_json_manifest(&path)
}

fn write_endpoints(app: &AppHandle, endpoints: &[RemoteEndpoint]) -> Result<(), String> {
    let path = endpoints_path(app).ok_or("app_data_dir unavailable")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(endpoints).map_err(|e| format!("serialize: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    crate::commands::system::write_atomic(&tmp, &path, json.as_bytes()).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("write endpoints: {e}")
    })
}

// ── Validation ──────────────────────────────────────────────────────────────

/// Trim + validate a base URL to an absolute `http(s)` URL with a host, and
/// return the trailing-slash-stripped form. `None` for anything malformed.
pub(crate) fn normalize_base_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let parsed = reqwest::Url::parse(trimmed).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    if parsed.host_str().map(str::is_empty).unwrap_or(true) {
        return None;
    }
    Some(trimmed.to_string())
}

/// Validate a proposed endpoint and return the normalized form (base URL
/// canonicalized, languages defaulted). Used by the upsert command.
pub(crate) fn validate_endpoint(mut ep: RemoteEndpoint) -> Result<RemoteEndpoint, String> {
    if !is_safe_ext_id(&ep.id) {
        return Err(format!("invalid endpoint id '{}'", ep.id));
    }
    if ep.display_name.trim().is_empty() {
        return Err("endpoint needs a display name".into());
    }
    ep.base_url = normalize_base_url(&ep.base_url)
        .ok_or("base URL must be an absolute http(s) URL, e.g. http://127.0.0.1:1234/v1")?;
    if ep.model.trim().is_empty() {
        return Err("endpoint needs a model name".into());
    }
    if ep.languages.is_empty() {
        ep.languages = vec!["multi".into()];
    }
    Ok(ep)
}

// ── Config mutation ─────────────────────────────────────────────────────────

/// Insert or replace an endpoint (matched by id). Returns the stored (normalized)
/// endpoint.
pub(crate) fn upsert_endpoint(
    app: &AppHandle,
    endpoint: RemoteEndpoint,
) -> Result<RemoteEndpoint, String> {
    let endpoint = validate_endpoint(endpoint)?;
    let mut endpoints = read_endpoints(app);
    match endpoints.iter_mut().find(|e| e.id == endpoint.id) {
        Some(existing) => *existing = endpoint.clone(),
        None => endpoints.push(endpoint.clone()),
    }
    write_endpoints(app, &endpoints)?;
    Ok(endpoint)
}

/// Remove an endpoint and its stored key.
pub(crate) fn delete_endpoint(app: &AppHandle, id: &str) -> Result<(), String> {
    let mut endpoints = read_endpoints(app);
    let before = endpoints.len();
    endpoints.retain(|e| e.id != id);
    if endpoints.len() != before {
        write_endpoints(app, &endpoints)?;
    }
    // Best-effort key removal — a missing key is not an error.
    let _ = delete_key(id);
    Ok(())
}

// ── Keyring (API key) ───────────────────────────────────────────────────────

fn key_entry(id: &str) -> keyring::Result<Entry> {
    Entry::new(KEYRING_SERVICE, &format!("remote-asr-{id}"))
}

pub(crate) fn store_key(id: &str, key: &str) -> Result<(), String> {
    key_entry(id)
        .and_then(|e| e.set_password(key))
        .map_err(|e| format!("keyring write failed: {e}"))
}

pub(crate) fn read_key(id: &str) -> Option<String> {
    key_entry(id).ok().and_then(|e| e.get_password().ok())
}

pub(crate) fn delete_key(id: &str) -> Result<(), String> {
    let Ok(entry) = key_entry(id) else {
        return Ok(());
    };
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("keyring delete failed: {e}")),
    }
}

pub(crate) fn has_key(id: &str) -> bool {
    read_key(id).is_some()
}

// ── Catalog ─────────────────────────────────────────────────────────────────

/// Turn one endpoint into its catalog id (`remote:<id>`).
pub(crate) fn model_id(endpoint_id: &str) -> String {
    format!("remote:{endpoint_id}")
}

/// Every configured remote endpoint as a `CaptionModel`. No files; availability
/// (key present) is evaluated by the caller.
pub fn remote_models(app: &AppHandle) -> Vec<CaptionModel> {
    read_endpoints(app)
        .into_iter()
        .map(|ep| CaptionModel {
            id: model_id(&ep.id),
            display_name: ep.display_name.clone(),
            engine: Engine::Remote,
            family: "Remote".into(),
            languages: ep.languages.clone(),
            approx_size_bytes: None,
            is_default: false,
            files: Vec::new(),
            requires_gpu: false,
            prefers_gpu: false,
            min_ram_bytes: None,
            source: ModelSource::Remote,
            remote: Some(ep),
            // The server owns the real model, so we can't honestly score it or claim capabilities on its behalf.
            capabilities: Default::default(),
            language_count: None,
            speed_score: None,
            accuracy_score: None,
            recommended: false,
        })
        .collect()
}

// ── WAV encoding ────────────────────────────────────────────────────────────

/// Encode mono f32 PCM (`-1.0..=1.0`) as a 16-bit PCM WAV byte buffer. The
/// endpoint wants an audio *file*; this is the smallest lossless container.
pub(crate) fn pcm_f32_to_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let channels: u16 = 1;
    let bits: u16 = 16;
    let block_align = channels * bits / 8;
    let byte_rate = sample_rate * block_align as u32;
    let data_len = (samples.len() * 2) as u32;

    let mut buf = Vec::with_capacity(44 + data_len as usize);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(36 + data_len).to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    buf.extend_from_slice(&1u16.to_le_bytes()); // audio format = PCM
    buf.extend_from_slice(&channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&bits.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let v = (clamped * i16::MAX as f32) as i16;
        let _ = buf.write_all(&v.to_le_bytes());
    }
    buf
}

// ── Response mapping ────────────────────────────────────────────────────────

/// Seconds (as the OpenAI API reports) to the integer milliseconds
/// `build_segments` works in.
fn secs_to_ms(v: f64) -> i64 {
    (v * 1000.0).round() as i64
}

/// Map an OpenAI-compatible transcription response into caption segments via the
/// shared `build_segments`, so remote captions come out identical to on-device:
/// `words[]` (real word timing, requested via `timestamp_granularities[]`) is
/// preferred and grouped into display lines; otherwise `segments[]`; otherwise a
/// single block from `text`. Pure, so it's unit-tested.
pub(crate) fn response_to_segments(body: &Value, total_secs: f64) -> Vec<TranscriptSegment> {
    // OpenAI word rows key on `word` and segment rows on `text`; a server ignoring word granularity returns no `words[]`.
    let words: Vec<RawWord> = body
        .get("words")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|w| {
                    let text = w
                        .get("word")
                        .or_else(|| w.get("text"))
                        .and_then(Value::as_str)?
                        .trim()
                        .to_string();
                    if text.is_empty() {
                        return None;
                    }
                    let start = w.get("start").and_then(Value::as_f64).unwrap_or(0.0);
                    let end = w.get("end").and_then(Value::as_f64).unwrap_or(start);
                    Some(RawWord {
                        t0_ms: secs_to_ms(start),
                        t1_ms: secs_to_ms(end),
                        text,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let segs: Vec<RawSeg> = body
        .get("segments")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|s| {
                    let text = s.get("text").and_then(Value::as_str)?.trim().to_string();
                    if text.is_empty() {
                        return None;
                    }
                    let start = s.get("start").and_then(Value::as_f64).unwrap_or(0.0);
                    let end = s.get("end").and_then(Value::as_f64).unwrap_or(start);
                    Some(RawSeg {
                        t0_ms: secs_to_ms(start),
                        t1_ms: secs_to_ms(end),
                        text,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let text = body.get("text").and_then(Value::as_str).unwrap_or("");
    super::words::build_segments(text, total_secs, &segs, &words)
}

// ── Transcription ───────────────────────────────────────────────────────────

/// POST 16 kHz mono audio to the endpoint and map the reply into a `Transcript`.
/// `api_key` is read from the keyring by the caller; it never leaves Rust.
pub async fn transcribe_remote(
    endpoint: &RemoteEndpoint,
    api_key: &str,
    samples: &[f32],
    language: Option<&str>,
) -> Result<Transcript, String> {
    let total_secs = samples.len() as f64 / 16_000.0;
    let wav = pcm_f32_to_wav(samples, 16_000);

    let file_part = multipart::Part::bytes(wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| format!("build audio part: {e}"))?;
    let mut form = multipart::Form::new()
        .part("file", file_part)
        .text("model", endpoint.model.clone())
        .text("response_format", "verbose_json")
        // Ask for word timestamps (OpenAI wants a repeated field) plus segment as a fallback; a server without support ignores both.
        .text("timestamp_granularities[]", "word")
        .text("timestamp_granularities[]", "segment");
    // Omit for auto-detect; a bogus/empty value would make some servers 400.
    if let Some(lang) = language.filter(|l| !l.is_empty() && *l != "auto") {
        form = form.text("language", lang.to_string());
    }

    let client = reqwest::Client::builder()
        .user_agent(format!("Recast/{} (remote-asr)", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let url = format!("{}/audio/transcriptions", endpoint.base_url);
    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        // Truncate: an HTML error page would otherwise flood the toast.
        let snippet: String = body.chars().take(300).collect();
        return Err(format!("endpoint returned {status}: {snippet}"));
    }

    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("parse response: {e}"))?;

    let language = body
        .get("language")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| language.map(str::to_string));

    Ok(Transcript {
        engine: "remote".into(),
        model_id: model_id(&endpoint.id),
        language,
        segments: response_to_segments(&body, total_secs),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn endpoint() -> RemoteEndpoint {
        RemoteEndpoint {
            id: "lmstudio-local".into(),
            display_name: "LM Studio".into(),
            base_url: "http://127.0.0.1:1234/v1/".into(),
            model: "whisper-large-v3".into(),
            languages: vec![],
        }
    }

    #[test]
    fn normalize_base_url_accepts_http_and_strips_slash() {
        assert_eq!(
            normalize_base_url("http://127.0.0.1:1234/v1/"),
            Some("http://127.0.0.1:1234/v1".into())
        );
        assert_eq!(
            normalize_base_url(" https://api.example.com "),
            Some("https://api.example.com".into())
        );
    }

    #[test]
    fn normalize_base_url_rejects_bad_input() {
        assert_eq!(normalize_base_url(""), None);
        assert_eq!(normalize_base_url("ftp://x.com"), None);
        assert_eq!(normalize_base_url("not a url"), None);
    }

    #[test]
    fn validate_defaults_languages_and_normalizes_url() {
        let ep = validate_endpoint(endpoint()).expect("valid");
        assert_eq!(ep.base_url, "http://127.0.0.1:1234/v1");
        assert_eq!(ep.languages, vec!["multi".to_string()]);
    }

    #[test]
    fn validate_rejects_bad_id_url_and_empty_fields() {
        let mut bad_id = endpoint();
        bad_id.id = "../x".into();
        assert!(validate_endpoint(bad_id).is_err());

        let mut bad_url = endpoint();
        bad_url.base_url = "file:///etc".into();
        assert!(validate_endpoint(bad_url).is_err());

        let mut no_model = endpoint();
        no_model.model = "  ".into();
        assert!(validate_endpoint(no_model).is_err());
    }

    #[test]
    fn model_id_is_namespaced() {
        assert_eq!(model_id("foo"), "remote:foo");
    }

    #[test]
    fn wav_header_is_well_formed() {
        let wav = pcm_f32_to_wav(&[0.0, 1.0, -1.0, 0.5], 16_000);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[36..40], b"data");
        // 44-byte header + 2 bytes per sample.
        assert_eq!(wav.len(), 44 + 4 * 2);
        // Sample rate encoded at offset 24.
        assert_eq!(
            u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]),
            16_000
        );
    }

    #[test]
    fn response_prefers_words_over_coarse_segments() {
        // Word timestamps win over a single coarse segment, so remote captions render word-by-word like on-device.
        let body = json!({
            "text": "hello world. foo",
            "segments": [ { "start": 0.0, "end": 2.0, "text": "hello world. foo" } ],
            "words": [
                { "word": "hello", "start": 0.0, "end": 0.3 },
                { "word": "world.", "start": 0.3, "end": 0.6 },
                { "word": "foo", "start": 0.7, "end": 1.0 }
            ]
        });
        let segs = response_to_segments(&body, 2.0);
        assert!(segs.len() >= 2, "a sentence break must split the line");
        assert_eq!(segs[0].text, "hello world.");
        assert_eq!(segs[1].text, "foo");
        // Real timing preserved (seconds -> secs), not synthesized.
        assert!((segs[0].words[1].start - 0.3).abs() < 1e-9);
    }

    #[test]
    fn response_falls_back_to_segments_without_words() {
        // Server ignored the word-granularity request: only segments came back.
        let body = json!({
            "text": "hello world foo",
            "segments": [
                { "start": 0.0, "end": 1.0, "text": "hello world" },
                { "start": 1.0, "end": 2.0, "text": "foo" }
            ]
        });
        let segs = response_to_segments(&body, 2.0);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].text, "hello world");
        // Word timing synthesized for animation.
        assert_eq!(segs[0].words.len(), 2);
        assert_eq!(segs[1].text, "foo");
    }

    #[test]
    fn response_falls_back_to_whole_clip_text() {
        let body = json!({ "text": "just text" });
        let segs = response_to_segments(&body, 3.0);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text, "just text");
        assert!((segs[0].end - 3.0).abs() < 1e-9);
        assert_eq!(segs[0].words.len(), 2);
    }

    #[test]
    fn response_empty_when_no_text_or_segments() {
        assert!(response_to_segments(&json!({}), 1.0).is_empty());
        assert!(response_to_segments(&json!({ "text": "" }), 1.0).is_empty());
    }
}
