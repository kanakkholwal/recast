use serde::{Deserialize, Serialize};

/// One raw cursor reading. Field names match the recorded track JSON, which is
/// what the editor loads and hands across.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorSample {
    pub timestamp_us: u64,
    pub x: f64,
    pub y: f64,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default)]
    pub left_down: bool,
    #[serde(default)]
    pub right_down: bool,
}

fn yes() -> bool {
    true
}

/// A stretch where the cursor did not move, used to hide it when idle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdlePeriod {
    pub start_us: u64,
    pub end_us: u64,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClickAnchor {
    pub timestamp_us: u64,
    pub x: f64,
    pub y: f64,
}
