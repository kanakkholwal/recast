use std::io::{BufRead, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tauri::AppHandle;

use super::progress::{is_ffmpeg_progress_key_line, parse_ffmpeg_progress_seconds, ProgressBand};
use super::state::{emit_export_state, ExportStateEvent};
use crate::commands::ffmpeg::{
    build_gif_palette_prepass_filter, summarize_ffmpeg_error, GifFilterOptions,
};

/// Pass 1 of the 2-pass GIF export. Consumes the source at the GIF's target
/// fps + scale and writes a single palette PNG. The main encode pass then
/// reads that palette as an external input and runs paletteuse on every
/// frame, which streams in real time so the progress bar actually moves.
///
/// Single-pass `palettegen → paletteuse` was stalling the UI: palettegen has
/// to consume every input frame before emitting its one output, so the
/// encoder's `out_time_us` stayed at 0 the entire palette phase and the bar
/// sat at 0% while only the elapsed counter ticked.
pub(crate) fn run_gif_palette_prepass(
    app: &AppHandle,
    export_id: &str,
    source_path: &Path,
    palette_path: &Path,
    trim_start: f64,
    duration: f64,
    source_duration: f64,
    options: GifFilterOptions<'_>,
    output_scale_filter: Option<&str>,
    cut_select: Option<&str>,
    cancel_flag: Arc<AtomicBool>,
    band: ProgressBand,
) -> Result<(), String> {
    let mut args: Vec<String> = vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-progress".to_string(),
        "pipe:2".to_string(),
        "-stats_period".to_string(),
        "0.1".to_string(),
    ];
    if trim_start > 0.0 {
        args.extend(["-ss".to_string(), format!("{trim_start:.3}")]);
    }
    if duration > 0.0 {
        args.extend(["-t".to_string(), format!("{duration:.3}")]);
    }
    args.extend(["-i".to_string(), source_path.to_string_lossy().to_string()]);

    let base_vf = build_gif_palette_prepass_filter(options, output_scale_filter);
    // Drop cut ranges and close the gaps before fps-resample + palettegen, so
    // the palette is built only from kept frames.
    let vf = match cut_select {
        Some(cs) if !cs.is_empty() => format!("{cs},{base_vf}"),
        _ => base_vf,
    };
    args.extend([
        "-vf".to_string(),
        vf,
        "-frames:v".to_string(),
        "1".to_string(),
        "-an".to_string(),
        palette_path.to_string_lossy().to_string(),
    ]);

    log::info!("export gif palette pre-pass args: {}", args.join(" "));

    let mut command = Command::new(crate::ffmpeg::ffmpeg_path());
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::ffmpeg::configure_silent_command(&mut command);

    let mut child = command
        .spawn()
        .map_err(|e| format!("failed to start ffmpeg palette pre-pass: {e}"))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "ffmpeg palette stdout pipe missing".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "ffmpeg palette stderr pipe missing".to_string())?;

    let stderr_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let stderr_buf_writer = stderr_buf.clone();
    let app_for_emit = app.clone();
    let export_id_for_emit = export_id.to_string();
    let effective_duration = if duration > 0.0 {
        duration
    } else {
        source_duration
    };

    let stderr_thread = std::thread::Builder::new()
        .name("recast-export-palette-stderr".into())
        .spawn(move || {
            let reader = std::io::BufReader::new(stderr);
            let mut last_emitted = -1.0_f64;
            for line in reader.lines().map_while(Result::ok) {
                if let Some(progress_secs) = parse_ffmpeg_progress_seconds(&line) {
                    if effective_duration > 0.0 {
                        let raw_pct =
                            (progress_secs / effective_duration * 100.0).clamp(0.0, 100.0);
                        let scaled = band.at(raw_pct);
                        if scaled > last_emitted + 0.5 {
                            last_emitted = scaled;
                            emit_export_state(
                                &app_for_emit,
                                ExportStateEvent::progress(&export_id_for_emit, scaled),
                            );
                        }
                    }
                    continue;
                }
                if line.trim() == "progress=end" || is_ffmpeg_progress_key_line(&line) {
                    continue;
                }
                let mut guard = stderr_buf_writer.lock();
                guard.extend_from_slice(line.as_bytes());
                guard.push(b'\n');
                if guard.len() > 8192 {
                    let overflow = guard.len() - 8192;
                    guard.drain(0..overflow);
                }
            }
        })
        .map_err(|e| format!("failed to spawn palette stderr drain: {e}"))?;

    let stdout_thread = std::thread::Builder::new()
        .name("recast-export-palette-stdout".into())
        .spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match stdout.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
        })
        .map_err(|e| format!("failed to spawn palette stdout drain: {e}"))?;

    // Poll cancel_flag while waiting for the child so a user cancel kills the
    // palette pre-pass mid-run instead of waiting for it to finish first.
    let exit_status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {
                if cancel_flag.load(Ordering::Acquire) {
                    let _ = child.kill();
                    break Err("export cancelled".to_string());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => break Err(format!("ffmpeg palette wait error: {e}")),
        }
    };

    let _ = stderr_thread.join();
    let _ = stdout_thread.join();

    match exit_status {
        Ok(status) => {
            if !status.success() {
                let stderr_bytes = stderr_buf.lock().clone();
                return Err(format!(
                    "export failed (palette pre-pass):\n{}",
                    summarize_ffmpeg_error(&stderr_bytes)
                ));
            }
            match std::fs::metadata(palette_path) {
                Ok(meta) if meta.len() > 0 => Ok(()),
                Ok(_) => Err("export failed: palette pre-pass wrote empty file".into()),
                Err(e) => Err(format!(
                    "export failed: palette pre-pass output missing: {e}"
                )),
            }
        }
        Err(e) => Err(e),
    }
}
