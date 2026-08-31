use crate::format::PixelFormat;

/// The chromaticities the source's primaries sit on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[non_exhaustive]
pub enum Primaries {
    /// BT.709, shared with sRGB.
    #[default]
    Bt709,
    /// BT.601 NTSC (SMPTE 170M).
    Bt601Ntsc,
    /// BT.601 PAL (BT.470 B/G).
    Bt601Pal,
    /// BT.2020, for wide-gamut and HDR sources.
    Bt2020,
    /// Display P3.
    DisplayP3,
}

/// The curve encoded sample values sit on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[non_exhaustive]
pub enum TransferFunction {
    /// The sRGB piecewise curve. What a desktop compositor actually hands out,
    /// including for content tagged BT.709.
    #[default]
    Srgb,
    /// The BT.709 camera OETF. Rarely what a display surface carries.
    Bt709,
    /// Plain gamma 2.2.
    Gamma22,
    /// Already linear light.
    Linear,
    /// SMPTE ST 2084 perceptual quantiser, for HDR10.
    Pq,
    /// Hybrid log-gamma.
    Hlg,
}

/// How far the code values are inset from the full range of the container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum ColorRange {
    /// 16-235 luma at 8 bits, the video convention.
    #[default]
    Limited,
    /// The whole container, the desktop convention.
    Full,
}

/// The matrix that takes luma and chroma back to RGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[non_exhaustive]
pub enum MatrixCoefficients {
    /// The samples are already RGB; no matrix applies.
    #[default]
    Identity,
    /// BT.709.
    Bt709,
    /// BT.601.
    Bt601,
    /// BT.2020 non-constant luminance.
    Bt2020Ncl,
}

/// Where chroma samples sit relative to their luma samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum ChromaSiting {
    /// Co-sited with the left luma sample, which H.264 and HEVC default to.
    #[default]
    Left,
    /// Centred between the luma samples, as JPEG and MPEG-1 use.
    Center,
}

/// Everything needed to take a captured sample back to linear light.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct ColorSpace {
    /// Chromaticities of the primaries.
    pub primaries: Primaries,
    /// Curve the samples are encoded on.
    pub transfer: TransferFunction,
    /// Whether code values use the full container range.
    pub range: ColorRange,
    /// Luma-chroma matrix, or `Identity` for RGB.
    pub matrix: MatrixCoefficients,
    /// Chroma sample position, ignored when not subsampled.
    pub siting: ChromaSiting,
}

impl Default for ColorSpace {
    /// Full-range sRGB, because desktop capture is the common case and the field-by-field defaults would otherwise compose into limited-range RGB, which nothing produces.
    fn default() -> Self {
        Self::SRGB
    }
}

impl ColorSpace {
    /// What a desktop compositor hands out: full-range sRGB.
    pub const SRGB: Self = Self {
        primaries: Primaries::Bt709,
        transfer: TransferFunction::Srgb,
        range: ColorRange::Full,
        matrix: MatrixCoefficients::Identity,
        siting: ChromaSiting::Left,
    };

    /// Limited-range BT.709 video, what an encoder expects.
    pub const BT709_VIDEO: Self = Self {
        primaries: Primaries::Bt709,
        transfer: TransferFunction::Srgb,
        range: ColorRange::Limited,
        matrix: MatrixCoefficients::Bt709,
        siting: ChromaSiting::Left,
    };

    /// BT.2100 PQ, for an HDR display surface.
    pub const BT2100_PQ: Self = Self {
        primaries: Primaries::Bt2020,
        transfer: TransferFunction::Pq,
        range: ColorRange::Limited,
        matrix: MatrixCoefficients::Bt2020Ncl,
        siting: ChromaSiting::Left,
    };

    /// Whether the samples carry colour components rather than luma and chroma.
    #[must_use]
    pub const fn is_rgb(self) -> bool {
        matches!(self.matrix, MatrixCoefficients::Identity)
    }

    /// Whether the transfer curve encodes more than a display can show.
    #[must_use]
    pub const fn is_hdr(self) -> bool {
        matches!(self.transfer, TransferFunction::Pq | TransferFunction::Hlg)
    }

    /// Whether this description can describe `format` at all.
    ///
    /// An RGB buffer tagged with a luma-chroma matrix, or a YUV buffer tagged
    /// `Identity`, means a backend mislabelled the frame. Catching it here beats
    /// discovering it as a green tint in an export.
    #[must_use]
    pub const fn is_consistent_with(self, format: PixelFormat) -> bool {
        format.is_rgb() == self.is_rgb()
    }
}

/// What colour space the caller wants frames delivered in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum ColorSpaceRequest {
    /// Take whatever the source reports, which is what a faithful capture wants.
    #[default]
    Auto,
    /// Convert to a fixed space, at whatever cost the backend incurs.
    Fixed(ColorSpace),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_default_is_full_range_rgb() {
        assert!(ColorSpace::SRGB.is_rgb());
        assert_eq!(ColorSpace::SRGB.range, ColorRange::Full);
    }

    #[test]
    fn video_bt709_is_not_rgb_and_is_not_hdr() {
        assert!(!ColorSpace::BT709_VIDEO.is_rgb());
        assert!(!ColorSpace::BT709_VIDEO.is_hdr());
    }

    #[test]
    fn only_pq_and_hlg_count_as_hdr() {
        assert!(ColorSpace::BT2100_PQ.is_hdr());
        assert!(!ColorSpace::SRGB.is_hdr());
    }

    #[test]
    fn an_rgb_space_matches_an_rgb_format() {
        assert!(ColorSpace::SRGB.is_consistent_with(PixelFormat::Bgra8));
        assert!(ColorSpace::BT709_VIDEO.is_consistent_with(PixelFormat::Nv12));
    }

    #[test]
    fn a_mislabelled_frame_is_caught_rather_than_tinting_the_export() {
        assert!(!ColorSpace::SRGB.is_consistent_with(PixelFormat::Nv12));
        assert!(!ColorSpace::BT709_VIDEO.is_consistent_with(PixelFormat::Bgra8));
    }

    #[test]
    fn the_default_colour_space_is_one_a_source_actually_produces() {
        assert_eq!(ColorSpace::default(), ColorSpace::SRGB);
    }
}
