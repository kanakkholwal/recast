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

/// Something made inside an apartment, paired with the apartment itself.
///
/// The pairing is the point. Returned as a tuple and bound as `let (value,
/// scope) = ...`, the two become separate locals and LOCALS DROP IN REVERSE:
/// the apartment closes first, and releasing a COM object into a closed
/// apartment faults instead of failing. Struct fields drop in DECLARATION
/// order, so `value` before `scope` is the invariant, and it is pinned by
/// `scoped_tests`.
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
