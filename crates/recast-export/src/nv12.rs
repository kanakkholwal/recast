use recast_color::apply;
use recast_compositor::{encode_matrix, PlaneLayout, SourceColor};

/// Why a frame could not be converted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Nv12Error {
    #[error("a frame with a zero dimension has no pixels")]
    EmptyFrame,
    #[error("the frame holds {got} bytes, {need} needed for {width}x{height} RGBA")]
    ShortFrame {
        need: usize,
        got: usize,
        width: u32,
        height: u32,
    },
}

/// One packed RGBA frame to packed NV12, appended to `out` so a caller can
/// reuse a buffer. Chroma is box-averaged per 2x2 block; see [`encode_matrix`].
pub fn rgba_to_nv12(
    out: &mut Vec<u8>,
    rgba: &[u8],
    width: u32,
    height: u32,
    color: &SourceColor,
) -> Result<(), Nv12Error> {
    if width == 0 || height == 0 {
        return Err(Nv12Error::EmptyFrame);
    }
    let (w, h) = (width as usize, height as usize);
    let need = w * h * 4;
    if rgba.len() < need {
        return Err(Nv12Error::ShortFrame {
            need,
            got: rgba.len(),
            width,
            height,
        });
    }

    let (matrix, bias) = encode_matrix(color);
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let base = out.len();
    out.resize(base + PlaneLayout::Nv12.packed_len(width, height), 0);
    let (luma, chroma) = out[base..].split_at_mut(w * h);

    // By chroma block, not by row: one pass, both planes, no per-frame scratch.
    for cy in 0..ch {
        for cx in 0..cw {
            let mut sum = [0.0f32; 2];
            let mut count = 0.0f32;
            for y in (cy * 2)..(cy * 2 + 2).min(h) {
                for x in (cx * 2)..(cx * 2 + 2).min(w) {
                    let at = (y * w + x) * 4;
                    let rgb = [
                        f32::from(rgba[at]) / 255.0,
                        f32::from(rgba[at + 1]) / 255.0,
                        f32::from(rgba[at + 2]) / 255.0,
                    ];
                    let mut code = apply(matrix, rgb);
                    for (value, offset) in code.iter_mut().zip(bias) {
                        *value += offset;
                    }
                    luma[y * w + x] = to_code(code[0]);
                    sum[0] += code[1];
                    sum[1] += code[2];
                    count += 1.0;
                }
            }
            let at = (cy * cw + cx) * 2;
            chroma[at] = to_code(sum[0] / count);
            chroma[at + 1] = to_code(sum[1] / count);
        }
    }
    Ok(())
}

/// Rounds rather than truncates: truncation loses half a code value on every
/// pixel, which reads as a whole-image darkening against the preview.
fn to_code(value: f32) -> u8 {
    (value * 255.0).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use recast_color::ColorRange;
    use recast_compositor::decode_matrix;

    use super::*;

    fn solid(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
        rgba.iter()
            .copied()
            .cycle()
            .take(width as usize * height as usize * 4)
            .collect()
    }

    fn convert(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        rgba_to_nv12(&mut out, rgba, width, height, &SourceColor::default()).expect("converted");
        out
    }

    /// The decode the compositor's shader performs, done on the CPU so a
    /// converted frame can be checked against the picture it came from.
    fn decode(y: u8, cb: u8, cr: u8) -> [f32; 3] {
        let (matrix, bias) = decode_matrix(&SourceColor::default());
        let code = [
            f32::from(y) / 255.0,
            f32::from(cb) / 255.0,
            f32::from(cr) / 255.0,
        ];
        let out = apply(matrix, code);
        [out[0] + bias[0], out[1] + bias[1], out[2] + bias[2]]
    }

    #[test]
    fn the_output_is_exactly_one_packed_nv12_frame() {
        let out = convert(4, 4, &solid(4, 4, [10, 20, 30, 255]));
        assert_eq!(out.len(), PlaneLayout::Nv12.packed_len(4, 4));
    }

    /// Limited range is the default. White at code 255 instead of 235 is the
    /// classic blown-out export, and it looks fine until it meets a real player.
    #[test]
    fn white_lands_on_the_limited_range_ceiling() {
        let out = convert(2, 2, &solid(2, 2, [255, 255, 255, 255]));
        assert_eq!(&out[..4], &[235, 235, 235, 235]);
        assert_eq!(&out[4..6], &[128, 128]);
    }

    #[test]
    fn black_lands_on_the_limited_range_floor() {
        let out = convert(2, 2, &solid(2, 2, [0, 0, 0, 255]));
        assert_eq!(&out[..4], &[16, 16, 16, 16]);
    }

    /// Guards the `neutral_chroma` trap: neutral is 128/255, not 0.5. Getting
    /// it wrong tints every grey faintly magenta.
    #[test]
    fn grey_stays_neutral_rather_than_drifting_off_the_chroma_centre() {
        let out = convert(2, 2, &solid(2, 2, [128, 128, 128, 255]));
        assert_eq!(&out[4..6], &[128, 128], "grey picked up colour");
    }

    #[test]
    fn a_colour_survives_the_trip_through_nv12_and_back() {
        for rgba in [
            [200, 40, 60, 255],
            [30, 180, 90, 255],
            [70, 90, 220, 255],
            [128, 128, 128, 255],
        ] {
            let out = convert(2, 2, &solid(2, 2, rgba));
            let back = decode(out[0], out[4], out[5]);
            for (channel, got) in back.iter().enumerate() {
                let want = f32::from(rgba[channel]) / 255.0;
                assert!((got - want).abs() < 0.01, "{rgba:?} came back as {back:?}");
            }
        }
    }

    /// Every other fixture here is a solid colour, which a converter that
    /// sampled one corner of each block would also pass.
    #[test]
    fn a_chroma_block_averages_its_pixels_rather_than_sampling_a_corner() {
        let mut rgba = Vec::new();
        for pixel in [
            [255u8, 0, 0, 255],
            [0, 0, 255, 255],
            [0, 0, 255, 255],
            [255, 0, 0, 255],
        ] {
            rgba.extend_from_slice(&pixel);
        }
        let out = convert(2, 2, &rgba);

        let mut red = Vec::new();
        rgba_to_nv12(
            &mut red,
            &solid(2, 2, [255, 0, 0, 255]),
            2,
            2,
            &SourceColor::default(),
        )
        .expect("red");
        let mut blue = Vec::new();
        rgba_to_nv12(
            &mut blue,
            &solid(2, 2, [0, 0, 255, 255]),
            2,
            2,
            &SourceColor::default(),
        )
        .expect("blue");

        for channel in 0..2 {
            let mean = (f32::from(red[4 + channel]) + f32::from(blue[4 + channel])) / 2.0;
            assert!(
                (f32::from(out[4 + channel]) - mean).abs() <= 1.0,
                "chroma {channel} was {}, the corners average {mean}",
                out[4 + channel]
            );
            assert_ne!(
                out[4 + channel],
                red[4 + channel],
                "chroma {channel} took a corner"
            );
        }
    }

    /// Chroma rounds up, so a 3-wide frame carries two chroma columns. Rounding
    /// down drops the right edge's colour entirely.
    #[test]
    fn an_odd_sized_frame_keeps_a_chroma_column_for_its_last_pixel() {
        let out = convert(3, 3, &solid(3, 3, [200, 40, 60, 255]));
        assert_eq!(out.len(), 3 * 3 + 2 * 2 * 2);
        let chroma = &out[9..];
        assert!(
            chroma.iter().all(|&c| c != 0),
            "an edge block went unwritten"
        );
    }

    #[test]
    fn full_range_uses_the_whole_scale_where_limited_range_does_not() {
        let mut out = Vec::new();
        let color = SourceColor {
            range: ColorRange::Full,
            ..SourceColor::default()
        };
        rgba_to_nv12(&mut out, &solid(2, 2, [255, 255, 255, 255]), 2, 2, &color).expect("ok");
        assert_eq!(out[0], 255);
    }

    #[test]
    fn converting_appends_so_one_buffer_serves_a_whole_export() {
        let mut out = vec![0xAA; 3];
        let frame = solid(2, 2, [0, 0, 0, 255]);
        rgba_to_nv12(&mut out, &frame, 2, 2, &SourceColor::default()).expect("first");
        assert_eq!(out.len(), 3 + PlaneLayout::Nv12.packed_len(2, 2));
        assert_eq!(&out[..3], &[0xAA, 0xAA, 0xAA]);
    }

    #[test]
    fn a_short_frame_is_refused_rather_than_read_past_its_end() {
        let mut out = Vec::new();
        let err = rgba_to_nv12(&mut out, &[0; 8], 4, 4, &SourceColor::default());
        assert!(matches!(err, Err(Nv12Error::ShortFrame { .. })), "{err:?}");
    }

    #[test]
    fn a_zero_sized_frame_is_refused() {
        let mut out = Vec::new();
        let err = rgba_to_nv12(&mut out, &[], 0, 4, &SourceColor::default());
        assert_eq!(err, Err(Nv12Error::EmptyFrame));
    }
}
