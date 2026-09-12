//! Edits to a document, addressed by id or kind path (see `address`): set, insert, remove, move. All-or-nothing over a batch.
//! Variant and field names are a WIRE CONTRACT stored in journals; renaming one invalidates every journal written.

use serde::{Deserialize, Serialize};

use crate::address::{AddressError, Location};
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
    #[error("nothing at '{0}'")]
    NoSuchId(String),
    #[error("bad address: {0}")]
    Address(#[from] AddressError),
    #[error("cannot move '{id}' into its own subtree")]
    IntoSelf { id: String },
    #[error("'{parent}' is the root; it cannot be moved or removed")]
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

fn resolve(doc: &Document, address: &str) -> Result<Location, OpError> {
    doc.locate(address)?
        .ok_or_else(|| OpError::NoSuchId(address.to_owned()))
}

/// Applies one op in place. On error the document may be partially changed; `apply_all` is the atomic entry point.
pub(crate) fn apply(doc: &mut Document, op: &Op) -> Result<(), OpError> {
    match op {
        Op::Set { id, attr, value } => {
            let loc = resolve(doc, id)?;
            let node = doc
                .node_at_mut(&loc)
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
            let loc = resolve(doc, id)?;
            let node = doc
                .node_at_mut(&loc)
                .ok_or_else(|| OpError::NoSuchId(id.clone()))?;
            if !crate::schema::has_text(&node.kind) {
                return Err(OpError::NoText {
                    kind: node.kind.clone(),
                });
            }
            // Empty text is no text: the serializer writes neither, and a diff must not tell them apart.
            node.text = (!text.is_empty()).then(|| text.clone());
            Ok(())
        }
        Op::Insert {
            parent,
            index,
            node,
        } => {
            let loc = resolve(doc, parent)?;
            let target = doc
                .node_at_mut(&loc)
                .ok_or_else(|| OpError::NoSuchId(parent.clone()))?;
            let at = (*index).min(target.children.len());
            target.children.insert(at, Node::from(node.clone()));
            Ok(())
        }
        Op::Remove { id } => {
            let loc = resolve(doc, id)?;
            detach(doc, &loc, id)?;
            Ok(())
        }
        Op::Move { id, parent, index } => {
            let from = resolve(doc, id)?;
            let to = resolve(doc, parent)?;
            if from.contains(&to) {
                return Err(OpError::IntoSelf { id: id.clone() });
            }
            let node = detach(doc, &from, id)?;
            // The parent's location was computed before the node left; it shifts if it sat after the node.
            let to = to
                .after_removal(&from)
                .ok_or_else(|| OpError::IntoSelf { id: id.clone() })?;
            let target = doc
                .node_at_mut(&to)
                .ok_or_else(|| OpError::NoSuchId(parent.clone()))?;
            let at = (*index).min(target.children.len());
            target.children.insert(at, node);
            Ok(())
        }
    }
}

fn detach(doc: &mut Document, loc: &Location, address: &str) -> Result<Node, OpError> {
    let (parent, index) = loc.parent().ok_or_else(|| OpError::Root {
        parent: address.to_owned(),
    })?;
    let parent = doc
        .node_at_mut(&parent)
        .ok_or_else(|| OpError::NoSuchId(address.to_owned()))?;
    Ok(parent.children.remove(index))
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
    fn id_less_elements_are_reached_by_kind_path_and_the_root_by_slash() {
        let ops = vec![
            Op::Insert {
                parent: "/".into(),
                index: 99,
                node: NodeSpec {
                    kind: "background".into(),
                    attrs: Default::default(),
                    text: None,
                    children: vec![NodeSpec {
                        kind: "solid".into(),
                        attrs: [("color".to_owned(), "#000000".to_owned())].into(),
                        text: None,
                        children: vec![],
                    }],
                },
            },
            Op::Set {
                id: "/background/solid".into(),
                attr: "color".into(),
                value: Some("#ff0000".into()),
            },
            Op::Move {
                id: "/background".into(),
                parent: "/".into(),
                index: 0,
            },
        ];
        let out = apply_all(&doc(), &ops).unwrap();
        assert_eq!(out.root.children[0].kind, "background");
        assert_eq!(
            out.root.children[0].children[0].attr("color"),
            Some("#ff0000")
        );
        let err = apply_all(&doc(), &[Op::Remove { id: "/".into() }]).unwrap_err();
        assert!(matches!(err.1, OpError::Root { .. }));
        let err = apply_all(&doc(), &[Op::Remove { id: "z1/x[".into() }]).unwrap_err();
        assert!(matches!(err.1, OpError::Address(_)));
    }

    #[test]
    fn moving_a_node_earlier_in_its_own_parent_lands_at_the_asked_index() {
        let out = apply_all(
            &doc(),
            &[Op::Move {
                id: "z2".into(),
                parent: "/screen/zooms".into(),
                index: 0,
            }],
        )
        .unwrap();
        let zooms = out.root.child("screen").unwrap().child("zooms").unwrap();
        assert_eq!(zooms.children[0].id(), Some("z2"));
        assert_eq!(zooms.children[1].id(), Some("z1"));
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
