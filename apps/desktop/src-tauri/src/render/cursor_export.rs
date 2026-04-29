//! Pre-renders the editor's cursor overlay (cursor dot + click highlight,
//! annotations, drop shadow) as an alpha QTRLE-in-MOV video so it can be muxed
//! onto the main export via a single FFmpeg `overlay` filter. Mirrors the
//! WebGL2 preview in `src/components/editor/VideoPreview.svelte`.
//!
//! QTRLE (QuickTime Animation, fourcc `rle `) is a lossless RLE codec with
//! true RGBA alpha support that ships with every FFmpeg build. We previously
//! used `libvpx-vp9 -pix_fmt yuva420p`, but the gyan.dev Windows builds (and
//! several Linux distros) silently drop the alpha plane during VP9 encode
//! — the overlay file ends up `pix_fmt=yuv420p` and decodes opaque, painting
//! the entire source area black during the final composite. QTRLE round-trips
//! alpha cleanly and compresses the (mostly transparent) cursor frames very
//! efficiently. The intermediate file lives in a scratch directory that the
//! TempDirGuard wipes after export.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{anyhow, Context, Result};
use image::{ImageReader, RgbaImage};

use crate::cursor::CursorTrack;
use crate::render::graph::RenderState;
use crate::render::node_types::{Annotation, AnnotationKind};

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

impl TempDirGuard {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
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
    let overlay_path = scratch_dir.join("cursor.mov");

    // Precompute derived settings (mirrors VideoPreview.svelte's draw loop).
    // Note: callers also invoke this overlay pass when only the drop-shadow
    // is enabled (the shadow draws further down the FFmpeg graph but lives
    // in the same alpha-VP9 overlay file), so we deliberately allow
    // both flags to be false here. The frame loop below will simply emit
    // fully-transparent frames in that case — composited as a no-op.
    let cursor_enabled = request.render_state.cursor_enabled;

    // Cursor radius in canvas pixels. WebGL shader uses:
    //   const cursorRadiusCanvas = (cs.size * 2 * canvasEl.width) / compW;
    // where compW = source_width + padding * 2.
    let comp_w = request.source_width + request.padding * 2;
    let cursor_radius_canvas = if comp_w > 0 {
        ((request.render_state.cursor_size * 2.0) * request.canvas_width as f64 / comp_w as f64)
            .max(2.0)
    } else {
        2.0
    };

    // Parse highlight color.
    let (hr, hg, hb) =
        parse_hex_color(&request.render_state.cursor_highlight_color).unwrap_or((0x3b, 0x82, 0xf6));

    // Allocate one reusable frame buffer.
    let canvas_w = request.canvas_width as usize;
    let canvas_h = request.canvas_height as usize;
    let bytes_per_frame = canvas_w * canvas_h * 4;
    let mut frame = vec![0u8; bytes_per_frame];

    // Spawn FFmpeg to encode raw RGBA → QTRLE-in-MOV. QTRLE is a lossless
    // RLE codec with true alpha (`-pix_fmt argb`) that compresses
    // mostly-transparent frames very efficiently — exactly the shape of a
    // cursor/annotation overlay. We do NOT use `-crf` / `-b:v` here: QTRLE
    // is lossless and ignores rate-control flags.
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
            "qtrle",
            "-pix_fmt",
            "argb",
        ])
        .arg(overlay_path.to_string_lossy().as_ref())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    crate::ffmpeg::configure_silent_command(&mut ffmpeg);

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
    let highlight_alpha_base =
        (request.render_state.cursor_highlight_opacity / 100.0).clamp(0.0, 1.0);

    // Pre-decode every image referenced by an Image annotation. The hybrid-
    // raster pipeline can produce many of these (one per text annotation),
    // but the count is bounded by the project size — far cheaper to decode
    // once than to re-decode per frame.
    let mut image_cache = build_image_cache(&request.render_state.annotations);
    // Pre-decode the cursor sprite (rest + press) once. Same cache so the
    // same blend_pixel path serves every overlay sprite.
    const CURSOR_SPRITE_KEY_REST: &str = "__recast_cursor_rest__";
    const CURSOR_SPRITE_KEY_PRESS: &str = "__recast_cursor_press__";
    if let Some(url) = &request.render_state.cursor_sprite_rest {
        if let Some(img) = decode_data_url(url) {
            image_cache.insert(CURSOR_SPRITE_KEY_REST.into(), img);
        }
    }
    if let Some(url) = &request.render_state.cursor_sprite_press {
        if let Some(img) = decode_data_url(url) {
            image_cache.insert(CURSOR_SPRITE_KEY_PRESS.into(), img);
        }
    }
    let cursor_sprite_active = image_cache.contains_key(CURSOR_SPRITE_KEY_REST);

    for i in 0..frame_count {
        // Clear frame to transparent.
        frame.fill(0);

        // Wall-clock time relative to the trimmed output, mapped to cursor-track time.
        let t_out_us = (i as u64 * 1_000_000) / request.fps as u64;
        let t_track_us = trim_start_us + t_out_us;
        // `t_track_secs` is the project-timeline time. Annotation/zoom-region
        // start/end fields are stored in timeline coordinates, so every check
        // against them must use this value — using output-stream time would
        // skip annotations whose timeline range falls before/around
        // `trim_start`, the same class of bug the FFmpeg zoom LUT had.
        let t_track_secs = t_track_us as f64 / 1_000_000.0;

        // Render in z-order so stacking is deterministic; skip hidden so the
        // user-toggled visibility flag matches the preview. `z_index` defaults
        // to 0 for v1 projects, which preserves insertion order via the stable
        // sort below.
        for annotation in sorted_visible_annotations(&request.render_state.annotations) {
            draw_annotation(
                &mut frame,
                canvas_w,
                canvas_h,
                annotation,
                &request,
                t_track_secs,
                &image_cache,
            );
        }

        if !cursor_enabled {
            stdin
                .write_all(&frame)
                .context("failed to write cursor frame to ffmpeg stdin")?;
            continue;
        }

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

        // Idle hide — smooth fade rather than a hard cut. Mirrors
        // `idleAlphaAt` in VideoPreview.svelte; same constants. When the
        // alpha is exactly 0 we skip the rest of the cursor work for the
        // frame; partial alpha multiplies through the dot/halo/sprite.
        let idle_alpha = if request.render_state.cursor_hide_when_idle {
            cursor_idle_alpha(t_track_us, &track.idle_periods, idle_timeout_us)
        } else {
            1.0
        };
        if idle_alpha <= 0.0 {
            stdin
                .write_all(&frame)
                .context("failed to write cursor frame to ffmpeg stdin")?;
            continue;
        }

        // Apply zoom transform in source-video coordinates. Zoom regions
        // index by timeline time (same as the FFmpeg-side LUT).
        let (mut cursor_source_x, mut cursor_source_y) = (sample.x, sample.y);
        if let Some((scale, center_x, center_y)) =
            active_zoom_at(&request.render_state.zoom_regions, t_track_secs)
        {
            let src_cx = center_x.clamp(0.0, 1.0) * request.source_width as f64;
            let src_cy = center_y.clamp(0.0, 1.0) * request.source_height as f64;
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
        let cursor_canvas_x = (request.padding as f64 + cursor_source_x) * scale_canvas;
        let cursor_canvas_y = (request.padding as f64 + cursor_source_y) * scale_canvas;

        // Click highlight halo — drawn underneath the dot/sprite so both
        // the soft-dot and macOS sprite share the same press indicator.
        let show_highlight =
            request.render_state.cursor_highlight_clicks && (sample.left_down || sample.right_down);
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
                highlight_alpha_base * idle_alpha,
            );
        }

        if cursor_sprite_active {
            // SVG sprite path — composite the rasterized cursor at the
            // sample position. Press state picks the alt sprite when
            // available (falls back to rest in JS if the style has none).
            let pressed = sample.left_down || sample.right_down;
            let key = if pressed && image_cache.contains_key(CURSOR_SPRITE_KEY_PRESS) {
                CURSOR_SPRITE_KEY_PRESS
            } else {
                CURSOR_SPRITE_KEY_REST
            };
            if let Some(img) = image_cache.get(key) {
                let hotspot = if pressed {
                    request
                        .render_state
                        .cursor_sprite_hotspot_press
                        .or(request.render_state.cursor_sprite_hotspot_rest)
                        .unwrap_or([0.5, 0.5])
                } else {
                    request
                        .render_state
                        .cursor_sprite_hotspot_rest
                        .unwrap_or([0.5, 0.5])
                };
                // Sprite size: source-pixel design size from JS, mapped to
                // canvas pixels with the same `scale_canvas` factor used
                // for the cursor position above.
                let sprite_source_px = request
                    .render_state
                    .cursor_sprite_size_px
                    .unwrap_or(request.render_state.cursor_size * 16.0);
                let target_size_px = sprite_source_px * scale_canvas;
                blit_cursor_sprite(
                    &mut frame,
                    canvas_w,
                    canvas_h,
                    img,
                    cursor_canvas_x,
                    cursor_canvas_y,
                    target_size_px,
                    hotspot,
                    idle_alpha,
                );
            }
        } else {
            // Soft-dot path (white, 90% alpha) — unchanged behaviour for
            // projects on the default `dot` style.
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
                0.9 * idle_alpha,
            );
        }

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
        return Err(anyhow!(
            "ffmpeg cursor overlay encode failed: {stderr_text}"
        ));
    }

    // Sanity check: the MOV must exist and be > 0 bytes.
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

//  Cursor interpolation (mirror of VideoPreview.svelte:317-342)

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

//  Zoom lookup (mirror of nested_region_expr in graph.rs)

fn active_zoom_at(
    regions: &[crate::render::node_types::ZoomRegion],
    t_secs: f64,
) -> Option<(f64, f64, f64)> {
    // Match the WebGL preview: the first region whose [start, end] contains t.
    for region in regions {
        if t_secs >= region.start && t_secs <= region.end {
            let scale = region.scale_at(t_secs).max(1.0);
            if scale > 1.0001 {
                return Some((scale, region.center_x, region.center_y));
            }
        }
    }
    None
}

//  Pixel drawing

fn draw_annotation(
    frame: &mut [u8],
    width: usize,
    height: usize,
    annotation: &Annotation,
    request: &CursorOverlayRequest,
    t_secs: f64,
    image_cache: &HashMap<String, RgbaImage>,
) {
    let opacity = annotation_opacity(annotation, t_secs);
    if opacity <= 0.0 {
        return;
    }

    match &annotation.kind {
        AnnotationKind::Rect { .. } | AnnotationKind::Ellipse { .. } => {
            draw_shape(frame, width, height, annotation, request, t_secs, opacity);
        }
        AnnotationKind::Arrow {
            x1,
            y1,
            x2,
            y2,
            head_size,
        } => {
            draw_arrow(
                frame,
                width,
                height,
                annotation,
                request,
                t_secs,
                opacity,
                *x1,
                *y1,
                *x2,
                *y2,
                *head_size,
            );
        }
        AnnotationKind::Image {
            x,
            y,
            w,
            h,
            path,
            opacity: img_opacity,
        } => {
            if let Some(img) = image_cache.get(path) {
                draw_image(
                    frame,
                    width,
                    height,
                    img,
                    request,
                    t_secs,
                    *x,
                    *y,
                    *w,
                    *h,
                    opacity * img_opacity.clamp(0.0, 1.0),
                );
            }
        }
        AnnotationKind::Unsupported => {
            // Silently skip — caller (JS) was supposed to rasterize/replace
            // before sending. Logged once at deserialize time would be ideal
            // but there's no hook for that here.
        }
    }
}

fn draw_shape(
    frame: &mut [u8],
    width: usize,
    height: usize,
    annotation: &Annotation,
    request: &CursorOverlayRequest,
    t_secs: f64,
    opacity: f64,
) {
    let Some((x, y, w, h, radius)) = annotation_box(annotation) else {
        return;
    };

    let (x1, y1) = uv_to_canvas(request, x, y, t_secs);
    let (x2, y2) = uv_to_canvas(request, x + w, y + h, t_secs);
    let x = x1.min(x2);
    let y = y1.min(y2);
    let w = (x1 - x2).abs();
    let h = (y1 - y2).abs();
    if w <= 0.5 || h <= 0.5 {
        return;
    }

    if let Some((r, g, b, a)) = parse_css_color(&annotation.fill) {
        if a > 0.0 {
            match annotation.kind {
                AnnotationKind::Rect { .. } => draw_rect(
                    frame,
                    width,
                    height,
                    x,
                    y,
                    w,
                    h,
                    radius * request.source_width.min(request.source_height) as f64,
                    r,
                    g,
                    b,
                    a * opacity,
                    true,
                    1.0,
                ),
                AnnotationKind::Ellipse { .. } => draw_ellipse(
                    frame,
                    width,
                    height,
                    x,
                    y,
                    w,
                    h,
                    r,
                    g,
                    b,
                    a * opacity,
                    true,
                    1.0,
                ),
                _ => {}
            }
        }
    }

    // v2 stroke-style fallback: dashed/dotted patterns require segmenting the
    // path, which the current SDF-based draw_rect/draw_ellipse can't express.
    // The preview canvas honors them; export falls back to solid here. See
    // docs/rfcs/annotations-v2.md (Phase F follow-up). Carrying the field on
    // the wire so the v2.1 Rust dash impl is a renderer-only change.
    if annotation.stroke.width > 0.0 {
        if let Some((r, g, b, a)) = parse_css_color(&annotation.stroke.color) {
            if a > 0.0 {
                let stroke_px = (annotation.stroke.width * request.source_width as f64).max(1.0);
                match annotation.kind {
                    AnnotationKind::Rect { .. } => draw_rect(
                        frame,
                        width,
                        height,
                        x,
                        y,
                        w,
                        h,
                        radius * request.source_width.min(request.source_height) as f64,
                        r,
                        g,
                        b,
                        a * opacity,
                        false,
                        stroke_px,
                    ),
                    AnnotationKind::Ellipse { .. } => draw_ellipse(
                        frame,
                        width,
                        height,
                        x,
                        y,
                        w,
                        h,
                        r,
                        g,
                        b,
                        a * opacity,
                        false,
                        stroke_px,
                    ),
                    _ => {}
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_arrow(
    frame: &mut [u8],
    width: usize,
    height: usize,
    annotation: &Annotation,
    request: &CursorOverlayRequest,
    t_secs: f64,
    opacity: f64,
    x1_uv: f64,
    y1_uv: f64,
    x2_uv: f64,
    y2_uv: f64,
    head_size: f64,
) {
    let stroke_color = parse_css_color(&annotation.stroke.color);
    let Some((sr, sg, sb, sa)) = stroke_color else {
        return;
    };
    if sa <= 0.0 {
        return;
    }
    let stroke_px = (annotation.stroke.width * request.source_width as f64).max(1.0);

    let (cx1, cy1) = uv_to_canvas(request, x1_uv, y1_uv, t_secs);
    let (cx2, cy2) = uv_to_canvas(request, x2_uv, y2_uv, t_secs);
    let dx = cx2 - cx1;
    let dy = cy2 - cy1;
    let line_len = (dx * dx + dy * dy).sqrt();
    if line_len < 1.0 {
        return;
    }

    let head_len = (head_size.clamp(0.05, 0.4) * line_len).max(stroke_px * 2.0);
    let head_width = head_len * 0.7;
    // Trim the line so it ends at the base of the head, otherwise the
    // capsule pokes through the triangle and looks blunt.
    let ux = dx / line_len;
    let uy = dy / line_len;
    let line_end_x = cx2 - ux * head_len;
    let line_end_y = cy2 - uy * head_len;
    let base_cx = line_end_x;
    let base_cy = line_end_y;
    let nx = -uy;
    let ny = ux;

    // Capsule line via SDF.
    let alpha = sa * opacity;
    draw_capsule(
        frame, width, height, cx1, cy1, line_end_x, line_end_y, stroke_px, sr, sg, sb, alpha,
    );

    // Filled arrowhead triangle: tip at (cx2, cy2), base perpendicular.
    let tip_x = cx2;
    let tip_y = cy2;
    let base_left_x = base_cx + nx * head_width * 0.5;
    let base_left_y = base_cy + ny * head_width * 0.5;
    let base_right_x = base_cx - nx * head_width * 0.5;
    let base_right_y = base_cy - ny * head_width * 0.5;
    draw_triangle_filled(
        frame, width, height, tip_x, tip_y, base_left_x, base_left_y, base_right_x, base_right_y,
        sr, sg, sb, alpha,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_image(
    frame: &mut [u8],
    width: usize,
    height: usize,
    img: &RgbaImage,
    request: &CursorOverlayRequest,
    t_secs: f64,
    x_uv: f64,
    y_uv: f64,
    w_uv: f64,
    h_uv: f64,
    alpha: f64,
) {
    if w_uv <= 0.0 || h_uv <= 0.0 || alpha <= 0.0 {
        return;
    }
    let (cx1, cy1) = uv_to_canvas(request, x_uv, y_uv, t_secs);
    let (cx2, cy2) = uv_to_canvas(request, x_uv + w_uv, y_uv + h_uv, t_secs);
    let dx = cx1.min(cx2);
    let dy = cy1.min(cy2);
    let dw = (cx2 - cx1).abs();
    let dh = (cy2 - cy1).abs();
    if dw < 1.0 || dh < 1.0 {
        return;
    }
    let (img_w, img_h) = img.dimensions();
    if img_w == 0 || img_h == 0 {
        return;
    }
    let x_min = dx.floor().max(0.0) as usize;
    let y_min = dy.floor().max(0.0) as usize;
    let x_max = (dx + dw).ceil().min(width as f64 - 1.0).max(0.0) as usize;
    let y_max = (dy + dh).ceil().min(height as f64 - 1.0).max(0.0) as usize;
    for py in y_min..=y_max {
        // Map dst pixel back into image space (nearest-neighbour). Bilinear
        // would look nicer but a single-pass nearest is plenty for screen
        // recordings where the rasterized text PNG already matches the
        // intended pixel size to within a few percent.
        let v = ((py as f64 + 0.5 - dy) / dh).clamp(0.0, 0.999);
        let sy = (v * img_h as f64) as u32;
        for px in x_min..=x_max {
            let u = ((px as f64 + 0.5 - dx) / dw).clamp(0.0, 0.999);
            let sx = (u * img_w as f64) as u32;
            let pixel = img.get_pixel(sx, sy);
            let src_a = pixel[3] as f64 / 255.0 * alpha;
            if src_a <= 0.0 {
                continue;
            }
            blend_pixel(
                frame,
                width,
                px,
                py,
                pixel[0],
                pixel[1],
                pixel[2],
                src_a,
            );
        }
    }
}

/// Smooth idle-fade — mirror of `idleAlphaAt` in VideoPreview.svelte.
/// Returns 1.0 when the cursor should be fully visible at `t_us`, 0.0
/// inside an idle period (past the timeout + fade-in), and a linear ramp
/// across 200 ms at each boundary so the cursor dissolves rather than
/// blinks. The constants match the JS side exactly.
fn cursor_idle_alpha(
    t_us: u64,
    idle_periods: &[crate::cursor::smoothing::IdlePeriod],
    idle_timeout_us: u64,
) -> f64 {
    const FADE_US: u64 = 200_000;
    for period in idle_periods {
        let fade_start = period.start_us.saturating_add(idle_timeout_us);
        if period.end_us <= fade_start {
            continue;
        }
        let fade_end = (fade_start + FADE_US).min(period.end_us);
        let resume_start = period.end_us.saturating_sub(FADE_US).max(fade_end);
        if t_us < fade_start || t_us > period.end_us {
            continue;
        }
        if t_us >= fade_end && t_us <= resume_start {
            return 0.0;
        }
        if t_us < fade_end {
            let span = (fade_end - fade_start).max(1) as f64;
            return 1.0 - (t_us - fade_start) as f64 / span;
        }
        let span = (period.end_us - resume_start).max(1) as f64;
        return 1.0 - (period.end_us - t_us) as f64 / span;
    }
    1.0
}

/// Blit an SVG-rasterized cursor sprite at a canvas-pixel position with
/// bilinear sampling. The sprite is anchored by `hotspot_uv` (0..1 within
/// the sprite) so the click point lands on (`canvas_x`, `canvas_y`)
/// regardless of size.
#[allow(clippy::too_many_arguments)]
fn blit_cursor_sprite(
    frame: &mut [u8],
    width: usize,
    height: usize,
    img: &RgbaImage,
    canvas_x: f64,
    canvas_y: f64,
    target_size_px: f64,
    hotspot_uv: [f64; 2],
    alpha: f64,
) {
    if alpha <= 0.0 || target_size_px < 1.0 {
        return;
    }
    let dst_w = target_size_px;
    let dst_h = target_size_px;
    let dx = canvas_x - hotspot_uv[0] * dst_w;
    let dy = canvas_y - hotspot_uv[1] * dst_h;
    let (img_w, img_h) = img.dimensions();
    if img_w == 0 || img_h == 0 {
        return;
    }
    let x_min = dx.floor().max(0.0) as usize;
    let y_min = dy.floor().max(0.0) as usize;
    let x_max = (dx + dst_w).ceil().min(width as f64 - 1.0).max(0.0) as usize;
    let y_max = (dy + dst_h).ceil().min(height as f64 - 1.0).max(0.0) as usize;
    if x_max < x_min || y_max < y_min {
        return;
    }
    for py in y_min..=y_max {
        let v = ((py as f64 + 0.5 - dy) / dst_h).clamp(0.0, 0.9999);
        let sy_f = v * (img_h - 1) as f64;
        let sy0 = sy_f.floor() as u32;
        let sy1 = (sy0 + 1).min(img_h - 1);
        let fy = sy_f - sy0 as f64;
        for px in x_min..=x_max {
            let u = ((px as f64 + 0.5 - dx) / dst_w).clamp(0.0, 0.9999);
            let sx_f = u * (img_w - 1) as f64;
            let sx0 = sx_f.floor() as u32;
            let sx1 = (sx0 + 1).min(img_w - 1);
            let fx = sx_f - sx0 as f64;

            let p00 = img.get_pixel(sx0, sy0).0;
            let p10 = img.get_pixel(sx1, sy0).0;
            let p01 = img.get_pixel(sx0, sy1).0;
            let p11 = img.get_pixel(sx1, sy1).0;
            let mix = |a: u8, b: u8, c: u8, d: u8| -> f64 {
                let top = a as f64 * (1.0 - fx) + b as f64 * fx;
                let bot = c as f64 * (1.0 - fx) + d as f64 * fx;
                top * (1.0 - fy) + bot * fy
            };
            let r = mix(p00[0], p10[0], p01[0], p11[0]);
            let g = mix(p00[1], p10[1], p01[1], p11[1]);
            let b = mix(p00[2], p10[2], p01[2], p11[2]);
            let a = mix(p00[3], p10[3], p01[3], p11[3]) / 255.0 * alpha;
            if a <= 0.0 {
                continue;
            }
            blend_pixel(frame, width, px, py, r as u8, g as u8, b as u8, a);
        }
    }
}

fn build_image_cache(annotations: &[Annotation]) -> HashMap<String, RgbaImage> {
    let mut cache = HashMap::new();
    for anno in annotations {
        if let AnnotationKind::Image { path, .. } = &anno.kind {
            if cache.contains_key(path) {
                continue;
            }
            if let Some(img) = decode_image_path_or_url(path) {
                cache.insert(path.clone(), img);
            }
        }
    }
    cache
}

/// Decode either a `data:image/png;base64,...` URL or a filesystem path.
/// Returns `None` and logs on failure rather than propagating — the caller
/// (export pipeline) should not abort an entire export over one bad image.
fn decode_image_path_or_url(path: &str) -> Option<RgbaImage> {
    use base64::Engine;
    let decoded: Result<image::DynamicImage> = if path.starts_with("data:") {
        let comma = path.find(',').ok_or_else(|| anyhow!("malformed data URL"));
        comma.and_then(|idx| {
            let payload = &path[idx + 1..];
            base64::engine::general_purpose::STANDARD
                .decode(payload)
                .map_err(|e| anyhow!(e))
                .and_then(|bytes| image::load_from_memory(&bytes).map_err(|e| anyhow!(e)))
        })
    } else {
        ImageReader::open(path)
            .and_then(|r| r.with_guessed_format())
            .map_err(|e| anyhow!(e))
            .and_then(|r| r.decode().map_err(|e| anyhow!(e)))
    };
    match decoded {
        Ok(img) => Some(img.to_rgba8()),
        Err(e) => {
            let preview = if path.len() > 40 { &path[..40] } else { path };
            log::warn!("failed to decode image ({preview}…): {e}");
            None
        }
    }
}

/// Convenience wrapper used by the cursor sprite preload — same decode
/// path as annotations but with a clearer name at the call site.
fn decode_data_url(url: &str) -> Option<RgbaImage> {
    decode_image_path_or_url(url)
}

#[allow(clippy::too_many_arguments)]
fn draw_capsule(
    buf: &mut [u8],
    width: usize,
    height: usize,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    thickness: f64,
    r: u8,
    g: u8,
    b: u8,
    alpha: f64,
) {
    if alpha <= 0.0 {
        return;
    }
    let radius = thickness * 0.5;
    let pad = radius + 2.0;
    let x_min = (x1.min(x2) - pad).floor().max(0.0) as usize;
    let y_min = (y1.min(y2) - pad).floor().max(0.0) as usize;
    let x_max = ((x1.max(x2) + pad).ceil() as i64)
        .min(width as i64 - 1)
        .max(0) as usize;
    let y_max = ((y1.max(y2) + pad).ceil() as i64)
        .min(height as i64 - 1)
        .max(0) as usize;
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = (dx * dx + dy * dy).max(1e-6);
    for py in y_min..=y_max {
        for px in x_min..=x_max {
            let fx = px as f64 + 0.5 - x1;
            let fy = py as f64 + 0.5 - y1;
            let t = ((fx * dx + fy * dy) / len_sq).clamp(0.0, 1.0);
            let cx = t * dx;
            let cy = t * dy;
            let dist = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            // 1-pixel anti-aliased edge.
            let coverage = (1.0 - (dist - (radius - 0.5)).clamp(0.0, 1.0)).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }
            blend_pixel(buf, width, px, py, r, g, b, alpha * coverage);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_triangle_filled(
    buf: &mut [u8],
    width: usize,
    height: usize,
    ax: f64,
    ay: f64,
    bx: f64,
    by: f64,
    cx: f64,
    cy: f64,
    r: u8,
    g: u8,
    b: u8,
    alpha: f64,
) {
    if alpha <= 0.0 {
        return;
    }
    let x_min = ax.min(bx).min(cx).floor().max(0.0) as usize;
    let y_min = ay.min(by).min(cy).floor().max(0.0) as usize;
    let x_max = ((ax.max(bx).max(cx)).ceil() as i64)
        .min(width as i64 - 1)
        .max(0) as usize;
    let y_max = ((ay.max(by).max(cy)).ceil() as i64)
        .min(height as i64 - 1)
        .max(0) as usize;
    // Edge-function rasterizer; sign indicates which side of an edge a point
    // lies on. Inside the triangle, all three edge functions agree in sign.
    let sign = |px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64| -> f64 {
        (px - bx) * (ay - by) - (ax - bx) * (py - by)
    };
    for py in y_min..=y_max {
        for px in x_min..=x_max {
            let pcx = px as f64 + 0.5;
            let pcy = py as f64 + 0.5;
            let d1 = sign(pcx, pcy, ax, ay, bx, by);
            let d2 = sign(pcx, pcy, bx, by, cx, cy);
            let d3 = sign(pcx, pcy, cx, cy, ax, ay);
            let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
            let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
            if has_neg && has_pos {
                continue;
            }
            blend_pixel(buf, width, px, py, r, g, b, alpha);
        }
    }
}

fn annotation_box(annotation: &Annotation) -> Option<(f64, f64, f64, f64, f64)> {
    match annotation.kind {
        AnnotationKind::Rect { x, y, w, h, radius } => {
            let left = x.min(x + w);
            let top = y.min(y + h);
            Some((left, top, w.abs(), h.abs(), radius.max(0.0)))
        }
        AnnotationKind::Ellipse { x, y, w, h } => {
            let left = x.min(x + w);
            let top = y.min(y + h);
            Some((left, top, w.abs(), h.abs(), 0.0))
        }
        _ => None,
    }
}

fn annotation_opacity(annotation: &Annotation, t_secs: f64) -> f64 {
    if t_secs < annotation.start || t_secs > annotation.end {
        return 0.0;
    }
    let duration = (annotation.end - annotation.start).max(0.0);
    let ramp_in = annotation.ramp_in.max(0.0).min(duration * 0.5);
    let ramp_out = annotation.ramp_out.max(0.0).min(duration * 0.5);
    let hold_start = annotation.start + ramp_in;
    let hold_end = annotation.end - ramp_out;
    let raw = if ramp_in > 0.0 && t_secs < hold_start {
        let phase = ((t_secs - annotation.start) / ramp_in).clamp(0.0, 1.0);
        annotation.ease_in.y(phase as f32) as f64
    } else if ramp_out > 0.0 && t_secs > hold_end {
        let phase = ((annotation.end - t_secs) / ramp_out).clamp(0.0, 1.0);
        annotation.ease_out.y(phase as f32) as f64
    } else {
        1.0
    };
    // Multiply by the v2 master opacity. Defaults to 1.0 for v1 projects via
    // the serde fallback so the export stays byte-identical to v1 unless the
    // user explicitly dialled the master slider.
    raw * annotation.opacity.clamp(0.0, 1.0)
}

/// Sort + filter annotations for export. Hidden annotations are dropped; the
/// rest come back sorted by `(z_index, original_index)` so equal z values
/// preserve insertion order (stable sort). Mirrors the canvas overlay's
/// `annotationsByZ` derivation in the editor store.
fn sorted_visible_annotations(annotations: &[Annotation]) -> Vec<&Annotation> {
    let mut indexed: Vec<(usize, &Annotation)> = annotations
        .iter()
        .enumerate()
        .filter(|(_, a)| !a.hidden)
        .collect();
    indexed.sort_by(|(ai, a), (bi, b)| a.z_index.cmp(&b.z_index).then(ai.cmp(bi)));
    indexed.into_iter().map(|(_, a)| a).collect()
}

fn uv_to_canvas(request: &CursorOverlayRequest, x: f64, y: f64, t_secs: f64) -> (f64, f64) {
    let mut uv_x = x;
    let mut uv_y = y;
    if let Some((scale, center_x, center_y)) = active_zoom_at(&request.render_state.zoom_regions, t_secs) {
        uv_x = (uv_x - center_x) * scale + center_x;
        uv_y = (uv_y - center_y) * scale + center_y;
    }
    let source_x = uv_x * request.source_width as f64;
    let source_y = uv_y * request.source_height as f64;
    let scale_canvas =
        request.canvas_width as f64 / (request.source_width + request.padding * 2) as f64;
    (
        (request.padding as f64 + source_x) * scale_canvas,
        (request.padding as f64 + source_y) * scale_canvas,
    )
}

#[allow(clippy::too_many_arguments)]
fn draw_rect(
    buf: &mut [u8],
    width: usize,
    height: usize,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    radius: f64,
    r: u8,
    g: u8,
    b: u8,
    alpha: f64,
    fill: bool,
    stroke: f64,
) {
    let x_min = x.floor().max(0.0) as usize;
    let y_min = y.floor().max(0.0) as usize;
    let x_max = (x + w).ceil().min(width as f64 - 1.0).max(0.0) as usize;
    let y_max = (y + h).ceil().min(height as f64 - 1.0).max(0.0) as usize;
    let cx = x + w * 0.5;
    let cy = y + h * 0.5;
    let hx = w * 0.5;
    let hy = h * 0.5;
    let rr = radius.min(hx.min(hy)).max(0.0);
    for py in y_min..=y_max {
        for px in x_min..=x_max {
            let sd = rounded_rect_sdf(px as f64 + 0.5 - cx, py as f64 + 0.5 - cy, hx, hy, rr);
            let coverage = if fill {
                (1.0 - smoothstep(-1.0, 0.0, sd)).clamp(0.0, 1.0)
            } else {
                (1.0 - smoothstep(stroke - 1.0, stroke, sd.abs())).clamp(0.0, 1.0)
                    * (1.0 - smoothstep(-1.0, 0.0, sd))
            };
            blend_pixel(buf, width, px, py, r, g, b, alpha * coverage);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_ellipse(
    buf: &mut [u8],
    width: usize,
    height: usize,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    r: u8,
    g: u8,
    b: u8,
    alpha: f64,
    fill: bool,
    stroke: f64,
) {
    let x_min = x.floor().max(0.0) as usize;
    let y_min = y.floor().max(0.0) as usize;
    let x_max = (x + w).ceil().min(width as f64 - 1.0).max(0.0) as usize;
    let y_max = (y + h).ceil().min(height as f64 - 1.0).max(0.0) as usize;
    let cx = x + w * 0.5;
    let cy = y + h * 0.5;
    let rx = (w * 0.5).max(0.5);
    let ry = (h * 0.5).max(0.5);
    for py in y_min..=y_max {
        for px in x_min..=x_max {
            let nx = (px as f64 + 0.5 - cx) / rx;
            let ny = (py as f64 + 0.5 - cy) / ry;
            let dist = (nx * nx + ny * ny).sqrt();
            let edge_px = 1.0 / rx.min(ry);
            let coverage = if fill {
                (1.0 - smoothstep(1.0 - edge_px, 1.0, dist)).clamp(0.0, 1.0)
            } else {
                let stroke_n = stroke / rx.min(ry);
                (1.0 - smoothstep(stroke_n - edge_px, stroke_n, (dist - 1.0).abs()))
                    .clamp(0.0, 1.0)
            };
            blend_pixel(buf, width, px, py, r, g, b, alpha * coverage);
        }
    }
}

fn rounded_rect_sdf(px: f64, py: f64, hx: f64, hy: f64, r: f64) -> f64 {
    let qx = px.abs() - hx + r;
    let qy = py.abs() - hy + r;
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
}

fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0).max(1e-6)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn blend_pixel(buf: &mut [u8], width: usize, x: usize, y: usize, r: u8, g: u8, b: u8, alpha: f64) {
    if alpha <= 0.0 {
        return;
    }
    let idx = y * width * 4 + x * 4;
    let dst_r = buf[idx] as f64 / 255.0;
    let dst_g = buf[idx + 1] as f64 / 255.0;
    let dst_b = buf[idx + 2] as f64 / 255.0;
    let dst_a = buf[idx + 3] as f64 / 255.0;
    let src_r = r as f64 / 255.0;
    let src_g = g as f64 / 255.0;
    let src_b = b as f64 / 255.0;
    let alpha = alpha.clamp(0.0, 1.0);
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

fn parse_css_color(value: &str) -> Option<(u8, u8, u8, f64)> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("transparent") {
        return None;
    }

    if let Some((r, g, b)) = parse_hex_color(value) {
        let trimmed = value.trim().trim_start_matches('#');
        let alpha = if trimmed.len() >= 8 {
            u8::from_str_radix(&trimmed[6..8], 16).ok()? as f64 / 255.0
        } else {
            1.0
        };
        return Some((r, g, b, alpha));
    }

    let lower = value.to_ascii_lowercase();
    let body = lower
        .strip_prefix("rgba(")
        .or_else(|| lower.strip_prefix("rgb("))?
        .trim_end_matches(')');
    let parts: Vec<&str> = body.split(',').map(str::trim).collect();
    if parts.len() < 3 {
        return None;
    }
    let r = parts[0].parse::<f64>().ok()?.round().clamp(0.0, 255.0) as u8;
    let g = parts[1].parse::<f64>().ok()?.round().clamp(0.0, 255.0) as u8;
    let b = parts[2].parse::<f64>().ok()?.round().clamp(0.0, 255.0) as u8;
    let a = parts
        .get(3)
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    Some((r, g, b, a))
}

