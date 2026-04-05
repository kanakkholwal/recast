mod project;
mod recording;
mod render;

use std::env;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;

use base64::{Engine as _, engine::general_purpose};
use image::codecs::png::PngEncoder;
use image::{ColorType, ImageEncoder};
use parking_lot::Mutex;
use project::ProjectMetadata;
use project::reader::ProjectOpenResult;
use project::writer::{ProjectWriteRequest, write_project};
use recording::{CaptureTarget, RecordingManager};
use render::graph::{RenderGraph, RenderState, SourceVideoMetadata};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use xcap::{Monitor, Window};

const THUMBNAIL_WIDTH: u32 = 320;
const THUMBNAIL_HEIGHT: u32 = 180;

#[derive(Serialize, Clone)]
pub struct DisplayInfo {
    id: u32,
    name: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    is_primary: bool,
    thumbnail: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct WindowInfo {
    id: u32,
    pid: u32,
    app_name: String,
    title: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    is_minimized: bool,
    thumbnail: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct RecordingEntry {
    filename: String,
    path: String,
    size_bytes: u64,
    created: u64,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct VideoMetadata {
    duration: f64,
    width: u32,
    height: u32,
    fps: f64,
    codec: String,
    size_bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EditorDocument {
    project_path: String,
    media_path: String,
    cursor_path: Option<String>,
    edits_path: Option<String>,
    metadata: VideoMetadata,
    render_state: RenderState,
}

#[derive(Serialize, Deserialize, Clone)]
struct AppConfig {
    output_dir: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self { output_dir: None }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportRequest {
    input_path: String,
    format: String,
    quality: String,
    render_state: RenderState,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewFrameRequest {
    input_path: String,
    time: f64,
    render_state: RenderState,
}

#[derive(Clone, Copy)]
struct ExportProfile {
    max_width: Option<u32>,
    max_height: Option<u32>,
    mp4_crf: u32,
    mp4_preset: &'static str,
    webm_crf: u32,
    gif_fps: u32,
}

struct AppState {
    recording_manager: RecordingManager,
    last_file_path: Mutex<Option<String>>,
    config: Mutex<AppConfig>,
}

fn run_preview_ffmpeg(args: &[String]) -> Result<String, String> {
    let output = Command::new("ffmpeg")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() || output.stdout.is_empty() {
        return Err(format!(
            "preview render failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!(
        "data:image/png;base64,{}",
        general_purpose::STANDARD.encode(output.stdout)
    ))
}

fn resolve_export_profile(quality: &str) -> ExportProfile {
    match quality {
        "small" => ExportProfile {
            max_width: Some(1280),
            max_height: Some(720),
            mp4_crf: 28,
            mp4_preset: "veryfast",
            webm_crf: 34,
            gif_fps: 12,
        },
        "4k" => ExportProfile {
            max_width: Some(3840),
            max_height: Some(2160),
            mp4_crf: 18,
            mp4_preset: "slow",
            webm_crf: 24,
            gif_fps: 18,
        },
        "source" => ExportProfile {
            max_width: None,
            max_height: None,
            mp4_crf: 20,
            mp4_preset: "slow",
            webm_crf: 28,
            gif_fps: 18,
        },
        _ => ExportProfile {
            max_width: Some(1920),
            max_height: Some(1080),
            mp4_crf: 22,
            mp4_preset: "medium",
            webm_crf: 30,
            gif_fps: 15,
        },
    }
}

fn build_output_scale_filter(profile: ExportProfile) -> Option<String> {
    match (profile.max_width, profile.max_height) {
        (Some(max_width), Some(max_height)) => Some(format!(
            "scale=w='min(iw,{max_width})':h='min(ih,{max_height})':force_original_aspect_ratio=decrease:flags=lanczos"
        )),
        _ => None,
    }
}

fn append_output_filters_to_complex(
    filter_complex: &str,
    input_label: &str,
    filters: &[String],
) -> (String, String) {
    let final_label = "[vfinal]";
    let normalized_input = if input_label.starts_with('[') {
        input_label.to_string()
    } else {
        format!("[{input_label}]")
    };

    (
        format!(
            "{filter_complex};{normalized_input}{}{final_label}",
            filters.join(",")
        ),
        final_label.to_string(),
    )
}

fn summarize_ffmpeg_error(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();

    if lines.is_empty() {
        "FFmpeg failed without returning a detailed error.".into()
    } else {
        lines
            .iter()
            .rev()
            .take(8)
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn config_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| env::temp_dir())
        .join("recast_config.json")
}

fn load_config(app: &AppHandle) -> AppConfig {
    let path = config_path(app);
    if let Ok(data) = fs::read_to_string(&path) {
        if let Ok(config) = serde_json::from_str(&data) {
            return config;
        }
    }
    AppConfig::default()
}

fn save_config(app: &AppHandle, config: &AppConfig) {
    let path = config_path(app);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(data) = serde_json::to_string_pretty(config) {
        let _ = fs::write(path, data);
    }
}

fn get_active_output_dir(state: &State<'_, AppState>) -> PathBuf {
    let config = state.config.lock();
    if let Some(dir) = &config.output_dir {
        PathBuf::from(dir)
    } else {
        env::temp_dir()
    }
}

fn static_root() -> PathBuf {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let candidate = cwd.join("..").join("static");
    if candidate.exists() {
        candidate
    } else {
        cwd.join("static")
    }
}

fn make_thumbnail(img: &image::RgbaImage) -> image::RgbaImage {
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return image::RgbaImage::from_pixel(
            THUMBNAIL_WIDTH,
            THUMBNAIL_HEIGHT,
            image::Rgba([0, 0, 0, 255]),
        );
    }

    let scale = (THUMBNAIL_WIDTH as f32 / w as f32)
        .min(THUMBNAIL_HEIGHT as f32 / h as f32)
        .max(f32::MIN_POSITIVE);
    let scaled_w = (w as f32 * scale)
        .round()
        .clamp(1.0, THUMBNAIL_WIDTH as f32) as u32;
    let scaled_h = (h as f32 * scale)
        .round()
        .clamp(1.0, THUMBNAIL_HEIGHT as f32) as u32;
    let resized =
        image::imageops::resize(img, scaled_w, scaled_h, image::imageops::FilterType::Triangle);
    let mut canvas = image::RgbaImage::from_pixel(
        THUMBNAIL_WIDTH,
        THUMBNAIL_HEIGHT,
        image::Rgba([18, 18, 20, 255]),
    );
    let ox = (THUMBNAIL_WIDTH - scaled_w) / 2;
    let oy = (THUMBNAIL_HEIGHT - scaled_h) / 2;
    image::imageops::overlay(&mut canvas, &resized, ox as i64, oy as i64);
    canvas
}

fn encode_thumbnail_base64(img: &image::RgbaImage) -> Option<String> {
    let mut buf = Cursor::new(Vec::new());
    let enc = PngEncoder::new(&mut buf);
    enc.write_image(img.as_raw(), img.width(), img.height(), ColorType::Rgba8.into())
        .ok()?;
    let b64 = general_purpose::STANDARD.encode(buf.into_inner());
    Some(format!("data:image/png;base64,{b64}"))
}

fn capture_monitor_thumbnail(monitor: &Monitor) -> Option<String> {
    let shot = monitor.capture_image().ok()?;
    encode_thumbnail_base64(&make_thumbnail(&shot))
}

fn capture_window_thumbnail(window: &Window) -> Option<String> {
    let shot = window.capture_image().ok()?;
    encode_thumbnail_base64(&make_thumbnail(&shot))
}

fn project_or_media_metadata(path: &Path) -> Result<VideoMetadata, String> {
    if path.extension().and_then(|value| value.to_str()) == Some("recast") {
        let project = project::reader::open_project(path).map_err(|e| e.to_string())?;
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

fn probe_video_metadata(path: &Path) -> Result<VideoMetadata, String> {
    if !path.exists() {
        return Err("File not found".into());
    }

    let size_bytes = fs::metadata(path).map(|m| m.len()).unwrap_or_default();
    let path_string = path.to_string_lossy().to_string();
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            &path_string,
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let parsed: serde_json::Value =
                serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
            let duration = parsed["format"]["duration"]
                .as_str()
                .and_then(|value| value.parse::<f64>().ok())
                .unwrap_or_default();
            let video_stream = parsed["streams"]
                .as_array()
                .and_then(|streams| streams.iter().find(|stream| stream["codec_type"].as_str() == Some("video")));

            let (width, height, fps, codec) = if let Some(stream) = video_stream {
                let fps_text = stream["r_frame_rate"].as_str().unwrap_or("30/1");
                let fps = if let Some((num, den)) = fps_text.split_once('/') {
                    let num = num.parse::<f64>().unwrap_or(30.0);
                    let den = den.parse::<f64>().unwrap_or(1.0);
                    if den > 0.0 { num / den } else { 30.0 }
                } else {
                    fps_text.parse::<f64>().unwrap_or(30.0)
                };

                (
                    stream["width"].as_u64().unwrap_or_default() as u32,
                    stream["height"].as_u64().unwrap_or_default() as u32,
                    fps,
                    stream["codec_name"].as_str().unwrap_or("unknown").to_string(),
                )
            } else {
                (0, 0, 30.0, "unknown".into())
            };

            Ok(VideoMetadata {
                duration,
                width,
                height,
                fps,
                codec,
                size_bytes,
            })
        }
        _ => Ok(VideoMetadata {
            duration: 0.0,
            width: 0,
            height: 0,
            fps: 30.0,
            codec: "unknown".into(),
            size_bytes,
        }),
    }
}

fn has_audio(path: &Path) -> bool {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "a",
            "-show_entries",
            "stream=index",
            "-of",
            "csv=p=0",
            &path.to_string_lossy(),
        ])
        .output();

    matches!(
        output,
        Ok(result) if result.status.success() && !String::from_utf8_lossy(&result.stdout).trim().is_empty()
    )
}

fn open_project_if_needed(path: &Path) -> Result<Option<ProjectOpenResult>, String> {
    if path.extension().and_then(|value| value.to_str()) == Some("recast") {
        project::reader::open_project(path)
            .map(Some)
            .map_err(|e| e.to_string())
    } else {
        Ok(None)
    }
}

#[tauri::command]
fn render_preview_frame(request: PreviewFrameRequest) -> Result<String, String> {
    let input_path = PathBuf::from(&request.input_path);
    let project = open_project_if_needed(&input_path)?;
    let source_video = project
        .as_ref()
        .map(|value| value.recording_path.clone())
        .unwrap_or(input_path);
    let metadata = probe_video_metadata(&source_video)?;
    let graph = RenderGraph::from_state(&request.render_state);
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
        "-v".to_string(),
        "error".to_string(),
        "-ss".to_string(),
        format!("{:.3}", request.time.max(0.0)),
        "-i".to_string(),
        source_video.to_string_lossy().to_string(),
    ];

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

    args.extend([
        "-frames:v".to_string(),
        "1".to_string(),
        "-f".to_string(),
        "image2pipe".to_string(),
        "-vcodec".to_string(),
        "png".to_string(),
        "-".to_string(),
    ]);

    match run_preview_ffmpeg(&args) {
        Ok(frame) => Ok(frame),
        Err(_) => {
            let fallback_args = vec![
                "-v".to_string(),
                "error".to_string(),
                "-ss".to_string(),
                format!("{:.3}", request.time.max(0.0)),
                "-i".to_string(),
                source_video.to_string_lossy().to_string(),
                "-frames:v".to_string(),
                "1".to_string(),
                "-vf".to_string(),
                "scale=1280:-1:flags=lanczos".to_string(),
                "-f".to_string(),
                "image2pipe".to_string(),
                "-vcodec".to_string(),
                "png".to_string(),
                "-".to_string(),
            ];
            run_preview_ffmpeg(&fallback_args)
        }
    }
}

#[tauri::command]
fn get_output_dir(state: State<'_, AppState>) -> Result<String, String> {
    Ok(get_active_output_dir(&state).to_string_lossy().to_string())
}

#[tauri::command]
fn set_output_dir(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<(), String> {
    if !Path::new(&path).exists() {
        return Err("Directory does not exist".into());
    }
    let mut config = state.config.lock();
    config.output_dir = Some(path);
    save_config(&app, &config);
    Ok(())
}

#[tauri::command]
fn get_displays() -> Result<Vec<DisplayInfo>, String> {
    let monitors = Monitor::all().map_err(|e| e.to_string())?;
    Ok(monitors
        .iter()
        .map(|monitor| DisplayInfo {
            id: monitor.id().unwrap_or_default(),
            name: monitor.name().unwrap_or_default(),
            x: monitor.x().unwrap_or_default(),
            y: monitor.y().unwrap_or_default(),
            width: monitor.width().unwrap_or_default(),
            height: monitor.height().unwrap_or_default(),
            is_primary: monitor.is_primary().unwrap_or_default(),
            thumbnail: capture_monitor_thumbnail(monitor),
        })
        .collect())
}

#[tauri::command]
fn get_windows() -> Result<Vec<WindowInfo>, String> {
    let windows = Window::all().map_err(|e| e.to_string())?;
    Ok(windows
        .iter()
        .filter(|window| !window.is_minimized().unwrap_or(false) && !window.title().unwrap_or_default().is_empty())
        .map(|window| WindowInfo {
            id: window.id().unwrap_or_default(),
            pid: window.pid().unwrap_or_default(),
            app_name: window.app_name().unwrap_or_default(),
            title: window.title().unwrap_or_default(),
            x: window.x().unwrap_or_default(),
            y: window.y().unwrap_or_default(),
            width: window.width().unwrap_or_default(),
            height: window.height().unwrap_or_default(),
            is_minimized: window.is_minimized().unwrap_or_default(),
            thumbnail: capture_window_thumbnail(window),
        })
        .collect())
}

#[tauri::command]
fn start_recording(target_type: String, target_id: u32, state: State<'_, AppState>) -> Result<(), String> {
    let target = CaptureTarget::resolve(&target_type, target_id).map_err(|e| e.to_string())?;
    let output_dir = get_active_output_dir(&state);
    state
        .recording_manager
        .start(target, output_dir)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn stop_recording(state: State<'_, AppState>) -> Result<String, String> {
    let artifacts = state.recording_manager.stop().map_err(|e| e.to_string())?;
    let final_path = get_active_output_dir(&state).join(format!(
        "recast_recording_{}.recast",
        artifacts.started_at_unix_ms
    ));
    let recording_meta = probe_video_metadata(&artifacts.recording_path)?;
    let metadata = ProjectMetadata {
        schema_version: 1,
        created_at_unix_ms: artifacts.started_at_unix_ms,
        capture_target: artifacts.capture_target.clone(),
        stats: artifacts.stats.clone(),
        video: project::ProjectVideoMetadata {
            width: if recording_meta.width > 0 {
                recording_meta.width
            } else {
                artifacts.capture_target.crop.width
            },
            height: if recording_meta.height > 0 {
                recording_meta.height
            } else {
                artifacts.capture_target.crop.height
            },
            fps: recording_meta.fps.round().max(1.0) as u32,
            duration_ms: artifacts.stats.duration_ms,
        },
    };
    let default_render_state = RenderState {
        trim_end: artifacts.stats.duration_ms as f64 / 1000.0,
        ..RenderState::default()
    };
    let project_path = write_project(ProjectWriteRequest {
        output_path: final_path.clone(),
        metadata,
        recording_path: artifacts.recording_path.clone(),
        cursor_path: artifacts.cursor_path.clone(),
        audio_path: artifacts.audio_path.clone(),
        edits_json: serde_json::to_string_pretty(&default_render_state).unwrap_or_else(|_| "{}".into()),
    })
    .map_err(|e| e.to_string())?;

    let _ = fs::remove_file(&artifacts.recording_path);
    let _ = fs::remove_file(&artifacts.cursor_path);
    let _ = fs::remove_file(&artifacts.audio_path);

    *state.last_file_path.lock() = Some(project_path.to_string_lossy().to_string());
    Ok(project_path.to_string_lossy().to_string())
}

#[tauri::command]
fn list_recordings(state: State<'_, AppState>) -> Result<Vec<RecordingEntry>, String> {
    let dir_path = get_active_output_dir(&state);
    let mut entries = Vec::new();

    for entry in fs::read_dir(&dir_path).map_err(|e| e.to_string())?.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let extension = path.extension().and_then(|value| value.to_str()).unwrap_or_default();
        if !matches!(extension, "recast" | "mp4") {
            continue;
        }

        if let Ok(meta) = entry.metadata() {
            let created = meta
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            entries.push(RecordingEntry {
                filename: name,
                path: path.to_string_lossy().to_string(),
                size_bytes: meta.len(),
                created,
            });
        }
    }

    entries.sort_by(|a, b| b.created.cmp(&a.created));
    Ok(entries)
}

#[tauri::command]
fn open_file_location(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .args(["/select,", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn get_video_metadata(path: String) -> Result<VideoMetadata, String> {
    project_or_media_metadata(Path::new(&path))
}

#[tauri::command]
fn load_editor_document(path: String) -> Result<EditorDocument, String> {
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
        metadata: metadata.clone(),
        render_state: RenderState {
            trim_end: metadata.duration,
            ..RenderState::default()
        },
    })
}

#[tauri::command]
fn generate_thumbnails(path: String, count: u32) -> Result<Vec<String>, String> {
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
    let temp_dir = env::temp_dir().join("recast-thumbnails");
    let _ = fs::create_dir_all(&temp_dir);
    let mut thumbnails = Vec::new();

    let scale_width = if count <= 2 { 480 } else { 240 };

    for index in 0..count {
        let timestamp = index as f64 * interval;
        let thumb_path = temp_dir.join(format!("thumb-{index}.jpg"));
        let result = Command::new("ffmpeg")
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
fn export_video(request: ExportRequest, state: State<'_, AppState>) -> Result<String, String> {
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
    let output_dir = get_active_output_dir(&state);
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
                args.extend([
                    "-c:a".to_string(),
                    "libopus".to_string(),
                ]);
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

    let output = Command::new("ffmpeg")
        .args(&args)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(format!("export failed:\n{}", summarize_ffmpeg_error(&output.stderr)));
    }
    Ok(output_path.to_string_lossy().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle();
            let config = load_config(&handle);
            app.manage(AppState {
                recording_manager: RecordingManager::default(),
                last_file_path: Mutex::new(None),
                config: Mutex::new(config),
            });

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            app.handle().plugin(tauri_plugin_dialog::init())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_output_dir,
            set_output_dir,
            get_displays,
            get_windows,
            start_recording,
            stop_recording,
            list_recordings,
            open_file_location,
            get_video_metadata,
            load_editor_document,
            render_preview_frame,
            generate_thumbnails,
            export_video
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
