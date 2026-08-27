//! Camera enumeration and capture through capturekit.
//!
//! Media Foundation on Windows, AVFoundation on macOS, V4L2 on Linux. This
//! replaces three per-OS listings (a DirectShow parse, an FFmpeg AVFoundation
//! parse and a sysfs scan) that each cost a spawn and could disagree with what
//! the capture backend would actually open.
//!
//! MF does not see DirectShow-only virtual cameras (NVIDIA Broadcast, OBS
//! before 28). That is a deliberate trade: modern virtual cameras register an
//! MF source, and the DirectShow path is legacy COM we do not want to own.

pub mod scale;
pub mod session;

use capturekit::Camera;

/// Whether this platform has a camera backend at all.
///
/// Every platform this ships on has one; the check stays so a port reports "no
/// cameras" rather than offering a picker that cannot open anything.
pub fn supported() -> bool {
    capturekit::capabilities().camera_capture
}

/// Cameras the capture backend can actually open, in the platform's own order.
///
/// A platform with no backend reports an empty list, not an error: "no cameras
/// here" is the honest answer for a picker, and [`supported`] is what explains
/// why.
pub fn devices() -> Result<Vec<Camera>, String> {
    if !supported() {
        return Ok(Vec::new());
    }
    capturekit::cameras().map_err(|e| e.to_string())
}

/// The camera a saved selection names.
///
/// Chromium labels a webcam `"USB2.0 HD UVC WebCam (3277:0029)"` while Media
/// Foundation calls the same device `"USB2.0 HD UVC WebCam"`, so a profile saved
/// before the backend owned the camera holds the browser spelling. Match exactly
/// first, then ignore a trailing `(vid:pid)`.
pub fn find<'a>(cameras: &'a [Camera], query: &str) -> Option<&'a Camera> {
    if let Some(exact) = cameras.iter().find(|camera| camera.name == query) {
        return Some(exact);
    }
    let trimmed = strip_usb_ids(query);
    cameras
        .iter()
        .find(|camera| strip_usb_ids(&camera.name) == trimmed)
}

/// `name` without a trailing ` (1234:abcd)` USB vendor/product pair.
fn strip_usb_ids(name: &str) -> &str {
    let Some(open) = name.rfind(" (") else {
        return name;
    };
    let inner = &name[open + 2..];
    let Some(body) = inner.strip_suffix(')') else {
        return name;
    };
    let Some((vendor, product)) = body.split_once(':') else {
        return name;
    };
    let is_hex = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit());
    if is_hex(vendor) && is_hex(product) {
        name[..open].trim_end()
    } else {
        name
    }
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
            // A bare runner has no media stack; that is not a mapping failure.
            return;
        };
        for camera in &cameras {
            assert!(!camera.name.is_empty(), "a camera reported no name");
            assert!(!camera.id.0.is_empty(), "{} reported no id", camera.name);
        }
    }

    fn camera(name: &str) -> Camera {
        Camera {
            id: capturekit::CameraId(format!("id:{name}")),
            name: name.to_string(),
            is_default: false,
            formats: Vec::new(),
        }
    }

    #[test]
    fn a_browser_label_resolves_to_the_media_foundation_device() {
        let cameras = [camera("USB2.0 HD UVC WebCam")];
        let found = find(&cameras, "USB2.0 HD UVC WebCam (3277:0029)");
        assert_eq!(found.map(|c| c.name.as_str()), Some("USB2.0 HD UVC WebCam"));
    }

    #[test]
    fn an_exact_name_wins_over_a_suffix_match() {
        let cameras = [camera("Cam (aaaa:bbbb)"), camera("Cam")];
        let found = find(&cameras, "Cam (aaaa:bbbb)");
        assert_eq!(found.map(|c| c.name.as_str()), Some("Cam (aaaa:bbbb)"));
    }

    #[test]
    fn a_parenthesised_name_that_is_not_usb_ids_is_kept() {
        // "Camera (NVIDIA Broadcast)" must not be truncated to "Camera".
        assert_eq!(
            strip_usb_ids("Camera (NVIDIA Broadcast)"),
            "Camera (NVIDIA Broadcast)"
        );
        assert!(find(&[camera("Camera")], "Camera (NVIDIA Broadcast)").is_none());
    }

    /// A platform with no backend must look like "no cameras", never like a
    /// broken enumeration: the picker renders one and errors on the other.
    #[test]
    fn a_platform_without_a_backend_reports_an_empty_list() {
        if supported() {
            return;
        }
        assert_eq!(devices(), Ok(Vec::new()));
    }

    #[test]
    fn an_unknown_camera_resolves_to_nothing() {
        assert!(find(&[camera("Cam")], "no such camera").is_none());
    }
}
