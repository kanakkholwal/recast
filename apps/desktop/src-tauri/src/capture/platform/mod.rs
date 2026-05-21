#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub mod linux_wayland;

#[cfg(target_os = "linux")]
mod linux_x11;

#[cfg(target_os = "macos")]
mod macos;

// `fallback` (xcap-backed) is the last-resort on Linux (when neither
// `WAYLAND_DISPLAY` nor `DISPLAY` is set) and on any future target we
// haven't ported yet. macOS now has a native FFmpeg-avfoundation
// backend so it no longer needs the slow fallback path.
#[cfg(not(any(windows, target_os = "macos")))]
mod fallback;

use anyhow::Result;

use super::CaptureSource;
use crate::recording::CaptureTarget;

pub fn create_source(target: &CaptureTarget) -> Result<Box<dyn CaptureSource>> {
    #[cfg(windows)]
    {
        windows::create_source(target)
    }
    #[cfg(target_os = "macos")]
    {
        macos::create_source(target)
    }
    #[cfg(target_os = "linux")]
    {
        // Linux session-type dispatch:
        //
        // 1. **Wayland** — the only legitimate capture path is through
        //    xdg-desktop-portal + PipeWire. The portal handshake runs
        //    earlier in `commands::recording::start_recording` and stashes
        //    the negotiated stream in `linux_wayland::PENDING_PORTAL_STREAM`
        //    for us to pick up here. If the user denied the dialog or
        //    the portal is unavailable, no stream is stashed and we
        //    fall through.
        //
        // 2. **X11** (or XWayland with no portal) — direct GetImage
        //    against the root window. Connection is owned by
        //    `X11CaptureSource` for the lifetime of the recording so
        //    we're not paying connection-setup latency per frame.
        //
        // 3. **xcap fallback** — last resort. Slow and on Wayland
        //    triggers per-frame portal dialogs (which is exactly what
        //    we wired the native paths to avoid), but kept as a safety
        //    net for unusual sessions.
        //
        // We check WAYLAND_DISPLAY before DISPLAY because XWayland sets
        // both — Wayland-native is preferred when available.
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            if linux_wayland::has_pending_stream() {
                return linux_wayland::create_source(target);
            }
            // Wayland session but no portal stream — user denied the
            // dialog or the portal is broken. Fall through to xcap;
            // we'll trigger the portal again per-frame which is bad,
            // but at least the user gets *some* output.
        }
        if std::env::var_os("DISPLAY").is_some() {
            return linux_x11::create_source(target);
        }
        fallback::create_source(target)
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        fallback::create_source(target)
    }
}
