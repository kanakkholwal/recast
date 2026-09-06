use capturekit_core::{CursorButtons, Result};
use objc2_core_graphics::{CGEvent, CGEventSource, CGEventSourceStateID, CGMouseButton};

use crate::pointer::{Pointer, PointerSource};

/// Reads the pointer through CoreGraphics, the only way to place the cursor on macOS since ScreenCaptureKit attaches no pointer metadata.
/// Buttons come from HID state, a query rather than an event tap, so it needs no Input Monitoring grant.
pub(crate) struct CgPointer;

fn is_down(button: CGMouseButton) -> bool {
    CGEventSource::button_state(CGEventSourceStateID::HIDSystemState, button)
}

impl PointerSource for CgPointer {
    fn read(&mut self) -> Option<Pointer> {
        // A null-source event carries the pointer location, with no event tap.
        let event = CGEvent::new(None)?;
        let at = CGEvent::location(Some(&event));
        Some(Pointer {
            // Global display space, top-left origin, unlike NSEvent's bottom-left.
            position: (at.x as i32, at.y as i32),
            // No public API reports the hidden state; surface bounds still hide it.
            visible: true,
            buttons: CursorButtons {
                left: is_down(CGMouseButton::Left),
                right: is_down(CGMouseButton::Right),
                middle: is_down(CGMouseButton::Center),
            },
        })
    }
}

pub(crate) fn source() -> Result<Box<dyn PointerSource>> {
    Ok(Box::new(CgPointer))
}
