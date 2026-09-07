pub const TIMECODE_BITS: u32 = 32;

/// Black/white blocks survive 4:2:0 subsampling and heavy quantisation, which a
/// colour or text encoding does not.
pub fn block_size(width: u32) -> u32 {
    (width / (TIMECODE_BITS + 8)).clamp(4, 32)
}

pub fn render_frame(width: u32, height: u32, frame_index: u64) -> Vec<u8> {
    assert!(
        width >= 64 && height >= 32,
        "frame too small to carry a timecode"
    );
    let block = block_size(width);
    let mut rgba = vec![0u8; (width * height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let shade = (((x / 32) + (y / 32)) % 2) as u8 * 24 + 40;
            let offset = ((y * width + x) * 4) as usize;
            rgba[offset] = shade;
            rgba[offset + 1] = shade;
            rgba[offset + 2] = shade;
            rgba[offset + 3] = 255;
        }
    }

    for bit in 0..TIMECODE_BITS {
        let set = (frame_index >> (TIMECODE_BITS - 1 - bit)) & 1 == 1;
        let value = if set { 255 } else { 0 };
        let x0 = bit * block;
        for y in 0..block.min(height) {
            for x in x0..(x0 + block).min(width) {
                let offset = ((y * width + x) * 4) as usize;
                rgba[offset] = value;
                rgba[offset + 1] = value;
                rgba[offset + 2] = value;
            }
        }
    }

    let marker_x = (frame_index as u32 % (width.saturating_sub(8))).min(width - 8);
    let marker_y = height - 8;
    for y in marker_y..height {
        for x in marker_x..(marker_x + 8).min(width) {
            let offset = ((y * width + x) * 4) as usize;
            rgba[offset] = 255;
            rgba[offset + 1] = 0;
            rgba[offset + 2] = 0;
        }
    }

    rgba
}

/// `None` when a block's average luma sits in the ambiguous middle, which means
/// the frame is torn or was not produced by [`render_frame`].
pub fn decode_frame(rgba: &[u8], width: u32, height: u32) -> Option<u64> {
    if rgba.len() < (width * height * 4) as usize {
        return None;
    }
    let block = block_size(width);
    let inset = (block / 4).max(1);
    let mut value = 0u64;

    for bit in 0..TIMECODE_BITS {
        let x0 = bit * block + inset;
        let x1 = (bit * block + block - inset).min(width);
        let y1 = block.saturating_sub(inset).min(height);
        if x0 >= x1 || inset >= y1 {
            return None;
        }

        let mut total = 0u64;
        let mut count = 0u64;
        for y in inset..y1 {
            for x in x0..x1 {
                let offset = ((y * width + x) * 4) as usize;
                let luma = rgba[offset] as u64 * 299
                    + rgba[offset + 1] as u64 * 587
                    + rgba[offset + 2] as u64 * 114;
                total += luma / 1000;
                count += 1;
            }
        }
        let mean = total / count.max(1);
        let bit_set = match mean {
            0..=80 => false,
            176..=255 => true,
            _ => return None,
        };
        value = (value << 1) | u64::from(bit_set);
    }

    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rendered_frame_decodes_to_its_own_index() {
        for index in [0u64, 1, 2, 59, 60, 1799, 216_000, u32::MAX as u64] {
            let frame = render_frame(1280, 720, index);
            assert_eq!(
                decode_frame(&frame, 1280, 720),
                Some(index),
                "index {index}"
            );
        }
    }

    #[test]
    fn decoding_survives_a_small_uniform_shift() {
        let mut frame = render_frame(640, 360, 12_345);
        for byte in frame.iter_mut() {
            *byte = byte.saturating_add(20);
        }
        assert_eq!(decode_frame(&frame, 640, 360), Some(12_345));
    }

    #[test]
    fn a_frame_that_is_not_a_timecode_is_rejected() {
        let flat = vec![128u8; 640 * 360 * 4];
        assert_eq!(decode_frame(&flat, 640, 360), None);
    }

    #[test]
    fn a_truncated_buffer_is_rejected() {
        assert_eq!(decode_frame(&[0u8; 16], 640, 360), None);
    }

    #[test]
    fn every_frame_index_renders_a_distinct_image() {
        let a = render_frame(320, 240, 7);
        let b = render_frame(320, 240, 8);
        assert_ne!(a, b);
    }
}
