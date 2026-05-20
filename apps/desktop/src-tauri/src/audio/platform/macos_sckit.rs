//! macOS system-audio loopback via ScreenCaptureKit — **placeholder**.
//!
//! ## Status
//!
//! Integration is **deferred pending API verification**. The first
//! attempt at wiring the `screencapturekit` crate used module paths
//! (`sc_content_filter`, `sc_stream`, `sc_output_handler`, ...) that
//! exist in some published versions but not in the current one on
//! crates.io. The crate's module layout has shifted across 0.2 / 0.3
//! releases and the only reliable way to write this against the
//! current API is to read its docs on docs.rs while having an actual
//! macOS host to compile against — neither of which the Windows dev
//! host can do, and trying to keep guessing module paths spends CI
//! cycles for no signal.
//!
//! ## Behaviour now
//!
//! `try_start` always returns `Err`. The caller in
//! `audio/platform/ffmpeg_unix.rs::PlatformAudioSession::start` reacts
//! to that by falling through to the existing virtual-driver
//! detection: BlackHole / Soundflower / Loopback / VB-Cable on macOS,
//! `pactl` monitor source on Linux. macOS users with such a driver
//! installed therefore still get loopback; users without one get
//! silence + the existing actionable warning that names installable
//! drivers.
//!
//! ## What needs to happen next
//!
//! 1. Verify the `screencapturekit` crate's current module paths
//!    against docs.rs (or pick a different crate — `objc2-screen-capture-kit`
//!    is the more stable lower-level option in the `objc2` ecosystem).
//! 2. Reinstate the `screencapturekit` (or alternative) dep in
//!    `Cargo.toml` under `[target.'cfg(target_os = "macos")']`.
//! 3. Implement `try_start` to:
//!    - call `SCShareableContent` to get a display,
//!    - build an `SCContentFilter` (display variant),
//!    - configure `SCStreamConfiguration` with `captures_audio = true`,
//!      `excludes_current_process_audio = true`, 48 kHz / 2 ch float32,
//!    - register an audio output handler that copies each
//!      `CMSampleBuffer`'s PCM payload into the shared `WavWriter`,
//!      honouring `pause_flag` exactly like the FFmpeg/WASAPI paths.
//! 4. Verify on macOS 13+ with Screen Recording permission granted.
//! 5. Confirm graceful failure on macOS 11/12 (SCKit unavailable) and
//!    Screen Recording denied so the BlackHole fallback still kicks in.
//!
//! The shape of `ScKitLoopback` (constructor returning `Result`, owned
//! WAV writer behind `Arc<Mutex<Option<...>>>`, separate `stop()`) is
//! preserved so the eventual implementation drops in without touching
//! the caller.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use anyhow::{anyhow, Result};

pub struct ScKitLoopback {
    // No fields while this is a placeholder — kept as a unit-shaped
    // struct rather than a `()` alias so the caller's match arm and
    // `stop()` signature stay stable when the real implementation
    // lands.
    _placeholder: (),
}

impl ScKitLoopback {
    /// Always returns `Err` while the real SCKit integration is
    /// pending — the caller treats that as "skip SCKit, try the
    /// virtual-driver chain next".
    pub fn try_start(_output_path: PathBuf, _pause_flag: Arc<AtomicBool>) -> Result<Self> {
        Err(anyhow!(
            "ScreenCaptureKit integration deferred — falling back to \
             virtual-driver detection (BlackHole / Soundflower / etc.)"
        ))
    }

    /// Unreachable while `try_start` always errs. Kept so the type's
    /// public surface matches what the caller would expect once the
    /// real implementation lands.
    pub fn stop(self) -> Result<PathBuf> {
        unreachable!(
            "ScKitLoopback::stop must not be called against the placeholder — \
             try_start always returns Err so no session ever exists"
        )
    }
}
