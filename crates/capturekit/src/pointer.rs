//! Reads the OS pointer on its own schedule so a consumer can sample faster than it captures.
//! A frame-attached cursor samples at the frame rate and no backend attaches button state to a frame at all.

use capturekit_core::{
    point_in_surface, point_offset_in_surface, CursorButtons, CursorSample, Rect, Result, Timestamp,
};

/// One raw pointer read, in virtual-desktop coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pointer {
    /// Where the pointer is, in the OS's own desktop coordinate space.
    pub position: (i32, i32),
    /// Whether the OS is drawing a cursor at all.
    pub visible: bool,
    /// Buttons held, or `NONE` from a platform that cannot report them.
    pub buttons: CursorButtons,
}

/// One pointer sample placed against a surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerSample {
    /// The cursor, whose `position` is `None` once it leaves the surface.
    pub cursor: CursorSample,
    /// Surface-relative position, unclamped, so a caller can still tell a
    /// pointer moving off the surface from one parked beside it.
    pub offset: (i32, i32),
}

/// A per-OS pointer reader, held open so its display connection is not
/// reopened per sample.
pub(crate) trait PointerSource: Send {
    /// The pointer now, or `None` when the OS refused to say.
    fn read(&mut self) -> Option<Pointer>;
}

/// Reads the pointer and places it inside one captured surface.
///
/// Owns the coordinate mapping so every consumer does not redo it: a pointer
/// outside the surface comes back as an absent sample rather than a clamped
/// one, which is what lets a renderer hide the cursor instead of parking it on
/// an edge.
pub struct PointerCapturer {
    source: Box<dyn PointerSource>,
    surface: Rect,
    scale: f64,
    /// Last buttons seen, so a read the OS refuses does not fabricate a release.
    last_buttons: CursorButtons,
}

impl PointerCapturer {
    /// Open the platform pointer reader for a surface at `surface`, whose pixels are `scale` times the units the OS reports the pointer in (1.0 everywhere but a scaled macOS display).
    pub fn open(surface: Rect, scale: f64) -> Result<Self> {
        Ok(Self {
            source: crate::platform::pointer_source()?,
            surface,
            scale,
            last_buttons: CursorButtons::NONE,
        })
    }

    /// Where the surface sits, so a caller that moves the capture can follow.
    pub fn set_surface(&mut self, surface: Rect) {
        self.surface = surface;
    }

    /// Sample the pointer, stamped at `pts`.
    ///
    /// `None` only when the OS refused the read, which is rare and transient
    /// (a UAC or secure-desktop switch on Windows); a pointer that is merely
    /// off the surface is `Some` with no position.
    pub fn sample(&mut self, pts: Timestamp) -> Option<PointerSample> {
        let read = self.source.read()?;
        self.last_buttons = read.buttons;
        let offset = point_offset_in_surface(read.position, &self.surface, self.scale);
        let position = point_in_surface(read.position, &self.surface, self.scale);
        Some(PointerSample {
            cursor: CursorSample {
                pts,
                position,
                visible: read.visible && position.is_some(),
                buttons: read.buttons,
                shape_id: 0,
            },
            offset,
        })
    }

    /// The buttons from the most recent successful read.
    /// A caller filling a gap where [`sample`](Self::sample) returned `None` uses this rather than `NONE`, so a click held across the gap is not reported as released and pressed again.
    #[must_use]
    pub const fn last_buttons(&self) -> CursorButtons {
        self.last_buttons
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scripted source, so the mapping and the gap handling are testable with
    /// no pointer and no display.
    struct Scripted(Vec<Option<Pointer>>);

    impl PointerSource for Scripted {
        fn read(&mut self) -> Option<Pointer> {
            if self.0.is_empty() {
                return None;
            }
            self.0.remove(0)
        }
    }

    const SURFACE: Rect = Rect {
        x: 100,
        y: 100,
        width: 800,
        height: 600,
    };

    fn capturer(reads: Vec<Option<Pointer>>) -> PointerCapturer {
        PointerCapturer {
            source: Box::new(Scripted(reads)),
            surface: SURFACE,
            scale: 1.0,
            last_buttons: CursorButtons::NONE,
        }
    }

    fn at(x: i32, y: i32) -> Pointer {
        Pointer {
            position: (x, y),
            visible: true,
            buttons: CursorButtons::NONE,
        }
    }

    #[test]
    fn a_pointer_on_the_surface_is_placed_relative_to_it() {
        let mut cap = capturer(vec![Some(at(150, 200))]);
        let sample = cap.sample(Timestamp::ZERO).expect("a read").cursor;
        assert_eq!(sample.position, Some((50, 100)));
    }

    /// Off the surface is a sample with no position, NOT a refused read: the
    /// two mean different things to a caller filling gaps.
    #[test]
    fn a_pointer_off_the_surface_is_a_sample_with_no_position() {
        let mut cap = capturer(vec![Some(at(10, 10))]);
        let sample = cap.sample(Timestamp::ZERO).expect("a read").cursor;
        assert_eq!(sample.position, None);
    }

    /// A visible cursor that has left the surface must not draw at its edge.
    #[test]
    fn a_pointer_off_the_surface_is_not_visible_even_when_the_os_draws_it() {
        let mut cap = capturer(vec![Some(at(10, 10))]);
        let sample = cap.sample(Timestamp::ZERO).expect("a read").cursor;
        assert!(!sample.visible);
    }

    #[test]
    fn a_hidden_cursor_on_the_surface_is_not_visible() {
        let mut cap = capturer(vec![Some(Pointer {
            visible: false,
            ..at(150, 200)
        })]);
        let sample = cap.sample(Timestamp::ZERO).expect("a read").cursor;
        assert!(!sample.visible);
        assert_eq!(sample.position, Some((50, 100)));
    }

    /// A pointer off the surface still has to report where it went: collapsed
    /// to one place it reads as stationary, which invents an idle period.
    #[test]
    fn a_pointer_off_the_surface_still_reports_where_it_moved() {
        let mut cap = capturer(vec![Some(at(10, 10)), Some(at(40, 10))]);
        let first = cap.sample(Timestamp::ZERO).expect("a read");
        let second = cap.sample(Timestamp::ZERO).expect("a read");
        assert_eq!((first.offset, second.offset), ((-90, -90), (-60, -90)));
        assert!(first.cursor.position.is_none() && second.cursor.position.is_none());
    }

    #[test]
    fn buttons_ride_along_with_the_sample() {
        let held = CursorButtons {
            left: true,
            ..CursorButtons::NONE
        };
        let mut cap = capturer(vec![Some(Pointer {
            buttons: held,
            ..at(150, 200)
        })]);
        assert_eq!(
            cap.sample(Timestamp::ZERO).expect("a read").cursor.buttons,
            held
        );
    }

    /// A refused read must not look like a release, or the gap becomes a
    /// spurious mouse-up and a second mouse-down when the OS answers again.
    #[test]
    fn a_refused_read_keeps_the_buttons_it_last_saw() {
        let held = CursorButtons {
            left: true,
            ..CursorButtons::NONE
        };
        let mut cap = capturer(vec![
            Some(Pointer {
                buttons: held,
                ..at(150, 200)
            }),
            None,
        ]);
        cap.sample(Timestamp::ZERO);
        assert!(cap.sample(Timestamp::ZERO).is_none());
        assert_eq!(cap.last_buttons(), held);
    }

    #[test]
    fn a_moved_surface_maps_against_the_new_origin() {
        let mut cap = capturer(vec![Some(at(150, 200)), Some(at(150, 200))]);
        cap.sample(Timestamp::ZERO);
        cap.set_surface(Rect {
            x: 140,
            y: 190,
            ..SURFACE
        });
        let sample = cap.sample(Timestamp::ZERO).expect("a read").cursor;
        assert_eq!(sample.position, Some((10, 10)));
    }
}
