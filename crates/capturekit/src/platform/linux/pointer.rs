use capturekit_core::{CaptureError, CursorButtons, Result};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, KeyButMask, Screen};
use x11rb::rust_connection::RustConnection;

use crate::pointer::{Pointer, PointerSource};

const BACKEND: &str = "x11-pointer";

/// Read the buttons out of a `QueryPointer` modifier mask.
/// Shared with the frame-attached cursor so the two paths cannot disagree, and so `Capabilities::cursor_buttons` is true of both.
pub(crate) fn buttons_of(mask: KeyButMask) -> CursorButtons {
    CursorButtons {
        left: mask.contains(KeyButMask::BUTTON1),
        middle: mask.contains(KeyButMask::BUTTON2),
        right: mask.contains(KeyButMask::BUTTON3),
    }
}

/// Reads the pointer through `XQueryPointer`.
/// One connection is held open for the life of the reader: reconnecting per sample would dominate the cost at any useful rate.
pub(crate) struct X11Pointer {
    conn: RustConnection,
    root: Screen,
}

impl PointerSource for X11Pointer {
    fn read(&mut self) -> Option<Pointer> {
        let reply = self.conn.query_pointer(self.root.root).ok()?.reply().ok()?;
        // Another X screen entirely, where root_x/root_y carry no meaning.
        if !reply.same_screen {
            return None;
        }
        Some(Pointer {
            position: (i32::from(reply.root_x), i32::from(reply.root_y)),
            // No hidden-state query exists; XFixes reports a zero-sized image instead.
            visible: true,
            buttons: buttons_of(reply.mask),
        })
    }
}

pub(crate) fn source() -> Result<Box<dyn PointerSource>> {
    let (conn, index) = x11rb::connect(None).map_err(|error| {
        CaptureError::backend(BACKEND, std::io::Error::other(error.to_string()))
    })?;
    let root = conn
        .setup()
        .roots
        .get(index)
        .cloned()
        .ok_or(CaptureError::NotFound {
            kind: "screen",
            id: index as u64,
        })?;
    Ok(Box::new(X11Pointer { conn, root }))
}

/// Wayland gives no client the global pointer position or button state, by
/// design: a client only learns about the pointer over its own surfaces. The
/// portal's PipeWire stream can carry cursor metadata for the captured surface,
/// but never buttons.
pub(crate) fn unavailable() -> CaptureError {
    CaptureError::Unsupported {
        backend: "wayland",
        operation: "read the pointer; Wayland reports it to no client outside its own surfaces",
    }
}
