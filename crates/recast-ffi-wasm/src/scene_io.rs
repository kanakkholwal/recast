use recast_scene::v1::RenderState;
use recast_scene::Scene;

#[derive(Debug)]
pub enum SceneParseError {
    NotJson(String),
    NotAScene(String),
}

impl std::fmt::Display for SceneParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotJson(e) => write!(f, "scene payload is not JSON: {e}"),
            Self::NotAScene(e) => write!(f, "scene payload is neither a Scene nor a v1 state: {e}"),
        }
    }
}

impl std::error::Error for SceneParseError {}

/// A v1 state kept as JSON so a later patch (a few changed top-level fields) can be merged in without the editor
/// serialising the whole state again. The edit path's cost is then the patch, the merge and one migration.
#[derive(Debug, Clone, Default)]
pub struct PatchableState {
    last: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug)]
pub enum PatchError {
    NoBase,
    NotAnObject,
    Json(String),
    State(String),
}

impl std::fmt::Display for PatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBase => write!(f, "patch before any full state was set"),
            Self::NotAnObject => {
                write!(f, "a patch must be a JSON object of top-level state fields")
            }
            Self::Json(e) => write!(f, "patch is not JSON: {e}"),
            Self::State(e) => write!(f, "patched state does not deserialise: {e}"),
        }
    }
}

impl std::error::Error for PatchError {}

impl PatchableState {
    /// Remembers a full state; a `Scene` payload clears it, since a scene cannot take a state patch.
    pub fn remember(&mut self, json: &str) {
        self.last = serde_json::from_str::<serde_json::Value>(json)
            .ok()
            .and_then(|v| match v {
                serde_json::Value::Object(map)
                    if map.get("schema").is_none() || map.get("layers").is_none() =>
                {
                    Some(map)
                }
                _ => None,
            });
    }

    /// Merges `patch` (top-level keys replace) into the remembered state and returns the result, keeping it as the new base.
    /// # Errors When nothing was remembered, the patch is not an object, or the merged state does not deserialise.
    pub fn apply(&mut self, patch: &str) -> Result<RenderState, PatchError> {
        let base = self.last.as_mut().ok_or(PatchError::NoBase)?;
        let patch: serde_json::Value =
            serde_json::from_str(patch).map_err(|e| PatchError::Json(e.to_string()))?;
        let serde_json::Value::Object(fields) = patch else {
            return Err(PatchError::NotAnObject);
        };
        let mut merged = base.clone();
        for (key, value) in fields {
            if value.is_null() {
                merged.remove(&key);
            } else {
                merged.insert(key, value);
            }
        }
        let state: RenderState = serde_json::from_value(serde_json::Value::Object(merged.clone()))
            .map_err(|e| PatchError::State(e.to_string()))?;
        *base = merged;
        Ok(state)
    }

    #[cfg(test)]
    pub fn has_base(&self) -> bool {
        self.last.is_some()
    }
}

/// Accepts either a `Scene` or a v1 `RenderState`, so the editor can hand over
/// whichever it holds while the migration is in flight. Kept out of the
/// `wasm_bindgen` layer so it is testable on the host.
pub fn parse_scene(json: &str) -> Result<Scene, SceneParseError> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| SceneParseError::NotJson(e.to_string()))?;

    if value.get("schema").is_some() && value.get("layers").is_some() {
        return serde_json::from_value(value)
            .map_err(|e| SceneParseError::NotAScene(e.to_string()));
    }

    let state: RenderState =
        serde_json::from_value(value).map_err(|e| SceneParseError::NotAScene(e.to_string()))?;
    Ok(recast_scene::migrate::to_scene(&state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use recast_scene::LayerSource;

    const V1: &str = r##"{
        "trimStart": 0.0, "trimEnd": 10.0,
        "backgroundType": "color", "backgroundValue": "#0f172a", "backgroundBlur": 0.0,
        "padding": 4.0, "cursorEnabled": false, "cursorSize": 3.0, "cursorSmoothing": 50.0,
        "cursorHighlightClicks": true, "cursorHighlightColor": "#3b82f6",
        "cursorHighlightOpacity": 40.0, "cursorHideWhenIdle": false, "cursorIdleTimeout": 3.0,
        "zoomRegions": []
    }"##;

    #[test]
    fn a_v1_render_state_is_migrated_on_the_way_in() {
        let scene = parse_scene(V1).expect("scene");
        assert!(scene.output.padding == 4.0);
        assert!(scene
            .layers
            .iter()
            .any(|l| matches!(l.source, LayerSource::Screen)));
    }

    #[test]
    fn a_scene_round_trips_without_going_through_the_migration() {
        let scene = parse_scene(V1).expect("scene");
        let json = serde_json::to_string(&scene).expect("serialize");
        let again = parse_scene(&json).expect("scene");
        assert_eq!(scene, again);
    }

    #[test]
    fn a_patch_replaces_top_level_fields_and_needs_a_base() {
        let mut state = PatchableState::default();
        assert!(matches!(
            state.apply(r#"{"padding": 9}"#),
            Err(PatchError::NoBase)
        ));
        state.remember(V1);
        let patched = state
            .apply(r#"{"padding": 9, "trimEnd": 8.0}"#)
            .expect("patch");
        assert_eq!(patched.padding, 9.0);
        assert_eq!(patched.trim_end, 8.0);
        let again = state
            .apply(r#"{"padding": 3}"#)
            .expect("second patch on the merged base");
        assert_eq!(again.trim_end, 8.0, "earlier patches stay merged");
        assert!(matches!(state.apply("[1]"), Err(PatchError::NotAnObject)));
        assert!(matches!(
            state.apply(r#"{"trimEnd": "no"}"#),
            Err(PatchError::State(_))
        ));
        assert_eq!(
            state.apply(r#"{}"#).expect("empty").trim_end,
            8.0,
            "a refused patch left the base intact"
        );
        state.remember(r#"{"schema":2,"layers":[]}"#);
        assert!(
            !state.has_base(),
            "a scene payload cannot take a state patch"
        );
    }

    #[test]
    fn malformed_json_is_reported_as_such() {
        let err = parse_scene("{not json").expect_err("should fail");
        assert!(matches!(err, SceneParseError::NotJson(_)));
    }

    #[test]
    fn json_that_is_neither_shape_is_reported_separately() {
        let err = parse_scene(r#"{"hello":"world"}"#).expect_err("should fail");
        assert!(matches!(err, SceneParseError::NotAScene(_)));
    }

    /// A `Scene` missing `layers` must NOT be silently reinterpreted as a v1
    /// state and migrated into an empty scene; that would swallow a real bug.
    #[test]
    fn a_scene_shaped_payload_missing_layers_fails_rather_than_migrating() {
        let err = parse_scene(r#"{"schema":2,"layers":[]}"#).expect_err("should fail");
        assert!(matches!(err, SceneParseError::NotAScene(_)));
    }
}
