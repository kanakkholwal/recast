//! Silence detection for the editor timeline.
//!
//! Finds time ranges where the recording has *nothing meaningful happening*:
//! no microphone speech, no system audio, and no visible screen motion. The
//! camera feed is deliberately ignored — a talking head on screen does not
//! make the screen "active".
//!
//! Detection leans on FFmpeg's battle-tested filters rather than hand-rolled
//! DSP:
//!   - `silencedetect` over each audio track (mic + system) → silent runs.
//!   - `freezedetect` over the screen video → frozen (static) runs.
//!
//! A candidate silence segment is the **interval intersection** of all three
//! signals — silent on mic AND silent on system audio AND frozen on screen.
//! Requiring agreement across every signal is what keeps quiet-but-meaningful
//! moments (narration over a static slide; a silent screen recording of a
//! video playing) from being flagged.
//!
//! Everything streams through FFmpeg one decode pass at a time, so memory and
//! compute stay flat regardless of recording length.

use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

/// Tunable detection thresholds. Every field has a sensible default so the
/// frontend may send a partial object (or nothing at all).
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SilenceOptions {
    /// Audio level (dBFS) at or below which audio counts as silent.
    /// Higher (less negative) = more aggressive. -30 dB ≈ "quiet room".
    #[serde(default = "d_noise_db")]
    pub noise_db: f64,
    /// Minimum continuous silent-audio run for `silencedetect` to register.
    #[serde(default = "d_min_audio_silence")]
    pub min_audio_silence: f64,
    /// `freezedetect` noise tolerance (dB) — how much frame-to-frame change
    /// still counts as "frozen". Compression noise and a blinking caret sit
    /// well under -45 dB.
    #[serde(default = "d_freeze_noise_db")]
    pub freeze_noise_db: f64,
    /// Minimum continuous frozen-video run for `freezedetect` to register.
    #[serde(default = "d_min_freeze")]
    pub min_freeze: f64,
    /// Minimum length of a returned silence segment. Shorter dead air is not
    /// worth a cut and tends to read as a stutter.
    #[serde(default = "d_min_segment")]
    pub min_segment: f64,
    /// Adjacent candidate segments closer than this merge into one — a tiny
    /// blip between two long silences should not split the cut.
    #[serde(default = "d_merge_gap")]
    pub merge_gap: f64,
}

fn d_noise_db() -> f64 {
    -30.0
}
fn d_min_audio_silence() -> f64 {
    0.6
}
fn d_freeze_noise_db() -> f64 {
    -45.0
}
fn d_min_freeze() -> f64 {
    0.5
}
fn d_min_segment() -> f64 {
    1.0
}
fn d_merge_gap() -> f64 {
    0.4
}

impl Default for SilenceOptions {
    fn default() -> Self {
        Self {
            noise_db: d_noise_db(),
            min_audio_silence: d_min_audio_silence(),
            freeze_noise_db: d_freeze_noise_db(),
            min_freeze: d_min_freeze(),
            min_segment: d_min_segment(),
            merge_gap: d_merge_gap(),
        }
    }
}

/// A detected silence range, in original-recording seconds.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilenceSegment {
    pub start: f64,
    pub end: f64,
    /// 0..1 — how strongly this range warrants a cut. Drives which
    /// suggestions are pre-selected vs. shown as "uncertain" in the UI.
    pub confidence: f32,
    /// Microphone track was present and confirmed silent over this range.
    pub mic_silent: bool,
    /// System-audio track was present and confirmed silent over this range.
    pub system_silent: bool,
    /// Screen video was confirmed frozen over this range.
    pub screen_static: bool,
}

type Interval = (f64, f64);

/// Analyse a recording and return candidate silence segments.
///
/// `video_path` is required (the screen-motion signal). `audio_path` /
/// `microphone_path` are optional — a missing track is treated as silent
/// everywhere, so a screen-only recording still produces useful results from
/// the freeze signal alone (with a confidence penalty applied).
#[tauri::command]
pub async fn detect_silence(
    video_path: String,
    audio_path: Option<String>,
    microphone_path: Option<String>,
    options: Option<SilenceOptions>,
) -> Result<Vec<SilenceSegment>, String> {
    let opts = options.unwrap_or_default();
    tokio::task::spawn_blocking(move || {
        detect_blocking(&video_path, audio_path.as_deref(), microphone_path.as_deref(), opts)
    })
    .await
    .map_err(|e| format!("silence-detection task panicked: {e}"))?
}

fn detect_blocking(
    video_path: &str,
    audio_path: Option<&str>,
    microphone_path: Option<&str>,
    opts: SilenceOptions,
) -> Result<Vec<SilenceSegment>, String> {
    // Screen-motion pass. This stderr also carries the input `Duration:` line,
    // which we use to close any silence/freeze run that lasts to EOF.
    let video_stderr = ffmpeg_stderr(&[
        "-hide_banner".into(),
        "-nostats".into(),
        "-i".into(),
        video_path.into(),
        "-vf".into(),
        format!(
            "freezedetect=n={:.1}dB:d={:.3}",
            opts.freeze_noise_db, opts.min_freeze
        ),
        "-an".into(),
        "-f".into(),
        "null".into(),
        "-".into(),
    ])?;
    let total = parse_duration(&video_stderr).unwrap_or(0.0);
    if total <= 0.0 {
        return Err("could not determine recording duration".into());
    }
    let freeze = parse_freeze_intervals(&video_stderr, total);

    // Audio passes. A missing track is "silent everywhere".
    let mic = match microphone_path {
        Some(p) if Path::new(p).exists() => Some(detect_audio_silence(p, &opts)?),
        _ => None,
    };
    let system = match audio_path {
        Some(p) if Path::new(p).exists() => Some(detect_audio_silence(p, &opts)?),
        _ => None,
    };
    let has_audio = mic.is_some() || system.is_some();

    let full = vec![(0.0, total)];
    let mic_iv = mic.clone().unwrap_or_else(|| full.clone());
    let system_iv = system.clone().unwrap_or_else(|| full.clone());

    // Silence = silent on mic AND on system audio AND frozen on screen.
    let mut candidate = intersect(&intersect(&mic_iv, &system_iv), &freeze);
    candidate = merge_close(candidate, opts.merge_gap);
    candidate.retain(|iv| iv.1 - iv.0 >= opts.min_segment);

    let segments = candidate
        .into_iter()
        .map(|seg| SilenceSegment {
            start: round3(seg.0),
            end: round3(seg.1),
            confidence: score(seg, &mic_iv, &system_iv, &freeze, has_audio),
            mic_silent: mic.is_some(),
            system_silent: system.is_some(),
            screen_static: true,
        })
        .collect();
    Ok(segments)
}

fn detect_audio_silence(path: &str, opts: &SilenceOptions) -> Result<Vec<Interval>, String> {
    let stderr = ffmpeg_stderr(&[
        "-hide_banner".into(),
        "-nostats".into(),
        "-i".into(),
        path.into(),
        "-af".into(),
        format!(
            "silencedetect=noise={:.1}dB:d={:.3}",
            opts.noise_db, opts.min_audio_silence
        ),
        "-vn".into(),
        "-f".into(),
        "null".into(),
        "-".into(),
    ])?;
    let total = parse_duration(&stderr).unwrap_or(f64::MAX);
    Ok(parse_silence_intervals(&stderr, total))
}

/// Spawn ffmpeg, discard stdout, return the full stderr text. ffmpeg writes
/// `silencedetect` / `freezedetect` results and the input banner to stderr.
fn ffmpeg_stderr(args: &[String]) -> Result<String, String> {
    let mut cmd = Command::new(crate::ffmpeg::ffmpeg_path());
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    crate::ffmpeg::configure_silent_command(&mut cmd);
    let output = cmd
        .output()
        .map_err(|e| format!("failed to run ffmpeg for analysis: {e}"))?;
    Ok(String::from_utf8_lossy(&output.stderr).into_owned())
}

//  stderr parsing

/// Pull the value after a `key:` token on a log line, parsed as seconds.
fn value_after(line: &str, key: &str) -> Option<f64> {
    let idx = line.find(key)? + key.len();
    line[idx..]
        .split_whitespace()
        .next()
        .and_then(|t| t.trim_end_matches([',', '|']).parse::<f64>().ok())
}

/// Parse `Duration: HH:MM:SS.ss` from ffmpeg's input banner.
fn parse_duration(stderr: &str) -> Option<f64> {
    for line in stderr.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("Duration:") {
            let token = rest.trim().split(',').next()?.trim();
            let mut parts = token.split(':');
            let h: f64 = parts.next()?.parse().ok()?;
            let m: f64 = parts.next()?.parse().ok()?;
            let s: f64 = parts.next()?.parse().ok()?;
            return Some(h * 3600.0 + m * 60.0 + s);
        }
    }
    None
}

/// Parse `silence_start` / `silence_end` pairs. A `silence_start` with no
/// matching `silence_end` ran to EOF and is closed at `total`.
fn parse_silence_intervals(stderr: &str, total: f64) -> Vec<Interval> {
    let mut out = Vec::new();
    let mut open: Option<f64> = None;
    for line in stderr.lines() {
        if !line.contains("silencedetect") {
            continue;
        }
        if let Some(start) = value_after(line, "silence_start:") {
            open = Some(start.max(0.0));
        } else if let Some(end) = value_after(line, "silence_end:") {
            if let Some(start) = open.take() {
                push_interval(&mut out, start, end);
            }
        }
    }
    if let Some(start) = open {
        push_interval(&mut out, start, total);
    }
    out
}

/// Parse `freeze_start` / `freeze_end` pairs from `freezedetect` metadata logs.
fn parse_freeze_intervals(stderr: &str, total: f64) -> Vec<Interval> {
    let mut out = Vec::new();
    let mut open: Option<f64> = None;
    for line in stderr.lines() {
        if !line.contains("freezedetect") {
            continue;
        }
        if let Some(start) = value_after(line, "freeze_start:") {
            open = Some(start.max(0.0));
        } else if let Some(end) = value_after(line, "freeze_end:") {
            if let Some(start) = open.take() {
                push_interval(&mut out, start, end);
            }
        }
    }
    if let Some(start) = open {
        push_interval(&mut out, start, total);
    }
    out
}

fn push_interval(out: &mut Vec<Interval>, start: f64, end: f64) {
    if end > start {
        out.push((start, end));
    }
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

/// Merge intervals separated by a gap smaller than `gap`.
fn merge_close(mut intervals: Vec<Interval>, gap: f64) -> Vec<Interval> {
    if intervals.is_empty() {
        return intervals;
    }
    intervals.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<Interval> = Vec::with_capacity(intervals.len());
    for iv in intervals {
        match out.last_mut() {
            Some(last) if iv.0 - last.1 <= gap => last.1 = last.1.max(iv.1),
            _ => out.push(iv),
        }
    }
    out
}

//  Confidence

/// Smallest distance from a segment's edges to the edges of whichever signal
/// interval contains it — a large margin means the segment sits comfortably
/// inside a solidly silent run rather than on a knife-edge.
fn containing_margin(seg: Interval, intervals: &[Interval]) -> f64 {
    intervals
        .iter()
        .filter(|iv| iv.0 <= seg.0 + 1e-6 && iv.1 + 1e-6 >= seg.1)
        .map(|iv| (seg.0 - iv.0).min(iv.1 - seg.1))
        .fold(0.0_f64, f64::max)
}

fn score(
    seg: Interval,
    mic: &[Interval],
    system: &[Interval],
    freeze: &[Interval],
    has_audio: bool,
) -> f32 {
    let len = seg.1 - seg.0;
    // Longer dead air is both more obviously cuttable and lower-risk.
    let len_score = (len / 4.0).min(1.0);
    // Robustness: how far each signal extends past the segment on its
    // tightest side. Near-threshold flecks score low.
    let margin = containing_margin(seg, mic)
        .min(containing_margin(seg, system))
        .min(containing_margin(seg, freeze));
    let margin_score = (margin / 1.0).min(1.0);
    let mut c = 0.45 + 0.35 * len_score + 0.20 * margin_score;
    // No audio track at all means we are leaning entirely on screen-freeze —
    // weaker evidence, so never let these read as high-confidence.
    if !has_audio {
        c -= 0.25;
    }
    c.clamp(0.0, 1.0) as f32
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}
