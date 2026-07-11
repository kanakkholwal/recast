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
use rayon::prelude::*;

use crate::cursor::smoothing::{
    smooth_cursor_path, smoothing_strength_to_sigma_ms, SmoothedSample,
};
use crate::cursor::CursorTrack;
use crate::render::cursor_anim::{
    build_press_events_from_iter, click_anchor_at, click_bounce_scale, click_highlight_at,
    idle_sway_offset, motion_blur_step_alpha, press_state_at,
};
use crate::render::graph::RenderState;
use crate::render::node_types::{
    Annotation, AnnotationAnchor, AnnotationGlow, AnnotationKind, AnnotationStroke,
};

/// Input for pre-rendering a cursor overlay track.
#[derive(Debug, Clone)]
pub struct CursorOverlayRequest {
    /// Path to the cursor.json track file (from `.recast` project).
    pub cursor_track_path: PathBuf,
    /// Comp dimensions (= source + padding × 2). The overlay PNG is
    /// rendered at these dimensions even when the final canvas is larger
    /// (aspect-changing preset). The caller composites it at the comp's
    /// offset inside the canvas via the FFmpeg overlay filter, so we
    /// don't pipe gigabytes of RGBA through stdin for a tall 9:16 canvas.
    pub canvas_width: u32,
    pub canvas_height: u32,
    /// Source video dimensions (without padding).
    pub source_width: u32,
    pub source_height: u32,
    /// Padding around the source video inside the comp.
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

    // Pre-compute click rising-edge timestamps (seconds, on the cursor-track
    // clock) for the bounce curve. We treat any 0→1 transition on either
    // mouse button as a click impact, deduplicated by sample boundaries.
    let mut click_events_secs: Vec<f64> = Vec::new();
    {
        let mut prev_left = false;
        let mut prev_right = false;
        for s in &track.samples {
            let down_now = s.left_down || s.right_down;
            let was_down = prev_left || prev_right;
            if down_now && !was_down {
                click_events_secs.push(s.timestamp_us as f64 / 1_000_000.0);
            }
            prev_left = s.left_down;
            prev_right = s.right_down;
        }
    }

    // Pre-compute the full press-event list (down/up timestamps + click
    // anchor x/y). Drives the sprite preroll, visibility boost, click-
    // impact scale, AND the always-on click-anchor snap that ensures the
    // visual click lands on the captured target regardless of smoothing.
    // Mirrors `rebuildPressEvents` in VideoPreview.svelte. Built from raw
    // samples so neither timing nor position can drift with smoothing.
    let press_events = build_press_events_from_iter(track.samples.iter().map(|s| {
        (
            s.timestamp_us,
            s.x as f64,
            s.y as f64,
            s.left_down,
            s.right_down,
        )
    }));

    // Smooth the cursor PATH exactly like the WebGL preview. The click timing
    // and click positions above are taken from the RAW samples and the pinned
    // highlight, so this only reshapes motion — it can never move where/when a
    // click lands. Every position lookup below interpolates this smoothed
    // buffer instead of the raw track; without it the export drew the raw path
    // while the preview drew the smoothed one, and a zoom magnified the gap
    // into a visibly off cursor.
    let smoothed = smooth_cursor_path(
        &track.samples,
        smoothing_strength_to_sigma_ms(request.render_state.cursor_smoothing),
        request.render_state.cursor_snap_to_clicks,
        request.render_state.cursor_snap_window_ms,
    );

    /// Find the click event nearest `t_secs`. Returns the offset in ms
    /// (`t - click_t`, signed, negative = click is in the future) or None
    /// when the track has no clicks.
    fn nearest_click_offset_ms(events: &[f64], t_secs: f64) -> Option<f64> {
        let mut best: Option<f64> = None;
        for &e in events {
            let dt_ms = (t_secs - e) * 1000.0;
            match best {
                None => best = Some(dt_ms),
                Some(cur) if dt_ms.abs() < cur.abs() => best = Some(dt_ms),
                _ => {}
            }
        }
        best
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

    // Frame geometry. Each frame below allocates its own zero-filled buffer so
    // the render can fan out across cores (see the parallel driver after the
    // closure) — frames are independent timestamp lookups, so the only ordering
    // constraint is the sequential stdin write.
    let canvas_w = request.canvas_width as usize;
    let canvas_h = request.canvas_height as usize;
    let bytes_per_frame = canvas_w * canvas_h * 4;

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
    const CURSOR_SPRITE_KEY_RIGHT_PRESS: &str = "__recast_cursor_right_press__";
    const CURSOR_SPRITE_KEY_DRAG: &str = "__recast_cursor_drag__";
    for (url, key) in [
        (
            &request.render_state.cursor_sprite_rest,
            CURSOR_SPRITE_KEY_REST,
        ),
        (
            &request.render_state.cursor_sprite_press,
            CURSOR_SPRITE_KEY_PRESS,
        ),
        (
            &request.render_state.cursor_sprite_right_press,
            CURSOR_SPRITE_KEY_RIGHT_PRESS,
        ),
        (
            &request.render_state.cursor_sprite_drag,
            CURSOR_SPRITE_KEY_DRAG,
        ),
    ] {
        if let Some(url) = url {
            if let Some(img) = decode_data_url(url) {
                image_cache.insert(key.into(), img);
            }
        }
    }
    let cursor_sprite_active = image_cache.contains_key(CURSOR_SPRITE_KEY_REST);

    // Renders frame `i` into its own buffer. Pure function of `i` over the
    // precomputed, read-only state above (smoothed path, press events, zoom
    // LUT, idle periods, image cache), so it's safe to call concurrently.
    let render_one = |i: u64| -> Vec<u8> {
        // Fresh buffer, zero-filled = fully transparent.
        let mut frame = vec![0u8; bytes_per_frame];

        // Wall-clock time relative to the trimmed output, mapped to cursor-track time.
        let t_out_us = (i * 1_000_000) / request.fps as u64;
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
            return frame;
        }

        // Sample cursor position at this timestamp.
        let sample = match interpolate_cursor(&smoothed, t_track_us) {
            Some(s) => s,
            None => {
                // No cursor data — emit the empty (annotation-only) frame.
                return frame;
            }
        };

        if !sample.visible {
            return frame;
        }

        // Per-frame click state — sprite-key preroll, visibility boost,
        // click-impact scale. Pulled once and reused below; mirrors the
        // preview's pressStateAt so a frame at this timestamp looks the
        // same in the editor and the rendered MP4.
        let press = press_state_at(t_track_us as i64, &press_events);

        // Idle hide — smooth fade rather than a hard cut. Mirrors
        // `idleAlphaAt` in VideoPreview.svelte; same constants. The press
        // window can override an idle-zero so a click that lands deep in
        // an idle stretch still gets its anticipation + impact + recovery
        // visible (e.g. a viewer can see "user reaches in, clicks, leaves"
        // even though the cursor was hidden moments before).
        let idle_alpha_raw = if request.render_state.cursor_hide_when_idle {
            cursor_idle_alpha(t_track_us, &track.idle_periods, idle_timeout_us)
        } else {
            1.0
        };
        let idle_alpha = idle_alpha_raw.max(press.visible_alpha);
        if idle_alpha <= 0.0 {
            return frame;
        }

        // Apply zoom transform in source-video coordinates. Zoom regions
        // index by timeline time (same as the FFmpeg-side LUT).
        let (mut cursor_source_x, mut cursor_source_y) = (sample.x, sample.y);
        // Always-on click-anchor snap. Cosine ramp pulls the rendered
        // cursor through the captured click target inside the snap
        // window, so the click impact lands exactly where the user
        // clicked regardless of any smoothing applied upstream. Done
        // pre-zoom so the anchor x/y is in the same source-pixel space
        // as the captured sample.
        if let Some((ax, ay, w)) = click_anchor_at(t_track_us as i64, &press_events) {
            cursor_source_x = cursor_source_x * (1.0 - w) + ax * w;
            cursor_source_y = cursor_source_y * (1.0 - w) + ay * w;
        }
        if let Some((scale, center_x, center_y)) = active_zoom_at(
            &request.render_state.zoom_regions,
            t_track_secs,
            request.trim_start,
        ) {
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
                return frame;
            }
        }

        // Cursor-anim: idle sway. Adds a tiny sinusoidal wobble in source-px
        // for slow-moving cursors. We approximate cursor velocity by sampling
        // 16 ms in the past and measuring distance — keeps sway alive at rest
        // and tapers it cleanly during fast gestures.
        if request.render_state.cursor_sway > 0.0 {
            let velocity_px_per_s = {
                let lookback_us = 16_000_u64;
                let past_us = t_track_us.saturating_sub(lookback_us);
                if let Some(prev) = interpolate_cursor(&smoothed, past_us) {
                    let dt = (t_track_us - past_us) as f64 / 1_000_000.0;
                    if dt > 0.0 {
                        ((sample.x - prev.x).powi(2) + (sample.y - prev.y).powi(2)).sqrt() / dt
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            };
            let (dx, dy) = idle_sway_offset(
                t_track_us as f64 / 1000.0,
                request.render_state.cursor_sway,
                velocity_px_per_s,
            );
            cursor_source_x += dx;
            cursor_source_y += dy;
        }

        // Cursor-anim: click bounce — modulates a per-frame scale multiplier
        // applied to both the soft-dot radius and the sprite render size.
        //
        // The baseline `press.scale` (anticipation lift → impact snap →
        // bounce-back) is always on so every click reads as a deliberate
        // tap. The user-tunable `cursor_click_bounce` knob is composed on
        // top via the legacy `click_bounce_scale` curve when set — it adds
        // extra squash/overshoot for cinematic demos without flattening the
        // baseline impact when it's at zero.
        let user_bounce_factor = if request.render_state.cursor_click_bounce > 0.0 {
            if let Some(dt_ms) = nearest_click_offset_ms(&click_events_secs, t_track_secs) {
                click_bounce_scale(
                    dt_ms,
                    request.render_state.cursor_bounce_speed_ms.max(60.0),
                    request.render_state.cursor_click_bounce,
                )
            } else {
                1.0
            }
        } else {
            1.0
        };
        let bounce_scale = press.scale * user_bounce_factor;

        // Map source coords → canvas coords.
        // Video area in the canvas is [padding, padding + source_width].
        let scale_canvas =
            request.canvas_width as f64 / (request.source_width + request.padding * 2) as f64;
        let cursor_canvas_x = (request.padding as f64 + cursor_source_x) * scale_canvas;
        let cursor_canvas_y = (request.padding as f64 + cursor_source_y) * scale_canvas;

        // Per-frame motion-blur trail positions: sample the cursor track at
        // a few sub-frames into the past at decreasing alpha so the export
        // shows a velocity-proportional smear that tracks the actual motion
        // path (not a uniform blur). Strength and step count come from the
        // render state's motion-blur slider.
        let mb_strength = request.render_state.cursor_motion_blur.clamp(0.0, 1.0);
        let mut motion_trail: Vec<(f64, f64, f64)> = Vec::new(); // (canvas_x, canvas_y, alpha)
        if mb_strength > 0.0 {
            const TRAIL_STEPS: usize = 6;
            // 8ms per step keeps the trail visible at 60fps without smearing
            // into prior gestures.
            const STEP_DT_US: i64 = 8_000;
            for i in 1..=TRAIL_STEPS {
                let alpha = motion_blur_step_alpha(i, TRAIL_STEPS, mb_strength);
                if alpha <= 0.0 {
                    continue;
                }
                let past_us = t_track_us as i64 - (i as i64) * STEP_DT_US;
                if past_us < 0 {
                    continue;
                }
                let past_sample = match interpolate_cursor(&smoothed, past_us as u64) {
                    Some(s) if s.visible => s,
                    _ => continue,
                };
                let (mut px, mut py) = (past_sample.x, past_sample.y);
                if let Some((scale, cx, cy)) = active_zoom_at(
                    &request.render_state.zoom_regions,
                    past_us as f64 / 1_000_000.0,
                    request.trim_start,
                ) {
                    let scx = cx.clamp(0.0, 1.0) * request.source_width as f64;
                    let scy = cy.clamp(0.0, 1.0) * request.source_height as f64;
                    px = (px - scx) * scale + scx;
                    py = (py - scy) * scale + scy;
                }
                let cx = (request.padding as f64 + px) * scale_canvas;
                let cy = (request.padding as f64 + py) * scale_canvas;
                motion_trail.push((cx, cy, alpha));
            }
        }

        // Click highlight halo — PINNED to the captured click point + instant
        // (via `click_highlight_at` on the raw press events), NOT the smoothed
        // cursor. The ring therefore lands exactly where and when the click
        // happened, independent of smoothing — riding `cursor_canvas_*` made it
        // lag behind under smoothing, reading as delayed/off-target feedback.
        // Drawn underneath the dot/sprite so both share one press indicator.
        // Mirrors the pinned `u_highlightPos` halo in VideoPreview.svelte.
        if request.render_state.cursor_highlight_clicks {
            if let Some((click_x, click_y, hl_env)) =
                click_highlight_at(t_track_us as i64, &press_events)
            {
                // Same affine zoom as the cursor so the ring tracks the
                // zoomed video; only draw when the click point is inside the
                // visible (zoomed) source rect.
                let (mut hx, mut hy) = (click_x, click_y);
                if let Some((scale, center_x, center_y)) = active_zoom_at(
                    &request.render_state.zoom_regions,
                    t_track_secs,
                    request.trim_start,
                ) {
                    let scx = center_x.clamp(0.0, 1.0) * request.source_width as f64;
                    let scy = center_y.clamp(0.0, 1.0) * request.source_height as f64;
                    hx = (hx - scx) * scale + scx;
                    hy = (hy - scy) * scale + scy;
                }
                if hx >= 0.0
                    && hx <= request.source_width as f64
                    && hy >= 0.0
                    && hy <= request.source_height as f64
                {
                    let hl_canvas_x = (request.padding as f64 + hx) * scale_canvas;
                    let hl_canvas_y = (request.padding as f64 + hy) * scale_canvas;
                    // Ring radius pulses with the press scale to match the
                    // preview's `u_cursorRadius` (which includes press.scale).
                    let hr_radius = cursor_radius_canvas * 6.0 * press.scale;
                    draw_filled_circle_soft(
                        &mut frame,
                        canvas_w,
                        canvas_h,
                        hl_canvas_x,
                        hl_canvas_y,
                        hr_radius,
                        hr,
                        hg,
                        hb,
                        highlight_alpha_base * hl_env,
                    );
                }
            }
        }

        if cursor_sprite_active {
            // SVG sprite path — composite the rasterized cursor at the
            // sample position. The preroll-aware `pressed_sprite` flag
            // swaps to the alt sprite ~320 ms before the click so the
            // pointer-hand telegraphs the impending press; the actual
            // click halo (below) still keys on the literal sample state
            // so the ring fires on the audio-sync frame.
            let pressed = press.pressed_sprite;
            // Preferred slot for this frame, falling back to whatever art the
            // style actually shipped: drag → press → rest, rightPress → press
            // → rest. The rest sprite is guaranteed present here.
            let rs = &request.render_state;
            let (key, slot_hotspot) = if pressed {
                if press.dragged && image_cache.contains_key(CURSOR_SPRITE_KEY_DRAG) {
                    (CURSOR_SPRITE_KEY_DRAG, rs.cursor_sprite_hotspot_drag)
                } else if press.right && image_cache.contains_key(CURSOR_SPRITE_KEY_RIGHT_PRESS) {
                    (
                        CURSOR_SPRITE_KEY_RIGHT_PRESS,
                        rs.cursor_sprite_hotspot_right_press,
                    )
                } else if image_cache.contains_key(CURSOR_SPRITE_KEY_PRESS) {
                    (CURSOR_SPRITE_KEY_PRESS, rs.cursor_sprite_hotspot_press)
                } else {
                    (CURSOR_SPRITE_KEY_REST, rs.cursor_sprite_hotspot_rest)
                }
            } else {
                (CURSOR_SPRITE_KEY_REST, rs.cursor_sprite_hotspot_rest)
            };
            if let Some(img) = image_cache.get(key) {
                let hotspot = slot_hotspot
                    .or(rs.cursor_sprite_hotspot_rest)
                    .unwrap_or([0.5, 0.5]);
                // Sprite size: source-pixel design size from JS, mapped to
                // canvas pixels with the same `scale_canvas` factor used
                // for the cursor position above. Bounce scale modulates
                // it per-frame so click impacts visually pop.
                let sprite_source_px = request
                    .render_state
                    .cursor_sprite_size_px
                    .unwrap_or(request.render_state.cursor_size * 16.0);
                let target_size_px = sprite_source_px * scale_canvas * bounce_scale;
                // Motion-blur trail (drawn before the sharp head so the
                // current position remains crisp on top).
                for &(tx, ty, talpha) in &motion_trail {
                    blit_cursor_sprite(
                        &mut frame,
                        canvas_w,
                        canvas_h,
                        img,
                        tx,
                        ty,
                        target_size_px,
                        hotspot,
                        idle_alpha * talpha,
                    );
                }
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
            // Soft-dot path (white, 90% alpha) — bounce scales the radius,
            // motion-blur draws faint copies behind the head.
            let bounced_radius = cursor_radius_canvas * bounce_scale;
            for &(tx, ty, talpha) in &motion_trail {
                draw_filled_circle_soft(
                    &mut frame,
                    canvas_w,
                    canvas_h,
                    tx,
                    ty,
                    bounced_radius,
                    255,
                    255,
                    255,
                    0.9 * idle_alpha * talpha,
                );
            }
            draw_filled_circle_soft(
                &mut frame,
                canvas_w,
                canvas_h,
                cursor_canvas_x,
                cursor_canvas_y,
                bounced_radius,
                255,
                255,
                255,
                0.9 * idle_alpha,
            );
        }

        frame
    };

    // Drive the render in parallel, writing each chunk to FFmpeg's stdin in
    // order. Frames within a chunk are produced concurrently across cores; the
    // chunk is bounded so peak memory stays modest even at 4K (a 4K RGBA frame
    // is ~33 MB, so an unbounded fan-out would balloon RAM). This turns the
    // formerly single-threaded pre-render — the dominant cost of a cursor
    // export — into an N-core pass.
    let threads = rayon::current_num_threads().max(1);
    const MAX_INFLIGHT_BYTES: usize = 256 * 1024 * 1024;
    let max_inflight = (MAX_INFLIGHT_BYTES / bytes_per_frame.max(1)).clamp(1, 512);
    let chunk = threads.clamp(1, max_inflight);

    let mut next = 0u64;
    while next < frame_count {
        let end = (next + chunk as u64).min(frame_count);
        // Render this window of frames concurrently, preserving order in the
        // collected Vec so the sequential write below stays in frame order.
        let frames: Vec<Vec<u8>> = (next..end).into_par_iter().map(&render_one).collect();
        for f in &frames {
            stdin
                .write_all(f)
                .context("failed to write cursor frame to ffmpeg stdin")?;
        }
        next = end;
    }

    // Close stdin so FFmpeg can finalize the overlay.
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
    // Interpolated button state, kept to mirror the JS `interpolateCursor`
    // shape. The click halo now keys off the raw press events
    // (`click_highlight_at`) rather than the per-sample button state, so these
    // are currently unread on the Rust side.
    #[allow(dead_code)]
    left_down: bool,
    #[allow(dead_code)]
    right_down: bool,
}

fn interpolate_cursor(samples: &[SmoothedSample], timestamp_us: u64) -> Option<InterpolatedCursor> {
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
            x: last.x,
            y: last.y,
            visible: last.visible,
            left_down: last.left_down,
            right_down: last.right_down,
        });
    }

    if idx == 0 || samples[idx].timestamp_us == timestamp_us {
        let s = &samples[idx];
        return Some(InterpolatedCursor {
            x: s.x,
            y: s.y,
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
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
        visible: pick.visible,
        left_down: pick.left_down,
        right_down: pick.right_down,
    })
}

//  Zoom lookup (mirror of nested_region_expr in graph.rs)

/// Returns `(scale, center_x, center_y)` for the zoom active at timeline time
/// `t_secs`, or `None` when no zoom applies.
///
/// CRITICAL: the scale is sampled from the SAME 20 Hz piecewise-linear LUT the
/// FFmpeg video filter uses (`build_zoom_filter` / `sample_region`), NOT the
/// exact bezier (`scale_at`). The exported video can only approximate the
/// easing curve as a linear LUT — so if the cursor used the exact curve while
/// the video used the LUT, the two would disagree on the zoom factor *during
/// the ramps*, and that disagreement is multiplied by the focus distance and
/// the zoom scale, making the cursor visibly drift off the content as the zoom
/// animates in/out. Reproducing the LUT here keeps the cursor locked to the
/// video frame-for-frame. (The focus centre is constant, so it factors out of
/// the LUT interpolation and the affine transform at the call sites stays
/// exact — see the alignment proof in `graph::sample_region`.) `time_offset`
/// is the export trim-start, matching `sample_region`'s clamp.
fn active_zoom_at(
    regions: &[crate::render::node_types::ZoomRegion],
    t_secs: f64,
    time_offset: f64,
) -> Option<(f64, f64, f64)> {
    for region in regions {
        // Hidden regions are muted in the video filter too — keep the cursor's
        // affine in lockstep so it isn't transformed by a zoom that never renders.
        if region.hidden || t_secs < region.start || t_secs > region.end {
            continue;
        }
        // Rebuild the exact sample grid `sample_region` emits, then linearly
        // interpolate scale across the bracketing samples — i.e. evaluate the
        // video's LUT at `t_secs`.
        let effective_start = region.start.max(time_offset);
        let duration = (region.end - effective_start).max(0.0);
        let n = ((duration * 20.0).ceil() as usize).clamp(8, 200);
        let step = if n > 0 { duration / n as f64 } else { 0.0 };
        let scale = if step > 0.0 {
            let rel = ((t_secs - effective_start) / step).clamp(0.0, n as f64);
            let i0 = rel.floor() as usize;
            let i1 = (i0 + 1).min(n);
            let frac = rel - i0 as f64;
            let t0 = effective_start + step * i0 as f64;
            let t1 = effective_start + step * i1 as f64;
            let s0 = region.scale_at(t0).max(1.0);
            let s1 = region.scale_at(t1).max(1.0);
            s0 + (s1 - s0) * frac
        } else {
            region.scale_at(t_secs).max(1.0)
        };
        if scale > 1.0001 {
            return Some((scale, region.center_x, region.center_y));
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
                frame, width, height, annotation, request, t_secs, opacity, *x1, *y1, *x2, *y2,
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
            radius,
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
                    *radius,
                    annotation.glow.as_ref(),
                    Some(&annotation.stroke),
                    annotation.anchor,
                );
            }
        }
        AnnotationKind::Blur { .. } => {
            // Blur regions are handled by the main video filter chain
            // (`build_annotation_blur_complex`) — the alpha overlay carries
            // no underlying pixels to blur, so this is a deliberate no-op.
        }
        AnnotationKind::Text { .. } => {
            // Text reaches export pre-rasterized as an `Image` (rasterize-text);
            // the raw Text variant only exists to round-trip save/load, so skip.
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

    let anchor = annotation.anchor;
    // Stroke width scales with the frame (ref_w); corner radius scales with the
    // box (below), so only the width reference is needed here.
    let (ref_w, _) = anchor_ref_dims(request, anchor);
    let (x1, y1) = uv_to_canvas(request, x, y, t_secs, anchor);
    let (x2, y2) = uv_to_canvas(request, x + w, y + h, t_secs, anchor);
    let x = x1.min(x2);
    let y = y1.min(y2);
    let w = (x1 - x2).abs();
    let h = (y1 - y2).abs();
    if w <= 0.5 || h <= 0.5 {
        return;
    }

    // Glow / soft shadow behind the shape, mirroring the preview's Glow.
    if let Some(g) = annotation.glow.as_ref() {
        let is_ellipse = matches!(annotation.kind, AnnotationKind::Ellipse { .. });
        // Radius = fraction (0..0.5) of the box's shorter side (matches preview).
        let radius_px = radius * w.min(h);
        draw_shape_shadow(
            frame, width, height, request, t_secs, x, y, w, h, radius_px, is_ellipse, opacity, g,
            anchor,
        );
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
                    radius * w.min(h),
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
                let stroke_px = (annotation.stroke.width * ref_w).max(1.0);
                match annotation.kind {
                    AnnotationKind::Rect { .. } => draw_rect(
                        frame,
                        width,
                        height,
                        x,
                        y,
                        w,
                        h,
                        radius * w.min(h),
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
    let anchor = annotation.anchor;
    let (ref_w, _) = anchor_ref_dims(request, anchor);
    let stroke_px = (annotation.stroke.width * ref_w).max(1.0);

    let (cx1, cy1) = uv_to_canvas(request, x1_uv, y1_uv, t_secs, anchor);
    let (cx2, cy2) = uv_to_canvas(request, x2_uv, y2_uv, t_secs, anchor);
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
        frame,
        width,
        height,
        tip_x,
        tip_y,
        base_left_x,
        base_left_y,
        base_right_x,
        base_right_y,
        sr,
        sg,
        sb,
        alpha,
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
    radius_uv: f64,
    glow: Option<&AnnotationGlow>,
    stroke: Option<&AnnotationStroke>,
    anchor: AnnotationAnchor,
) {
    if w_uv <= 0.0 || h_uv <= 0.0 || alpha <= 0.0 {
        return;
    }
    let (cx1, cy1) = uv_to_canvas(request, x_uv, y_uv, t_secs, anchor);
    let (cx2, cy2) = uv_to_canvas(request, x_uv + w_uv, y_uv + h_uv, t_secs, anchor);
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
    // Corner radius in dst pixels, mirroring the preview (`radius` × shorter
    // side of the box). Clamped so it can't exceed half the box.
    let corner = (radius_uv.max(0.0) * dw.min(dh)).min(dw.min(dh) / 2.0);

    // Soft shadow / glow behind the image, mirroring the preview's Glow.
    if let Some(g) = glow {
        draw_image_shadow(
            frame, width, height, img, request, t_secs, dx, dy, dw, dh, corner, alpha, g, anchor,
        );
    }

    let x_min = dx.floor().max(0.0) as usize;
    let y_min = dy.floor().max(0.0) as usize;
    let x_max = (dx + dw).ceil().min(width as f64 - 1.0).max(0.0) as usize;
    let y_max = (dy + dh).ceil().min(height as f64 - 1.0).max(0.0) as usize;
    for py in y_min..=y_max {
        let v = ((py as f64 + 0.5 - dy) / dh).clamp(0.0, 1.0);
        for px in x_min..=x_max {
            let u = ((px as f64 + 0.5 - dx) / dw).clamp(0.0, 1.0);
            // Bilinear sample so scaled images aren't blocky (the preview smooths).
            let s = sample_bilinear(img, u, v);
            // Rounded-corner coverage: distance from the pixel to the inner
            // rect (box shrunk by `corner`); 1px anti-aliased falloff.
            let cover = if corner > 0.5 {
                let lx = px as f64 + 0.5 - dx;
                let ly = py as f64 + 0.5 - dy;
                let nx = lx.clamp(corner, dw - corner);
                let ny = ly.clamp(corner, dh - corner);
                let dist = ((lx - nx).powi(2) + (ly - ny).powi(2)).sqrt();
                (corner - dist + 0.5).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let src_a = s[3] / 255.0 * alpha * cover;
            if src_a <= 0.0 {
                continue;
            }
            blend_pixel(
                frame,
                width,
                px,
                py,
                s[0].round() as u8,
                s[1].round() as u8,
                s[2].round() as u8,
                src_a,
            );
        }
    }

    // Border: a rounded-rect stroke over the image, mirroring the preview.
    if let Some(stroke) = stroke {
        if stroke.width > 0.0 {
            if let Some((sr, sg, sb, sa)) = parse_css_color(&stroke.color) {
                if sa > 0.0 {
                    let stroke_px = (stroke.width * anchor_ref_dims(request, anchor).0).max(1.0);
                    draw_rect(
                        frame,
                        width,
                        height,
                        dx,
                        dy,
                        dw,
                        dh,
                        corner,
                        sr,
                        sg,
                        sb,
                        sa * alpha,
                        false,
                        stroke_px,
                    );
                }
            }
        }
    }
}

/// Soft symmetric shadow behind an image annotation — the export equivalent of
/// the preview's `Glow`. Rasterizes the image's alpha silhouette (including the
/// rounded-corner mask), box-blurs it, and blends the tinted result behind the
/// image. `glow.blur` is in UV, matched to the video width like the preview.
#[allow(clippy::too_many_arguments)]
fn draw_image_shadow(
    frame: &mut [u8],
    width: usize,
    height: usize,
    img: &RgbaImage,
    request: &CursorOverlayRequest,
    t_secs: f64,
    dx: f64,
    dy: f64,
    dw: f64,
    dh: f64,
    corner: f64,
    alpha: f64,
    glow: &AnnotationGlow,
    anchor: AnnotationAnchor,
) {
    if glow.opacity <= 0.0 || glow.blur <= 0.0 {
        return;
    }
    let Some((gr, gg, gb, ga)) = parse_css_color(&glow.color) else {
        return;
    };
    // Blur radius in dst pixels, relative to the anchor rect's width so it
    // matches the preview's `glow.blur × rect.width`.
    let (fx0, _) = uv_to_canvas(request, 0.0, 0.0, t_secs, anchor);
    let (fx1, _) = uv_to_canvas(request, 1.0, 0.0, t_secs, anchor);
    let radius = (glow.blur * (fx1 - fx0).abs()).clamp(0.0, 80.0);
    if radius < 0.5 {
        return;
    }
    let radius_i = radius.round() as usize;

    // Expanded region covering the image rect plus a blur margin.
    let margin = radius_i as isize + 1;
    let rx0 = (dx.floor() as isize - margin).max(0) as usize;
    let ry0 = (dy.floor() as isize - margin).max(0) as usize;
    let rx1 = ((dx + dw).ceil() as isize + margin).min(width as isize - 1);
    let ry1 = ((dy + dh).ceil() as isize + margin).min(height as isize - 1);
    if rx1 < rx0 as isize || ry1 < ry0 as isize {
        return;
    }
    let (rx1, ry1) = (rx1 as usize, ry1 as usize);
    let rw = rx1 - rx0 + 1;
    let rh = ry1 - ry0 + 1;
    let (img_w, img_h) = img.dimensions();

    // Silhouette = image alpha × rounded-corner coverage, in [0,1].
    let mut sil = vec![0f32; rw * rh];
    for gy in ry0..=ry1 {
        let ly = gy as f64 + 0.5 - dy;
        if ly < 0.0 || ly > dh {
            continue;
        }
        let sy = ((ly / dh).clamp(0.0, 0.999) * img_h as f64) as u32;
        for gx in rx0..=rx1 {
            let lx = gx as f64 + 0.5 - dx;
            if lx < 0.0 || lx > dw {
                continue;
            }
            let sx = ((lx / dw).clamp(0.0, 0.999) * img_w as f64) as u32;
            let a = img.get_pixel(sx, sy)[3] as f32 / 255.0;
            if a <= 0.0 {
                continue;
            }
            let cover = if corner > 0.5 {
                let nx = lx.clamp(corner, dw - corner);
                let ny = ly.clamp(corner, dh - corner);
                let dist = ((lx - nx).powi(2) + (ly - ny).powi(2)).sqrt();
                (corner - dist + 0.5).clamp(0.0, 1.0) as f32
            } else {
                1.0
            };
            sil[(gy - ry0) * rw + (gx - rx0)] = a * cover;
        }
    }

    box_blur_alpha(&mut sil, rw, rh, radius_i);

    let scale = (alpha * glow.opacity * ga).clamp(0.0, 1.0);
    if scale <= 0.0 {
        return;
    }
    for gy in ry0..=ry1 {
        for gx in rx0..=rx1 {
            let s = sil[(gy - ry0) * rw + (gx - rx0)] as f64;
            if s <= 0.0 {
                continue;
            }
            blend_pixel(frame, width, gx, gy, gr, gg, gb, s * scale);
        }
    }
}

/// Soft glow / shadow behind a rect or ellipse annotation — the export
/// equivalent of the preview's Glow for shapes. Rasterizes the shape's fill
/// silhouette, box-blurs it, and blends the tinted result behind the shape.
/// `x,y,w,h,radius_px` are canvas pixels; `is_ellipse` selects the coverage SDF.
#[allow(clippy::too_many_arguments)]
fn draw_shape_shadow(
    frame: &mut [u8],
    width: usize,
    height: usize,
    request: &CursorOverlayRequest,
    t_secs: f64,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    radius_px: f64,
    is_ellipse: bool,
    alpha: f64,
    glow: &AnnotationGlow,
    anchor: AnnotationAnchor,
) {
    if glow.opacity <= 0.0 || glow.blur <= 0.0 {
        return;
    }
    let Some((gr, gg, gb, ga)) = parse_css_color(&glow.color) else {
        return;
    };
    let (fx0, _) = uv_to_canvas(request, 0.0, 0.0, t_secs, anchor);
    let (fx1, _) = uv_to_canvas(request, 1.0, 0.0, t_secs, anchor);
    let radius = (glow.blur * (fx1 - fx0).abs()).clamp(0.0, 80.0);
    if radius < 0.5 {
        return;
    }
    let radius_i = radius.round() as usize;

    let margin = radius_i as isize + 1;
    let rx0 = (x.floor() as isize - margin).max(0) as usize;
    let ry0 = (y.floor() as isize - margin).max(0) as usize;
    let rx1 = ((x + w).ceil() as isize + margin).min(width as isize - 1);
    let ry1 = ((y + h).ceil() as isize + margin).min(height as isize - 1);
    if rx1 < rx0 as isize || ry1 < ry0 as isize {
        return;
    }
    let (rx1, ry1) = (rx1 as usize, ry1 as usize);
    let rw = rx1 - rx0 + 1;
    let rh = ry1 - ry0 + 1;

    let cx = x + w * 0.5;
    let cy = y + h * 0.5;
    let hx = (w * 0.5).max(0.5);
    let hy = (h * 0.5).max(0.5);
    let rr = radius_px.min(hx.min(hy)).max(0.0);
    let edge = 1.0 / hx.min(hy);

    // Silhouette = the shape's fill coverage, matching draw_rect/draw_ellipse.
    let mut sil = vec![0f32; rw * rh];
    for gy in ry0..=ry1 {
        let ly = gy as f64 + 0.5 - cy;
        for gx in rx0..=rx1 {
            let lx = gx as f64 + 0.5 - cx;
            let cover = if is_ellipse {
                let dist = ((lx / hx).powi(2) + (ly / hy).powi(2)).sqrt();
                (1.0 - smoothstep(1.0 - edge, 1.0, dist)).clamp(0.0, 1.0)
            } else {
                let sd = rounded_rect_sdf(lx, ly, hx, hy, rr);
                (1.0 - smoothstep(-1.0, 0.0, sd)).clamp(0.0, 1.0)
            };
            sil[(gy - ry0) * rw + (gx - rx0)] = cover as f32;
        }
    }

    box_blur_alpha(&mut sil, rw, rh, radius_i);

    let scale = (alpha * glow.opacity * ga).clamp(0.0, 1.0);
    if scale <= 0.0 {
        return;
    }
    for gy in ry0..=ry1 {
        for gx in rx0..=rx1 {
            let s = sil[(gy - ry0) * rw + (gx - rx0)] as f64;
            if s <= 0.0 {
                continue;
            }
            blend_pixel(frame, width, gx, gy, gr, gg, gb, s * scale);
        }
    }
}

/// Separable running-sum box blur over an alpha buffer, run twice to approximate
/// a Gaussian. Edges clamp; the buffer is zero-padded by construction.
fn box_blur_alpha(buf: &mut [f32], w: usize, h: usize, radius: usize) {
    if radius == 0 || w == 0 || h == 0 {
        return;
    }
    let win = (2 * radius + 1) as f32;
    let mut tmp = vec![0f32; buf.len()];
    for _ in 0..2 {
        // Horizontal: buf -> tmp.
        for y in 0..h {
            let base = y * w;
            let mut sum = 0f32;
            for k in -(radius as isize)..=(radius as isize) {
                sum += buf[base + k.clamp(0, w as isize - 1) as usize];
            }
            tmp[base] = sum / win;
            for x in 1..w {
                let add_i = (x + radius).min(w - 1);
                let sub_i = (x as isize - 1 - radius as isize).max(0) as usize;
                sum += buf[base + add_i] - buf[base + sub_i];
                tmp[base + x] = sum / win;
            }
        }
        // Vertical: tmp -> buf.
        for x in 0..w {
            let mut sum = 0f32;
            for k in -(radius as isize)..=(radius as isize) {
                sum += tmp[k.clamp(0, h as isize - 1) as usize * w + x];
            }
            buf[x] = sum / win;
            for y in 1..h {
                let add_i = (y + radius).min(h - 1) * w + x;
                let sub_i = (y as isize - 1 - radius as isize).max(0) as usize * w + x;
                sum += tmp[add_i] - tmp[sub_i];
                buf[y * w + x] = sum / win;
            }
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
        Ok(img) => {
            // Cap the longest side so a huge source (e.g. an 8000×8000 photo,
            // ~256 MB as RGBA) can't OOM the export — it's only ever composited
            // at the annotation's box size anyway.
            const MAX_DIM: u32 = 4096;
            let (w, h) = (img.width(), img.height());
            let img = if w > MAX_DIM || h > MAX_DIM {
                let scale = MAX_DIM as f32 / w.max(h) as f32;
                let nw = ((w as f32 * scale).round() as u32).max(1);
                let nh = ((h as f32 * scale).round() as u32).max(1);
                log::warn!(
                    "annotation image {w}x{h} exceeds {MAX_DIM}px, downscaling to {nw}x{nh}"
                );
                img.resize(nw, nh, image::imageops::FilterType::Triangle)
            } else {
                img
            };
            Some(img.to_rgba8())
        }
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

/// Bilinear RGBA sample at UV `(u, v)` in `[0, 1]`, returning channels as
/// 0..255 floats. Smooth-scales an image annotation instead of the blocky
/// nearest-neighbour the canvas preview never shows.
fn sample_bilinear(img: &RgbaImage, u: f64, v: f64) -> [f64; 4] {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return [0.0; 4];
    }
    let fx = (u * w as f64 - 0.5).max(0.0);
    let fy = (v * h as f64 - 0.5).max(0.0);
    let x0 = fx.floor() as u32;
    let y0 = fy.floor() as u32;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let tx = fx - x0 as f64;
    let ty = fy - y0 as f64;
    let p00 = img.get_pixel(x0.min(w - 1), y0).0;
    let p10 = img.get_pixel(x1, y0).0;
    let p01 = img.get_pixel(x0.min(w - 1), y1).0;
    let p11 = img.get_pixel(x1, y1).0;
    let mut out = [0.0; 4];
    for (c, o) in out.iter_mut().enumerate() {
        let top = p00[c] as f64 * (1.0 - tx) + p10[c] as f64 * tx;
        let bot = p01[c] as f64 * (1.0 - tx) + p11[c] as f64 * tx;
        *o = top * (1.0 - ty) + bot * ty;
    }
    out
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
    // Edge-function rasterizer. The raw edge function is twice the signed area
    // of (p, a, b); dividing by the edge length gives the perpendicular pixel
    // distance to that edge. Taking the min distance across the three edges and
    // running a 1px smoothstep gives an anti-aliased edge instead of a hard
    // stair-step (matching the AA'd capsule shaft the head attaches to).
    let sign = |px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64| -> f64 {
        (px - bx) * (ay - by) - (ax - bx) * (py - by)
    };
    let len = |ax: f64, ay: f64, bx: f64, by: f64| {
        ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt().max(1e-6)
    };
    let l1 = len(ax, ay, bx, by);
    let l2 = len(bx, by, cx, cy);
    let l3 = len(cx, cy, ax, ay);
    // Orient so "inside" is positive regardless of winding.
    let orient = if sign(cx, cy, ax, ay, bx, by) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    for py in y_min..=y_max {
        for px in x_min..=x_max {
            let pcx = px as f64 + 0.5;
            let pcy = py as f64 + 0.5;
            let e1 = orient * sign(pcx, pcy, ax, ay, bx, by) / l1;
            let e2 = orient * sign(pcx, pcy, bx, by, cx, cy) / l2;
            let e3 = orient * sign(pcx, pcy, cx, cy, ax, ay) / l3;
            let m = e1.min(e2).min(e3);
            let coverage = smoothstep(-0.5, 0.5, m);
            if coverage <= 0.0 {
                continue;
            }
            blend_pixel(buf, width, px, py, r, g, b, alpha * coverage);
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

fn uv_to_canvas(
    request: &CursorOverlayRequest,
    x: f64,
    y: f64,
    t_secs: f64,
    anchor: AnnotationAnchor,
) -> (f64, f64) {
    // Frame-anchored annotations span the whole comp buffer (canvas_* = comp
    // dims) and ignore zoom — the export mirror of the preview's frame anchor.
    if anchor == AnnotationAnchor::Frame {
        return (
            x * request.canvas_width as f64,
            y * request.canvas_height as f64,
        );
    }
    let mut uv_x = x;
    let mut uv_y = y;
    if let Some((scale, center_x, center_y)) = active_zoom_at(
        &request.render_state.zoom_regions,
        t_secs,
        request.trim_start,
    ) {
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

/// Reference dimensions a stroke width / corner radius scales against, matching
/// the preview: the video for a video anchor, the padded frame for a frame anchor.
fn anchor_ref_dims(request: &CursorOverlayRequest, anchor: AnnotationAnchor) -> (f64, f64) {
    match anchor {
        AnnotationAnchor::Frame => (request.canvas_width as f64, request.canvas_height as f64),
        AnnotationAnchor::Video => (request.source_width as f64, request.source_height as f64),
    }
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
            let dx = px as f64 + 0.5 - cx;
            let dy = py as f64 + 0.5 - cy;
            let dist = ((dx / rx).powi(2) + (dy / ry).powi(2)).sqrt();
            // Convert the normalized distance to a pixel distance via the field
            // gradient, so the edge AA and the stroke are a uniform pixel width
            // even for an eccentric (flat/tall) ellipse.
            let grad =
                ((dx / (rx * rx)).powi(2) + (dy / (ry * ry)).powi(2)).sqrt() / dist.max(1e-6);
            let px_signed = (dist - 1.0) / grad.max(1e-6);
            let coverage = if fill {
                (1.0 - smoothstep(-1.0, 0.0, px_signed)).clamp(0.0, 1.0)
            } else {
                (1.0 - smoothstep(stroke - 1.0, stroke, px_signed.abs())).clamp(0.0, 1.0)
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

// Hot per-pixel blend: discrete channel args avoid packing/unpacking a struct
// in the inner loop.
#[allow(clippy::too_many_arguments)]
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

#[cfg(test)]
mod anchor_tests {
    use super::{anchor_ref_dims, uv_to_canvas, CursorOverlayRequest};
    use crate::render::graph::RenderState;
    use crate::render::node_types::{AnnotationAnchor, ZoomRegion};

    // source 200x100, padding 10 → comp (canvas_*) 220x120. scale_canvas = 1.
    fn req(zoom: Vec<ZoomRegion>) -> CursorOverlayRequest {
        let render_state = RenderState {
            zoom_regions: zoom,
            ..RenderState::default()
        };
        CursorOverlayRequest {
            cursor_track_path: std::path::PathBuf::from("none"),
            canvas_width: 220,
            canvas_height: 120,
            source_width: 200,
            source_height: 100,
            padding: 10,
            fps: 30,
            duration_secs: 4.0,
            trim_start: 0.0,
            render_state,
        }
    }

    fn active_zoom() -> ZoomRegion {
        serde_json::from_value(serde_json::json!({
            "start": 0.0, "end": 4.0, "scale": 2.0, "centerX": 0.5, "centerY": 0.5
        }))
        .unwrap()
    }

    #[test]
    fn frame_anchor_spans_comp_without_padding_offset_or_zoom() {
        let r = req(vec![active_zoom()]);
        // 0..1 maps across the whole comp buffer, even with a 2x zoom active.
        assert_eq!(
            uv_to_canvas(&r, 0.0, 0.0, 2.0, AnnotationAnchor::Frame),
            (0.0, 0.0)
        );
        assert_eq!(
            uv_to_canvas(&r, 1.0, 1.0, 2.0, AnnotationAnchor::Frame),
            (220.0, 120.0)
        );
        // Ignores the zoom: 0.75 stays at 0.75 * comp width regardless of `t`.
        let (fx, _) = uv_to_canvas(&r, 0.75, 0.5, 2.0, AnnotationAnchor::Frame);
        assert!((fx - 0.75 * 220.0).abs() < 1e-9);
    }

    #[test]
    fn video_anchor_offsets_by_padding_and_follows_zoom() {
        // No zoom: 0..1 spans the video region [padding, padding+source].
        let r0 = req(vec![]);
        assert_eq!(
            uv_to_canvas(&r0, 0.0, 0.0, 0.0, AnnotationAnchor::Video),
            (10.0, 10.0)
        );
        let (vx0, _) = uv_to_canvas(&r0, 0.75, 0.5, 0.0, AnnotationAnchor::Video);
        assert!((vx0 - (0.75 * 200.0 + 10.0)).abs() < 1e-9); // 160

        // With a 2x zoom about centre, the same UV is pushed outward — proving
        // video anchor tracks zoom while frame anchor (above) does not.
        let r = req(vec![active_zoom()]);
        let (vx, _) = uv_to_canvas(&r, 0.75, 0.5, 2.0, AnnotationAnchor::Video);
        assert!(vx > 200.0, "expected zoom to push x past 200, got {vx}");
    }

    #[test]
    fn anchor_ref_dims_pick_frame_vs_source() {
        let r = req(vec![]);
        assert_eq!(anchor_ref_dims(&r, AnnotationAnchor::Frame), (220.0, 120.0));
        assert_eq!(anchor_ref_dims(&r, AnnotationAnchor::Video), (200.0, 100.0));
    }
}
