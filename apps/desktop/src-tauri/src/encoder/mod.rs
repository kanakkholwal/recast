use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{ChildStderr, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

// `parking_lot::Mutex` can't poison, so a panicking holder can't silently drop the diagnostic tail.
use parking_lot::Mutex;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

use crate::capture::CaptureArea;
use crate::recording::pipeline::RecordingPipeline;

pub mod h264;
#[cfg(windows)]
pub mod native;

use h264::{EncodePurpose, H264Encoder};

/// Copy rows into a tightly packed buffer.
/// Capture backends deliver rows at the driver's own stride, and FFmpeg's `rawvideo` demuxer expects `width * 4` per row with no gaps.
pub fn pack_rows(bytes: &[u8], stride: u32, width: u32, height: u32) -> Vec<u8> {
    let mut packed = Vec::with_capacity(width as usize * 4 * height as usize);
    pack_rows_into(&mut packed, bytes, stride, width, height);
    packed
}

/// [`pack_rows`] reusing `dst`'s allocation. A live pump packs every frame, so
/// allocating one is megabytes of churn per second at capture resolution.
pub fn pack_rows_into(dst: &mut Vec<u8>, bytes: &[u8], stride: u32, width: u32, height: u32) {
    let row_bytes = width as usize * 4;
    let stride = stride as usize;
    dst.clear();
    if stride == row_bytes {
        dst.extend_from_slice(bytes);
        return;
    }
    for row in 0..height as usize {
        let start = row * stride;
        let Some(line) = bytes.get(start..start + row_bytes) else {
            break;
        };
        dst.extend_from_slice(line);
    }
}

/// Maximum stderr tail retained for diagnostics. The fatal line is always at
/// the end (codec error, disk full, etc.); FFmpeg's startup chatter is noise.
const STDERR_TAIL_LIMIT: usize = 8192;

/// Drains FFmpeg's stderr to EOF on its own thread, keeping the last `STDERR_TAIL_LIMIT` bytes.
/// Load-bearing, not diagnostic: an undrained ~64KB pipe blocks FFmpeg's write, it stops reading stdin, and the encoder deadlocks mid-recording.
fn pump_stderr_tail(stderr: ChildStderr, sink: Arc<Mutex<String>>) {
    let mut reader = std::io::BufReader::new(stderr);
    let mut chunk = [0u8; 4096];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break, // EOF — FFmpeg closed stderr (i.e. exited).
            Ok(n) => {
                let mut tail = sink.lock();
                tail.push_str(&String::from_utf8_lossy(&chunk[..n]));
                if tail.len() > STDERR_TAIL_LIMIT {
                    let mut cut = tail.len() - STDERR_TAIL_LIMIT;
                    // Prefer a newline boundary so the tail starts on a clean line; fall back to the raw offset.
                    if let Some(nl) = tail[cut..].find('\n') {
                        cut += nl + 1;
                    }
                    // `drain` panics off a char boundary, and lossy decoding can straddle chunks, so back off first.
                    while cut < tail.len() && !tail.is_char_boundary(cut) {
                        cut += 1;
                    }
                    tail.drain(..cut);
                }
            }
            Err(_) => break,
        }
    }
}

/// Join the stderr pump (if it was spawned) and return the retained tail.
/// The pump exits when FFmpeg closes stderr, so the process must already have
/// exited — or be exiting — before this is called.
fn collect_stderr_tail(
    pump: &mut Option<thread::JoinHandle<()>>,
    sink: &Arc<Mutex<String>>,
) -> String {
    if let Some(handle) = pump.take() {
        let _ = handle.join();
    }
    sink.lock().trim().to_string()
}

/// Capture-time quality tier. `Balanced` emits byte-identical args to the historical default, so old recordings are unchanged.
/// Every tier stays 8-bit 4:2:0: the editor previews the raw file in a WebView decoder capped at High profile.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RecordingQuality {
    #[default]
    Balanced,
    High,
    Pristine,
}

impl RecordingQuality {
    /// Parse the frontend's string tier; anything unrecognized (incl. `None`) falls back to `Balanced` so an old/garbled payload can never break a recording.
    pub fn from_label(label: Option<&str>) -> Self {
        match label {
            Some("high") => Self::High,
            Some("pristine") => Self::Pristine,
            _ => Self::Balanced,
        }
    }

    /// Resolves the frontend's tier including the `"auto"` default: a hardware encoder gets `High`, pure `libx264` stays `Balanced`.
    /// The capture master is what every export re-encodes from, so a GPU machine has no reason to record at the low-latency tier.
    pub fn resolve(label: Option<&str>, encoder: &str) -> Self {
        match label {
            Some("auto") | None => {
                if H264Encoder::from_ffmpeg_name(encoder).is_hardware() {
                    Self::High
                } else {
                    Self::Balanced
                }
            }
            other => Self::from_label(other),
        }
    }
}

/// Configuration for the live recording encoder.
#[derive(Clone, Debug)]
pub struct EncoderConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub crop: Option<CaptureArea>,
    pub output_path: PathBuf,
    /// Capture-time quality tier. `Default` (`Balanced`) keeps the historical
    /// fast/low-latency encoder args unchanged.
    pub quality: RecordingQuality,
}

/// Number of duplicate frames to emit alongside one real frame to make up
/// for pacer drops, bounded by `cap` so a large backlog drains over several
/// iterations rather than blocking the encode loop in one burst. The residual
/// (anything above `cap`) is flushed after the capture loop ends.
fn dup_count(total_drops: u64, compensated: u64, cap: u64) -> u64 {
    total_drops.saturating_sub(compensated).min(cap)
}

fn build_video_filter(crop: Option<CaptureArea>) -> Option<String> {
    crop.map(|area| {
        format!(
            "crop={}:{}:{}:{}",
            area.width,
            area.height,
            area.x.max(0),
            area.y.max(0)
        )
    })
}

/// Spawn the encoder thread. Pulls raw BGRA frames from the pipeline
/// and pipes them to FFmpeg for H.264 encoding.
pub fn spawn_encoder_loop(
    config: EncoderConfig,
    stop_flag: Arc<AtomicBool>,
    pipeline: RecordingPipeline,
) -> Result<thread::JoinHandle<Result<()>>> {
    thread::Builder::new()
        .name("recast-encoder".into())
        .spawn(move || {
            let encoder = H264Encoder::from_ffmpeg_name(crate::ffmpeg::preferred_h264_encoder());
            let mut args = vec![
                "-y".to_string(),
                "-f".to_string(),
                "rawvideo".to_string(),
                "-pixel_format".to_string(),
                "bgra".to_string(),
                "-video_size".to_string(),
                format!("{}x{}", config.width, config.height),
                "-framerate".to_string(),
                config.fps.to_string(),
                "-i".to_string(),
                "-".to_string(),
                "-an".to_string(),
            ];

            if let Some(filter) = build_video_filter(config.crop) {
                args.extend(["-vf".to_string(), filter]);
            }

            // Hardware gets a low-latency preset on Balanced; libx264 stays ultrafast there so weak CPUs don't drop frames.
            args.extend(h264::codec_args(
                encoder,
                EncodePurpose::RealtimeCapture(config.quality),
            ));

            // A ~0.5s GOP instead of the ~4s default: a seek re-decodes from the preceding keyframe, which at 4K was a multi-second freeze.
            args.push("-g".to_string());
            args.push((config.fps / 2).max(1).to_string());

            args.push(config.output_path.to_string_lossy().to_string());

            let mut command = Command::new(crate::ffmpeg::ffmpeg_path());
            command
                .args(&args)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped());
            crate::ffmpeg::configure_silent_command(&mut command);
            let mut child = command
                .spawn()
                .with_context(|| "failed to start ffmpeg encoder")?;

            let mut stdin = child
                .stdin
                .take()
                .context("ffmpeg encoder stdin was not available")?;

            // Drain stderr on a side thread: see `pump_stderr_tail`, this avoids a deadlock rather than being nice-to-have.
            let stderr_tail = Arc::new(Mutex::new(String::new()));
            let mut stderr_pump = match child.stderr.take() {
                Some(stderr) => {
                    let sink = stderr_tail.clone();
                    thread::Builder::new()
                        .name("recast-encoder-stderr".into())
                        .spawn(move || pump_stderr_tail(stderr, sink))
                        .ok()
                }
                None => None,
            };
            let stats = pipeline.stats();

            // Check liveness every ~30 frames: try_wait is cheap but not free, and per-frame would load the hot path.
            let mut frames_since_alive_check: u32 = 0;
            const ALIVE_CHECK_EVERY: u32 = 30;

            // Re-emit the last frame per pacer drop: with a fixed `-framerate` and timestamp-less rawvideo, drops shorten the output and desync audio.
            let mut compensated_drops: u64 = 0;
            let mut last_frame: Option<std::sync::Arc<[u8]>> = None;
            // Bound the dup burst per real frame so a backlog drains over iterations; the residual flushes after the loop.
            const MAX_DUPS_PER_ITER: u64 = 120;

            loop {
                if let Some(frame) = pipeline.pop() {
                    // Detect an early exit BEFORE writing, or Windows reports 'pipe is being closed (os error 232)' instead of the real reason.
                    if frames_since_alive_check >= ALIVE_CHECK_EVERY {
                        frames_since_alive_check = 0;
                        if let Ok(Some(status)) = child.try_wait() {
                            drop(stdin);
                            let tail = collect_stderr_tail(&mut stderr_pump, &stderr_tail);
                            return Err(anyhow!(
                                "ffmpeg encoder exited unexpectedly mid-recording \
                                 (status: {status}). Last stderr output:\n{tail}"
                            ));
                        }
                    }
                    frames_since_alive_check += 1;

                    // The real frame, then one duplicate per pacer drop seen since the last write, capped per iteration.
                    let drops = stats.dropped_frames.load(Ordering::Relaxed);
                    let dups = dup_count(drops, compensated_drops, MAX_DUPS_PER_ITER);
                    compensated_drops += dups;
                    for _ in 0..(1 + dups) {
                        if let Err(e) = stdin.write_all(&frame) {
                            // FFmpeg died between the liveness check and this write; the stderr pump is draining, so `wait()` can't hang.
                            drop(stdin);
                            let _ = child.wait();
                            let tail = collect_stderr_tail(&mut stderr_pump, &stderr_tail);
                            return Err(anyhow!(
                                "ffmpeg encoder stdin write failed ({e}). \
                                 FFmpeg likely exited mid-recording. \
                                 Last stderr output:\n{tail}"
                            ));
                        }
                        stats.encoded_frames.fetch_add(1, Ordering::Relaxed);
                    }
                    last_frame = Some(frame);
                    continue;
                }

                if stop_flag.load(Ordering::Acquire) && pipeline.is_empty() {
                    break;
                }

                thread::sleep(Duration::from_millis(2));
            }

            // Flush the drops the per-iteration cap left behind, so encoded frames equal captured and length matches wall clock.
            if let Some(last) = last_frame {
                let drops = stats.dropped_frames.load(Ordering::Relaxed);
                let mut remaining = drops.saturating_sub(compensated_drops);
                while remaining > 0 {
                    if stdin.write_all(&last).is_err() {
                        break;
                    }
                    stats.encoded_frames.fetch_add(1, Ordering::Relaxed);
                    remaining -= 1;
                }
            }

            drop(stdin);

            // stdout is null and stderr is drained by the pump, so a bare `wait()` can't deadlock.
            let status = child.wait()?;
            let tail = collect_stderr_tail(&mut stderr_pump, &stderr_tail);
            if !status.success() {
                return Err(anyhow!("ffmpeg encoder failed (status: {status}): {tail}"));
            }

            Ok(())
        })
        .map_err(Into::into)
}

#[cfg(test)]
mod pack_rows_tests {
    use super::pack_rows;

    #[test]
    fn packing_a_frame_without_padding_copies_it_verbatim() {
        let bytes: Vec<u8> = (0..16u8).collect();
        assert_eq!(pack_rows(&bytes, 8, 2, 2), bytes);
    }

    #[test]
    fn packing_drops_the_padding_between_rows() {
        // 1px wide at a stride of 6: 2 padding bytes per row the encoder must not see.
        let bytes = vec![1, 2, 3, 4, 0xFF, 0xFF, 5, 6, 7, 8, 0xFF, 0xFF];
        assert_eq!(pack_rows(&bytes, 6, 1, 2), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn a_short_buffer_yields_the_rows_it_actually_has() {
        let bytes = vec![1, 2, 3, 4, 0xFF, 0xFF, 5, 6];
        assert_eq!(pack_rows(&bytes, 6, 1, 2), vec![1, 2, 3, 4]);
    }

    /// The camera pump packs into one buffer for the life of the preview, so a
    /// second frame must replace the first rather than append to it.
    #[test]
    fn packing_into_a_reused_buffer_leaves_only_the_latest_frame() {
        let first = vec![1, 2, 3, 4, 0xFF, 0xFF, 5, 6, 7, 8, 0xFF, 0xFF];
        let second = vec![9, 9, 9, 9, 0xFF, 0xFF, 8, 8, 8, 8, 0xFF, 0xFF];
        let mut buffer = Vec::new();
        super::pack_rows_into(&mut buffer, &first, 6, 1, 2);
        super::pack_rows_into(&mut buffer, &second, 6, 1, 2);
        assert_eq!(buffer, vec![9, 9, 9, 9, 8, 8, 8, 8]);
    }

    #[test]
    fn packing_into_a_buffer_matches_the_allocating_form() {
        let bytes = vec![1, 2, 3, 4, 0xFF, 0xFF, 5, 6, 7, 8, 0xFF, 0xFF];
        let mut buffer = Vec::new();
        super::pack_rows_into(&mut buffer, &bytes, 6, 1, 2);
        assert_eq!(buffer, pack_rows(&bytes, 6, 1, 2));
    }
}

#[cfg(test)]
mod tests {
    use super::{build_video_filter, dup_count, RecordingQuality};
    use crate::capture::CaptureArea;

    #[test]
    fn recording_quality_parses_with_safe_default() {
        assert_eq!(
            RecordingQuality::from_label(Some("high")),
            RecordingQuality::High
        );
        assert_eq!(
            RecordingQuality::from_label(Some("pristine")),
            RecordingQuality::Pristine
        );
        // Unknown / missing / default → Balanced, never an error.
        assert_eq!(
            RecordingQuality::from_label(Some("balanced")),
            RecordingQuality::Balanced
        );
        assert_eq!(
            RecordingQuality::from_label(Some("garbage")),
            RecordingQuality::Balanced
        );
        assert_eq!(
            RecordingQuality::from_label(None),
            RecordingQuality::Balanced
        );
    }

    #[test]
    fn resolve_auto_picks_high_on_hardware_and_balanced_on_software() {
        // Every hardware encoder has real-time headroom, so auto or unset goes up to High regardless of label.
        for enc in ["h264_nvenc", "h264_amf", "h264_qsv", "h264_videotoolbox"] {
            assert_eq!(
                RecordingQuality::resolve(Some("auto"), enc),
                RecordingQuality::High,
                "auto on {enc}"
            );
            assert_eq!(
                RecordingQuality::resolve(None, enc),
                RecordingQuality::High,
                "unset on {enc}"
            );
        }
        // Pure software fallback stays Balanced so weak CPUs don't drop frames.
        assert_eq!(
            RecordingQuality::resolve(Some("auto"), "libx264"),
            RecordingQuality::Balanced
        );
        assert_eq!(
            RecordingQuality::resolve(None, "libx264"),
            RecordingQuality::Balanced
        );
        // An explicit tier is always honored, even on hardware that could do more.
        assert_eq!(
            RecordingQuality::resolve(Some("balanced"), "h264_nvenc"),
            RecordingQuality::Balanced
        );
        assert_eq!(
            RecordingQuality::resolve(Some("high"), "libx264"),
            RecordingQuality::High
        );
        assert_eq!(
            RecordingQuality::resolve(Some("pristine"), "libx264"),
            RecordingQuality::Pristine
        );
    }

    // Per-encoder argument construction and its regression guards moved to `encoder::h264` tests.

    /// Simulates the encoder's emit accounting over a whole recording to assert the load-bearing invariant.
    /// Frames written to FFmpeg, real plus compensating duplicates and the flush, must equal what the pacer captured, so one wall-clock second is always one second of PTS.
    fn total_emitted(captured: u64, drops: u64, cap: u64) -> u64 {
        assert!(drops <= captured);
        let real_frames = captured - drops; // frames that made it through the queue
        let mut compensated = 0u64;
        let mut emitted = 0u64;
        for _ in 0..real_frames {
            // Worst case for placement: every drop is already visible when each real frame is written.
            let dups = dup_count(drops, compensated, cap);
            compensated += dups;
            emitted += 1 + dups;
        }
        // Post-loop flush of any residual the per-iteration cap left behind.
        emitted += drops.saturating_sub(compensated);
        emitted
    }

    #[test]
    fn no_drops_emits_exactly_captured() {
        assert_eq!(total_emitted(600, 0, 120), 600);
    }

    #[test]
    fn drops_are_fully_compensated_to_match_captured() {
        // Encoded must equal captured across drop counts, with a cap small enough to force multi-iteration draining.
        for &(captured, drops) in &[(600, 50), (600, 599), (100, 1), (3600, 1200)] {
            assert_eq!(
                total_emitted(captured, drops, 8),
                captured,
                "captured={captured} drops={drops}"
            );
        }
    }

    #[test]
    fn dup_count_is_bounded_by_cap_and_never_over_compensates() {
        assert_eq!(dup_count(100, 0, 30), 30); // capped
        assert_eq!(dup_count(100, 90, 30), 10); // only the remainder
        assert_eq!(dup_count(100, 100, 30), 0); // fully compensated
        assert_eq!(dup_count(5, 9, 30), 0); // never negative
    }

    #[test]
    fn no_crop_yields_no_filter() {
        assert_eq!(build_video_filter(None), None);
    }

    #[test]
    fn crop_renders_ffmpeg_crop_filter() {
        let area = CaptureArea {
            x: 10,
            y: 20,
            width: 100,
            height: 50,
        };
        // Order is width:height:x:y — the FFmpeg `crop` argument order.
        assert_eq!(
            build_video_filter(Some(area)).as_deref(),
            Some("crop=100:50:10:20")
        );
    }

    #[test]
    fn negative_offsets_clamp_to_zero() {
        // A crop origin can go negative after coordinate math, and FFmpeg rejects negative offsets.
        let area = CaptureArea {
            x: -5,
            y: -3,
            width: 40,
            height: 30,
        };
        assert_eq!(
            build_video_filter(Some(area)).as_deref(),
            Some("crop=40:30:0:0")
        );
    }
}
