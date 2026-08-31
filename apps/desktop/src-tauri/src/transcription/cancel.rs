//! Cooperative cancel; one run at a time, so a module-level flag is the whole registry.
//! The coarse flag is checked at phase boundaries, but only ggml's native abort callback can interrupt inference mid-recording.

use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;

static REQUESTED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "ggml")]
static ENGINE: Mutex<Option<transcribe_cpp::CancelToken>> = Mutex::new(None);

/// Arm a new run, clearing any request left over from the previous one.
pub(crate) fn begin() {
    REQUESTED.store(false, Ordering::SeqCst);
    #[cfg(feature = "ggml")]
    {
        *ENGINE.lock() = None;
    }
}

/// Ask the in-flight run to stop. Safe to call when nothing is running.
pub(crate) fn request() {
    REQUESTED.store(true, Ordering::SeqCst);
    #[cfg(feature = "ggml")]
    if let Some(token) = ENGINE.lock().as_ref() {
        token.cancel();
    }
}

pub(crate) fn is_requested() -> bool {
    REQUESTED.load(Ordering::SeqCst)
}

/// Publish the engine's token for the duration of a run. Pre-cancels it when a
/// request already landed, which closes the gap between `begin` and the run.
#[cfg(feature = "ggml")]
pub(crate) fn install(token: &transcribe_cpp::CancelToken) {
    if is_requested() {
        token.cancel();
    }
    *ENGINE.lock() = Some(token.clone());
}

#[cfg(feature = "ggml")]
pub(crate) fn uninstall() {
    *ENGINE.lock() = None;
}

/// Marker the frontend matches on to tell a user cancel from a real failure.
pub(crate) const CANCELLED_MSG: &str = "transcription cancelled";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_clears_a_stale_request() {
        request();
        assert!(is_requested());
        begin();
        assert!(!is_requested());
    }
}
