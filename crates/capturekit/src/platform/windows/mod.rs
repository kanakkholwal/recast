mod com;
mod d3d;
mod dpi;
mod dxgi;
mod enumerate;
mod mf;
mod pointer;
mod wasapi;
mod wgc;

use capturekit_core::{
    AudioDevice, AudioDeviceId, AudioDirection, Capabilities, ExclusionSupport, Permission,
    PermissionKind, RegionCrop, Result, Target,
};

use crate::backend::{AudioSource, FrameSource};
use crate::platform::OpenOptions;

pub(crate) use enumerate::{displays, windows};
pub(crate) use pointer::source as pointer_source;

/// What this platform can do, reported as data so callers branch on the answer
/// rather than on `cfg`.
pub(crate) fn capabilities() -> Capabilities {
    Capabilities {
        backend: "dxgi",
        // No per-session exclude list exists; `SetWindowDisplayAffinity` is the mechanism, and only the owning process may call it.
        exclusion: ExclusionSupport::OwnWindowsOnly,
        window_capture: wgc::is_supported(),
        camera_capture: true,
        window_enumeration: true,
        display_enumeration: true,
        region_crop: RegionCrop::DuringAcquisition,
        // Only Graphics Capture composites a cursor; Desktop Duplication refuses `CursorMode::Include` rather than dropping it.
        cursor_in_frame: wgc::is_supported(),
        cursor_samples: true,
        cursor_pointer: true,
        cursor_buttons: true,
        dirty_rects: true,
        audio_loopback: true,
        audio_loopback_gap_filling: true,
        audio_device_enumeration: true,
    }
}

/// Windows gates neither screen nor window capture behind consent.
pub(crate) fn permission(kind: PermissionKind) -> Permission {
    match kind {
        PermissionKind::Screen => Permission::NotRequired,
        // Privacy settings gate only packaged apps; a desktop process is told nothing until it opens the device.
        PermissionKind::Camera | PermissionKind::Microphone => Permission::NotDetermined,
        // `PermissionKind` is non-exhaustive, and an unknown capability is not one this platform can claim to have granted.
        _ => Permission::NotDetermined,
    }
}

pub(crate) fn request_permission(kind: PermissionKind) -> Permission {
    permission(kind)
}

/// The current instant on the clock the backends stamp frames with, so a
/// screenshot can tell a frame from before its request from one after it.
pub(crate) fn now() -> capturekit_core::Timestamp {
    use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
    let mut ticks = 0i64;
    let mut frequency = 0i64;
    // SAFETY: both counters write one `i64` each into live locals.
    unsafe {
        let _ = QueryPerformanceCounter(&mut ticks);
        let _ = QueryPerformanceFrequency(&mut frequency);
    }
    capturekit_core::Timestamp::from_ticks(ticks, frequency)
}

pub(crate) fn cameras() -> Result<Vec<capturekit_core::Camera>> {
    mf::cameras()
}

pub(crate) fn audio_devices() -> Result<Vec<AudioDevice>> {
    wasapi::devices()
}

pub(crate) fn open_audio(
    device: Option<&AudioDeviceId>,
    direction: AudioDirection,
) -> Result<Box<dyn AudioSource>> {
    Ok(Box::new(wasapi::WasapiSource::open(device, direction)?))
}

pub(crate) fn open(target: &Target, opts: &OpenOptions) -> Result<Box<dyn FrameSource>> {
    match target {
        Target::Display(id) => Ok(Box::new(dxgi::DxgiSource::open(*id, opts)?)),
        Target::Region { display, rect } => {
            let opts = OpenOptions {
                region: Some(*rect),
                ..opts.clone()
            };
            Ok(Box::new(dxgi::DxgiSource::open(*display, &opts)?))
        }
        // Graphics Capture, not duplication plus a crop: a maximised or overlapped window is only separable at the compositor.
        Target::Window(id) => Ok(Box::new(wgc::WgcSource::open(*id, opts)?)),
        Target::Camera(id) => Ok(Box::new(mf::MfCameraSource::open(id, opts)?)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Enumeration is the one part of the backend that needs no session, no
    /// permission and no window, so it runs anywhere CI does.
    #[test]
    fn every_display_has_a_usable_size_and_a_unique_id() {
        let displays = displays().expect("windows always has a display list");
        assert!(!displays.is_empty(), "no displays enumerated");
        let mut ids: Vec<u64> = displays.iter().map(|d| d.id.0).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), before, "two displays share an id");
        for display in &displays {
            assert!(!display.bounds.is_empty(), "{} has no area", display.name);
            assert!(display.scale_factor > 0.0);
        }
    }

    #[test]
    fn exactly_one_display_is_primary() {
        let displays = displays().expect("a display list");
        assert_eq!(displays.iter().filter(|d| d.is_primary).count(), 1);
    }

    #[test]
    fn listed_windows_have_titles_and_sit_on_a_known_display() {
        let windows = windows().expect("a window list");
        let displays: Vec<u64> = displays()
            .expect("a display list")
            .iter()
            .map(|d| d.id.0)
            .collect();
        for window in &windows {
            assert!(!window.title.is_empty(), "an untitled window was listed");
            assert!(
                displays.contains(&window.display.0),
                "{} is on an unknown display",
                window.title
            );
        }
    }

    #[test]
    fn screen_capture_needs_no_permission_on_windows() {
        assert!(permission(PermissionKind::Screen).is_usable());
    }
}
