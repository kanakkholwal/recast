use capturekit_core::{Display, DisplayId, Rect, Result, Rotation, Window, WindowId};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, EnumDisplaySettingsW, GetMonitorInfoW, MonitorFromWindow, DEVMODEW,
    DEVMODE_DISPLAY_ORIENTATION, DMDO_180, DMDO_270, DMDO_90, ENUM_CURRENT_SETTINGS, HDC, HMONITOR,
    MONITORINFOEXW, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowLongW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
};

use crate::platform::windows::d3d;
use crate::platform::windows::dpi::PhysicalPixels;

/// The reference DPI a scale factor of 1.0 corresponds to.
const BASELINE_DPI: f32 = 96.0;

/// `MONITORINFOF_PRIMARY`, which the metadata-only Gdi module does not export.
const MONITORINFOF_PRIMARY: u32 = 1;

fn rect_of(rect: RECT) -> Rect {
    Rect::new(
        rect.left,
        rect.top,
        (rect.right - rect.left).max(0) as u32,
        (rect.bottom - rect.top).max(0) as u32,
    )
}

fn rotation_of(orientation: DEVMODE_DISPLAY_ORIENTATION) -> Rotation {
    match orientation {
        DMDO_90 => Rotation::Cw90,
        DMDO_180 => Rotation::Cw180,
        DMDO_270 => Rotation::Cw270,
        _ => Rotation::None,
    }
}

fn wide_to_string(chars: &[u16]) -> String {
    let end = chars.iter().position(|c| *c == 0).unwrap_or(chars.len());
    String::from_utf16_lossy(&chars[..end])
}

unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _hdc: HDC,
    _clip: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let monitors = &mut *(data.0 as *mut Vec<HMONITOR>);
    monitors.push(monitor);
    TRUE
}

unsafe extern "system" fn collect_window(window: HWND, data: LPARAM) -> BOOL {
    let windows = &mut *(data.0 as *mut Vec<HWND>);
    windows.push(window);
    TRUE
}

/// Refresh rate and orientation, which live on the display device rather than
/// the monitor handle.
fn display_mode(device: &[u16]) -> Option<DEVMODEW> {
    let mut mode = DEVMODEW {
        dmSize: core::mem::size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };
    let ok = unsafe {
        EnumDisplaySettingsW(
            windows::core::PCWSTR(device.as_ptr()),
            ENUM_CURRENT_SETTINGS,
            &mut mode,
        )
    };
    ok.as_bool().then_some(mode)
}

/// The display's own resolution and position, in physical pixels.
///
/// `dmPelsWidth` is the mode the hardware is actually running, so it does not
/// move with the caller's DPI awareness the way a monitor rect does.
fn physical_bounds(mode: DEVMODEW) -> Rect {
    // SAFETY: `dmPosition` is the active member for a display device, which is what `EnumDisplaySettingsW` was asked about.
    let position = unsafe { mode.Anonymous1.Anonymous2.dmPosition };
    Rect::new(position.x, position.y, mode.dmPelsWidth, mode.dmPelsHeight)
}

fn scale_factor(monitor: HMONITOR) -> f32 {
    let mut dpi_x = 0u32;
    let mut dpi_y = 0u32;
    match unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) } {
        Ok(()) if dpi_x > 0 => dpi_x as f32 / BASELINE_DPI,
        _ => 1.0,
    }
}

pub(crate) fn displays() -> Result<Vec<Display>> {
    let _physical = PhysicalPixels::scope();
    let mut handles: Vec<HMONITOR> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect_monitor),
            LPARAM(core::ptr::addr_of_mut!(handles) as isize),
        );
    }

    let mut displays = Vec::with_capacity(handles.len());
    for monitor in handles {
        let mut info = MONITORINFOEXW {
            monitorInfo: windows::Win32::Graphics::Gdi::MONITORINFO {
                cbSize: core::mem::size_of::<MONITORINFOEXW>() as u32,
                ..Default::default()
            },
            ..Default::default()
        };
        let ok = unsafe { GetMonitorInfoW(monitor, core::ptr::addr_of_mut!(info).cast()) };
        if !ok.as_bool() {
            continue;
        }
        let mode = display_mode(&info.szDevice);
        displays.push(Display {
            id: DisplayId(monitor.0 as u64),
            name: wide_to_string(&info.szDevice),
            bounds: mode
                .map(physical_bounds)
                .unwrap_or_else(|| rect_of(info.monitorInfo.rcMonitor)),
            scale_factor: scale_factor(monitor),
            refresh_hz: mode
                .filter(|mode| mode.dmDisplayFrequency > 1)
                .map(|mode| mode.dmDisplayFrequency as f32),
            is_primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
            rotation: mode
                .map(|mode| rotation_of(unsafe { mode.Anonymous1.Anonymous2.dmDisplayOrientation }))
                .unwrap_or_default(),
        });
    }
    Ok(displays)
}

/// The executable name owning `window`, which is the closest thing Win32 offers
/// to an application display name without a package manifest lookup.
/// The owning process id and its executable name, from one lookup.
fn owner(window: HWND) -> (u32, String) {
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    if pid == 0 {
        return (0, String::new());
    }
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
        return (pid, String::new());
    };
    let mut buffer = [0u16; 260];
    let mut len = buffer.len() as u32;
    let queried = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut len,
        )
    };
    let _ = unsafe { windows::Win32::Foundation::CloseHandle(handle) };
    if queried.is_err() {
        return (pid, String::new());
    }
    let name = wide_to_string(&buffer[..len as usize])
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or_default()
        .trim_end_matches(".exe")
        .to_string();
    (pid, name)
}

fn window_title(window: HWND) -> String {
    let len = unsafe { GetWindowTextLengthW(window) };
    if len <= 0 {
        return String::new();
    }
    let mut buffer = vec![0u16; len as usize + 1];
    let written = unsafe { GetWindowTextW(window, &mut buffer) };
    wide_to_string(&buffer[..written.max(0) as usize])
}

/// Whether a window is one a user would recognise and could sensibly capture.
///
/// Tool windows and title-less windows are the shell's own scaffolding: every
/// desktop has dozens, and listing them makes a picker useless.
fn is_listable(window: HWND, title: &str) -> bool {
    if title.is_empty() || !unsafe { IsWindowVisible(window) }.as_bool() {
        return false;
    }
    let ex_style = unsafe { GetWindowLongW(window, GWL_EXSTYLE) } as u32;
    ex_style & WS_EX_TOOLWINDOW.0 == 0
}

pub(crate) fn windows() -> Result<Vec<Window>> {
    // Window rects have no mode to read, so the awareness scope is the only way to get physical pixels from `GetWindowRect`.
    let _physical = PhysicalPixels::scope();
    let mut handles: Vec<HWND> = Vec::new();
    unsafe {
        EnumWindows(
            Some(collect_window),
            LPARAM(core::ptr::addr_of_mut!(handles) as isize),
        )
        .map_err(d3d::err)?;
    }

    let mut listed = Vec::new();
    for window in handles {
        let title = window_title(window);
        if !is_listable(window, &title) {
            continue;
        }
        let mut rect = RECT::default();
        if unsafe { GetWindowRect(window, &mut rect) }.is_err() {
            continue;
        }
        let monitor = unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST) };
        let (pid, app_name) = owner(window);
        listed.push(Window {
            id: WindowId(window.0 as u64),
            title,
            app_name,
            pid,
            bounds: rect_of(rect),
            display: DisplayId(monitor.0 as u64),
            is_minimized: unsafe { IsIconic(window) }.as_bool(),
            is_on_screen: true,
        });
    }
    Ok(listed)
}
