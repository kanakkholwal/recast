use capturekit_core::{CursorButtons, Result};
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorInfo, GetCursorPos, CURSORINFO, CURSOR_SHOWING,
};

use super::dpi::PhysicalPixels;
use crate::pointer::{Pointer, PointerSource};

/// Reads the pointer through Win32.
///
/// `GetCursorPos` answers a DPI-unaware caller in logical points while the
/// capture is physical, so every read is taken inside a per-thread DPI scope,
/// the same one the display enumeration uses.
pub(crate) struct WinPointer;

/// Whether a virtual key is down right now.
///
/// The high bit of `GetAsyncKeyState` is the current state; the low bit means
/// "was pressed since the last call" and would report a click the user already
/// finished, so it is masked off.
fn is_down(key: i32) -> bool {
    // SAFETY: reads a virtual-key state; no preconditions, no pointers.
    (unsafe { GetAsyncKeyState(key) } as u16 & 0x8000) != 0
}

/// Buttons held right now.
///
/// Shared with the frame-attached cursor so the two paths cannot disagree about
/// a click, and so `Capabilities::cursor_buttons` is true of both.
pub(crate) fn buttons() -> CursorButtons {
    CursorButtons {
        // The mapped buttons, so a left-handed swap reads as the user set it.
        left: is_down(i32::from(VK_LBUTTON.0)),
        right: is_down(i32::from(VK_RBUTTON.0)),
        middle: is_down(i32::from(VK_MBUTTON.0)),
    }
}

impl PointerSource for WinPointer {
    fn read(&mut self) -> Option<Pointer> {
        let _dpi = PhysicalPixels::scope();
        let mut point = POINT::default();
        let mut info = CURSORINFO {
            cbSize: u32::try_from(core::mem::size_of::<CURSORINFO>()).ok()?,
            ..Default::default()
        };
        // SAFETY: both write into stack storage sized by the calls' contracts.
        unsafe {
            GetCursorPos(&mut point).ok()?;
            GetCursorInfo(&mut info).ok()?;
        }
        Some(Pointer {
            position: (point.x, point.y),
            visible: info.flags == CURSOR_SHOWING,
            buttons: buttons(),
        })
    }
}

pub(crate) fn source() -> Result<Box<dyn PointerSource>> {
    Ok(Box::new(WinPointer))
}
