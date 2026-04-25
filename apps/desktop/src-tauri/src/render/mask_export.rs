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
    fs::create_dir_all(&scratch_dir)
        .with_context(|| format!("failed to create mask scratch dir {}", scratch_dir.display()))?;
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
