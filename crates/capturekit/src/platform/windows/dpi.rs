use windows::Win32::UI::HiDpi::{
    SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};

/// Makes Win32 report physical pixels to this thread; without it a 1920px display at 125% enumerates as 1536 while capture duplicates the real 1920.
/// Thread-local on purpose: `SetProcessDpiAwareness` is process-global and belongs to the application, not a library it links.
pub(crate) struct PhysicalPixels(DPI_AWARENESS_CONTEXT);

impl PhysicalPixels {
    pub(crate) fn scope() -> Self {
        // SAFETY: a thread-local switch; null means rejected, as Windows before 10 1607 does.
        Self(unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) })
    }
}

impl Drop for PhysicalPixels {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: restores the context this scope replaced, checked valid just above.
            unsafe { SetThreadDpiAwarenessContext(self.0) };
        }
    }
}
