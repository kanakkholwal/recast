use recast_color::{Gradient, Srgba};
use recast_cursor::CursorTrack;
use recast_time::{
    build_time_map, clamp_speed, derive_segments, segment_speed_at, time_map_from_segments,
    ClipShape, Cut, Segment, SegmentSpeed, TimeMap, TimeSpan,
};
use serde::{Deserialize, Serialize};

use crate::v1::easing::Easing;
use crate::v1::nodes::{
    Annotation, AudioClip, AudioSettings, CameraOverlaySettings, ShadowSettings, ZoomRegion,
};
use crate::v1::{SegmentAnim, SegmentSpeed as V1SegmentSpeed};

pub const SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LayerId(pub u32);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub schema: u32,
    pub output: OutputSpec,
    pub timeline: Timeline,
    pub layers: Vec<Layer>,
    pub audio: AudioGraph,
    /// The recorded pointer path. Kept out of the layer list because it is a
    /// captured signal, not an authored one: the cursor LAYER holds the
    /// settings, this holds the samples they are applied to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_track: Option<CursorTrack>,
    #[serde(default)]
    pub flags: SceneFlags,
    /// Editor-owned keys the engine never reads. Carried so a round trip
    /// through the engine cannot reset a user's settings.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub passthrough: serde_json::Map<String, serde_json::Value>,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            schema: SCHEMA_VERSION,
            output: OutputSpec::default(),
            timeline: Timeline::default(),
            layers: Vec::new(),
            audio: AudioGraph::default(),
            cursor_track: None,
            flags: SceneFlags::default(),
            passthrough: serde_json::Map::new(),
        }
    }
}

/// Lane master switches the editor owns. The effects stay authored while their
/// lane is off, so these gate evaluation rather than dropping scene data.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneFlags {
    /// The zoom lane. Also gates the camera bubble's zoom-follow.
    #[serde(default = "enabled")]
    pub focus: bool,
    #[serde(default = "enabled")]
    pub annotations: bool,
}

impl Default for SceneFlags {
    fn default() -> Self {
        Self {
            focus: true,
            annotations: true,
        }
    }
}

fn enabled() -> bool {
    true
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputSpec {
    /// `None` matches the source dimensions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect: Option<String>,
    /// Percent of the shorter source edge.
    #[serde(default)]
    pub padding: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    pub trim_start: f64,
    pub trim_end: f64,
    #[serde(default)]
    pub cuts: Vec<TimelineCut>,
    #[serde(default)]
    pub split_points: Vec<f64>,
    #[serde(default)]
    pub segment_speeds: Vec<V1SegmentSpeed>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineCut {
    pub start: f64,
    pub end: f64,
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Timeline {
    pub fn segments(&self) -> Vec<Segment> {
        derive_segments(&ClipShape {
            trim_start: self.trim_start,
            trim_end: self.trim_end,
            cuts: self.cuts.iter().map(|c| Cut::new(c.start, c.end)).collect(),
            split_points: self.split_points.clone(),
        })
    }

    /// The one source-to-output projection. Everything downstream reads this
    /// rather than re-deriving cuts and speeds for itself.
    pub fn time_map(&self) -> TimeMap {
        let segments = self.segments();
        let overrides: Vec<SegmentSpeed> = self
            .segment_speeds
            .iter()
            .map(|s| SegmentSpeed {
                start: s.start,
                speed: s.speed,
            })
            .collect();
        time_map_from_segments(&segments, |index| {
            let start = segments
                .iter()
                .find(|s| s.index == index)
                .map(|s| s.start)
                .unwrap_or(0.0);
            clamp_speed(segment_speed_at(&overrides, start))
        })
    }

    /// The projection the editor sent with an export, replayed verbatim. Used
    /// in place of `time_map` when a payload carries a resolved map, so the
    /// engine never recomputes an axis the editor already resolved.
    pub fn time_map_from_wire(spans: &[TimeSpan]) -> TimeMap {
        build_time_map(spans)
    }

    pub fn output_duration(&self) -> f64 {
        self.time_map().output_duration
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub id: LayerId,
    pub source: LayerSource,
    #[serde(default)]
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub blend: BlendMode,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default = "unit")]
    pub opacity: f64,
}

fn unit() -> f64 {
    1.0
}

impl Layer {
    pub fn new(id: u32, source: LayerSource) -> Self {
        Self {
            id: LayerId(id),
            source,
            effects: Vec::new(),
            blend: BlendMode::Normal,
            hidden: false,
            opacity: 1.0,
        }
    }

    pub fn with_effects(mut self, effects: Vec<Effect>) -> Self {
        self.effects = effects;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LayerSource {
    Screen,
    Camera(Box<CameraOverlaySettings>),
    Cursor(Box<CursorSpec>),
    Annotation(Box<Annotation>),
    Solid {
        color: Srgba,
    },
    Gradient {
        gradient: Gradient,
    },
    /// A file-backed background: `wallpaper` and `image` both land here, the
    /// discriminator kept so the round trip restores the original tag.
    Asset {
        kind: String,
        value: String,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlendMode {
    #[default]
    Normal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Effect {
    Zoom(Box<ZoomRegion>),
    CornerRadius { percent: f64 },
    DropShadow(Box<ShadowSettings>),
    Blur { amount: f64 },
    SceneAnim(Box<SegmentAnim>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorSpec {
    pub size: f64,
    pub smoothing: f64,
    pub snap_to_clicks: bool,
    pub snap_window_ms: f64,
    pub highlight_clicks: bool,
    pub highlight_color: String,
    pub highlight_opacity: f64,
    pub hide_when_idle: bool,
    pub idle_timeout: f64,
    pub motion_blur: f64,
    pub click_bounce: f64,
    pub bounce_speed_ms: f64,
    pub sway: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite_rest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite_press: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite_right_press: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite_drag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite_hotspot_rest: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite_hotspot_press: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite_hotspot_right_press: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite_hotspot_drag: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite_size_px: Option<f64>,
    /// Reshapes the interpolation parameter between two captured samples.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion_easing: Option<Easing>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioGraph {
    pub settings: AudioSettings,
    #[serde(default)]
    pub clips: Vec<AudioClip>,
}

impl Scene {
    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    pub fn next_layer_id(&self) -> LayerId {
        LayerId(
            self.layers
                .iter()
                .map(|l| l.id.0)
                .max()
                .map_or(0, |m| m + 1),
        )
    }

    /// The screen layer, which every v1 project has exactly one of.
    pub fn screen_layer(&self) -> Option<&Layer> {
        self.layers
            .iter()
            .find(|l| matches!(l.source, LayerSource::Screen))
    }

    pub fn zoom_regions(&self) -> Vec<&ZoomRegion> {
        self.layers
            .iter()
            .flat_map(|l| &l.effects)
            .filter_map(|e| match e {
                Effect::Zoom(z) => Some(&**z),
                _ => None,
            })
            .collect()
    }

    pub fn annotations(&self) -> Vec<&Annotation> {
        self.layers
            .iter()
            .filter_map(|l| match &l.source {
                LayerSource::Annotation(a) => Some(&**a),
                _ => None,
            })
            .collect()
    }
}
