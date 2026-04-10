use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use base64::{Engine as _, engine::general_purpose};
use tauri::{AppHandle, Emitter, State};

use super::ffmpeg::{
    append_output_filters_to_complex, build_output_scale_filter, has_audio, probe_video_metadata,
    resolve_export_profile, summarize_ffmpeg_error,
};
use super::system::get_active_output_dir;
use super::types::{AppState, EditorDocument, ExportRequest, VideoMetadata};
use crate::project::reader::ProjectOpenResult;
use crate::render::graph::{RenderGraph, RenderState, SourceVideoMetadata};

fn static_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let candidate = cwd.join("..").join("static");
    if candidate.exists() {
        candidate
    } else {
        cwd.join("static")
    }
}

fn open_project_if_needed(path: &Path) -> Result<Option<ProjectOpenResult>, String> {
    if path.extension().and_then(|value| value.to_str()) == Some("recast") {
        crate::project::reader::open_project(path)
            .map(Some)
            .map_err(|e| e.to_string())
    } else {
        Ok(None)
    }
}

fn project_or_media_metadata(path: &Path) -> Result<VideoMetadata, String> {
    if path.extension().and_then(|value| value.to_str()) == Some("recast") {
        let project = crate::project::reader::open_project(path).map_err(|e| e.to_string())?;
        return Ok(VideoMetadata {
            duration: project.metadata.video.duration_ms as f64 / 1000.0,
            width: project.metadata.video.width,
            height: project.metadata.video.height,
            fps: project.metadata.video.fps as f64,
            codec: "h264".into(),
            size_bytes: fs::metadata(path).map(|m| m.len()).unwrap_or_default(),
        });
    }
    probe_video_metadata(path)
}

#[tauri::command]
pub fn get_video_metadata(path: String) -> Result<VideoMetadata, String> {
    project_or_media_metadata(Path::new(&path))
}

#[tauri::command]
pub fn load_editor_document(path: String) -> Result<EditorDocument, String> {
    let input = PathBuf::from(&path);
    if let Some(project) = open_project_if_needed(&input)? {
        let render_state = fs::read_to_string(&project.edits_path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_else(|| RenderState {
                trim_end: project.metadata.video.duration_ms as f64 / 1000.0,
                ..RenderState::default()
            });

        return Ok(EditorDocument {
            project_path: path,
            media_path: project.recording_path.to_string_lossy().to_string(),
            cursor_path: Some(project.cursor_path.to_string_lossy().to_string()),
            edits_path: Some(project.edits_path.to_string_lossy().to_string()),
            audio_path: project.audio_path.map(|p| p.to_string_lossy().to_string()),
            microphone_path: project.microphone_path.map(|p| p.to_string_lossy().to_string()),
            camera_path: project.camera_path.map(|p| p.to_string_lossy().to_string()),
            metadata: VideoMetadata {
                duration: project.metadata.video.duration_ms as f64 / 1000.0,
                width: project.metadata.video.width,
                height: project.metadata.video.height,
                fps: project.metadata.video.fps as f64,
                codec: "h264".into(),
                size_bytes: fs::metadata(&input).map(|m| m.len()).unwrap_or_default(),
            },
            render_state,
        });
    }

    let metadata = probe_video_metadata(&input)?;
    Ok(EditorDocument {
        project_path: path.clone(),
        media_path: path,
        cursor_path: None,
        edits_path: None,
        audio_path: None,
        microphone_path: None,
        camera_path: None,
        metadata: metadata.clone(),
        render_state: RenderState {
            trim_end: metadata.duration,
            ..RenderState::default()
        },
    })
}

#[tauri::command]
pub fn generate_thumbnails(path: String, count: u32) -> Result<Vec<String>, String> {
    let input = PathBuf::from(&path);
    let project = open_project_if_needed(&input)?;
    let media_path = project
        .as_ref()
        .map(|value| value.recording_path.clone())
        .unwrap_or(input);
    let meta = probe_video_metadata(&media_path)?;
    if meta.duration <= 0.0 || count == 0 {
        return Ok(Vec::new());
    }

    let interval = meta.duration / count as f64;
    let temp_dir = std::env::temp_dir().join("recast-thumbnails");
    let _ = fs::create_dir_all(&temp_dir);
    let mut thumbnails = Vec::new();

    let scale_width = if count <= 2 { 480 } else { 240 };

    for index in 0..count {
        let timestamp = index as f64 * interval;
        let thumb_path = temp_dir.join(format!("thumb-{index}.jpg"));
        let result = Command::new(crate::ffmpeg::ffmpeg_path())
            .args([
                "-y",
                "-ss",
                &format!("{timestamp:.2}"),
                "-i",
                &media_path.to_string_lossy(),
                "-vframes",
                "1",
                "-vf",
                &format!("scale={scale_width}:-1"),
                "-q:v",
                "4",
                thumb_path.to_string_lossy().as_ref(),
            ])
            .output();

        if let Ok(output) = result {
            if output.status.success() {
                if let Ok(data) = fs::read(&thumb_path) {
                    thumbnails.push(format!(
                        "data:image/jpeg;base64,{}",
                        general_purpose::STANDARD.encode(data)
                    ));
                }
            }
        }
        let _ = fs::remove_file(&thumb_path);
    }

    Ok(thumbnails)
}

#[tauri::command]
pub async fn export_video(
    app: AppHandle,
    request: ExportRequest,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let input_path = PathBuf::from(&request.input_path);
    let project = open_project_if_needed(&input_path)?;
    let source_video = project
        .as_ref()
        .map(|value| value.recording_path.clone())
        .unwrap_or_else(|| input_path.clone());
    let metadata = probe_video_metadata(&source_video)?;
    if metadata.width == 0 || metadata.height == 0 {
        return Err("export failed: source video metadata is incomplete".into());
    }
    let graph = RenderGraph::from_state(&request.render_state);
    let (trim_start, trim_end) = graph.trim_range();
    let duration = (trim_end - trim_start).max(0.0);
    let profile = resolve_export_profile(&request.quality);
    let output_scale_filter = build_output_scale_filter(profile);
    let output_dir = get_active_output_dir(&state).join("exports");
    let _ = std::fs::create_dir_all(&output_dir);
    let extension = match request.format.as_str() {
        "gif" => "gif",
        "webm" => "webm",
        _ => "mp4",
    };
    let output_path = output_dir.join(format!(
        "recast_export_{}.{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        extension
    ));

    let export_plan = graph
        .build_export_plan(
            SourceVideoMetadata {
                width: metadata.width,
                height: metadata.height,
            },
            &static_root(),
            1,
        )
        .map_err(|e| e.to_string())?;

    let mut args = vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
    ];
    if trim_start > 0.0 {
        args.extend(["-ss".to_string(), format!("{trim_start:.3}")]);
    }
    if duration > 0.0 {
        args.extend(["-t".to_string(), format!("{duration:.3}")]);
    }
    args.extend(["-i".to_string(), source_video.to_string_lossy().to_string()]);

    for input in &export_plan.extra_inputs {
        args.extend([
            "-loop".to_string(),
            "1".to_string(),
            "-i".to_string(),
            input.to_string_lossy().to_string(),
        ]);
    }

    if let Some(filter_complex) = &export_plan.filter_complex {
        args.extend([
            "-filter_complex".to_string(),
            filter_complex.clone(),
            "-map".to_string(),
            export_plan.video_map.clone(),
        ]);
    } else {
        args.extend(["-map".to_string(), "0:v:0".to_string()]);
    }

    let has_source_audio = has_audio(&source_video) && request.format != "gif";
    if has_source_audio {
        args.extend(["-map".to_string(), "0:a?".to_string()]);
    }

    let mut output_filters = Vec::new();
    if request.format == "gif" {
        output_filters.push(format!("fps={}", profile.gif_fps));
    }
    if let Some(scale_filter) = output_scale_filter {
        output_filters.push(scale_filter);
    }
    if !output_filters.is_empty() && export_plan.filter_complex.is_none() {
        args.extend(["-vf".to_string(), output_filters.join(",")]);
    }

    if !export_plan.extra_inputs.is_empty() {
        args.push("-shortest".to_string());
    }

    match request.format.as_str() {
        "gif" => {
            args.extend([
                "-an".to_string(),
                "-loop".to_string(),
                "0".to_string(),
                output_path.to_string_lossy().to_string(),
            ]);
        }
        "webm" => {
            args.extend([
                "-c:v".to_string(),
                "libvpx-vp9".to_string(),
                "-crf".to_string(),
                profile.webm_crf.to_string(),
                "-b:v".to_string(),
                "0".to_string(),
            ]);
            if has_source_audio {
                args.extend(["-c:a".to_string(), "libopus".to_string()]);
            } else {
                args.push("-an".to_string());
            }
            args.push(output_path.to_string_lossy().to_string());
        }
        _ => {
            args.extend([
                "-c:v".to_string(),
                "libx264".to_string(),
                "-preset".to_string(),
                profile.mp4_preset.to_string(),
                "-crf".to_string(),
                profile.mp4_crf.to_string(),
                "-pix_fmt".to_string(),
                "yuv420p".to_string(),
                "-movflags".to_string(),
                "+faststart".to_string(),
            ]);
            if has_source_audio {
                args.extend([
                    "-c:a".to_string(),
                    "aac".to_string(),
                    "-b:a".to_string(),
                    "192k".to_string(),
                ]);
            } else {
                args.push("-an".to_string());
            }
            args.push(output_path.to_string_lossy().to_string());
        }
    }

    if !output_filters.is_empty() && export_plan.filter_complex.is_some() {
        let (complex_filter, map_label) = append_output_filters_to_complex(
            export_plan.filter_complex.as_deref().unwrap_or_default(),
            &export_plan.video_map,
            &output_filters,
        );

        let filter_index = args
            .iter()
            .position(|arg| arg == "-filter_complex")
            .and_then(|index| args.get_mut(index + 1));
        if let Some(slot) = filter_index {
            *slot = complex_filter;
        }

        let map_index = args
            .iter()
            .position(|arg| arg == "-map")
            .and_then(|index| args.get_mut(index + 1));
        if let Some(slot) = map_index {
            *slot = map_label;
        }
    }

    // Add progress reporting: FFmpeg writes progress to stderr with -progress pipe:1
    args.extend(["-progress".to_string(), "pipe:1".to_string()]);

    let output_path_str = output_path.to_string_lossy().to_string();
    let output_path_clone = output_path_str.clone();

    // Spawn FFmpeg in a background thread so the UI stays responsive
    let result = tokio::task::spawn_blocking(move || {
        let mut child = Command::new(crate::ffmpeg::ffmpeg_path())
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to start ffmpeg: {e}"))?;

        // Read stdout for progress updates (FFmpeg -progress pipe:1)
        if let Some(stdout) = child.stdout.take() {
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if let Some(time_str) = line.strip_prefix("out_time_us=") {
                    if let Ok(time_us) = time_str.trim().parse::<f64>() {
                        let progress_secs = time_us / 1_000_000.0;
                        let pct = if duration > 0.0 {
                            (progress_secs / duration * 100.0).clamp(0.0, 100.0)
                        } else {
                            0.0
                        };
                        let _ = app.emit("export-progress", pct);
                    }
                }
            }
        }

        let output = child.wait_with_output().map_err(|e| e.to_string())?;

        if !output.status.success() {
            return Err(format!("export failed:\n{}", summarize_ffmpeg_error(&output.stderr)));
        }

        let _ = app.emit("export-progress", 100.0_f64);
        Ok(output_path_clone)
    })
    .await
    .map_err(|e| format!("export task failed: {e}"))?;

    result
}

#[tauri::command]
pub fn autosave_project(project_path: String, edits_json: String) -> Result<(), String> {
    crate::project::autosave::save_autosave(
        Path::new(&project_path),
        &edits_json,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_autosave(project_path: String) {
    crate::project::autosave::clear_autosave(Path::new(&project_path));
}

#[tauri::command]
pub fn get_recoverable_sessions() -> Vec<crate::project::autosave::AutosaveState> {
    crate::project::autosave::find_recoverable_sessions()
}
