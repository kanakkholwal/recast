mod shot;
mod source;
mod target;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use capturekit::{Display, GpuHandle, Rect};

pub use shot::{grab, grab_region, thumbnail};
pub use source::{create_capture_source, window_capture_supported, FrameMode};
pub use target::{CaptureArea, CaptureKind, CaptureTarget, RegionRect};

/// A source of screen frames, as raw BGRA8 at `width * 4` bytes per row.
///
/// One implementation, over capturekit. The trait is the seam the pacer's tests
/// script a source through, and it is where the contract below lives.
///
/// **Frames are `source`-sized, never pre-cropped.** The encoder is configured
/// for the full source and applies its own crop filter; a backend that cropped
/// first would crop twice.
/// Something about the capture the user has to be told, because a recording
/// that silently repeats its last frame looks like a working one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureNotice {
    /// Lost, and being retried. The recording repeats its last frame meanwhile.
    Interrupted(String),
    /// Gone, and not coming back on its own. Nothing more can be recorded.
    Ended(String),
    /// Frames are flowing again after an interruption.
    Resumed,
}

impl CaptureNotice {
    /// The sentence shown to the user.
    pub fn message(&self) -> &str {
        match self {
            Self::Interrupted(message) | Self::Ended(message) => message,
            Self::Resumed => "The screen is being recorded again.",
        }
    }

    /// Whether the recording can still produce new pixels after this.
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Ended(_))
    }
}

/// One captured frame, in whichever place the source was asked to leave it.
///
/// A source is opened in one mode for its whole life, so a consumer sees only
/// the variant it asked for.
#[derive(Clone)]
pub enum CapturedFrame {
    /// Packed BGRA8 at `width * 4` per row, ready for an encoder's stdin.
    Host(Arc<[u8]>),
    /// Left on the GPU. Nothing orders it: wait on the fence before sampling.
    ///
    /// Only the Windows writer reads the handle so far, so off Windows this is
    /// a variant the backends can produce and nothing yet consumes.
    #[cfg_attr(not(windows), allow(dead_code))]
    ///
    /// The backend reuses ONE shared texture, so this addresses whatever the
    /// source most recently copied there, not a snapshot. It must be consumed
    /// before the next `capture_next`, which rules out queueing it.
    Gpu(GpuHandle),
}

pub trait CaptureSource: Send {
    /// The next frame, or `Ok(None)` if none arrived within `timeout`.
    ///
    /// A recoverable loss reopens the source and reports `Ok(None)`, so the
    /// recording repeats its last frame rather than ending.
    fn capture_next(&mut self, timeout: Duration) -> Result<Option<CapturedFrame>>;

    /// Anything the user needs telling since the last call, taken once.
    ///
    /// Separate from `capture_next` because a loss is reported on the tick it
    /// happens while frames keep being asked for, and because the recorder
    /// forwards these to the UI rather than acting on them itself.
    fn take_notice(&mut self) -> Option<CaptureNotice> {
        None
    }

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

/// The rectangle covering every display, in physical virtual-desktop pixels.
///
/// What a full-screen overlay has to span. Sizing it to the primary display
/// instead leaves the other monitors unselectable, and on a layout with a
/// monitor above or left of the primary the origin is negative, so it cannot be
/// assumed to be (0, 0) either.
pub fn virtual_bounds(displays: &[Display]) -> Option<Rect> {
    displays
        .iter()
        .map(|display| display.bounds)
        .reduce(|all, bounds| all.union(&bounds))
}

#[cfg(test)]
mod tests {
    use super::*;
    use capturekit::DisplayId;

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

    #[test]
    fn the_virtual_bounds_cover_every_display() {
        let bounds = virtual_bounds(&desktop()).expect("two displays");
        assert_eq!(bounds, Rect::new(0, 0, 4480, 1440));
    }

    /// A monitor placed above or to the left of the primary puts the origin in
    /// negative space; an overlay pinned to (0, 0) misses it entirely.
    #[test]
    fn the_virtual_bounds_start_at_the_topmost_leftmost_display() {
        let spread = [
            display(1, (0, 0, 1920, 1080), true),
            display(2, (-1280, -300, 1280, 1024), false),
        ];
        let bounds = virtual_bounds(&spread).expect("two displays");
        assert_eq!(bounds, Rect::new(-1280, -300, 3200, 1380));
    }

    #[test]
    fn a_desktop_with_no_displays_has_no_bounds() {
        assert_eq!(virtual_bounds(&[]), None);
    }

    /// A desktop with no primary flagged still has to answer.
    #[test]
    fn a_desktop_with_no_primary_falls_back_to_the_first_listed() {
        let unflagged = [display(1, (0, 0, 800, 600), false)];
        assert_eq!(display_at(&unflagged, (-1, -1)).map(|d| d.id.0), Some(1));
        assert!(display_at(&[], (0, 0)).is_none());
    }
}
