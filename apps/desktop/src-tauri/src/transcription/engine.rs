//! Inference seam — where the model is actually run.
//!
//! **Parakeet** via `transcribe-rs` (ONNX/`ort`). `ort` ships a prebuilt
//! runtime, so this needs no libclang/CMake. It shares one `ort` (rc.12) with
//! the `vad-rs` silence detector.
//!
//! **Whisper** (whisper.cpp) is intentionally deferred — enabling
//! transcribe-rs's `whisper-cpp` feature would pull the LLVM+CMake toolchain.
//! Until then the Whisper arm returns a typed error pointing at Parakeet.
//!
//! Behind the `captions` Cargo feature so the engine deps only enter an opt-in
//! build.

use std::path::Path;

use super::models::CaptionModel;
use super::Transcript;

#[cfg(not(feature = "captions"))]
pub fn transcribe(
    _model: &CaptionModel,
    _model_dir: &Path,
    _samples: &[f32],
    _language: Option<&str>,
) -> Result<Transcript, String> {
    Err("captions feature is not enabled in this build".into())
}

#[cfg(feature = "captions")]
pub fn transcribe(
    model: &CaptionModel,
    model_dir: &Path,
    samples: &[f32],
    _language: Option<&str>,
) -> Result<Transcript, String> {
    use super::models::Engine;
    use transcribe_rs::onnx::{
        canary::CanaryModel, cohere::CohereModel, gigaam::GigaAMModel, Quantization,
    };

    match model.engine {
        // Parakeet requests WORD-level timestamps; we group them into lines.
        Engine::Parakeet => parakeet_transcribe(model, model_dir, samples),
        // The other ONNX engines share one path via the `SpeechModel` trait.
        Engine::Canary => {
            let m = CanaryModel::load(model_dir, &Quantization::Int8)
                .map_err(|e| format!("load Canary model: {e}"))?;
            run_speech_model(m, model, samples)
        }
        Engine::GigaAM => {
            let m = GigaAMModel::load(model_dir, &Quantization::Int8)
                .map_err(|e| format!("load GigaAM model: {e}"))?;
            run_speech_model(m, model, samples)
        }
        Engine::Cohere => {
            let m = CohereModel::load(model_dir, &Quantization::Int4)
                .map_err(|e| format!("load Cohere model: {e}"))?;
            run_speech_model(m, model, samples)
        }
        Engine::Whisper => Err("Whisper (whisper.cpp) isn't enabled in this build yet — \
             use a Parakeet or Canary model. See docs/captions-transcription-plan.md."
            .into()),
        // Remote models are transcribed over HTTP in `transcribe_project` before
        // reaching the local engine, so this arm is never hit in practice.
        Engine::Remote => Err("remote models are transcribed via the remote path".into()),
    }
}

#[cfg(feature = "captions")]
fn parakeet_transcribe(
    model: &CaptionModel,
    model_dir: &Path,
    samples: &[f32],
) -> Result<Transcript, String> {
    use super::TranscriptWord;
    use transcribe_rs::onnx::parakeet::{ParakeetModel, ParakeetParams, TimestampGranularity};
    use transcribe_rs::onnx::Quantization;

    let mut pk = ParakeetModel::load(model_dir, &Quantization::Int8)
        .map_err(|e| format!("load Parakeet model: {e}"))?;
    // `Word` granularity returns a flat stream of one-word "segments"; we own the
    // grouping into caption lines (chunking is a caption concern, not an ASR one).
    let params = ParakeetParams {
        timestamp_granularity: Some(TimestampGranularity::Word),
        ..Default::default()
    };
    let result = pk
        .transcribe_with(samples, &params)
        .map_err(|e| format!("Parakeet transcription failed: {e}"))?;

    let words: Vec<TranscriptWord> = result
        .segments
        .as_ref()
        .map(|segs| {
            segs.iter()
                .map(|s| TranscriptWord {
                    start: s.start as f64,
                    end: s.end as f64,
                    text: s.text.trim().to_string(),
                })
                .filter(|w| !w.text.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let segments = if words.is_empty() {
        super::words::whole_clip_segment(&result.text, samples.len() as f64 / 16_000.0)
    } else {
        super::words::group_words_into_segments(words)
    };

    Ok(Transcript {
        engine: "parakeet".into(),
        model_id: model.id.clone(),
        language: None,
        segments,
    })
}

/// Run any engine that implements `SpeechModel` with default options, then map
/// its result into our transcript shape.
#[cfg(feature = "captions")]
fn run_speech_model<M: transcribe_rs::SpeechModel>(
    mut model_impl: M,
    model: &CaptionModel,
    samples: &[f32],
) -> Result<Transcript, String> {
    let result = model_impl
        .transcribe(samples, &transcribe_rs::TranscribeOptions::default())
        .map_err(|e| format!("transcription failed: {e}"))?;
    Ok(build_transcript(model, samples, result))
}

/// Map a transcribe-rs result into our `Transcript`. Segments carry seconds.
/// When an engine returns no segment timing, fall back to one block spanning
/// the clip so captions still render.
#[cfg(feature = "captions")]
fn build_transcript(
    model: &CaptionModel,
    samples: &[f32],
    result: transcribe_rs::TranscriptionResult,
) -> Transcript {
    use super::models::Engine;
    use super::TranscriptSegment;

    let mut segments: Vec<TranscriptSegment> = match result.segments {
        Some(segs) if !segs.is_empty() => segs
            .into_iter()
            .enumerate()
            .map(|(i, s)| TranscriptSegment {
                id: format!("seg-{i}"),
                start: s.start as f64,
                end: s.end as f64,
                text: s.text.trim().to_string(),
                words: Vec::new(),
            })
            .collect(),
        _ => super::words::whole_clip_segment(&result.text, samples.len() as f64 / 16_000.0),
    };
    // These engines give sentence timing only — synthesize per-word timing so
    // animated caption styles have something to drive (lower accuracy than real
    // word timestamps; documented in caption-animations-plan.md).
    super::words::fill_segment_words(&mut segments);

    let engine = match model.engine {
        Engine::Parakeet => "parakeet",
        Engine::Canary => "canary",
        Engine::GigaAM => "gigaam",
        Engine::Cohere => "cohere",
        Engine::Whisper => "whisper",
        Engine::Remote => "remote",
    };

    Transcript {
        engine: engine.into(),
        model_id: model.id.clone(),
        language: None,
        segments,
    }
}
