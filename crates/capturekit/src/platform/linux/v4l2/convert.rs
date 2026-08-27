//! What a V4L2 device hands over, turned into the BGRA every capturekit backend
//! delivers.
//!
//! Four kernels cover every uncompressed format a UVC webcam offers, so the
//! FourCC table below is data and the conversion is not repeated per format.
//! Compressed formats (MJPEG, H.264) are not here: decoding them is a codec, not
//! a capture backend.

use capturekit_core::{CaptureError, PixelFormat, Result};

use super::uapi::fourcc;

/// How a source format lays its samples out in memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Layout {
    /// 4:2:2 packed: the byte offsets of y0, u, y1 and v within each four bytes.
    Packed422([usize; 4]),
    /// 4:2:0 luma plane followed by an interleaved chroma plane.
    SemiPlanar420 { u: usize, v: usize },
    /// Three-byte colour triplets, by byte offset of red and blue.
    Rgb24 { r: usize, b: usize },
    /// Already BGRA or BGRX, which only needs its alpha forced opaque.
    Bgra32,
}

impl Layout {
    /// Whether chroma is shared between neighbouring pixels, which forces the
    /// negotiated size to be even.
    pub(super) const fn is_subsampled(self) -> bool {
        matches!(self, Self::Packed422(_) | Self::SemiPlanar420 { .. })
    }

    /// The tightest stride a row of this layout can be delivered at.
    pub(super) const fn min_stride(self, width: u32) -> u32 {
        match self {
            Self::Packed422(_) => width.saturating_mul(2),
            Self::SemiPlanar420 { .. } => width,
            Self::Rgb24 { .. } => width.saturating_mul(3),
            Self::Bgra32 => width.saturating_mul(4),
        }
    }

    /// Bytes a whole frame occupies at `stride`.
    fn frame_len(self, width: u32, height: u32, stride: u32) -> usize {
        let (stride, width, height) = (stride as usize, width as usize, height as usize);
        if width == 0 || height == 0 {
            return 0;
        }
        match self {
            // Half the rows of chroma at the same stride, the last stopping at its width.
            Self::SemiPlanar420 { .. } => {
                let chroma_rows = height.div_ceil(2);
                stride * (height + chroma_rows - 1) + width
            }
            _ => stride * (height - 1) + self.min_stride(width as u32) as usize,
        }
    }
}

const YUYV: Layout = Layout::Packed422([0, 1, 2, 3]);
const YVYU: Layout = Layout::Packed422([0, 3, 2, 1]);
const UYVY: Layout = Layout::Packed422([1, 0, 3, 2]);
const NV12: Layout = Layout::SemiPlanar420 { u: 0, v: 1 };
const NV21: Layout = Layout::SemiPlanar420 { u: 1, v: 0 };
const RGB24: Layout = Layout::Rgb24 { r: 0, b: 2 };
const BGR24: Layout = Layout::Rgb24 { b: 0, r: 2 };

/// The FourCCs this backend accepts and how each is laid out, best first.
///
/// 4:2:2 outranks 4:2:0 because it keeps more chroma, and both outrank the
/// 24-bit RGB modes because almost no webcam offers those above VGA. The order
/// is what negotiation walks, so it decides what a device gets opened as.
/// 'AR24' and 'XR24' are stored B, G, R, A despite what their names suggest.
pub(super) const SUPPORTED: &[(u32, Layout)] = &[
    (fourcc(b"YUYV"), YUYV),
    (fourcc(b"UYVY"), UYVY),
    (fourcc(b"YVYU"), YVYU),
    (fourcc(b"NV12"), NV12),
    (fourcc(b"NV21"), NV21),
    (fourcc(b"BGR3"), BGR24),
    (fourcc(b"RGB3"), RGB24),
    (fourcc(b"AR24"), Layout::Bgra32),
    (fourcc(b"XR24"), Layout::Bgra32),
];

/// The layout for a FourCC, or `None` if this backend cannot read it.
pub(super) fn layout_of(fourcc: u32) -> Option<Layout> {
    SUPPORTED
        .iter()
        .find(|(code, _)| *code == fourcc)
        .map(|(_, layout)| *layout)
}

fn clamp(value: i32) -> u8 {
    value.clamp(0, 255) as u8
}

/// One YCbCr sample as BGR, in BT.601 which is what every webcam produces.
///
/// `full_range` follows the driver's quantization field: taking a full-range
/// stream as limited crushes the blacks and clips the whites.
fn ycbcr(y: u8, u: u8, v: u8, full_range: bool) -> (u8, u8, u8) {
    let d = i32::from(u) - 128;
    let e = i32::from(v) - 128;
    let (r, g, b) = if full_range {
        let y = i32::from(y);
        (
            y + ((359 * e) >> 8),
            y - ((88 * d + 183 * e) >> 8),
            y + ((454 * d) >> 8),
        )
    } else {
        let c = i32::from(y) - 16;
        (
            (298 * c + 409 * e + 128) >> 8,
            (298 * c - 100 * d - 208 * e + 128) >> 8,
            (298 * c + 516 * d + 128) >> 8,
        )
    };
    (clamp(b), clamp(g), clamp(r))
}

fn write_bgr(out: &mut [u8], at: usize, bgr: (u8, u8, u8)) {
    let Some(pixel) = out.get_mut(at..at + 4) else {
        return;
    };
    pixel[0] = bgr.0;
    pixel[1] = bgr.1;
    pixel[2] = bgr.2;
    pixel[3] = 255;
}

/// Convert one frame into tightly packed BGRA, reusing `out`'s allocation.
///
/// `stride` is the source's `bytesperline`, which a driver pads freely; the
/// output is always `width * 4`.
pub(super) fn to_bgra(
    layout: Layout,
    src: &[u8],
    width: u32,
    height: u32,
    stride: u32,
    full_range: bool,
    out: &mut Vec<u8>,
) -> Result<()> {
    let needed = layout.frame_len(width, height, stride);
    if src.len() < needed {
        return Err(CaptureError::ShortBuffer {
            format: PixelFormat::Bgra8,
            width,
            height,
            needed,
            got: src.len(),
        });
    }
    out.clear();
    out.resize(width as usize * height as usize * 4, 0);
    let (w, h, stride) = (width as usize, height as usize, stride as usize);
    match layout {
        Layout::Packed422([y0, u, y1, v]) => {
            for row in 0..h {
                let line = &src[row * stride..];
                for pair in 0..w.div_ceil(2) {
                    let group = &line[pair * 4..pair * 4 + 4];
                    let (cb, cr) = (group[u], group[v]);
                    let at = (row * w + pair * 2) * 4;
                    write_bgr(out, at, ycbcr(group[y0], cb, cr, full_range));
                    if pair * 2 + 1 < w {
                        write_bgr(out, at + 4, ycbcr(group[y1], cb, cr, full_range));
                    }
                }
            }
        }
        Layout::SemiPlanar420 { u, v } => {
            let chroma = h * stride;
            for row in 0..h {
                let luma = &src[row * stride..row * stride + w];
                let pair = &src[chroma + (row / 2) * stride..];
                for (x, y) in luma.iter().enumerate() {
                    let at = (x / 2) * 2;
                    let bgr = ycbcr(*y, pair[at + u], pair[at + v], full_range);
                    write_bgr(out, (row * w + x) * 4, bgr);
                }
            }
        }
        Layout::Rgb24 { r, b } => {
            for row in 0..h {
                let line = &src[row * stride..];
                for x in 0..w {
                    let px = &line[x * 3..x * 3 + 3];
                    write_bgr(out, (row * w + x) * 4, (px[b], px[1], px[r]));
                }
            }
        }
        Layout::Bgra32 => {
            for row in 0..h {
                let line = &src[row * stride..];
                for x in 0..w {
                    let px = &line[x * 4..x * 4 + 4];
                    write_bgr(out, (row * w + x) * 4, (px[0], px[1], px[2]));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Y'CbCr for saturated red in BT.601, and what it must come out as.
    const RED_Y: u8 = 81;
    const RED_U: u8 = 90;
    const RED_V: u8 = 240;

    fn convert(layout: Layout, src: &[u8], w: u32, h: u32, stride: u32) -> Vec<u8> {
        let mut out = Vec::new();
        to_bgra(layout, src, w, h, stride, false, &mut out).expect("the frame converts");
        out
    }

    #[test]
    fn limited_range_luma_maps_the_broadcast_endpoints_onto_black_and_white() {
        let src = [16, 128, 235, 128];
        let out = convert(YUYV, &src, 2, 1, 4);
        assert_eq!(out, vec![0, 0, 0, 255, 255, 255, 255, 255]);
    }

    /// Limited range stretches 16-235 onto 0-255, so reading a full-range stream
    /// as limited would lift black off zero.
    #[test]
    fn full_range_luma_keeps_the_value_it_was_given() {
        let mut out = Vec::new();
        to_bgra(YUYV, &[16, 128, 235, 128], 2, 1, 4, true, &mut out).expect("converts");
        assert_eq!(out[0], 16);
        assert_eq!(out[4], 235);
    }

    #[test]
    fn a_red_macropixel_lands_in_the_red_channel() {
        let out = convert(YUYV, &[RED_Y, RED_U, RED_Y, RED_V], 2, 1, 4);
        assert_eq!(out[2], 255, "red channel");
        assert_eq!(out[1], 0, "green channel");
        assert_eq!(out[0], 0, "blue channel");
        assert_eq!(out[3], 255, "alpha");
    }

    /// The three 4:2:2 orders differ only in where the samples sit, so the same
    /// colour spelled three ways must convert identically. Swapping any offset
    /// in the table breaks exactly this.
    #[test]
    fn the_packed_orders_all_decode_to_the_same_colour() {
        let yuyv = convert(YUYV, &[RED_Y, RED_U, RED_Y, RED_V], 2, 1, 4);
        let uyvy = convert(UYVY, &[RED_U, RED_Y, RED_V, RED_Y], 2, 1, 4);
        let yvyu = convert(YVYU, &[RED_Y, RED_V, RED_Y, RED_U], 2, 1, 4);
        assert_eq!(yuyv, uyvy);
        assert_eq!(yuyv, yvyu);
    }

    #[test]
    fn a_2x2_block_shares_one_chroma_pair_in_nv12() {
        let src = [RED_Y, RED_Y, RED_Y, RED_Y, RED_U, RED_V];
        let out = convert(NV12, &src, 2, 2, 2);
        for pixel in out.chunks_exact(4) {
            assert_eq!(pixel[2], 255, "every pixel of the block is red");
        }
    }

    /// NV21 is NV12 with the chroma pair reversed, so the same bytes have to
    /// come out a different colour.
    #[test]
    fn nv21_reads_the_chroma_pair_the_other_way_round() {
        let src = [RED_Y, RED_Y, RED_Y, RED_Y, RED_U, RED_V];
        let nv12 = convert(NV12, &src, 2, 2, 2);
        let nv21 = convert(NV21, &src, 2, 2, 2);
        assert_ne!(nv12, nv21);
    }

    #[test]
    fn rgb_triplets_are_reordered_rather_than_copied() {
        let rgb = convert(RGB24, &[10, 20, 30], 1, 1, 3);
        assert_eq!(rgb, vec![30, 20, 10, 255]);
        let bgr = convert(BGR24, &[10, 20, 30], 1, 1, 3);
        assert_eq!(bgr, vec![10, 20, 30, 255]);
    }

    /// A driver may hand back BGRX, whose fourth byte is undefined.
    #[test]
    fn a_32_bit_source_is_forced_opaque() {
        let out = convert(Layout::Bgra32, &[1, 2, 3, 0], 1, 1, 4);
        assert_eq!(out, vec![1, 2, 3, 255]);
    }

    #[test]
    fn padding_between_rows_is_left_behind() {
        let mut src = vec![0u8; 2 * 8];
        src[..4].copy_from_slice(&[16, 128, 16, 128]);
        src[8..12].copy_from_slice(&[235, 128, 235, 128]);
        let out = convert(YUYV, &src, 2, 2, 8);
        assert_eq!(out[0], 0, "the first row is black");
        assert_eq!(out[8], 255, "the second row is white, not padding");
    }

    #[test]
    fn a_frame_shorter_than_it_claims_is_refused_rather_than_read() {
        let mut out = Vec::new();
        let err = to_bgra(YUYV, &[0; 4], 640, 480, 1280, false, &mut out);
        assert!(
            matches!(err, Err(CaptureError::ShortBuffer { .. })),
            "{err:?}"
        );
    }

    #[test]
    fn the_chroma_plane_is_counted_into_a_semi_planar_frame_length() {
        assert_eq!(NV12.frame_len(4, 4, 4), 4 * 4 + 4 * 2);
        assert_eq!(YUYV.frame_len(4, 4, 8), 4 * 8);
    }

    #[test]
    fn every_listed_fourcc_resolves_to_its_own_layout() {
        assert_eq!(layout_of(fourcc(b"YUYV")), Some(YUYV));
        assert_eq!(layout_of(fourcc(b"UYVY")), Some(UYVY));
        assert_eq!(layout_of(fourcc(b"MJPG")), None);
    }
}
