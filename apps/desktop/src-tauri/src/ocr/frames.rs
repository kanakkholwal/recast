//! One coarse decode pass keeping only frames where the screen changed, via a dHash gate plus an adaptive change score.
//! Frames stay raw RGBA: re-encoding to JPEG before OCR only adds artifacts that hurt small text.

use std::collections::VecDeque;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

use image::imageops::FilterType;
use image::{DynamicImage, RgbImage, RgbaImage};

use crate::ffmpeg::{configure_silent_command, ffmpeg_path, ffprobe_path};

/// One kept frame: its timestamp on the video clock and its raw RGBA pixels.
pub struct SampledFrame {
    pub t_secs: f64,
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Progress of the coarse decode pass, reported once per walked frame.
#[derive(Debug, Clone, Copy)]
pub struct SampleTick {
    /// Coarse frames walked so far.
    pub scanned: u64,
    /// Coarse frames the pass expects to walk, from duration x `base_fps`. An
    /// estimate: a container with a wrong duration can over- or undershoot, so a
    /// consumer must clamp rather than trust `scanned <= total`.
    pub total: u64,
    /// Frames kept for OCR so far.
    pub kept: u64,
}

/// Tuning for the sampler. Defaults are chosen for screen recordings.
#[derive(Debug, Clone)]
pub struct SampleOpts {
    /// Coarse decode rate. We never look at more than this many frames a second.
    pub base_fps: f32,
    /// Keep a frame when its change score exceeds this multiple of the recent
    /// average (survives smooth scroll/motion a fixed threshold over-triggers on).
    pub adaptive_ratio: f32,
    /// Drop a frame whose dHash is within this Hamming distance of the last kept
    /// frame (near-duplicate gate).
    pub dedup_hamming: u32,
    /// Never keep two frames closer than this (anti-spam), unless forced.
    pub min_gap_secs: f64,
    /// Always keep a frame if this long has passed since the last keep (coverage).
    pub max_gap_secs: f64,
    /// Cap the frame's long edge to this many pixels (0 = native).
    pub max_dim: u32,
    /// Timestamps (seconds) that must be sampled regardless of change score, e.g.
    /// cursor clicks. Empty in slice 1 (cursor enrichment is a later layer).
    pub forced_timestamps: Vec<f64>,
    /// Source-time ranges (seconds) that survive the current edit: the kept
    /// segments after trim and cuts. Frames outside them are footage the user
    /// removed, so reading them would produce spans for content that is not in the
    /// video and would waste an OCR pass (~390ms) on each. EMPTY means the whole
    /// file, which is what a headless/CLI caller with no edit context wants.
    pub include_ranges: Vec<(f64, f64)>,
}

impl Default for SampleOpts {
    fn default() -> Self {
        Self {
            base_fps: 3.0,
            adaptive_ratio: 3.0,
            dedup_hamming: 6,
            min_gap_secs: 0.4,
            max_gap_secs: 8.0,
            max_dim: 1600,
            forced_timestamps: Vec::new(),
            include_ranges: Vec::new(),
        }
    }
}

/// Source video dimensions and duration, via ffprobe.
pub fn probe_dims(media: &Path) -> Result<(u32, u32, f64), String> {
    let mut cmd = Command::new(ffprobe_path());
    cmd.args([
        "-v",
        "error",
        "-select_streams",
        "v:0",
        "-show_entries",
        "stream=width,height:format=duration",
        "-of",
        "json",
    ]);
    cmd.arg(media);
    configure_silent_command(&mut cmd);
    let out = cmd.output().map_err(|e| format!("ffprobe spawn: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("ffprobe json: {e}"))?;
    let stream = v
        .get("streams")
        .and_then(|s| s.get(0))
        .ok_or("ffprobe: no video stream")?;
    let width = stream
        .get("width")
        .and_then(|w| w.as_u64())
        .ok_or("ffprobe: no width")? as u32;
    let height = stream
        .get("height")
        .and_then(|h| h.as_u64())
        .ok_or("ffprobe: no height")? as u32;
    let duration = v
        .get("format")
        .and_then(|f| f.get("duration"))
        .and_then(|d| d.as_str())
        .and_then(|d| d.parse::<f64>().ok())
        .unwrap_or(0.0);
    Ok((width, height, duration))
}

/// Scale `(w, h)` so the long edge is at most `max_dim`, preserving aspect and rounding to even dimensions. `max_dim == 0` (or an already-small frame) keeps the source size.
fn target_dims(w: u32, h: u32, max_dim: u32) -> (u32, u32) {
    if max_dim == 0 || w.max(h) <= max_dim {
        return (even(w), even(h));
    }
    let scale = max_dim as f64 / w.max(h) as f64;
    let tw = (w as f64 * scale).round() as u32;
    let th = (h as f64 * scale).round() as u32;
    (even(tw.max(2)), even(th.max(2)))
}

fn even(n: u32) -> u32 {
    if n.is_multiple_of(2) {
        n
    } else {
        n + 1
    }
}

/// Sample the frames where the screen changed. One coarse decode pass, gated by
/// dedup + adaptive change score + cadence bounds.
///
/// `on_tick` fires once per walked frame, including the ones skipped as outside
/// the kept ranges, because the decode still pays for them and a progress bar that
/// stalled through a cut would read as a hang.
pub fn sample_frames(
    media: &Path,
    opts: &SampleOpts,
    on_tick: &mut dyn FnMut(SampleTick),
) -> Result<Vec<SampledFrame>, String> {
    let (sw, sh, duration) = probe_dims(media)?;
    let (tw, th) = target_dims(sw, sh, opts.max_dim);
    let expected = expected_frames(duration, opts.base_fps);
    let frame_bytes = (tw as usize) * (th as usize) * 4;
    if frame_bytes == 0 {
        return Err("video has zero-sized frames".into());
    }

    let mut cmd = Command::new(ffmpeg_path());
    cmd.args(["-nostdin", "-loglevel", "error"]);
    cmd.arg("-i").arg(media);
    cmd.args([
        "-vf",
        &format!("fps={},scale={tw}:{th}", opts.base_fps),
        "-pix_fmt",
        "rgba",
        "-f",
        "rawvideo",
        "pipe:1",
    ]);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    configure_silent_command(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| format!("ffmpeg spawn: {e}"))?;

    // Drain stderr on a side thread so a full pipe can't block ffmpeg while we read stdout; bounded against a spew.
    let stderr = child.stderr.take();
    let stderr_handle = stderr.map(|mut s| {
        std::thread::spawn(move || {
            let mut buf = String::new();
            let mut chunk = [0u8; 4096];
            while let Ok(n) = s.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                if buf.len() < 8192 {
                    buf.push_str(&String::from_utf8_lossy(&chunk[..n]));
                }
            }
            buf
        })
    });

    let mut stdout = child.stdout.take().ok_or("ffmpeg produced no stdout")?;
    let sampler = SamplerChild {
        child: Some(child),
        stderr: stderr_handle,
    };
    let mut frame = vec![0u8; frame_bytes];

    // Retained RGBA frames are ~5.8 MB each and were unbounded; at the ceiling we halve holdings and double spacing, keeping whole-video coverage.
    let max_frames = (SAMPLE_BUDGET_BYTES / frame_bytes.max(1)).max(1);
    let mut gap_scale = 1.0f64;

    let mut kept: Vec<SampledFrame> = Vec::new();
    let mut prev_small: Option<RgbImage> = None;
    let mut rolling: VecDeque<f32> = VecDeque::with_capacity(4);
    let mut last_kept_t: Option<f64> = None;
    let mut last_kept_hash: u64 = 0;
    let mut index: u64 = 0;

    // Reads one frame per iteration; ends at a clean EOF on a frame boundary.
    while read_full(&mut stdout, &mut frame).map_err(|e| format!("read frame: {e}"))? {
        let t = index as f64 / opts.base_fps as f64;
        index += 1;
        on_tick(SampleTick {
            scanned: index,
            total: expected,
            kept: kept.len() as u64,
        });

        // Skip footage the edit removed before doing any work: decode is one cheap pass, but OCR is ~390ms a frame.
        if !in_ranges(t, &opts.include_ranges) {
            continue;
        }

        let dyn_img = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(tw, th, frame.clone())
                .ok_or("frame buffer size mismatch during sampling")?,
        );
        let small = dyn_img.resize_exact(32, 32, FilterType::Triangle).to_rgb8();
        let hash = dhash(&dyn_img);

        // Change score vs the previous coarse frame (first frame = infinite).
        let score = match &prev_small {
            Some(p) => color_mad(&small, p),
            None => f32::INFINITY,
        };
        let avg = if rolling.is_empty() {
            0.0
        } else {
            rolling.iter().sum::<f32>() / rolling.len() as f32
        };

        let changed = is_changed(score, avg, opts.adaptive_ratio);
        let forced = is_forced(t, &opts.forced_timestamps, opts.base_fps);
        let dup = last_kept_t.is_some() && hamming(last_kept_hash, hash) <= opts.dedup_hamming;
        let keep = should_keep(t, last_kept_t, dup, forced, changed, opts);

        // Once thinned, honour the widened spacing; forced frames are why the hook exists, so they still win.
        let spaced = last_kept_t.is_none_or(|last| t - last >= opts.min_gap_secs * gap_scale);
        if keep && (forced || spaced) {
            kept.push(SampledFrame {
                t_secs: t,
                rgba: frame.clone(),
                width: tw,
                height: th,
            });
            last_kept_t = Some(t);
            last_kept_hash = hash;

            if kept.len() >= max_frames {
                let mut seen = 0usize;
                kept.retain(|_| {
                    seen += 1;
                    seen.is_multiple_of(2)
                });
                gap_scale *= 2.0;
                log::warn!(
                    "ocr sampling hit its {max_frames}-frame memory budget; \
                     thinning to every {gap_scale}x min-gap ({} retained)",
                    kept.len()
                );
            }
        }

        if score.is_finite() {
            rolling.push_back(score);
            if rolling.len() > 3 {
                rolling.pop_front();
            }
        }
        prev_small = Some(small);
    }

    let (status, stderr_tail) = sampler.finish()?;
    if !status.success() {
        return Err(format!("ffmpeg sampling failed: {stderr_tail}"));
    }

    Ok(kept)
}

/// How many coarse frames a decode at `base_fps` will walk over a `duration`
/// second video. Feeds the progress bar's denominator, so it must never be zero
/// (a progress bar dividing by it would show NaN) and never negative.
fn expected_frames(duration: f64, base_fps: f32) -> u64 {
    if !duration.is_finite() || duration <= 0.0 || base_fps <= 0.0 {
        return 0; // unknown: the caller shows an indeterminate bar
    }
    (duration * base_fps as f64).ceil().max(1.0) as u64
}

/// Absolute floor on the change score (0..255) below which a difference is
/// treated as compression noise, not a real change.
const MIN_ABS_SCORE: f32 = 2.0;
/// Guard so a near-zero rolling average does not blow the ratio up.
const SCORE_EPS: f32 = 0.5;

/// Whether a frame's change score counts as a real change. It must clear an
/// absolute noise floor AND exceed `adaptive_ratio` times the recent average.
/// Dividing by the recent average (rather than testing a fixed threshold) is what
/// keeps a steady scroll from registering as a change on every frame while a real
/// transition still stands out.
fn is_changed(score: f32, recent_avg: f32, adaptive_ratio: f32) -> bool {
    if score <= MIN_ABS_SCORE {
        return false;
    }
    // Coming out of a static run there is no meaningful average, so clearing the noise floor is enough.
    if recent_avg <= SCORE_EPS {
        return true;
    }
    score / recent_avg > adaptive_ratio
}

/// Whether `t` lands on a timestamp that must be sampled regardless of change
/// (e.g. a cursor click), within half a coarse-frame period.
fn is_forced(t: f64, forced: &[f64], base_fps: f32) -> bool {
    let tol = 0.5 / base_fps as f64;
    forced.iter().any(|f| (f - t).abs() <= tol)
}

/// Whether `t` falls inside the kept (post-edit) source ranges. An empty range
/// list means "no edit context", i.e. read the whole file.
fn in_ranges(t: f64, ranges: &[(f64, f64)]) -> bool {
    if ranges.is_empty() {
        return true;
    }
    ranges.iter().any(|(start, end)| t >= *start && t < *end)
}

/// The keep decision for one coarse frame. Pure, so the whole matrix is testable.
///
/// Order matters. `duplicate` comes from the dHash gate, which compares a
/// luma gradient and is therefore blind to colour: two flat frames of different
/// colours hash the same. So a detected change has to beat the duplicate veto,
/// or a theme swap on a low-texture screen would be dropped as a "duplicate"
/// before the colour-aware score ever got a say.
/// Memory ceiling for retained sample frames. Expressed as bytes rather than a
/// frame count because frame size scales with the sampling resolution.
const SAMPLE_BUDGET_BYTES: usize = 512 * 1024 * 1024;

/// Owns the sampler's ffmpeg child and its stderr drain thread so both are
/// reaped on EVERY exit path.
///
/// The frame loop below propagates with `?`. Without this, such an exit left
/// ffmpeg decoding the rest of the video with nobody reading stdout — it never
/// terminates — and the drain thread blocked forever on a pipe that never
/// closed. There is no cancel signal into that loop, so a dropped guard is the
/// only thing that can stop it.
struct SamplerChild {
    child: Option<std::process::Child>,
    stderr: Option<std::thread::JoinHandle<String>>,
}

impl SamplerChild {
    /// Success path: wait normally and collect the stderr tail.
    fn finish(mut self) -> Result<(std::process::ExitStatus, String), String> {
        let mut child = self
            .child
            .take()
            .ok_or_else(|| "ffmpeg child already reaped".to_string())?;
        let status = child.wait().map_err(|e| format!("ffmpeg wait: {e}"))?;
        let tail = self
            .stderr
            .take()
            .and_then(|h| h.join().ok())
            .unwrap_or_default();
        Ok((status, tail))
    }
}

impl Drop for SamplerChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        // Killing the child closes the pipe, so the drain thread now returns.
        if let Some(handle) = self.stderr.take() {
            let _ = handle.join();
        }
    }
}

fn should_keep(
    t: f64,
    last_kept_t: Option<f64>,
    duplicate: bool,
    forced: bool,
    changed: bool,
    opts: &SampleOpts,
) -> bool {
    // Always anchor on the first frame; there is nothing to compare it against.
    let Some(last) = last_kept_t else {
        return true;
    };
    let gap = t - last;
    if forced {
        return true;
    }
    if gap < opts.min_gap_secs {
        return false; // anti-spam: never sample two frames back to back
    }
    if changed {
        return true; // a real change outranks the colour-blind duplicate gate
    }
    if duplicate {
        return false; // same picture, nothing to add
    }
    gap >= opts.max_gap_secs // coverage: catch slow drift the ratio missed
}

/// Read exactly `buf.len()` bytes. Returns `Ok(true)` on a full read, `Ok(false)` on a clean EOF at a frame boundary (zero bytes read), and an error on a partial trailing frame.
fn read_full<R: Read>(r: &mut R, buf: &mut [u8]) -> std::io::Result<bool> {
    let mut filled = 0;
    while filled < buf.len() {
        let n = r.read(&mut buf[filled..])?;
        if n == 0 {
            if filled == 0 {
                return Ok(false);
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "partial frame at end of stream",
            ));
        }
        filled += n;
    }
    Ok(true)
}

/// Mean absolute difference across R, G, B over two equal-sized images, in 0..255.
/// Color-aware on purpose: a theme or highlight recolor at constant brightness
/// moves the color channels even when a grayscale diff would miss it.
fn color_mad(a: &RgbImage, b: &RgbImage) -> f32 {
    debug_assert_eq!(a.dimensions(), b.dimensions());
    let (w, h) = a.dimensions();
    let mut sum: u64 = 0;
    for (pa, pb) in a.pixels().zip(b.pixels()) {
        sum += (pa[0] as i32 - pb[0] as i32).unsigned_abs() as u64;
        sum += (pa[1] as i32 - pb[1] as i32).unsigned_abs() as u64;
        sum += (pa[2] as i32 - pb[2] as i32).unsigned_abs() as u64;
    }
    let count = (w as u64) * (h as u64) * 3;
    if count == 0 {
        0.0
    } else {
        sum as f32 / count as f32
    }
}

/// A 64-bit difference hash: resize to 9x8 grayscale and mark, per row, whether
/// each pixel is brighter than the one to its right. Robust to small brightness
/// shifts, which is what we want for a near-duplicate gate.
fn dhash(img: &DynamicImage) -> u64 {
    let small = img.resize_exact(9, 8, FilterType::Triangle).to_luma8();
    let mut hash: u64 = 0;
    let mut bit = 0u32;
    for y in 0..8u32 {
        for x in 0..8u32 {
            let left = small.get_pixel(x, y)[0];
            let right = small.get_pixel(x + 1, y)[0];
            if left < right {
                hash |= 1 << bit;
            }
            bit += 1;
        }
    }
    hash
}

fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> SampleOpts {
        SampleOpts {
            min_gap_secs: 0.4,
            max_gap_secs: 8.0,
            ..Default::default()
        }
    }

    #[test]
    fn target_dims_caps_long_edge_and_keeps_aspect() {
        // 3840x2160 capped to 1600 long edge -> 1600x900 (even).
        assert_eq!(target_dims(3840, 2160, 1600), (1600, 900));
        // Portrait: the long edge is the height.
        assert_eq!(target_dims(1080, 1920, 960), (540, 960));
        // Already small: unchanged.
        assert_eq!(target_dims(1280, 720, 1600), (1280, 720));
        // Native (max_dim 0): unchanged.
        assert_eq!(target_dims(1920, 1080, 0), (1920, 1080));
        // Odd source dimensions round up to even.
        assert_eq!(target_dims(1919, 1079, 0), (1920, 1080));
    }

    #[test]
    fn expected_frames_is_the_progress_denominator() {
        // 9s at the 3fps coarse rate.
        assert_eq!(expected_frames(9.0, 3.0), 27);
        // Partial trailing frame still gets walked, so round up.
        assert_eq!(expected_frames(9.1, 3.0), 28);
        // An unknown duration reports 0, which the UI reads as indeterminate rather than dividing by it.
        assert_eq!(expected_frames(0.0, 3.0), 0);
        assert_eq!(expected_frames(f64::NAN, 3.0), 0);
        assert_eq!(expected_frames(-1.0, 3.0), 0);
    }

    #[test]
    fn dhash_identical_is_zero_distance() {
        let a = DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            64,
            64,
            image::Rgba([10, 20, 30, 255]),
        ));
        assert_eq!(hamming(dhash(&a), dhash(&a)), 0);
    }

    #[test]
    fn dhash_separates_visibly_different_frames() {
        // A flat field vs a hard vertical split: the gradient hash must diverge.
        let flat = DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            64,
            64,
            image::Rgba([128, 128, 128, 255]),
        ));
        let split = DynamicImage::ImageRgba8(RgbaImage::from_fn(64, 64, |x, _| {
            if x < 32 {
                image::Rgba([0, 0, 0, 255])
            } else {
                image::Rgba([255, 255, 255, 255])
            }
        }));
        assert!(hamming(dhash(&flat), dhash(&split)) > 0);
    }

    #[test]
    fn color_mad_detects_constant_brightness_recolor() {
        // Two colours with near-equal luma: a grayscale diff misses this, which is why the score is colour-aware.
        let red = RgbImage::from_pixel(8, 8, image::Rgb([180, 40, 40]));
        let green = RgbImage::from_pixel(8, 8, image::Rgb([40, 180, 40]));
        assert!(color_mad(&red, &green) > MIN_ABS_SCORE);
        assert_eq!(color_mad(&red, &red), 0.0);
    }

    #[test]
    fn is_changed_needs_both_noise_floor_and_ratio() {
        // Below the absolute floor is noise, however big the ratio looks.
        assert!(!is_changed(1.0, 0.01, 3.0));
        // Above the floor with no meaningful history: a real change.
        assert!(is_changed(10.0, 0.0, 3.0));
        // Above the floor but not enough above the recent average: steady motion.
        assert!(!is_changed(10.0, 5.0, 3.0));
        // Clearly above the recent average: a real transition.
        assert!(is_changed(30.0, 5.0, 3.0));
    }

    #[test]
    fn in_ranges_respects_trim_and_cuts() {
        // No edit context: read the whole file.
        assert!(in_ranges(5.0, &[]));

        // Two kept segments with a cut between 3s and 6s, trimmed to end at 9s.
        let kept = [(1.0, 3.0), (6.0, 9.0)];
        assert!(in_ranges(1.0, &kept)); // start of a segment is inclusive
        assert!(in_ranges(2.5, &kept));
        assert!(in_ranges(6.0, &kept));
        assert!(in_ranges(8.9, &kept));

        assert!(!in_ranges(0.5, &kept)); // trimmed off the head
        assert!(!in_ranges(4.0, &kept)); // inside the cut
        assert!(!in_ranges(9.5, &kept)); // trimmed off the tail
        assert!(!in_ranges(3.0, &kept)); // segment end is exclusive
    }

    #[test]
    fn is_forced_matches_within_half_a_frame_period() {
        // At 4 fps a coarse frame is 0.25s, so the tolerance is 0.125s.
        assert!(is_forced(2.0, &[2.05], 4.0));
        assert!(!is_forced(2.0, &[2.5], 4.0));
        assert!(!is_forced(2.0, &[], 4.0));
    }

    #[test]
    fn should_keep_always_anchors_the_first_frame() {
        // No previous keep: taken regardless of every other signal.
        assert!(should_keep(0.0, None, true, false, false, &opts()));
    }

    #[test]
    fn should_keep_forced_beats_everything() {
        // A click on a barely-changed frame must still be sampled; forced even overrides the anti-spam gap.
        assert!(should_keep(5.0, Some(4.0), true, true, false, &opts()));
        assert!(should_keep(4.1, Some(4.0), true, true, false, &opts()));
    }

    #[test]
    fn should_keep_lets_a_real_change_beat_the_duplicate_gate() {
        // dHash compares a luma gradient, so flat frames of different colours hash alike; colour-aware change must outrank the duplicate veto.
        assert!(should_keep(5.0, Some(4.0), true, false, true, &opts()));
    }

    #[test]
    fn should_keep_drops_duplicates_and_respects_cadence() {
        let o = opts();
        // Duplicate and unchanged: dropped.
        assert!(!should_keep(5.0, Some(4.0), true, false, false, &o));
        // Changed, and comfortably past the min gap: kept.
        assert!(should_keep(5.0, Some(4.0), false, false, true, &o));
        // Changed, but too soon after the last keep: suppressed (anti-spam).
        assert!(!should_keep(4.2, Some(4.0), false, false, true, &o));
        // Unchanged and inside the max gap: dropped.
        assert!(!should_keep(5.0, Some(4.0), false, false, false, &o));
        // Unchanged but past the max gap: kept anyway (coverage).
        assert!(should_keep(13.0, Some(4.0), false, false, false, &o));
        // A duplicate past the max gap adds nothing, so coverage does not save it.
        assert!(!should_keep(13.0, Some(4.0), true, false, false, &o));
    }

    #[test]
    fn read_full_reports_clean_eof_and_rejects_partial_frames() {
        // Exactly two frames of 4 bytes: two full reads, then a clean EOF.
        let data = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let mut cursor = std::io::Cursor::new(data.to_vec());
        let mut buf = [0u8; 4];
        assert!(read_full(&mut cursor, &mut buf).unwrap());
        assert_eq!(buf, [1, 2, 3, 4]);
        assert!(read_full(&mut cursor, &mut buf).unwrap());
        assert_eq!(buf, [5, 6, 7, 8]);
        assert!(!read_full(&mut cursor, &mut buf).unwrap());

        // A trailing partial frame is corruption, not a clean end.
        let mut short = std::io::Cursor::new(vec![1u8, 2, 3]);
        assert!(read_full(&mut short, &mut buf).is_err());
    }
}
