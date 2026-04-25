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
    /// Drop shadow cast by the video rect. Preview-only today.
    #[serde(default)]
    pub shadow: ShadowSettings,
    #[serde(default)]
    pub audio_settings: AudioSettings,
    #[serde(default)]
    pub watermark_settings: WatermarkSettings,
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
                    padding: state.padding.max(0.0).round() as u32,
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
    ) -> Result<ExportPlan> {
        let background = self.nodes.iter().find_map(|node| match node {
            RenderNode::Background(background) => Some(background),
            _ => None,
        });
        let zoom = self.nodes.iter().find_map(|node| match node {
            RenderNode::Zoom(zoom) => Some(zoom),
            _ => None,
        });

        let padding = background.map(|node| node.padding).unwrap_or_default();
        let canvas_width = source.width + padding * 2;
        let canvas_height = source.height + padding * 2;
        let zoom_filter = zoom
            .map(|node| build_zoom_filter(node, source))
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

        let filter_complex = match background {
            Some(background)
                if matches!(background.background_type.as_str(), "wallpaper" | "image") =>
            {
                if let Some(background_path) =
                    resolve_background_path(&background.value, static_root, asset_cache_dir)
                {
                    extra_inputs.push(background_path);
                    let mut segments = prelude_segments.clone();
                    let blur_sigma = (background.blur / 8.0).max(0.0);
                    segments.push(format!(
                        "[{bg_image_input_index}:v]scale={canvas_width}:{canvas_height}:force_original_aspect_ratio=increase,crop={canvas_width}:{canvas_height},boxblur={blur_sigma}[bg]"
                    ));
                    segments.push(format!(
                        "[bg]{video_label}overlay={padding}:{padding}[vout]"
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

fn build_color_background_filter(
    background: &BackgroundNode,
    prelude_segments: Vec<String>,
    video_label: &str,
    canvas_width: u32,
    canvas_height: u32,
    padding: u32,
) -> Option<String> {
    let color = match background.background_type.as_str() {
        "color" => normalize_color(&background.value),
        "gradient" => gradient_fallback_color(&background.value),
        _ => "#111111".into(),
    };

    let mut segments = prelude_segments;
    segments.push(format!(
        "color=c={color}:s={canvas_width}x{canvas_height}[bg]"
    ));
    segments.push(format!(
        "[bg]{video_label}overlay={padding}:{padding}[vout]"
    ));
    Some(segments.join(";"))
}

fn build_zoom_filter(node: &ZoomNode, source: SourceVideoMetadata) -> String {
    if node.regions.is_empty() {
        return String::new();
    }

    // Pre-sample each region's scale curve so the filter stays simple math.
    // FFmpeg's expression evaluator can't call our Rust bezier solver, but a
    // dense piecewise-linear LUT is visually indistinguishable from the real
    // curve at 20 Hz (every ~3 frames at 60 fps). Each sample records the
    // four derived quantities we feed to `crop` so we don't recompute them
    // inside the filter string.
    let samples_per_region: Vec<Vec<ZoomSample>> = node
        .regions
        .iter()
        .map(|region| sample_region(region, source))
        .collect();

    let width_expr = build_piecewise_expr(&samples_per_region, "iw", |s| s.width);
    let height_expr = build_piecewise_expr(&samples_per_region, "ih", |s| s.height);
    let x_expr = build_piecewise_expr(&samples_per_region, "0", |s| s.x);
    let y_expr = build_piecewise_expr(&samples_per_region, "0", |s| s.y);

    format!(
        "crop=w='{width_expr}':h='{height_expr}':x='{x_expr}':y='{y_expr}',scale={}:{}",
        source.width, source.height
    )
}

#[derive(Debug, Clone, Copy)]
struct ZoomSample {
    t: f64,
    width: f64,  // iw / scale
    height: f64, // ih / scale
    x: f64,      // (iw - iw/scale) / 2
    y: f64,      // (ih - ih/scale) / 2
}

fn sample_region(region: &ZoomRegion, source: SourceVideoMetadata) -> Vec<ZoomSample> {
    let duration = (region.end - region.start).max(0.0);
    // 20 Hz baseline, capped so very long regions don't explode the filter
    // string. 200 samples per region over a 10 s region is ~6 KB which
    // FFmpeg handles fine; past that the LUT is denser than any human eye
    // needs and we trade some fidelity for parser health.
    let samples = ((duration * 20.0).ceil() as usize).clamp(8, 200);
    let step = if samples > 0 {
        duration / samples as f64
    } else {
        0.0
    };
    let iw = source.width as f64;
    let ih = source.height as f64;
    // Focus centre eases from (0.5, 0.5) → (center_x, center_y) across the
    // ramp, so the crop window drifts smoothly into the focused area rather
    // than snapping off-centre on the first frame.
    let fx_target = region.center_x.clamp(0.0, 1.0);
    let fy_target = region.center_y.clamp(0.0, 1.0);
    let mut out = Vec::with_capacity(samples + 1);
    for i in 0..=samples {
        let t = region.start + step * i as f64;
        let scale = region.scale_at(t).max(1.0);
        // Fractional progress of the zoom from 1.0 → region.scale; drives the
        // centre lerp so rest frames stay centred.
        let p = if region.scale > 1.0 {
            ((scale - 1.0) / (region.scale - 1.0)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let fx = 0.5 + (fx_target - 0.5) * p;
        let fy = 0.5 + (fy_target - 0.5) * p;
        let cw = iw / scale;
        let ch = ih / scale;
        // Clamp so the crop window never leaves the source frame.
        let x = ((iw - cw) * fx).clamp(0.0, iw - cw);
        let y = ((ih - ch) * fy).clamp(0.0, ih - ch);
        out.push(ZoomSample {
            t,
            width: cw,
            height: ch,
            x,
            y,
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
