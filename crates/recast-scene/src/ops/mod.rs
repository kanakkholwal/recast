//! Typed edits against a [`Scene`].
//!
//! Every op is pure and deterministic: the same op on the same scene produces
//! the same scene and the same wire result, on any machine. That is what lets a
//! journal be replayed, an agent work on a branch, and undo be a fold rather
//! than a snapshot.
//!
//! Variant names, field names and path strings are all a WIRE CONTRACT. They are
//! stored in journals on disk, so renaming one invalidates every journal already
//! written.

pub mod path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::{Effect, Layer, LayerId, Scene};
pub use path::{PathError, ScenePath, Step};

#[derive(Debug, Clone, PartialEq)]
pub enum OpError {
    /// The path does not name anything in this scene.
    NotFound(String),
    /// The path is not well formed.
    BadPath(PathError),
    /// The value does not fit the field the path names.
    TypeMismatch { path: String, detail: String },
    /// No layer carries this id.
    NoSuchLayer(u32),
    /// A layer has no effect at that position.
    NoSuchEffect { layer: u32, index: usize },
    /// A position outside the layer list.
    BadPosition { index: usize, len: usize },
    /// The scene itself would not serialise, which is a bug in the model.
    NotSerializable(String),
    /// A v1 edit was asked for on a scene the v1 model cannot hold.
    NotV1Representable,
}

impl std::fmt::Display for OpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(p) => write!(f, "nothing at '{p}'"),
            Self::BadPath(e) => write!(f, "{e}"),
            Self::TypeMismatch { path, detail } => {
                write!(f, "the value at '{path}' does not fit: {detail}")
            }
            Self::NoSuchLayer(id) => write!(f, "no layer with id {id}"),
            Self::NoSuchEffect { layer, index } => {
                write!(f, "layer {layer} has no effect at {index}")
            }
            Self::BadPosition { index, len } => {
                write!(f, "position {index} is outside a list of {len}")
            }
            Self::NotSerializable(e) => write!(f, "the scene did not serialise: {e}"),
            Self::NotV1Representable => {
                write!(f, "this scene holds more than the v1 model can carry")
            }
        }
    }
}

impl std::error::Error for OpError {}

/// One edit to a scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Op {
    /// Writes one value at one path. The escape hatch, and the only op most
    /// field edits need.
    Set {
        path: String,
        value: Value,
    },
    /// Shallow field merge at a path, so an edit to two fields of one object is
    /// one op rather than two.
    Merge {
        path: String,
        patch: Map<String, Value>,
    },
    /// Appends a layer, which lands on top: the index IS the z-order.
    LayerAdd {
        layer: Box<Layer>,
    },
    LayerRemove {
        id: u32,
    },
    /// Moves a layer to `to`, shifting the rest. This is how z-order is edited.
    LayerMove {
        id: u32,
        to: usize,
    },
    EffectAdd {
        layer: u32,
        effect: Box<Effect>,
    },
    EffectRemove {
        layer: u32,
        index: usize,
    },
}

/// Applies one op in place, returning the verb's wire result.
///
/// On error the scene is left EXACTLY as it was. Every variant either validates
/// before mutating or works on a copy, because a journal that half-applies an op
/// cannot be replayed.
pub fn apply(scene: &mut Scene, op: &Op) -> Result<Value, OpError> {
    match op {
        Op::Set { path, value } => {
            let parsed = ScenePath::parse(path).map_err(OpError::BadPath)?;
            let mut doc = to_value(scene)?;
            let slot = path::resolve_mut(&mut doc, &parsed)
                .ok_or_else(|| OpError::NotFound(path.clone()))?;
            *slot = value.clone();
            *scene = from_value(doc, path)?;
            Ok(json!({ "applied": true, "path": path }))
        }

        Op::Merge { path, patch } => {
            let parsed = ScenePath::parse(path).map_err(OpError::BadPath)?;
            let mut doc = to_value(scene)?;
            let slot = path::resolve_mut(&mut doc, &parsed)
                .ok_or_else(|| OpError::NotFound(path.clone()))?;
            let object = slot.as_object_mut().ok_or_else(|| OpError::TypeMismatch {
                path: path.clone(),
                detail: "merge needs an object".into(),
            })?;
            for (key, value) in patch {
                object.insert(key.clone(), value.clone());
            }
            *scene = from_value(doc, path)?;
            Ok(json!({ "applied": true, "path": path, "keys": patch.len() }))
        }

        Op::LayerAdd { layer } => {
            let id = layer.id.0;
            scene.layers.push((**layer).clone());
            Ok(json!({ "id": id, "index": scene.layers.len() - 1 }))
        }

        Op::LayerRemove { id } => {
            let index = layer_index(scene, *id)?;
            scene.layers.remove(index);
            Ok(json!({ "removed": id, "from": index }))
        }

        Op::LayerMove { id, to } => {
            let from = layer_index(scene, *id)?;
            let len = scene.layers.len();
            if *to >= len {
                return Err(OpError::BadPosition { index: *to, len });
            }
            let layer = scene.layers.remove(from);
            scene.layers.insert(*to, layer);
            Ok(json!({ "id": id, "from": from, "to": to }))
        }

        Op::EffectAdd { layer, effect } => {
            let index = layer_index(scene, *layer)?;
            let effects = &mut scene.layers[index].effects;
            effects.push((**effect).clone());
            Ok(json!({ "layer": layer, "index": effects.len() - 1 }))
        }

        Op::EffectRemove { layer, index } => {
            let at = layer_index(scene, *layer)?;
            let effects = &mut scene.layers[at].effects;
            if *index >= effects.len() {
                return Err(OpError::NoSuchEffect {
                    layer: *layer,
                    index: *index,
                });
            }
            effects.remove(*index);
            Ok(json!({ "layer": layer, "removed": index }))
        }
    }
}

/// Folds ops in order, returning each one's wire result.
///
/// Stops at the first error with the scene PARTIALLY applied, so callers fold
/// onto a clone they can discard. That is the same contract the v1 journal has.
pub fn apply_all(scene: &mut Scene, ops: &[Op]) -> Result<Vec<Value>, OpError> {
    ops.iter().map(|op| apply(scene, op)).collect()
}

/// Where a layer sits, by id. Ids are stable across inserts; positions are not.
pub fn layer_index(scene: &Scene, id: u32) -> Result<usize, OpError> {
    scene
        .layers
        .iter()
        .position(|layer| layer.id == LayerId(id))
        .ok_or(OpError::NoSuchLayer(id))
}

fn to_value(scene: &Scene) -> Result<Value, OpError> {
    serde_json::to_value(scene).map_err(|e| OpError::NotSerializable(e.to_string()))
}

fn from_value(doc: Value, path: &str) -> Result<Scene, OpError> {
    serde_json::from_value(doc).map_err(|e| OpError::TypeMismatch {
        path: path.to_string(),
        detail: e.to_string(),
    })
}

/// Whether every part of `scene` survives a trip through the v1 model.
///
/// Checked by ROUND-TRIPPING rather than by a hand-kept list of what v1 can
/// hold, so it cannot drift out of date as either model grows.
pub fn is_v1_representable(scene: &Scene) -> bool {
    &crate::migrate::to_scene(&crate::migrate::to_render_state(scene)) == scene
}

/// Applies a v1 edit to a scene by projecting down to `RenderState`, editing
/// there, and projecting back.
///
/// **This is the journal migration.** Every op already written to disk addresses
/// a flat `RenderState` field, and rewriting those journals is neither safe nor
/// necessary while the projection is lossless.
///
/// It REFUSES on a scene v1 cannot represent, because the projection would
/// silently drop whatever v1 has no room for. A scene reaches that state the
/// moment a scene-native op adds a layer or reorders one, so a journal cannot
/// mix the two freely and this is where that shows up.
pub fn with_render_state<T>(
    scene: &mut Scene,
    edit: impl FnOnce(&mut crate::v1::RenderState) -> T,
) -> Result<T, OpError> {
    if !is_v1_representable(scene) {
        return Err(OpError::NotV1Representable);
    }
    let mut state = crate::migrate::to_render_state(scene);
    let out = edit(&mut state);
    *scene = crate::migrate::to_scene(&state);
    Ok(out)
}
