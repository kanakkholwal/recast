use crate::error::CaptureError;
use crate::time::Timestamp;

/// How a cursor image is stored, which differs per platform and per cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[non_exhaustive]
pub enum CursorShapeKind {
    /// Straight 32-bit BGRA with a real alpha channel.
    Color,
    /// Two stacked 1-bit masks, AND then XOR, each row padded to the stride.
    /// The image is half `height`, because both masks are counted in it.
    Monochrome,
    /// 32-bit BGRA whose alpha byte is a flag, not coverage: `0` copies the
    /// pixel, `0xFF` inverts what is underneath.
    MaskedColor,
}

/// A cursor image, in whatever form the platform delivered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorShape {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels. For [`CursorShapeKind::Monochrome`] this counts BOTH
    /// stacked masks, so the drawn cursor is half this tall.
    pub height: u32,
    /// Bytes between rows.
    pub stride: u32,
    /// Where the click lands, relative to the image's top-left.
    pub hotspot_x: u32,
    /// Where the click lands, relative to the image's top-left.
    pub hotspot_y: u32,
    /// How the bytes are stored.
    pub kind: CursorShapeKind,
    /// The raw image, as the platform gave it.
    pub bytes: Vec<u8>,
}

impl CursorShape {
    /// The height of the cursor as drawn, which is not `height` for a monochrome
    /// shape because that counts both stacked masks.
    #[must_use]
    pub const fn drawn_height(&self) -> u32 {
        match self.kind {
            CursorShapeKind::Monochrome => self.height / 2,
            _ => self.height,
        }
    }

    /// Decode to straight (non-premultiplied) RGBA.
    ///
    /// **Inverting pixels cannot be represented.** Monochrome and masked-colour
    /// cursors can ask to invert whatever is underneath, which needs the
    /// destination this function does not have. Those pixels come back opaque
    /// black, which is what they look like over a light background and is the
    /// same choice every screenshot tool makes. A compositor that wants the real
    /// thing has `bytes` and `kind` to do it properly.
    pub fn to_rgba(&self) -> Result<Vec<u8>, CaptureError> {
        match self.kind {
            CursorShapeKind::Color => self.decode_bgra(false),
            CursorShapeKind::MaskedColor => self.decode_bgra(true),
            CursorShapeKind::Monochrome => self.decode_monochrome(),
        }
    }

    fn require(&self, needed: usize) -> Result<(), CaptureError> {
        if self.bytes.len() < needed {
            return Err(CaptureError::ShortCursorShape {
                kind: self.kind,
                needed,
                got: self.bytes.len(),
            });
        }
        Ok(())
    }

    fn decode_bgra(&self, masked: bool) -> Result<Vec<u8>, CaptureError> {
        let stride = self.stride as usize;
        let width = self.width as usize;
        let height = self.height as usize;
        self.require(stride * height.saturating_sub(1) + width * 4)?;

        let mut rgba = vec![0u8; width * height * 4];
        for y in 0..height {
            for x in 0..width {
                let at = y * stride + x * 4;
                let out = (y * width + x) * 4;
                let (b, g, r, a) = (
                    self.bytes[at],
                    self.bytes[at + 1],
                    self.bytes[at + 2],
                    self.bytes[at + 3],
                );
                if masked {
                    // Alpha is a flag here: 0 copies, 0xFF inverts.
                    let inverts = a != 0;
                    rgba[out] = if inverts { 0 } else { r };
                    rgba[out + 1] = if inverts { 0 } else { g };
                    rgba[out + 2] = if inverts { 0 } else { b };
                    rgba[out + 3] = 255;
                } else {
                    rgba[out] = r;
                    rgba[out + 1] = g;
                    rgba[out + 2] = b;
                    rgba[out + 3] = a;
                }
            }
        }
        Ok(rgba)
    }

    /// Decode the stacked AND/XOR masks.
    ///
    /// Per pixel, reading the AND mask then the XOR mask:
    /// `0,0` opaque black, `0,1` opaque white, `1,0` transparent, `1,1` inverts.
    fn decode_monochrome(&self) -> Result<Vec<u8>, CaptureError> {
        let stride = self.stride as usize;
        let width = self.width as usize;
        let height = self.drawn_height() as usize;
        // The XOR mask starts where the AND mask ends, so both must be present.
        self.require(stride * height * 2)?;

        let mut rgba = vec![0u8; width * height * 4];
        for y in 0..height {
            for x in 0..width {
                // Bits run most-significant first within each byte.
                let byte = x / 8;
                let bit = 7 - (x % 8);
                let and = (self.bytes[y * stride + byte] >> bit) & 1;
                let xor = (self.bytes[(height + y) * stride + byte] >> bit) & 1;
                let out = (y * width + x) * 4;
                let (value, alpha) = match (and, xor) {
                    (0, 0) => (0, 255),
                    (0, _) => (255, 255),
                    (_, 0) => (0, 0),
                    // Inverting, which needs the destination. See `to_rgba`.
                    (_, _) => (0, 255),
                };
                rgba[out] = value;
                rgba[out + 1] = value;
                rgba[out + 2] = value;
                rgba[out + 3] = alpha;
            }
        }
        Ok(rgba)
    }
}

/// Where the cursor was, and whether it was showing, at one instant.
///
/// Sampled by the capture backend on the same clock as the frames rather than by
/// a separate poller: a cursor track on its own clock drifts against the video
/// and has to be smoothed to hide it.
#[derive(Debug, Clone, PartialEq)]
pub struct CursorSample {
    /// When the position was true, on the source's clock.
    pub pts: Timestamp,
    /// Position in the captured surface's own pixels, or `None` when the cursor
    /// is outside it.
    pub position: Option<(i32, i32)>,
    /// Whether the cursor was being drawn at all.
    pub visible: bool,
    /// Identifies the shape, so a consumer can cache decoded images instead of
    /// decoding one per frame. Changes whenever the image does.
    pub shape_id: u64,
}

impl CursorSample {
    /// A sample saying the cursor is not on this surface.
    #[must_use]
    pub const fn absent(pts: Timestamp) -> Self {
        Self {
            pts,
            position: None,
            visible: false,
            shape_id: 0,
        }
    }

    /// Whether this sample should draw anything.
    #[must_use]
    pub const fn is_drawable(&self) -> bool {
        self.visible && self.position.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 8x1 monochrome cursor exercising all four mask combinations, with the
    /// XOR mask stacked under the AND mask as Windows delivers it.
    fn four_cases() -> CursorShape {
        // AND bits: 0,0,1,1 then padding. XOR bits: 0,1,0,1 then padding.
        CursorShape {
            width: 4,
            height: 2,
            stride: 1,
            hotspot_x: 0,
            hotspot_y: 0,
            kind: CursorShapeKind::Monochrome,
            bytes: vec![0b0011_0000, 0b0101_0000],
        }
    }

    #[test]
    fn a_monochrome_cursor_decodes_all_four_mask_cases() {
        let rgba = four_cases().to_rgba().expect("a well-formed shape");
        let pixel = |i: usize| &rgba[i * 4..i * 4 + 4];
        assert_eq!(pixel(0), [0, 0, 0, 255], "and=0 xor=0 is opaque black");
        assert_eq!(
            pixel(1),
            [255, 255, 255, 255],
            "and=0 xor=1 is opaque white"
        );
        assert_eq!(pixel(2), [0, 0, 0, 0], "and=1 xor=0 is transparent");
        assert_eq!(
            pixel(3),
            [0, 0, 0, 255],
            "and=1 xor=1 inverts, drawn as black"
        );
    }

    #[test]
    fn a_monochrome_shape_is_half_as_tall_as_its_two_masks() {
        assert_eq!(four_cases().drawn_height(), 1);
        assert_eq!(four_cases().to_rgba().expect("decoded").len(), 4 * 4);
    }

    #[test]
    fn a_colour_cursor_keeps_its_alpha_and_swaps_to_rgba() {
        let shape = CursorShape {
            width: 1,
            height: 1,
            stride: 4,
            hotspot_x: 0,
            hotspot_y: 0,
            kind: CursorShapeKind::Color,
            // BGRA in, RGBA out.
            bytes: vec![10, 20, 30, 128],
        };
        assert_eq!(shape.to_rgba().expect("decoded"), vec![30, 20, 10, 128]);
    }

    #[test]
    fn a_masked_colour_cursor_copies_where_the_flag_is_clear() {
        let shape = CursorShape {
            width: 2,
            height: 1,
            stride: 8,
            hotspot_x: 0,
            hotspot_y: 0,
            kind: CursorShapeKind::MaskedColor,
            // First pixel copies, second inverts.
            bytes: vec![10, 20, 30, 0, 40, 50, 60, 0xFF],
        };
        let rgba = shape.to_rgba().expect("decoded");
        assert_eq!(
            &rgba[0..4],
            [30, 20, 10, 255],
            "flag clear copies the colour"
        );
        assert_eq!(
            &rgba[4..8],
            [0, 0, 0, 255],
            "flag set inverts, drawn as black"
        );
    }

    #[test]
    fn padding_between_rows_is_skipped() {
        let shape = CursorShape {
            width: 1,
            height: 2,
            stride: 16,
            hotspot_x: 0,
            hotspot_y: 0,
            kind: CursorShapeKind::Color,
            bytes: {
                let mut bytes = vec![0u8; 16 + 4];
                bytes[0..4].copy_from_slice(&[1, 2, 3, 255]);
                bytes[16..20].copy_from_slice(&[4, 5, 6, 255]);
                bytes
            },
        };
        let rgba = shape.to_rgba().expect("decoded");
        assert_eq!(&rgba[0..4], [3, 2, 1, 255]);
        assert_eq!(&rgba[4..8], [6, 5, 4, 255]);
    }

    #[test]
    fn a_shape_shorter_than_its_masks_is_refused_rather_than_read_past() {
        let mut shape = four_cases();
        shape.bytes.pop();
        assert!(matches!(
            shape.to_rgba(),
            Err(CaptureError::ShortCursorShape { .. })
        ));
    }

    #[test]
    fn an_absent_cursor_draws_nothing() {
        let sample = CursorSample::absent(Timestamp::ZERO);
        assert!(!sample.is_drawable());
    }

    #[test]
    fn a_visible_cursor_with_no_position_still_draws_nothing() {
        let sample = CursorSample {
            pts: Timestamp::ZERO,
            position: None,
            visible: true,
            shape_id: 7,
        };
        assert!(!sample.is_drawable());
    }

    /// The case both other tests miss: they have no position, so dropping the
    /// visibility check entirely still passes them.
    #[test]
    fn a_positioned_but_hidden_cursor_draws_nothing() {
        let sample = CursorSample {
            pts: Timestamp::ZERO,
            position: Some((10, 20)),
            visible: false,
            shape_id: 7,
        };
        assert!(!sample.is_drawable());
    }

    #[test]
    fn a_visible_positioned_cursor_draws() {
        let sample = CursorSample {
            pts: Timestamp::ZERO,
            position: Some((10, 20)),
            visible: true,
            shape_id: 7,
        };
        assert!(sample.is_drawable());
    }
}
