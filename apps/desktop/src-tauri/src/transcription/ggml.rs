//! transcribe.cpp (ggml / GGUF) on-device engine.
//!
//! One engine for every on-device model family (Parakeet, Whisper, Canary, ...):
//! the loaded GGUF file decides the architecture. Compiled from source (its -sys
//! crate runs CMake; ggml is vendored, no submodules), so unlike `ort` it builds
//! on every OS including Intel Mac. Input PCM is 16 kHz mono f32 in [-1, 1] —
//! exactly what `audio::extract_pcm_f32` produces.
//!
//! transcribe.cpp returns real per-segment and per-word timing (ms). We map those
//! through `words::build_segments`, which handles every shape (segments+words,
//! words-only, text-only) so animated captions always have clean timing.

use std::path::Path;

use transcribe_cpp::{CancelToken, Model, RunOptions};

use super::cancel;

use super::words::{RawSeg, RawWord};
use super::Transcript;

/// Transcribe 16 kHz mono f32 PCM with a GGUF model. `language` is an optional
/// ISO source-language hint (`None` = let the model autodetect).
pub(crate) fn transcribe_gguf(
    model_path: &Path,
    model_id: &str,
    samples: &[f32],
    language: Option<&str>,
) -> Result<Transcript, String> {
    let model = Model::load(model_path).map_err(|e| format!("load GGUF model: {e}"))?;
    let mut session = model
        .session()
        .map_err(|e| format!("open ggml session: {e}"))?;

    // Default timestamps = Auto (richest the model supports). Only pass a language
    // hint; leave task = Transcribe and the rest at their defaults.
    let opts = RunOptions {
        language: language.map(|s| s.to_string()),
        ..Default::default()
    };

    // The native abort callback is the only thing that can interrupt inference
    // part-way; without it Cancel would just hide a run still burning the CPU.
    let token = CancelToken::new();
    session.set_cancel_token(&token);
    cancel::install(&token);
    let run = session.run(samples, &opts);
    cancel::uninstall();
    let result = run.map_err(|e| {
        if token.is_cancelled() {
            cancel::CANCELLED_MSG.to_string()
        } else {
            format!("ggml transcription failed: {e}")
        }
    })?;

    // Shape varies by family (Whisper: many segments; Parakeet: one segment +
    // per-word times). Logged so a missing-timing report can be diagnosed.
    log::info!(
        "ggml {model_id}: kind={:?} segments={} words={} tokens={}",
        result.timestamp_kind,
        result.segments.len(),
        result.words.len(),
        result.tokens.len()
    );

    let total_secs = samples.len() as f64 / 16_000.0;
    let segs: Vec<RawSeg> = result
        .segments
        .iter()
        .map(|s| RawSeg {
            t0_ms: s.t0_ms,
            t1_ms: s.t1_ms,
            text: s.text.clone(),
        })
        .collect();
    let words: Vec<RawWord> = result
        .words
        .iter()
        .map(|w| RawWord {
            t0_ms: w.t0_ms,
            t1_ms: w.t1_ms,
            text: w.text.clone(),
        })
        .collect();
    let segments = super::words::build_segments(&result.text, total_secs, &segs, &words);

    Ok(Transcript {
        engine: "ggml".into(),
        model_id: model_id.to_string(),
        language: result.language.clone(),
        segments,
    })
}
