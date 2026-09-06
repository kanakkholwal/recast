//! On-device OCR turning a video into a timestamped span timeline an agent can act on.
//! Pure-Rust `ocrs` on `rten`, so it builds everywhere including Intel Mac; behind the off-by-default `ocr` feature.

pub mod command;
pub mod engine;
pub mod frames;
pub mod models;
pub mod timeline;

/// End-to-end tests that drive the real FFmpeg decode (and, for the OCR leg, the
/// real models). All `#[ignore]`d: CI never executes FFmpeg, so they run by hand.
/// Needs the real engine, so it is gated with the feature as well as `test`.
#[cfg(all(test, feature = "ocr"))]
mod harness;

// `read_video_text` is deliberately not re-exported: `#[tauri::command]` generates a companion item a `pub use` doesn't carry, so `generate_handler!` needs the defining path.
pub use command::run;
