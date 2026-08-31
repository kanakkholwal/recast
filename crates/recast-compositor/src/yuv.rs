use recast_color::TransferFunction;
use recast_color::{apply, convert_matrix, ColorRange, Mat3, MatrixCoefficients, Primaries};

/// How a decoded frame's planes are packed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaneLayout {
    /// Y, then Cb and Cr interleaved at half resolution in both axes.
    #[default]
    Nv12,
    /// Y, Cb, Cr in three planes, chroma at half resolution in both axes.
    I420,
    /// Y, Cb, Cr in three planes at full resolution.
    I444,
}

/// Where a chroma sample sits relative to the luma samples it covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChromaSiting {
    /// Co-sited with the left luma column, centred between rows. The H.264 and
    /// HEVC default, so it is what a decoder's NV12 is unless it says otherwise.
    #[default]
    Left,
    /// Centred in both axes, which is what JPEG and MPEG-1 use.
    Center,
}

/// Everything needed to take one frame's code values to linear light.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceColor {
    pub matrix: MatrixCoefficients,
    pub range: ColorRange,
    pub transfer: TransferFunction,
    pub primaries: Primaries,
    pub siting: ChromaSiting,
    pub bit_depth: u32,
}

impl Default for SourceColor {
    /// `Srgb` rather than `Rec709`, deliberately. BT.709 defines a CAMERA curve,
    /// not a display one, and every player and browser decodes 709-tagged SDR
    /// through roughly sRGB instead. The true inverse OETF would lift the
    /// shadows of every recording away from what the editor previews.
    fn default() -> Self {
        Self {
            matrix: MatrixCoefficients::Bt709,
            range: ColorRange::Limited,
            transfer: TransferFunction::Srgb,
            primaries: Primaries::Bt709,
            siting: ChromaSiting::Left,
            bit_depth: 8,
        }
    }
}

impl PlaneLayout {
    pub fn plane_count(self) -> usize {
        match self {
            Self::Nv12 => 2,
            Self::I420 | Self::I444 => 3,
        }
    }

    /// Samples across, rows down, and bytes per sample for one plane.
    /// Chroma rounds UP on an odd dimension: a 3-pixel-wide frame carries two chroma columns, and truncating would drop the right edge's colour.
    pub fn plane_size(self, index: usize, width: u32, height: u32) -> (u32, u32, u32) {
        let (cw, ch) = (width.div_ceil(2), height.div_ceil(2));
        match (self, index) {
            (_, 0) => (width, height, 1),
            (Self::Nv12, _) => (cw, ch, 2),
            (Self::I420, _) => (cw, ch, 1),
            (Self::I444, _) => (width, height, 1),
        }
    }

    pub fn plane_bytes(self, index: usize, width: u32, height: u32) -> usize {
        let (w, h, sample) = self.plane_size(index, width, height);
        w as usize * h as usize * sample as usize
    }

    /// Total length of one tightly packed frame.
    pub fn packed_len(self, width: u32, height: u32) -> usize {
        (0..self.plane_count())
            .map(|i| self.plane_bytes(i, width, height))
            .sum()
    }

    pub(crate) fn plane_format(self, index: usize) -> wgpu::TextureFormat {
        match self.plane_size(index, 2, 2).2 {
            2 => wgpu::TextureFormat::Rg8Unorm,
            _ => wgpu::TextureFormat::R8Unorm,
        }
    }

    fn code(self) -> u32 {
        match self {
            Self::Nv12 => 0,
            Self::I420 => 1,
            Self::I444 => 2,
        }
    }
}

/// The 3x3 and bias taking normalised Y'CbCr code values to R'G'B'.
/// Range scaling is folded into the matrix rather than applied first, so the shader does one multiply-add per channel instead of a scale then a matrix.
pub fn decode_matrix(color: &SourceColor) -> (Mat3, [f32; 3]) {
    let depth = color.bit_depth.clamp(8, 16);
    let (luma_scale, luma_offset) = color.range.luma_scale_offset(depth);
    let chroma_scale = color.range.chroma_scale(depth);
    let m = color.matrix.ycbcr_to_rgb();

    let mut folded = m;
    for row in folded.iter_mut() {
        row[0] *= luma_scale;
        row[1] *= chroma_scale;
        row[2] *= chroma_scale;
    }
    let neutral = neutral_chroma(depth);
    let bias = [
        luma_scale * luma_offset,
        -neutral * chroma_scale,
        -neutral * chroma_scale,
    ];
    (folded, apply(m, bias))
}

/// The 3x3 and bias taking R'G'B' back to normalised Y'CbCr code values: the
/// exact inverse of [`decode_matrix`], tested as one, and export's only source.
pub fn encode_matrix(color: &SourceColor) -> (Mat3, [f32; 3]) {
    let depth = color.bit_depth.clamp(8, 16);
    let (luma_scale, luma_offset) = color.range.luma_scale_offset(depth);
    let chroma_scale = color.range.chroma_scale(depth);
    let m = color.matrix.rgb_to_ycbcr();

    let mut folded = m;
    for value in &mut folded[0] {
        *value /= luma_scale;
    }
    for row in &mut folded[1..] {
        for value in row.iter_mut() {
            *value /= chroma_scale;
        }
    }
    let neutral = neutral_chroma(depth);
    (folded, [-luma_offset, neutral, neutral])
}

/// The code value that means no colour, as a sampler reports it.
///
/// NOT 0.5. Neutral is 2^(n-1) and a unorm texture normalises by 2^n - 1, so
/// 8-bit neutral arrives as 128/255. Subtracting a half instead leaves every
/// grey pixel faintly magenta, which is small enough to survive review and
/// large enough to see on a gradient.
fn neutral_chroma(depth: u32) -> f32 {
    (1u32 << (depth - 1)) as f32 / ((1u32 << depth) - 1) as f32
}

/// Takes the source's gamut to the BT.709 primaries the working space uses.
pub fn gamut_matrix(color: &SourceColor) -> Mat3 {
    convert_matrix(color.primaries, Primaries::Bt709)
}

/// How far to shift the chroma sample position, in chroma texels.
///
/// A linear sampler reading a half-width chroma plane at the luma UV lands a
/// quarter of a chroma texel left of a co-sited sample, because the plane has
/// half the texels and the sampler centres on its own. Vertical siting is
/// already centred between rows, so only x moves.
pub fn chroma_offset(color: &SourceColor, layout: PlaneLayout) -> [f32; 2] {
    let subsampled = layout.plane_size(1, 4, 4).0 < 4;
    if !subsampled || color.siting == ChromaSiting::Center {
        return [0.0, 0.0];
    }
    [0.25, 0.0]
}

fn transfer_code(transfer: TransferFunction) -> u32 {
    match transfer {
        TransferFunction::Srgb => 0,
        TransferFunction::Linear => 1,
        TransferFunction::Gamma22 => 2,
        TransferFunction::Rec709 => 3,
        TransferFunction::Pq => 4,
        TransferFunction::Hlg => 5,
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct YuvUniform {
    /// Rows of the folded decode matrix, `w` holding that row's bias.
    decode: [[f32; 4]; 3],
    gamut: [[f32; 4]; 3],
    /// xy = chroma texel offset, zw unused.
    chroma: [f32; 4],
    /// x = plane layout, y = transfer function, zw unused.
    codes: [u32; 4],
}

pub(crate) fn yuv_uniform(color: &SourceColor, layout: PlaneLayout) -> YuvUniform {
    let (matrix, bias) = decode_matrix(color);
    let gamut = gamut_matrix(color);
    let offset = chroma_offset(color, layout);
    YuvUniform {
        decode: [
            [matrix[0][0], matrix[0][1], matrix[0][2], bias[0]],
            [matrix[1][0], matrix[1][1], matrix[1][2], bias[1]],
            [matrix[2][0], matrix[2][1], matrix[2][2], bias[2]],
        ],
        gamut: [
            [gamut[0][0], gamut[0][1], gamut[0][2], 0.0],
            [gamut[1][0], gamut[1][1], gamut[1][2], 0.0],
            [gamut[2][0], gamut[2][1], gamut[2][2], 0.0],
        ],
        chroma: [offset[0], offset[1], 0.0, 0.0],
        codes: [layout.code(), transfer_code(color.transfer), 0, 0],
    }
}

/// One plane's bytes and its row stride.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plane<'a> {
    pub bytes: &'a [u8],
    /// Bytes per row, which equals the plane's row width when tightly packed.
    pub stride: u32,
}

/// Where a frame's samples live.
#[derive(Debug, Clone, Copy)]
pub enum PlaneData<'a> {
    /// Every plane back to back with no row padding, which is what a CPU
    /// decoder hands back.
    Packed(&'a [u8]),
    /// Per-plane slices carrying their own strides.
    Planar(&'a [Plane<'a>]),
}

/// One decoded frame, still in its native colour space.
#[derive(Debug, Clone, Copy)]
pub struct SourcePlanes<'a> {
    pub width: u32,
    pub height: u32,
    pub layout: PlaneLayout,
    pub color: SourceColor,
    pub data: PlaneData<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YuvError {
    /// Zero width or height, which no texture can hold.
    EmptyFrame,
    /// The wrong number of planes for the layout.
    PlaneCount { need: usize, got: usize },
    /// A plane is shorter than its stride and row count require.
    ShortPlane {
        index: usize,
        need: usize,
        got: usize,
    },
    /// A stride narrower than the row it claims to hold.
    ShortStride { index: usize, need: u32, got: u32 },
    /// The destination is not the linear working format.
    TargetFormat,
    /// The destination is not the frame's size, which would misplace every UV.
    TargetSize { need: (u32, u32), got: (u32, u32) },
}

impl std::fmt::Display for YuvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyFrame => write!(f, "a frame with no pixels"),
            Self::PlaneCount { need, got } => {
                write!(f, "this layout needs {need} planes, got {got}")
            }
            Self::ShortPlane { index, need, got } => {
                write!(f, "plane {index} needs {need} bytes, got {got}")
            }
            Self::ShortStride { index, need, got } => {
                write!(f, "plane {index} needs a stride of {need}, got {got}")
            }
            Self::TargetFormat => write!(f, "the destination is not the working format"),
            Self::TargetSize { need, got } => {
                write!(f, "a {need:?} frame cannot decode into a {got:?} target")
            }
        }
    }
}

impl std::error::Error for YuvError {}

/// Resolves either representation to one slice and stride per plane.
pub(crate) fn planes_of<'a>(frame: &SourcePlanes<'a>) -> Result<Vec<Plane<'a>>, YuvError> {
    if frame.width == 0 || frame.height == 0 {
        return Err(YuvError::EmptyFrame);
    }
    let count = frame.layout.plane_count();
    let planes: Vec<Plane<'a>> = match frame.data {
        PlaneData::Planar(planes) => {
            if planes.len() != count {
                return Err(YuvError::PlaneCount {
                    need: count,
                    got: planes.len(),
                });
            }
            planes.to_vec()
        }
        PlaneData::Packed(data) => {
            let mut out = Vec::with_capacity(count);
            let mut at = 0usize;
            for index in 0..count {
                let (w, _, sample) = frame.layout.plane_size(index, frame.width, frame.height);
                let len = frame.layout.plane_bytes(index, frame.width, frame.height);
                let end = at.saturating_add(len);
                if end > data.len() {
                    return Err(YuvError::ShortPlane {
                        index,
                        need: end,
                        got: data.len(),
                    });
                }
                out.push(Plane {
                    bytes: &data[at..end],
                    stride: w * sample,
                });
                at = end;
            }
            out
        }
    };

    for (index, plane) in planes.iter().enumerate() {
        let (w, h, sample) = frame.layout.plane_size(index, frame.width, frame.height);
        let row = w * sample;
        if plane.stride < row {
            return Err(YuvError::ShortStride {
                index,
                need: row,
                got: plane.stride,
            });
        }
        // The last row needs only its own width, not a whole stride, which is what a tightly packed buffer gives.
        let need = plane.stride as usize * (h as usize - 1) + row as usize;
        if plane.bytes.len() < need {
            return Err(YuvError::ShortPlane {
                index,
                need,
                got: plane.bytes.len(),
            });
        }
    }
    Ok(planes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(color: &SourceColor, y: f32, cb: f32, cr: f32) -> [f32; 3] {
        let (m, bias) = decode_matrix(color);
        let rgb = apply(m, [y, cb, cr]);
        [rgb[0] + bias[0], rgb[1] + bias[1], rgb[2] + bias[2]]
    }

    fn close(a: [f32; 3], b: [f32; 3], tolerance: f32) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < tolerance)
    }

    fn code(v: u8) -> f32 {
        v as f32 / 255.0
    }

    #[test]
    fn limited_range_black_and_white_land_on_zero_and_one() {
        let color = SourceColor::default();
        let black = decode(&color, code(16), code(128), code(128));
        let white = decode(&color, code(235), code(128), code(128));
        assert!(close(black, [0.0; 3], 1e-3), "{black:?}");
        assert!(close(white, [1.0; 3], 1e-3), "{white:?}");
    }

    /// The whole reason the range has to be carried: the same code values mean
    /// different light depending on it.
    #[test]
    fn full_range_black_is_zero_where_limited_range_black_is_negative() {
        let full = SourceColor {
            range: ColorRange::Full,
            ..Default::default()
        };
        let at_zero = decode(&full, 0.0, code(128), code(128));
        assert!(close(at_zero, [0.0; 3], 2e-3), "{at_zero:?}");
        let limited = decode(&SourceColor::default(), 0.0, code(128), code(128));
        assert!(limited[0] < -0.05, "{limited:?}");
    }

    #[test]
    fn neutral_chroma_leaves_grey_grey() {
        let color = SourceColor::default();
        let grey = decode(&color, code(126), code(128), code(128));
        assert!((grey[0] - grey[1]).abs() < 1e-4, "{grey:?}");
        assert!((grey[1] - grey[2]).abs() < 1e-4, "{grey:?}");
    }

    /// Round-tripping through `recast-color`'s forward matrix is what proves the folded scale and bias undo the encoder rather than merely looking plausible.
    #[test]
    fn a_limited_range_encode_decodes_back_to_the_same_rgb() {
        for matrix in [
            MatrixCoefficients::Bt709,
            MatrixCoefficients::Bt601,
            MatrixCoefficients::Bt2020Ncl,
        ] {
            let color = SourceColor {
                matrix,
                ..Default::default()
            };
            let rgb = [0.8, 0.3, 0.15];
            let ycbcr = apply(matrix.rgb_to_ycbcr(), rgb);
            let (luma_scale, luma_offset) = ColorRange::Limited.luma_scale_offset(8);
            let chroma_scale = ColorRange::Limited.chroma_scale(8);
            let neutral = neutral_chroma(8);
            let encoded = [
                ycbcr[0] / luma_scale - luma_offset,
                ycbcr[1] / chroma_scale + neutral,
                ycbcr[2] / chroma_scale + neutral,
            ];
            let back = decode(&color, encoded[0], encoded[1], encoded[2]);
            assert!(close(back, rgb, 1e-4), "{matrix:?}: {back:?}");
        }
    }

    #[test]
    fn bt601_and_bt709_decode_the_same_codes_differently() {
        let a = decode(&SourceColor::default(), code(120), code(90), code(200));
        let b = decode(
            &SourceColor {
                matrix: MatrixCoefficients::Bt601,
                ..Default::default()
            },
            code(120),
            code(90),
            code(200),
        );
        assert!((a[1] - b[1]).abs() > 1e-2, "{a:?} vs {b:?}");
    }

    #[test]
    fn ten_bit_limited_range_uses_its_own_footroom() {
        let color = SourceColor {
            bit_depth: 10,
            ..Default::default()
        };
        let neutral = 512.0 / 1023.0;
        let black = decode(&color, 64.0 / 1023.0, neutral, neutral);
        let white = decode(&color, 940.0 / 1023.0, neutral, neutral);
        assert!(close(black, [0.0; 3], 1e-3), "{black:?}");
        assert!(close(white, [1.0; 3], 1e-3), "{white:?}");
    }

    #[test]
    fn a_bt709_source_needs_no_gamut_conversion() {
        let m = gamut_matrix(&SourceColor::default());
        assert!(close(apply(m, [0.2, 0.6, 0.9]), [0.2, 0.6, 0.9], 1e-4));
    }

    #[test]
    fn a_bt2020_source_is_brought_into_the_working_gamut() {
        let m = gamut_matrix(&SourceColor {
            primaries: Primaries::Bt2020,
            ..Default::default()
        });
        let green = apply(m, [0.0, 1.0, 0.0]);
        assert!(green[1] > 1.0, "2020 green overshoots 709: {green:?}");
    }

    #[test]
    fn subsampled_chroma_is_shifted_but_full_resolution_chroma_is_not() {
        let left = SourceColor::default();
        assert_eq!(chroma_offset(&left, PlaneLayout::Nv12), [0.25, 0.0]);
        assert_eq!(chroma_offset(&left, PlaneLayout::I420), [0.25, 0.0]);
        assert_eq!(chroma_offset(&left, PlaneLayout::I444), [0.0, 0.0]);
        let centred = SourceColor {
            siting: ChromaSiting::Center,
            ..Default::default()
        };
        assert_eq!(chroma_offset(&centred, PlaneLayout::Nv12), [0.0, 0.0]);
    }

    #[test]
    fn packed_lengths_match_the_known_frame_sizes() {
        assert_eq!(
            PlaneLayout::Nv12.packed_len(1920, 1080),
            1920 * 1080 * 3 / 2
        );
        assert_eq!(
            PlaneLayout::I420.packed_len(1920, 1080),
            1920 * 1080 * 3 / 2
        );
        assert_eq!(PlaneLayout::I444.packed_len(1920, 1080), 1920 * 1080 * 3);
    }

    /// Truncating instead of rounding up loses the last chroma column, which
    /// shows as a colourless right edge rather than a crash.
    #[test]
    fn an_odd_dimension_keeps_a_whole_chroma_sample() {
        assert_eq!(PlaneLayout::Nv12.plane_size(1, 3, 5), (2, 3, 2));
        assert_eq!(PlaneLayout::I420.plane_size(2, 3, 5), (2, 3, 1));
        assert_eq!(PlaneLayout::Nv12.packed_len(3, 5), 15 + 12);
    }

    #[test]
    fn interleaved_chroma_needs_two_channels_and_planar_chroma_needs_one() {
        assert_eq!(
            PlaneLayout::Nv12.plane_format(1),
            wgpu::TextureFormat::Rg8Unorm
        );
        assert_eq!(
            PlaneLayout::I420.plane_format(1),
            wgpu::TextureFormat::R8Unorm
        );
        assert_eq!(
            PlaneLayout::Nv12.plane_format(0),
            wgpu::TextureFormat::R8Unorm
        );
    }

    fn frame<'a>(layout: PlaneLayout, data: PlaneData<'a>, w: u32, h: u32) -> SourcePlanes<'a> {
        SourcePlanes {
            width: w,
            height: h,
            layout,
            color: SourceColor::default(),
            data,
        }
    }

    #[test]
    fn a_packed_buffer_splits_into_planes_at_the_right_offsets() {
        let data = vec![0u8; PlaneLayout::I420.packed_len(4, 4)];
        let planes =
            planes_of(&frame(PlaneLayout::I420, PlaneData::Packed(&data), 4, 4)).expect("planes");
        assert_eq!(planes.len(), 3);
        assert_eq!((planes[0].bytes.len(), planes[0].stride), (16, 4));
        assert_eq!((planes[1].bytes.len(), planes[1].stride), (4, 2));
        assert_eq!((planes[2].bytes.len(), planes[2].stride), (4, 2));
    }

    #[test]
    fn nv12_chroma_is_one_plane_of_pairs_rather_than_two() {
        let data = vec![0u8; PlaneLayout::Nv12.packed_len(4, 4)];
        let planes =
            planes_of(&frame(PlaneLayout::Nv12, PlaneData::Packed(&data), 4, 4)).expect("planes");
        assert_eq!(planes.len(), 2);
        assert_eq!((planes[1].bytes.len(), planes[1].stride), (8, 4));
    }

    #[test]
    fn a_short_buffer_is_refused_rather_than_read_past() {
        let luma_only = vec![0u8; 10];
        assert!(matches!(
            planes_of(&frame(
                PlaneLayout::Nv12,
                PlaneData::Packed(&luma_only),
                4,
                4
            )),
            Err(YuvError::ShortPlane { index: 0, .. })
        ));
        // Luma fits and chroma does not: a whole-buffer length check would catch it, but a per-plane one must attribute it.
        let half_chroma = vec![0u8; 20];
        assert!(matches!(
            planes_of(&frame(
                PlaneLayout::Nv12,
                PlaneData::Packed(&half_chroma),
                4,
                4
            )),
            Err(YuvError::ShortPlane { index: 1, .. })
        ));
    }

    /// The reason the neutral is derived rather than written as 0.5.
    #[test]
    fn neutral_chroma_is_the_code_value_not_half_of_full_scale() {
        assert!((neutral_chroma(8) - 128.0 / 255.0).abs() < 1e-9);
        assert!((neutral_chroma(10) - 512.0 / 1023.0).abs() < 1e-9);
        let drift = (neutral_chroma(8) - 0.5) * ColorRange::Limited.chroma_scale(8);
        let cast = apply(
            MatrixCoefficients::Bt709.ycbcr_to_rgb(),
            [0.0, drift, drift],
        );
        assert!(cast[0].abs() > 2e-3, "half scale tints red: {cast:?}");
        assert!(cast[2].abs() > 2e-3, "half scale tints blue: {cast:?}");
    }

    #[test]
    fn an_empty_frame_is_refused() {
        let data = vec![0u8; 4];
        assert_eq!(
            planes_of(&frame(PlaneLayout::Nv12, PlaneData::Packed(&data), 0, 4)),
            Err(YuvError::EmptyFrame)
        );
        assert_eq!(
            planes_of(&frame(PlaneLayout::Nv12, PlaneData::Packed(&data), 4, 0)),
            Err(YuvError::EmptyFrame)
        );
    }

    #[test]
    fn the_wrong_number_of_planes_is_refused() {
        let bytes = vec![0u8; 64];
        let one = [Plane {
            bytes: &bytes,
            stride: 4,
        }];
        assert_eq!(
            planes_of(&frame(PlaneLayout::I420, PlaneData::Planar(&one), 4, 4)),
            Err(YuvError::PlaneCount { need: 3, got: 1 })
        );
    }

    /// A decoder that pads rows is the reason `Planar` exists, so the last row
    /// must not be required to carry the padding it does not have.
    #[test]
    fn a_padded_plane_needs_stride_times_rows_minus_one_plus_a_row() {
        // Stride 6 over a 4-wide plane, so the padding is real and the last row is the only one without it.
        let luma = vec![0u8; 6 * 5 + 4];
        let chroma = vec![0u8; 6 * 2 + 4];
        let planes = [
            Plane {
                bytes: &luma,
                stride: 6,
            },
            Plane {
                bytes: &chroma,
                stride: 6,
            },
        ];
        assert!(planes_of(&frame(PlaneLayout::Nv12, PlaneData::Planar(&planes), 4, 6)).is_ok());

        let short = vec![0u8; 6 * 5 + 3];
        let bad = [
            Plane {
                bytes: &short,
                stride: 6,
            },
            Plane {
                bytes: &chroma,
                stride: 6,
            },
        ];
        assert!(matches!(
            planes_of(&frame(PlaneLayout::Nv12, PlaneData::Planar(&bad), 4, 6)),
            Err(YuvError::ShortPlane { index: 0, .. })
        ));
    }

    #[test]
    fn a_stride_narrower_than_the_row_is_refused() {
        let bytes = vec![0u8; 256];
        let planes = [
            Plane {
                bytes: &bytes,
                stride: 3,
            },
            Plane {
                bytes: &bytes,
                stride: 4,
            },
        ];
        assert_eq!(
            planes_of(&frame(PlaneLayout::Nv12, PlaneData::Planar(&planes), 4, 4)),
            Err(YuvError::ShortStride {
                index: 0,
                need: 4,
                got: 3
            })
        );
    }

    #[test]
    fn the_uniform_carries_the_layout_and_transfer_the_shader_branches_on() {
        let u = yuv_uniform(&SourceColor::default(), PlaneLayout::I420);
        assert_eq!(u.codes[0], 1);
        assert_eq!(u.codes[1], 0);
        let hlg = yuv_uniform(
            &SourceColor {
                transfer: TransferFunction::Hlg,
                ..Default::default()
            },
            PlaneLayout::I444,
        );
        assert_eq!(hlg.codes[0], 2);
        assert_eq!(hlg.codes[1], 5);
        assert_eq!(hlg.chroma[0], 0.0);
    }

    fn round_trip(color: &SourceColor, rgb: [f32; 3]) -> [f32; 3] {
        let (enc, enc_bias) = encode_matrix(color);
        let mut code = apply(enc, rgb);
        for (value, bias) in code.iter_mut().zip(enc_bias) {
            *value += bias;
        }
        let (dec, dec_bias) = decode_matrix(color);
        let out = apply(dec, code);
        [
            out[0] + dec_bias[0],
            out[1] + dec_bias[1],
            out[2] + dec_bias[2],
        ]
    }

    /// The whole reason `encode_matrix` lives beside `decode_matrix`: a colour
    /// that survives the pair is a colour that survives export.
    #[test]
    fn encoding_then_decoding_returns_the_colour_it_started_with() {
        let color = SourceColor::default();
        for rgb in [
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.5, 0.5, 0.5],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.2, 0.7, 0.4],
        ] {
            let back = round_trip(&color, rgb);
            for (got, want) in back.iter().zip(rgb) {
                assert!((got - want).abs() < 1e-4, "{rgb:?} came back as {back:?}");
            }
        }
    }

    #[test]
    fn the_pair_round_trips_at_full_range_too() {
        let color = SourceColor {
            range: ColorRange::Full,
            ..SourceColor::default()
        };
        let back = round_trip(&color, [0.2, 0.7, 0.4]);
        assert!((back[0] - 0.2).abs() < 1e-4, "{back:?}");
        assert!((back[2] - 0.4).abs() < 1e-4, "{back:?}");
    }

    /// Limited range is the default, and getting it wrong is the classic
    /// washed-out or crushed export. White must land on code 235, not 255.
    #[test]
    fn limited_range_white_encodes_to_code_235_and_neutral_chroma() {
        let color = SourceColor::default();
        let (enc, bias) = encode_matrix(&color);
        let mut code = apply(enc, [1.0, 1.0, 1.0]);
        for (value, b) in code.iter_mut().zip(bias) {
            *value += b;
        }
        assert!((code[0] * 255.0 - 235.0).abs() < 0.01, "luma {code:?}");
        assert!((code[1] * 255.0 - 128.0).abs() < 0.01, "cb {code:?}");
        assert!((code[2] * 255.0 - 128.0).abs() < 0.01, "cr {code:?}");
    }

    #[test]
    fn limited_range_black_encodes_to_code_16() {
        let color = SourceColor::default();
        let (enc, bias) = encode_matrix(&color);
        let code = apply(enc, [0.0, 0.0, 0.0])[0] + bias[0];
        assert!((code * 255.0 - 16.0).abs() < 0.01, "luma {code}");
    }
}
