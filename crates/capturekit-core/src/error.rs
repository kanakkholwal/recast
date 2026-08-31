use core::time::Duration;

use crate::format::PixelFormat;
use crate::permission::PermissionKind;

/// The result type every fallible capturekit call returns.
pub type Result<T, E = CaptureError> = core::result::Result<T, E>;

/// Everything that can go wrong acquiring or reading a frame.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CaptureError {
    /// The requested display, window or camera is gone or was never there.
    #[error("no {kind} matches id {id}")]
    NotFound {
        /// What was being looked up.
        kind: &'static str,
        /// The id that matched nothing.
        id: u64,
    },

    /// A source named by a string rather than a handle, that is gone or was never there.
    /// Separate from [`CaptureError::NotFound`] because a camera is a device path and an audio endpoint a node name: narrowing either to a number merges two devices into one.
    #[error("no {kind} matches {id:?}")]
    NotFoundNamed {
        /// What was being looked up.
        kind: &'static str,
        /// The name that matched nothing.
        id: String,
    },

    /// The source is already being captured and cannot be handed out twice.
    #[error("{kind} {id} is already being captured by this process")]
    AlreadyCaptured {
        /// What was already open.
        kind: &'static str,
        /// Its id.
        id: u64,
    },

    /// The OS refused, and the user has to grant access before a retry can work.
    #[error("{0} capture permission was not granted")]
    PermissionDenied(PermissionKind),

    /// The source went away mid-stream.
    #[error("the capture source was lost: {0}")]
    Lost(#[from] LostReason),

    /// The active backend cannot do this, and no amount of retrying changes that.
    #[error("{backend} cannot {operation}")]
    Unsupported {
        /// The backend that refused.
        backend: &'static str,
        /// What was asked of it, phrased to complete the sentence.
        operation: &'static str,
    },

    /// No frame arrived in time.
    #[error("no frame arrived within {0:?}")]
    Timeout(Duration),

    /// A row of pixels does not fit in the stride the caller declared.
    #[error("a {width}px {format:?} row needs {needed} bytes, but the stride is {stride}")]
    StrideTooSmall {
        /// The format whose row does not fit.
        format: PixelFormat,
        /// Frame width in pixels.
        width: u32,
        /// The stride that was offered.
        stride: u32,
        /// The stride that was required.
        needed: u32,
    },

    /// A subsampled format was handed a frame it cannot represent.
    #[error("{format:?} is subsampled and cannot hold a {width}x{height} frame")]
    OddDimensions {
        /// The subsampled format.
        format: PixelFormat,
        /// Frame width in pixels.
        width: u32,
        /// Frame height in pixels.
        height: u32,
    },

    /// The buffer is too small for the frame it claims to hold.
    #[error("a {width}x{height} {format:?} frame needs {needed} bytes, got {got}")]
    ShortBuffer {
        /// The format being read.
        format: PixelFormat,
        /// Frame width in pixels.
        width: u32,
        /// Frame height in pixels.
        height: u32,
        /// Bytes the frame requires.
        needed: usize,
        /// Bytes actually available.
        got: usize,
    },

    /// An exclusion request cannot be honoured by this platform.
    /// Never downgraded to a warning: the caller asked for something to be kept out of the recording, and capturing it anyway is the failure they were trying to prevent.
    #[error("{backend} cannot exclude {requested} window(s) from a capture ({detail})")]
    ExclusionUnsupported {
        /// The backend that refused.
        backend: &'static str,
        /// How many windows were asked for.
        requested: usize,
        /// What the platform can do instead.
        detail: &'static str,
    },

    /// An audio buffer ends mid-frame, so the reader and the device disagree
    /// about the channel count.
    #[error("{len} bytes is not a whole number of {bytes_per_frame}-byte {format:?} frames")]
    PartialAudioFrame {
        /// The format being read.
        format: crate::audio::AudioFormat,
        /// Bytes offered.
        len: usize,
        /// Bytes one sample frame occupies.
        bytes_per_frame: usize,
    },

    /// A backend handed over per-channel planes that do not agree.
    /// One plane shorter than another means the buffer list was misread, which interleaving would turn into swapped channels rather than an error.
    #[error("channel {channel} carries {len} bytes where the first carries {expected}")]
    RaggedAudioPlanes {
        /// The channel that disagreed.
        channel: usize,
        /// Bytes that channel carries.
        len: usize,
        /// Bytes every channel should carry.
        expected: usize,
    },

    /// A cursor image is shorter than the masks it declares.
    #[error("a {kind:?} cursor needs {needed} bytes, got {got}")]
    ShortCursorShape {
        /// How the image said it was stored.
        kind: crate::cursor::CursorShapeKind,
        /// Bytes the image requires.
        needed: usize,
        /// Bytes actually present.
        got: usize,
    },

    /// The frame does not fit in this platform's address space.
    #[error("a {width}x{height} frame needs {bytes} bytes, more than this platform can address")]
    FrameTooLarge {
        /// Frame width in pixels.
        width: u32,
        /// Frame height in pixels.
        height: u32,
        /// Bytes the frame would require.
        bytes: u64,
    },

    /// The OS failed in a way capturekit does not model.
    /// Carries the platform error as a source so `{:#}` and `Error::source` still reach the original HRESULT or errno.
    #[error("{backend} capture failed")]
    Backend {
        /// The backend that failed.
        backend: &'static str,
        /// The underlying platform error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl CaptureError {
    /// Wrap a platform error without leaking its type into the public API.
    pub fn backend<E>(backend: &'static str, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Backend {
            backend,
            source: Box::new(source),
        }
    }

    /// Whether reacquiring the source could plausibly succeed.
    /// The recovery policy keys off this, so a denied permission does not spin in a reacquire loop the user cannot break out of.
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Lost(reason) => reason.is_recoverable(),
            Self::Timeout(_) => true,
            _ => false,
        }
    }
}

/// Why a live capture stopped producing frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum LostReason {
    /// Another process took exclusive control, or a secure desktop appeared.
    #[error("another process took exclusive access")]
    AccessLost,
    /// The GPU was reset or removed.
    #[error("the graphics device was reset or removed")]
    DeviceLost,
    /// The captured display was unplugged or reconfigured.
    #[error("the display was disconnected")]
    DisplayDisconnected,
    /// The captured window closed.
    #[error("the window closed")]
    WindowClosed,
    /// The user revoked a portal or TCC grant while recording.
    #[error("the capture permission was revoked")]
    PermissionRevoked,
}

impl LostReason {
    /// Whether reacquiring the source could plausibly succeed.
    #[must_use]
    pub const fn is_recoverable(self) -> bool {
        matches!(self, Self::AccessLost | Self::DeviceLost)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lost_display_is_not_worth_reacquiring_but_a_lost_device_is() {
        assert!(CaptureError::Lost(LostReason::DeviceLost).is_recoverable());
        assert!(!CaptureError::Lost(LostReason::DisplayDisconnected).is_recoverable());
    }

    #[test]
    fn a_denied_permission_never_looks_recoverable() {
        let denied = CaptureError::PermissionDenied(PermissionKind::Screen);
        assert!(!denied.is_recoverable());
    }

    #[test]
    fn a_backend_error_keeps_the_platform_error_as_its_source() {
        let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "E_ACCESSDENIED");
        let err = CaptureError::backend("dxgi", io);
        let source = std::error::Error::source(&err).expect("the platform error is retained");
        assert!(source.to_string().contains("E_ACCESSDENIED"));
    }

    #[test]
    fn error_messages_name_the_numbers_a_caller_needs_to_fix_them() {
        let err = CaptureError::ShortBuffer {
            format: PixelFormat::Nv12,
            width: 1920,
            height: 1080,
            needed: 3_110_400,
            got: 100,
        };
        let text = err.to_string();
        assert!(text.contains("3110400"), "{text}");
        assert!(text.contains("1920x1080"), "{text}");
    }
}
