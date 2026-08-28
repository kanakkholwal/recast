use windows::Win32::Foundation::S_FALSE;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

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

impl ComScope {
    /// Join the multi-threaded apartment, which every backend here wants.
    pub(crate) fn mta() -> Self {
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
