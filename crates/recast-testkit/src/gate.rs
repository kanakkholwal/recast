//! Turning a silent skip into a failure where the dependency is meant to exist.
//!
//! A harness that skips when its adapter or binary is missing reports the same
//! green as one that ran, which is how a GPU path can ship never having been
//! dispatched on a platform.

/// Whether a missing GPU adapter should fail rather than skip. CI sets
/// `RECAST_REQUIRE_GPU=1` on every runner expected to have one.
#[must_use]
pub fn gpu_required() -> bool {
    std::env::var("RECAST_REQUIRE_GPU").as_deref() == Ok("1")
}

/// Panics when a required dependency is missing, and otherwise says to skip.
///
/// Call at the top of a test that has just failed to acquire something:
/// `if skip_or_fail("no GPU adapter") { return; }`.
#[must_use]
pub fn skip_or_fail(what: &str) -> bool {
    assert!(
        !gpu_required(),
        "RECAST_REQUIRE_GPU=1 but this machine has {what}"
    );
    eprintln!("skipping: {what}");
    true
}
