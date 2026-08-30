//! Static layers rasterised once per export: the rounded-corner mask, the
//! drop-shadow PNG, the gradient background, and the pre-baked wallpaper.
//!
//! Split out of `run_export_job`. Everything here is frame-independent, so it
//! runs once before the filter graph is assembled rather than per frame.

use std::path::{Path, PathBuf};

use crate::commands::error::{AppError, AppResult};
use crate::commands::types::ExportRequest;
use crate::render::cursor_export::TempDirGuard;
use crate::render::graph::CanvasGeometry;
use crate::render::mask_export::MaskResult;

/// The rasterised layers plus the geometry they were sized against.
///
/// Holds the `MaskResult`/`TempDirGuard` values, not just their paths: each one
/// deletes its PNG on drop, so this must outlive the encode that reads them.
pub(crate) struct StaticLayers {
    pub geom: CanvasGeometry,
    pub border_radius_mask: Option<MaskResult>,
    pub drop_shadow_mask: Option<MaskResult>,
    pub gradient_bg: Option<MaskResult>,
    _prebaked_bg: Option<TempDirGuard>,
}

/// Rasterise everything static for this export.
///
/// Takes `request` mutably because pre-baking a wallpaper/image background
/// rewrites `background_value` to the baked PNG and zeroes `background_blur` —
/// the caller must rebuild its `RenderGraph` afterwards so the plan sees it.
pub(crate) fn rasterize_static_layers(
    request: &mut ExportRequest,
    source_width: u32,
    source_height: u32,
    asset_cache_dir: Option<&Path>,
    static_root: &Path,
    prebake: impl Fn(&Path, u32, u32, f64) -> Option<(PathBuf, TempDirGuard)>,
) -> AppResult<StaticLayers> {
    // Border radius is a 0..50 percent of the shorter source edge; the mask alphamerges onto the zoomed source before background composition.
    let border_radius_pct = request.render_state.border_radius.clamp(0.0, 50.0);
    let border_radius_px = border_radius_pct / 100.0 * source_width.min(source_height) as f64;
    let border_radius_mask: Option<MaskResult> = if border_radius_px > 0.5 {
        crate::render::mask_export::render_border_radius_mask(
            source_width,
            source_height,
            border_radius_px,
        )
        .map_err(|e| AppError::msg(format!("border-radius mask render failed: {e}")))?
    } else {
        None
    };

    // Cursor and shadow PNGs render at COMP dims, not canvas dims: the other way piped ~28 MB/frame for a 9:16 of 1080p and stalled the sub-encode.
    let geom = crate::render::graph::compute_canvas_geometry(
        source_width,
        source_height,
        request.render_state.padding,
        request.render_state.output_aspect.as_deref(),
    );

    // The gates are also enforced inside `render_drop_shadow_mask`, but checking here saves the canvas-sized allocation.
    let shadow_settings = &request.render_state.shadow;
    let drop_shadow_mask: Option<MaskResult> =
        if shadow_settings.enabled && shadow_settings.opacity > 0.0 {
            crate::render::mask_export::render_drop_shadow_mask(
                crate::render::mask_export::DropShadowRequest {
                    canvas_width: geom.comp_w,
                    canvas_height: geom.comp_h,
                    video_width: source_width,
                    video_height: source_height,
                    padding: geom.padding_px,
                    video_border_radius: border_radius_px,
                    blur: shadow_settings.blur,
                    spread: shadow_settings.spread,
                    offset_y: shadow_settings.offset_y,
                    opacity: shadow_settings.opacity,
                    color: shadow_settings.color.clone(),
                },
            )
            .map_err(|e| AppError::msg(format!("drop-shadow mask render failed: {e}")))?
        } else {
            None
        };

    // Rasterised to a canvas-sized PNG so the export matches the preview's angled multi-stop gradient, not a flat colour.
    let gradient_bg: Option<MaskResult> = if request.render_state.background_type == "gradient" {
        crate::render::mask_export::render_gradient_background(
            &request.render_state.background_value,
            geom.canvas_w,
            geom.canvas_h,
        )
        .map_err(|e| AppError::msg(format!("gradient background render failed: {e}")))?
    } else {
        None
    };

    // A static background is identical every frame (~19.5 ms/frame at 120 fps), so bake it once and loop it with blur 0.
    let _prebaked_bg = if matches!(
        request.render_state.background_type.as_str(),
        "wallpaper" | "image"
    ) {
        crate::render::graph::resolve_background_path(
            &request.render_state.background_value,
            static_root,
            asset_cache_dir,
        )
        .and_then(|src| {
            prebake(
                &src,
                geom.canvas_w,
                geom.canvas_h,
                request.render_state.background_blur,
            )
        })
        .map(|(path, guard)| {
            request.render_state.background_value = path.to_string_lossy().into_owned();
            request.render_state.background_blur = 0.0;
            guard
        })
    } else {
        None
    };

    Ok(StaticLayers {
        geom,
        border_radius_mask,
        drop_shadow_mask,
        gradient_bg,
        _prebaked_bg,
    })
}
