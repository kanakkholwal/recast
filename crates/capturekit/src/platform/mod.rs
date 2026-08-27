#[cfg(windows)]
pub(crate) mod windows;
#[cfg(windows)]
pub(crate) use windows as os;

#[cfg(target_os = "macos")]
pub(crate) mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos as os;

#[cfg(target_os = "linux")]
pub(crate) mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux as os;

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub(crate) mod unsupported;
#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub(crate) use unsupported as os;

use capturekit_core::{ColorSpaceRequest, Pacing, Rect, WindowId};

use crate::shot::{CursorMode, ShotOptions};

/// What every backend is opened with, whatever surface asked for it.
///
/// One struct for the one-shot and the streaming path: a screenshot is a
/// recording that stops after one frame, so the two must not be able to
/// negotiate different cursors, regions or colour spaces.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct OpenOptions {
    pub cursor: CursorMode,
    /// Crop applied during acquisition, in the target's own coordinates.
    pub region: Option<Rect>,
    /// How the output timeline relates to what the source produced.
    pub pacing: Pacing,
    /// Windows to keep out of the capture, whoever owns them.
    pub exclude: Vec<WindowId>,
    pub color_space: ColorSpaceRequest,
}

impl OpenOptions {
    /// Frames per second the backend should pace at, where it paces at all.
    pub(crate) fn frame_rate(&self) -> Option<u32> {
        self.pacing.fps()
    }
}

impl From<&ShotOptions> for OpenOptions {
    fn from(opts: &ShotOptions) -> Self {
        Self {
            cursor: opts.cursor,
            region: opts.region,
            // A screenshot wants the next frame, not a paced one.
            pacing: Pacing::Passthrough,
            exclude: Vec::new(),
            color_space: opts.color_space,
        }
    }
}
