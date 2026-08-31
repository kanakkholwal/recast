//! Native OS permission preflights; no-ops off macOS.
//! Screen Recording hard-fails (its absence yields an empty recording); Accessibility only warns, costing cursor samples.

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
            // Surfaces the system dialog on first run; after a denial it no-ops and the message below points at the fix.
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
    // Declaring the three stable C entry points directly avoids pulling in objc2 or core-graphics for three boolean calls.
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        // Both return C `bool` (`_Bool`), which maps to Rust `bool`.
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        // `AXIsProcessTrusted` returns a CoreFoundation `Boolean` (unsigned char), so read it as `u8` rather than `bool`.
        fn AXIsProcessTrusted() -> u8;
    }

    pub fn screen_recording_authorized() -> bool {
        // SAFETY: argument-less CoreGraphics query with no preconditions.
        unsafe { CGPreflightScreenCaptureAccess() }
    }

    pub fn request_screen_recording() -> bool {
        // SAFETY: argument-less CoreGraphics call; it shows the consent prompt once and returns the grant state.
        unsafe { CGRequestScreenCaptureAccess() }
    }

    pub fn accessibility_trusted() -> bool {
        // SAFETY: argument-less HIServices query with no preconditions.
        unsafe { AXIsProcessTrusted() != 0 }
    }
}
