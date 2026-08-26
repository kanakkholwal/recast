use serde::{Deserialize, Serialize};

/// One transcribed word with its own source-time span, in seconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptWord {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// Word-by-word animation. The string fields mirror the TypeScript unions
/// rather than being enums, so an unknown value from a newer project falls
/// through to the default arm instead of failing the whole deserialize.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionAnimation {
    pub chunk: String,
    pub chunk_size: u32,
    pub emphasis: String,
    pub emphasis_color: String,
    /// Absent in a project saved before this field existed, which resolves to
    /// "active" so those projects keep the look they were saved with.
    #[serde(default)]
    pub highlight: Option<String>,
    pub entrance: String,
    pub entrance_ms: f64,
    pub hold_gaps: bool,
}

impl Default for CaptionAnimation {
    /// Mirrors `DEFAULT_CAPTION_ANIMATION` in @recast/captions.
    fn default() -> Self {
        Self {
            chunk: "line".into(),
            chunk_size: 3,
            emphasis: "none".into(),
            emphasis_color: "#facc15".into(),
            highlight: Some("none".into()),
            entrance: "none".into(),
            entrance_ms: 220.0,
            hold_gaps: true,
        }
    }
}

impl CaptionAnimation {
    /// True when the spec has no visible effect, so a renderer can take its
    /// one-event-per-line path. Mirrors `isStaticAnimation`.
    pub fn is_static(&self) -> bool {
        self.chunk == "line"
            && self.emphasis == "none"
            && self.highlight() == "none"
            && self.entrance == "none"
    }

    pub fn highlight(&self) -> &str {
        self.highlight.as_deref().unwrap_or("active")
    }
}

/// How captions render over the video. Deserialized from the render state's
/// `captionStyle`, and mirrored field for field by the TypeScript type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionStyle {
    pub enabled: bool,
    pub font_family: String,
    pub font_weight: u32,
    pub font_size_pct: f64,
    pub position: String,
    pub align: String,
    pub offset_pct: f64,
    pub color: String,
    #[serde(default = "default_muted_color")]
    pub muted_color: String,
    pub uppercase: bool,
    pub letter_spacing: f64,
    pub background: String,
    pub background_color: String,
    pub background_opacity: f64,
    #[serde(default = "default_box_padding_x")]
    pub box_padding_x_em: f64,
    #[serde(default = "default_box_padding_y")]
    pub box_padding_y_em: f64,
    #[serde(default = "default_box_radius")]
    pub box_radius_em: f64,
    #[serde(default = "default_line_height")]
    pub line_height: f64,
    pub outline_width: f64,
    pub outline_color: String,
    pub max_lines: u32,
    #[serde(default = "default_max_chars_per_line")]
    pub max_chars_per_line: u32,
    #[serde(default)]
    pub animation: Option<CaptionAnimation>,
}

fn default_muted_color() -> String {
    "#a1a1aa".into()
}
fn default_box_padding_x() -> f64 {
    0.7
}
fn default_box_padding_y() -> f64 {
    0.32
}
fn default_box_radius() -> f64 {
    0.6
}
fn default_line_height() -> f64 {
    1.35
}
fn default_max_chars_per_line() -> u32 {
    42
}

impl Default for CaptionStyle {
    /// Mirrors `DEFAULT_CAPTION_STYLE` in @recast/captions (the Loom preset).
    fn default() -> Self {
        Self {
            enabled: true,
            font_family: "'Inter', sans-serif".into(),
            font_weight: 600,
            font_size_pct: 3.8,
            position: "bottom".into(),
            align: "center".into(),
            offset_pct: 8.0,
            color: "#ffffff".into(),
            muted_color: default_muted_color(),
            uppercase: false,
            letter_spacing: 0.0,
            background: "box".into(),
            background_color: "#0b0b12".into(),
            background_opacity: 78.0,
            box_padding_x_em: default_box_padding_x(),
            box_padding_y_em: default_box_padding_y(),
            box_radius_em: default_box_radius(),
            line_height: default_line_height(),
            outline_width: 0.0,
            outline_color: "#0a0a0a".into(),
            max_lines: 2,
            max_chars_per_line: default_max_chars_per_line(),
            animation: Some(CaptionAnimation {
                chunk: "phrase".into(),
                chunk_size: 6,
                emphasis: "none".into(),
                emphasis_color: "#ffffff".into(),
                highlight: Some("progressive".into()),
                entrance: "slide".into(),
                entrance_ms: 125.0,
                hold_gaps: true,
            }),
        }
    }
}
