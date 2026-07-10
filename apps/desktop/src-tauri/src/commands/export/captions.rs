//! Caption burn-in for the video export: styles the render-state transcript into
//! an ASS script and splices a libass `subtitles` stage into the filter graph.
//! Split out of commands/editor.rs's `export_video`.

use tauri::AppHandle;

use crate::commands::ffmpeg::append_subtitles_to_complex;
use crate::commands::types::ExportRequest;
use crate::render::graph::CanvasGeometry;

/// Burn captions into the export (overlay) via libass. The transcript + style
/// ride along in the render-state passthrough; they're styled into an ASS script
/// and composited on the trimmed-but-uncut axis, so the cut/speed stage re-times
/// the burned pixels with the rest.
///
/// Returns the updated `(filter_complex, video_map)` when a caption stage was
/// added, or `None` when there's nothing to burn (no `burn_captions`, GIF export
/// — its paletteuse tail can't take another stage — an empty/absent transcript,
/// or an ASS write failure, which degrades to no captions rather than failing).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn append_caption_burn_in(
    app: &AppHandle,
    request: &ExportRequest,
    canvas_width: u32,
    canvas_height: u32,
    canvas_geom: &CanvasGeometry,
    trim_start: f64,
    duration: f64,
    filter_complex: Option<&str>,
    video_map: &str,
) -> Option<(String, String)> {
    if !(request.burn_captions && request.format != "gif") {
        return None;
    }
    let transcript: crate::transcription::Transcript = request
        .render_state
        .passthrough
        .get("transcript")
        .filter(|v| !v.is_null())
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .filter(|t: &crate::transcription::Transcript| !t.segments.is_empty())?;
    let style: crate::transcription::CaptionStyle = request
        .render_state
        .passthrough
        .get("captionStyle")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    // Embed the preset's font so it renders in the burn instead of a libass
    // fallback. System/generic faces are skipped (libass resolves them); a fetch
    // failure degrades to the fallback, never blocks export.
    let family = crate::transcription::subtitles::first_family(&style.font_family);
    let fontsdir: Option<String> = if crate::transcription::subtitles::is_system_family(&family) {
        None
    } else {
        match crate::fonts::ensure_caption_font_dir(app, &family, style.font_weight).await {
            Ok(dir) => Some(dir.to_string_lossy().to_string()),
            Err(e) => {
                log::warn!("caption font embed ({family}): {e}");
                None
            }
        }
    };
    let ass = crate::transcription::subtitles::to_ass(
        &transcript,
        &style,
        canvas_width,
        canvas_height,
        crate::transcription::subtitles::VideoRectPx {
            x: canvas_geom.video_x,
            y: canvas_geom.video_y,
            w: canvas_geom.video_w,
            h: canvas_geom.video_h,
        },
        trim_start,
        duration,
        fontsdir.is_some(),
    );
    let ass_path = std::env::temp_dir().join(format!("recast-captions-{}.ass", request.export_id));
    match std::fs::write(&ass_path, ass) {
        Ok(()) => Some(append_subtitles_to_complex(
            filter_complex,
            video_map,
            &ass_path.to_string_lossy(),
            fontsdir.as_deref(),
        )),
        Err(e) => {
            log::warn!("caption burn-in: failed to write ASS script: {e}");
            None
        }
    }
}
