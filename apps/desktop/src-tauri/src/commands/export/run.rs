use std::io::{BufRead, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tauri::AppHandle;

use super::progress::{is_ffmpeg_progress_key_line, parse_ffmpeg_progress_seconds, ProgressBand};
use super::state::{emit_export_state, ExportStateEvent};
use crate::commands::ffmpeg::{probe_video_metadata, summarize_ffmpeg_failure};

fn completed_export_looks_usable(path: &Path, expected_duration: f64) -> bool {
    if !path.exists() {
        return false;
    }

    let Ok(metadata) = probe_video_metadata(path) else {
        return false;
    };

    if metadata.duration <= 0.0 || metadata.width == 0 || metadata.height == 0 {
        return false;
    }

    if expected_duration <= 0.0 {
        return true;
    }

    let min_duration = if expected_duration > 1.0 {
        (expected_duration - 0.5).max(expected_duration * 0.95)
    } else {
        expected_duration * 0.75
    };

    metadata.duration + 0.05 >= min_duration
}

/// True for an abnormal-termination exit code (a crash / signal) rather than a
/// normal ffmpeg error exit (0..=255). A hardware-encoder crash on Windows shows
/// up as an NTSTATUS such as 0xC0000005 (`-1073741819` as i32).
pub(crate) fn is_ffmpeg_crash_code(code: i32) -> bool {
    !(0..=255).contains(&code)
}

/// Pull the ffmpeg exit code out of a `run_encode` error string
/// (`export failed (ffmpeg exit <code>): …`). None when the message carries none.
pub(crate) fn parse_ffmpeg_exit_code(err: &str) -> Option<i32> {
    let start = err.find("ffmpeg exit ")? + "ffmpeg exit ".len();
    let rest = &err[start..];
    let end = rest
        .char_indices()
        .find(|&(_, c)| !(c.is_ascii_digit() || c == '-'))
        .map(|(i, _)| i)
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

pub(crate) fn run_encode(
    args: Vec<String>,
    app: AppHandle,
    export_id: String,
    cancel_flag: Arc<AtomicBool>,
    output_path_str: String,
    expected_output_secs: f64,
    progress_band: ProgressBand,
) -> Result<String, String> {
    let mut command = Command::new(crate::ffmpeg::ffmpeg_path());
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::ffmpeg::configure_silent_command(&mut command);

    // ffmpeg segfaults print nothing to stderr, so this is the only record of encoder, filters and inputs when it dies.
    log::info!("export: ffmpeg args: {}", args.join(" "));

    let mut child = command
        .spawn()
        .map_err(|e| format!("failed to start ffmpeg: {e}"))?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "ffmpeg stdout pipe not available".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "ffmpeg stderr pipe not available".to_string())?;

    // Shared by the stderr parser (progress events) and the watchdog (stall detection).
    let last_progress = Arc::new(Mutex::new(Instant::now()));
    let last_progress_secs = Arc::new(Mutex::new(-1.0_f64));
    let killed_by_timeout = Arc::new(AtomicBool::new(false));
    let killed_by_user = Arc::new(AtomicBool::new(false));
    let finalizing_seen = Arc::new(AtomicBool::new(false));
    let near_end_seen = Arc::new(AtomicBool::new(false));
    let progress_end_seen = Arc::new(AtomicBool::new(false));
    // Latched on the first progress block: the watchdog allows a longer cold-start budget until frames flow.
    let first_progress_seen = Arc::new(AtomicBool::new(false));

    // Progress blocks are filtered out; only real log output enters the 8 KB ring, and `progress=end` means only the trailer remains.
    let stderr_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let stderr_buf_writer = stderr_buf.clone();
    // Kept apart from the rotating tail: FFmpeg names the cause while opening inputs, then prints kilobytes that flush it out.
    let stderr_errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let stderr_errors_writer = stderr_errors.clone();
    let stderr_last_progress = last_progress.clone();
    let stderr_last_progress_secs = last_progress_secs.clone();
    let stderr_app = app.clone();
    let stderr_export_id = export_id.clone();
    let stderr_finalizing_seen = finalizing_seen.clone();
    let stderr_near_end_seen = near_end_seen.clone();
    let stderr_progress_end_seen = progress_end_seen.clone();
    let stderr_first_progress_seen = first_progress_seen.clone();
    let encode_started_at = Instant::now();
    let stderr_thread = std::thread::Builder::new()
        .name("recast-export-stderr".into())
        .spawn(move || {
            let mut reader = std::io::BufReader::new(stderr);
            let mut logged_near_done = false;
            // Raw bytes, not `.lines()`: that stops at the first non-UTF-8 line and drops any real error printed after it.
            let mut raw: Vec<u8> = Vec::new();
            loop {
                raw.clear();
                match reader.read_until(b'\n', &mut raw) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(_) => break,
                }
                while matches!(raw.last(), Some(b'\n' | b'\r')) {
                    raw.pop();
                }
                let line = String::from_utf8_lossy(&raw).into_owned();
                // Progress blocks are key=value lines ending in `progress=continue` or `progress=end`; all of it is non-log noise.
                if let Some(progress_secs) = parse_ffmpeg_progress_seconds(&line) {
                    let effective_duration = expected_output_secs;
                    // Any parseable progress line is proof of life: Windows NVENC repeats `out_time_us`, and gating on advance starved the watchdog.
                    {
                        let mut guard = stderr_last_progress.lock();
                        *guard = Instant::now();
                    }
                    // The first progress line flips the startup-grace flag, and the log shows how long filter_complex and NVENC warmup took.
                    if !stderr_first_progress_seen.swap(true, Ordering::AcqRel) {
                        log::info!(
                            "export: first progress parsed at T+{}ms",
                            encode_started_at.elapsed().as_millis()
                        );
                    }
                    // Only publish when out_time actually advanced; redundant emits spam the bar with the same value.
                    let advanced = {
                        let mut last_secs = stderr_last_progress_secs.lock();
                        if progress_secs > *last_secs + 0.01 {
                            *last_secs = progress_secs;
                            true
                        } else {
                            false
                        }
                    };
                    if !advanced {
                        continue;
                    }
                    let pct = if effective_duration > 0.0 {
                        (progress_secs / effective_duration * 100.0).clamp(0.0, 100.0)
                    } else {
                        0.0
                    };
                    if effective_duration > 0.0
                        && (effective_duration - progress_secs).max(0.0) <= 0.25
                    {
                        stderr_near_end_seen.store(true, Ordering::Release);
                    }
                    // Log the 99.5% crossing so 'stuck at 99%' reports can locate the gap before `progress=end`.
                    if !logged_near_done && pct >= 99.5 {
                        logged_near_done = true;
                        log::info!(
                            "export: reached {:.1}% at T+{}ms, awaiting progress=end",
                            pct,
                            encode_started_at.elapsed().as_millis()
                        );
                    }
                    // The GIF pre-pass owns 0..40% and this pass 40..100%; scaling here keeps the terminal 100% emits honest.
                    let scaled_pct = progress_band.at(pct);
                    emit_export_state(
                        &stderr_app,
                        ExportStateEvent::progress(&stderr_export_id, scaled_pct),
                    );
                    continue;
                }
                // Flip to finalizing on `progress=end`: Windows stderr close can lag the encoder by seconds, parking the bar at 100%.
                if line.trim() == "progress=end" {
                    stderr_progress_end_seen.store(true, Ordering::Release);
                    if !stderr_finalizing_seen.swap(true, Ordering::AcqRel) {
                        emit_export_state(
                            &stderr_app,
                            ExportStateEvent::progress(&stderr_export_id, 100.0_f64),
                        );
                        emit_export_state(
                            &stderr_app,
                            ExportStateEvent::finalizing(&stderr_export_id),
                        );
                        log::info!(
                            "export: progress=end seen at T+{}ms, flipping UI to finalizing",
                            encode_started_at.elapsed().as_millis()
                        );
                    }
                    let mut guard = stderr_last_progress.lock();
                    *guard = Instant::now();
                    continue;
                }
                if is_ffmpeg_progress_key_line(&line) {
                    continue;
                }
                // Capture anything that names a cause first, so it survives wherever in the stream it appeared.
                if crate::commands::ffmpeg::is_diagnostic_line(&line) {
                    let mut errors = stderr_errors_writer.lock();
                    if errors.len() < crate::commands::ffmpeg::MAX_RETAINED_ERRORS {
                        errors.push(line.clone());
                    }
                }
                let mut guard = stderr_buf_writer.lock();
                guard.extend_from_slice(line.as_bytes());
                guard.push(b'\n');
                if guard.len() > 8192 {
                    let overflow = guard.len() - 8192;
                    guard.drain(0..overflow);
                }
            }
            log::info!(
                "export: stderr thread exiting at T+{}ms (pipe closed)",
                encode_started_at.elapsed().as_millis()
            );
        })
        .map_err(|e| format!("failed to spawn stderr drain thread: {e}"))?;

    // Stdout carries nothing now, but an undrained pipe can make FFmpeg hit EPIPE on a stray write and abort.
    let stdout_thread = std::thread::Builder::new()
        .name("recast-export-stdout".into())
        .spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match stdout.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
            log::info!("export: stdout thread exiting (pipe closed)");
        })
        .map_err(|e| format!("failed to spawn stdout drain thread: {e}"))?;

    // Kills only on a >60s progress stall or user cancel: the old quiet-means-finalizing rule fired falsely on Windows buffering.
    let watchdog_last_progress = last_progress.clone();
    let watchdog_killed = killed_by_timeout.clone();
    let watchdog_cancel_flag = cancel_flag.clone();
    let watchdog_user_kill = killed_by_user.clone();
    let watchdog_near_end_seen = near_end_seen.clone();
    let watchdog_progress_end_seen = progress_end_seen.clone();
    let watchdog_first_progress_seen = first_progress_seen.clone();
    let watchdog_stop = Arc::new(AtomicBool::new(false));
    let watchdog_stop_flag = watchdog_stop.clone();
    // Share the child with the watchdog via a mutex so it can call kill().
    let child_handle = Arc::new(Mutex::new(Some(child)));
    let watchdog_child = child_handle.clone();
    let watchdog_output_path = output_path_str.clone();
    let watchdog_thread = std::thread::Builder::new()
            .name("recast-export-watchdog".into())
            .spawn(move || {
                const ENCODE_TIMEOUT: Duration = Duration::from_secs(60);
                const NEAR_END_TIMEOUT: Duration = Duration::from_secs(20);
                // filter_complex parsing, NVENC surface alloc and VP9 first-pass all run before the first frame, so grant a bigger budget.
                const FIRST_PROGRESS_TIMEOUT: Duration = Duration::from_secs(120);
                // A no-file-growth bound, not a wall-clock cap: the trailer write grows the file and stamps liveness, so only zero growth stalls.
                const FINALIZING_TIMEOUT: Duration = Duration::from_secs(60);
                const POLL_INTERVAL: Duration = Duration::from_millis(250);
                let mut last_file_size: u64 = 0;
                while !watchdog_stop_flag.load(Ordering::Acquire) {
                    std::thread::sleep(POLL_INTERVAL);
                    if watchdog_stop_flag.load(Ordering::Acquire) {
                        return;
                    }
                    if watchdog_cancel_flag.load(Ordering::Acquire) {
                        let mut guard = watchdog_child.lock();
                        if let Some(ref mut child) = *guard {
                            log::info!("export cancel: killing ffmpeg process on user request");
                            let _ = child.kill();
                            watchdog_user_kill.store(true, Ordering::Release);
                        }
                        return;
                    }
                    let in_finalizing = watchdog_progress_end_seen.load(Ordering::Acquire);
                    // File growth is liveness in both phases, independent of whether the stderr thread has refreshed the stamp yet.
                    if let Ok(meta) = std::fs::metadata(&watchdog_output_path) {
                        let size = meta.len();
                        if size > last_file_size {
                            last_file_size = size;
                            let mut guard = watchdog_last_progress.lock();
                            *guard = Instant::now();
                        }
                    }
                    let elapsed = {
                        let guard = watchdog_last_progress.lock();
                        guard.elapsed()
                    };
                    let near_end = watchdog_near_end_seen.load(Ordering::Acquire);
                    let first_seen = watchdog_first_progress_seen.load(Ordering::Acquire);
                    let allowed_idle = if in_finalizing {
                        FINALIZING_TIMEOUT
                    } else if near_end {
                        NEAR_END_TIMEOUT
                    } else if !first_seen {
                        FIRST_PROGRESS_TIMEOUT
                    } else {
                        ENCODE_TIMEOUT
                    };
                    if elapsed > allowed_idle {
                        let mut guard = watchdog_child.lock();
                        if let Some(ref mut child) = *guard {
                            let total_elapsed = encode_started_at.elapsed().as_millis();
                            if in_finalizing {
                                log::warn!(
                                    "export watchdog: killing ffmpeg after progress=end at T+{total_elapsed}ms; no exit for {elapsed:?}"
                                );
                            } else if near_end {
                                log::warn!(
                                    "export watchdog: killing ffmpeg near end of encode at T+{total_elapsed}ms; progress stopped for {elapsed:?}"
                                );
                            } else {
                                log::warn!(
                                    "export watchdog: killing stalled ffmpeg at T+{total_elapsed}ms (no progress for {elapsed:?})"
                                );
                            }
                            let _ = child.kill();
                            watchdog_killed.store(true, Ordering::Release);
                        }
                        return;
                    }
                }
            })
            .map_err(|e| format!("failed to spawn watchdog thread: {e}"))?;

    // Both drain threads unblock when FFmpeg closes its pipes, which happens as it exits.
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();
    log::info!(
        "export: drain threads joined at T+{}ms (pipes closed)",
        encode_started_at.elapsed().as_millis()
    );

    // Idempotent: if `progress=end` never arrived, the UI still gets a finalizing flip before `export-done`.
    if !killed_by_user.load(Ordering::Acquire)
        && !killed_by_timeout.load(Ordering::Acquire)
        && !finalizing_seen.swap(true, Ordering::AcqRel)
    {
        emit_export_state(&app, ExportStateEvent::progress(&export_id, 100.0_f64));
        emit_export_state(&app, ExportStateEvent::finalizing(&export_id));
    }

    // Stop the watchdog now that the I/O is done.
    watchdog_stop.store(true, Ordering::Release);
    let _ = watchdog_thread.join();

    let expected_output_duration = expected_output_secs;

    // Pipes closed means the file is written, so probe and report success now rather than waiting on a reap that takes seconds on Windows.
    let early_success_emitted = if !killed_by_user.load(Ordering::Acquire)
        && !killed_by_timeout.load(Ordering::Acquire)
        && progress_end_seen.load(Ordering::Acquire)
        && completed_export_looks_usable(Path::new(&output_path_str), expected_output_duration)
    {
        log::info!(
                "export: pipes closed and output probe ok at T+{}ms; emitting success early and reaping child",
                encode_started_at.elapsed().as_millis()
            );
        emit_export_state(&app, ExportStateEvent::progress(&export_id, 100.0_f64));
        emit_export_state(
            &app,
            ExportStateEvent::success(&export_id, &output_path_str),
        );
        true
    } else {
        false
    };

    // Stdout has closed, so exit should take milliseconds; POST_CLOSE_TIMEOUT force-kills rather than leaking the process.
    let mut child = {
        let mut guard = child_handle.lock();
        guard.take()
    }
    .ok_or_else(|| "ffmpeg child handle missing".to_string())?;

    const POST_CLOSE_TIMEOUT: Duration = Duration::from_secs(30);
    let wait_deadline = Instant::now() + POST_CLOSE_TIMEOUT;
    let mut forced_exit = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= wait_deadline {
                    log::warn!(
                        "export post-close wait exceeded {:?} at T+{}ms; force-killing ffmpeg",
                        POST_CLOSE_TIMEOUT,
                        encode_started_at.elapsed().as_millis()
                    );
                    let _ = child.kill();
                    forced_exit = true;
                    // One final wait after kill to reap the process.
                    break child.wait().map_err(|e| e.to_string())?;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    log::info!(
        "export: child exited at T+{}ms (status={:?}, forced_exit={}, early_success_emitted={})",
        encode_started_at.elapsed().as_millis(),
        status.code(),
        forced_exit,
        early_success_emitted
    );

    // The UI already succeeded off a probe of a complete file, so the reap outcome is only bookkeeping.
    if early_success_emitted {
        return Ok(output_path_str);
    }

    if forced_exit {
        let output_path = Path::new(&output_path_str);
        // With `progress_end` the encoder finished before the kill and the probe confirms playability; without it, refuse the file.
        let encode_completed = progress_end_seen.load(Ordering::Acquire);
        if encode_completed && completed_export_looks_usable(output_path, expected_output_duration)
        {
            log::warn!(
                    "export: ffmpeg was force-killed after post-close timeout, but progress=end was seen and output looks usable; treating as success"
                );
            emit_export_state(&app, ExportStateEvent::progress(&export_id, 100.0_f64));
            emit_export_state(
                &app,
                ExportStateEvent::success(&export_id, &output_path_str),
            );
            return Ok(output_path_str);
        }

        let _ = std::fs::remove_file(output_path);
        let err_msg = format!(
            "export failed: ffmpeg did not exit within {}s of finishing the encode",
            POST_CLOSE_TIMEOUT.as_secs()
        );
        emit_export_state(&app, ExportStateEvent::error(&export_id, &err_msg));
        return Err(err_msg);
    }

    if killed_by_user.load(Ordering::Acquire) {
        // Remove the half-written output so the exports list doesn't show a broken artifact from the aborted run.
        let _ = std::fs::remove_file(&output_path_str);
        emit_export_state(&app, ExportStateEvent::cancelled(&export_id));
        return Err("export cancelled".to_string());
    }

    if killed_by_timeout.load(Ordering::Acquire) {
        let output_path = Path::new(&output_path_str);
        // Trust the file only if `progress=end` arrived: killed mid-encode it is truncated, and a probe can still pass on a partial mux.
        let encode_completed = progress_end_seen.load(Ordering::Acquire);
        if encode_completed && completed_export_looks_usable(output_path, expected_output_duration)
        {
            log::warn!(
                    "export: watchdog killed ffmpeg after progress=end; output looks usable, treating as success"
                );
            emit_export_state(&app, ExportStateEvent::progress(&export_id, 100.0_f64));
            emit_export_state(
                &app,
                ExportStateEvent::success(&export_id, &output_path_str),
            );
            return Ok(output_path_str);
        }

        let _ = std::fs::remove_file(output_path);
        let base_msg = if encode_completed {
            "export failed: ffmpeg reached finalizing but the output file stopped growing for 60s"
        } else if near_end_seen.load(Ordering::Acquire) {
            "export failed: ffmpeg stopped making progress near the end of the encode"
        } else {
            "export timed out: ffmpeg produced no progress for 60s"
        };
        // Take the final line or two of the 8 KB ring, so the error is actionable and still scannable.
        let stderr_tail = {
            let guard = stderr_buf.lock();
            let text = String::from_utf8_lossy(&guard).into_owned();
            text.lines()
                .rev()
                .filter(|l| !l.trim().is_empty())
                .take(2)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join(" | ")
        };
        let err_msg = if stderr_tail.is_empty() {
            base_msg.to_string()
        } else {
            format!("{base_msg} — last stderr: {stderr_tail}")
        };
        emit_export_state(&app, ExportStateEvent::error(&export_id, &err_msg));
        return Err(err_msg);
    }

    if !status.success() {
        let stderr_bytes = stderr_buf.lock().clone();
        // ERROR level because release defaults to Warn and would drop the `info!` copy of the args logged before the spawn.
        log::error!(
            "export: ffmpeg failed (status {:?})
  args: {}
  diagnostics: {:?}
  stderr tail:
{}",
            status.code(),
            args.join(" "),
            stderr_errors.lock().as_slice(),
            String::from_utf8_lossy(&stderr_bytes)
        );
        let _ = std::fs::remove_file(&output_path_str);
        // Without a diagnostic line the code is the only signal: a large Windows code like 0xC0000005 means a crash, not an encode error.
        let code = status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "terminated by signal".into());
        let err_msg = format!(
            "export failed (ffmpeg exit {code}):\n{}",
            summarize_ffmpeg_failure(&stderr_errors.lock(), &stderr_bytes)
        );
        emit_export_state(&app, ExportStateEvent::error(&export_id, &err_msg));
        return Err(err_msg);
    }

    // Log the tail on success too, to catch warnings that give a valid exit code and a broken file.
    let stderr_bytes = stderr_buf.lock().clone();
    if !stderr_bytes.is_empty() {
        let tail = String::from_utf8_lossy(&stderr_bytes);
        log::info!("export ffmpeg stderr tail: {tail}");
    }

    // On status 0 with `progress=end` we trust FFmpeg's exit: an extra ffprobe would park the UI in Finalizing, the exact hang users hit.
    let _ = expected_output_duration;

    let output_path = Path::new(&output_path_str);
    if output_path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(super::faststart::needs_faststart)
    {
        emit_export_state(&app, ExportStateEvent::finalizing(&export_id));
        if let Err(e) = super::faststart::apply(output_path) {
            log::warn!("faststart remux skipped, shipping moov-at-end: {e}");
        }
    }

    // The frontend transitions on `export-done`, decoupled from the `exportVideo` Promise, which resolves a beat later through IPC.
    emit_export_state(&app, ExportStateEvent::progress(&export_id, 100.0_f64));
    emit_export_state(
        &app,
        ExportStateEvent::success(&export_id, &output_path_str),
    );
    log::info!(
        "export: success emitted at T+{}ms for {output_path_str}",
        encode_started_at.elapsed().as_millis()
    );
    Ok(output_path_str)
}

#[cfg(test)]
mod tests {
    use super::{is_ffmpeg_crash_code, parse_ffmpeg_exit_code};

    #[test]
    fn crash_codes_are_out_of_the_normal_exit_range() {
        assert!(is_ffmpeg_crash_code(-1073741819)); // 0xC0000005 access violation
        assert!(is_ffmpeg_crash_code(-1));
        assert!(is_ffmpeg_crash_code(256));
        assert!(!is_ffmpeg_crash_code(0));
        assert!(!is_ffmpeg_crash_code(1)); // a normal ffmpeg error, not a crash
        assert!(!is_ffmpeg_crash_code(255));
    }

    #[test]
    fn parses_exit_code_from_the_error_message() {
        assert_eq!(
            parse_ffmpeg_exit_code("export failed (ffmpeg exit -1073741819):\nboom"),
            Some(-1073741819)
        );
        assert_eq!(
            parse_ffmpeg_exit_code("export failed (ffmpeg exit 1):\nx"),
            Some(1)
        );
        assert_eq!(
            parse_ffmpeg_exit_code("export timed out: no progress"),
            None
        );
    }
}
