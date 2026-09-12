//! Where an op lands: an id, or a kind path below an id or the root, for the elements the schema gives no id.
//! `z1`, `z1/enter`, `/background/solid`, `/background/gradient/stop[1]`, `/` (the root). Ordinals count same-kind siblings from 0.

use std::fmt;

use crate::document::{Document, Node};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    anchor: Anchor,
    steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Anchor {
    Root,
    Id(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Step {
    kind: String,
    nth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AddressError {
    #[error("empty address")]
    Empty,
    #[error("bad step '{0}': expected kind or kind[n]")]
    BadStep(String),
}

impl Address {
    /// # Errors On an empty string or a step that is not `kind` or `kind[n]`.
    pub fn parse(text: &str) -> Result<Self, AddressError> {
        if text.is_empty() {
            return Err(AddressError::Empty);
        }
        let (anchor, rest) = match text.strip_prefix('/') {
            Some(rest) => (Anchor::Root, rest),
            None => match text.split_once('/') {
                Some((id, rest)) => (Anchor::Id(id.to_owned()), rest),
                None => (Anchor::Id(text.to_owned()), ""),
            },
        };
        let steps = rest
            .split('/')
            .filter(|s| !s.is_empty())
            .map(parse_step)
            .collect::<Result<_, _>>()?;
        Ok(Self { anchor, steps })
    }

    #[must_use]
    pub fn root() -> Self {
        Self {
            anchor: Anchor::Root,
            steps: Vec::new(),
        }
    }

    #[must_use]
    pub fn id(id: &str) -> Self {
        Self {
            anchor: Anchor::Id(id.to_owned()),
            steps: Vec::new(),
        }
    }

    /// Whether this address names the root element itself.
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.anchor == Anchor::Root && self.steps.is_empty()
    }
}

fn parse_step(text: &str) -> Result<Step, AddressError> {
    let bad = || AddressError::BadStep(text.to_owned());
    let (kind, nth) = match text.split_once('[') {
        Some((kind, rest)) => {
            let n = rest.strip_suffix(']').ok_or_else(bad)?;
            (kind, n.parse::<usize>().map_err(|_| bad())?)
        }
        None => (text, 0),
    };
    if kind.is_empty() || !kind.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(bad());
    }
    Ok(Step {
        kind: kind.to_owned(),
        nth,
    })
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.anchor {
            Anchor::Root => f.write_str("/")?,
            Anchor::Id(id) => f.write_str(id)?,
        }
        for (i, step) in self.steps.iter().enumerate() {
            if i > 0 || matches!(self.anchor, Anchor::Id(_)) {
                f.write_str("/")?;
            }
            f.write_str(&step.kind)?;
            if step.nth > 0 {
                write!(f, "[{}]", step.nth)?;
            }
        }
        Ok(())
    }
}

/// A resolved position: child indices from the root. Empty is the root itself.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Location(pub Vec<usize>);

impl Location {
    #[must_use]
    pub fn parent(&self) -> Option<(Location, usize)> {
        let (last, rest) = self.0.split_last()?;
        Some((Location(rest.to_vec()), *last))
    }

    #[must_use]
    pub fn child(&self, index: usize) -> Location {
        let mut v = self.0.clone();
        v.push(index);
        Location(v)
    }

    /// Whether `other` is this location or lies inside it.
    #[must_use]
    pub fn contains(&self, other: &Location) -> bool {
        other.0.starts_with(&self.0)
    }

    /// The same location after the node at `removed` left the tree; `None` if it was inside the removed subtree.
    #[must_use]
    pub fn after_removal(&self, removed: &Location) -> Option<Location> {
        if removed.contains(self) {
            return None;
        }
        let (parent, index) = removed.parent()?;
        let mut v = self.0.clone();
        let depth = parent.0.len();
        if v.len() > depth && v[..depth] == parent.0[..] && v[depth] > index {
            v[depth] -= 1;
        }
        Some(Location(v))
    }
}

impl Document {
    /// # Errors When the address does not parse; `Ok(None)` when it parses but nothing is there.
    pub fn locate(&self, address: &str) -> Result<Option<Location>, AddressError> {
        let addr = Address::parse(address)?;
        Ok(self.locate_parsed(&addr))
    }

    #[must_use]
    pub fn locate_parsed(&self, addr: &Address) -> Option<Location> {
        let mut loc = match &addr.anchor {
            Anchor::Root => Location::default(),
            Anchor::Id(id) => location_of_id(&self.root, id, Location::default())?,
        };
        for step in &addr.steps {
            let node = self.node_at(&loc)?;
            let index = node
                .children
                .iter()
                .enumerate()
                .filter(|(_, c)| c.kind == step.kind)
                .nth(step.nth)
                .map(|(i, _)| i)?;
            loc = loc.child(index);
        }
        Some(loc)
    }

    #[must_use]
    pub fn node_at(&self, loc: &Location) -> Option<&Node> {
        loc.0.iter().try_fold(&self.root, |n, &i| n.children.get(i))
    }

    pub fn node_at_mut(&mut self, loc: &Location) -> Option<&mut Node> {
        loc.0
            .iter()
            .try_fold(&mut self.root, |n, &i| n.children.get_mut(i))
    }

    /// The shortest address for `loc`: anchored on the nearest id on the way down, else the root.
    #[must_use]
    pub fn address_of(&self, loc: &Location) -> Option<Address> {
        let mut addr = Address::root();
        let mut node = &self.root;
        if let Some(id) = node.id() {
            addr = Address::id(id);
        }
        for &i in &loc.0 {
            let child = node.children.get(i)?;
            match child.id() {
                Some(id) => addr = Address::id(id),
                None => addr.steps.push(Step {
                    kind: child.kind.clone(),
                    nth: node.children[..i]
                        .iter()
                        .filter(|c| c.kind == child.kind)
                        .count(),
                }),
            }
            node = child;
        }
        Some(addr)
    }
}

fn location_of_id(node: &Node, id: &str, at: Location) -> Option<Location> {
    if node.id() == Some(id) {
        return Some(at);
    }
    node.children
        .iter()
        .enumerate()
        .find_map(|(i, c)| location_of_id(c, id, at.child(i)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn doc() -> Document {
        parse(
            "<recast v=\"3\"><background><gradient><stop at=\"0\" color=\"#000000\"/><stop at=\"1\" color=\"#ffffff\"/></gradient></background><timeline><clip id=\"k1\" at=\"0\" dur=\"1\"><enter kind=\"fade\"/></clip></timeline></recast>",
        )
        .unwrap()
    }

    #[test]
    fn root_id_and_kind_paths_resolve_and_print_back() {
        let d = doc();
        assert_eq!(d.locate("/").unwrap(), Some(Location(vec![])));
        let second_stop = d.locate("/background/gradient/stop[1]").unwrap().unwrap();
        assert_eq!(second_stop, Location(vec![0, 0, 1]));
        assert_eq!(d.node_at(&second_stop).unwrap().attr("at"), Some("1"));
        let enter = d.locate("k1/enter").unwrap().unwrap();
        assert_eq!(d.node_at(&enter).unwrap().kind, "enter");
        assert_eq!(
            d.address_of(&second_stop).unwrap().to_string(),
            "/background/gradient/stop[1]"
        );
        assert_eq!(d.address_of(&enter).unwrap().to_string(), "k1/enter");
        assert_eq!(
            d.address_of(&Location(vec![1, 0])).unwrap().to_string(),
            "k1"
        );
    }

    #[test]
    fn a_missing_target_is_none_and_a_bad_step_is_an_error() {
        let d = doc();
        assert_eq!(d.locate("/background/stop[7]").unwrap(), None);
        assert_eq!(d.locate("nope").unwrap(), None);
        assert!(matches!(
            Address::parse("k1/enter[x]"),
            Err(AddressError::BadStep(_))
        ));
        assert_eq!(Address::parse(""), Err(AddressError::Empty));
    }

    #[test]
    fn locations_shift_after_a_removal_only_when_they_sit_after_it() {
        let removed = Location(vec![0, 0, 0]);
        assert_eq!(
            Location(vec![0, 0, 1]).after_removal(&removed),
            Some(Location(vec![0, 0, 0]))
        );
        assert_eq!(
            Location(vec![1, 0]).after_removal(&removed),
            Some(Location(vec![1, 0]))
        );
        assert_eq!(Location(vec![0, 0, 0, 3]).after_removal(&removed), None);
    }
}
