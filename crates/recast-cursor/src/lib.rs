#![forbid(unsafe_code)]

pub mod anim;
pub mod sample;
pub mod smooth;
pub mod track;

pub use anim::{
    build_press_events_from_iter, click_anchor_at, click_highlight_at, press_state_at, PressEvent,
    PressFrameState, CLICK_SNAP_HALF_US,
};
pub use sample::{ClickAnchor, CursorSample, IdlePeriod};
pub use smooth::{
    smooth_cursor_path, smoothing_strength_to_sigma_ms, SmoothResult, SmoothingOptions,
};
pub use track::{
    idle_alpha_at, interpolate_at, CursorPlacement, CursorSettings, CursorTrack, Highlight,
    IDLE_FADE_US,
};
