//! Audio extraction for transcription: mono 16 kHz, the same rate the Silero VAD path already decodes to.
//! FFmpeg does the downmix and resample to `s16le`; this converts to normalised f32 in [-1, 1].

use std::path::Path;
use std::process::{Command, Stdio};

/// Target sample rate for ASR input. Whisper and Parakeet both expect 16 kHz.
pub const SAMPLE_RATE: u32 = 16_000;

/// Decode + downmix the given audio sources to mono 16 kHz f32 samples. Multiple
/// inputs are mixed without normalisation (matching the silence path). Returns
/// an empty vec when no source exists.
pub fn extract_pcm_f32(sources: &[&str]) -> Result<Vec<f32>, String> {
    let inputs: Vec<&str> = sources
        .iter()
        .copied()
        .filter(|p| Path::new(p).exists())
        .collect();
    if inputs.is_empty() {
        return Ok(Vec::new());
    }

    let mut args: Vec<String> = Vec::new();
    for p in &inputs {
        args.push("-i".into());
        args.push((*p).to_string());
    }
    if inputs.len() > 1 {
        args.push("-filter_complex".into());
        args.push(format!("amix=inputs={}:normalize=0", inputs.len()));
    }
    args.extend([
        "-ac".into(),
        "1".into(),
        "-ar".into(),
        SAMPLE_RATE.to_string(),
        "-f".into(),
        "s16le".into(),
        "-".into(),
    ]);

    let pcm = ffmpeg_stdout(&args)?;
    let samples = pcm
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| i16::from_le_bytes(c) as f32 / 32768.0)
        .collect();
    Ok(samples)
}

/// Spawn ffmpeg and return its raw stdout bytes. Mirrors `silence.rs` — silent
/// command (no Windows console flash), stderr discarded.
fn ffmpeg_stdout(args: &[String]) -> Result<Vec<u8>, String> {
    let mut cmd = Command::new(crate::ffmpeg::ffmpeg_path());
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    crate::ffmpeg::configure_silent_command(&mut cmd);
    let output = cmd
        .output()
        .map_err(|e| format!("failed to run ffmpeg: {e}"))?;
    if !output.status.success() {
        return Err("ffmpeg exited with an error while decoding audio".into());
    }
    Ok(output.stdout)
}
