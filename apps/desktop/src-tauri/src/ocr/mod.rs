//! On-device OCR for screen understanding (agent automation).
//!
//! This reads a video into a timestamped, structured text timeline: sample the
//! frames where the screen actually changed, OCR each of those, and collapse
//! runs of near-identical frames into spans. The output is a list a text-only
//! tool-calling model can act on (OmniParser-shaped element records with
//! normalized coordinates), with an optional image left for a multimodal model.
//!
//! It runs the pure-Rust `ocrs` engine on its `rten` runtime (no C / ONNX-Runtime
//! dependency), so it builds on every target including Intel Mac, mirroring the
//! `tract` choice for silence detection. Native OS OCR (Apple Vision,
//! Windows.Media.Ocr) slots in behind the same [`engine::OcrEngine`] trait as a
//! fast-follow. Gated behind the off-by-default `ocr` Cargo feature.
//!
//! Slice 1 (this): frames -> sampler -> ocrs -> span timeline, reachable via the
//! `read_video_text` command and the `screen.read` control method. Deferred:
//! cursor/click enrichment, per-span thumbnails, and the native OCR tiers.

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
