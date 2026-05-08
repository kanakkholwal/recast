#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub mod linux_wayland;

#[cfg(not(windows))]
mod fallback;

use anyhow::Result;

use super::CaptureSource;
use crate::recording::CaptureTarget;

pub fn create_source(target: &CaptureTarget) -> Result<Box<dyn CaptureSource>> {
    #[cfg(windows)]
    {
        windows::create_source(target)
    }
    #[cfg(target_os = "linux")]
    {
        // Wayland sessions go through the xdg-desktop-portal + PipeWire
        // path. The portal handshake happens earlier — in
        // `commands::recording::start_recording` — and the stream handle is
        // stashed in `linux_wayland::PENDING_PORTAL_STREAM` for this thread
        // to pick up. We detect Wayland by `WAYLAND_DISPLAY` because that's
        // what the portal actually keys off; relying on `XDG_SESSION_TYPE`
        // misses XWayland-tunneled processes that *should* still use the
        // native path. If a stashed stream is present we use it; otherwise
        // we fall through to the xcap-based fallback (e.g. pure X11
        // sessions, or Wayland sessions where the user denied the portal).
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            if linux_wayland::has_pending_stream() {
                return linux_wayland::create_source(target);
            }
        }
        fallback::create_source(target)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        fallback::create_source(target)
    }
}
