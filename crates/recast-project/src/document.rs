//! The in-memory document: a tree of nodes with string attributes, in document order.
//! Attributes stay as spelled so an unknown one round-trips; typed reads go through `schema` and `value`.

use std::collections::BTreeMap;

use crate::ids::Id;
use crate::parse::Position;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Node {
    pub kind: String,
    pub attrs: BTreeMap<String, String>,
    /// Element content, trimmed; only `text`-bearing elements carry it.
    pub text: Option<String>,
    pub children: Vec<Node>,
    /// Where the element started in the source it was parsed from; `None` when built in memory.
    pub at: Option<Position>,
}

impl Node {
    #[must_use]
    pub fn new(kind: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_id(mut self, id: &str) -> Self {
        self.attrs.insert("id".into(), id.to_owned());
        self
    }

    #[must_use]
    pub fn with(mut self, name: &str, value: impl Into<String>) -> Self {
        self.attrs.insert(name.into(), value.into());
        self
    }

    #[must_use]
    pub fn with_child(mut self, child: Node) -> Self {
        self.children.push(child);
        self
    }

    #[must_use]
    pub fn id(&self) -> Option<&str> {
        self.attrs.get("id").map(String::as_str)
    }

    #[must_use]
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(String::as_str)
    }

    pub fn set(&mut self, name: &str, value: impl Into<String>) {
        self.attrs.insert(name.into(), value.into());
    }

    /// Writes only when `value` is `true`; a false flag is absent, which is the documented default.
    pub fn set_flag(&mut self, name: &str, value: bool) {
        if value {
            self.attrs.insert(name.into(), "true".into());
        } else {
            self.attrs.remove(name);
        }
    }

    #[must_use]
    pub fn flag(&self, name: &str) -> bool {
        self.attr(name) == Some("true")
    }

    #[must_use]
    pub fn child(&self, kind: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.kind == kind)
    }

    pub fn child_mut(&mut self, kind: &str) -> Option<&mut Node> {
        self.children.iter_mut().find(|c| c.kind == kind)
    }

    pub fn children_of<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.kind == kind)
    }

    /// Depth-first walk over this node and every descendant.
    pub fn walk<'a>(&'a self, visit: &mut dyn FnMut(&'a Node)) {
        visit(self);
        for child in &self.children {
            child.walk(visit);
        }
    }
}

/// A whole document. The root is always a `recast` element.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub root: Node,
}

impl Document {
    /// An empty v3 document with the version stamped.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            root: Node::new("recast").with("v", crate::FORMAT_VERSION.to_string()),
        }
    }

    #[must_use]
    pub fn find(&self, id: &str) -> Option<&Node> {
        let mut found = None;
        self.root.walk(&mut |n| {
            if found.is_none() && n.id() == Some(id) {
                found = Some(n);
            }
        });
        found
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut Node> {
        find_mut_in(&mut self.root, id)
    }

    /// The parent of the node with `id` and the index the node sits at.
    pub fn parent_of_mut(&mut self, id: &str) -> Option<(&mut Node, usize)> {
        parent_in(&mut self.root, id)
    }

    /// Every id in the document, in document order. Duplicates appear twice; `validate` reports them.
    #[must_use]
    pub fn ids(&self) -> Vec<Id> {
        let mut out = Vec::new();
        self.root.walk(&mut |n| {
            if let Some(id) = n.id().and_then(|s| Id::parse(s).ok()) {
                out.push(id);
            }
        });
        out
    }

    #[must_use]
    pub fn version(&self) -> Option<u32> {
        self.root.attr("v").and_then(|v| v.parse().ok())
    }
}

fn find_mut_in<'a>(node: &'a mut Node, id: &str) -> Option<&'a mut Node> {
    if node.id() == Some(id) {
        return Some(node);
    }
    node.children.iter_mut().find_map(|c| find_mut_in(c, id))
}

fn parent_in<'a>(node: &'a mut Node, id: &str) -> Option<(&'a mut Node, usize)> {
    if let Some(index) = node.children.iter().position(|c| c.id() == Some(id)) {
        return Some((node, index));
    }
    node.children.iter_mut().find_map(|c| parent_in(c, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Document {
        let mut d = Document::empty();
        d.root.children.push(
            Node::new("screen")
                .with_id("scr")
                .with_child(Node::new("zoom").with_id("z1").with("at", "4.500")),
        );
        d
    }

    #[test]
    fn find_walks_the_tree_and_parent_reports_the_index() {
        let mut d = doc();
        assert_eq!(d.find("z1").unwrap().attr("at"), Some("4.500"));
        assert!(d.find("nope").is_none());
        let (parent, index) = d.parent_of_mut("z1").unwrap();
        assert_eq!(parent.kind, "screen");
        assert_eq!(index, 0);
    }

    #[test]
    fn flags_are_written_only_when_true() {
        let mut n = Node::new("camera");
        n.set_flag("mirror", true);
        assert!(n.flag("mirror"));
        n.set_flag("mirror", false);
        assert!(n.attr("mirror").is_none());
        assert!(!n.flag("mirror"));
    }

    #[test]
    fn ids_come_back_in_document_order() {
        let ids: Vec<String> = doc().ids().into_iter().map(|i| i.to_string()).collect();
        assert_eq!(ids, vec!["scr", "z1"]);
    }
}
