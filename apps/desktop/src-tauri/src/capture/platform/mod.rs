#[cfg(windows)]
pub(crate) mod windows;

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

/// Parse FFmpeg's avfoundation device listing into `(screen_ordinal,
/// avfoundation_input_index)` pairs, one per "Capture screen N" entry.
///
/// FFmpeg prints lines like `[AVFoundation indev @ 0x..] [3] Capture screen 1`,
/// where `[3]` is the *global* input index `-i "3:"` expects (cameras are
/// listed first, then screens) and the trailing `1` is the screen ordinal.
/// macOS maps a chosen display (by its position in `CGGetActiveDisplayList`) to
/// the matching capture input through this. Kept pure + listing-driven — no
/// macOS APIs — so it's unit-testable on any host. Used by
/// `capture::platform::macos`; dead elsewhere.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn parse_capture_screen_listing(stderr: &str) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut in_video = false;
    for line in stderr.lines() {
        if line.contains("video devices:") {
            in_video = true;
            continue;
        }
        if line.contains("audio devices:") {
            // Audio devices can also be named "...screen..."; only the video
            // section holds capture screens, so stop matching here.
            in_video = false;
            continue;
        }
        if !in_video {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        let Some(pos) = lower.find("capture screen") else {
            continue;
        };
        // Screen ordinal: the first integer after "capture screen" (handles
        // double digits — "Capture screen 10" → 10, not 1).
        let after = &lower[pos + "capture screen".len()..];
        let ordinal: u32 = match after
            .split(|c: char| !c.is_ascii_digit())
            .find(|t| !t.is_empty())
            .and_then(|t| t.parse().ok())
        {
            Some(n) => n,
            None => continue,
        };
        // Global input index: the last `[N]` bracket pair BEFORE the "Capture
        // screen" text — skips the `[AVFoundation indev @ 0x..]` log prefix and
        // never reads into the label itself.
        let prefix = &line[..pos];
        let Some(close) = prefix.rfind(']') else {
            continue;
        };
        let Some(open) = prefix[..close].rfind('[') else {
            continue;
        };
        if let Ok(global) = prefix[open + 1..close].trim().parse::<u32>() {
            out.push((ordinal, global));
        }
    }
    out
}

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

#[cfg(test)]
mod tests {
    use super::parse_capture_screen_listing;

    // A realistic `ffmpeg -f avfoundation -list_devices true -i ""` dump:
    // one camera (global [0]) ahead of two screens, then the audio section.
    const LISTING: &str = "\
[AVFoundation indev @ 0x7fa] AVFoundation video devices:
[AVFoundation indev @ 0x7fa] [0] FaceTime HD Camera
[AVFoundation indev @ 0x7fa] [1] Capture screen 0
[AVFoundation indev @ 0x7fa] [2] Capture screen 1
[AVFoundation indev @ 0x7fa] AVFoundation audio devices:
[AVFoundation indev @ 0x7fa] [0] MacBook Pro Microphone";

    #[test]
    fn maps_screen_ordinals_to_global_input_indices() {
        // Screen 0 is global input 1 (camera took 0); screen 1 is input 2.
        assert_eq!(parse_capture_screen_listing(LISTING), vec![(0, 1), (1, 2)]);
    }

    #[test]
    fn ignores_audio_section_and_handles_no_screens() {
        let camera_only = "\
[AVFoundation indev @ 0x1] AVFoundation video devices:
[AVFoundation indev @ 0x1] [0] FaceTime HD Camera
[AVFoundation indev @ 0x1] AVFoundation audio devices:
[AVFoundation indev @ 0x1] [0] Capture screen audio (not a real device)";
        assert!(parse_capture_screen_listing(camera_only).is_empty());
    }

    #[test]
    fn parses_double_digit_ordinals() {
        let many = "\
[x] AVFoundation video devices:
[x] [9] Capture screen 9
[x] [10] Capture screen 10";
        assert_eq!(parse_capture_screen_listing(many), vec![(9, 9), (10, 10)]);
    }

    #[test]
    fn empty_or_garbage_listing_yields_nothing() {
        assert!(parse_capture_screen_listing("").is_empty());
        assert!(parse_capture_screen_listing("no devices here").is_empty());
    }
}
