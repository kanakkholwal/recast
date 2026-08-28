//! Native OS permission preflights.
//!
//! macOS gates screen capture and global cursor sampling behind two *separate*
//! TCC buckets, which the app previously conflated:
//!
//! - **Screen Recording** — required by ScreenCaptureKit, which serves both the
//!   picture and the system-audio tap. Without it the stream starts but
//!   delivers nothing, so the recording captures nothing while the UI timer
//!   keeps ticking. We HARD-FAIL the start (and trigger the system prompt) so
//!   the user gets an actionable error rather than an empty recording found at
//!   stop(). The microphone is separate: it goes through AVFoundation and takes
//!   the Microphone grant.
//! - **Accessibility** — required by capturekit's pointer reader (CoreGraphics mouse-button
//!   state) for the cursor track. This is non-essential — the screen capture
//!   still works without it — so we only WARN; the cursor track just has gaps
//!   until it's granted.
//!
//! Both checks are no-ops on Windows/Linux (the functions return the
//! permissive default), so callers stay platform-agnostic.

use anyhow::Result;

/// Ensure the OS-level permission required to capture the screen is granted.
///
/// On macOS, if Screen Recording is not authorized this triggers the system
/// consent prompt and returns an error — the grant only takes effect on the
/// next capture attempt, so the user grants it then presses Record again.
/// No-op (always `Ok`) on Windows/Linux.
pub fn ensure_screen_recording() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        if !macos::screen_recording_authorized() {
            // Surfaces the system dialog on first run; on a prior denial it
            // no-ops and the message below tells the user where to fix it.
            macos::request_screen_recording();
            return Err(anyhow::anyhow!(
                "Screen Recording permission is required to record. Grant Recast \
                 in System Settings → Privacy & Security → Screen Recording, then \
                 start the recording again (you may need to restart Recast for the \
                 grant to take effect)."
            ));
        }
    }
    Ok(())
}

/// Whether global cursor sampling (the cursor track) is permitted.
///
/// macOS: reflects the Accessibility trust state. Always `true` elsewhere.
pub fn cursor_tracking_authorized() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::accessibility_trusted()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

#[cfg(target_os = "macos")]
mod macos {
    // The CoreGraphics screen-capture consent API (macOS 10.15+) and the
    // Accessibility trust check live in system frameworks Tauri already links.
    // Declaring the three stable C entry points directly avoids pulling a
    // heavyweight objc2 / core-graphics crate just for three boolean calls.
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        // Both return C `bool` (`_Bool`), which maps to Rust `bool`.
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        // `AXIsProcessTrusted` returns CoreFoundation `Boolean`
        // (`typedef unsigned char Boolean`), NOT C `_Bool` — read it as `u8`
        // and compare, rather than risk a non-0/1 byte as Rust `bool`.
        fn AXIsProcessTrusted() -> u8;
    }

    pub fn screen_recording_authorized() -> bool {
        // SAFETY: argument-less CoreGraphics query with no preconditions.
        unsafe { CGPreflightScreenCaptureAccess() }
    }

    pub fn request_screen_recording() -> bool {
        // SAFETY: argument-less CoreGraphics call; shows the consent prompt the
        // first time and returns the (possibly still-pending) grant state.
        unsafe { CGRequestScreenCaptureAccess() }
    }

    pub fn accessibility_trusted() -> bool {
        // SAFETY: argument-less HIServices query with no preconditions.
        unsafe { AXIsProcessTrusted() != 0 }
    }
}
