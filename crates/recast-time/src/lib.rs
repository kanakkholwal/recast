#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod cuts;
mod map;
mod segments;
mod speed;
mod timestamp;

#[cfg(all(feature = "clock", not(target_arch = "wasm32")))]
mod clock;

pub use cuts::{
    cut_containing, normalize_cuts, original_to_output_cuts, output_to_original_cuts,
    total_cut_duration, Cut,
};
pub use map::{
    build_gap_map, build_time_map, display_time_map, original_to_output, output_to_original,
    span_at_original, time_map_from_segments, to_regions, DisplayAxis, MappedSpan, Region, TimeMap,
    TimeSpan,
};
pub use segments::{
    derive_seams, derive_segments, plan_delete_segment, plan_split, segment_at, ClipShape,
    DeletePlan, Seam, Segment,
};
pub use speed::{
    clamp_speed, prune_segment_speeds, segment_speed_at, segment_speed_at_time, set_segment_speed,
    SegmentSpeed, MAX_SEGMENT_SPEED, MIN_SEGMENT_SPEED,
};
pub use timestamp::Timestamp;

// No monotonic clock on wasm32: `Instant::now` compiles there and then panics.
#[cfg(all(feature = "clock", not(target_arch = "wasm32")))]
pub use clock::SessionClock;

/// Two times within this are the same boundary. Locked to `EPS` in
/// packages/editor/src/lib/timeline/{cuts,segments,time-map}.ts.
pub const EPS: f64 = 1e-4;
