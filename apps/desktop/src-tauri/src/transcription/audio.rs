//! Audio extraction for transcription: mono 16 kHz, the same rate the Silero VAD path already decodes to.

use std::path::Path;

/// Target sample rate for ASR input. Whisper and Parakeet both expect 16 kHz.
pub const SAMPLE_RATE: u32 = 16_000;

/// Decode and downmix `sources` to mono 16 kHz f32. Multiple inputs are summed
/// without normalisation; an empty vec when nothing readable exists.
pub fn extract_pcm_f32(sources: &[&str]) -> Result<Vec<f32>, String> {
    let paths: Vec<&Path> = sources.iter().map(|p| Path::new(*p)).collect();
    crate::audio_decode::decode_mono(&paths, SAMPLE_RATE)
}
