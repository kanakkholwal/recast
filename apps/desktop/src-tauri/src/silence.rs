//! Silence detection for the editor timeline.
//!
//! Candidates come from a Silero voice-activity model — per-frame speech
//! probability — not an energy threshold. Room tone, breathing and keyboard
//! noise all sit well above the noise floor an RMS gate keys on, so the old
//! envelope approach both leaked false silences and swallowed quiet speech.
//! A range is a candidate when the model reports non-speech (with hysteresis,
//! so a single dipping frame doesn't split a run) for at least
//! `min_audio_silence` seconds.
//!
//! The cursor track is a *confidence* signal, not a gate. An idle cursor over
//! the range raises the score; a moving cursor no longer vetoes it, so
//! webcam / talking-head recordings with no meaningful cursor still get
//! suggestions. Nothing is cut automatically — these are suggestions only.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tauri::AppHandle;
use tract_onnx::prelude::*;

use serde::{Deserialize, Serialize};

use crate::commands::error::{AppError, AppResult};

//  Options / output

/// Detection thresholds. Every field has a default so the frontend may send
/// a partial object — or nothing at all.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SilenceOptions {
    /// Speech-probability threshold in [0,1]: a frame is speech once the model
    /// scores at or above it. Higher = more aggressive (more gets called
    /// silence). Hysteresis derives a lower release threshold from this.
    #[serde(default = "d_threshold")]
    pub threshold: f32,
    /// Minimum continuous non-speech run for a candidate (seconds).
    #[serde(default = "d_min_audio_silence")]
    pub min_audio_silence: f64,
    /// Minimum length of a returned silence segment (seconds).
    #[serde(default = "d_min_segment")]
    pub min_segment: f64,
}

fn d_threshold() -> f32 {
    0.5
}
fn d_min_audio_silence() -> f64 {
    0.6
}
fn d_min_segment() -> f64 {
    1.0
}

impl Default for SilenceOptions {
    fn default() -> Self {
        Self {
            threshold: d_threshold(),
            min_audio_silence: d_min_audio_silence(),
            min_segment: d_min_segment(),
        }
    }
}

/// A detected silence range, in original-recording seconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SilenceSegment {
    pub start: f64,
    pub end: f64,
    /// 0..1 — how strongly this range warrants a cut.
    pub confidence: f32,
    /// Microphone track was present and contributed to the audio analysis.
    pub mic_silent: bool,
    /// System-audio track was present and contributed to the audio analysis.
    pub system_silent: bool,
    /// Cursor track was present and confirmed idle over the range.
    pub cursor_idle: bool,
}

type Interval = (f64, f64);

// Silero v5 runs at 16 kHz with a fixed 512-sample window (32 ms/frame).
const RATE: u32 = 16_000;
const CHUNK: usize = 512;
/// Silero v5's combined recurrent state: one [2, 1, 128] tensor. v5 merged v4's
/// separate `h`/`c` LSTM tensors into a single `state` in/out.
const STATE: [usize; 3] = [2, 1, 128];

/// Silero VAD on **tract** (pure Rust — no native ONNX Runtime, so the always-on
/// silence path builds on every target incl. x86_64-apple-darwin). v5 model I/O:
/// `input`/`state`/`sr` → `output`/`stateN`, carrying one recurrent-state tensor
/// between windows. (Feeding v4's `h`/`c` crashed on the v5 model — "No node named h".)
struct SileroVad {
    plan: TypedSimplePlan<TypedModel>,
    state: Tensor,
    sr: Tensor,
}

impl SileroVad {
    fn new(path: &Path) -> Result<Self, String> {
        let map = |e: TractError, what: &str| format!("Silero {what}: {e}");
        let plan = tract_onnx::onnx()
            .model_for_path(path)
            .map_err(|e| map(e, "load"))?
            // Pin input/output order and shapes so tract can optimize the graph. v5 order: input, state, sr to output, stateN.
            .with_input_names(["input", "state", "sr"])
            .map_err(|e| map(e, "input names"))?
            .with_output_names(["output", "stateN"])
            .map_err(|e| map(e, "output names"))?
            .with_input_fact(0, f32::fact([1, CHUNK]).into())
            .map_err(|e| map(e, "input fact"))?
            .with_input_fact(1, f32::fact(STATE).into())
            .map_err(|e| map(e, "state fact"))?
            .with_input_fact(2, i64::fact([1]).into())
            .map_err(|e| map(e, "sr fact"))?
            .into_optimized()
            .map_err(|e| map(e, "optimize"))?
            .into_runnable()
            .map_err(|e| map(e, "runnable"))?;
        Ok(Self {
            plan,
            state: Tensor::zero::<f32>(&STATE).map_err(|e| map(e, "state"))?,
            sr: tensor1(&[RATE as i64]),
        })
    }

    /// Clear the LSTM state so the next window starts a fresh sequence.
    fn reset(&mut self) -> Result<(), String> {
        self.state = Tensor::zero::<f32>(&STATE).map_err(|e| format!("Silero reset: {e}"))?;
        Ok(())
    }

    /// Speech probability for one 512-sample window; advances the LSTM state.
    fn compute(&mut self, window: &[f32]) -> Result<f32, String> {
        let input = Tensor::from_shape(&[1, window.len()], window)
            .map_err(|e| format!("Silero window: {e}"))?;
        let out = self
            .plan
            .run(tvec!(
                input.into(),
                self.state.clone().into(),
                self.sr.clone().into(),
            ))
            .map_err(|e| format!("Silero run: {e}"))?;
        let prob = out[0]
            .to_array_view::<f32>()
            .map_err(|e| format!("Silero output: {e}"))?
            .iter()
            .copied()
            .next()
            .unwrap_or(0.0);
        self.state = out[1].clone().into_tensor();
        Ok(prob)
    }
}
/// How far below `threshold` the score must fall to *end* a speech run. The
/// gap is the hysteresis band: it stops a single quiet frame mid-word from
/// fracturing speech into spurious micro-silences.
const RELEASE_MARGIN: f32 = 0.15;
/// Cursor counts as idle once it stays within this radius for this long.
const CURSOR_IDLE_MIN_US: u64 = 300_000;
const CURSOR_IDLE_RADIUS_PX: f64 = 8.0;
/// A candidate whose duration is at least this fraction covered by cursor-idle
/// time is reported as cursor-confirmed (drives the `cursor_idle` flag).
const CURSOR_CONFIRM_FRAC: f64 = 0.5;

//  Command

/// Silero VAD v5 ONNX (16 kHz, 512-sample window). `vad-rs` loads it from a
/// path — it isn't embedded — so we fetch it on first use.
/// TODO: confirm this is the exact model `vad-rs` expects + pin its sha256.
const SILERO_URL: &str =
    "https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx";

/// Ensure `silero_vad.onnx` is on disk (downloaded once), under
/// `app_data/models/silero/`. Reuses the captions download/verify helper.
async fn ensure_silero(app: &AppHandle) -> Result<PathBuf, String> {
    let path = crate::transcription::models_dir(app)?
        .join("silero")
        .join("silero_vad.onnx");
    if !path.exists() {
        let client = reqwest::Client::builder()
            .user_agent("recast-desktop")
            .build()
            .map_err(|e| format!("client: {e}"))?;
        crate::transcription::download_file(&client, SILERO_URL, None, &path, |_, _| {}).await?;
    }
    Ok(path)
}

#[tauri::command]
pub async fn detect_silence(
    app: AppHandle,
    audio_path: Option<String>,
    microphone_path: Option<String>,
    cursor_path: Option<String>,
    options: Option<SilenceOptions>,
) -> AppResult<Vec<SilenceSegment>> {
    let opts = options.unwrap_or_default();
    let silero = ensure_silero(&app).await?;
    tokio::task::spawn_blocking(move || {
        detect_blocking(
            &silero,
            audio_path.as_deref(),
            microphone_path.as_deref(),
            cursor_path.as_deref(),
            opts,
        )
    })
    .await
    .map_err(|e| AppError::msg(format!("silence-detection task panicked: {e}")))?
    .map_err(Into::into)
}

fn detect_blocking(
    silero_path: &Path,
    audio_path: Option<&str>,
    microphone_path: Option<&str>,
    cursor_path: Option<&str>,
    opts: SilenceOptions,
) -> Result<Vec<SilenceSegment>, String> {
    let inputs: Vec<&str> = [audio_path, microphone_path]
        .into_iter()
        .flatten()
        .filter(|p| Path::new(p).exists())
        .collect();
    if inputs.is_empty() {
        return Err("no audio track available to analyse".into());
    }

    // Each run is a full FFmpeg decode plus per-frame inference, so serve it from the identity cache; the cursor track is a source too.
    let mut sources: Vec<&Path> = inputs.iter().map(|p| Path::new(*p)).collect();
    if let Some(c) = cursor_path.filter(|c| Path::new(c).exists()) {
        sources.push(Path::new(c));
    }
    let key = opts_key(&opts);
    if let Some(cached) = crate::cache::get::<Vec<SilenceSegment>>("silence", &sources, key) {
        return Ok(cached);
    }

    // Decode the mixed audio to mono s16le at our analysis rate.
    let mut args: Vec<String> = vec!["-hide_banner".into(), "-nostats".into()];
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
        RATE.to_string(),
        "-f".into(),
        "s16le".into(),
        "-".into(),
    ]);
    let pcm = ffmpeg_stdout(&args)?;
    let samples: Vec<i16> = pcm
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| i16::from_le_bytes(c))
        .collect();
    if samples.len() < CHUNK {
        return Ok(Vec::new());
    }
    let total = samples.len() as f64 / RATE as f64;

    // Silero is a stateful LSTM, so frames are scored in order; the short trailing frame is zero-padded to a full window.
    let mut vad = SileroVad::new(silero_path)?;
    vad.reset()?;
    let mut probs: Vec<f32> = Vec::with_capacity(samples.len() / CHUNK + 1);
    let mut window = [0f32; CHUNK];
    for chunk in samples.chunks(CHUNK) {
        for (i, slot) in window.iter_mut().enumerate() {
            *slot = chunk.get(i).map(|s| *s as f32 / 32768.0).unwrap_or(0.0);
        }
        probs.push(vad.compute(&window)?);
    }
    let frame_dur = CHUNK as f64 / RATE as f64;

    // Non-speech runs as frame-index ranges, gated by minimum duration.
    let runs = silence_runs(&probs, frame_dur, opts.threshold, opts.min_audio_silence);

    // A confidence signal, not a gate: a missing cursor track just means no confirmation, and candidates still stand.
    let (cursor_idle, has_cursor) = match cursor_path {
        Some(p) if Path::new(p).exists() => {
            let bytes =
                std::fs::read(Path::new(p)).map_err(|e| format!("read cursor track: {e}"))?;
            let track: crate::cursor::CursorTrack =
                serde_json::from_slice(&bytes).map_err(|e| format!("parse cursor track: {e}"))?;
            let periods = crate::cursor::smoothing::detect_idle_periods(
                &track.samples,
                CURSOR_IDLE_MIN_US,
                CURSOR_IDLE_RADIUS_PX,
            );
            let ivs: Vec<Interval> = periods
                .into_iter()
                .map(|p| {
                    (
                        p.start_us as f64 / 1_000_000.0,
                        p.end_us as f64 / 1_000_000.0,
                    )
                })
                .collect();
            (ivs, true)
        }
        _ => (Vec::new(), false),
    };

    let mic_present = microphone_path
        .map(|p| Path::new(p).exists())
        .unwrap_or(false);
    let system_present = audio_path.map(|p| Path::new(p).exists()).unwrap_or(false);

    let mut out = Vec::new();
    for (s, e) in runs {
        let start = s as f64 * frame_dur;
        let end = (e as f64 * frame_dur).min(total);
        if end - start < opts.min_segment {
            continue;
        }
        let mean_speech = mean(&probs[s..e]);
        let idle_frac = if has_cursor {
            overlap_fraction((start, end), &cursor_idle)
        } else {
            0.0
        };
        out.push(SilenceSegment {
            start: round3(start),
            end: round3(end),
            confidence: score(end - start, mean_speech, idle_frac, has_cursor),
            mic_silent: mic_present,
            system_silent: system_present,
            cursor_idle: has_cursor && idle_frac >= CURSOR_CONFIRM_FRAC,
        });
    }
    crate::cache::put("silence", &sources, key, &out);
    Ok(out)
}

/// Fold the detection options into a cache discriminator so a different
/// sensitivity doesn't collide with a previously cached result.
fn opts_key(opts: &SilenceOptions) -> u64 {
    let mut h = DefaultHasher::new();
    opts.threshold.to_bits().hash(&mut h);
    opts.min_audio_silence.to_bits().hash(&mut h);
    opts.min_segment.to_bits().hash(&mut h);
    h.finish()
}

//  Audio analysis

/// Walk per-frame speech probabilities and return the non-speech runs as
/// half-open frame-index ranges `[start, end)`, keeping only those at least
/// `min_dur` seconds long.
///
/// A two-threshold state machine provides hysteresis: a frame opens a speech
/// run at `threshold` and only closes it once the score falls below
/// `threshold - RELEASE_MARGIN`. Without the gap, one quiet frame inside a
/// word would carve a real utterance into spurious micro-silences.
fn silence_runs(
    probs: &[f32],
    frame_dur: f64,
    threshold: f32,
    min_dur: f64,
) -> Vec<(usize, usize)> {
    let release = (threshold - RELEASE_MARGIN).max(0.0);
    let min_frames = (min_dur / frame_dur).ceil() as usize;

    let mut out = Vec::new();
    let mut speaking = false;
    let mut run_start = 0usize;
    for (i, &p) in probs.iter().enumerate() {
        if speaking {
            if p < release {
                speaking = false;
                run_start = i;
            }
        } else if p >= threshold {
            if i - run_start >= min_frames {
                out.push((run_start, i));
            }
            speaking = true;
        }
    }
    if !speaking && probs.len() - run_start >= min_frames {
        out.push((run_start, probs.len()));
    }
    out
}

fn mean(xs: &[f32]) -> f32 {
    if xs.is_empty() {
        return 0.0;
    }
    xs.iter().sum::<f32>() / xs.len() as f32
}

//  Interval algebra

/// Fraction of `seg`'s duration covered by the (sorted, disjoint) `cover`
/// intervals, in [0,1].
fn overlap_fraction(seg: Interval, cover: &[Interval]) -> f64 {
    let span = seg.1 - seg.0;
    if span <= 0.0 {
        return 0.0;
    }
    let covered: f64 = intersect(&[seg], cover).iter().map(|iv| iv.1 - iv.0).sum();
    (covered / span).clamp(0.0, 1.0)
}

/// Intersect two sorted, non-overlapping interval lists.
fn intersect(a: &[Interval], b: &[Interval]) -> Vec<Interval> {
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        let lo = a[i].0.max(b[j].0);
        let hi = a[i].1.min(b[j].1);
        if hi > lo {
            out.push((lo, hi));
        }
        if a[i].1 < b[j].1 {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

//  Confidence

/// Blend three signals into a 0..1 score:
///   - how confidently non-speech the audio is (`1 - mean_speech`),
///   - how long the run is (saturating at 4 s),
///   - cursor confirmation (only when a track was present), proportional to
///     how much of the run the cursor sat idle through.
fn score(len: f64, mean_speech: f32, idle_frac: f64, has_cursor: bool) -> f32 {
    let audio_conf = (1.0 - mean_speech).clamp(0.0, 1.0) as f64;
    let len_score = (len / 4.0).min(1.0);
    let cursor_bonus = if has_cursor { 0.15 * idle_frac } else { 0.0 };
    (0.55 * audio_conf + 0.30 * len_score + cursor_bonus).clamp(0.0, 1.0) as f32
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

//  ffmpeg I/O

/// Spawn ffmpeg and return its raw stdout bytes.
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

//  Waveform extraction (the timeline-display backing data)

/// Decode a recording's audio to a compact peak envelope for the timeline.
///
/// Mic + system audio are mixed (if both exist), downsampled to a low rate,
/// and reduced to `buckets` normalised peak values in [0,1]. The result is
/// purely visual — it lets the user *see* where the silence is.
#[tauri::command]
pub async fn extract_waveform(
    audio_path: Option<String>,
    microphone_path: Option<String>,
    buckets: Option<usize>,
) -> AppResult<Vec<f32>> {
    let buckets = buckets.unwrap_or(2000).clamp(64, 8000);
    tokio::task::spawn_blocking(move || {
        waveform_blocking(audio_path.as_deref(), microphone_path.as_deref(), buckets)
    })
    .await
    .map_err(|e| AppError::msg(format!("waveform task panicked: {e}")))?
    .map_err(Into::into)
}

fn waveform_blocking(
    audio_path: Option<&str>,
    microphone_path: Option<&str>,
    buckets: usize,
) -> Result<Vec<f32>, String> {
    // Visual fidelity only: 4 kHz mono is plenty for an envelope and keeps hour-long recordings bounded.
    const WAVE_RATE: u32 = 4000;

    let inputs: Vec<&str> = [audio_path, microphone_path]
        .into_iter()
        .flatten()
        .filter(|p| Path::new(p).exists())
        .collect();
    if inputs.is_empty() {
        return Ok(Vec::new());
    }

    // Computing the envelope means a full decode (1-3s), so cache it keyed by every input's identity plus the bucket count.
    let input_paths: Vec<&Path> = inputs.iter().map(|p| Path::new(*p)).collect();
    if let Some(cached) = crate::cache::get::<Vec<f32>>("waveform", &input_paths, buckets as u64) {
        return Ok(cached);
    }

    let mut args: Vec<String> = vec!["-hide_banner".into(), "-nostats".into()];
    for input in &inputs {
        args.push("-i".into());
        args.push((*input).to_string());
    }
    if inputs.len() > 1 {
        args.push("-filter_complex".into());
        args.push(format!("amix=inputs={}:normalize=0", inputs.len()));
    }
    args.extend([
        "-ac".into(),
        "1".into(),
        "-ar".into(),
        WAVE_RATE.to_string(),
        "-f".into(),
        "s16le".into(),
        "-".into(),
    ]);

    let pcm = ffmpeg_stdout(&args)?;
    let samples: Vec<i16> = pcm
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| i16::from_le_bytes(c))
        .collect();
    if samples.len() < 2 {
        return Ok(Vec::new());
    }

    let n = buckets.min(samples.len()).max(1);
    let per = samples.len() as f64 / n as f64;
    let mut out = vec![0f32; n];
    for (i, bucket) in out.iter_mut().enumerate() {
        let lo = (i as f64 * per) as usize;
        let hi = (((i + 1) as f64 * per) as usize)
            .min(samples.len())
            .max(lo + 1);
        let peak = samples[lo..hi]
            .iter()
            .map(|s| (*s as i32).unsigned_abs())
            .max()
            .unwrap_or(0);
        *bucket = (peak as f32 / 32768.0).min(1.0);
    }
    crate::cache::put("waveform", &input_paths, buckets as u64, &out);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{intersect, overlap_fraction, round3, score, silence_runs};

    // frame_dur 1.0 makes frame index == seconds, so min_dur reads directly.
    const DUR: f64 = 1.0;

    #[test]
    fn silence_runs_finds_leading_internal_and_trailing_gaps() {
        let probs = [0.1, 0.1, 0.9, 0.9, 0.1, 0.1, 0.1, 0.9];
        assert_eq!(silence_runs(&probs, DUR, 0.5, 2.0), vec![(0, 2), (4, 7)]);
    }

    #[test]
    fn silence_runs_hysteresis_does_not_split_speech_on_a_single_dip() {
        // 0.4 sits in the release-to-threshold band, so the speech run holds through it instead of fracturing.
        let probs = [0.9, 0.9, 0.4, 0.9, 0.9];
        assert!(silence_runs(&probs, DUR, 0.5, 2.0).is_empty());
    }

    #[test]
    fn silence_runs_drops_runs_below_min_duration() {
        // A single-frame gap under 2s is discarded; the trailing 3-frame gap is kept and runs to the end.
        assert!(silence_runs(&[0.1, 0.9, 0.9], DUR, 0.5, 2.0).is_empty());
        assert_eq!(
            silence_runs(&[0.9, 0.9, 0.1, 0.1, 0.1], DUR, 0.5, 2.0),
            vec![(2, 5)]
        );
        assert_eq!(silence_runs(&[0.1; 5], DUR, 0.5, 2.0), vec![(0, 5)]);
    }

    #[test]
    fn intersect_overlapping_disjoint_and_nested() {
        assert_eq!(intersect(&[(0.0, 5.0)], &[(3.0, 8.0)]), vec![(3.0, 5.0)]);
        assert!(intersect(&[(0.0, 1.0)], &[(2.0, 3.0)]).is_empty());
        assert_eq!(
            intersect(&[(0.0, 2.0), (5.0, 9.0)], &[(1.0, 6.0)]),
            vec![(1.0, 2.0), (5.0, 6.0)]
        );
        assert_eq!(intersect(&[(0.0, 10.0)], &[(2.0, 4.0)]), vec![(2.0, 4.0)]);
    }

    #[test]
    fn overlap_fraction_measures_covered_share() {
        assert!((overlap_fraction((0.0, 10.0), &[(2.0, 4.0), (6.0, 7.0)]) - 0.3).abs() < 1e-9);
        assert_eq!(overlap_fraction((0.0, 10.0), &[]), 0.0);
        assert_eq!(overlap_fraction((5.0, 5.0), &[(0.0, 10.0)]), 0.0);
    }

    #[test]
    fn score_blends_audio_length_and_cursor() {
        // Deeply silent (mean 0), 4 s saturates length: 0.55 + 0.30.
        assert!((score(4.0, 0.0, 0.0, false) - 0.85).abs() < 1e-6);
        // Full cursor-idle confirmation adds 0.15 → clamps at 1.0.
        assert!((score(4.0, 0.0, 1.0, true) - 1.0).abs() < 1e-6);
        // High mean speech probability collapses the audio term.
        assert!((score(4.0, 1.0, 0.0, false) - 0.30).abs() < 1e-6);
        // Shorter run → half the length term, no cursor track.
        assert!((score(2.0, 0.0, 0.0, false) - 0.70).abs() < 1e-6);
        assert!((0.0..=1.0).contains(&score(100.0, 0.0, 1.0, true)));
    }

    #[test]
    fn round3_rounds_to_milliseconds() {
        assert_eq!(round3(1.234_56), 1.235);
        assert_eq!(round3(0.0), 0.0);
        assert_eq!(round3(2.0 / 3.0), 0.667);
    }

    // Integration guard: the model is fetched at runtime, so point RECAST_SILERO_PATH at a local copy or the test skips.
    #[test]
    fn silero_model_loads_and_scores_silence_low() {
        let Ok(model) = std::env::var("RECAST_SILERO_PATH") else {
            eprintln!("skipping: set RECAST_SILERO_PATH to the silero_vad.onnx file");
            return;
        };
        let mut vad = super::SileroVad::new(std::path::Path::new(&model)).expect("init Silero VAD");
        vad.reset().expect("reset");
        let p = vad
            .compute(&[0f32; super::CHUNK])
            .expect("Silero VAD compute");
        assert!((0.0..=1.0).contains(&p), "probability in range, got {p}");
        assert!(
            p < 0.5,
            "digital silence should read as non-speech, got {p}"
        );
    }
}
