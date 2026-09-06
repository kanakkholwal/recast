//! One decode for every analysis that wants mono audio at a chosen rate:
//! transcription, the waveform envelope and the silence VAD all asked FFmpeg
//! for the same `-ac 1 -ar N -f s16le` and parsed the same bytes back.

use std::path::Path;
use std::process::{Command, Stdio};

/// Mono samples in [-1, 1] at `rate`, summed across every readable input.
///
/// Summed, not averaged: it reproduces FFmpeg's `amix=normalize=0`, which is
/// what all three callers asked for and what keeps a quiet mic quiet next to
/// loud system audio.
pub fn decode_mono(paths: &[&Path], rate: u32) -> Result<Vec<f32>, String> {
    let inputs: Vec<&Path> = paths.iter().copied().filter(|p| p.exists()).collect();
    if inputs.is_empty() {
        return Ok(Vec::new());
    }
    match decode_native(&inputs, rate) {
        Some(samples) => Ok(samples),
        None => decode_via_ffmpeg(&inputs, rate, 1),
    }
}

/// One file as interleaved samples at `rate` and `channels`, decoded by FFmpeg.
///
/// The fallback for a codec the in-process reader refuses: without it an Opus
/// or FLAC source exports silent rather than merely slower.
pub fn decode_interleaved(path: &Path, rate: u32, channels: u16) -> Result<Vec<f32>, String> {
    decode_via_ffmpeg(&[path], rate, channels)
}

/// The in-process decoder, or `None` where there is none or a source it cannot
/// read. `None` is not a failure: the FFmpeg path takes over.
#[cfg(windows)]
fn decode_native(inputs: &[&Path], rate: u32) -> Option<Vec<f32>> {
    use recast_codec_mf::{AudioFormat, AudioReader};

    let format = AudioFormat {
        sample_rate: rate,
        channels: 1,
    };
    let mut mixed: Vec<f32> = Vec::new();
    for path in inputs {
        let mut reader = match AudioReader::open(path, format) {
            Ok(Some(reader)) => reader,
            // A source with no audio track contributes silence, as it does to `amix`.
            Ok(None) => continue,
            Err(error) => {
                log::warn!("native audio decode ({}): {error}", path.display());
                return None;
            }
        };
        let samples = match reader.read_all() {
            Ok(samples) => samples,
            Err(error) => {
                log::warn!("native audio decode ({}): {error}", path.display());
                return None;
            }
        };
        sum_into(&mut mixed, &samples);
    }
    Some(mixed)
}

#[cfg(not(windows))]
fn decode_native(_inputs: &[&Path], _rate: u32) -> Option<Vec<f32>> {
    None
}

/// Adds `samples` onto `mixed`, growing it for a longer source.
// Only the native mixer calls this; off Windows its sole callers are the tests.
#[cfg(any(windows, test))]
fn sum_into(mixed: &mut Vec<f32>, samples: &[f32]) {
    if mixed.len() < samples.len() {
        mixed.resize(samples.len(), 0.0);
    }
    for (slot, add) in mixed.iter_mut().zip(samples) {
        *slot += add;
    }
}

fn decode_via_ffmpeg(inputs: &[&Path], rate: u32, channels: u16) -> Result<Vec<f32>, String> {
    let mut args: Vec<String> = vec!["-hide_banner".into(), "-nostats".into()];
    for path in inputs {
        args.push("-i".into());
        args.push(path.to_string_lossy().into_owned());
    }
    if inputs.len() > 1 {
        args.push("-filter_complex".into());
        args.push(format!("amix=inputs={}:normalize=0", inputs.len()));
    }
    args.extend([
        "-ac".into(),
        channels.to_string(),
        "-ar".into(),
        rate.to_string(),
        "-f".into(),
        "s16le".into(),
        "-".into(),
    ]);

    let mut command = Command::new(crate::ffmpeg::ffmpeg_path());
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    crate::ffmpeg::configure_silent_command(&mut command);
    let output = command
        .output()
        .map_err(|e| format!("failed to run ffmpeg: {e}"))?;
    if !output.status.success() {
        return Err("ffmpeg exited with an error while decoding audio".into());
    }
    Ok(output
        .stdout
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| f32::from(i16::from_le_bytes(c)) / 32768.0)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tone written by the sidecar, so both decoders read the same file.
    #[cfg(windows)]
    fn tone(ffmpeg: &Path, path: &Path, hz: u32, seconds: f64) {
        let status = Command::new(ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("sine=frequency={hz}:duration={seconds}"),
                "-c:a",
                "aac",
                &path.to_string_lossy(),
            ])
            .status()
            .expect("ffmpeg runs");
        assert!(status.success(), "the fixture tone did not encode");
    }

    #[cfg(windows)]
    fn rms(samples: &[f32]) -> f64 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = samples.iter().map(|s| f64::from(*s) * f64::from(*s)).sum();
        (sum / samples.len() as f64).sqrt()
    }

    /// The swap is only safe if the in-process decoder hears what FFmpeg heard.
    /// Compared by loudness and length rather than sample for sample: the two
    /// resamplers and AAC decoders are different implementations.
    #[cfg(windows)]
    #[test]
    fn the_native_decode_matches_what_ffmpeg_hears() {
        let Some(ffmpeg) = recast_testkit::ffmpeg_path() else {
            return;
        };
        let scratch = recast_testkit::Scratch::new("audio-decode");
        let input = scratch.file("tone.m4a");
        tone(&ffmpeg, &input, 440, 1.0);

        let Some(native) = decode_native(&[&input], 16_000) else {
            panic!("the native decoder refused a file FFmpeg wrote");
        };
        let piped = decode_via_ffmpeg(&[&input], 16_000, 1).expect("the ffmpeg decode runs");

        assert!(
            !native.is_empty() && !piped.is_empty(),
            "one decode was silent"
        );
        let (a, b) = (native.len() as f64, piped.len() as f64);
        assert!(
            (a - b).abs() / b.max(1.0) < 0.05,
            "lengths disagree: {a} native, {b} piped"
        );
        let (loud_a, loud_b) = (rms(&native), rms(&piped));
        assert!(
            (loud_a - loud_b).abs() < 0.02,
            "loudness disagrees: {loud_a:.4} native, {loud_b:.4} piped"
        );
    }

    /// Two sources have to reach the mix, not just the first: the silence and
    /// transcription paths both pass system audio and a microphone.
    #[cfg(windows)]
    #[test]
    fn a_second_source_is_added_to_the_mix() {
        let Some(ffmpeg) = recast_testkit::ffmpeg_path() else {
            return;
        };
        let scratch = recast_testkit::Scratch::new("audio-mix");
        let one = scratch.file("a.m4a");
        let two = scratch.file("b.m4a");
        tone(&ffmpeg, &one, 440, 1.0);
        tone(&ffmpeg, &two, 660, 1.0);

        let single = decode_mono(&[&one], 16_000).expect("one source");
        let both = decode_mono(&[&one, &two], 16_000).expect("two sources");
        assert!(
            rms(&both) > rms(&single) * 1.15,
            "the second source did not reach the mix: {:.4} then {:.4}",
            rms(&single),
            rms(&both)
        );
    }

    #[test]
    fn nothing_readable_decodes_to_nothing() {
        let missing = std::env::temp_dir().join("recast-no-such-audio.wav");
        assert!(decode_mono(&[&missing], 16_000)
            .expect("not an error")
            .is_empty());
        assert!(decode_mono(&[], 16_000).expect("not an error").is_empty());
    }

    #[test]
    fn a_longer_source_grows_the_mix_rather_than_being_clipped() {
        let mut mixed = vec![0.25, 0.25];
        sum_into(&mut mixed, &[0.5, 0.5, 0.5, 0.5]);
        assert_eq!(mixed, vec![0.75, 0.75, 0.5, 0.5]);
    }

    /// Summed, not averaged: averaging would halve a mic track the moment
    /// system audio was recorded alongside it.
    #[test]
    fn two_sources_add_rather_than_average() {
        let mut mixed = vec![0.5, 0.5];
        sum_into(&mut mixed, &[0.5, 0.5]);
        assert_eq!(mixed, vec![1.0, 1.0]);
    }
}
