//! Camera enumeration and capture through capturekit.
//!
//! Media Foundation on Windows, AVFoundation on macOS. This replaces parsing
//! FFmpeg's DirectShow listing, which cost an FFmpeg spawn per call and could
//! block for seconds on a slow webcam.
//!
//! MF does not see DirectShow-only virtual cameras (NVIDIA Broadcast, OBS
//! before 28). That is a deliberate trade: modern virtual cameras register an
//! MF source, and the DirectShow path is legacy COM we do not want to own.

pub mod scale;
pub mod session;

use capturekit::Camera;

/// Cameras the capture backend can actually open, in the platform's own order.
pub fn devices() -> Result<Vec<Camera>, String> {
    capturekit::cameras().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A machine with no camera is an empty list, not a failure. Windows is
    /// asserted strictly because Media Foundation is always present there, so
    /// tolerating an error would let a broken backend pass as "no cameras".
    #[test]
    fn enumeration_reports_a_list_rather_than_failing() {
        let found = devices();
        #[cfg(windows)]
        let cameras = found.expect("Media Foundation enumeration failed on Windows");
        #[cfg(not(windows))]
        let Ok(cameras) = found
        else {
            // No backend on Linux yet, and a bare runner has no media stack.
            return;
        };
        for camera in &cameras {
            assert!(!camera.name.is_empty(), "a camera reported no name");
            assert!(!camera.id.0.is_empty(), "{} reported no id", camera.name);
        }
    }
}
