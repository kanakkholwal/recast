//! Pre-renders a static rounded-rectangle alpha mask as a PNG so FFmpeg's
//! `alphamerge` filter can clip the source video's corners during export. The
//! preview path uses a WebGL shader for this; for export we generate the same
//! shape once and reuse it as a `-loop 1` image input.
//!
//! The PNG encodes coverage in the RGB channels (white = opaque, black =
//! transparent) because `alphamerge` consumes the **luminance** of the second
//! input as the alpha plane of the first. Alpha channel itself is set to 255.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{anyhow, Context, Result};
use image::{Rgba, RgbaImage};

use crate::render::cursor_export::TempDirGuard;

static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct MaskResult {
    pub path: PathBuf,
    _guard: TempDirGuard,
}

/// Render a rounded-rectangle mask at the given dimensions and corner radius.
/// Returns `Ok(None)` when `radius_px <= 0.5` (caller should skip the
/// alphamerge step entirely in that case).
pub fn render_border_radius_mask(
    width: u32,
    height: u32,
    radius_px: f64,
) -> Result<Option<MaskResult>> {
    if width == 0 || height == 0 {
        return Err(anyhow!("border-radius mask has zero dimension"));
    }
    if radius_px <= 0.5 {
        return Ok(None);
    }

    let counter = SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let scratch_dir = std::env::temp_dir().join(format!("recast-export-mask-{ts}-{counter}"));
    fs::create_dir_all(&scratch_dir).with_context(|| {
        format!(
            "failed to create mask scratch dir {}",
            scratch_dir.display()
        )
    })?;
    let guard = TempDirGuard::new(scratch_dir.clone());
    let mask_path = scratch_dir.join("border_radius_mask.png");

    let mut img = RgbaImage::new(width, height);
    let hx = width as f64 / 2.0;
    let hy = height as f64 / 2.0;
    let r = radius_px.min(hx.min(hy)).max(0.0);

    for y in 0..height {
        for x in 0..width {
            let px = (x as f64 + 0.5) - hx;
            let py = (y as f64 + 0.5) - hy;
            let qx = px.abs() - hx + r;
            let qy = py.abs() - hy + r;
            let sd = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r;
            // 1-pixel smooth edge keeps the corners from looking jagged when
            // the source video has high contrast against its background.
            let coverage = (1.0 - smoothstep(-1.0, 0.0, sd)).clamp(0.0, 1.0);
            let v = (coverage * 255.0).round().clamp(0.0, 255.0) as u8;
            img.put_pixel(x, y, Rgba([v, v, v, 255]));
        }
    }

    img.save(&mask_path)
        .with_context(|| format!("failed to write border-radius mask {}", mask_path.display()))?;

    Ok(Some(MaskResult {
        path: mask_path,
        _guard: guard,
    }))
}

fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0).max(1e-6)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Inputs for the drop-shadow rasteriser. Mirrors the WebGL shader stanza in
/// `VideoPreview.svelte` so the export visually matches the preview. All
/// distances are in canvas pixels (= source pixels + padding × 2 — the
/// preview's `vpToCanvas` factor is 1.0 here because we render directly at
/// canvas resolution).
pub struct DropShadowRequest {
    /// Comp dimensions (= source + padding × 2). The PNG is rendered at
    /// these dims even when the final canvas is larger (aspect preset);
    /// the caller composites it at the comp's offset within the canvas.
    pub canvas_width: u32,
    pub canvas_height: u32,
    /// Video-rect dimensions.
    pub video_width: u32,
    pub video_height: u32,
    /// Padding around the video rect inside the comp.
    pub padding: u32,
    /// Border radius in pixels applied to the video rect (preview also adds
    /// `spread * 0.5` so the shadow has slightly softer corners than the
    /// rect itself; we replicate that).
    pub video_border_radius: f64,
    /// Soft-edge falloff distance. Clamped to ≥ 0.5 to match the shader.
    pub blur: f64,
    /// Rect grows by this before SDF — produces a wider penumbra.
    pub spread: f64,
    /// Y-axis translate (no X axis: matches the preview, which only binds
    /// `u_shadowOffsetPx.y`).
    pub offset_y: f64,
    /// 0..100 — divided by 100 for the alpha multiplier.
    pub opacity: f64,
    /// Hex CSS colour: `#rgb`, `#rrggbb`, or `#rrggbbaa` (alpha ignored).
    pub color: String,
}

/// Pre-render the drop shadow as a transparent canvas-sized RGBA PNG. The
/// rect's position, soft edge, spread, offset, colour, and opacity are all
/// baked in, so the FFmpeg side can simply `overlay=0:0` it onto the
/// background before compositing the video on top.
///
/// Returns `Ok(None)` when the shadow would be invisible (`opacity <= 0`).
pub fn render_drop_shadow_mask(req: DropShadowRequest) -> Result<Option<MaskResult>> {
    if req.canvas_width == 0 || req.canvas_height == 0 {
        return Err(anyhow!("drop-shadow canvas has zero dimension"));
    }
    if req.opacity <= 0.0 {
        return Ok(None);
    }

    let counter = SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let scratch_dir = std::env::temp_dir().join(format!("recast-export-shadow-{ts}-{counter}"));
    fs::create_dir_all(&scratch_dir).with_context(|| {
        format!(
            "failed to create drop-shadow scratch dir {}",
            scratch_dir.display()
        )
    })?;
    let guard = TempDirGuard::new(scratch_dir.clone());
    let mask_path = scratch_dir.join("drop_shadow.png");

    let mut img = RgbaImage::new(req.canvas_width, req.canvas_height);

    // Video rect's centre on the canvas — the preview computes
    // `videoCenter = padding + halfSize`. We do the same in canvas pixels.
    let half_w = req.video_width as f64 / 2.0;
    let half_h = req.video_height as f64 / 2.0;
    let cx = req.padding as f64 + half_w;
    let cy = req.padding as f64 + half_h;

    let spread = req.spread.max(0.0);
    let blur_px = req.blur.max(0.5);
    // The shader's corner radius for the shadow rect: `r + spread*0.5`. We
    // additionally clamp to the half-extent of the spread-expanded rect so
    // very large radii on small rects degrade gracefully (full ellipse).
    let shadow_r = (req.video_border_radius + spread * 0.5)
        .min((half_w + spread).min(half_h + spread))
        .max(0.0);

    let (sr, sg, sb) = parse_hex_rgb(&req.color).unwrap_or((0, 0, 0));
    let opacity_norm = (req.opacity / 100.0).clamp(0.0, 1.0);

    for y in 0..req.canvas_height {
        for x in 0..req.canvas_width {
            // Same coordinate transform as the shader:
            //     shadowP = (canvasPx - videoCenter) - offsetPx
            // SDF then evaluated against `halfSize + spread`.
            let px = (x as f64 + 0.5) - cx;
            let py = (y as f64 + 0.5) - cy - req.offset_y;
            let hx = half_w + spread;
            let hy = half_h + spread;
            let qx = px.abs() - hx + shadow_r;
            let qy = py.abs() - hy + shadow_r;
            let sd = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - shadow_r;
            let coverage = (1.0 - smoothstep(0.0, blur_px, sd)).clamp(0.0, 1.0);
            // No `1 - videoCoverage` clip here: the FFmpeg side overlays
            // the video AFTER this shadow layer, so the video physically
            // covers any shadow underneath the rect — no need to mask.
            let alpha = (coverage * opacity_norm * 255.0).round().clamp(0.0, 255.0) as u8;
            img.put_pixel(x, y, Rgba([sr, sg, sb, alpha]));
        }
    }

    img.save(&mask_path)
        .with_context(|| format!("failed to write drop-shadow mask {}", mask_path.display()))?;

    Ok(Some(MaskResult {
        path: mask_path,
        _guard: guard,
    }))
}

/// Parse a `#rgb` / `#rrggbb` / `#rrggbbaa` hex string. Alpha (when present)
/// is ignored — the export treats the shadow's overall opacity as the
/// authoritative alpha source (matches the preview shader).
fn parse_hex_rgb(value: &str) -> Option<(u8, u8, u8)> {
    let trimmed = value.trim().trim_start_matches('#');
    match trimmed.len() {
        3 => {
            // #rgb → expand each digit.
            let r = u8::from_str_radix(&trimmed[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&trimmed[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&trimmed[2..3].repeat(2), 16).ok()?;
            Some((r, g, b))
        }
        6 | 8 => {
            let r = u8::from_str_radix(&trimmed[0..2], 16).ok()?;
            let g = u8::from_str_radix(&trimmed[2..4], 16).ok()?;
            let b = u8::from_str_radix(&trimmed[4..6], 16).ok()?;
            Some((r, g, b))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_rgb_handles_three_six_eight_digit_forms() {
        assert_eq!(parse_hex_rgb("#000"), Some((0, 0, 0)));
        assert_eq!(parse_hex_rgb("#fff"), Some((255, 255, 255)));
        assert_eq!(parse_hex_rgb("#ff0000"), Some((255, 0, 0)));
        assert_eq!(parse_hex_rgb("#FF0000"), Some((255, 0, 0)));
        assert_eq!(parse_hex_rgb("#3b82f6"), Some((59, 130, 246)));
        // 8-digit form: alpha ignored.
        assert_eq!(parse_hex_rgb("#3b82f680"), Some((59, 130, 246)));
        assert_eq!(parse_hex_rgb("not-a-color"), None);
    }

    #[test]
    fn drop_shadow_skipped_when_opacity_zero() {
        let req = DropShadowRequest {
            canvas_width: 64,
            canvas_height: 64,
            video_width: 32,
            video_height: 32,
            padding: 16,
            video_border_radius: 0.0,
            blur: 4.0,
            spread: 0.0,
            offset_y: 0.0,
            opacity: 0.0,
            color: "#000000".into(),
        };
        let result = render_drop_shadow_mask(req).expect("must not error");
        assert!(result.is_none(), "opacity 0 must short-circuit");
    }
}
