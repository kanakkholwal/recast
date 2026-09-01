use std::sync::OnceLock;

use windows::Win32::Foundation::S_FALSE;
use windows::Win32::System::Com::{
    CoIncrementMTAUsage, CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED,
};

/// Holds this thread's COM apartment open for the life of the objects made in it.
/// Exiting without uninitialising leaves the refcount wrong and the next thread faults, which on a recorder lands on take two.
pub(crate) struct ComScope {
    owned: bool,
}

/// Holds the process MTA open for the rest of the run, once, via `CoIncrementMTAUsage` and deliberately never decremented.
/// windows-rs caches factories in process globals, so tearing the MTA down dangles them and the next call FAULTS: a recorder per take hits exactly that.
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
        // SAFETY: initialises COM for this thread; the matching uninitialise is in `Drop`.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Self {
            owned: hr.is_ok() && hr != S_FALSE,
        }
    }
}

/// Something made inside an apartment, paired with the apartment itself; the pairing is the point.
/// As a tuple they become separate locals and LOCALS DROP IN REVERSE, releasing a COM object into a closed apartment; struct fields drop in declaration order.
pub(crate) struct Scoped<T, S = ComScope> {
    pub(crate) value: T,
    scope: S,
}

impl<T, S> Scoped<T, S> {
    pub(crate) const fn new(value: T, scope: S) -> Self {
        Self { value, scope }
    }

    /// Hand the apartment on, releasing `value` first.
    /// For a caller that outlives this scope: the apartment has to stay open for as long as anything made in it is still alive.
    pub(crate) fn into_scope(self) -> S {
        self.scope
    }
}

impl Drop for ComScope {
    fn drop(&mut self) {
        if self.owned {
            // SAFETY: only when this guard owns the initialisation it is undoing.
            unsafe { CoUninitialize() };
        }
    }
}

#[cfg(test)]
mod scoped_tests {
    use super::Scoped;
    use std::sync::{Arc, Mutex};

    /// Records the order things were dropped in.
    struct Probe(&'static str, Arc<Mutex<Vec<&'static str>>>);

    impl Drop for Probe {
        fn drop(&mut self) {
            self.1
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(self.0);
        }
    }

    /// The invariant the COM code depends on: the thing made in the apartment is released BEFORE the apartment closes. Reordering `Scoped`'s fields inverts this and fails here.
    #[test]
    fn the_value_is_dropped_before_the_scope_that_made_it() {
        let order = Arc::new(Mutex::new(Vec::new()));
        drop(Scoped::new(
            Probe("value", Arc::clone(&order)),
            Probe("scope", Arc::clone(&order)),
        ));
        let order = order.lock().unwrap_or_else(|e| e.into_inner()).clone();
        assert_eq!(order, vec!["value", "scope"]);
    }

    /// The shape that caused the fault, kept as the contrast: two locals from
    /// one `let` drop in reverse, so the apartment would go first.
    #[test]
    fn two_locals_drop_in_the_opposite_order() {
        let order = Arc::new(Mutex::new(Vec::new()));
        {
            let _value = Probe("value", Arc::clone(&order));
            let _scope = Probe("scope", Arc::clone(&order));
        }
        let order = order.lock().unwrap_or_else(|e| e.into_inner()).clone();
        assert_eq!(order, vec!["scope", "value"]);
    }
}
