//! Box downscaling for the preview feed.
//!
//! The preview bubble is a few hundred pixels, but the camera delivers 720p or
//! more. Sending capture resolution over IPC is 110 MB/s at 720p30, so frames
//! are reduced here first, where the pixels already live.

/// A BGRA frame reduced so neither side exceeds `max_dim`.
///
/// Averages every source pixel that lands in a destination pixel rather than
/// point-sampling: a webcam bubble shown at a third of capture size shimmers
/// badly under nearest-neighbour.
pub fn downscale_bgra(src: &[u8], width: u32, height: u32, max_dim: u32) -> (Vec<u8>, u32, u32) {
    let (dst_w, dst_h) = fit(width, height, max_dim);
    if dst_w == width && dst_h == height {
        return (src.to_vec(), width, height);
    }
    let mut out = vec![0u8; dst_w as usize * dst_h as usize * 4];
    let row = width as usize * 4;
    for y in 0..dst_h as usize {
        let y0 = y * height as usize / dst_h as usize;
        let y1 = (((y + 1) * height as usize).div_ceil(dst_h as usize)).max(y0 + 1);
        for x in 0..dst_w as usize {
            let x0 = x * width as usize / dst_w as usize;
            let x1 = (((x + 1) * width as usize).div_ceil(dst_w as usize)).max(x0 + 1);
            let mut sums = [0u32; 4];
            let mut count = 0u32;
            for sy in y0..y1.min(height as usize) {
                for sx in x0..x1.min(width as usize) {
                    let at = sy * row + sx * 4;
                    let Some(px) = src.get(at..at + 4) else {
                        continue;
                    };
                    for channel in 0..4 {
                        sums[channel] += u32::from(px[channel]);
                    }
                    count += 1;
                }
            }
            if count == 0 {
                continue;
            }
            let at = (y * dst_w as usize + x) * 4;
            for channel in 0..4 {
                out[at + channel] = (sums[channel] / count) as u8;
            }
        }
    }
    (out, dst_w, dst_h)
}

/// The largest size within `max_dim` that keeps the source aspect ratio.
fn fit(width: u32, height: u32, max_dim: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= max_dim || max_dim == 0 || width == 0 || height == 0 {
        return (width, height);
    }
    let scale = f64::from(max_dim) / f64::from(longest);
    let w = ((f64::from(width) * scale).round() as u32).max(1);
    let h = ((f64::from(height) * scale).round() as u32).max(1);
    (w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_already_within_the_bound_is_untouched() {
        let src = vec![7u8; 4 * 4 * 4];
        let (out, w, h) = downscale_bgra(&src, 4, 4, 16);
        assert_eq!((w, h), (4, 4));
        assert_eq!(out, src);
    }

    #[test]
    fn the_aspect_ratio_survives_the_reduction() {
        assert_eq!(fit(1280, 720, 480), (480, 270));
        assert_eq!(fit(720, 1280, 480), (270, 480));
        assert_eq!(fit(640, 640, 480), (480, 480));
    }

    #[test]
    fn halving_averages_each_block_rather_than_sampling_one_pixel() {
        // Reduced to 1x1 this must be the mean, not any one source pixel.
        let src = vec![
            0, 0, 0, 0, // (0,0)
            40, 40, 40, 40, // (1,0)
            80, 80, 80, 80, // (0,1)
            120, 120, 120, 120, // (1,1)
        ];
        let (out, w, h) = downscale_bgra(&src, 2, 2, 1);
        assert_eq!((w, h), (1, 1));
        assert_eq!(out, vec![60, 60, 60, 60]);
    }

    #[test]
    fn a_truncated_frame_does_not_panic() {
        let src = vec![9u8; 8];
        let (out, w, h) = downscale_bgra(&src, 4, 4, 2);
        assert_eq!((w, h), (2, 2));
        assert_eq!(out.len(), 2 * 2 * 4);
    }
}
