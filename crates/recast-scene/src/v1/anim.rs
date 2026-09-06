use serde::{Deserialize, Serialize};

use super::easing::Easing;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneAnimSpec {
    pub kind: String,
    pub duration_ms: f64,
    #[serde(default)]
    pub easing: Easing,
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default)]
    pub intensity: Option<f64>,
}

/// Entrance/exit animation anchored to a segment's ORIGINAL start time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentAnim {
    pub start: f64,
    #[serde(rename = "in", default)]
    pub anim_in: Option<SceneAnimSpec>,
    #[serde(rename = "out", default)]
    pub anim_out: Option<SceneAnimSpec>,
}
