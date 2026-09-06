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
