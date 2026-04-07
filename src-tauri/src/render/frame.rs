/// An RGBA8 pixel buffer with dimensions.
#[derive(Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Row-major RGBA8 pixel data. Length = width * height * 4.
    pub data: Vec<u8>,
}

impl Frame {
    /// Create a frame filled with a solid RGBA color.
    pub fn solid(width: u32, height: u32, r: u8, g: u8, b: u8, a: u8) -> Self {
        let pixel = [r, g, b, a];
        let data = pixel.repeat((width * height) as usize);
        Self { width, height, data }
    }

    /// Get a pixel at (x, y). Returns [R, G, B, A].
    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let offset = ((y * self.width + x) * 4) as usize;
        [
            self.data[offset],
            self.data[offset + 1],
            self.data[offset + 2],
            self.data[offset + 3],
        ]
    }

    /// Set a pixel at (x, y) with [R, G, B, A].
    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, pixel: [u8; 4]) {
        let offset = ((y * self.width + x) * 4) as usize;
        self.data[offset] = pixel[0];
        self.data[offset + 1] = pixel[1];
        self.data[offset + 2] = pixel[2];
        self.data[offset + 3] = pixel[3];
    }

    /// Alpha-composite `src` over `self` at position (dst_x, dst_y).
    /// Clips to bounds. Handles alpha blending.
    pub fn overlay(&mut self, src: &Frame, dst_x: i32, dst_y: i32) {
        let src_start_x = (-dst_x).max(0) as u32;
        let src_start_y = (-dst_y).max(0) as u32;
        let dst_start_x = dst_x.max(0) as u32;
        let dst_start_y = dst_y.max(0) as u32;

        let copy_w = (src.width - src_start_x).min(self.width.saturating_sub(dst_start_x));
        let copy_h = (src.height - src_start_y).min(self.height.saturating_sub(dst_start_y));

        for row in 0..copy_h {
            let sy = src_start_y + row;
            let dy = dst_start_y + row;
            for col in 0..copy_w {
                let sx = src_start_x + col;
                let dx = dst_start_x + col;
                let src_px = src.get_pixel(sx, sy);
                let sa = src_px[3] as u32;

                if sa == 0 {
                    continue;
                }
                if sa == 255 {
                    self.set_pixel(dx, dy, src_px);
                    continue;
                }

                let dst_px = self.get_pixel(dx, dy);
                let da = dst_px[3] as u32;
                let inv_sa = 255 - sa;

                let out_a = sa + (da * inv_sa) / 255;
                if out_a == 0 {
                    continue;
                }

                let blend = |s: u8, d: u8| -> u8 {
                    ((s as u32 * sa + d as u32 * da * inv_sa / 255) / out_a).min(255) as u8
                };

                self.set_pixel(dx, dy, [
                    blend(src_px[0], dst_px[0]),
                    blend(src_px[1], dst_px[1]),
                    blend(src_px[2], dst_px[2]),
                    out_a.min(255) as u8,
                ]);
            }
        }
    }

    /// Resize this frame to the target dimensions using bilinear interpolation.
    pub fn resize(&self, target_width: u32, target_height: u32) -> Frame {
        if target_width == self.width && target_height == self.height {
            return self.clone();
        }

        let mut result = Frame::solid(target_width, target_height, 0, 0, 0, 0);
        let x_ratio = self.width as f64 / target_width as f64;
        let y_ratio = self.height as f64 / target_height as f64;

        for dy in 0..target_height {
            let src_y = dy as f64 * y_ratio;
            let sy0 = (src_y.floor() as u32).min(self.height - 1);
            let sy1 = (sy0 + 1).min(self.height - 1);
            let fy = src_y - src_y.floor();

            for dx in 0..target_width {
                let src_x = dx as f64 * x_ratio;
                let sx0 = (src_x.floor() as u32).min(self.width - 1);
                let sx1 = (sx0 + 1).min(self.width - 1);
                let fx = src_x - src_x.floor();

                let p00 = self.get_pixel(sx0, sy0);
                let p10 = self.get_pixel(sx1, sy0);
                let p01 = self.get_pixel(sx0, sy1);
                let p11 = self.get_pixel(sx1, sy1);

                let mut px = [0u8; 4];
                for c in 0..4 {
                    let v = p00[c] as f64 * (1.0 - fx) * (1.0 - fy)
                        + p10[c] as f64 * fx * (1.0 - fy)
                        + p01[c] as f64 * (1.0 - fx) * fy
                        + p11[c] as f64 * fx * fy;
                    px[c] = v.round().clamp(0.0, 255.0) as u8;
                }
                result.set_pixel(dx, dy, px);
            }
        }

        result
    }

    /// Crop a rectangular region from this frame.
    pub fn crop(&self, x: u32, y: u32, w: u32, h: u32) -> Frame {
        let cw = w.min(self.width.saturating_sub(x));
        let ch = h.min(self.height.saturating_sub(y));
        let mut result = Frame::solid(cw, ch, 0, 0, 0, 0);

        for row in 0..ch {
            let src_offset = ((y + row) * self.width + x) as usize * 4;
            let dst_offset = (row * cw) as usize * 4;
            let len = cw as usize * 4;
            result.data[dst_offset..dst_offset + len]
                .copy_from_slice(&self.data[src_offset..src_offset + len]);
        }

        result
    }

    /// Encode this frame as a PNG and return the bytes.
    pub fn encode_png(&self) -> Result<Vec<u8>, String> {
        use image::codecs::png::PngEncoder;
        use image::{ColorType, ImageEncoder};
        use std::io::Cursor;

        let mut buf = Cursor::new(Vec::new());
        let enc = PngEncoder::new(&mut buf);
        enc.write_image(&self.data, self.width, self.height, ColorType::Rgba8.into())
            .map_err(|e| e.to_string())?;
        Ok(buf.into_inner())
    }
}
