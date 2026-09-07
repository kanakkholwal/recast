use crate::error::CaptureError;
use crate::geom::Rect;
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
    /// 32-bit BGRA whose colour channels are already multiplied by alpha, which is what X11's XFixes hands over.
    /// Drawing these as if they were straight alpha darkens every soft edge, which on a cursor is a visible black fringe.
    PremultipliedColor,
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

/// Divide colour back out of alpha, in place.
/// A fully transparent pixel has no colour left to recover, and its colour cannot affect anything drawn, so it stays zero.
fn unpremultiply(rgba: &mut [u8]) {
    for pixel in rgba.chunks_exact_mut(4) {
        let alpha = u32::from(pixel[3]);
        if alpha == 0 || alpha == 255 {
            continue;
        }
        for channel in &mut pixel[..3] {
            *channel = u8::try_from((u32::from(*channel) * 255 / alpha).min(255)).unwrap_or(255);
        }
    }
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
    /// Inverting pixels cannot be represented without the destination, so they come back opaque black, as every screenshot tool does; `bytes` and `kind` allow doing it properly.
    pub fn to_rgba(&self) -> Result<Vec<u8>, CaptureError> {
        match self.kind {
            CursorShapeKind::Color => self.decode_bgra(false),
            CursorShapeKind::PremultipliedColor => {
                let mut rgba = self.decode_bgra(false)?;
                unpremultiply(&mut rgba);
                Ok(rgba)
            }
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
    /// Per pixel, reading the AND mask then the XOR mask: `0,0` opaque black, `0,1` opaque white, `1,0` transparent, `1,1` inverts.
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

/// Where a virtual-desktop pointer read lands inside a captured surface, or `None` when outside; `scale` lifts logical points into physical pixels for macOS.
/// `None` rather than a clamp is what lets a consumer hide the cursor leaving the recorded area instead of pinning it to an edge for the rest of the video.
#[must_use]
pub fn point_in_surface(point: (i32, i32), surface: &Rect, scale: f64) -> Option<(i32, i32)> {
    let (x, y) = point_offset_in_surface(point, surface, scale);
    // try_from rejects a pointer left of or above the surface, so this is both bounds.
    let inside = u32::try_from(x).is_ok_and(|x| x < surface.width)
        && u32::try_from(y).is_ok_and(|y| y < surface.height);
    inside.then_some((x, y))
}

/// Where a pointer sits relative to a surface, on it or not: the unclamped half of [`point_in_surface`].
/// A caller tracking movement needs this, since an off-surface pointer collapsed to one position reads as stationary and turns moving away into a detected idle period.
#[must_use]
pub fn point_offset_in_surface(point: (i32, i32), surface: &Rect, scale: f64) -> (i32, i32) {
    let lift = |v: i32| (f64::from(v) * scale).round() as i32;
    (lift(point.0) - surface.x, lift(point.1) - surface.y)
}

/// Which mouse buttons were held at a sample; a property of the mouse, not the surface, so they stay true while a drag leaves it.
/// Backends that cannot read them report [`CursorButtons::NONE`] and clear the capability, so a consumer tells "no button" from "cannot know".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct CursorButtons {
    /// Primary button, whichever physical button the user has mapped to it.
    pub left: bool,
    /// Secondary (context menu) button.
    pub right: bool,
    /// Wheel click.
    pub middle: bool,
}

impl CursorButtons {
    /// Nothing held, which is also what a backend without button support says.
    pub const NONE: Self = Self {
        left: false,
        right: false,
        middle: false,
    };

    /// Whether any button is held, which is what makes a move a drag.
    #[must_use]
    pub const fn any(&self) -> bool {
        self.left || self.right || self.middle
    }
}

/// Where the cursor was, which buttons were held, and whether it was showing.
/// Sampled on the capture's own clock rather than by a separate poller: a cursor track on its own clock drifts against the video and has to be smoothed to hide it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorSample {
    /// When the position was true, on the source's clock.
    pub pts: Timestamp,
    /// Position in the captured surface's own pixels, or `None` when the cursor
    /// is outside it.
    pub position: Option<(i32, i32)>,
    /// Whether the cursor was being drawn at all.
    pub visible: bool,
    /// Buttons held at `pts`, or `NONE` from a backend that cannot read them.
    pub buttons: CursorButtons,
    /// Identifies the shape, so a consumer can cache decoded images instead of
    /// decoding one per frame. Changes whenever the image does.
    pub shape_id: u64,
}

impl CursorSample {
    /// A sample saying the cursor is not on this surface, with the buttons that
    /// were still held. Takes them rather than assuming none: a drag that leaves
    /// the surface would otherwise read as a button release and a fresh press
    /// when it comes back.
    #[must_use]
    pub const fn absent(pts: Timestamp, buttons: CursorButtons) -> Self {
        Self {
            pts,
            position: None,
            visible: false,
            buttons,
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

    /// Premultiplied colour drawn as straight alpha is a black fringe on every
    /// soft edge, which is what a cursor is mostly made of.
    #[test]
    fn a_premultiplied_cursor_has_its_colour_divided_back_out() {
        let shape = CursorShape {
            width: 1,
            height: 1,
            stride: 4,
            hotspot_x: 0,
            hotspot_y: 0,
            kind: CursorShapeKind::PremultipliedColor,
            // Half-transparent white, stored premultiplied: 128 in every channel.
            bytes: vec![128, 128, 128, 128],
        };
        let rgba = shape.to_rgba().expect("decoded");
        assert_eq!(rgba[3], 128, "alpha must not be touched");
        for channel in &rgba[..3] {
            assert!(*channel > 250, "{rgba:?} was left premultiplied");
        }
    }

    #[test]
    fn an_opaque_premultiplied_pixel_is_left_exactly_alone() {
        let shape = CursorShape {
            width: 1,
            height: 1,
            stride: 4,
            hotspot_x: 0,
            hotspot_y: 0,
            kind: CursorShapeKind::PremultipliedColor,
            bytes: vec![10, 20, 30, 255],
        };
        assert_eq!(shape.to_rgba().expect("decoded"), vec![30, 20, 10, 255]);
    }

    /// Nothing to divide by, and nothing the colour could affect.
    #[test]
    fn a_fully_transparent_premultiplied_pixel_stays_transparent() {
        let shape = CursorShape {
            width: 1,
            height: 1,
            stride: 4,
            hotspot_x: 0,
            hotspot_y: 0,
            kind: CursorShapeKind::PremultipliedColor,
            bytes: vec![0, 0, 0, 0],
        };
        assert_eq!(shape.to_rgba().expect("decoded"), vec![0, 0, 0, 0]);
    }

    /// A channel brighter than its alpha is a malformed premultiplied pixel;
    /// dividing it out overflows a byte and must clamp rather than wrap.
    #[test]
    fn a_channel_brighter_than_its_alpha_clamps_instead_of_wrapping() {
        let shape = CursorShape {
            width: 1,
            height: 1,
            stride: 4,
            hotspot_x: 0,
            hotspot_y: 0,
            kind: CursorShapeKind::PremultipliedColor,
            bytes: vec![200, 200, 200, 100],
        };
        let rgba = shape.to_rgba().expect("decoded");
        assert_eq!(&rgba[..3], &[255, 255, 255]);
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
        let sample = CursorSample::absent(Timestamp::ZERO, CursorButtons::NONE);
        assert!(!sample.is_drawable());
    }

    const SURFACE: Rect = Rect {
        x: 1920,
        y: 0,
        width: 1280,
        height: 720,
    };

    #[test]
    fn a_pointer_inside_the_surface_maps_relative_to_its_origin() {
        assert_eq!(point_in_surface((1930, 10), &SURFACE, 1.0), Some((10, 10)));
    }

    /// A second monitor puts the surface origin far from zero, which is the case
    /// that made every sample land outside the frame before this existed.
    #[test]
    fn a_pointer_on_another_monitor_is_outside() {
        assert_eq!(point_in_surface((100, 10), &SURFACE, 1.0), None);
    }

    #[test]
    fn the_far_edge_is_outside_because_the_last_pixel_is_width_minus_one() {
        assert_eq!(point_in_surface((1920 + 1280, 10), &SURFACE, 1.0), None);
        assert_eq!(
            point_in_surface((1920 + 1279, 719), &SURFACE, 1.0),
            Some((1279, 719))
        );
    }

    /// Only the vertical bound is violated here, which is what an x-only pair of
    /// cases lets a missing height check slip through.
    #[test]
    fn a_pointer_below_the_surface_is_outside() {
        assert_eq!(point_in_surface((1930, 720), &SURFACE, 1.0), None);
    }

    #[test]
    fn a_pointer_above_the_surface_is_outside() {
        assert_eq!(point_in_surface((1930, -1), &SURFACE, 1.0), None);
    }

    /// macOS reports the pointer in logical points against a physical surface.
    #[test]
    fn a_logical_pointer_is_lifted_into_physical_pixels_before_the_origin() {
        let surface = Rect {
            x: 0,
            y: 0,
            width: 2560,
            height: 1440,
        };
        assert_eq!(
            point_in_surface((640, 360), &surface, 2.0),
            Some((1280, 720))
        );
    }

    /// A monitor left of or above the primary has a negative origin, so the
    /// subtraction has to move the point up, not down.
    #[test]
    fn a_surface_at_a_negative_origin_maps_a_negative_pointer_inside() {
        let surface = Rect {
            x: -1920,
            y: -100,
            width: 1920,
            height: 1080,
        };
        assert_eq!(
            point_in_surface((-1910, -90), &surface, 1.0),
            Some((10, 10))
        );
    }

    /// The reason `absent` takes buttons: a drag that leaves the surface would
    /// otherwise read as a release, and its return as a fresh press.
    #[test]
    fn a_cursor_that_drags_off_the_surface_keeps_its_buttons() {
        let held = CursorButtons {
            left: true,
            ..CursorButtons::NONE
        };
        let sample = CursorSample::absent(Timestamp::ZERO, held);
        assert_eq!(sample.buttons, held);
    }

    #[test]
    fn no_buttons_held_is_not_a_drag() {
        assert!(!CursorButtons::NONE.any());
    }

    #[test]
    fn any_single_button_makes_a_drag() {
        for held in [
            CursorButtons {
                left: true,
                ..CursorButtons::NONE
            },
            CursorButtons {
                right: true,
                ..CursorButtons::NONE
            },
            CursorButtons {
                middle: true,
                ..CursorButtons::NONE
            },
        ] {
            assert!(held.any(), "{held:?} should count as held");
        }
    }

    #[test]
    fn a_visible_cursor_with_no_position_still_draws_nothing() {
        let sample = CursorSample {
            pts: Timestamp::ZERO,
            position: None,
            visible: true,
            buttons: CursorButtons::NONE,
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
            buttons: CursorButtons::NONE,
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
            buttons: CursorButtons::NONE,
            shape_id: 7,
        };
        assert!(sample.is_drawable());
    }
}
