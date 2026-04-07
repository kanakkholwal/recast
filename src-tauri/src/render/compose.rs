use std::path::Path;

use super::decode::decode_frame_at;
use super::nodes::{BackgroundCompositor, CursorOverlay, ZoomEffect, ZoomRegion};
use super::pipeline::{RenderContext, RenderPipeline};
use crate::render::graph::RenderState;

/// Build a render pipeline from the editor's render state.
pub fn build_pipeline(state: &RenderState) -> RenderPipeline {
    let mut pipeline = RenderPipeline::new();

    // Zoom first — crop and scale the raw video frame.
    if !state.zoom_regions.is_empty() {
        pipeline.push(Box::new(ZoomEffect {
            regions: state
                .zoom_regions
                .iter()
                .map(|r| ZoomRegion {
                    start: r.start,
                    end: r.end,
                    scale: r.scale,
                })
                .collect(),
        }));
    }

    // Background — add padding and canvas.
    let padding = state.padding.max(0.0).round() as u32;
    if padding > 0 {
        pipeline.push(Box::new(BackgroundCompositor {
            background_type: state.background_type.clone(),
            value: state.background_value.clone(),
            blur: state.background_blur,
            padding,
        }));
    }

    // Cursor overlay — render cursor indicator on top.
    if state.cursor_enabled {
        pipeline.push(Box::new(CursorOverlay::new(
            state.cursor_enabled,
            state.cursor_size,
            state.cursor_smoothing,
            state.cursor_highlight_clicks,
            state.cursor_highlight_color.clone(),
            state.cursor_highlight_opacity,
            state.cursor_hide_when_idle,
            state.cursor_idle_timeout,
        )));
    }

    pipeline
}

/// Render a single preview frame at the given timestamp.
/// Returns a PNG-encoded image as bytes.
pub fn render_preview(
    video_path: &Path,
    timestamp: f64,
    state: &RenderState,
    static_root: &Path,
    cursor_track_path: Option<&Path>,
    source_width: u32,
    source_height: u32,
) -> Result<Vec<u8>, String> {
    let frame = decode_frame_at(video_path, timestamp, None)?;
    let pipeline = build_pipeline(state);

    let ctx = RenderContext {
        timestamp,
        static_root,
        cursor_track_path,
        source_width,
        source_height,
    };

    let composed = pipeline.process_frame(frame, &ctx)?;
    composed.encode_png()
}
