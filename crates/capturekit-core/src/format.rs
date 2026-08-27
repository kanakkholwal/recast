use crate::error::CaptureError;

/// One plane's sample geometry, from which every stride and length is derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaneFormat {
    /// Bytes in a single component sample.
    pub bytes_per_sample: u32,
    /// Components stored per plane pixel; 2 for NV12's interleaved CbCr.
    pub samples_per_pixel: u32,
    /// Horizontal, then vertical, divisor against the frame size.
    pub subsampling: (u32, u32),
}

impl PlaneFormat {
    /// Bytes a single row of this plane occupies, ignoring padding.
    #[must_use]
    pub const fn row_bytes(&self, width: u32) -> u32 {
        div_ceil(width, self.subsampling.0)
            .saturating_mul(self.samples_per_pixel)
            .saturating_mul(self.bytes_per_sample)
    }

    /// Rows this plane holds for a frame of `height`.
    #[must_use]
    pub const fn rows(&self, height: u32) -> u32 {
        div_ceil(height, self.subsampling.1)
    }
}

/// `u32::div_ceil` is not const on the pinned toolchain, and this is used to
/// build the const plane tables.
const fn div_ceil(value: u32, divisor: u32) -> u32 {
    value / divisor + (value % divisor != 0) as u32
}

const fn packed(bytes_per_sample: u32) -> PlaneFormat {
    PlaneFormat {
        bytes_per_sample,
        samples_per_pixel: 4,
        subsampling: (1, 1),
    }
}

const fn luma(bytes_per_sample: u32) -> PlaneFormat {
    PlaneFormat {
        bytes_per_sample,
        samples_per_pixel: 1,
        subsampling: (1, 1),
    }
}

const fn interleaved_chroma(bytes_per_sample: u32) -> PlaneFormat {
    PlaneFormat {
        bytes_per_sample,
        samples_per_pixel: 2,
        subsampling: (2, 2),
    }
}

const BGRA8: &[PlaneFormat] = &[packed(1)];
const RGBA8: &[PlaneFormat] = &[packed(1)];
const RGBA16F: &[PlaneFormat] = &[packed(2)];
const NV12: &[PlaneFormat] = &[luma(1), interleaved_chroma(1)];
const P010: &[PlaneFormat] = &[luma(2), interleaved_chroma(2)];

/// The pixel layout a capture backend delivers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[non_exhaustive]
pub enum PixelFormat {
    /// 8-bit BGRA, the native output of every desktop compositor here.
    Bgra8,
    /// 8-bit RGBA.
    Rgba8,
    /// 16-bit float RGBA, for HDR sources kept in linear light.
    Rgba16Float,
    /// 8-bit 4:2:0 with interleaved chroma, the usual encoder input.
    Nv12,
    /// 10-bit 4:2:0 with interleaved chroma, in the high bits of 16-bit samples.
    P010,
}

impl PixelFormat {
    /// The plane table this format's geometry is derived from.
    #[must_use]
    pub const fn planes(self) -> &'static [PlaneFormat] {
        match self {
            Self::Bgra8 => BGRA8,
            Self::Rgba8 => RGBA8,
            Self::Rgba16Float => RGBA16F,
            Self::Nv12 => NV12,
            Self::P010 => P010,
        }
    }

    /// Whether the samples are colour components rather than luma and chroma.
    #[must_use]
    pub const fn is_rgb(self) -> bool {
        matches!(self, Self::Bgra8 | Self::Rgba8 | Self::Rgba16Float)
    }

    /// Whether chroma is stored at reduced resolution, which forces even dimensions.
    #[must_use]
    pub fn is_subsampled(self) -> bool {
        self.planes()
            .iter()
            .any(|p| p.subsampling.0 > 1 || p.subsampling.1 > 1)
    }

    /// The tightest stride that can hold a row of this format.
    ///
    /// One number for every plane: NV12 and P010 stack their planes in a single
    /// allocation at a shared stride, and plane 0 sets it because no plane is
    /// ever wider than luma. A format with a per-plane stride would need a
    /// different shape here, and none of the backends deliver one.
    #[must_use]
    pub fn min_stride(self, width: u32) -> u32 {
        self.planes().first().map_or(0, |p| p.row_bytes(width))
    }

    /// Bytes a frame of this size occupies at `stride`.
    ///
    /// Only the final row of the final plane is allowed to stop at its own width,
    /// since nothing follows it that a short buffer could truncate.
    pub fn buffer_len(self, width: u32, height: u32, stride: u32) -> Result<usize, CaptureError> {
        // Before the stride check: an odd width also inflates the chroma row, so
        // the wrong complaint would surface first and hide the real problem.
        if self.is_subsampled() && (width % 2 == 1 || height % 2 == 1) {
            return Err(CaptureError::OddDimensions {
                format: self,
                width,
                height,
            });
        }
        let min_stride = self.min_stride(width);
        if stride < min_stride {
            return Err(CaptureError::StrideTooSmall {
                format: self,
                width,
                stride,
                needed: min_stride,
            });
        }
        if width == 0 || height == 0 {
            return Ok(0);
        }

        let planes = self.planes();
        let stride = u64::from(stride);
        let mut total: u64 = 0;
        for (index, plane) in planes.iter().enumerate() {
            let rows = u64::from(plane.rows(height));
            total += if index + 1 == planes.len() {
                stride * (rows - 1) + u64::from(plane.row_bytes(width))
            } else {
                stride * rows
            };
        }
        usize::try_from(total).map_err(|_| CaptureError::FrameTooLarge {
            width,
            height,
            bytes: total,
        })
    }

    /// Check a buffer really holds this frame before anything indexes into it.
    pub fn validate_buffer(
        self,
        width: u32,
        height: u32,
        stride: u32,
        len: usize,
    ) -> Result<(), CaptureError> {
        let needed = self.buffer_len(width, height, stride)?;
        if len < needed {
            return Err(CaptureError::ShortBuffer {
                format: self,
                width,
                height,
                needed,
                got: len,
            });
        }
        Ok(())
    }

    /// Byte offset of `plane` from the start of the buffer.
    #[must_use]
    pub fn plane_offset(self, plane: usize, height: u32, stride: u32) -> usize {
        self.planes()
            .iter()
            .take(plane)
            .map(|p| p.rows(height) as usize * stride as usize)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_formats_need_four_bytes_a_pixel() {
        assert_eq!(PixelFormat::Bgra8.min_stride(1920), 1920 * 4);
        assert_eq!(PixelFormat::Rgba16Float.min_stride(1920), 1920 * 8);
    }

    #[test]
    fn nv12_luma_and_chroma_rows_are_the_same_width_in_bytes() {
        // Half the columns, but two samples each, which is why one stride serves both.
        let planes = PixelFormat::Nv12.planes();
        assert_eq!(planes[0].row_bytes(1920), planes[1].row_bytes(1920));
        assert_eq!(PixelFormat::Nv12.min_stride(1920), 1920);
    }

    #[test]
    fn nv12_is_one_and_a_half_times_the_luma_plane() {
        let len = PixelFormat::Nv12
            .buffer_len(1920, 1080, 1920)
            .expect("a tight 1080p NV12 buffer");
        assert_eq!(len, 1920 * 1080 * 3 / 2);
    }

    #[test]
    fn p010_is_twice_the_size_of_nv12() {
        let nv12 = PixelFormat::Nv12
            .buffer_len(1920, 1080, 1920)
            .expect("nv12");
        let p010 = PixelFormat::P010
            .buffer_len(1920, 1080, 3840)
            .expect("p010");
        assert_eq!(p010, nv12 * 2);
    }

    #[test]
    fn padding_is_charged_for_every_row_but_the_last() {
        // 4 rows at a 4096-byte stride: three full strides plus one 3840-byte row.
        let len = PixelFormat::Bgra8
            .buffer_len(960, 4, 4096)
            .expect("a padded buffer");
        assert_eq!(len, 4096 * 3 + 960 * 4);
    }

    #[test]
    fn a_stride_shorter_than_a_row_is_refused() {
        let err = PixelFormat::Bgra8
            .buffer_len(1920, 1080, 1920)
            .expect_err("1920 bytes cannot hold a 1920px BGRA row");
        assert!(matches!(err, CaptureError::StrideTooSmall { needed, .. } if needed == 7680));
    }

    #[test]
    fn subsampled_formats_refuse_odd_dimensions() {
        assert!(matches!(
            PixelFormat::Nv12.buffer_len(1921, 1080, 1921),
            Err(CaptureError::OddDimensions { .. })
        ));
        assert!(matches!(
            PixelFormat::Nv12.buffer_len(1920, 1081, 1920),
            Err(CaptureError::OddDimensions { .. })
        ));
    }

    #[test]
    fn packed_formats_accept_odd_dimensions() {
        assert_eq!(
            PixelFormat::Bgra8.buffer_len(3, 3, 12).expect("packed 3x3"),
            36
        );
    }

    #[test]
    fn validate_buffer_rejects_a_buffer_one_byte_short() {
        let needed = PixelFormat::Bgra8.buffer_len(64, 64, 256).expect("len");
        assert!(PixelFormat::Bgra8
            .validate_buffer(64, 64, 256, needed)
            .is_ok());
        assert!(matches!(
            PixelFormat::Bgra8.validate_buffer(64, 64, 256, needed - 1),
            Err(CaptureError::ShortBuffer { .. })
        ));
    }

    #[test]
    fn chroma_starts_after_every_luma_row() {
        assert_eq!(PixelFormat::Nv12.plane_offset(1, 1080, 2048), 2048 * 1080);
        assert_eq!(PixelFormat::Nv12.plane_offset(0, 1080, 2048), 0);
    }

    #[test]
    fn an_empty_frame_needs_no_bytes() {
        assert_eq!(PixelFormat::Bgra8.buffer_len(0, 0, 0).expect("empty"), 0);
    }

    #[test]
    fn only_subsampled_formats_report_subsampling() {
        assert!(PixelFormat::Nv12.is_subsampled());
        assert!(PixelFormat::P010.is_subsampled());
        assert!(!PixelFormat::Bgra8.is_subsampled());
    }
}
