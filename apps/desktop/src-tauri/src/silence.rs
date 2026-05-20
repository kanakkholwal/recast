//! Silence detection for the editor timeline.
//!
//! A range counts as silence when **both** of the following hold for at least
//! `min_segment` seconds:
//!
//!   1. The audio envelope is *flat and minimal* — frames sit within
//!      `flatness_db` of the recording's own estimated noise floor. This is
//!      explicitly relative, not an absolute dB threshold: a quiet hum in a
//!      noisy room reads as "background" because the floor itself adapts.
//!   2. The mouse cursor is *idle* — the cursor track shows no meaningful
//!      movement over the same range.
//!
//! Background-noise suppression is intentionally out of scope here. So is
//! pixel-level screen-motion detection (blinking carets and ticking clocks
//! made `freezedetect` too noisy to be useful as a hard gate).

use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

//  Options / output

/// Detection thresholds. Every field has a default so the frontend may send
/// a partial object — or nothing at all.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SilenceOptions {
    /// Frames must sit within this many dB of the recording's noise floor
    /// to count as background. Smaller = stricter (only very flat
    /// stretches register); larger = more aggressive.
    #[serde(default = "d_flatness_db")]
    pub flatness_db: f64,
    /// Minimum continuous flat-audio run for a candidate (seconds).
    #[serde(default = "d_min_audio_silence")]
    pub min_audio_silence: f64,
    /// Minimum length of a returned silence segment (seconds).
    #[serde(default = "d_min_segment")]
    pub min_segment: f64,
}

fn d_flatness_db() -> f64 {
    5.0
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
            flatness_db: d_flatness_db(),
            min_audio_silence: d_min_audio_silence(),
            min_segment: d_min_segment(),
        }
    }
}

/// A detected silence range, in original-recording seconds.
#[derive(Debug, Clone, Serialize)]
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

// 8 kHz mono is plenty for envelope detection — speech and dynamics live well
// below half that.
const RATE: u32 = 8000;
const FRAME_MS: f64 = 50.0;
/// Cursor counts as idle once it stays within this radius for this long.
const CURSOR_IDLE_MIN_US: u64 = 300_000;
const CURSOR_IDLE_RADIUS_PX: f64 = 8.0;
/// Where the noise floor sits in the frame-energy distribution. The 5th
/// percentile is conservative: it remains inside the genuine background
/// even on a recording that is mostly speech (the 20th percentile crept
/// up into quiet-syllable territory and made `audio_flat` swallow speech).
const PERCENTILE_FLOOR: f64 = 0.05;
/// Hard ceiling — a frame above this is never "silent" regardless of how
/// close it sits to the estimated floor. Guards against pathological
/// percentile bias on very noisy recordings.
const ABS_QUIET_DBFS: f64 = -28.0;

//  Command

#[tauri::command]
pub async fn detect_silence(
    audio_path: Option<String>,
    microphone_path: Option<String>,
    cursor_path: Option<String>,
    options: Option<SilenceOptions>,
) -> Result<Vec<SilenceSegment>, String> {
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
    .map_err(|e| format!("silence-detection task panicked: {e}"))?
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
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    if samples.len() < 2 {
        return Ok(Vec::new());
    }
    let total = samples.len() as f64 / RATE as f64;

    // Per-frame RMS in dBFS.
    let frame_size = (RATE as f64 * FRAME_MS / 1000.0).round() as usize;
    let frame_dur = FRAME_MS / 1000.0;
    let frame_db: Vec<f64> = samples.chunks(frame_size).map(frame_rms_db).collect();

    // Audio-flat runs: stretches whose envelope stays within `flatness_db`
    // of the recording's own noise floor *and* below an absolute quiet
    // ceiling. We deliberately do NOT merge_close these — bridging a short
    // gap would silently swallow a speech burst between two flat runs.
    let audio_flat = flat_intervals(
        &frame_db,
        frame_dur,
        opts.min_audio_silence,
        opts.flatness_db,
    );

    // Cursor-idle intervals. A missing track is treated as "idle everywhere"
    // so the feature still works without cursor data, but the segment's
    // `cursor_idle` flag and score reflect the unverified state.
    let (cursor_idle, has_cursor) = match cursor_path {
        Some(p) if Path::new(p).exists() => {
            let bytes = std::fs::read(Path::new(p))
                .map_err(|e| format!("read cursor track: {e}"))?;
            let track: crate::cursor::CursorTrack = serde_json::from_slice(&bytes)
                .map_err(|e| format!("parse cursor track: {e}"))?;
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
        _ => (vec![(0.0, total)], false),
    };

    // Both constraints must hold — the user's spec, intentionally strict.
    // No `merge_close` here either: a gap between two intersected ranges
    // means at least one of {audio quiet, cursor still} *failed* in that
    // gap, and bridging would mark a region as silent that isn't.
    let mut candidates = intersect(&audio_flat, &cursor_idle);
    candidates.retain(|iv| iv.1 - iv.0 >= opts.min_segment);

    Ok(candidates
        .into_iter()
        .map(|seg| SilenceSegment {
            start: round3(seg.0),
            end: round3(seg.1),
            confidence: score(seg, has_cursor),
            mic_silent: microphone_path
                .map(|p| Path::new(p).exists())
                .unwrap_or(false),
            system_silent: audio_path.map(|p| Path::new(p).exists()).unwrap_or(false),
            cursor_idle: has_cursor,
        })
        .collect())
}

//  Audio analysis

fn frame_rms_db(chunk: &[i16]) -> f64 {
    if chunk.is_empty() {
        return -120.0;
    }
    let sum_sq: f64 = chunk
        .iter()
        .map(|s| {
            let v = *s as f64;
            v * v
        })
        .sum();
    let rms = (sum_sq / chunk.len() as f64).sqrt();
    if rms > 1e-6 {
        20.0 * (rms / 32768.0).log10()
    } else {
        -120.0
    }
}

/// Find runs of frames whose energy sits within `tol_db` of the recording's
/// estimated noise floor *and* below an absolute quiet ceiling. Returns
/// intervals in seconds.
///
/// The dual gate matters on recordings that are mostly speech: the
/// percentile-based floor drifts upward into quiet-syllable territory there,
/// and a relative-only check would happily mark speech as silence. The hard
/// `ABS_QUIET_DBFS` ceiling rejects any frame that simply isn't quiet.
fn flat_intervals(
    frame_db: &[f64],
    frame_dur: f64,
    min_dur: f64,
    tol_db: f64,
) -> Vec<Interval> {
    if frame_db.is_empty() {
        return Vec::new();
    }
    let mut finite: Vec<f64> = frame_db.iter().copied().filter(|v| v.is_finite()).collect();
    if finite.is_empty() {
        return Vec::new();
    }
    finite.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((finite.len() as f64) * PERCENTILE_FLOOR).floor() as usize;
    let floor = finite[idx.min(finite.len() - 1)];
    let cap = floor + tol_db;

    // Per-frame bg decision: close to the noise floor AND below the absolute
    // quiet ceiling. Both must hold.
    let raw_mask: Vec<bool> = frame_db
        .iter()
        .map(|v| *v <= cap && *v <= ABS_QUIET_DBFS)
        .collect();

    // 3-frame majority smoothing — kills single-frame jitter (an isolated
    // envelope spike inside a silent stretch, or a one-frame bg fleck inside
    // speech) without bridging a real burst, which is always longer than the
    // 100 ms smoothing window.
    let mask: Vec<bool> = (0..raw_mask.len())
        .map(|i| {
            let a = if i > 0 { raw_mask[i - 1] } else { false };
            let b = raw_mask[i];
            let c = if i + 1 < raw_mask.len() { raw_mask[i + 1] } else { false };
            (a as u8 + b as u8 + c as u8) >= 2
        })
        .collect();

    let mut out = Vec::new();
    let mut run_start: Option<usize> = None;
    let n = mask.len();
    for i in 0..n {
        match (mask[i], run_start) {
            (true, None) => run_start = Some(i),
            (false, Some(s)) => {
                let st = s as f64 * frame_dur;
                let en = i as f64 * frame_dur;
                if en - st >= min_dur {
                    out.push((st, en));
                }
                run_start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = run_start {
        let st = s as f64 * frame_dur;
        let en = n as f64 * frame_dur;
        if en - st >= min_dur {
            out.push((st, en));
        }
    }
    out
}

//  Interval algebra

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

fn score(seg: Interval, has_cursor: bool) -> f32 {
    let len = seg.1 - seg.0;
    let len_score = (len / 4.0).min(1.0);
    let mut c = 0.45 + 0.40 * len_score;
    // Verified-idle cursor is a real second-source confirmation; an
    // unverified cursor (track missing) gets none.
    if has_cursor {
        c += 0.15;
    }
    c.clamp(0.0, 1.0) as f32
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
) -> Result<Vec<f32>, String> {
    let buckets = buckets.unwrap_or(2000).clamp(64, 8000);
    tokio::task::spawn_blocking(move || {
        waveform_blocking(
            audio_path.as_deref(),
            microphone_path.as_deref(),
            buckets,
        )
    })
    .await
    .map_err(|e| format!("waveform task panicked: {e}"))?
}

fn waveform_blocking(
    audio_path: Option<&str>,
    microphone_path: Option<&str>,
    buckets: usize,
) -> Result<Vec<f32>, String> {
    // Visual fidelity only — 4 kHz mono is plenty for an envelope and keeps
    // even hour-long recordings to a bounded buffer.
    const WAVE_RATE: u32 = 4000;

    let inputs: Vec<&str> = [audio_path, microphone_path]
        .into_iter()
        .flatten()
        .filter(|p| Path::new(p).exists())
        .collect();
    if inputs.is_empty() {
        return Ok(Vec::new());
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
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
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
    Ok(out)
}
