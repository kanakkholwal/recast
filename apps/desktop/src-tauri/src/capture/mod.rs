mod shot;
mod source;
mod target;

use std::time::Duration;

use anyhow::Result;
use capturekit::Display;

pub use shot::{grab, grab_region, thumbnail};
pub use source::{create_capture_source, window_capture_supported};
pub use target::{CaptureArea, CaptureKind, CaptureTarget, RegionRect};

/// A source of screen frames, as raw BGRA8 at `width * 4` bytes per row.
///
/// One implementation, over capturekit. The trait is the seam the pacer's tests
/// script a source through, and it is where the contract below lives.
///
/// **Frames are `source`-sized, never pre-cropped.** The encoder is configured
/// for the full source and applies its own crop filter; a backend that cropped
/// first would crop twice.
pub trait CaptureSource: Send {
    /// The next frame, or `Ok(None)` if none arrived within `timeout`.
    ///
    /// A recoverable loss reopens the source and reports `Ok(None)`, so the
    /// recording repeats its last frame rather than ending.
    fn capture_next(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>>;

    /// Width of the captured frames in pixels.
    fn width(&self) -> u32;

    /// Height of the captured frames in pixels.
    fn height(&self) -> u32;
}

/// The display holding `point`, else the primary one, else the first listed.
///
/// One answer for the recorder, the screenshot commands and the picker, so a
/// window near a display edge cannot be assigned to different displays by each.
pub fn display_at(displays: &[Display], point: (i32, i32)) -> Option<&Display> {
    displays
        .iter()
        .find(|display| display.bounds.contains_point(point))
        .or_else(|| displays.iter().find(|display| display.is_primary))
        .or_else(|| displays.first())
}

#[cfg(test)]
mod tests {
    use super::*;
    use capturekit::{DisplayId, Rect};

    fn display(id: u64, bounds: (i32, i32, u32, u32), primary: bool) -> Display {
        Display {
            id: DisplayId(id),
            name: String::new(),
            bounds: Rect::new(bounds.0, bounds.1, bounds.2, bounds.3),
            scale_factor: 1.0,
            refresh_hz: None,
            is_primary: primary,
            rotation: capturekit::Rotation::None,
        }
    }

    fn desktop() -> Vec<Display> {
        vec![
            display(1, (0, 0, 1920, 1080), false),
            display(2, (1920, 0, 2560, 1440), true),
        ]
    }

    #[test]
    fn a_point_resolves_to_the_display_it_lands_on() {
        assert_eq!(display_at(&desktop(), (100, 100)).map(|d| d.id.0), Some(1));
        assert_eq!(display_at(&desktop(), (2000, 100)).map(|d| d.id.0), Some(2));
    }

    /// Adjacent displays share an edge; it belongs to the one on the right.
    #[test]
    fn a_point_on_the_shared_edge_belongs_to_one_display_only() {
        assert_eq!(display_at(&desktop(), (1920, 0)).map(|d| d.id.0), Some(2));
        assert_eq!(display_at(&desktop(), (1919, 0)).map(|d| d.id.0), Some(1));
    }

    /// A point off every display is a stale window position, not a reason to
    /// refuse to record.
    #[test]
    fn a_point_on_no_display_falls_back_to_the_primary_one() {
        assert_eq!(
            display_at(&desktop(), (-5000, -5000)).map(|d| d.id.0),
            Some(2)
        );
    }

    /// A desktop with no primary flagged still has to answer.
    #[test]
    fn a_desktop_with_no_primary_falls_back_to_the_first_listed() {
        let unflagged = [display(1, (0, 0, 800, 600), false)];
        assert_eq!(display_at(&unflagged, (-1, -1)).map(|d| d.id.0), Some(1));
        assert!(display_at(&[], (0, 0)).is_none());
    }
}
