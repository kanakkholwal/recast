//! Pre-renders the editor's cursor overlay as an alpha VP9 video so it can be
//! muxed onto the main export via a single FFmpeg `overlay` filter.
//!
//! This mirrors the cursor rendering done by the WebGL2 preview in
//! `src/components/editor/VideoPreview.svelte` (cursor dot + click highlight,
//! with zoom-aware coordinates and idle-hide). Rendering in Rust and encoding
//! to an intermediate `.webm` file avoids both the "thousands of PNG files"
//! disk-cost problem and any pixel-handoff via IPC.
//!
//! NOTE: this module is currently dormant. `export_video` in
//! `src/commands/editor.rs` temporarily skips the cursor overlay pass while
//! we diagnose a hang + corruption issue. The code is kept intact so we can
//! re-enable it once the root cause is understood.

#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, anyhow};
use image::RgbaImage;

use crate::cursor::CursorTrack;
use crate::render::graph::RenderState;

/// Input for pre-rendering a cursor overlay track.
#[derive(Debug, Clone)]
pub struct CursorOverlayRequest {
    /// Path to the cursor.json track file (from `.recast` project).
    pub cursor_track_path: PathBuf,
    /// Canvas dimensions (source video + padding × 2).
    pub canvas_width: u32,
    pub canvas_height: u32,
    /// Source video dimensions (without padding).
    pub source_width: u32,
    pub source_height: u32,
    /// Padding in source pixels around the video area.
    pub padding: u32,
    /// Output framerate for the overlay video (matches source video fps).
    pub fps: u32,
    /// Duration in seconds of the overlay track to produce.
    pub duration_secs: f64,
    /// Trim start in seconds (to offset cursor timestamps).
    pub trim_start: f64,
    /// Full render state (we care about cursor settings + zoom regions).
    pub render_state: RenderState,
}

/// Result of a successful pre-render — includes a drop guard for the scratch dir.
pub struct CursorOverlayResult {
    pub overlay_path: PathBuf,
    _guard: TempDirGuard,
}

/// RAII guard that recursively deletes a scratch directory on drop.
pub struct TempDirGuard {
    path: PathBuf,
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        if self.path.exists() {
            if let Err(e) = fs::remove_dir_all(&self.path) {
                log::warn!(
                    "failed to clean up cursor overlay scratch dir {}: {e}",
                    self.path.display()
                );
            }
        }
    }
}

/// Unique scratch directory counter so concurrent exports don't collide.
static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Render the cursor overlay track and return a path to the resulting alpha
/// VP9 webm. The caller must keep the returned `CursorOverlayResult` alive
/// until FFmpeg has finished reading the file.
pub fn render_cursor_overlay(request: CursorOverlayRequest) -> Result<CursorOverlayResult> {
    if request.canvas_width == 0 || request.canvas_height == 0 {
        return Err(anyhow!("cursor overlay canvas has zero dimension"));
    }
    if request.fps == 0 {
        return Err(anyhow!("cursor overlay fps must be > 0"));
    }
    if request.duration_secs <= 0.0 {
        return Err(anyhow!("cursor overlay duration must be > 0"));
    }

    // Load cursor track.
    let track_bytes = fs::read(&request.cursor_track_path).with_context(|| {
        format!(
            "failed to read cursor track: {}",
            request.cursor_track_path.display()
        )
    })?;
    let track: CursorTrack = serde_json::from_slice(&track_bytes)
        .with_context(|| "failed to parse cursor track JSON")?;

    if track.samples.is_empty() {
        return Err(anyhow!("cursor track has no samples"));
    }

    // Create a unique scratch directory.
    let counter = SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let scratch_dir = std::env::temp_dir().join(format!("recast-export-cursor-{ts}-{counter}"));
    fs::create_dir_all(&scratch_dir)
        .with_context(|| format!("failed to create scratch dir {}", scratch_dir.display()))?;
    let guard = TempDirGuard {
        path: scratch_dir.clone(),
    };
    let overlay_path = scratch_dir.join("cursor.webm");

    // Precompute derived settings (mirrors VideoPreview.svelte's draw loop).
    let cursor_enabled = request.render_state.cursor_enabled;
    if !cursor_enabled {
        return Err(anyhow!("cursor not enabled — caller should skip"));
    }

    // Cursor radius in canvas pixels. WebGL shader uses:
    //   const cursorRadiusCanvas = (cs.size * 2 * canvasEl.width) / compW;
    // where compW = source_width + padding * 2.
    let comp_w = request.source_width + request.padding * 2;
    let cursor_radius_canvas = if comp_w > 0 {
        ((request.render_state.cursor_size * 2.0) * request.canvas_width as f64
            / comp_w as f64)
            .max(2.0)
    } else {
        2.0
    };

    // Parse highlight color.
    let (hr, hg, hb) = parse_hex_color(&request.render_state.cursor_highlight_color)
        .unwrap_or((0x3b, 0x82, 0xf6));

    // Allocate one reusable frame buffer.
    let canvas_w = request.canvas_width as usize;
    let canvas_h = request.canvas_height as usize;
    let bytes_per_frame = canvas_w * canvas_h * 4;
    let mut frame = vec![0u8; bytes_per_frame];

    // Spawn FFmpeg to encode raw RGBA → VP9 with alpha.
    let mut ffmpeg = Command::new(crate::ffmpeg::ffmpeg_path());
    ffmpeg
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-video_size",
            &format!("{}x{}", request.canvas_width, request.canvas_height),
            "-framerate",
            &request.fps.to_string(),
            "-i",
            "-",
            "-c:v",
            "libvpx-vp9",
            "-pix_fmt",
            "yuva420p",
            "-b:v",
            "0",
            "-crf",
            "40",
            "-deadline",
            "realtime",
            "-cpu-used",
            "5",
            "-auto-alt-ref",
            "0",
        ])
        .arg(overlay_path.to_string_lossy().as_ref())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        ffmpeg.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = ffmpeg
        .spawn()
        .context("failed to start ffmpeg for cursor overlay encode")?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("ffmpeg stdin pipe not available"))?;

    // Render frames.
    let frame_count = (request.duration_secs * request.fps as f64).ceil() as u64;
    let trim_start_us = (request.trim_start * 1_000_000.0).max(0.0) as u64;

    let idle_timeout_us = (request.render_state.cursor_idle_timeout * 1_000_000.0) as u64;
    let highlight_alpha_base = (request.render_state.cursor_highlight_opacity / 100.0).clamp(0.0, 1.0);

    for i in 0..frame_count {
        // Clear frame to transparent.
        frame.fill(0);

        // Wall-clock time relative to the trimmed output, mapped to cursor-track time.
        let t_out_us = (i as u64 * 1_000_000) / request.fps as u64;
        let t_track_us = trim_start_us + t_out_us;
        let t_out_secs = t_out_us as f64 / 1_000_000.0;

        // Sample cursor position at this timestamp.
        let sample = match interpolate_cursor(&track, t_track_us) {
            Some(s) => s,
            None => {
                // No cursor data — write the empty frame.
                stdin
                    .write_all(&frame)
                    .context("failed to write cursor frame to ffmpeg stdin")?;
                continue;
            }
        };

        if !sample.visible {
            stdin
                .write_all(&frame)
                .context("failed to write cursor frame to ffmpeg stdin")?;
            continue;
        }

        // Idle hide check.
        if request.render_state.cursor_hide_when_idle {
            let mut idle = false;
            for period in &track.idle_periods {
                if t_track_us >= period.start_us.saturating_add(idle_timeout_us)
                    && t_track_us <= period.end_us
                {
                    idle = true;
                    break;
                }
            }
            if idle {
                stdin
                    .write_all(&frame)
                    .context("failed to write cursor frame to ffmpeg stdin")?;
                continue;
            }
        }

        // Apply zoom transform in source-video coordinates.
        let (mut cursor_source_x, mut cursor_source_y) = (sample.x, sample.y);
        if let Some((scale, _)) =
            active_zoom_at(&request.render_state.zoom_regions, t_out_secs)
        {
            let src_cx = request.source_width as f64 / 2.0;
            let src_cy = request.source_height as f64 / 2.0;
            cursor_source_x = (cursor_source_x - src_cx) * scale + src_cx;
            cursor_source_y = (cursor_source_y - src_cy) * scale + src_cy;

            // Cursor must remain inside the (zoomed-visible) source rect — the
            // WebGL shader skips rendering if the cursor leaves the visible area.
            if cursor_source_x < 0.0
                || cursor_source_x > request.source_width as f64
                || cursor_source_y < 0.0
                || cursor_source_y > request.source_height as f64
            {
                stdin
                    .write_all(&frame)
                    .context("failed to write cursor frame to ffmpeg stdin")?;
                continue;
            }
        }

        // Map source coords → canvas coords.
        // Video area in the canvas is [padding, padding + source_width].
        let scale_canvas =
            request.canvas_width as f64 / (request.source_width + request.padding * 2) as f64;
        let cursor_canvas_x =
            (request.padding as f64 + cursor_source_x) * scale_canvas;
        let cursor_canvas_y =
            (request.padding as f64 + cursor_source_y) * scale_canvas;

        // Click highlight (drawn first, underneath cursor dot).
        let show_highlight = request.render_state.cursor_highlight_clicks
            && (sample.left_down || sample.right_down);
        if show_highlight {
            let hr_radius = cursor_radius_canvas * 6.0;
            draw_filled_circle_soft(
                &mut frame,
                canvas_w,
                canvas_h,
                cursor_canvas_x,
                cursor_canvas_y,
                hr_radius,
                hr,
                hg,
                hb,
                highlight_alpha_base,
            );
        }

        // Cursor dot (white, 90% alpha).
        draw_filled_circle_soft(
            &mut frame,
            canvas_w,
            canvas_h,
            cursor_canvas_x,
            cursor_canvas_y,
            cursor_radius_canvas,
            255,
            255,
            255,
            0.9,
        );

        stdin
            .write_all(&frame)
            .context("failed to write cursor frame to ffmpeg stdin")?;
    }

    // Close stdin so FFmpeg can finalize the webm.
    drop(stdin);

    let output = child
        .wait_with_output()
        .context("failed to wait for ffmpeg cursor encode")?;

    if !output.status.success() {
        let stderr_text = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("ffmpeg cursor overlay encode failed: {stderr_text}"));
    }

    // Sanity check: the webm must exist and be > 0 bytes.
    let meta = fs::metadata(&overlay_path)
        .with_context(|| format!("cursor overlay not written: {}", overlay_path.display()))?;
    if meta.len() == 0 {
        return Err(anyhow!("cursor overlay is empty"));
    }

    // Leaked frame buffer back to OS — not strictly needed since we're in spawn_blocking.
    drop(frame);

    Ok(CursorOverlayResult {
        overlay_path,
        _guard: guard,
    })
}

// ── Cursor interpolation (mirror of VideoPreview.svelte:317-342) ────────

#[derive(Debug, Clone, Copy)]
struct InterpolatedCursor {
    x: f64,
    y: f64,
    visible: bool,
    left_down: bool,
    right_down: bool,
}

fn interpolate_cursor(track: &CursorTrack, timestamp_us: u64) -> Option<InterpolatedCursor> {
    let samples = &track.samples;
    if samples.is_empty() {
        return None;
    }

    // Binary search for the first sample with timestamp >= target.
    let mut lo = 0usize;
    let mut hi = samples.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        if samples[mid].timestamp_us < timestamp_us {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    let idx = lo;

    if idx >= samples.len() {
        let last = samples.last().unwrap();
        return Some(InterpolatedCursor {
            x: last.x as f64,
            y: last.y as f64,
            visible: last.visible,
            left_down: last.left_down,
            right_down: last.right_down,
        });
    }

    if idx == 0 || samples[idx].timestamp_us == timestamp_us {
        let s = &samples[idx];
        return Some(InterpolatedCursor {
            x: s.x as f64,
            y: s.y as f64,
            visible: s.visible,
            left_down: s.left_down,
            right_down: s.right_down,
        });
    }

    let a = &samples[idx - 1];
    let b = &samples[idx];
    let range = b.timestamp_us.saturating_sub(a.timestamp_us) as f64;
    let t = if range > 0.0 {
        (timestamp_us - a.timestamp_us) as f64 / range
    } else {
        0.0
    };

    // Linear interpolate position; nearest-neighbor for discrete flags.
    let pick = if t < 0.5 { a } else { b };
    Some(InterpolatedCursor {
        x: a.x as f64 + (b.x as f64 - a.x as f64) * t,
        y: a.y as f64 + (b.y as f64 - a.y as f64) * t,
        visible: pick.visible,
        left_down: pick.left_down,
        right_down: pick.right_down,
    })
}

// ── Zoom lookup (mirror of nested_region_expr in graph.rs) ──────────────

fn active_zoom_at(
    regions: &[crate::render::node_types::ZoomRegion],
    t_secs: f64,
) -> Option<(f64, ())> {
    // Match the WebGL preview: the first region whose [start, end] contains t.
    for region in regions {
        if t_secs >= region.start && t_secs <= region.end {
            let scale = region.scale.max(1.0);
            if scale > 1.0001 {
                return Some((scale, ()));
            }
        }
    }
    None
}

// ── Pixel drawing ────────────────────────────────────────────────────────

/// Alpha-blend a filled circle into the RGBA buffer using a 1-px smoothstep
/// edge to match the WebGL shader's `smoothstep(r-1.5, r, dist)` aesthetic.
#[allow(clippy::too_many_arguments)]
fn draw_filled_circle_soft(
    buf: &mut [u8],
    width: usize,
    height: usize,
    cx: f64,
    cy: f64,
    radius: f64,
    r: u8,
    g: u8,
    b: u8,
    alpha_base: f64,
) {
    if alpha_base <= 0.0 {
        return;
    }
    let edge = 1.5_f64;
    let outer = radius + edge;
    let x_min = ((cx - outer).floor().max(0.0)) as usize;
    let y_min = ((cy - outer).floor().max(0.0)) as usize;
    let x_max = ((cx + outer).ceil() as i64).min(width as i64 - 1).max(0) as usize;
    let y_max = ((cy + outer).ceil() as i64).min(height as i64 - 1).max(0) as usize;

    if x_max < x_min || y_max < y_min {
        return;
    }

    for y in y_min..=y_max {
        let dy = y as f64 + 0.5 - cy;
        let row_start = y * width * 4;
        for x in x_min..=x_max {
            let dx = x as f64 + 0.5 - cx;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > outer {
                continue;
            }
            // smoothstep(radius - edge, radius, dist) then invert → inside = 1
            let t_raw = ((dist - (radius - edge)) / edge).clamp(0.0, 1.0);
            let smooth = t_raw * t_raw * (3.0 - 2.0 * t_raw);
            let coverage = (1.0 - smooth).clamp(0.0, 1.0);
            let alpha = coverage * alpha_base;
            if alpha <= 0.0 {
                continue;
            }
            let idx = row_start + x * 4;
            // Source-over alpha blending into RGBA8.
            let dst_r = buf[idx] as f64 / 255.0;
            let dst_g = buf[idx + 1] as f64 / 255.0;
            let dst_b = buf[idx + 2] as f64 / 255.0;
            let dst_a = buf[idx + 3] as f64 / 255.0;
            let src_r = r as f64 / 255.0;
            let src_g = g as f64 / 255.0;
            let src_b = b as f64 / 255.0;
            let out_a = alpha + dst_a * (1.0 - alpha);
            let (out_r, out_g, out_b) = if out_a > 0.0 {
                (
                    (src_r * alpha + dst_r * dst_a * (1.0 - alpha)) / out_a,
                    (src_g * alpha + dst_g * dst_a * (1.0 - alpha)) / out_a,
                    (src_b * alpha + dst_b * dst_a * (1.0 - alpha)) / out_a,
                )
            } else {
                (0.0, 0.0, 0.0)
            };
            buf[idx] = (out_r * 255.0).round().clamp(0.0, 255.0) as u8;
            buf[idx + 1] = (out_g * 255.0).round().clamp(0.0, 255.0) as u8;
            buf[idx + 2] = (out_b * 255.0).round().clamp(0.0, 255.0) as u8;
            buf[idx + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

fn parse_hex_color(value: &str) -> Option<(u8, u8, u8)> {
    let trimmed = value.trim().trim_start_matches('#');
    if trimmed.len() < 6 {
        return None;
    }
    let r = u8::from_str_radix(&trimmed[0..2], 16).ok()?;
    let g = u8::from_str_radix(&trimmed[2..4], 16).ok()?;
    let b = u8::from_str_radix(&trimmed[4..6], 16).ok()?;
    Some((r, g, b))
}

// Silence unused-import lint if Path is not used elsewhere in this module.
#[allow(dead_code)]
fn _unused_path_marker(_p: &Path) {}

// Silence unused-import lint.
#[allow(dead_code)]
fn _unused_image_marker(_img: &RgbaImage) {}
