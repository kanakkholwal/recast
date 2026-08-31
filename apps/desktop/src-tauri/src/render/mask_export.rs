//! Pre-renders the rounded-corner mask as a PNG so FFmpeg's `alphamerge` can clip the source during export.
//! Coverage is encoded in RGB, not alpha, because `alphamerge` consumes the second input's luminance.

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
            // A 1-pixel smooth edge keeps the corners from looking jagged against a high-contrast background.
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
    /// Border radius in pixels applied to the video rect (preview also adds `spread * 0.5` so the shadow has slightly softer corners than the rect itself; we replicate that).
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

    // The video rect's centre on the canvas, matching the preview's padding plus half-size, in canvas pixels.
    let half_w = req.video_width as f64 / 2.0;
    let half_h = req.video_height as f64 / 2.0;
    let cx = req.padding as f64 + half_w;
    let cy = req.padding as f64 + half_h;

    let spread = req.spread.max(0.0);
    let blur_px = req.blur.max(0.5);
    // The shader's shadow radius is r + spread*0.5, clamped to the half-extent so large radii degrade to an ellipse.
    let shadow_r = (req.video_border_radius + spread * 0.5)
        .min((half_w + spread).min(half_h + spread))
        .max(0.0);

    let (sr, sg, sb) = parse_hex_rgb(&req.color).unwrap_or((0, 0, 0));
    let opacity_norm = (req.opacity / 100.0).clamp(0.0, 1.0);

    for y in 0..req.canvas_height {
        for x in 0..req.canvas_width {
            // Same transform as the shader: shadowP = (canvasPx - videoCenter) - offsetPx, then the SDF against halfSize + spread.
            let px = (x as f64 + 0.5) - cx;
            let py = (y as f64 + 0.5) - cy - req.offset_y;
            let hx = half_w + spread;
            let hy = half_h + spread;
            let qx = px.abs() - hx + shadow_r;
            let qy = py.abs() - hy + shadow_r;
            let sd = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - shadow_r;
            let coverage = (1.0 - smoothstep(0.0, blur_px, sd)).clamp(0.0, 1.0);
            // No video-coverage clip: FFmpeg overlays the video AFTER this layer, so it physically covers the shadow.
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

/// Inputs for the camera-bubble drop shadow. Unlike `render_drop_shadow_mask`
/// (video-rect on the full comp canvas), this renders a small padded silhouette
/// of the bubble shape that FFmpeg scales + positions to follow the bubble.
pub struct CameraShadowRequest {
    /// Base bubble dimensions in canvas pixels.
    pub bubble_w: u32,
    pub bubble_h: u32,
    /// Bubble shape corner radius in px (circle = w/2, rounded = frac·w, square = 0).
    pub corner_radius_px: f64,
    /// Soft-edge falloff distance, centred on the silhouette edge.
    pub blur_px: f64,
    /// Downward shadow offset in px (x offset is 0, mirroring the CSS box-shadow).
    pub offset_y: f64,
    /// 0..1 alpha multiplier.
    pub opacity: f64,
    /// Transparent margin around the bubble so blur + offset don't clip.
    pub padding: u32,
}

/// Pre-render the camera bubble's drop shadow as a transparent RGBA PNG sized
/// `(bubble + 2·padding)`. Black silhouette, soft edge, offset and opacity baked
/// in; the FFmpeg side scales it by the bubble's live size and overlays it just
/// under the bubble. Mirrors the preview's `cameraShadowStyle` box-shadow.
/// Returns `Ok(None)` when invisible (`opacity ≤ 0`).
pub fn render_camera_shadow(req: CameraShadowRequest) -> Result<Option<MaskResult>> {
    if req.opacity <= 0.0 || req.bubble_w == 0 || req.bubble_h == 0 {
        return Ok(None);
    }
    let cw = req.bubble_w + 2 * req.padding;
    let ch = req.bubble_h + 2 * req.padding;

    let counter = SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let scratch_dir = std::env::temp_dir().join(format!("recast-export-cam-shadow-{ts}-{counter}"));
    fs::create_dir_all(&scratch_dir).with_context(|| {
        format!(
            "failed to create camera-shadow scratch dir {}",
            scratch_dir.display()
        )
    })?;
    let guard = TempDirGuard::new(scratch_dir.clone());
    let out_path = scratch_dir.join("camera_shadow.png");

    let mut img = RgbaImage::new(cw, ch);
    let hx = req.bubble_w as f64 / 2.0;
    let hy = req.bubble_h as f64 / 2.0;
    let cx = req.padding as f64 + hx;
    let cy = req.padding as f64 + hy + req.offset_y;
    let r = req.corner_radius_px.clamp(0.0, hx.min(hy));
    let blur = req.blur_px.max(0.5);
    let opacity = req.opacity.clamp(0.0, 1.0);

    for y in 0..ch {
        for x in 0..cw {
            let px = (x as f64 + 0.5) - cx;
            let py = (y as f64 + 0.5) - cy;
            let qx = px.abs() - hx + r;
            let qy = py.abs() - hy + r;
            let sd = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r;
            // Falloff centred on the edge → visually close to a CSS box-shadow.
            let coverage = (1.0 - smoothstep(-blur * 0.5, blur * 0.5, sd)).clamp(0.0, 1.0);
            let alpha = (coverage * opacity * 255.0).round().clamp(0.0, 255.0) as u8;
            img.put_pixel(x, y, Rgba([0, 0, 0, alpha]));
        }
    }

    img.save(&out_path)
        .with_context(|| format!("failed to write camera shadow {}", out_path.display()))?;

    Ok(Some(MaskResult {
        path: out_path,
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

/// One parsed gradient stop: sRGB color + position in 0..1 along the line.
struct GradStop {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
    pos: f64,
}

/// Parse a CSS `linear-gradient(<deg>, <#hex> <pct>%, …)` string into an angle
/// (radians) and a sorted list of stops. Mirrors the TS `parseGradient`: a
/// missing angle defaults to 135°, missing positions distribute evenly, and at
/// least two stops are always returned. Returns `None` only when no color can
/// be found at all (caller falls back to a flat color).
fn parse_css_gradient(value: &str) -> Option<(f64, Vec<GradStop>)> {
    // Slice the comma-separated body inside the outermost parentheses.
    let inner = value
        .find('(')
        .and_then(|s| value.rfind(')').map(|e| &value[s + 1..e]))
        .unwrap_or(value);
    let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();

    let mut angle_deg = 135.0_f64;
    let mut color_parts: Vec<&str> = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if i == 0 && part.ends_with("deg") {
            if let Ok(a) = part.trim_end_matches("deg").trim().parse::<f64>() {
                angle_deg = a;
            }
            continue;
        }
        if part.starts_with('#') {
            color_parts.push(part);
        }
    }

    let n = color_parts.len();
    if n == 0 {
        return None;
    }

    let mut stops: Vec<GradStop> = Vec::with_capacity(n.max(2));
    for (i, part) in color_parts.iter().enumerate() {
        let mut tokens = part.split_whitespace();
        let hex = tokens.next().unwrap_or("");
        let (r, g, b) = parse_hex_rgb(hex)?;
        // Alpha is premultiplied over black on rasterisation, so a translucent stop matches the preview's clear-to-black.
        let a = {
            let t = hex.trim_start_matches('#');
            if t.len() == 8 {
                u8::from_str_radix(&t[6..8], 16)
                    .map(|v| v as f64 / 255.0)
                    .unwrap_or(1.0)
            } else {
                1.0
            }
        };
        let pos = tokens
            .next()
            .and_then(|p| p.trim_end_matches('%').parse::<f64>().ok())
            .map(|p| (p / 100.0).clamp(0.0, 1.0))
            .unwrap_or_else(|| {
                if n <= 1 {
                    0.0
                } else {
                    i as f64 / (n - 1) as f64
                }
            });
        stops.push(GradStop {
            r: r as f64 / 255.0,
            g: g as f64 / 255.0,
            b: b as f64 / 255.0,
            a,
            pos,
        });
    }

    if stops.len() == 1 {
        let s = &stops[0];
        let dup = GradStop {
            r: s.r,
            g: s.g,
            b: s.b,
            a: s.a,
            pos: 1.0,
        };
        stops[0].pos = 0.0;
        stops.push(dup);
    }

    stops.sort_by(|a, b| {
        a.pos
            .partial_cmp(&b.pos)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Some((angle_deg.to_radians(), stops))
}

/// Sample the gradient at parameter `t` (0..1), interpolating in sRGB to match
/// the WebGL shader's `mix` (which never linearises). Returns premultiplied
/// 0..1 RGB (alpha folded over black).
fn sample_gradient(stops: &[GradStop], t: f64) -> (f64, f64, f64) {
    let first = &stops[0];
    let last = &stops[stops.len() - 1];
    let (r, g, b, a) = if t <= first.pos {
        (first.r, first.g, first.b, first.a)
    } else if t >= last.pos {
        (last.r, last.g, last.b, last.a)
    } else {
        let mut out = (last.r, last.g, last.b, last.a);
        for w in stops.windows(2) {
            let (s0, s1) = (&w[0], &w[1]);
            if t >= s0.pos && t <= s1.pos {
                let span = (s1.pos - s0.pos).max(1e-6);
                let f = ((t - s0.pos) / span).clamp(0.0, 1.0);
                out = (
                    s0.r + (s1.r - s0.r) * f,
                    s0.g + (s1.g - s0.g) * f,
                    s0.b + (s1.b - s0.b) * f,
                    s0.a + (s1.a - s0.a) * f,
                );
                break;
            }
        }
        out
    };
    (r * a, g * a, b * a)
}

/// Rasterise a CSS linear-gradient to an opaque PNG at the given canvas size so
/// the FFmpeg export composites the exact gradient the preview shows (the
/// pipeline otherwise collapses gradients to a flat color). The projection math
/// is identical to the WebGL shader in `VideoPreview.svelte` — keep them in
/// lockstep. Returns `Ok(None)` if the value carries no parseable color.
pub fn render_gradient_background(
    value: &str,
    width: u32,
    height: u32,
) -> Result<Option<MaskResult>> {
    if width == 0 || height == 0 {
        return Err(anyhow!("gradient background has zero dimension"));
    }
    let Some((angle, stops)) = parse_css_gradient(value) else {
        return Ok(None);
    };

    let counter = SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let scratch_dir = std::env::temp_dir().join(format!("recast-export-gradient-{ts}-{counter}"));
    fs::create_dir_all(&scratch_dir).with_context(|| {
        format!(
            "failed to create gradient scratch dir {}",
            scratch_dir.display()
        )
    })?;
    let guard = TempDirGuard::new(scratch_dir.clone());
    let out_path = scratch_dir.join("gradient_bg.png");

    let w = width as f64;
    let h = height as f64;
    let dir = (angle.sin(), -angle.cos());
    let ext = (dir.0.abs() * w + dir.1.abs() * h).max(1.0);

    let mut img = RgbaImage::new(width, height);
    for y in 0..height {
        let py = (y as f64 + 0.5) - h / 2.0;
        for x in 0..width {
            let px = (x as f64 + 0.5) - w / 2.0;
            let proj = px * dir.0 + py * dir.1;
            let t = (0.5 + proj / ext).clamp(0.0, 1.0);
            let (r, g, b) = sample_gradient(&stops, t);
            img.put_pixel(
                x,
                y,
                Rgba([
                    (r * 255.0).round().clamp(0.0, 255.0) as u8,
                    (g * 255.0).round().clamp(0.0, 255.0) as u8,
                    (b * 255.0).round().clamp(0.0, 255.0) as u8,
                    255,
                ]),
            );
        }
    }

    img.save(&out_path)
        .with_context(|| format!("failed to write gradient background {}", out_path.display()))?;

    Ok(Some(MaskResult {
        path: out_path,
        _guard: guard,
    }))
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

    #[test]
    fn camera_shadow_skipped_when_opacity_zero() {
        let req = CameraShadowRequest {
            bubble_w: 100,
            bubble_h: 100,
            corner_radius_px: 20.0,
            blur_px: 8.0,
            offset_y: 4.0,
            opacity: 0.0,
            padding: 20,
        };
        assert!(render_camera_shadow(req).expect("no error").is_none());
    }

    #[test]
    fn camera_shadow_is_padded_and_opaque_at_the_centre() {
        let padding = 24;
        let req = CameraShadowRequest {
            bubble_w: 120,
            bubble_h: 120,
            corner_radius_px: 0.0,
            blur_px: 10.0,
            offset_y: 6.0,
            opacity: 0.5,
            padding,
        };
        let result = render_camera_shadow(req)
            .expect("no error")
            .expect("visible");
        let img = image::open(&result.path).expect("readable png").to_rgba8();
        assert_eq!(img.dimensions(), (120 + 2 * padding, 120 + 2 * padding));
        // Silhouette centre (offset down by offset_y) is at ~half opacity → ~127.
        let centre = img.get_pixel(img.width() / 2, img.height() / 2)[3];
        assert!(
            centre > 100,
            "centre alpha should be near 0.5*255, got {centre}"
        );
        // The far transparent corner stays clear.
        assert_eq!(img.get_pixel(0, 0)[3], 0);
    }

    #[test]
    fn parse_css_gradient_reads_angle_and_stops() {
        let (angle, stops) =
            parse_css_gradient("linear-gradient(135deg, #6366f1 0%, #8b5cf6 50%, #d946ef 100%)")
                .expect("parses");
        assert!((angle - 135f64.to_radians()).abs() < 1e-9);
        assert_eq!(stops.len(), 3);
        assert!((stops[0].pos - 0.0).abs() < 1e-9);
        assert!((stops[1].pos - 0.5).abs() < 1e-9);
        assert!((stops[2].pos - 1.0).abs() < 1e-9);
        // First stop is #6366f1.
        assert!((stops[0].r - 0x63 as f64 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn parse_css_gradient_distributes_missing_positions() {
        let (_, stops) =
            parse_css_gradient("linear-gradient(90deg, #000000, #ffffff)").expect("parses");
        assert_eq!(stops.len(), 2);
        assert!((stops[0].pos - 0.0).abs() < 1e-9);
        assert!((stops[1].pos - 1.0).abs() < 1e-9);
    }

    #[test]
    fn render_gradient_background_varies_across_axis() {
        let result =
            render_gradient_background("linear-gradient(90deg, #000000 0%, #ffffff 100%)", 64, 8)
                .expect("must not error")
                .expect("produces an image");
        let img = image::open(&result.path).expect("readable png").to_rgba8();
        // 90deg = left→right: left edge is black, right edge is white.
        let left = img.get_pixel(0, 4)[0];
        let right = img.get_pixel(63, 4)[0];
        assert!(left < 20, "left edge should be near-black, got {left}");
        assert!(right > 235, "right edge should be near-white, got {right}");
    }
}
