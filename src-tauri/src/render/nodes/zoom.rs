use crate::render::frame::Frame;
use crate::render::pipeline::{ProcessingNode, RenderContext};

/// A time-based zoom region.
#[derive(Debug, Clone)]
pub struct ZoomRegion {
    pub start: f64,
    pub end: f64,
    pub scale: f64,
}

/// Applies time-based zoom by cropping and scaling the frame.
pub struct ZoomEffect {
    pub regions: Vec<ZoomRegion>,
}

impl ProcessingNode for ZoomEffect {
    fn name(&self) -> &str {
        "zoom"
    }

    fn process(&self, frame: Frame, ctx: &RenderContext) -> Result<Frame, String> {
        // Find the active zoom region for this timestamp.
        let active = self
            .regions
            .iter()
            .find(|r| ctx.timestamp >= r.start && ctx.timestamp <= r.end);

        let Some(region) = active else {
            return Ok(frame);
        };

        let scale = region.scale.max(1.0);
        if (scale - 1.0).abs() < 0.001 {
            return Ok(frame);
        }

        // Crop from center, then scale back to original size.
        let crop_w = (frame.width as f64 / scale).round() as u32;
        let crop_h = (frame.height as f64 / scale).round() as u32;
        let crop_x = (frame.width - crop_w) / 2;
        let crop_y = (frame.height - crop_h) / 2;

        let cropped = frame.crop(crop_x, crop_y, crop_w, crop_h);
        Ok(cropped.resize(frame.width, frame.height))
    }
}
