//! `.recast` format v2 layout, edits split/merge, and canonical serialization.
//!
//! v2 explodes the old single `edits.json` into a `project.json` manifest plus
//! per-concern `edits/<section>.json` files, alongside `assets/` media.
//!
//! The split is a key→section grouping table, not a type mirror: `RenderState`
//! has a `#[serde(flatten)] passthrough`, so regrouping the flat camelCase key
//! space round-trips losslessly. The reader merges sections back into one
//! `edits.json` in the cache, leaving the export/thumbnail pipeline untouched.

use std::collections::BTreeMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Bundle layout version written into `project.json`.
pub const FORMAT_VERSION: u32 = 2;
/// Manifest marker string.
pub const FORMAT_TAG: &str = "recast-project";

pub const MANIFEST_NAME: &str = "project.json";
pub const METADATA_NAME: &str = "metadata.json";

// Asset entry names (under `assets/`).
pub const ASSET_VIDEO: &str = "assets/recording.mp4";
pub const ASSET_AUDIO: &str = "assets/audio.wav";
pub const ASSET_MICROPHONE: &str = "assets/microphone.wav";
pub const ASSET_CAMERA: &str = "assets/camera.mp4";
pub const ASSET_CURSOR_TRACK: &str = "assets/cursor.track.json";

// Edit section names.
pub const SECTION_FRAME: &str = "frame";
pub const SECTION_CURSOR: &str = "cursor";
pub const SECTION_ZOOM: &str = "zoom";
pub const SECTION_ANNOTATIONS: &str = "annotations";
pub const SECTION_TIMELINE: &str = "timeline";
pub const SECTION_AUDIO: &str = "audio";
pub const SECTION_OVERLAYS: &str = "overlays";

/// All sections, in a stable declared order. The writer emits every section
/// (even if it holds only its `version`) so the bundle layout is consistent.
pub const SECTIONS: [&str; 7] = [
    SECTION_FRAME,
    SECTION_CURSOR,
    SECTION_ZOOM,
    SECTION_ANNOTATIONS,
    SECTION_TIMELINE,
    SECTION_AUDIO,
    SECTION_OVERLAYS,
];

/// Schema version of a given section. All sections start at 1; bump
/// independently when a section's shape changes, alongside a migrator.
pub fn section_version(_section: &str) -> u32 {
    1
}

/// Path of a section file inside the archive.
pub fn section_path(section: &str) -> String {
    format!("edits/{section}.json")
}

/// The section that owns a given flat camelCase edits key. The grouping table.
/// Unrecognised keys fall back to `frame`, so a new editor toggle is never
/// dropped on round-trip even before this table learns about it.
pub fn section_for_key(key: &str) -> &'static str {
    // Export-time cursor sprites are bulky data URIs, kept out of cursor settings; checked first, since they also start 'cursor'.
    if key.starts_with("cursorSprite") {
        return SECTION_OVERLAYS;
    }
    match key {
        "zoomRegions" | "autoZoomEnabled" | "autoZoomApplied" => SECTION_ZOOM,
        "annotations" | "annotationsEnabled" => SECTION_ANNOTATIONS,
        "cuts" | "cutsEnabled" | "splitPoints" | "dismissedSilences" => SECTION_TIMELINE,
        "audioSettings" => SECTION_AUDIO,
        "cameraOverlay" | "watermarkSettings" => SECTION_OVERLAYS,
        "trimStart"
        | "trimEnd"
        | "outputAspect"
        | "backgroundType"
        | "backgroundValue"
        | "backgroundBlur"
        | "padding"
        | "borderRadius"
        | "shadow"
        | "layoutMode"
        | "lastAppliedPresetId"
        | "focusEnabled" => SECTION_FRAME,
        _ if key.starts_with("cursor") => SECTION_CURSOR,
        _ => SECTION_FRAME,
    }
}

/// Split a flat edits object into `{section -> {version, …keys}}`. Every section
/// is present; a section with no keys carries just its `version`.
pub fn split_edits(flat: &Value) -> BTreeMap<&'static str, Value> {
    let mut buckets: BTreeMap<&'static str, Map<String, Value>> = BTreeMap::new();
    for section in SECTIONS {
        let mut map = Map::new();
        map.insert("version".to_string(), Value::from(section_version(section)));
        buckets.insert(section, map);
    }
    if let Some(obj) = flat.as_object() {
        for (key, value) in obj {
            let section = section_for_key(key);
            buckets
                .get_mut(section)
                .expect("every section pre-seeded")
                .insert(key.clone(), value.clone());
        }
    }
    buckets
        .into_iter()
        .map(|(section, map)| (section, Value::Object(map)))
        .collect()
}

/// Fan the section objects back into one flat edits object (dropping each
/// section's `version`). Inverse of `split_edits`.
pub fn merge_sections<I, S>(sections: I) -> Value
where
    I: IntoIterator<Item = (S, Value)>,
    S: AsRef<str>,
{
    let mut merged = Map::new();
    for (_name, value) in sections {
        if let Some(obj) = value.as_object() {
            for (key, val) in obj {
                if key == "version" {
                    continue;
                }
                merged.insert(key.clone(), val.clone());
            }
        }
    }
    Value::Object(merged)
}

/// Canonicalize a value for diff-stable output: object keys are already sorted
/// (serde_json `Map` is a `BTreeMap` — no `preserve_order` feature here), and
/// this additionally sorts arrays whose every element is an object with a
/// string `id` by that id, so reordering an array never produces a spurious
/// diff. Recurses into nested structures.
pub fn canonicalize(value: Value) -> Value {
    match value {
        Value::Array(items) => {
            let mut mapped: Vec<Value> = items.into_iter().map(canonicalize).collect();
            let all_have_id = !mapped.is_empty()
                && mapped
                    .iter()
                    .all(|v| v.get("id").and_then(Value::as_str).is_some());
            if all_have_id {
                mapped.sort_by(|a, b| {
                    let ka = a.get("id").and_then(Value::as_str).unwrap_or("");
                    let kb = b.get("id").and_then(Value::as_str).unwrap_or("");
                    ka.cmp(kb)
                });
            }
            Value::Array(mapped)
        }
        Value::Object(map) => {
            let mut out = Map::new();
            for (key, val) in map {
                out.insert(key, canonicalize(val));
            }
            Value::Object(out)
        }
        other => other,
    }
}

/// Pretty-print a value canonically (sorted keys, id-sorted arrays) with a
/// trailing newline. This is what makes section files diff cleanly in git.
pub fn to_canonical_string(value: &Value) -> Result<String> {
    let canon = canonicalize(value.clone());
    Ok(serde_json::to_string_pretty(&canon)? + "\n")
}

/// `project.json` — links capture metadata, assets, and edit sections.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectManifest {
    pub format: String,
    pub format_version: u32,
    pub created_at_unix_ms: u64,
    pub capture: String,
    pub assets: ManifestAssets,
    /// section name -> archive path.
    pub edits: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestAssets {
    pub video: String,
    #[serde(default)]
    pub audio: Option<String>,
    #[serde(default)]
    pub microphone: Option<String>,
    #[serde(default)]
    pub camera: Option<String>,
    #[serde(default)]
    pub cursor_track: Option<String>,
}

impl ProjectManifest {
    /// Build a manifest for the given present assets. `created_at_unix_ms`
    /// mirrors the capture metadata so the bundle has one creation stamp.
    pub fn new(
        created_at_unix_ms: u64,
        has_audio: bool,
        has_microphone: bool,
        has_camera: bool,
        has_cursor_track: bool,
    ) -> Self {
        let mut edits = BTreeMap::new();
        for section in SECTIONS {
            edits.insert(section.to_string(), section_path(section));
        }
        ProjectManifest {
            format: FORMAT_TAG.to_string(),
            format_version: FORMAT_VERSION,
            created_at_unix_ms,
            capture: METADATA_NAME.to_string(),
            assets: ManifestAssets {
                video: ASSET_VIDEO.to_string(),
                audio: has_audio.then(|| ASSET_AUDIO.to_string()),
                microphone: has_microphone.then(|| ASSET_MICROPHONE.to_string()),
                camera: has_camera.then(|| ASSET_CAMERA.to_string()),
                cursor_track: has_cursor_track.then(|| ASSET_CURSOR_TRACK.to_string()),
            },
            edits,
        }
    }
}

/// A zip is v2 iff it carries a `project.json` manifest. Anything else (a root
/// `edits.json`) is treated as v1 and routed through migration.
pub fn is_v2(entry_names: &[String]) -> bool {
    entry_names.iter().any(|n| n == MANIFEST_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_flat() -> Value {
        json!({
            "trimStart": 0.0,
            "trimEnd": 42.5,
            "padding": 6,
            "backgroundType": "gradient",
            "cursorEnabled": true,
            "cursorSmoothing": 60,
            "cursorMotionEasing": { "x1": 0.4, "y1": 0.0, "x2": 0.2, "y2": 1.0 },
            "cursorSpriteRest": "data:image/png;base64,AAAA",
            "zoomRegions": [
                { "id": "z2", "start": 12.0, "scale": 2.2 },
                { "id": "z1", "start": 2.0, "scale": 1.8 }
            ],
            "autoZoomEnabled": true,
            "annotations": [{ "id": "a1", "kind": "rect" }],
            "annotationsEnabled": true,
            "cuts": [{ "id": "c1", "start": 8.0, "end": 9.2 }],
            "splitPoints": [15.0, 30.0],
            "audioSettings": { "volume": 1.0, "muted": false },
            "cameraOverlay": { "enabled": true },
            "watermarkSettings": { "enabled": false },
            "someFutureToggle": 123
        })
    }

    #[test]
    fn split_then_merge_round_trips() {
        let flat = sample_flat();
        let sections = split_edits(&flat);
        // Every section present.
        assert_eq!(sections.len(), SECTIONS.len());
        // Each carries a version.
        for section in SECTIONS {
            assert_eq!(sections[section]["version"], json!(1));
        }
        let merged = merge_sections(sections.into_iter().map(|(k, v)| (k.to_string(), v)));
        assert_eq!(merged, flat, "split→merge must be lossless");
    }

    #[test]
    fn keys_route_to_expected_sections() {
        assert_eq!(section_for_key("trimStart"), SECTION_FRAME);
        assert_eq!(section_for_key("cursorSmoothing"), SECTION_CURSOR);
        assert_eq!(section_for_key("cursorMotionEasing"), SECTION_CURSOR);
        assert_eq!(section_for_key("cursorSpriteRest"), SECTION_OVERLAYS);
        assert_eq!(section_for_key("zoomRegions"), SECTION_ZOOM);
        assert_eq!(section_for_key("annotations"), SECTION_ANNOTATIONS);
        assert_eq!(section_for_key("splitPoints"), SECTION_TIMELINE);
        assert_eq!(section_for_key("audioSettings"), SECTION_AUDIO);
        assert_eq!(section_for_key("cameraOverlay"), SECTION_OVERLAYS);
        // Unknown key → fallback.
        assert_eq!(section_for_key("someFutureToggle"), SECTION_FRAME);
    }

    #[test]
    fn future_key_survives_round_trip_via_fallback() {
        let sections = split_edits(&sample_flat());
        let merged = merge_sections(sections.into_iter().map(|(k, v)| (k.to_string(), v)));
        assert_eq!(merged["someFutureToggle"], json!(123));
    }

    #[test]
    fn canonicalize_sorts_id_arrays() {
        let sections = split_edits(&sample_flat());
        let zoom = canonicalize(sections[SECTION_ZOOM].clone());
        let ids: Vec<&str> = zoom["zoomRegions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec!["z1", "z2"], "id arrays sorted ascending");
    }

    #[test]
    fn canonical_string_is_order_independent() {
        // Two objects with the same content but different insertion order must serialize identically.
        let a = json!({ "b": 1, "a": 2, "arr": [{ "id": "y" }, { "id": "x" }] });
        let b = json!({ "a": 2, "arr": [{ "id": "x" }, { "id": "y" }], "b": 1 });
        assert_eq!(
            to_canonical_string(&a).unwrap(),
            to_canonical_string(&b).unwrap()
        );
        // And it ends in a newline.
        assert!(to_canonical_string(&a).unwrap().ends_with("}\n"));
    }

    #[test]
    fn manifest_reflects_present_assets() {
        let m = ProjectManifest::new(123, true, false, true, true);
        assert_eq!(m.format, FORMAT_TAG);
        assert_eq!(m.format_version, FORMAT_VERSION);
        assert_eq!(m.assets.video, ASSET_VIDEO);
        assert_eq!(m.assets.audio.as_deref(), Some(ASSET_AUDIO));
        assert_eq!(m.assets.microphone, None);
        assert_eq!(m.assets.camera.as_deref(), Some(ASSET_CAMERA));
        assert_eq!(m.edits.len(), SECTIONS.len());
        assert_eq!(m.edits["frame"], "edits/frame.json");
    }

    #[test]
    fn detects_v2_by_manifest_presence() {
        assert!(is_v2(&["project.json".into(), "metadata.json".into()]));
        assert!(!is_v2(&["edits.json".into(), "metadata.json".into()]));
    }
}
