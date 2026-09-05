//! Silence candidates from a Silero VAD, not an energy gate: room tone and keystrokes sit above any RMS floor.
//! The cursor track only weights confidence, so talking-head recordings still get suggestions; nothing is cut automatically.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

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

impl SilenceOptions {
    /// Clamp what the frontend sent into the range the analysis is defined over.
    /// Nothing validates these on the way in, and a non-finite threshold made
    /// every comparison false, which reads the whole recording as silent.
    #[must_use]
    fn sanitised(self) -> Self {
        let finite = |v: f64, fallback: f64| if v.is_finite() { v.max(0.0) } else { fallback };
        Self {
            threshold: if self.threshold.is_finite() {
                self.threshold.clamp(0.0, 1.0)
            } else {
                d_threshold()
            },
            min_audio_silence: finite(self.min_audio_silence, d_min_audio_silence()),
            min_segment: finite(self.min_segment, d_min_segment()),
        }
    }
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
}

/// The 16 kHz-only Silero v5 graph, built by `scripts/build-silero-vad.py`.
///
/// Embedded rather than downloaded: the published model branches on sample rate
/// with an ONNX `If` that `tract` cannot analyse, so it has to be pre-folded,
/// and a bundled file cannot go missing, move, or arrive as something else.
const MODEL: &[u8] = include_bytes!("../resources/silero_vad_16k.onnx");

impl SileroVad {
    fn new() -> Result<Self, String> {
        let map = |e: TractError, what: &str| format!("Silero {what}: {e}");
        // Sample rate is folded into the graph, so this takes input and state only.
        let plan = tract_onnx::onnx()
            .model_for_read(&mut std::io::Cursor::new(MODEL))
            .map_err(|e| map(e, "load"))?
            .with_input_names(["input", "state"])
            .map_err(|e| map(e, "input names"))?
            .with_output_names(["output", "stateN"])
            .map_err(|e| map(e, "output names"))?
            .with_input_fact(0, f32::fact([1, CHUNK]).into())
            .map_err(|e| map(e, "input fact"))?
            .with_input_fact(1, f32::fact(STATE).into())
            .map_err(|e| map(e, "state fact"))?
            .into_optimized()
            .map_err(|e| map(e, "optimize"))?
            .into_runnable()
            .map_err(|e| map(e, "runnable"))?;
        Ok(Self {
            plan,
            state: Tensor::zero::<f32>(&STATE).map_err(|e| map(e, "state"))?,
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
            .run(tvec!(input.into(), self.state.clone().into()))
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

#[tauri::command]
pub async fn detect_silence(
    audio_path: Option<String>,
    microphone_path: Option<String>,
    cursor_path: Option<String>,
    options: Option<SilenceOptions>,
) -> AppResult<Vec<SilenceSegment>> {
    let opts = options.unwrap_or_default();
    tokio::task::spawn_blocking(move || {
        detect_blocking(
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

    let decode_paths: Vec<&Path> = inputs.iter().map(|p| Path::new(*p)).collect();
    let samples = crate::audio_decode::decode_mono(&decode_paths, RATE)?;
    if samples.len() < CHUNK {
        return Ok(Vec::new());
    }
    let total = samples.len() as f64 / RATE as f64;

    // Silero is a stateful LSTM, so frames are scored in order; the short trailing frame is zero-padded to a full window.
    let mut vad = SileroVad::new()?;
    vad.reset()?;
    let mut probs: Vec<f32> = Vec::with_capacity(samples.len() / CHUNK + 1);
    let mut window = [0f32; CHUNK];
    for chunk in samples.chunks(CHUNK) {
        for (i, slot) in window.iter_mut().enumerate() {
            *slot = chunk.get(i).copied().unwrap_or(0.0);
        }
        probs.push(vad.compute(&window)?);
    }

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

    let out = segments_from(
        &probs,
        total,
        &opts,
        &CursorEvidence {
            idle: cursor_idle,
            present: has_cursor,
            mic: mic_present,
            system: system_present,
        },
    );
    crate::cache::put("silence", &sources, key, &out);
    Ok(out)
}

/// What the cursor track and the input files say about a candidate, kept apart
/// from the audio so the decision below is a pure function of both.
struct CursorEvidence {
    idle: Vec<Interval>,
    present: bool,
    mic: bool,
    system: bool,
}

/// Per-frame speech probabilities to the segments the editor offers to cut.
///
/// Sanitises the options itself: this is the one place they are interpreted, and
/// leaving that to the caller means a second entry point can skip it.
fn segments_from(
    probs: &[f32],
    total: f64,
    opts: &SilenceOptions,
    cursor: &CursorEvidence,
) -> Vec<SilenceSegment> {
    let opts = opts.sanitised();
    let frame_dur = CHUNK as f64 / RATE as f64;
    let mut out = Vec::new();
    for (s, e) in silence_runs(probs, frame_dur, opts.threshold, opts.min_audio_silence) {
        let start = s as f64 * frame_dur;
        let end = (e as f64 * frame_dur).min(total);
        if end - start < opts.min_segment {
            continue;
        }
        let mean_speech = mean(&probs[s..e]);
        let idle_frac = match cursor.present {
            true => overlap_fraction((start, end), &cursor.idle),
            false => 0.0,
        };
        out.push(SilenceSegment {
            start: round3(start),
            end: round3(end),
            confidence: score(end - start, mean_speech, idle_frac, cursor.present),
            mic_silent: cursor.mic,
            system_silent: cursor.system,
            cursor_idle: cursor.present && idle_frac >= CURSOR_CONFIRM_FRAC,
        });
    }
    out
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

/// Non-speech runs as half-open frame ranges, keeping only those at least `min_dur` seconds long.
/// Two thresholds give hysteresis: without the release margin, one quiet frame inside a word would carve an utterance into spurious micro-silences.
fn silence_runs(
    probs: &[f32],
    frame_dur: f64,
    threshold: f32,
    min_dur: f64,
) -> Vec<(usize, usize)> {
    let release = (threshold - RELEASE_MARGIN).max(0.0);
    // At least one frame: a zero or negative minimum otherwise matched the empty span at every speech onset and emitted runs of no length.
    let min_frames = ((min_dur / frame_dur).ceil() as usize).max(1);

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

/// Blends three signals into a 0..1 score: how confidently non-speech the audio is, how long the run is (saturating at 4 s), and cursor confirmation.
/// The cursor term only applies when a track was present, and is proportional to how much of the run the cursor sat idle through.
fn score(len: f64, mean_speech: f32, idle_frac: f64, has_cursor: bool) -> f32 {
    let audio_conf = (1.0 - mean_speech).clamp(0.0, 1.0) as f64;
    let len_score = (len / 4.0).min(1.0);
    let cursor_bonus = if has_cursor { 0.15 * idle_frac } else { 0.0 };
    (0.55 * audio_conf + 0.30 * len_score + cursor_bonus).clamp(0.0, 1.0) as f32
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

//  Waveform extraction (the timeline-display backing data)

/// Decodes a recording's audio to a compact peak envelope for the timeline.
/// Mic and system audio are mixed if both exist, downsampled, and reduced to `buckets` normalised peaks; purely visual, so the user can SEE where the silence is.
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

    let samples = crate::audio_decode::decode_mono(&input_paths, WAVE_RATE)?;
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
            .fold(0.0f32, |worst, s| worst.max(s.abs()));
        *bucket = peak.min(1.0);
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

    /// The frontend may send any number it likes, and a zero minimum made every
    /// speech onset emit a run of no length at all.
    #[test]
    fn a_zero_minimum_does_not_invent_empty_runs() {
        let runs = super::silence_runs(&[0.9, 0.9, 0.9], 0.032, 0.5, 0.0);
        assert!(
            runs.iter().all(|(s, e)| e > s),
            "zero-length run in {runs:?}"
        );
    }

    #[test]
    fn a_negative_minimum_does_not_invent_empty_runs() {
        let runs = super::silence_runs(&[0.9, 0.1, 0.9], 0.032, 0.5, -5.0);
        assert!(
            runs.iter().all(|(s, e)| e > s),
            "zero-length run in {runs:?}"
        );
    }

    // --- silence_runs ---

    #[test]
    fn continuous_speech_leaves_no_silence_to_cut() {
        assert!(super::silence_runs(&[0.9; 40], 0.032, 0.5, 0.2).is_empty());
    }

    #[test]
    fn a_recording_of_nothing_is_one_run_end_to_end() {
        assert_eq!(super::silence_runs(&[0.01; 40], 0.032, 0.5, 0.2), [(0, 40)]);
    }

    #[test]
    fn no_frames_at_all_is_no_runs() {
        assert!(super::silence_runs(&[], 0.032, 0.5, 0.2).is_empty());
    }

    /// The minimum is a floor, not a rounding: a run one frame short of it is
    /// not a candidate, and a run exactly on it is.
    #[test]
    fn the_minimum_duration_is_inclusive_at_its_boundary() {
        let with_gap = |gap: usize| {
            let mut probs = vec![0.9];
            probs.extend(std::iter::repeat_n(0.01, gap));
            probs.push(0.9);
            super::silence_runs(&probs, 0.1, 0.5, 0.3).len()
        };
        assert_eq!(with_gap(2), 0, "two frames is under 0.3s");
        assert_eq!(with_gap(3), 1, "three frames is exactly 0.3s");
    }

    /// Hysteresis is the whole reason there are two thresholds: a dip that stays
    /// above the release margin is still speech.
    #[test]
    fn a_dip_above_the_release_margin_does_not_open_a_run() {
        let release = 0.5 - super::RELEASE_MARGIN;
        assert!(super::silence_runs(&[0.9, release + 0.01, 0.9], 0.032, 0.5, 0.0).is_empty());
        assert_eq!(
            super::silence_runs(&[0.9, release - 0.01, 0.9], 0.032, 0.5, 0.0),
            [(1, 2)]
        );
    }

    /// A threshold of 1 asks for everything to be silence and 0 for none of it.
    /// Neither is useful, but neither may panic or emit a malformed run.
    #[test]
    fn threshold_extremes_stay_well_formed() {
        let probs = [0.0, 0.5, 1.0, 0.2];
        for threshold in [0.0, 1.0] {
            let runs = super::silence_runs(&probs, 0.032, threshold, 0.0);
            assert!(
                runs.iter().all(|(s, e)| e > s && *e <= probs.len()),
                "threshold {threshold} gave {runs:?}"
            );
        }
    }

    // --- options ---

    #[test]
    fn absent_options_fall_back_to_the_documented_defaults() {
        let opts: super::SilenceOptions = serde_json::from_str("{}").expect("empty object");
        assert_eq!(opts.threshold, super::d_threshold());
        assert_eq!(opts.min_audio_silence, super::d_min_audio_silence());
        assert_eq!(opts.min_segment, super::d_min_segment());
    }

    #[test]
    fn a_partial_options_object_keeps_the_defaults_it_omitted() {
        let json = "{\"threshold\": 0.8}";
        let opts: super::SilenceOptions = serde_json::from_str(json).expect("partial object");
        assert_eq!(opts.threshold, 0.8);
        assert_eq!(opts.min_segment, super::d_min_segment());
    }

    /// A non-finite threshold made every comparison false, which reads a whole
    /// recording as silent rather than failing.
    #[test]
    fn sanitising_replaces_a_threshold_that_is_not_a_number() {
        let opts = super::SilenceOptions {
            threshold: f32::NAN,
            min_audio_silence: f64::NAN,
            min_segment: f64::INFINITY,
        }
        .sanitised();
        assert_eq!(opts.threshold, super::d_threshold());
        assert_eq!(opts.min_audio_silence, super::d_min_audio_silence());
        assert_eq!(opts.min_segment, super::d_min_segment());
    }

    #[test]
    fn sanitising_pulls_out_of_range_values_into_range() {
        let opts = super::SilenceOptions {
            threshold: 4.0,
            min_audio_silence: -1.0,
            min_segment: -0.5,
        }
        .sanitised();
        assert_eq!(opts.threshold, 1.0);
        assert_eq!(opts.min_audio_silence, 0.0);
        assert_eq!(opts.min_segment, 0.0);
    }

    /// Two different sensitivities must not read each other's cached answer.
    #[test]
    fn a_different_sensitivity_is_a_different_cache_key() {
        let base = super::SilenceOptions::default();
        let keener = super::SilenceOptions {
            threshold: 0.8,
            ..base
        };
        let longer = super::SilenceOptions {
            min_segment: base.min_segment + 1.0,
            ..base
        };
        assert_ne!(super::opts_key(&base), super::opts_key(&keener));
        assert_ne!(super::opts_key(&base), super::opts_key(&longer));
        assert_eq!(super::opts_key(&base), super::opts_key(&base));
    }

    // --- segments_from ---

    fn no_cursor() -> super::CursorEvidence {
        super::CursorEvidence {
            idle: Vec::new(),
            present: false,
            mic: true,
            system: false,
        }
    }

    /// One frame is 32ms, so these lengths are in frames of that.
    fn gap(frames: usize) -> Vec<f32> {
        let mut probs = vec![0.9];
        probs.extend(std::iter::repeat_n(0.01, frames));
        probs.push(0.9);
        probs
    }

    #[test]
    fn a_run_shorter_than_the_minimum_segment_is_not_offered() {
        let opts = super::SilenceOptions {
            threshold: 0.5,
            min_audio_silence: 0.0,
            min_segment: 1.0,
        };
        let probs = gap(5); // 0.16s, well under the 1s minimum
        assert!(super::segments_from(&probs, 10.0, &opts, &no_cursor()).is_empty());
    }

    #[test]
    fn a_run_past_the_minimum_segment_is_offered_with_its_bounds() {
        let opts = super::SilenceOptions {
            threshold: 0.5,
            min_audio_silence: 0.0,
            min_segment: 0.1,
        };
        let out = super::segments_from(&gap(10), 10.0, &opts, &no_cursor());
        assert_eq!(out.len(), 1);
        assert!(out[0].end > out[0].start, "empty segment: {:?}", out[0]);
        assert!(out[0].start > 0.0, "the run started at the first frame");
    }

    /// The options reach the analysis through this function, so a threshold that
    /// is not a number must not read the whole recording as silent here either.
    #[test]
    fn options_are_sanitised_before_they_reach_the_analysis() {
        let broken = super::SilenceOptions {
            threshold: f32::NAN,
            min_audio_silence: -1.0,
            min_segment: f64::NAN,
        };
        // Speech at both ends, so an unsanitised NaN threshold (never speaking, since every comparison is false) shows up as one run over the whole thing.
        let out = super::segments_from(&gap(50), 1.66, &broken, &no_cursor());
        assert!(
            out.iter().all(|s| s.end > s.start),
            "malformed segment from broken options: {out:?}"
        );
        assert_eq!(out.len(), 1, "the gap should be the one candidate");
        assert!(
            out[0].start > 0.0,
            "a NaN threshold swallowed the leading speech: {:?}",
            out[0]
        );
    }

    #[test]
    fn a_segment_never_runs_past_the_end_of_the_recording() {
        let opts = super::SilenceOptions {
            threshold: 0.5,
            min_audio_silence: 0.0,
            min_segment: 0.0,
        };
        // The last frame is zero-padded, so the frame grid outruns the audio.
        let total = 0.1;
        for seg in super::segments_from(&[0.01; 20], total, &opts, &no_cursor()) {
            assert!(seg.end <= total + 1e-9, "{seg:?} runs past {total}s");
        }
    }

    /// The cursor is confirmation, not a gate: a candidate stands without it,
    /// and only gets the flag once the pointer sat still through enough of it.
    #[test]
    fn the_cursor_flag_needs_the_pointer_idle_through_enough_of_the_run() {
        let opts = super::SilenceOptions {
            threshold: 0.5,
            min_audio_silence: 0.0,
            min_segment: 0.0,
        };
        let probs = [0.01; 40]; // 40 frames = 1.28s
        let barely = super::CursorEvidence {
            idle: vec![(0.0, 0.2)],
            present: true,
            mic: true,
            system: false,
        };
        let mostly = super::CursorEvidence {
            idle: vec![(0.0, 1.28)],
            present: true,
            mic: true,
            system: false,
        };
        let a = super::segments_from(&probs, 1.28, &opts, &barely);
        let b = super::segments_from(&probs, 1.28, &opts, &mostly);
        assert_eq!(a.len(), 1, "the candidate needs the cursor to stand");
        assert!(!a[0].cursor_idle, "0.2s of 1.28s should not confirm");
        assert!(b[0].cursor_idle, "a fully idle pointer should confirm");
        assert!(
            b[0].confidence > a[0].confidence,
            "confirmation did not raise confidence"
        );
    }

    #[test]
    fn which_tracks_were_present_is_reported_on_every_segment() {
        let opts = super::SilenceOptions {
            threshold: 0.5,
            min_audio_silence: 0.0,
            min_segment: 0.0,
        };
        let both = super::CursorEvidence {
            idle: Vec::new(),
            present: false,
            mic: true,
            system: true,
        };
        let out = super::segments_from(&[0.01; 20], 1.0, &opts, &both);
        assert!(out.iter().all(|s| s.mic_silent && s.system_silent));
    }

    // --- interval algebra ---

    #[test]
    fn intervals_that_only_touch_do_not_intersect() {
        assert!(super::intersect(&[(0.0, 1.0)], &[(1.0, 2.0)]).is_empty());
    }

    #[test]
    fn intersecting_with_nothing_covers_nothing() {
        assert!(super::intersect(&[(0.0, 1.0)], &[]).is_empty());
        assert!(super::intersect(&[], &[(0.0, 1.0)]).is_empty());
        assert_eq!(super::overlap_fraction((0.0, 1.0), &[]), 0.0);
    }

    #[test]
    fn a_segment_with_no_duration_is_covered_by_nothing() {
        assert_eq!(super::overlap_fraction((2.0, 2.0), &[(0.0, 9.0)]), 0.0);
        assert_eq!(super::overlap_fraction((3.0, 1.0), &[(0.0, 9.0)]), 0.0);
    }

    // --- confidence ---

    #[test]
    fn confidence_stays_inside_zero_and_one_at_the_extremes() {
        for (len, speech, idle, cursor) in [
            (0.0, 0.0, 0.0, false),
            (1e9, 0.0, 1.0, true),
            (1e9, 1.0, 1.0, true),
            (-1.0, 2.0, 5.0, true),
            // No cursor term to carry it back up, so only the clamp keeps this off a negative confidence.
            (-10.0, 1.0, 0.0, false),
        ] {
            let c = super::score(len, speech, idle, cursor);
            assert!((0.0..=1.0).contains(&c), "score out of range: {c}");
        }
    }

    /// The length term saturates at four seconds. Without that a long pause
    /// swamps the audio evidence, and the outer clamp hides it by reading 1.0.
    #[test]
    fn past_four_seconds_a_longer_run_stops_adding_confidence() {
        assert_eq!(
            super::score(4.0, 1.0, 0.0, false),
            super::score(400.0, 1.0, 0.0, false)
        );
    }

    #[test]
    fn a_longer_and_quieter_run_is_never_less_confident() {
        let short = super::score(0.5, 0.4, 0.0, false);
        assert!(super::score(4.0, 0.4, 0.0, false) > short, "length ignored");
        assert!(super::score(0.5, 0.1, 0.0, false) > short, "quiet ignored");
    }

    /// Without a track there is nothing to confirm, so the cursor term is absent
    /// rather than assumed idle.
    #[test]
    fn the_cursor_only_adds_confidence_when_a_track_was_present() {
        assert_eq!(
            super::score(2.0, 0.2, 1.0, false),
            super::score(2.0, 0.2, 0.0, false)
        );
        assert!(super::score(2.0, 0.2, 1.0, true) > super::score(2.0, 0.2, 0.0, true));
    }

    // --- model ---

    /// The same window and state must score the same every time, or a cached
    /// result and a fresh one disagree about the same recording.
    #[test]
    fn the_model_is_deterministic_for_one_window_and_state() {
        let mut window = [0f32; super::CHUNK];
        for (i, s) in window.iter_mut().enumerate() {
            *s = 0.25 * (i as f32 * 0.11).sin();
        }
        let run = || {
            let mut vad = super::SileroVad::new().expect("init");
            vad.compute(&window).expect("compute")
        };
        assert_eq!(run(), run());
    }

    /// A regenerated model that takes different inputs would still load, and
    /// this is the only place that would notice.
    #[test]
    fn the_bundled_model_keeps_the_signature_the_code_feeds_it() {
        use tract_onnx::prelude::Framework;
        let model = tract_onnx::onnx()
            .model_for_read(&mut std::io::Cursor::new(super::MODEL))
            .expect("the bundled graph parses");
        let names: Vec<String> = model
            .input_outlets()
            .expect("inputs")
            .iter()
            .map(|o| model.node(o.node).name.clone())
            .collect();
        assert_eq!(names, ["input", "state"], "input signature changed");
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
        let mut vad = super::SileroVad::new().expect("init Silero VAD");
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

    /// The published model branches on sample rate with an ONNX `If` that tract
    /// refuses, so the graph is pre-folded. A regenerated model that reintroduces
    /// one loads nowhere, and this is the only place that would say so.
    #[test]
    fn the_bundled_model_carries_no_branch_tract_cannot_read() {
        let mut vad = super::SileroVad::new().expect("the bundled graph loads");
        let mut louder = [0f32; super::CHUNK];
        for (i, s) in louder.iter_mut().enumerate() {
            *s = 0.4 * (i as f32 * 0.05).sin();
        }
        let quiet = vad.compute(&[0f32; super::CHUNK]).expect("silence");
        let tone = vad.compute(&louder).expect("tone");
        assert!(
            (0.0..=1.0).contains(&quiet),
            "silence out of range: {quiet}"
        );
        assert!((0.0..=1.0).contains(&tone), "tone out of range: {tone}");
    }

    /// State has to advance: a plan that returned the input state unchanged would
    /// score every window as if it were the first and lose the LSTM entirely.
    #[test]
    fn the_recurrent_state_advances_between_windows() {
        let mut vad = super::SileroVad::new().expect("init Silero VAD");
        let mut speechish = [0f32; super::CHUNK];
        for (i, s) in speechish.iter_mut().enumerate() {
            *s = 0.5 * (i as f32 * 0.31).sin() * (i as f32 * 0.017).cos();
        }
        vad.compute(&speechish).expect("first window");
        let after_one = vad.state.clone();
        vad.compute(&speechish).expect("second window");
        assert_ne!(
            after_one.as_slice::<f32>().expect("f32 state"),
            vad.state.as_slice::<f32>().expect("f32 state"),
            "the LSTM state never moved"
        );
        vad.reset().expect("reset");
        assert!(
            vad.state
                .as_slice::<f32>()
                .expect("f32 state")
                .iter()
                .all(|v| *v == 0.0),
            "reset left state behind"
        );
    }
}
