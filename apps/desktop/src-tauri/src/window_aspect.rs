//! Aspect-ratio-locked resizing: Tao exposes only min/max, and a JS snap-back after `onResized` reads as a rubber-band.
//! Windows intercepts `WM_SIZING` to rewrite the drag rect live; other platforms get a no-op `apply`.

mod imp {
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::sync::OnceLock;

    // `parking_lot::Mutex` can't poison: a panic in the `WM_SIZING` subclass while holding this would abort the process.
    use parking_lot::Mutex;

    use tauri::AppHandle;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{
        WMSZ_BOTTOM, WMSZ_BOTTOMLEFT, WMSZ_LEFT, WMSZ_TOP, WMSZ_TOPLEFT, WMSZ_TOPRIGHT,
        WM_NCDESTROY, WM_SIZING,
    };

    /// Arbitrary, app-unique subclass id (only needs to be stable within the
    /// process so `Set`/`RemoveWindowSubclass` agree).
    const SUBCLASS_ID: usize = 0x5245_4341; // "RECA"

    #[derive(Clone, Copy)]
    struct Constraint {
        /// Aspect ratio of the *video* region (width / height) — not the whole
        /// window, which also carries `chrome` rows of controls below it.
        ratio: f64,
        /// Max width as a fraction of the window's monitor work-area width.
        max_fraction: f64,
        /// Minimum width in physical pixels.
        min_w: i32,
        /// Fixed, non-scaling vertical extent (physical px) reserved at the
        /// bottom of the window for the control bar that lives *outside* the
        /// rounded video so it never gets clipped. `ratio` applies to
        /// `height - chrome`, so the visible bubble keeps its aspect while the
        /// window is `chrome` px taller. 0 == video fills the window.
        chrome: i32,
    }

    /// HWND pointer value → its live aspect constraint. The subclass proc runs
    /// on the UI thread and reads this; the command writes it from whichever
    /// thread Tauri dispatched on, so it's behind a mutex.
    fn registry() -> &'static Mutex<HashMap<isize, Constraint>> {
        static REG: OnceLock<Mutex<HashMap<isize, Constraint>>> = OnceLock::new();
        REG.get_or_init(|| Mutex::new(HashMap::new()))
    }

    /// Register (or update) the aspect constraint for a window and ensure the
    /// `WM_SIZING` subclass is installed. Updating an existing window's ratio
    /// (e.g. the user cycling 1:1 → 16:9) just rewrites the registry entry —
    /// the already-installed subclass picks it up on the next drag.
    pub fn apply(
        app: &AppHandle,
        hwnd_raw: isize,
        ratio: f64,
        max_fraction: f64,
        min_w: i32,
        chrome: i32,
    ) {
        if !(ratio.is_finite() && ratio > 0.0) {
            return;
        }
        let constraint = Constraint {
            ratio,
            max_fraction: if max_fraction > 0.0 {
                max_fraction
            } else {
                0.25
            },
            min_w: min_w.max(1),
            chrome: chrome.max(0),
        };

        let first = {
            let mut reg = registry().lock();
            let first = !reg.contains_key(&hwnd_raw);
            reg.insert(hwnd_raw, constraint);
            first
        };

        // SetWindowSubclass must run on the window's UI thread, and only once; later ratio changes flow through the registry.
        if first {
            let _ = app.run_on_main_thread(move || unsafe {
                let hwnd = HWND(hwnd_raw as *mut c_void);
                let _ = SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, 0);
            });
        }
    }

    /// Width of the work area (excludes the taskbar — matches JS
    /// `screen.availWidth`) of the monitor the window currently sits on.
    unsafe fn monitor_work_width(hwnd: HWND) -> i32 {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            (info.rcWork.right - info.rcWork.left).max(1)
        } else {
            1920
        }
    }

    /// Rewrite the proposed drag rectangle so it keeps `ratio` and stays within
    /// [min_w, max_w]. The edge being dragged decides which dimension leads and
    /// which corner stays anchored, so the side under the cursor tracks the
    /// pointer while the opposite side holds still.
    fn constrain_rect(rect: &mut RECT, edge: u32, c: Constraint, max_w: i32) {
        let cur_w = (rect.right - rect.left).max(1);
        let cur_h = (rect.bottom - rect.top).max(1);

        // The control strip is fixed height, so strip it off before the aspect math and add it back to the final height.
        let chrome = c.chrome.max(0);
        let max_w = max_w.max(c.min_w);
        let max_vid_h = ((max_w as f64) / c.ratio).round() as i32;
        let min_vid_h = (((c.min_w as f64) / c.ratio).round() as i32).max(1);

        // Top and bottom edges are height-led; everything else, including all four corners, is width-led.
        let (new_w, new_h) = if edge == WMSZ_TOP || edge == WMSZ_BOTTOM {
            let vid_h = (cur_h - chrome).clamp(min_vid_h, max_vid_h.max(min_vid_h));
            ((vid_h as f64 * c.ratio).round() as i32, vid_h + chrome)
        } else {
            let w = cur_w.clamp(c.min_w, max_w);
            (w, (((w as f64) / c.ratio).round() as i32) + chrome)
        };

        // Anchor the edge opposite the one under the cursor.
        if edge == WMSZ_LEFT || edge == WMSZ_TOPLEFT || edge == WMSZ_BOTTOMLEFT {
            rect.left = rect.right - new_w;
        } else {
            rect.right = rect.left + new_w;
        }
        if edge == WMSZ_TOP || edge == WMSZ_TOPLEFT || edge == WMSZ_TOPRIGHT {
            rect.top = rect.bottom - new_h;
        } else {
            rect.bottom = rect.top + new_h;
        }
    }

    unsafe extern "system" fn subclass_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _ref: usize,
    ) -> LRESULT {
        match msg {
            WM_SIZING => {
                let key = hwnd.0 as isize;
                let constraint = registry().lock().get(&key).copied();
                if let Some(c) = constraint {
                    // lParam is the OS's new-bounds LPRECT: edit it in place, then chain to tao's proc so its size bookkeeping stays in sync.
                    let rect = &mut *(lparam.0 as *mut RECT);
                    let max_w = ((monitor_work_width(hwnd) as f64) * c.max_fraction).round() as i32;
                    constrain_rect(rect, wparam.0 as u32, c, max_w);
                }
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
            WM_NCDESTROY => {
                // Detach before the window goes away: the chain is freed for us, but this keeps the registry from leaking.
                registry().lock().remove(&(hwnd.0 as isize));
                let _ = RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
            _ => DefSubclassProc(hwnd, msg, wparam, lparam),
        }
    }
}

pub use imp::apply;
