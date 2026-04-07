use crate::render::frame::Frame;
use crate::render::pipeline::{ProcessingNode, RenderContext};

/// Composites the video frame onto a background canvas with padding.
pub struct BackgroundCompositor {
    pub background_type: String,
    pub value: String,
    pub blur: f64,
    pub padding: u32,
}

impl ProcessingNode for BackgroundCompositor {
    fn name(&self) -> &str {
        "background"
    }

    fn process(&self, frame: Frame, ctx: &RenderContext) -> Result<Frame, String> {
        if self.padding == 0 {
            return Ok(frame);
        }

        let canvas_w = frame.width + self.padding * 2;
        let canvas_h = frame.height + self.padding * 2;

        let mut canvas = match self.background_type.as_str() {
            "wallpaper" | "image" => {
                self.load_image_background(canvas_w, canvas_h, ctx)
                    .unwrap_or_else(|_| self.solid_canvas(canvas_w, canvas_h))
            }
            "gradient" => {
                // Use first color from gradient as fallback solid.
                let color = parse_first_hex_color(&self.value);
                solid_color_frame(canvas_w, canvas_h, &color)
            }
            _ => self.solid_canvas(canvas_w, canvas_h),
        };

        canvas.overlay(&frame, self.padding as i32, self.padding as i32);
        Ok(canvas)
    }
}

impl BackgroundCompositor {
    fn solid_canvas(&self, w: u32, h: u32) -> Frame {
        let color = if self.value.trim().is_empty() {
            "#111111"
        } else {
            self.value.trim()
        };
        solid_color_frame(w, h, color)
    }

    fn load_image_background(
        &self,
        canvas_w: u32,
        canvas_h: u32,
        ctx: &RenderContext,
    ) -> Result<Frame, String> {
        let path = if let Some(rest) = self.value.strip_prefix("/wallpapers/") {
            ctx.static_root.join("wallpapers").join(rest)
        } else {
            std::path::PathBuf::from(&self.value)
        };

        if !path.exists() {
            return Err("background image not found".into());
        }

        let img = image::open(&path).map_err(|e| e.to_string())?;
        let img = img.resize_to_fill(canvas_w, canvas_h, image::imageops::FilterType::Triangle);
        let rgba = img.to_rgba8();

        Ok(Frame {
            width: canvas_w,
            height: canvas_h,
            data: rgba.into_raw(),
        })
    }
}

fn solid_color_frame(w: u32, h: u32, hex: &str) -> Frame {
    let (r, g, b) = parse_hex_color(hex);
    Frame::solid(w, h, r, g, b, 255)
}

fn parse_hex_color(hex: &str) -> (u8, u8, u8) {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(17);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(17);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(17);
        (r, g, b)
    } else {
        (17, 17, 20)
    }
}

fn parse_first_hex_color(value: &str) -> String {
    value
        .split(|c: char| c == ',' || c.is_whitespace())
        .find(|token| token.starts_with('#'))
        .map(|token| token.trim_matches(')').to_string())
        .unwrap_or_else(|| "#111111".into())
}
