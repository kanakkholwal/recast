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

    // Log the full invocation so a crash (ffmpeg segfaults print nothing to
    // stderr) is still diagnosable — this is the only record of which encoder /
    // filters / inputs were in play when it died.
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

    // Shared state consumed by the stderr parser (progress events) and the
    // watchdog (stall detection).
    let last_progress = Arc::new(Mutex::new(Instant::now()));
    let last_progress_secs = Arc::new(Mutex::new(-1.0_f64));
    let killed_by_timeout = Arc::new(AtomicBool::new(false));
    let killed_by_user = Arc::new(AtomicBool::new(false));
    let finalizing_seen = Arc::new(AtomicBool::new(false));
    let near_end_seen = Arc::new(AtomicBool::new(false));
    let progress_end_seen = Arc::new(AtomicBool::new(false));
    // Latched the first time the stderr parser parses a progress block.
    // The watchdog uses this to apply a longer budget during ffmpeg's
    // cold-start window (filter_complex parse, NVENC surface alloc, VP9
    // first-pass init) before falling back to the tighter steady-state
    // timeout once frames start flowing.
    let first_progress_seen = Arc::new(AtomicBool::new(false));

    // Parse stderr line-by-line. Progress blocks (key=value lines) get
    // filtered out; only genuine log output is appended to the 8 KB error
    // ring buffer used for post-mortem in the failure path. `out_time_us=`
    // lines drive the UI `export-progress` emits, and `progress=end`
    // signals the encoder has finished and only the mux trailer remains.
    let stderr_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let stderr_buf_writer = stderr_buf.clone();
    // Diagnostic lines are kept SEPARATELY from the rotating tail above.
    // FFmpeg names the cause while opening its inputs and then prints
    // kilobytes of stream listings and shutdown noise, so on any real export
    // the cause had already been drained out of the 8 KB tail by the time the
    // run failed — every failure reported its own epilogue instead.
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
            // Read raw bytes per line and lossy-decode, rather than `.lines()`
            // (which yields `Result<String>` and stops at the FIRST non-UTF-8 line
            // — silently dropping any real error printed after it, so the failure
            // path then reports "no detailed error").
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
                // FFmpeg progress blocks are key=value lines terminated by
                // `progress=continue` (between blocks) or `progress=end`
                // (final block). Treat all of these as non-log noise.
                if let Some(progress_secs) = parse_ffmpeg_progress_seconds(&line) {
                    let effective_duration = expected_output_secs;
                    // Watchdog proof-of-life: any parseable progress line
                    // means ffmpeg is alive. Don't gate this on out_time
                    // advancing — on Windows/NVENC we regularly see
                    // back-to-back blocks with unchanged `out_time_us`
                    // while surfaces flush or a GOP is primed, and
                    // waiting for advancement starved the watchdog reset.
                    {
                        let mut guard = stderr_last_progress.lock();
                        *guard = Instant::now();
                    }
                    // First progress line ever → flip the startup-grace
                    // flag and log it so post-mortems can see how long
                    // filter_complex/NVENC warmup took.
                    if !stderr_first_progress_seen.swap(true, Ordering::AcqRel) {
                        log::info!(
                            "export: first progress parsed at T+{}ms",
                            encode_started_at.elapsed().as_millis()
                        );
                    }
                    // UI emit gate: only publish a new pct when out_time
                    // actually advanced. Redundant emits would spam the
                    // progress bar with the same value.
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
                    // Log the moment we cross 99.5% so post-mortems of
                    // "stuck at 99%" reports can locate the gap between
                    // here and the eventual `progress=end` / drain-thread
                    // exit in the captured stderr tail.
                    if !logged_near_done && pct >= 99.5 {
                        logged_near_done = true;
                        log::info!(
                            "export: reached {:.1}% at T+{}ms, awaiting progress=end",
                            pct,
                            encode_started_at.elapsed().as_millis()
                        );
                    }
                    // For 2-pass GIF the pre-pass owns 0..40% and this
                    // pass owns 40..100%; for everything else it's 0..100.
                    // Scaling here (vs. at every progress emit site) keeps
                    // the 100% terminal emits below honest — they always
                    // mean "done", not "60% done because we're in pass 2".
                    let scaled_pct = progress_band.at(pct);
                    emit_export_state(
                        &stderr_app,
                        ExportStateEvent::progress(&stderr_export_id, scaled_pct),
                    );
                    continue;
                }
                // `progress=end` means FFmpeg has finished encoding and
                // is about to write the container trailer / exit. Flip
                // the UI to finalizing NOW rather than waiting for the
                // pipes to close — on Windows stderr close can lag the
                // actual encoder finish by seconds, which manifested as
                // the bar sitting at 100% with no state change. Also
                // stamp `last_progress` so the watchdog gives the trailer
                // write its own fresh budget.
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
                // Everything else is real log output. Capture anything that
                // names a cause first, so it survives regardless of where in
                // the stream it appeared.
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

    // Stdout carries nothing useful now that progress is on stderr, but we
    // still need to drain it — closing or ignoring the pipe can cause
    // FFmpeg to hit EPIPE on any stray write (e.g. `-report`) and abort.
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

    // Spawn the watchdog thread — narrow responsibility: only kill the
    // child if it stops producing progress for >60s (genuine stall) OR if
    // the user-facing cancel flag flips. Previous versions also auto-
    // emitted `export-finalizing` when progress went quiet for 1.5s, but
    // that fired falsely on Windows when FFmpeg's pipe buffering batched
    // progress into multi-second bursts, flipping the UI to "Finalizing"
    // mid-encode and leaving it there. Finalization is now reserved for
    // FFmpeg's explicit `progress=end` signal.
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
                // Startup grace: ffmpeg can take a long time to emit its
                // first progress block when filter_complex parsing, NVENC
                // surface allocation, or VP9 first-pass init runs before
                // the first frame is output. Use a bigger budget until
                // that first progress line arrives, then fall back to
                // ENCODE_TIMEOUT for steady state.
                const FIRST_PROGRESS_TIMEOUT: Duration = Duration::from_secs(120);
                // `FINALIZING_TIMEOUT` is a *no-file-growth* bound, not a
                // wall-clock cap on the finalizing phase. While FFmpeg is
                // legitimately writing the mux trailer the output file grows
                // continuously — we watch for that below and stamp
                // `watchdog_last_progress` on every size increase, so slow-
                // but-productive trailer writes keep us out of the timeout.
                // 60s of *no growth whatsoever* is a real stall.
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
                    // File-size growth as a liveness signal. Applies in both
                    // phases: during the encode the output file is already
                    // being written as GOPs complete, and during finalizing
                    // the trailer mux continues to grow the file. If the
                    // file is growing we know ffmpeg is alive and productive,
                    // regardless of whether the stderr progress thread has
                    // been able to refresh the stamp yet.
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
                                    "export watchdog: killing ffmpeg after progress=end at T+{}ms; no exit for {:?}",
                                    total_elapsed,
                                    elapsed
                                );
                            } else if near_end {
                                log::warn!(
                                    "export watchdog: killing ffmpeg near end of encode at T+{}ms; progress stopped for {:?}",
                                    total_elapsed,
                                    elapsed
                                );
                            } else {
                                log::warn!(
                                    "export watchdog: killing stalled ffmpeg at T+{}ms (no progress for {:?})",
                                    total_elapsed,
                                    elapsed
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

    // Wait for the I/O drain threads to finish. Both unblock when FFmpeg
    // closes its respective pipes, which happens as it's exiting.
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();
    log::info!(
        "export: drain threads joined at T+{}ms (pipes closed)",
        encode_started_at.elapsed().as_millis()
    );

    // Redundant-but-idempotent final emit: if `progress=end` wasn't seen
    // (e.g. FFmpeg was killed before finishing), make sure the UI still
    // gets a finalizing flip before `export-done` arrives so the dialog
    // has a consistent visual sequence.
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

    // Pipes are closed, which means ffmpeg has finished writing the file.
    // Probe the output NOW and, if it's usable, emit `success` to the UI
    // immediately — we should not make the user watch "Writing video
    // file…" while we wait for the OS to reap the child process. On
    // Windows that reap can legitimately take hundreds of ms to a couple
    // of seconds after stdio close. The reap still happens below, but
    // its only job now is to reap cleanly; its latency no longer blocks
    // the user-visible completion.
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

    // Pull the child back out and wait for its exit status. Stdout has
    // already closed, so FFmpeg should be on its last gasp (trailer write +
    // teardown). A well-behaved exit happens within milliseconds. We still
    // bound the wait with a hard timeout — if it takes longer than
    // POST_CLOSE_TIMEOUT we force-kill so the ffmpeg process doesn't leak.
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

    // If we already told the UI the export succeeded based on the probe
    // of a fully-written file, the reap outcome (clean exit or forced
    // kill) is bookkeeping — the file is good either way. Return Ok so
    // the caller's Promise resolves cleanly.
    if early_success_emitted {
        return Ok(output_path_str);
    }

    if forced_exit {
        let output_path = Path::new(&output_path_str);
        // Force-kill happens only after the I/O drain threads exited
        // (pipes already closed = FFmpeg finished writing) AND we waited
        // POST_CLOSE_TIMEOUT for the process to reap. If `progress_end`
        // was seen, the encoder definitely got through the trailer write
        // before this point — the salvage probe then confirms the file is
        // playable. Without `progress_end` we can't trust the output even
        // if probe succeeds; refuse rather than ship a corrupted file.
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
        // Clean up the half-written output file so the exports list doesn't
        // show a broken artifact from the aborted run.
        let _ = std::fs::remove_file(&output_path_str);
        emit_export_state(&app, ExportStateEvent::cancelled(&export_id));
        return Err("export cancelled".to_string());
    }

    if killed_by_timeout.load(Ordering::Acquire) {
        let output_path = Path::new(&output_path_str);
        // Salvage path: only trust the on-disk file if FFmpeg actually
        // signalled `progress=end` before the watchdog fired. That means
        // the encoder finished writing every frame and we killed it
        // partway through the trailer write — `completed_export_looks_usable`
        // can probe successfully on the partial mux result, but the moov
        // atom may be incomplete. Without `progress=end` we were killed
        // mid-encode and the output is almost certainly truncated;
        // refuse to surface a corrupted file as a successful export.
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
        // Surface whatever ffmpeg last said so this error is actionable
        // without needing to re-instrument. The stderr ring buffer holds
        // up to 8 KB; take the final line (or two) to keep the message
        // scannable.
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
        // The toast only gets a summary; keep the WHOLE invocation and stderr in
        // the log so a report that quotes the toast can still be traced back.
        // Logged at ERROR because release builds default to Warn, which would
        // drop the `info!` copy of the args written before the spawn.
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
        // Include the exit code: when stderr carries no diagnostic (e.g. ffmpeg was
        // killed by the OS, crashed, or aborted before logging), the code is the
        // only signal — a large Windows code like 3221225477 (0xC0000005) means a
        // crash, not a normal encode error.
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

    // Log stderr tail even on success so we can diagnose silent warnings
    // (e.g. mux trailer problems) that produce a "valid" exit code but a
    // broken file.
    let stderr_bytes = stderr_buf.lock().clone();
    if !stderr_bytes.is_empty() {
        let tail = String::from_utf8_lossy(&stderr_bytes);
        log::info!("export ffmpeg stderr tail: {tail}");
    }

    // On the happy path (status 0 + progress=end observed) we trust
    // FFmpeg's own exit as the integrity signal — spawning ffprobe here
    // just to re-verify what we already know would park the UI in
    // "Finalizing…" for the duration of that probe, which is exactly the
    // hang symptom users hit. Corruption guards remain on the salvage
    // paths above (force-kill, watchdog-kill) where the exit code isn't
    // trustworthy. `_expected_output_duration` kept in scope to make the
    // salvage branches' dependency explicit.
    let _ = expected_output_duration;

    // Final 100% ping + an `export-done` event with the result. The
    // frontend uses `export-done` to transition the dialog to the success
    // state immediately — decoupled from the `exportVideo` Promise, which
    // may take an extra beat to resolve through Tauri's IPC layer.
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
