//! Caption burn-in for the video export: styles the render-state transcript into
//! an ASS script and splices a libass `subtitles` stage into the filter graph.
//! Split out of commands/editor.rs's `export_video`.

use tauri::AppHandle;

use crate::commands::error::{AppError, AppResult};
use crate::commands::ffmpeg::append_subtitles_to_complex;
use crate::commands::types::ExportRequest;
use crate::render::graph::CanvasGeometry;

/// Burn captions into the export (overlay) via libass. The transcript + style
/// ride along in the render-state passthrough; they're styled into an ASS script
/// and composited on the trimmed-but-uncut axis, so the cut/speed stage re-times
/// the burned pixels with the rest.
///
/// Returns the updated `(filter_complex, video_map)` when a caption stage was
/// added, or `Ok(None)` when there's nothing to burn (no `burn_captions`, GIF
/// export, whose paletteuse tail can't take another stage, an empty/absent
/// transcript, or an ASS write failure, which degrades to no captions rather
/// than failing).
///
/// Errors only when the user asked for captions and the resolved FFmpeg cannot
/// render them. Silently exporting a caption-less video in that case would be
/// worse: the user sees a "successful" export and only finds the missing
/// captions after uploading it.
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
) -> AppResult<Option<(String, String)>> {
    if !(request.burn_captions && request.format != "gif") {
        return Ok(None);
    }
    let transcript: crate::transcription::Transcript = match request
        .render_state
        .passthrough
        .get("transcript")
        .filter(|v| !v.is_null())
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .filter(|t: &crate::transcription::Transcript| !t.segments.is_empty())
    {
        Some(t) => t,
        None => return Ok(None),
    };
    // libass is a separate build flag, so catch a missing `ass` filter here instead of FFmpeg's bare error mid-export.
    if !crate::ffmpeg::has_filter("ass") {
        return Err(AppError::msg(format!(
            "Captions can't be burned in: the FFmpeg at {} was built without libass (no `ass` filter). \
             Reinstall Recast to restore its bundled FFmpeg, or install one with libass \
             (macOS: `brew install ffmpeg`). To export without captions, turn off caption burn-in.",
            crate::ffmpeg::ffmpeg_path().display()
        )));
    }
    let style: crate::transcription::CaptionStyle = request
        .render_state
        .passthrough
        .get("captionStyle")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    // Embed the preset's font so it renders in the burn; system faces are skipped and a fetch failure degrades, never blocks.
    let family = crate::transcription::subtitles::first_family(&style.font_family);
    let is_system = crate::transcription::subtitles::is_system_family(&family);
    let fontsdir: Option<String> = if is_system {
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
    // Resolve the exact face off the font file: its legacy match name (so Inter-600 isn't Arial) and libass's winAscent plus winDescent size correction.
    let search_name = if is_system {
        crate::transcription::subtitles::ass_font_name(&style.font_family)
    } else {
        family.clone()
    };
    let dir_path = fontsdir.as_deref().map(std::path::Path::new);
    let font = match crate::transcription::text_measure::resolve_font(
        &search_name,
        style.font_weight,
        dir_path,
    ) {
        Some(m) => crate::transcription::subtitles::RenderFont {
            ass_name: m.ass_name,
            embedded: fontsdir.is_some(),
            ass_scale: m.ass_scale,
            measure: Some(m.measure),
        },
        // Unresolved (offline or no match): best-effort name, no size correction, and keep the embed flag so libass tries the fontsdir.
        None => {
            let mut f = crate::transcription::subtitles::RenderFont::fallback(&style);
            f.embedded = fontsdir.is_some();
            f
        }
    };
    // Chunk per kept span: a chunk straddling a cut burns words the export removed and breaks at different points than the preview.
    let trim_end = request.render_state.trim_end.max(trim_start);
    let cuts: Vec<(f64, f64)> = crate::commands::export::cuts_speed::collect_export_cuts(
        &request.render_state,
        trim_start,
        trim_end,
    )
    .into_iter()
    // `collect_export_cuts` returns trim-relative ranges, while the transcript and ASS axis stay source-time until `to_ass`.
    .map(|(lo, hi)| (lo + trim_start, hi + trim_start))
    .collect();
    let spans = crate::transcription::subtitles::kept_spans(trim_start, trim_end, &cuts);
    let transcript =
        crate::transcription::subtitles::split_transcript_by_spans(&transcript, &spans);

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
        &font,
    );
    let ass_path = std::env::temp_dir().join(format!("recast-captions-{}.ass", request.export_id));
    match std::fs::write(&ass_path, ass) {
        Ok(()) => Ok(Some(append_subtitles_to_complex(
            filter_complex,
            video_map,
            &ass_path.to_string_lossy(),
            fontsdir.as_deref(),
        ))),
        Err(e) => {
            log::warn!("caption burn-in: failed to write ASS script: {e}");
            Ok(None)
        }
    }
}
