use std::sync::OnceLock;
use std::fs;

use crate::cursor::smoothing::{SmoothingConfig, interpolate_at, smooth_samples};
use crate::cursor::{CursorSample, CursorTrack};
use crate::render::frame::Frame;
use crate::render::pipeline::{ProcessingNode, RenderContext};

/// Pre-loaded cursor data (samples + track), computed once.
struct CursorData {
    samples: Vec<CursorSample>,
    track: CursorTrack,
}

/// Overlays a cursor indicator on the video frame at the correct position.
pub struct CursorOverlay {
    pub enabled: bool,
    pub size: f64,
    pub smoothing: f64,
    pub highlight_clicks: bool,
    pub highlight_color: String,
    pub highlight_opacity: f64,
    pub hide_when_idle: bool,
    pub idle_timeout: f64,
    /// Lazily loaded cursor data.
    data: OnceLock<Option<CursorData>>,
}

impl CursorOverlay {
    pub fn new(
        enabled: bool,
        size: f64,
        smoothing: f64,
        highlight_clicks: bool,
        highlight_color: String,
        highlight_opacity: f64,
        hide_when_idle: bool,
        idle_timeout: f64,
    ) -> Self {
        Self {
            enabled,
            size,
            smoothing,
            highlight_clicks,
            highlight_color,
            highlight_opacity,
            hide_when_idle,
            idle_timeout,
            data: OnceLock::new(),
        }
    }

    fn get_data(&self, ctx: &RenderContext) -> &Option<CursorData> {
        self.data.get_or_init(|| {
            let path = ctx.cursor_track_path?;

            let raw = fs::read_to_string(path).ok()?;
            let track: CursorTrack = serde_json::from_str(&raw).ok()?;
            let config = SmoothingConfig::from_slider(self.smoothing);
            let samples = smooth_samples(&track.samples, config);

            Some(CursorData { samples, track })
        })
    }
}

impl ProcessingNode for CursorOverlay {
    fn name(&self) -> &str {
        "cursor"
    }

    fn process(&self, frame: Frame, ctx: &RenderContext) -> Result<Frame, String> {
        if !self.enabled {
            return Ok(frame);
        }

        let cursor_data = match self.get_data(ctx) {
            Some(d) if !d.samples.is_empty() => d,
            _ => return Ok(frame),
        };

        let timestamp_us = (ctx.timestamp * 1_000_000.0) as u64;

        // Check if cursor should be hidden due to idle.
        if self.hide_when_idle {
            let idle_threshold_us = (self.idle_timeout * 1_000_000.0) as u64;
            let is_idle = cursor_data.track.idle_periods.iter().any(|period| {
                timestamp_us >= period.start_us + idle_threshold_us
                    && timestamp_us <= period.end_us
            });
            if is_idle {
                return Ok(frame);
            }
        }

        let Some(pos) = interpolate_at(&cursor_data.samples, timestamp_us) else {
            return Ok(frame);
        };

        if !pos.visible {
            return Ok(frame);
        }

        // Map cursor position from screen coordinates to video frame coordinates.
        let cursor_x = pos.x - 0.0; // TODO: subtract capture region offset if cropped
        let cursor_y = pos.y - 0.0;

        // Scale cursor position to frame coordinates.
        let scale_x = frame.width as f64 / ctx.source_width as f64;
        let scale_y = frame.height as f64 / ctx.source_height as f64;
        let frame_x = (cursor_x * scale_x).round() as i32;
        let frame_y = (cursor_y * scale_y).round() as i32;

        let mut result = frame;

        // Draw click highlight circle first (behind cursor).
        if self.highlight_clicks && (pos.left_down || pos.right_down) {
            let (hr, hg, hb) = parse_hex_color(&self.highlight_color);
            let ha = ((self.highlight_opacity / 100.0) * 255.0).round() as u8;
            let highlight_radius = (self.size * 12.0).round() as i32;
            draw_filled_circle(&mut result, frame_x, frame_y, highlight_radius, [hr, hg, hb, ha]);
        }

        // Draw cursor dot.
        let cursor_radius = (self.size * 2.0).max(2.0).round() as i32;
        draw_filled_circle(&mut result, frame_x, frame_y, cursor_radius, [255, 255, 255, 230]);
        // Dark border for visibility.
        draw_circle_border(&mut result, frame_x, frame_y, cursor_radius, [0, 0, 0, 180]);

        Ok(result)
    }
}

fn draw_filled_circle(frame: &mut Frame, cx: i32, cy: i32, radius: i32, color: [u8; 4]) {
    let r2 = (radius * radius) as f64;
    let x_min = (cx - radius).max(0) as u32;
    let x_max = ((cx + radius) as u32).min(frame.width.saturating_sub(1));
    let y_min = (cy - radius).max(0) as u32;
    let y_max = ((cy + radius) as u32).min(frame.height.saturating_sub(1));

    for y in y_min..=y_max {
        for x in x_min..=x_max {
            let dx = x as f64 - cx as f64;
            let dy = y as f64 - cy as f64;
            let dist2 = dx * dx + dy * dy;

            if dist2 <= r2 {
                // Anti-alias the edge.
                let edge_dist = r2.sqrt() - dist2.sqrt();
                let alpha = if edge_dist < 1.0 {
                    (color[3] as f64 * edge_dist).round() as u8
                } else {
                    color[3]
                };

                if alpha == 0 {
                    continue;
                }

                let dst = frame.get_pixel(x, y);
                let sa = alpha as u32;
                let inv_sa = 255 - sa;

                let blend = |s: u8, d: u8| -> u8 {
                    ((s as u32 * sa + d as u32 * inv_sa) / 255).min(255) as u8
                };

                frame.set_pixel(x, y, [
                    blend(color[0], dst[0]),
                    blend(color[1], dst[1]),
                    blend(color[2], dst[2]),
                    (sa + dst[3] as u32 * inv_sa / 255).min(255) as u8,
                ]);
            }
        }
    }
}

fn draw_circle_border(frame: &mut Frame, cx: i32, cy: i32, radius: i32, color: [u8; 4]) {
    let outer_r2 = ((radius + 1) * (radius + 1)) as f64;
    let inner_r2 = (radius * radius) as f64;
    let x_min = (cx - radius - 1).max(0) as u32;
    let x_max = ((cx + radius + 1) as u32).min(frame.width.saturating_sub(1));
    let y_min = (cy - radius - 1).max(0) as u32;
    let y_max = ((cy + radius + 1) as u32).min(frame.height.saturating_sub(1));

    for y in y_min..=y_max {
        for x in x_min..=x_max {
            let dx = x as f64 - cx as f64;
            let dy = y as f64 - cy as f64;
            let dist2 = dx * dx + dy * dy;

            if dist2 >= inner_r2 && dist2 <= outer_r2 {
                let dst = frame.get_pixel(x, y);
                let sa = color[3] as u32;
                let inv_sa = 255 - sa;

                let blend = |s: u8, d: u8| -> u8 {
                    ((s as u32 * sa + d as u32 * inv_sa) / 255).min(255) as u8
                };

                frame.set_pixel(x, y, [
                    blend(color[0], dst[0]),
                    blend(color[1], dst[1]),
                    blend(color[2], dst[2]),
                    (sa + dst[3] as u32 * inv_sa / 255).min(255) as u8,
                ]);
            }
        }
    }
}

fn parse_hex_color(hex: &str) -> (u8, u8, u8) {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(59);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(130);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(246);
        (r, g, b)
    } else {
        (59, 130, 246) // #3b82f6 default
    }
}
