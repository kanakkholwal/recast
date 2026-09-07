#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub type Mat3 = [[f32; 3]; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(rename_all = "kebab-case")
)]
pub enum Primaries {
    #[default]
    Bt709,
    Bt601Ntsc,
    Bt2020,
    DisplayP3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(rename_all = "kebab-case")
)]
pub enum MatrixCoefficients {
    #[default]
    Bt709,
    Bt601,
    Bt2020Ncl,
    Identity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(rename_all = "kebab-case")
)]
pub enum ColorRange {
    #[default]
    Limited,
    Full,
}

impl Primaries {
    pub fn to_xyz(self) -> Mat3 {
        match self {
            Self::Bt709 => [
                [0.412_390_8, 0.357_584_3, 0.180_480_8],
                [0.212_639, 0.715_168_7, 0.072_192_32],
                [0.019_330_82, 0.119_194_78, 0.950_532_2],
            ],
            Self::Bt601Ntsc => [
                [0.393_521, 0.365_258_4, 0.191_677],
                [0.212_376, 0.701_060, 0.086_564],
                [0.018_739, 0.111_934, 0.958_385],
            ],
            Self::Bt2020 => [
                [0.636_958, 0.144_617, 0.168_881],
                [0.262_700, 0.677_998, 0.059_302],
                [0.0, 0.028_073, 1.060_985],
            ],
            Self::DisplayP3 => [
                [0.486_571, 0.265_668, 0.198_217],
                [0.228_975, 0.691_739, 0.079_287],
                [0.0, 0.045_113, 1.043_944],
            ],
        }
    }

    pub fn from_xyz(self) -> Mat3 {
        invert(self.to_xyz())
    }

    pub fn luma_weights(self) -> [f32; 3] {
        let m = self.to_xyz();
        m[1]
    }
}

pub fn convert_matrix(from: Primaries, to: Primaries) -> Mat3 {
    multiply(to.from_xyz(), from.to_xyz())
}

pub fn multiply(a: Mat3, b: Mat3) -> Mat3 {
    let mut out = [[0.0f32; 3]; 3];
    for (r, row) in out.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().enumerate() {
            *cell = a[r][0] * b[0][c] + a[r][1] * b[1][c] + a[r][2] * b[2][c];
        }
    }
    out
}

pub fn apply(m: Mat3, v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

pub fn invert(m: Mat3) -> Mat3 {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() < f32::EPSILON {
        return IDENTITY;
    }
    let inv = 1.0 / det;
    [
        [
            (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * inv,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * inv,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * inv,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * inv,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * inv,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * inv,
        ],
        [
            (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * inv,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * inv,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * inv,
        ],
    ]
}

pub const IDENTITY: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

impl MatrixCoefficients {
    pub fn kr_kb(self) -> (f32, f32) {
        match self {
            Self::Bt709 => (0.2126, 0.0722),
            Self::Bt601 => (0.299, 0.114),
            Self::Bt2020Ncl => (0.2627, 0.0593),
            Self::Identity => (0.0, 0.0),
        }
    }

    /// Non-constant-luminance R'G'B' to Y'CbCr, with Cb/Cr centred on zero.
    pub fn rgb_to_ycbcr(self) -> Mat3 {
        if self == Self::Identity {
            return IDENTITY;
        }
        let (kr, kb) = self.kr_kb();
        let kg = 1.0 - kr - kb;
        [
            [kr, kg, kb],
            [-kr / (2.0 * (1.0 - kb)), -kg / (2.0 * (1.0 - kb)), 0.5],
            [0.5, -kg / (2.0 * (1.0 - kr)), -kb / (2.0 * (1.0 - kr))],
        ]
    }

    pub fn ycbcr_to_rgb(self) -> Mat3 {
        invert(self.rgb_to_ycbcr())
    }
}

impl ColorRange {
    /// Scale and offset taking normalised code values to 0..1 luma and
    /// -0.5..0.5 chroma, for the given bit depth.
    pub fn luma_scale_offset(self, bit_depth: u32) -> (f32, f32) {
        match self {
            Self::Full => (1.0, 0.0),
            Self::Limited => {
                let max = ((1u32 << bit_depth) - 1) as f32;
                let low = 16.0 * (1 << (bit_depth - 8)) as f32;
                let high = 235.0 * (1 << (bit_depth - 8)) as f32;
                (max / (high - low), -low / max)
            }
        }
    }

    pub fn chroma_scale(self, bit_depth: u32) -> f32 {
        match self {
            Self::Full => 1.0,
            Self::Limited => {
                let max = ((1u32 << bit_depth) - 1) as f32;
                let low = 16.0 * (1 << (bit_depth - 8)) as f32;
                let high = 240.0 * (1 << (bit_depth - 8)) as f32;
                max / (high - low)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close3(a: [f32; 3], b: [f32; 3], tolerance: f32) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < tolerance)
    }

    #[test]
    fn a_primaries_matrix_inverts_to_identity() {
        for p in [
            Primaries::Bt709,
            Primaries::Bt601Ntsc,
            Primaries::Bt2020,
            Primaries::DisplayP3,
        ] {
            let product = multiply(p.from_xyz(), p.to_xyz());
            for (r, row) in product.iter().enumerate() {
                for (c, cell) in row.iter().enumerate() {
                    let expected = if r == c { 1.0 } else { 0.0 };
                    assert!((cell - expected).abs() < 1e-4, "{p:?} at {r},{c}: {cell}");
                }
            }
        }
    }

    #[test]
    fn bt709_white_maps_to_d65() {
        let xyz = apply(Primaries::Bt709.to_xyz(), [1.0, 1.0, 1.0]);
        assert!(close3(xyz, [0.9505, 1.0, 1.089], 2e-3), "{xyz:?}");
    }

    #[test]
    fn luma_weights_are_the_rec709_coefficients() {
        let w = Primaries::Bt709.luma_weights();
        assert!((w[0] - 0.2126).abs() < 1e-3);
        assert!((w[1] - 0.7152).abs() < 1e-3);
        assert!((w[2] - 0.0722).abs() < 1e-3);
    }

    #[test]
    fn converting_a_gamut_to_itself_is_the_identity() {
        let m = convert_matrix(Primaries::Bt709, Primaries::Bt709);
        assert!(close3(apply(m, [0.3, 0.6, 0.9]), [0.3, 0.6, 0.9], 1e-4));
    }

    #[test]
    fn bt2020_is_wider_so_srgb_green_moves_inward() {
        let m = convert_matrix(Primaries::Bt709, Primaries::Bt2020);
        let green = apply(m, [0.0, 1.0, 0.0]);
        assert!(green[1] < 1.0, "{green:?}");
        assert!(green[1] > 0.5, "{green:?}");
    }

    #[test]
    fn ycbcr_round_trips_through_its_inverse() {
        for coeff in [
            MatrixCoefficients::Bt709,
            MatrixCoefficients::Bt601,
            MatrixCoefficients::Bt2020Ncl,
        ] {
            let rgb = [0.2, 0.7, 0.4];
            let ycbcr = apply(coeff.rgb_to_ycbcr(), rgb);
            let back = apply(coeff.ycbcr_to_rgb(), ycbcr);
            assert!(close3(back, rgb, 1e-4), "{coeff:?}: {back:?}");
        }
    }

    #[test]
    fn grey_has_no_chroma() {
        let ycbcr = apply(MatrixCoefficients::Bt709.rgb_to_ycbcr(), [0.5, 0.5, 0.5]);
        assert!((ycbcr[0] - 0.5).abs() < 1e-5);
        assert!(ycbcr[1].abs() < 1e-5);
        assert!(ycbcr[2].abs() < 1e-5);
    }

    #[test]
    fn bt709_and_bt601_disagree_which_is_why_the_matrix_must_be_carried() {
        let rgb = [0.9, 0.2, 0.1];
        let a = apply(MatrixCoefficients::Bt709.rgb_to_ycbcr(), rgb);
        let b = apply(MatrixCoefficients::Bt601.rgb_to_ycbcr(), rgb);
        assert!((a[0] - b[0]).abs() > 1e-2, "{a:?} vs {b:?}");
    }

    #[test]
    fn limited_range_expands_16_235_to_full_scale() {
        let (scale, offset) = ColorRange::Limited.luma_scale_offset(8);
        let black = (16.0 / 255.0 + offset) * scale;
        let white = (235.0 / 255.0 + offset) * scale;
        assert!(black.abs() < 1e-5, "{black}");
        assert!((white - 1.0).abs() < 1e-5, "{white}");
    }

    #[test]
    fn full_range_is_a_no_op() {
        assert_eq!(ColorRange::Full.luma_scale_offset(8), (1.0, 0.0));
        assert_eq!(ColorRange::Full.chroma_scale(10), 1.0);
    }

    #[test]
    fn a_singular_matrix_inverts_to_identity_rather_than_producing_nans() {
        let singular = [[1.0, 2.0, 3.0], [2.0, 4.0, 6.0], [1.0, 1.0, 1.0]];
        assert_eq!(invert(singular), IDENTITY);
    }
}
