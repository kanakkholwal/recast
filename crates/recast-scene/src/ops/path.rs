use serde_json::Value;

/// One step along a path into a [`crate::Scene`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// An object key, already in the camelCase serde uses.
    Key(String),
    /// A position in an array.
    Index(usize),
    /// A layer picked by its `LayerId` rather than its position.
    LayerId(u32),
}

/// A location in a scene as `/`-separated steps, e.g. `layers/id:7/effects/0/amount`. Path strings are a WIRE CONTRACT stored in journals.
/// Address a layer by id in a journal: a position is stable only while the list is, so `layers/2` replayed after an insert silently edits someone else's layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenePath {
    steps: Vec<Step>,
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    Empty,
    /// `layers/id:` with nothing after it, or a non-numeric id.
    BadLayerId(String),
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "an empty path addresses nothing"),
            Self::BadLayerId(s) => write!(f, "'{s}' is not a layer id"),
        }
    }
}

impl std::error::Error for PathError {}

impl ScenePath {
    /// Parses `a/b/2/c`. A leading slash is accepted so a JSON pointer pasted from elsewhere works, and dots are accepted as separators because the v1 journal wrote them that way.
    pub fn parse(text: &str) -> Result<Self, PathError> {
        let normalised = text.trim().trim_start_matches('/').replace('.', "/");
        let mut steps = Vec::new();
        for part in normalised.split('/') {
            if part.is_empty() {
                continue;
            }
            if let Some(id) = part.strip_prefix("id:") {
                let id = id
                    .parse::<u32>()
                    .map_err(|_| PathError::BadLayerId(part.to_string()))?;
                steps.push(Step::LayerId(id));
            } else if let Ok(index) = part.parse::<usize>() {
                steps.push(Step::Index(index));
            } else {
                steps.push(Step::Key(part.to_string()));
            }
        }
        if steps.is_empty() {
            return Err(PathError::Empty);
        }
        Ok(Self {
            steps,
            text: normalised,
        })
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl std::fmt::Display for ScenePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

/// Walks `root` to the value at `path`.
pub fn resolve<'a>(root: &'a Value, path: &ScenePath) -> Option<&'a Value> {
    let mut at = root;
    for step in &path.steps {
        at = step_into(at, step)?;
    }
    Some(at)
}

/// Walks `root` to the value at `path`, for writing. Missing OBJECT keys are created, since a scene serialises with `skip_serializing_if` and setting an absent field would fail.
/// Missing array positions are NOT created, because inventing one would silently move every element after it.
pub fn resolve_mut<'a>(root: &'a mut Value, path: &ScenePath) -> Option<&'a mut Value> {
    let mut at = root;
    for step in &path.steps {
        at = step_into_mut(at, step)?;
    }
    Some(at)
}

fn step_into<'a>(value: &'a Value, step: &Step) -> Option<&'a Value> {
    match step {
        Step::Key(key) => value.get(key),
        Step::Index(index) => value.get(index),
        Step::LayerId(id) => value
            .as_array()?
            .iter()
            .find(|layer| layer.get("id").and_then(Value::as_u64) == Some(*id as u64)),
    }
}

fn step_into_mut<'a>(value: &'a mut Value, step: &Step) -> Option<&'a mut Value> {
    match step {
        Step::Key(key) => {
            let object = value.as_object_mut()?;
            Some(object.entry(key.clone()).or_insert(Value::Null))
        }
        Step::Index(index) => value.get_mut(index),
        Step::LayerId(id) => value
            .as_array_mut()?
            .iter_mut()
            .find(|layer| layer.get("id").and_then(Value::as_u64) == Some(*id as u64)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn path(text: &str) -> ScenePath {
        ScenePath::parse(text).expect("a path")
    }

    #[test]
    fn a_path_splits_into_keys_and_positions() {
        assert_eq!(
            path("layers/2/effects/0/amount").steps(),
            &[
                Step::Key("layers".into()),
                Step::Index(2),
                Step::Key("effects".into()),
                Step::Index(0),
                Step::Key("amount".into()),
            ]
        );
    }

    /// The v1 journal wrote dotted fields, and those strings are on disk.
    #[test]
    fn a_dotted_path_reads_the_same_as_a_slashed_one() {
        assert_eq!(path("output.width"), path("output/width"));
        assert_eq!(path("/output/width"), path("output/width"));
    }

    #[test]
    fn a_layer_id_is_a_step_of_its_own() {
        assert_eq!(
            path("layers/id:7/opacity").steps(),
            &[
                Step::Key("layers".into()),
                Step::LayerId(7),
                Step::Key("opacity".into()),
            ]
        );
    }

    #[test]
    fn a_malformed_layer_id_is_refused_rather_than_read_as_a_key() {
        assert_eq!(
            ScenePath::parse("layers/id:x"),
            Err(PathError::BadLayerId("id:x".into()))
        );
        assert_eq!(ScenePath::parse("  "), Err(PathError::Empty));
        assert_eq!(ScenePath::parse("/"), Err(PathError::Empty));
    }

    #[test]
    fn resolving_walks_objects_and_arrays() {
        let doc = json!({"output": {"width": 1920}, "layers": [{"id": 4, "opacity": 0.5}]});
        assert_eq!(resolve(&doc, &path("output/width")), Some(&json!(1920)));
        assert_eq!(resolve(&doc, &path("layers/0/opacity")), Some(&json!(0.5)));
        assert_eq!(resolve(&doc, &path("layers/1/opacity")), None);
        assert_eq!(resolve(&doc, &path("output/height")), None);
    }

    /// The reason ids exist. Both paths address the same layer here; after an
    /// insert at the front, only one of them still does.
    #[test]
    fn a_layer_id_survives_an_insert_where_a_position_does_not() {
        let mut doc = json!({"layers": [{"id": 4, "opacity": 0.5}, {"id": 9, "opacity": 1.0}]});
        assert_eq!(resolve(&doc, &path("layers/1/opacity")), Some(&json!(1.0)));
        assert_eq!(
            resolve(&doc, &path("layers/id:9/opacity")),
            Some(&json!(1.0))
        );

        let layers = doc["layers"].as_array_mut().unwrap();
        layers.insert(0, json!({"id": 1, "opacity": 0.25}));

        assert_eq!(resolve(&doc, &path("layers/1/opacity")), Some(&json!(0.5)));
        assert_eq!(
            resolve(&doc, &path("layers/id:9/opacity")),
            Some(&json!(1.0))
        );
    }

    /// A scene skips its optional fields when serialising, so a key that is
    /// absent from the JSON is not an unknown field.
    #[test]
    fn writing_creates_a_missing_object_key() {
        let mut doc = json!({"output": {}});
        *resolve_mut(&mut doc, &path("output/width")).expect("a slot") = json!(1280);
        assert_eq!(doc, json!({"output": {"width": 1280}}));
    }

    /// Inventing an array position would shift every element after it.
    #[test]
    fn writing_does_not_create_a_missing_array_position() {
        let mut doc = json!({"layers": []});
        assert!(resolve_mut(&mut doc, &path("layers/0/opacity")).is_none());
        assert_eq!(doc, json!({"layers": []}));
    }

    #[test]
    fn writing_through_a_layer_id_finds_the_right_layer() {
        let mut doc = json!({"layers": [{"id": 4, "opacity": 0.5}, {"id": 9, "opacity": 1.0}]});
        *resolve_mut(&mut doc, &path("layers/id:9/opacity")).expect("a slot") = json!(0.2);
        assert_eq!(doc["layers"][0]["opacity"], json!(0.5));
        assert_eq!(doc["layers"][1]["opacity"], json!(0.2));
    }

    #[test]
    fn an_unknown_layer_id_resolves_to_nothing() {
        let mut doc = json!({"layers": [{"id": 4}]});
        assert!(resolve(&doc, &path("layers/id:99/opacity")).is_none());
        assert!(resolve_mut(&mut doc, &path("layers/id:99/opacity")).is_none());
    }
}
