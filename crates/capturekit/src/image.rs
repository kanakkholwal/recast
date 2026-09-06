use capturekit_core::{CaptureError, ColorSpace, PixelFormat, Rect, Result, Timestamp};

/// An owned CPU image, what a one-shot capture returns.
/// Owned rather than borrowed so the one-shot path has no lifetime tied to a backend that has already been torn down.
#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
    stride: u32,
    format: PixelFormat,
    color_space: ColorSpace,
    captured_at: Timestamp,
}

impl Image {
    /// Build an image, checking the buffer really holds the frame it claims to.
    pub fn new(
        bytes: Vec<u8>,
        width: u32,
        height: u32,
        stride: u32,
        format: PixelFormat,
        color_space: ColorSpace,
        captured_at: Timestamp,
    ) -> Result<Self> {
        format.validate_buffer(width, height, stride, bytes.len())?;
        if !color_space.is_consistent_with(format) {
            return Err(CaptureError::Unsupported {
                backend: "capturekit",
                operation: "describe an RGB buffer with a luma-chroma matrix",
            });
        }
        Ok(Self {
            bytes,
            width,
            height,
            stride,
            format,
            color_space,
            captured_at,
        })
    }

    /// Width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Bytes between the start of one row and the next, padding included.
    #[must_use]
    pub const fn stride(&self) -> u32 {
        self.stride
    }

    /// Pixel layout of the bytes.
    #[must_use]
    pub const fn format(&self) -> PixelFormat {
        self.format
    }

    /// Colour description of the bytes.
    #[must_use]
    pub const fn color_space(&self) -> ColorSpace {
        self.color_space
    }

    /// When the source produced the frame, on its own monotonic clock.
    #[must_use]
    pub const fn captured_at(&self) -> Timestamp {
        self.captured_at
    }

    /// The whole buffer, padding included.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Take ownership of the buffer.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// One row of the first plane, padding excluded.
    #[must_use]
    pub fn row(&self, y: u32) -> Option<&[u8]> {
        if y >= self.height {
            return None;
        }
        let start = y as usize * self.stride as usize;
        let width = self.format.planes().first()?.row_bytes(self.width) as usize;
        self.bytes.get(start..start + width)
    }

    /// The image rectangle in its own coordinates.
    #[must_use]
    pub const fn rect(&self) -> Rect {
        Rect::from_size(self.width, self.height)
    }

    /// A cropped copy, with the region shrunk to fit and to an even size.
    /// The fallback for backends that cannot crop during acquisition. Prefer passing the region to the capture itself, which crops on the GPU and never touches the pixels outside it.
    pub fn cropped(&self, region: Rect) -> Result<Self> {
        let fitted = region
            .fit_inside(&self.rect())
            .ok_or(CaptureError::Unsupported {
                backend: "capturekit",
                operation: "crop to a region outside the frame",
            })?;
        if fitted == self.rect() {
            return Ok(self.clone());
        }
        if !self.format.is_rgb() {
            return Err(CaptureError::Unsupported {
                backend: "capturekit",
                operation: "crop a subsampled frame on the CPU",
            });
        }

        let bytes_per_pixel = self.format.min_stride(1) as usize;
        let stride = fitted.width as usize * bytes_per_pixel;
        let mut bytes = Vec::with_capacity(stride * fitted.height as usize);
        let left = fitted.x as usize * bytes_per_pixel;
        for y in 0..fitted.height {
            let row_start = (fitted.y as u32 + y) as usize * self.stride as usize + left;
            let row =
                self.bytes
                    .get(row_start..row_start + stride)
                    .ok_or(CaptureError::ShortBuffer {
                        format: self.format,
                        width: self.width,
                        height: self.height,
                        needed: row_start + stride,
                        got: self.bytes.len(),
                    })?;
            bytes.extend_from_slice(row);
        }

        Self::new(
            bytes,
            fitted.width,
            fitted.height,
            stride as u32,
            self.format,
            self.color_space,
            self.captured_at,
        )
    }
}

impl core::fmt::Debug for Image {
    /// Omits the pixels; a 4K frame's bytes are not a useful thing to print.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Image")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("stride", &self.stride)
            .field("format", &self.format)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(width: u32, height: u32) -> Image {
        let stride = width * 4;
        let mut bytes = vec![0u8; (stride * height) as usize];
        for y in 0..height {
            for x in 0..width {
                let at = (y * stride + x * 4) as usize;
                bytes[at] = x as u8;
                bytes[at + 1] = y as u8;
                bytes[at + 3] = 255;
            }
        }
        Image::new(
            bytes,
            width,
            height,
            stride,
            PixelFormat::Bgra8,
            ColorSpace::SRGB,
            Timestamp::ZERO,
        )
        .expect("a well-formed test image")
    }

    #[test]
    fn a_buffer_too_small_for_its_dimensions_is_refused_at_construction() {
        let err = Image::new(
            vec![0; 10],
            64,
            64,
            256,
            PixelFormat::Bgra8,
            ColorSpace::SRGB,
            Timestamp::ZERO,
        )
        .expect_err("10 bytes cannot hold a 64x64 frame");
        assert!(matches!(err, CaptureError::ShortBuffer { .. }));
    }

    #[test]
    fn an_rgb_buffer_tagged_with_a_video_matrix_is_refused() {
        let err = Image::new(
            vec![0; 64 * 64 * 4],
            64,
            64,
            256,
            PixelFormat::Bgra8,
            ColorSpace::BT709_VIDEO,
            Timestamp::ZERO,
        )
        .expect_err("BGRA is not luma and chroma");
        assert!(matches!(err, CaptureError::Unsupported { .. }));
    }

    #[test]
    fn cropping_takes_the_pixels_the_region_names() {
        let image = gradient(16, 16);
        let cropped = image
            .cropped(Rect::new(4, 6, 8, 4))
            .expect("the region is inside the frame");
        assert_eq!((cropped.width(), cropped.height()), (8, 4));
        let first = cropped.row(0).expect("row 0");
        assert_eq!(first[0], 4, "the crop starts at x=4");
        assert_eq!(first[1], 6, "the crop starts at y=6");
    }

    #[test]
    fn cropping_the_whole_frame_changes_nothing() {
        let image = gradient(16, 16);
        assert_eq!(image.cropped(image.rect()).expect("identity crop"), image);
    }

    #[test]
    fn a_crop_outside_the_frame_is_refused_rather_than_clamped_to_nothing() {
        let image = gradient(16, 16);
        assert!(image.cropped(Rect::new(100, 100, 8, 8)).is_err());
    }

    #[test]
    fn an_odd_crop_shrinks_to_even_rather_than_reading_past_the_frame() {
        let image = gradient(16, 16);
        let cropped = image.cropped(Rect::new(11, 11, 9, 9)).expect("overlaps");
        assert_eq!((cropped.width(), cropped.height()), (4, 4));
    }

    #[test]
    fn row_stops_at_the_last_row_rather_than_reading_padding() {
        let image = gradient(4, 4);
        assert!(image.row(3).is_some());
        assert!(image.row(4).is_none());
    }

    /// The bounds check matters only when something follows the last row. A
    /// packed image ends exactly there, so a slice range catches it either way;
    /// NV12's chroma plane is what a missing check would hand back as luma.
    #[test]
    fn a_row_past_the_last_one_does_not_return_the_chroma_plane() {
        let image = Image::new(
            vec![9; 24],
            4,
            4,
            4,
            PixelFormat::Nv12,
            ColorSpace::BT709_VIDEO,
            Timestamp::ZERO,
        )
        .expect("a 4x4 NV12 image");
        assert!(image.row(3).is_some());
        assert!(image.row(4).is_none());
    }

    #[test]
    fn padding_is_excluded_from_a_row() {
        let stride = 64;
        let image = Image::new(
            vec![7; (stride * 4) as usize],
            4,
            4,
            stride,
            PixelFormat::Bgra8,
            ColorSpace::SRGB,
            Timestamp::ZERO,
        )
        .expect("a padded image");
        assert_eq!(image.row(0).expect("row 0").len(), 16);
    }
}
