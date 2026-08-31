use std::io::{BufRead, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tauri::AppHandle;

use super::cuts_speed::{
    build_cut_select_expr, build_speed_setpts_expr, collect_export_cuts, has_speed_change,
    resolve_speed_segments,
};
use super::progress::{is_ffmpeg_progress_key_line, parse_ffmpeg_progress_seconds, ProgressBand};
use super::state::{emit_export_state, ExportStateEvent};
use crate::commands::ffmpeg::{
    build_gif_palette_prepass_filter, build_gif_paletteuse_external_complex,
    summarize_ffmpeg_error, GifFilterOptions,
};
use crate::commands::types::GifSettings;
use crate::render::graph::RenderState;

/// Pass 1 of the 2-pass GIF export. Consumes the source at the GIF's target
/// fps + scale and writes a single palette PNG. The main encode pass then
/// reads that palette as an external input and runs paletteuse on every
/// frame, which streams in real time so the progress bar actually moves.
///
/// Single-pass `palettegen → paletteuse` was stalling the UI: palettegen has
/// to consume every input frame before emitting its one output, so the
/// encoder's `out_time_us` stayed at 0 the entire palette phase and the bar
/// sat at 0% while only the elapsed counter ticked.
// Many discrete inputs (paths, trim window, durations); bundling them into a struct wouldn't add clarity.
#[allow(clippy::too_many_arguments)]
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
    // Drop cut ranges before fps-resample and palettegen, so the palette is built only from kept frames.
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

    // Poll cancel_flag while waiting so a user cancel kills the palette pre-pass mid-run.
    let exit_status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {
                if cancel_flag.load(Ordering::Acquire) {
                    let _ = child.kill();
                    // Reap it: on Unix a killed child stays a zombie until waited on, so a cancelled GIF export leaked one each time.
                    let _ = child.wait();
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

/// Inputs for the 2-pass GIF pipeline. The current filter-complex accumulator is moved in and the updated one returned, so the caller stays the single owner of that state.
pub(crate) struct GifPassParams<'a> {
    pub app: &'a AppHandle,
    pub export_id: &'a str,
    pub cancel_flag: Arc<AtomicBool>,
    pub source_video: &'a Path,
    pub output_dir: &'a Path,
    pub output_scale_filter: Option<&'a str>,
    pub trim_start: f64,
    pub trim_end: f64,
    pub duration: f64,
    pub source_duration: f64,
    pub render_state: &'a RenderState,
    /// The editor's resolved kept-timeline, when the payload carried one.
    pub time_map: Option<&'a Vec<super::cuts_speed::TimeSpanWire>>,
    pub gif_settings: &'a GifSettings,
    /// The profile's default GIF fps (used when the settings don't override it).
    pub gif_fps: u32,
    /// Index the palette PNG will occupy in the FFmpeg input list.
    pub palette_input_index: usize,
    pub filter_complex: Option<String>,
    pub video_map: String,
}

/// Why the GIF pre-pass stopped short. The caller maps these to the shared
/// cancel/error emit + cancel-token cleanup so this stays UI-agnostic.
pub(crate) enum GifPassError {
    Cancelled,
    Failed(String),
}

/// Result of a successful GIF pre-pass: the palette input to splice into `args`,
/// the paletteuse-terminated filter graph, and the temp palette path to clean up.
pub(crate) struct GifPassOutput {
    pub palette_input_args: [String; 2],
    pub filter_complex: Option<String>,
    pub video_map: String,
    pub palette_temp_path: PathBuf,
}

/// Run pass 1 (palette generation off the main thread) and wire the paletteuse
/// pass. Extracted verbatim from `export_video`'s GIF branch; the only change is
/// that the three early error/cancel returns become typed [`GifPassError`]s and
/// the mutated `args`/filter state flow through [`GifPassOutput`].
pub(crate) async fn run_gif_pass(p: GifPassParams<'_>) -> Result<GifPassOutput, GifPassError> {
    let resolved_fps = p.gif_settings.fps.unwrap_or(p.gif_fps);
    let gif_max_colors = p.gif_settings.max_colors();
    // `GifFilterOptions` holds a `&str`, so stash the owned String and rebuild the struct inside each `'static` closure.
    let gif_dither_owned: String = p.gif_settings.dither.clone();

    // Unique per run so concurrent exports don't clobber each other's palette.
    let palette_stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let palette_path = p.output_dir.join(format!(
        "recast_palette_{palette_stamp}_{}.png",
        std::process::id()
    ));

    // GIF's two-pass palette runs before the generic cut and speed stage, so build the same select+setpts warp for both passes.
    let gif_cut_select: Option<String> = {
        let export_cuts = collect_export_cuts(p.render_state, p.trim_start, p.trim_end);
        let gif_speed_segments = resolve_speed_segments(
            p.time_map,
            p.duration,
            &export_cuts,
            &p.render_state.split_points,
            &p.render_state.segment_speeds,
            p.trim_start,
        );
        let gif_speed_active = has_speed_change(&gif_speed_segments);
        let has_cuts = !export_cuts.is_empty();
        (has_cuts || gif_speed_active).then(|| {
            let select_prefix = if has_cuts {
                format!("select='{}',", build_cut_select_expr(&export_cuts))
            } else {
                String::new()
            };
            let setpts = if gif_speed_active {
                // Single-quote: the warp expression's commas would otherwise read as filtergraph separators.
                format!(
                    "setpts='({})/TB'",
                    build_speed_setpts_expr(&gif_speed_segments)
                )
            } else {
                "setpts=N/FRAME_RATE/TB".to_string()
            };
            format!("{select_prefix}{setpts}")
        })
    };

    let cut_select_for_prepass = gif_cut_select.clone();
    let app_for_prepass = p.app.clone();
    let export_id_for_prepass = p.export_id.to_string();
    let source_for_prepass = p.source_video.to_path_buf();
    let palette_for_prepass = palette_path.clone();
    let cancel_for_prepass = p.cancel_flag.clone();
    let scale_for_prepass = p.output_scale_filter.map(|s| s.to_string());
    let dither_for_prepass = gif_dither_owned.clone();
    let trim_start = p.trim_start;
    let duration = p.duration;
    let source_duration = p.source_duration;
    let prepass_result = tokio::task::spawn_blocking(move || {
        let inner_options = GifFilterOptions {
            fps: resolved_fps,
            max_colors: gif_max_colors,
            dither: dither_for_prepass.as_str(),
        };
        run_gif_palette_prepass(
            &app_for_prepass,
            &export_id_for_prepass,
            &source_for_prepass,
            &palette_for_prepass,
            trim_start,
            duration,
            source_duration,
            inner_options,
            scale_for_prepass.as_deref(),
            cut_select_for_prepass.as_deref(),
            cancel_for_prepass,
            ProgressBand {
                offset: 0.0,
                scale: 0.4,
            },
        )
    })
    .await;

    match prepass_result {
        Ok(Ok(())) => {}
        Ok(Err(err_msg)) => {
            let _ = std::fs::remove_file(&palette_path);
            if p.cancel_flag.load(Ordering::Acquire) {
                return Err(GifPassError::Cancelled);
            }
            return Err(GifPassError::Failed(err_msg));
        }
        Err(join_err) => {
            let _ = std::fs::remove_file(&palette_path);
            return Err(GifPassError::Failed(format!(
                "export task failed (palette pre-pass): {join_err}"
            )));
        }
    }

    if p.cancel_flag.load(Ordering::Acquire) {
        let _ = std::fs::remove_file(&palette_path);
        return Err(GifPassError::Cancelled);
    }

    // Palette is the LAST input: GIF skips audio, so ordering is source, extra_inputs, cursor, then palette.
    let palette_input_args = ["-i".to_string(), palette_path.to_string_lossy().to_string()];

    // Drop cut ranges before palette-use so removed frames never reach the GIF; the generic cut stage is MP4 and WebM only.
    let mut filter_complex = p.filter_complex;
    let mut video_map = p.video_map;
    if let Some(ref cs) = gif_cut_select {
        let (mut complex, vlabel) = match filter_complex.take() {
            Some(existing) => (existing, video_map.clone()),
            None => ("[0:v]".to_string(), "[0:v]".to_string()),
        };
        if !complex.is_empty() && !complex.ends_with(';') && !vlabel.is_empty() {
            complex.push(';');
        }
        complex.push_str(&vlabel);
        complex.push_str(&format!("{cs}[vgifcut]"));
        filter_complex = Some(complex);
        video_map = "[vgifcut]".to_string();
    }

    let pass2_options = GifFilterOptions {
        fps: resolved_fps,
        max_colors: gif_max_colors,
        dither: gif_dither_owned.as_str(),
    };
    let (gif_complex, gif_map) = build_gif_paletteuse_external_complex(
        filter_complex.as_deref(),
        &video_map,
        p.palette_input_index,
        pass2_options,
        p.output_scale_filter,
    );

    Ok(GifPassOutput {
        palette_input_args,
        filter_complex: Some(gif_complex),
        video_map: gif_map,
        palette_temp_path: palette_path,
    })
}
