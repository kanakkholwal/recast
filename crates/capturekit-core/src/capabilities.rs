/// How much of an exclusion request a platform can actually honour.
///
/// Exclusion is a privacy control: a caller asks for a password manager, or its
/// own recording panel, to be kept out of the capture. Quietly ignoring that
/// request records the thing the user was promised would be hidden, so a backend
/// that cannot do it must say so rather than degrade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum ExclusionSupport {
    /// Any window can be excluded, whoever owns it.
    AnyWindow,
    /// Only windows owned by the calling process.
    ///
    /// Windows has no per-session exclude list; the mechanism is
    /// `SetWindowDisplayAffinity`, which the excluded window's own process must
    /// call. Hiding your own overlay works; hiding someone else's does not.
    OwnWindowsOnly,
    /// No exclusion at all. The compositor decides what the capture contains and
    /// gives a client no say, which is the case under the Wayland portal.
    None,
}

impl ExclusionSupport {
    /// Whether a request naming windows this process does not own can be met.
    #[must_use]
    pub const fn allows_foreign_windows(self) -> bool {
        matches!(self, Self::AnyWindow)
    }

    /// Whether any exclusion at all is possible.
    #[must_use]
    pub const fn allows_any(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// Where a region crop happens, which decides what it costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum RegionCrop {
    /// Cropped during acquisition, so the pixels outside it are never read back.
    DuringAcquisition,
    /// Cropped after the full surface has been read back to host memory.
    OnHost,
}

/// What a platform's capture backend can and cannot do.
///
/// Reported as data so a caller branches on the answer rather than on `cfg`.
/// That is what keeps one API across three very different systems: the shape
/// never changes, only the values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Capabilities {
    /// Which backend answered, for logs and bug reports.
    pub backend: &'static str,
    /// How much of an exclusion request can be honoured.
    pub exclusion: ExclusionSupport,
    /// Whether a single window can be captured without the windows over it.
    pub window_capture: bool,
    /// Whether cameras can be listed and opened as capture targets.
    pub camera_capture: bool,
    /// Whether windows can be listed at all. False under the Wayland portal,
    /// which shows its own picker instead.
    pub window_enumeration: bool,
    /// Whether displays can be listed with real geometry. False under the
    /// Wayland portal, which reports one placeholder for "whatever is picked".
    pub display_enumeration: bool,
    /// Where a region crop is applied.
    pub region_crop: RegionCrop,
    /// Whether the OS can composite the cursor into captured frames.
    pub cursor_in_frame: bool,
    /// Whether the OS reports cursor position and shape alongside frames.
    pub cursor_samples: bool,
    /// Whether the backend reports which regions changed.
    pub dirty_rects: bool,
    /// Whether system output can be captured back as audio.
    pub audio_loopback: bool,
    /// Whether audio devices can be listed by name.
    ///
    /// False on macOS: ScreenCaptureKit serves the system default and names no
    /// others, so a caller picks a direction rather than a device.
    pub audio_device_enumeration: bool,
}

impl Capabilities {
    /// Whether excluding every window in `count_foreign` foreign windows and
    /// `count_own` of this process's own windows can be satisfied.
    #[must_use]
    pub const fn can_exclude(&self, own: usize, foreign: usize) -> bool {
        if own == 0 && foreign == 0 {
            return true;
        }
        match self.exclusion {
            ExclusionSupport::AnyWindow => true,
            ExclusionSupport::OwnWindowsOnly => foreign == 0,
            ExclusionSupport::None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAYLAND: Capabilities = Capabilities {
        backend: "pipewire",
        exclusion: ExclusionSupport::None,
        window_capture: true,
        camera_capture: false,
        window_enumeration: false,
        display_enumeration: false,
        region_crop: RegionCrop::OnHost,
        cursor_in_frame: true,
        cursor_samples: false,
        dirty_rects: false,
        audio_loopback: true,
        audio_device_enumeration: false,
    };

    const WINDOWS: Capabilities = Capabilities {
        backend: "dxgi",
        exclusion: ExclusionSupport::OwnWindowsOnly,
        window_capture: true,
        camera_capture: true,
        window_enumeration: true,
        display_enumeration: true,
        region_crop: RegionCrop::DuringAcquisition,
        cursor_in_frame: true,
        cursor_samples: true,
        dirty_rects: true,
        audio_loopback: true,
        audio_device_enumeration: true,
    };

    const MACOS: Capabilities = Capabilities {
        backend: "screencapturekit",
        exclusion: ExclusionSupport::AnyWindow,
        window_capture: true,
        camera_capture: false,
        window_enumeration: true,
        display_enumeration: true,
        region_crop: RegionCrop::DuringAcquisition,
        cursor_in_frame: true,
        cursor_samples: false,
        dirty_rects: false,
        audio_loopback: true,
        audio_device_enumeration: false,
    };

    #[test]
    fn asking_to_exclude_nothing_always_succeeds() {
        assert!(WAYLAND.can_exclude(0, 0));
        assert!(WINDOWS.can_exclude(0, 0));
    }

    #[test]
    fn wayland_refuses_every_exclusion_rather_than_ignoring_it() {
        assert!(!WAYLAND.can_exclude(1, 0));
        assert!(!WAYLAND.can_exclude(0, 1));
        assert!(!WAYLAND.exclusion.allows_any());
    }

    #[test]
    fn windows_excludes_its_own_windows_but_not_a_strangers() {
        assert!(WINDOWS.can_exclude(3, 0));
        assert!(!WINDOWS.can_exclude(3, 1));
        assert!(!WINDOWS.exclusion.allows_foreign_windows());
    }

    #[test]
    fn macos_excludes_anything() {
        assert!(MACOS.can_exclude(2, 5));
        assert!(MACOS.exclusion.allows_foreign_windows());
    }
}
