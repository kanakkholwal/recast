use windows::Win32::UI::HiDpi::{
    SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};

/// Makes Win32 report physical pixels to this thread for as long as it lives.
///
/// Without it, `GetMonitorInfoW` and `GetWindowRect` answer a DPI-unaware process
/// in logical points: a 1920px display at 125% reports 1536, while the capture
/// backends duplicate the real 1920. Enumeration and capture would then disagree
/// on every scaled display.
///
/// Thread-local on purpose. `SetProcessDpiAwareness` is process-global and
/// belongs to the application, not to a library it happens to link.
pub(crate) struct PhysicalPixels(DPI_AWARENESS_CONTEXT);

impl PhysicalPixels {
    pub(crate) fn scope() -> Self {
        // Null means the context was rejected, as Windows before 10 1607 does; those builds predate per-monitor scaling anyway.
        Self(unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) })
    }
}

impl Drop for PhysicalPixels {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe { SetThreadDpiAwarenessContext(self.0) };
        }
    }
}
