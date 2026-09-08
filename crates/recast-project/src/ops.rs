//! Edits to a document, addressed by id: set, insert, remove, move. All-or-nothing over a batch.
//! Variant and field names are a WIRE CONTRACT stored in journals; renaming one invalidates every journal written.

use serde::{Deserialize, Serialize};

use crate::document::{Document, Node};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Op {
    /// Sets an attribute; `None` removes it.
    Set {
        id: String,
        attr: String,
        value: Option<String>,
    },
    /// Sets the element content of a text-bearing element.
    SetText {
        id: String,
        text: String,
    },
    /// Inserts `node` under `parent` at `index`, appending when `index` is past the end.
    Insert {
        parent: String,
        index: usize,
        node: NodeSpec,
    },
    Remove {
        id: String,
    },
    Move {
        id: String,
        parent: String,
        index: usize,
    },
}

/// A node as an op carries it: kind, attributes, optional text, children. No positions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSpec {
    pub kind: String,
    #[serde(default)]
    pub attrs: std::collections::BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<NodeSpec>,
}

impl From<NodeSpec> for Node {
    fn from(spec: NodeSpec) -> Self {
        Node {
            kind: spec.kind,
            attrs: spec.attrs,
            text: spec.text,
            children: spec.children.into_iter().map(Node::from).collect(),
            at: None,
        }
    }
}

impl From<&Node> for NodeSpec {
    fn from(node: &Node) -> Self {
        Self {
            kind: node.kind.clone(),
            attrs: node.attrs.clone(),
            text: node.text.clone(),
            children: node.children.iter().map(NodeSpec::from).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpError {
    #[error("no element with id '{0}'")]
    NoSuchId(String),
    #[error("cannot move '{id}' into its own subtree")]
    IntoSelf { id: String },
    #[error("'{parent}' is the root's ancestor; the root cannot be moved or removed")]
    Root { parent: String },
    #[error("<{kind}> does not carry text")]
    NoText { kind: String },
}

/// Applies `ops` in order onto a clone and returns it; the input is untouched on any error.
/// # Errors On the first op that cannot apply, naming it by index.
pub fn apply_all(doc: &Document, ops: &[Op]) -> Result<Document, (usize, OpError)> {
    let mut out = doc.clone();
    for (i, op) in ops.iter().enumerate() {
        apply(&mut out, op).map_err(|e| (i, e))?;
    }
    Ok(out)
}

fn apply(doc: &mut Document, op: &Op) -> Result<(), OpError> {
    match op {
        Op::Set { id, attr, value } => {
            let node = doc
                .find_mut(id)
                .ok_or_else(|| OpError::NoSuchId(id.clone()))?;
            match value {
                Some(v) => node.set(attr, v.clone()),
                None => {
                    node.attrs.remove(attr);
                }
            }
            Ok(())
        }
        Op::SetText { id, text } => {
            let node = doc
                .find_mut(id)
                .ok_or_else(|| OpError::NoSuchId(id.clone()))?;
            if !crate::schema::has_text(&node.kind) {
                return Err(OpError::NoText {
                    kind: node.kind.clone(),
                });
            }
            node.text = Some(text.clone());
            Ok(())
        }
        Op::Insert {
            parent,
            index,
            node,
        } => {
            let target = doc
                .find_mut(parent)
                .ok_or_else(|| OpError::NoSuchId(parent.clone()))?;
            let at = (*index).min(target.children.len());
            target.children.insert(at, Node::from(node.clone()));
            Ok(())
        }
        Op::Remove { id } => {
            if doc.root.id() == Some(id) {
                return Err(OpError::Root { parent: id.clone() });
            }
            let (parent, index) = doc
                .parent_of_mut(id)
                .ok_or_else(|| OpError::NoSuchId(id.clone()))?;
            parent.children.remove(index);
            Ok(())
        }
        Op::Move { id, parent, index } => {
            let moving = doc.find(id).ok_or_else(|| OpError::NoSuchId(id.clone()))?;
            if moving.id() == Some(parent) || subtree_has(moving, parent) {
                return Err(OpError::IntoSelf { id: id.clone() });
            }
            doc.find(parent)
                .ok_or_else(|| OpError::NoSuchId(parent.clone()))?;
            let (old_parent, old_index) = doc
                .parent_of_mut(id)
                .ok_or_else(|| OpError::NoSuchId(id.clone()))?;
            let node = old_parent.children.remove(old_index);
            let target = doc
                .find_mut(parent)
                .ok_or_else(|| OpError::NoSuchId(parent.clone()))?;
            let at = (*index).min(target.children.len());
            target.children.insert(at, node);
            Ok(())
        }
    }
}

fn subtree_has(node: &Node, id: &str) -> bool {
    node.children
        .iter()
        .any(|c| c.id() == Some(id) || subtree_has(c, id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;
    use crate::serialize::serialize;

    fn doc() -> Document {
        parse("<recast v=\"3\"><screen id=\"scr\"><zooms><zoom id=\"z1\" at=\"1\" dur=\"2\"/><zoom id=\"z2\" at=\"5\" dur=\"2\"/></zooms></screen><annotations><text id=\"t1\" at=\"1\" dur=\"1\">hi</text></annotations></recast>").unwrap()
    }

    #[test]
    fn set_insert_remove_and_move_land_by_id() {
        let ops = vec![
            Op::Set {
                id: "z1".into(),
                attr: "scale".into(),
                value: Some("2".into()),
            },
            Op::Set {
                id: "z2".into(),
                attr: "dur".into(),
                value: None,
            },
            Op::SetText {
                id: "t1".into(),
                text: "hello".into(),
            },
            Op::Insert {
                parent: "scr".into(),
                index: 99,
                node: NodeSpec {
                    kind: "shadow".into(),
                    attrs: [("blur".to_owned(), "40".to_owned())].into(),
                    text: None,
                    children: vec![],
                },
            },
            Op::Move {
                id: "z2".into(),
                parent: "scr".into(),
                index: 0,
            },
            Op::Remove { id: "z1".into() },
        ];
        let out = apply_all(&doc(), &ops).unwrap();
        let text = serialize(&out);
        assert!(text.contains("<zoom id=\"z2\" at=\"5.000\"/>"), "{text}");
        assert!(!text.contains("z1"));
        assert!(text.contains(">hello</text>"));
        assert_eq!(
            out.root.child("screen").unwrap().children[0].id(),
            Some("z2")
        );
        assert!(
            out.root
                .child("screen")
                .unwrap()
                .children
                .last()
                .unwrap()
                .kind
                == "shadow"
        );
    }

    #[test]
    fn a_failing_op_leaves_the_input_untouched_and_names_its_index() {
        let before = doc();
        let ops = vec![
            Op::Set {
                id: "z1".into(),
                attr: "scale".into(),
                value: Some("2".into()),
            },
            Op::Remove { id: "nope".into() },
        ];
        let err = apply_all(&before, &ops).unwrap_err();
        assert_eq!(err, (1, OpError::NoSuchId("nope".into())));
        assert_eq!(before.find("z1").unwrap().attr("scale"), None);
    }

    #[test]
    fn a_node_cannot_move_into_itself_and_text_needs_a_text_element() {
        let err = apply_all(
            &doc(),
            &[Op::Move {
                id: "scr".into(),
                parent: "z1".into(),
                index: 0,
            }],
        )
        .unwrap_err();
        assert_eq!(err.1, OpError::IntoSelf { id: "scr".into() });
        let err = apply_all(
            &doc(),
            &[Op::SetText {
                id: "z1".into(),
                text: "x".into(),
            }],
        )
        .unwrap_err();
        assert_eq!(
            err.1,
            OpError::NoText {
                kind: "zoom".into()
            }
        );
    }

    #[test]
    fn ops_are_a_stable_wire_shape() {
        let op = Op::Set {
            id: "z1".into(),
            attr: "scale".into(),
            value: Some("2".into()),
        };
        assert_eq!(
            serde_json::to_string(&op).unwrap(),
            r#"{"op":"set","id":"z1","attr":"scale","value":"2"}"#
        );
        let json = r#"{"op":"insert","parent":"scr","index":0,"node":{"kind":"corner","attrs":{"pct":"1.2"}}}"#;
        let parsed: Op = serde_json::from_str(json).unwrap();
        assert!(matches!(parsed, Op::Insert { .. }));
    }
}
