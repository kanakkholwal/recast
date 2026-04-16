use crate::cursor::CursorState;

/// Sample cursor position, visibility, and button state via Win32 APIs.
pub fn sample_cursor_state() -> Option<CursorState> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetCursorInfo, GetCursorPos, CURSORINFO, CURSOR_SHOWING,
    };

    let mut point = POINT::default();
    let mut info = CURSORINFO {
        cbSize: std::mem::size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };

    unsafe {
        if GetCursorPos(&mut point).is_err() {
            return None;
        }
        if GetCursorInfo(&mut info).is_err() {
            return None;
        }
    }

    Some(CursorState {
        x: point.x,
        y: point.y,
        visible: info.flags == CURSOR_SHOWING,
        left_down: unsafe { (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0 },
        right_down: unsafe { (GetAsyncKeyState(VK_RBUTTON.0 as i32) as u16 & 0x8000) != 0 },
    })
}
