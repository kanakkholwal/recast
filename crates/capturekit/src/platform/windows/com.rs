use std::sync::OnceLock;

use windows::Win32::Foundation::S_FALSE;
use windows::Win32::System::Com::{
    CoIncrementMTAUsage, CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED,
};

/// Holds this thread's COM apartment open for as long as the objects made in it
/// are used, and closes it again on the way out.
///
/// A thread that initialises COM and exits without uninitialising leaves the
/// apartment's reference count wrong. The next thread to create an object in it
/// can then fault: on a recorder that opens the devices once per take, the
/// second take is where that lands.
///
/// A host that already chose an apartment gets `RPC_E_CHANGED_MODE` here, and a
/// thread already initialised gets `S_FALSE`. Uninitialising on either would
/// close an apartment this library does not own.
pub(crate) struct ComScope {
    owned: bool,
}

/// Hold the process MTA open for the rest of the run, once.
///
/// windows-rs caches activation factories and COM proxies in process globals.
/// When the last apartment reference goes away the MTA is torn down and those
/// cached pointers dangle, so the next call FAULTS instead of failing. Anything
/// that opens and closes an apartment per unit of work hits that: a recorder
/// per take, a test harness per test. `CoIncrementMTAUsage` keeps the apartment
/// alive without joining it, and is deliberately never decremented.
fn pin_mta() {
    static PINNED: OnceLock<()> = OnceLock::new();
    PINNED.get_or_init(|| {
        // SAFETY: the cookie is dropped on purpose; releasing it is the fault.
        unsafe {
            let _ = CoIncrementMTAUsage();
        }
    });
}

impl ComScope {
    /// Join the multi-threaded apartment, which every backend here wants.
    pub(crate) fn mta() -> Self {
        pin_mta();
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Self {
            owned: hr.is_ok() && hr != S_FALSE,
        }
    }
}

impl Drop for ComScope {
    fn drop(&mut self) {
        if self.owned {
            unsafe { CoUninitialize() };
        }
    }
}
