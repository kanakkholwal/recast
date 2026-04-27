use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::node_types::{
    Annotation, AudioSettings, BackgroundNode, CursorNode, RenderNode, ShadowSettings, TrimNode,
    WatermarkSettings, ZoomNode, ZoomRegion,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderState {
    pub trim_start: f64,
    pub trim_end: f64,
    pub background_type: String,
    pub background_value: String,
    pub background_blur: f64,
    /// Frame padding as percent of the shorter source edge (0..20).
    pub padding: f64,
    /// Corner rounding as a percentage (0..50) of the shorter video edge.
    #[serde(default)]
    pub border_radius: f64,
    pub cursor_enabled: bool,
    pub cursor_size: f64,
    pub cursor_smoothing: f64,
    pub cursor_highlight_clicks: bool,
    pub cursor_highlight_color: String,
    pub cursor_highlight_opacity: f64,
    pub cursor_hide_when_idle: bool,
    pub cursor_idle_timeout: f64,
    pub zoom_regions: Vec<ZoomRegion>,
    /// Annotation overlays (rect/ellipse for Phase 1, more to follow).
    /// Preview-only today; export integration lands with the cursor-overlay rewrite.
    #[serde(default)]
    pub annotations: Vec<Annotation>,
    /// Drop shadow cast by the video rect.
    ///
    /// Rendered in both the WebGL preview and the export. On export, the
    /// shadow is rasterised once as a canvas-sized RGBA PNG by
    /// `render::mask_export::render_drop_shadow_mask` and overlaid onto the
    /// background by `build_export_plan_with` before the video composite.
    /// This bakes `blur`, `spread`, `offset_y`, `opacity`, and `color` into
    /// the static PNG — no time-varying parameters are involved, so the
    /// FFmpeg filter chain stays free of expression evaluation here.
    #[serde(default)]
    pub shadow: ShadowSettings,
    #[serde(default)]
    pub audio_settings: AudioSettings,
    #[serde(default)]
    pub watermark_settings: WatermarkSettings,
    // Hybrid-raster cursor sprite. Populated by the JS export trigger
    // when the active style is non-`dot`; the soft-dot path is unchanged
    // when these are `None`.
    #[serde(default)]
    pub cursor_sprite_rest: Option<String>,
    #[serde(default)]
    pub cursor_sprite_press: Option<String>,
    #[serde(default)]
    pub cursor_sprite_hotspot_rest: Option<[f64; 2]>,
    #[serde(default)]
    pub cursor_sprite_hotspot_press: Option<[f64; 2]>,
    #[serde(default)]
    pub cursor_sprite_size_px: Option<f64>,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            trim_start: 0.0,
            trim_end: 0.0,
            background_type: "color".into(),
            background_value: "#111111".into(),
            background_blur: 0.0,
            padding: 0.0,
            border_radius: 0.0,
            cursor_enabled: true,
            cursor_size: 3.0,
            cursor_smoothing: 50.0,
            cursor_highlight_clicks: true,
            cursor_highlight_color: "#3b82f6".into(),
            cursor_highlight_opacity: 40.0,
            cursor_hide_when_idle: false,
            cursor_idle_timeout: 3.0,
            zoom_regions: Vec::new(),
            annotations: Vec::new(),
            shadow: ShadowSettings::default(),
            audio_settings: AudioSettings::default(),
            watermark_settings: WatermarkSettings::default(),
            cursor_sprite_rest: None,
            cursor_sprite_press: None,
            cursor_sprite_hotspot_rest: None,
            cursor_sprite_hotspot_press: None,
            cursor_sprite_size_px: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SourceVideoMetadata {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct ExportPlan {
    pub extra_inputs: Vec<PathBuf>,
    pub filter_complex: Option<String>,
    pub video_map: String,
}

#[derive(Debug, Clone)]
pub struct RenderGraph {
    pub nodes: Vec<RenderNode>,
}

impl RenderGraph {
    pub fn from_state(state: &RenderState) -> Self {
        Self {
            nodes: vec![
                RenderNode::Trim(TrimNode {
                    start: state.trim_start,
                    end: state.trim_end,
                }),
                RenderNode::Background(BackgroundNode {
                    background_type: state.background_type.clone(),
                    value: state.background_value.clone(),
                    blur: state.background_blur,
                    padding: state.padding.max(0.0),
                }),
                RenderNode::Cursor(CursorNode {
                    enabled: state.cursor_enabled,
                    size: state.cursor_size,
                    smoothing: state.cursor_smoothing,
                    highlight_clicks: state.cursor_highlight_clicks,
                    highlight_color: state.cursor_highlight_color.clone(),
                    highlight_opacity: state.cursor_highlight_opacity,
                    hide_when_idle: state.cursor_hide_when_idle,
                    idle_timeout: state.cursor_idle_timeout,
                }),
                RenderNode::Zoom(ZoomNode {
                    regions: state.zoom_regions.clone(),
                }),
            ],
        }
    }

    pub fn trim_range(&self) -> (f64, f64) {
        self.nodes
            .iter()
            .find_map(|node| match node {
                RenderNode::Trim(trim) => Some((trim.start, trim.end)),
                _ => None,
            })
            .unwrap_or((0.0, 0.0))
    }

    pub fn build_export_plan_with(
        &self,
        source: SourceVideoMetadata,
        static_root: &Path,
        background_input_index: usize,
        asset_cache_dir: Option<&Path>,
        border_radius_mask: Option<PathBuf>,
        drop_shadow_mask: Option<PathBuf>,
    ) -> Result<ExportPlan> {
        let background = self.nodes.iter().find_map(|node| match node {
            RenderNode::Background(background) => Some(background),
            _ => None,
        });
        let zoom = self.nodes.iter().find_map(|node| match node {
            RenderNode::Zoom(zoom) => Some(zoom),
            _ => None,
        });

        let padding = background
            .map(|node| padding_percent_to_pixels(node.padding, source))
            .unwrap_or_default();
        let canvas_width = source.width + padding * 2;
        let canvas_height = source.height + padding * 2;
        // Zoom region times are stored in PROJECT-timeline seconds, but the
        // FFmpeg expression evaluator's `t` is OUTPUT-stream time, which is
        // reset to 0 by the input-side `-ss <trim_start>` we emit in
        // `export_video`. If we don't subtract the trim offset here, the LUT
        // fires at timeline-t inside the output stream — which, with any
        // trim, is past the output's end, so the zoom never visibly applies.
        // Without trim the offset is 0 and the behaviour is unchanged.
        let trim_start = self.trim_range().0.max(0.0);
        let zoom_filter = zoom
            .map(|node| build_zoom_filter(node, source, trim_start))
            .filter(|value: &String| !value.is_empty());

        // The mask, when present, occupies the first extra_input slot so its
        // input index is deterministic (= background_input_index). The
        // background image (if any) shifts to the next slot.
        let mut extra_inputs: Vec<PathBuf> = Vec::new();
        let mask_input_index = border_radius_mask.as_ref().map(|_| background_input_index);
        if let Some(path) = border_radius_mask {
            extra_inputs.push(path);
        }
        let bg_image_input_index = background_input_index + extra_inputs.len();
        // Drop-shadow PNG slot is reserved up front so its index is known
        // before the bg image is conditionally pushed; the actual push (if
        // any) happens below, AFTER the bg image, so existing
        // `cursor_input_index = 1 + extra_inputs.len()` math stays correct.
        // `shadow_input_index` is `None` when the caller didn't supply a
        // shadow PNG; the filter chain below treats that as "no shadow stage".
        let mut shadow_input_index: Option<usize> = None;

        // Build the chain that produces the source-video label `[video0]`.
        // When neither zoom nor mask are present, the source can be referenced
        // directly as `[0:v]` (saves a filter pass).
        //
        // For the mask paths we MUST normalize pixel formats: alphamerge
        // expects the main input to already carry an alpha plane (yuva420p)
        // and the mask input to be a single-plane gray image. Without these
        // explicit `format=` conversions FFmpeg tends to negotiate yuv420p
        // (no alpha) on the main input, at which point alphamerge silently
        // outputs a fully-transparent stream — the visual symptom is a black
        // background showing through with only the cursor overlay visible.
        let mut prelude_segments: Vec<String> = Vec::new();
        let video_label: String = match (zoom_filter.as_ref(), mask_input_index) {
            (None, None) => "[0:v]".into(),
            (Some(zoom_filter), None) => {
                prelude_segments.push(format!("[0:v]{zoom_filter}[video0]"));
                "[video0]".into()
            }
            (None, Some(mask_idx)) => {
                prelude_segments.push(format!(
                    "[0:v]format=yuva420p[video0pre];[{mask_idx}:v]format=gray[mask0];[video0pre][mask0]alphamerge[video0]"
                ));
                "[video0]".into()
            }
            (Some(zoom_filter), Some(mask_idx)) => {
                prelude_segments.push(format!(
                    "[0:v]{zoom_filter},format=yuva420p[video0pre];[{mask_idx}:v]format=gray[mask0];[video0pre][mask0]alphamerge[video0]"
                ));
                "[video0]".into()
            }
        };

        // Resolve the wallpaper/image bg path up-front (without pushing yet)
        // so we know whether a bg-image input slot will be allocated; that
        // determines the shadow-input slot index, which is then baked into
        // the filter strings before any extra_inputs are pushed.
        let resolved_bg_image = match background {
            Some(bg) if matches!(bg.background_type.as_str(), "wallpaper" | "image") => {
                resolve_background_path(&bg.value, static_root, asset_cache_dir)
            }
            _ => None,
        };
        let will_push_bg_image = resolved_bg_image.is_some();
        if drop_shadow_mask.is_some() {
            shadow_input_index = Some(
                background_input_index + extra_inputs.len() + will_push_bg_image as usize,
            );
        }

        let filter_complex = match background {
            Some(background)
                if matches!(background.background_type.as_str(), "wallpaper" | "image") =>
            {
                if resolved_bg_image.is_some() {
                    let mut segments = prelude_segments.clone();
                    let blur_sigma = (background.blur / 8.0).max(0.0);
                    segments.push(format!(
                        "[{bg_image_input_index}:v]scale={canvas_width}:{canvas_height}:force_original_aspect_ratio=increase,crop={canvas_width}:{canvas_height},boxblur={blur_sigma}[bg0]"
                    ));
                    let bg_label = compose_shadow_stage(&mut segments, shadow_input_index);
                    segments.push(format!(
                        "{bg_label}{video_label}overlay={padding}:{padding}[vout]"
                    ));
                    Some(segments.join(";"))
                } else {
                    build_color_background_filter(
                        background,
                        prelude_segments.clone(),
                        &video_label,
                        canvas_width,
                        canvas_height,
                        padding,
                        shadow_input_index,
                    )
                }
            }
            Some(background) => build_color_background_filter(
                background,
                prelude_segments.clone(),
                &video_label,
                canvas_width,
                canvas_height,
                padding,
                shadow_input_index,
            ),
            None => {
                if prelude_segments.is_empty() {
                    None
                } else {
                    // Source is `[video0]`; surface it as `[vout]` so the
                    // outer pipeline always maps a labelled stream.
                    let mut segments = prelude_segments.clone();
                    segments.push(format!("{video_label}null[vout]"));
                    Some(segments.join(";"))
                }
            }
        };

        // Now that filter strings are built (and reference the eventual
        // shadow input index), push the actual extra inputs in the
        // committed order: bg_image then drop_shadow.
        if let Some(path) = resolved_bg_image {
            extra_inputs.push(path);
        }
        if let Some(path) = drop_shadow_mask {
            extra_inputs.push(path);
        }

        let requires_map = filter_complex.is_some();

        Ok(ExportPlan {
            extra_inputs,
            filter_complex,
            video_map: if requires_map {
                "[vout]".into()
            } else {
                "0:v:0".into()
            },
        })
    }
}

fn padding_percent_to_pixels(padding_percent: f64, source: SourceVideoMetadata) -> u32 {
    let pct = padding_percent.clamp(0.0, 20.0);
    let shorter_edge = source.width.min(source.height) as f64;
    ((shorter_edge * pct) / 100.0).round() as u32
}

fn build_color_background_filter(
    background: &BackgroundNode,
    prelude_segments: Vec<String>,
    video_label: &str,
    canvas_width: u32,
    canvas_height: u32,
    padding: u32,
    shadow_input_index: Option<usize>,
) -> Option<String> {
    let color = match background.background_type.as_str() {
        "color" => normalize_color(&background.value),
        "gradient" => gradient_fallback_color(&background.value),
        _ => "#111111".into(),
    };

    let mut segments = prelude_segments;
    segments.push(format!(
        "color=c={color}:s={canvas_width}x{canvas_height}[bg0]"
    ));
    let bg_label = compose_shadow_stage(&mut segments, shadow_input_index);
    segments.push(format!(
        "{bg_label}{video_label}overlay={padding}:{padding}[vout]"
    ));
    Some(segments.join(";"))
}

/// When a drop-shadow PNG is supplied, append the two extra filter segments
/// that overlay it on top of the freshly-emitted `[bg0]` stage and produce
/// the `[bg]` label the video composite consumes. Returns the label that the
/// next stage should use as its background — `[bg]` when shadow is present,
/// `[bg0]` otherwise (the latter is a label rename, no extra filter pass).
fn compose_shadow_stage(
    segments: &mut Vec<String>,
    shadow_input_index: Option<usize>,
) -> &'static str {
    match shadow_input_index {
        Some(idx) => {
            // `format=rgba` normalises the shadow input — the PNG already
            // carries an alpha plane, but ffmpeg sometimes negotiates a
            // non-alpha pixel format on the decoder side which would make
            // the overlay opaque.
            segments.push(format!("[{idx}:v]format=rgba[shadow]"));
            segments.push("[bg0][shadow]overlay=0:0[bg]".into());
            "[bg]"
        }
        None => "[bg0]",
    }
}

fn build_zoom_filter(node: &ZoomNode, source: SourceVideoMetadata, time_offset: f64) -> String {
    if node.regions.is_empty() {
        return String::new();
    }

    // Pre-sample each region's curve. FFmpeg's expression evaluator can't
    // call our Rust bezier solver, but a dense piecewise-linear LUT at 20 Hz
    // is visually indistinguishable from the real curve.
    //
    // `time_offset` (= trim_start) shifts the LUT so its t-values are in
    // OUTPUT-stream coordinates rather than project-timeline coordinates;
    // see `build_export_plan_with` for the rationale.
    //
    // Filter shape — IMPORTANT:
    //   `scale=w='iw*Z(t)':h='ih*Z(t)':eval=frame, crop=W:H:x='X(t)':y='Y(t)'`
    //
    // We deliberately do NOT use the more obvious `crop=w='iw/Z':h='ih/Z',
    // scale=W:H` form, because **ffmpeg's `crop` filter evaluates `w` and
    // `h` only ONCE at filter init**, where `t = 0`. With the LUT default
    // returning `iw`/`ih` outside any region, that one-time evaluation
    // resolves to the source dimensions and the crop is a fixed identity for
    // the whole export — zoom never visibly applies. `scale=eval=frame`
    // re-evaluates per frame, and `crop` with literal `w/h` (the constant
    // source dimensions) doesn't hit the init-only limitation; its `x` and
    // `y` are evaluated per frame regardless. This was the actual root cause
    // of "zoom is missing in exported videos" — verified by pixel-diffing
    // FFmpeg outputs of both filter shapes against an identity baseline.
    let samples_per_region: Vec<Vec<ZoomSample>> = node
        .regions
        .iter()
        // Skip regions whose entire timeline window precedes `trim_start` —
        // their LUT entries would all have negative output-t and never fire.
        .filter(|region| region.end > time_offset)
        .map(|region| sample_region(region, source, time_offset))
        .collect();

    // If filtering left us with nothing, skip the prelude entirely.
    if samples_per_region.iter().all(|s| s.is_empty()) {
        return String::new();
    }

    // Three time-varying expressions:
    //   z_expr — multiplicative zoom factor, default 1.0 outside regions.
    //   x_expr — crop top-left X in POST-SCALE absolute pixels.
    //   y_expr — crop top-left Y in POST-SCALE absolute pixels.
    //
    // Defaults outside any region produce a centred crop on the un-zoomed
    // source: x = (iw - W) / 2 = 0, y = (ih - H) / 2 = 0 (since iw == W and
    // ih == H when Z = 1.0). Inside `crop`'s expressions, `iw` is the input
    // (post-scale) width — so even though we name the constant default `0`,
    // it remains correct because at Z=1 the post-scale dims equal source.
    let z_expr = build_piecewise_expr(&samples_per_region, "1", |s| s.scale_factor);
    let x_expr = build_piecewise_expr(&samples_per_region, "0", |s| s.crop_x);
    let y_expr = build_piecewise_expr(&samples_per_region, "0", |s| s.crop_y);

    format!(
        "scale=w='iw*({z_expr})':h='ih*({z_expr})':eval=frame,crop={}:{}:x='{x_expr}':y='{y_expr}'",
        source.width, source.height
    )
}

#[derive(Debug, Clone, Copy)]
struct ZoomSample {
    t: f64,            // output-stream time (post-trim) at this sample
    scale_factor: f64, // multiplicative zoom factor (>= 1.0)
    crop_x: f64,       // crop top-left X in POST-SCALE absolute pixels
    crop_y: f64,       // crop top-left Y in POST-SCALE absolute pixels
}

fn sample_region(
    region: &ZoomRegion,
    source: SourceVideoMetadata,
    time_offset: f64,
) -> Vec<ZoomSample> {
    // Clamp the sampling window to the post-trim portion of the region.
    // `region.scale_at` still receives the true timeline t, so the eased
    // ramp curve is sampled correctly.
    let effective_start = region.start.max(time_offset);
    let duration = (region.end - effective_start).max(0.0);
    let samples = ((duration * 20.0).ceil() as usize).clamp(8, 200);
    let step = if samples > 0 {
        duration / samples as f64
    } else {
        0.0
    };
    let iw = source.width as f64;
    let ih = source.height as f64;
    // Output crop window matches source dimensions — we scale UP by Z(t),
    // then crop a source-sized window from the upscaled frame.
    let out_w = iw;
    let out_h = ih;
    // Focus centre eases from (0.5, 0.5) → (center_x, center_y) across the
    // ramp, so the crop drifts smoothly into the focused area rather than
    // snapping off-centre on the first frame.
    let fx_target = region.center_x.clamp(0.0, 1.0);
    let fy_target = region.center_y.clamp(0.0, 1.0);
    let mut out = Vec::with_capacity(samples + 1);
    for i in 0..=samples {
        // `timeline_t` drives `scale_at`; `output_t` is what we emit into
        // the FFmpeg LUT (t inside the filter is post-trim output time).
        let timeline_t = effective_start + step * i as f64;
        let output_t = timeline_t - time_offset;
        let scale = region.scale_at(timeline_t).max(1.0);
        // Fractional progress 1.0 → region.scale; drives the centre lerp so
        // rest frames stay centred.
        let p = if region.scale > 1.0 {
            ((scale - 1.0) / (region.scale - 1.0)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let fx = 0.5 + (fx_target - 0.5) * p;
        let fy = 0.5 + (fy_target - 0.5) * p;
        // Post-scale dimensions: iw * Z, ih * Z. Crop a (out_w × out_h)
        // window such that the focus point (fx, fy) in source UV space
        // lands at the centre of the cropped frame. Clamp so the window
        // never leaves the upscaled image.
        let iw_post = iw * scale;
        let ih_post = ih * scale;
        let crop_x = (fx * iw_post - out_w / 2.0).clamp(0.0, (iw_post - out_w).max(0.0));
        let crop_y = (fy * ih_post - out_h / 2.0).clamp(0.0, (ih_post - out_h).max(0.0));
        out.push(ZoomSample {
            t: output_t,
            scale_factor: scale,
            crop_x,
            crop_y,
        });
    }
    out
}

/// Build one FFmpeg expression that evaluates a per-sample quantity via a
/// piecewise-linear lookup over all regions, falling back to `default` when
/// `t` is outside every region. Each segment emits
/// `if(between(t,ti,tj), vi + (vj-vi)*(t-ti)/(tj-ti), ACC)`. Built as a
/// right-fold so the innermost if handles the first segment and the outer
/// ones layer the fallback.
fn build_piecewise_expr<F>(
    samples_per_region: &[Vec<ZoomSample>],
    default: &str,
    field: F,
) -> String
where
    F: Fn(&ZoomSample) -> f64,
{
    // Collect every (t_i, v_i, t_{i+1}, v_{i+1}) segment across all regions in
    // a flat list. Gaps between regions naturally fall through to `default`.
    let mut segments: Vec<(f64, f64, f64, f64)> = Vec::new();
    for samples in samples_per_region {
        for pair in samples.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            if b.t <= a.t {
                continue;
            }
            segments.push((a.t, field(a), b.t, field(b)));
        }
    }

    segments
        .into_iter()
        .rev()
        .fold(default.to_string(), |acc, (ta, va, tb, vb)| {
            // If va == vb, skip the linear-interp arithmetic — keeps strings
            // shorter during the hold phase where the scale is constant.
            let value_expr = if (va - vb).abs() < 1e-6 {
                format!("{va:.4}")
            } else {
                let dt = tb - ta;
                let dv = vb - va;
                format!("({va:.4}+{dv:.6}*(t-{ta:.4})/{dt:.4})")
            };
            format!("if(between(t,{ta:.4},{tb:.4}),{value_expr}, {acc})")
        })
}

fn resolve_background_path(
    value: &str,
    static_root: &Path,
    asset_cache_dir: Option<&Path>,
) -> Option<PathBuf> {
    if value.is_empty() {
        return None;
    }

    // External-asset scheme: `asset:<id>` resolves against the downloaded
    // manifest cache in the app data dir. Read manifest.lock.json there.
    if let Some(id) = value.strip_prefix("asset:") {
        if let Some(dir) = asset_cache_dir {
            let lock = dir.join("manifest.lock.json");
            if let Ok(bytes) = std::fs::read(&lock) {
                if let Ok(manifest) =
                    serde_json::from_slice::<crate::commands::assets::Manifest>(&bytes)
                {
                    if let Some(entry) = manifest.assets.iter().find(|a| a.id == id) {
                        let path = dir.join(&entry.filename);
                        if path.exists() {
                            return Some(path);
                        }
                    }
                }
            }
        }
        return None;
    }

    // Frontend wallpapers are served from `/backgrounds/wallpapers/...` — map
    // those back to `static/backgrounds/wallpapers/...` on disk. Also handle the
    // legacy `/wallpapers/...` prefix for any stored projects.
    if let Some(rest) = value.strip_prefix("/backgrounds/wallpapers/") {
        let resolved = static_root
            .join("backgrounds")
            .join("wallpapers")
            .join(rest);
        if resolved.exists() {
            return Some(resolved);
        }
    }
    if let Some(rest) = value.strip_prefix("/wallpapers/") {
        let resolved = static_root.join("wallpapers").join(rest);
        if resolved.exists() {
            return Some(resolved);
        }
        // Also try backgrounds/wallpapers/ as a fallback.
        let alt = static_root
            .join("backgrounds")
            .join("wallpapers")
            .join(rest);
        if alt.exists() {
            return Some(alt);
        }
    }
    // Any other `/`-rooted path is treated as relative to static_root.
    if let Some(rest) = value.strip_prefix('/') {
        let resolved = static_root.join(rest);
        if resolved.exists() {
            return Some(resolved);
        }
    }

    if let Some(decoded_path) = decode_background_uri(value) {
        if decoded_path.exists() {
            return Some(decoded_path);
        }
    }

    let as_path = PathBuf::from(value);
    if as_path.exists() {
        Some(as_path)
    } else {
        None
    }
}

fn decode_background_uri(value: &str) -> Option<PathBuf> {
    const PREFIXES: [&str; 4] = [
        "asset://localhost/",
        "http://asset.localhost/",
        "https://asset.localhost/",
        "file:///",
    ];

    for prefix in PREFIXES {
        if let Some(rest) = value.strip_prefix(prefix) {
            let decoded = percent_decode(rest);
            let normalized = if decoded.starts_with('/') && decoded.as_bytes().get(2) == Some(&b':')
            {
                decoded[1..].to_string()
            } else {
                decoded
            };
            return Some(PathBuf::from(normalized));
        }
    }

    None
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    decoded.push(byte);
                    index += 3;
                    continue;
                }
            }
        }

        decoded.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&decoded).into_owned()
}

fn normalize_color(value: &str) -> String {
    if value.trim().is_empty() {
        "#111111".into()
    } else {
        value.trim().to_string()
    }
}

fn gradient_fallback_color(value: &str) -> String {
    value
        .split(|c: char| c == ',' || c.is_whitespace())
        .find(|token| token.starts_with('#'))
        .map(|token| token.trim_matches(')').to_string())
        .unwrap_or_else(|| "#111111".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::node_types::ZoomRegion;

    fn region(start: f64, end: f64, scale: f64) -> ZoomRegion {
        ZoomRegion {
            start,
            end,
            scale,
            ease_in: Default::default(),
            ease_out: Default::default(),
            ramp_in: 0.5,
            ramp_out: 0.5,
            center_x: 0.5,
            center_y: 0.5,
            motion_blur: 0.0,
        }
    }

    fn render_state_with_zoom(trim_start: f64, trim_end: f64, regions: Vec<ZoomRegion>) -> RenderState {
        RenderState {
            trim_start,
            trim_end,
            zoom_regions: regions,
            ..RenderState::default()
        }
    }

    fn export_plan(state: &RenderState) -> ExportPlan {
        RenderGraph::from_state(state)
            .build_export_plan_with(
                SourceVideoMetadata { width: 1920, height: 1080 },
                Path::new("."),
                1,
                None,
                None,
                None,
            )
            .expect("plan")
    }

    fn export_plan_with_shadow(state: &RenderState, shadow_path: PathBuf) -> ExportPlan {
        RenderGraph::from_state(state)
            .build_export_plan_with(
                SourceVideoMetadata { width: 1920, height: 1080 },
                Path::new("."),
                1,
                None,
                None,
                Some(shadow_path),
            )
            .expect("plan")
    }

    /// Without trim, the LUT t-values are timeline = output, and the filter
    /// must include `between(t,1.0,...)` segments because the zoom region
    /// starts at timeline 1.0.
    /// The filter MUST be a `scale=eval=frame` + fixed-size `crop` chain.
    /// The previous `crop=w='<expr>':h='<expr>'` form silently never fired
    /// because ffmpeg's `crop` evaluates `w`/`h` only ONCE at filter init,
    /// where `t = 0`; that was the actual root cause of "zoom missing in
    /// exported videos". This test asserts the new shape directly.
    #[test]
    fn zoom_filter_uses_scale_eval_frame_not_crop_wh_lut() {
        let state = render_state_with_zoom(0.0, 5.0, vec![region(1.0, 4.0, 1.5)]);
        let plan = export_plan(&state);
        let fc = plan.filter_complex.expect("filter_complex must exist when zoom present");
        // Must use scale with eval=frame so width/height re-evaluate per frame.
        assert!(
            fc.contains("scale=w='iw*(") && fc.contains(":eval=frame"),
            "zoom must scale via eval=frame: {fc}"
        );
        // Crop must have LITERAL fixed w/h (=source dims) — anything inside
        // `crop=w='<expr>'` would hit the init-only evaluation bug again.
        assert!(
            fc.contains("crop=1920:1080:"),
            "crop must use fixed source dimensions, not LUT-driven w/h: {fc}"
        );
        // LUT must reference output-stream time at the region start.
        assert!(
            fc.contains("between(t,1.0000"),
            "expected output-t LUT entry at 1.0000: {fc}"
        );
    }

    /// With trim_start = 2.0, the FFmpeg `t` is OUTPUT-stream time. A region
    /// at timeline [3.0, 5.0] must appear in the LUT at output [1.0, 3.0].
    /// Pre-fix, this assertion failed: the LUT had `between(t,3.0000,...)`
    /// which never fires because the output never reaches t=3 (the visible
    /// duration is 5 - 2 = 3 s, but scrubbing/preview seeing zoom at
    /// timeline 3 expects it at output 1).
    #[test]
    fn zoom_filter_shifts_lut_by_trim_start() {
        let state = render_state_with_zoom(2.0, 5.0, vec![region(3.0, 5.0, 1.5)]);
        let plan = export_plan(&state);
        let fc = plan.filter_complex.expect("filter_complex must exist when zoom present");
        assert!(
            fc.contains("between(t,1.0000"),
            "LUT must be shifted to output-time (start at output t=1.0): {fc}"
        );
        assert!(
            !fc.contains("between(t,3.0000,"),
            "stale timeline-t LUT entry at 3.0000 must NOT be present: {fc}"
        );
    }

    /// A zoom region whose entire timeline range precedes trim_start used
    /// to produce a LUT whose t-values were negative — harmless to FFmpeg
    /// (`between(t, -2.0, -1.0)` simply never fires) but a waste of filter
    /// string. Now we prune those regions entirely, so the planner doesn't
    /// emit a zoom prelude at all in this case. The test still verifies
    /// "doesn't panic" and that the rest of the plan is intact.
    #[test]
    fn zoom_region_entirely_before_trim_does_not_panic() {
        let state = render_state_with_zoom(5.0, 10.0, vec![region(1.0, 3.0, 1.5)]);
        let plan = export_plan(&state);
        let fc = plan.filter_complex.expect("filter_complex still emitted");
        // Color-bg branch always emits its own composite, but the zoom
        // crop should be skipped now that the only region is pre-trim.
        assert!(!fc.contains("crop=w="), "pruned zoom should leave no crop filter: {fc}");
        assert!(fc.contains("[vout]"), "rest of plan intact: {fc}");
    }

    /// Plan must always include the zoom prelude when regions exist, even
    /// with the default color background — this was the agent's mis-diagnosis
    /// originally, but verifying it locks in the contract.
    #[test]
    fn zoom_filter_present_with_default_background() {
        let state = render_state_with_zoom(0.0, 5.0, vec![region(1.0, 4.0, 1.5)]);
        let plan = export_plan(&state);
        let fc = plan.filter_complex.expect("filter_complex must exist");
        assert!(fc.contains("[video0]"), "zoom prelude must label its output [video0]: {fc}");
        assert_eq!(plan.video_map, "[vout]");
    }

    /// Auto-zoom typically produces 3-6 regions. Each must contribute
    /// segments to the LUT, and a sample at each region's start should be
    /// represented.
    #[test]
    fn multiple_zoom_regions_all_appear_in_lut() {
        let state = render_state_with_zoom(
            0.0,
            10.0,
            vec![
                region(1.0, 2.0, 1.4),
                region(3.0, 4.5, 1.6),
                region(6.0, 8.0, 1.5),
            ],
        );
        let plan = export_plan(&state);
        let fc = plan.filter_complex.expect("filter_complex must exist");
        assert!(fc.contains("between(t,1.0000"), "first region missing: {fc}");
        assert!(fc.contains("between(t,3.0000"), "second region missing: {fc}");
        assert!(fc.contains("between(t,6.0000"), "third region missing: {fc}");
    }

    /// Region partially overlapping `trim_start` (e.g. region [1, 4],
    /// trim_start = 2.0): the LUT must NOT contain segments before the
    /// trim; samples should start at the post-trim portion (output t ≥ 0).
    #[test]
    fn zoom_region_partially_before_trim_is_clamped() {
        let state = render_state_with_zoom(2.0, 6.0, vec![region(1.0, 4.0, 1.5)]);
        let plan = export_plan(&state);
        let fc = plan.filter_complex.expect("filter_complex must exist");
        // First segment should start at output t = 0 (corresponding to
        // timeline t = 2.0, the clamped effective_start).
        assert!(
            fc.contains("between(t,0.0000"),
            "clamped LUT must start at output t=0: {fc}"
        );
        // No stale pre-trim segment should appear.
        assert!(
            !fc.contains("between(t,-1.0000"),
            "negative-t segment should be pruned by clamping: {fc}"
        );
    }

    /// Region whose entire timeline range is before trim_start should not
    /// contribute ANY segments to the LUT (and previously emitted dead
    /// `between(t, negative, negative)` calls).
    #[test]
    fn fully_pre_trim_zoom_region_is_dropped() {
        let state = render_state_with_zoom(
            5.0,
            10.0,
            vec![
                region(1.0, 3.0, 1.5),  // entirely before trim
                region(6.0, 8.0, 1.5),  // post-trim, should fire
            ],
        );
        let plan = export_plan(&state);
        let fc = plan.filter_complex.expect("filter_complex must exist");
        assert!(fc.contains("between(t,1.0000"), "post-trim region present: {fc}");
        // Pre-trim region's first sample would have been at output_t = -4.0.
        assert!(
            !fc.contains("-4.0000"),
            "pre-trim region must not contribute LUT entries: {fc}"
        );
    }

    /// When ALL regions are pre-trim, the prelude should not exist at all
    /// (since `build_zoom_filter` returns empty, the `.filter(!is_empty)`
    /// drops the prelude, and with default color bg + no other prelude,
    /// the plan still has a filter_complex but no zoom in it).
    #[test]
    fn all_pre_trim_zoom_regions_yields_no_zoom_prelude() {
        let state = render_state_with_zoom(5.0, 10.0, vec![region(1.0, 3.0, 1.5)]);
        let plan = export_plan(&state);
        let fc = plan.filter_complex.expect("color bg still produces a complex");
        assert!(
            !fc.contains("crop=w="),
            "no zoom prelude expected when all regions are pre-trim: {fc}"
        );
    }

    /// Drop shadow path injects a `[N:v]format=rgba[shadow]` stage and
    /// composes it onto the bg before the video overlay. The shadow input
    /// index lands AFTER the bg-image slot (when present) — for the
    /// default color-bg case, that's index 1 (only extra input).
    #[test]
    fn drop_shadow_inserts_overlay_stage_with_color_bg() {
        let state = RenderState::default();
        let plan = export_plan_with_shadow(&state, PathBuf::from("/tmp/fake_shadow.png"));
        let fc = plan.filter_complex.expect("filter_complex must exist");
        assert!(
            fc.contains("[1:v]format=rgba[shadow]"),
            "shadow input stage missing: {fc}"
        );
        assert!(
            fc.contains("[bg0][shadow]overlay=0:0[bg]"),
            "shadow composite stage missing: {fc}"
        );
        assert!(
            fc.contains("[bg]") && fc.contains("overlay=0:0[vout]"),
            "video should still composite onto the shadowed bg: {fc}"
        );
        assert_eq!(
            plan.extra_inputs.len(),
            1,
            "shadow PNG appended to extra_inputs"
        );
    }

    /// Without shadow, the extra `[bg0]` rename should NOT cost a real
    /// filter pass — the planner just labels the color stage `[bg0]` and
    /// the video composite reads from `[bg0]` directly. Quick sanity test
    /// that no `format=rgba[shadow]` ever leaks in.
    #[test]
    fn no_shadow_means_no_shadow_overlay_stage() {
        let state = RenderState::default();
        let plan = export_plan(&state);
        let fc = plan.filter_complex.expect("filter_complex must exist");
        assert!(
            !fc.contains("[shadow]"),
            "shadow stage must not appear when no shadow PNG was supplied: {fc}"
        );
    }
}
