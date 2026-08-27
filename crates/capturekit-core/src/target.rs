use crate::color::ColorSpace;
use crate::format::PixelFormat;
use crate::geom::{Rect, Rotation};

/// A monitor's stable identifier.
///
/// 64 bits because a Windows `HMONITOR` and a macOS `CGDirectDisplayID` are
/// pointer-width and 32-bit respectively, and narrowing the first to fit the
/// second is how two monitors end up sharing an id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct DisplayId(pub u64);

/// A window's stable identifier, an `HWND` or `CGWindowID` widened to 64 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct WindowId(pub u64);

/// A camera's stable identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct CameraId(pub String);

/// What to capture.
///
/// A region is a display plus a rectangle rather than a bare rectangle, so the
/// backend can crop on the GPU instead of grabbing the monitor and cropping the
/// result on the CPU.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase", tag = "kind"))]
pub enum Target {
    /// A whole monitor.
    Display(DisplayId),
    /// A single window, including the parts of it another window covers.
    Window(WindowId),
    /// A rectangle of a monitor, in that monitor's own coordinates.
    Region {
        /// The monitor the rectangle belongs to.
        display: DisplayId,
        /// The rectangle, relative to the monitor's top-left.
        rect: Rect,
    },
    /// A camera.
    Camera(CameraId),
}

impl Target {
    /// The word to use for this target in a message to a human.
    #[must_use]
    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::Display(_) => "display",
            Self::Window(_) => "window",
            Self::Region { .. } => "region",
            Self::Camera(_) => "camera",
        }
    }

    /// The display this target lives on, if it is a screen target at all.
    #[must_use]
    pub const fn display(&self) -> Option<DisplayId> {
        match self {
            Self::Display(id) | Self::Region { display: id, .. } => Some(*id),
            Self::Window(_) | Self::Camera(_) => None,
        }
    }
}

/// A monitor available to capture.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Display {
    /// Stable identifier to pass back as a [`Target`].
    pub id: DisplayId,
    /// Human-readable name, from EDID where the OS exposes it.
    pub name: String,
    /// Position and size in physical pixels, in virtual-desktop coordinates.
    pub bounds: Rect,
    /// Physical pixels per logical point.
    pub scale_factor: f32,
    /// Refresh rate, where the OS reports one.
    pub refresh_hz: Option<f32>,
    /// Whether this is the primary display.
    pub is_primary: bool,
    /// How far the panel is rotated from upright.
    pub rotation: Rotation,
}

/// A window available to capture.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Window {
    /// Stable identifier to pass back as a [`Target`].
    pub id: WindowId,
    /// Window title at the time of enumeration.
    pub title: String,
    /// Owning application's display name.
    pub app_name: String,
    /// Position and size in physical pixels, in virtual-desktop coordinates.
    pub bounds: Rect,
    /// The display holding most of the window.
    pub display: DisplayId,
    /// Whether the window is minimised, and so has nothing to capture.
    pub is_minimized: bool,
    /// Whether the window is on the active desktop and visible.
    pub is_on_screen: bool,
}

impl Window {
    /// Whether capturing this window right now would produce anything.
    #[must_use]
    pub fn is_capturable(&self) -> bool {
        !self.is_minimized && !self.bounds.is_empty()
    }
}

/// A camera available to capture.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Camera {
    /// Stable identifier to pass back as a [`Target`].
    pub id: CameraId,
    /// Human-readable device name.
    pub name: String,
    /// Whether the OS reports this as the default camera.
    pub is_default: bool,
}

/// What a backend actually negotiated, which is not always what was asked for.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct SourceDesc {
    /// Frame width in pixels, before rotation.
    pub width: u32,
    /// Frame height in pixels, before rotation.
    pub height: u32,
    /// Pixel layout of delivered frames.
    pub format: PixelFormat,
    /// Colour description of delivered frames.
    pub color_space: ColorSpace,
    /// Rotation to apply for the frame to appear upright.
    pub rotation: Rotation,
    /// Physical pixels per logical point on the source display.
    pub scale_factor: f32,
    /// Frames per second the backend paces at, where it paces at all.
    pub frame_rate: Option<u32>,
    /// Name of the backend serving this source, for logs and bug reports.
    pub backend: &'static str,
}

impl SourceDesc {
    /// The frame rectangle in its own coordinates, which every crop is checked
    /// against.
    #[must_use]
    pub const fn frame_rect(&self) -> Rect {
        Rect::from_size(self.width, self.height)
    }

    /// The size a consumer sees once rotation is applied.
    #[must_use]
    pub const fn presented_size(&self) -> (u32, u32) {
        self.rotation.apply_to_size(self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_region_knows_which_display_it_belongs_to() {
        let target = Target::Region {
            display: DisplayId(7),
            rect: Rect::new(0, 0, 100, 100),
        };
        assert_eq!(target.display(), Some(DisplayId(7)));
        assert_eq!(target.kind_name(), "region");
    }

    #[test]
    fn a_window_target_has_no_display_of_its_own() {
        assert_eq!(Target::Window(WindowId(1)).display(), None);
    }

    #[test]
    fn a_minimized_window_is_not_capturable() {
        let mut window = Window {
            id: WindowId(1),
            title: "Editor".into(),
            app_name: "Recast".into(),
            bounds: Rect::from_size(800, 600),
            display: DisplayId(0),
            is_minimized: false,
            is_on_screen: true,
        };
        assert!(window.is_capturable());
        window.is_minimized = true;
        assert!(!window.is_capturable());
    }

    #[test]
    fn a_zero_sized_window_is_not_capturable_even_when_it_is_not_minimized() {
        let window = Window {
            id: WindowId(1),
            title: String::new(),
            app_name: String::new(),
            bounds: Rect::from_size(0, 0),
            display: DisplayId(0),
            is_minimized: false,
            is_on_screen: true,
        };
        assert!(!window.is_capturable());
    }

    #[test]
    fn a_rotated_source_presents_its_size_swapped() {
        let desc = SourceDesc {
            width: 1920,
            height: 1080,
            format: PixelFormat::Bgra8,
            color_space: ColorSpace::SRGB,
            rotation: Rotation::Cw90,
            scale_factor: 1.0,
            frame_rate: Some(60),
            backend: "test",
        };
        assert_eq!(desc.presented_size(), (1080, 1920));
        assert_eq!(desc.frame_rect(), Rect::from_size(1920, 1080));
    }
}
