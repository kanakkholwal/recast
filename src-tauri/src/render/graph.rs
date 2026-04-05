use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::nodes::{BackgroundNode, CursorNode, RenderNode, TrimNode, ZoomNode, ZoomRegion};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderState {
    pub trim_start: f64,
    pub trim_end: f64,
    pub background_type: String,
    pub background_value: String,
    pub background_blur: f64,
    pub padding: f64,
    pub cursor_enabled: bool,
    pub cursor_size: f64,
    pub cursor_smoothing: f64,
    pub cursor_highlight_clicks: bool,
    pub cursor_highlight_color: String,
    pub cursor_highlight_opacity: f64,
    pub cursor_hide_when_idle: bool,
    pub cursor_idle_timeout: f64,
    pub zoom_regions: Vec<ZoomRegion>,
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
            cursor_enabled: true,
            cursor_size: 3.0,
            cursor_smoothing: 50.0,
            cursor_highlight_clicks: true,
            cursor_highlight_color: "#3b82f6".into(),
            cursor_highlight_opacity: 40.0,
            cursor_hide_when_idle: false,
            cursor_idle_timeout: 3.0,
            zoom_regions: Vec::new(),
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

    pub fn build_export_plan(
        &self,
        source: SourceVideoMetadata,
        static_root: &Path,
        background_input_index: usize,
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
            .filter(|value| !value.is_empty());

        let mut extra_inputs = Vec::new();
        let filter_complex = match background {
            Some(background) if matches!(background.background_type.as_str(), "wallpaper" | "image") => {
                if let Some(background_path) = resolve_background_path(&background.value, static_root) {
                    extra_inputs.push(background_path);
                    let mut segments = Vec::new();
                    if let Some(zoom_filter) = zoom_filter {
                        segments.push(format!("[0:v]{zoom_filter}[video0]"));
                    }
                    let video_label = if segments.is_empty() { "0:v".to_string() } else { "[video0]".to_string() };
                    let blur_sigma = (background.blur / 8.0).max(0.0);
                    segments.push(format!(
                        "[{background_input_index}:v]scale={canvas_width}:{canvas_height}:force_original_aspect_ratio=increase,crop={canvas_width}:{canvas_height},boxblur={blur_sigma}[bg]"
                    ));
                    segments.push(format!("[bg]{video_label}overlay={padding}:{padding}[vout]"));
                    Some(segments.join(";"))
                } else {
                    build_color_background_filter(background, zoom_filter, canvas_width, canvas_height, padding)
                }
            }
            Some(background) => build_color_background_filter(
                background,
                zoom_filter,
                canvas_width,
                canvas_height,
                padding,
            ),
            None => zoom_filter.map(|value| format!("[0:v]{value}[vout]")),
        };

        let requires_map = filter_complex.is_some();

        Ok(ExportPlan {
            extra_inputs,
            filter_complex,
            video_map: if requires_map { "[vout]".into() } else { "0:v:0".into() },
        })
    }
}

fn build_color_background_filter(
    background: &BackgroundNode,
    zoom_filter: Option<String>,
    canvas_width: u32,
    canvas_height: u32,
    padding: u32,
) -> Option<String> {
    let color = match background.background_type.as_str() {
        "color" => normalize_color(&background.value),
        "gradient" => gradient_fallback_color(&background.value),
        _ => "#111111".into(),
    };

    let mut segments = Vec::new();
    if let Some(zoom_filter) = zoom_filter {
        segments.push(format!("[0:v]{zoom_filter}[video0]"));
    }
    let video_label = if segments.is_empty() { "0:v".to_string() } else { "[video0]".to_string() };
    segments.push(format!("color=c={color}:s={canvas_width}x{canvas_height}[bg]"));
    segments.push(format!("[bg]{video_label}overlay={padding}:{padding}[vout]"));
    Some(segments.join(";"))
}

fn build_zoom_filter(node: &ZoomNode, source: SourceVideoMetadata) -> String {
    if node.regions.is_empty() {
        return String::new();
    }

    let width_expr = nested_region_expr(&node.regions, "iw".into(), |region| {
        format!("iw/{:.4}", region.scale.max(1.0))
    });
    let height_expr = nested_region_expr(&node.regions, "ih".into(), |region| {
        format!("ih/{:.4}", region.scale.max(1.0))
    });
    let x_expr = nested_region_expr(&node.regions, "0".into(), |region| {
        format!("(iw-iw/{:.4})/2", region.scale.max(1.0))
    });
    let y_expr = nested_region_expr(&node.regions, "0".into(), |region| {
        format!("(ih-ih/{:.4})/2", region.scale.max(1.0))
    });

    format!(
        "crop=w='{width_expr}':h='{height_expr}':x='{x_expr}':y='{y_expr}',scale={}:{}",
        source.width, source.height
    )
}

fn nested_region_expr<F>(regions: &[ZoomRegion], default: String, value: F) -> String
where
    F: Fn(&ZoomRegion) -> String,
{
    regions.iter().rev().fold(default, |acc, region| {
        format!(
            "if(between(t,{:.3},{:.3}),{}, {})",
            region.start,
            region.end,
            value(region),
            acc
        )
    })
}

fn resolve_background_path(value: &str, static_root: &Path) -> Option<PathBuf> {
    if value.is_empty() {
        return None;
    }

    if let Some(rest) = value.strip_prefix("/wallpapers/") {
        return Some(static_root.join("wallpapers").join(rest));
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
            let normalized = if decoded.starts_with('/')
                && decoded.as_bytes().get(2) == Some(&b':')
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
