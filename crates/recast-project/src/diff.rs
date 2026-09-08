//! `diff(from, to)`: the ops that turn one document into another, so a whole-state save or an edited file becomes one sequenced batch.
//! Nodes match by id anywhere in the tree, id-less nodes by kind and ordinal under the same parent; an unmatched target subtree is inserted whole.

use std::collections::HashSet;

use crate::address::Location;
use crate::document::{Document, Node};
use crate::ops::{apply, NodeSpec, Op, OpError};
use crate::schema::has_text;

/// # Errors Only on an internal inconsistency, since every emitted op is applied to a working copy as it is produced.
pub fn diff(from: &Document, to: &Document) -> Result<Vec<Op>, OpError> {
    let mut differ = Differ {
        work: from.clone(),
        ops: Vec::new(),
        target_ids: ids_in(&to.root),
    };
    differ.node(&Location::default(), &to.root)?;
    Ok(differ.ops)
}

struct Differ {
    work: Document,
    ops: Vec<Op>,
    target_ids: HashSet<String>,
}

/// Where a target child comes from: the same parent, another parent (by id), or nowhere yet.
enum Source {
    Here,
    Elsewhere(String),
    Fresh,
}

impl Differ {
    fn emit(&mut self, op: Op) -> Result<(), OpError> {
        apply(&mut self.work, &op)?;
        self.ops.push(op);
        Ok(())
    }

    fn address(&self, loc: &Location) -> Result<String, OpError> {
        self.work
            .address_of(loc)
            .map(|a| a.to_string())
            .ok_or_else(|| OpError::NoSuchId(format!("{loc:?}")))
    }

    fn node(&mut self, loc: &Location, target: &Node) -> Result<(), OpError> {
        let addr = self.address(loc)?;
        let current = self
            .work
            .node_at(loc)
            .cloned()
            .ok_or_else(|| OpError::NoSuchId(addr.clone()))?;
        self.attrs(&addr, &current, target)?;
        let fresh = self.children(loc, &addr, target)?;
        for (i, child) in target.children.iter().enumerate() {
            if !fresh[i] {
                self.node(&loc.child(i), child)?;
            }
        }
        Ok(())
    }

    fn attrs(&mut self, addr: &str, current: &Node, target: &Node) -> Result<(), OpError> {
        for (name, value) in &target.attrs {
            if current.attrs.get(name) != Some(value) {
                self.emit(Op::Set {
                    id: addr.to_owned(),
                    attr: name.clone(),
                    value: Some(value.clone()),
                })?;
            }
        }
        for name in current.attrs.keys() {
            if !target.attrs.contains_key(name) {
                self.emit(Op::Set {
                    id: addr.to_owned(),
                    attr: name.clone(),
                    value: None,
                })?;
            }
        }
        if has_text(&target.kind) && text_of(current) != text_of(target) {
            self.emit(Op::SetText {
                id: addr.to_owned(),
                text: text_of(target).to_owned(),
            })?;
        }
        Ok(())
    }

    /// Reconciles one parent's children into target order; returns which target children were inserted whole.
    fn children(
        &mut self,
        loc: &Location,
        addr: &str,
        target: &Node,
    ) -> Result<Vec<bool>, OpError> {
        let current: Vec<Node> = self
            .work
            .node_at(loc)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        let (claims, sources) = self.claim(&current, target);
        // `order` mirrors the parent's children as ops land: the target index each holds, or None for a node another parent will claim.
        let mut order: Vec<Option<usize>> = claims.clone();
        for (ci, child) in current.iter().enumerate().rev() {
            if claims[ci].is_some() || child.id().is_some_and(|id| self.target_ids.contains(id)) {
                continue;
            }
            let child_addr = self.address(&loc.child(ci))?;
            self.emit(Op::Remove { id: child_addr })?;
            order.remove(ci);
        }
        let mut fresh = vec![false; target.children.len()];
        for (ti, child) in target.children.iter().enumerate() {
            if order.get(ti) == Some(&Some(ti)) {
                continue;
            }
            match &sources[ti] {
                Source::Here => {
                    let j = order
                        .iter()
                        .position(|o| *o == Some(ti))
                        .ok_or_else(|| OpError::NoSuchId(format!("{addr}[{ti}]")))?;
                    let child_addr = self.address(&loc.child(j))?;
                    self.emit(Op::Move {
                        id: child_addr,
                        parent: addr.to_owned(),
                        index: ti,
                    })?;
                    let entry = order.remove(j);
                    order.insert(ti, entry);
                }
                Source::Elsewhere(id) if !self.would_nest_into_itself(id, loc) => {
                    self.emit(Op::Move {
                        id: id.clone(),
                        parent: addr.to_owned(),
                        index: ti,
                    })?;
                    order.insert(ti, Some(ti));
                }
                Source::Elsewhere(_) | Source::Fresh => {
                    self.purge(child)?;
                    self.emit(Op::Insert {
                        parent: addr.to_owned(),
                        index: ti,
                        node: NodeSpec::from(child),
                    })?;
                    order.insert(ti, Some(ti));
                    fresh[ti] = true;
                }
            }
        }
        Ok(fresh)
    }

    /// Pairs target children with current ones: ids first, then id-less nodes by kind in order.
    fn claim(&self, current: &[Node], target: &Node) -> (Vec<Option<usize>>, Vec<Source>) {
        let mut claims: Vec<Option<usize>> = vec![None; current.len()];
        let mut sources: Vec<Source> = Vec::with_capacity(target.children.len());
        for (ti, child) in target.children.iter().enumerate() {
            let source = match child.id() {
                Some(id) => match current.iter().position(|c| c.id() == Some(id)) {
                    Some(ci) => {
                        claims[ci] = Some(ti);
                        Source::Here
                    }
                    None if self.work.find(id).is_some() => Source::Elsewhere(id.to_owned()),
                    None => Source::Fresh,
                },
                None => Source::Fresh,
            };
            sources.push(source);
        }
        for (ti, child) in target.children.iter().enumerate() {
            if child.id().is_some() {
                continue;
            }
            let free = current.iter().enumerate().position(|(ci, c)| {
                c.kind == child.kind && c.id().is_none() && claims[ci].is_none()
            });
            if let Some(ci) = free {
                claims[ci] = Some(ti);
                sources[ti] = Source::Here;
            }
        }
        (claims, sources)
    }

    /// A node cannot be moved under its own descendant; the caller inserts a copy and lets the old one be removed instead.
    fn would_nest_into_itself(&self, id: &str, into: &Location) -> bool {
        self.work
            .locate(id)
            .ok()
            .flatten()
            .is_some_and(|at| at.contains(into))
    }

    /// Removes every node whose id the inserted subtree will reuse, so an insert never duplicates an id.
    fn purge(&mut self, subtree: &Node) -> Result<(), OpError> {
        for id in ids_in(subtree) {
            if self.work.find(&id).is_some() {
                self.emit(Op::Remove { id })?;
            }
        }
        Ok(())
    }
}

fn ids_in(node: &Node) -> HashSet<String> {
    let mut out = HashSet::new();
    node.walk(&mut |n| {
        if let Some(id) = n.id() {
            out.insert(id.to_owned());
        }
    });
    out
}

fn text_of(node: &Node) -> &str {
    node.text.as_deref().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::apply_all;
    use crate::parse::parse;
    use crate::serialize::serialize;

    fn doc(text: &str) -> Document {
        parse(text).unwrap()
    }

    fn assert_diff_reaches(from: &str, to: &str) -> Vec<Op> {
        let (from, to) = (doc(from), doc(to));
        let ops = diff(&from, &to).unwrap();
        let reached = apply_all(&from, &ops).unwrap();
        assert_eq!(serialize(&reached), serialize(&to), "ops: {ops:#?}");
        ops
    }

    const BASE: &str = r##"<recast v="3"><background><gradient><stop at="0" color="#000000"/><stop at="1" color="#ffffff"/></gradient></background><screen id="scr"><zooms><zoom id="z1" at="1" dur="2"/><zoom id="z2" at="5" dur="2"/></zooms></screen><annotations><text id="t1" at="1" dur="1">hi</text></annotations></recast>"##;

    #[test]
    fn identical_documents_diff_to_nothing() {
        assert!(assert_diff_reaches(BASE, BASE).is_empty());
    }

    #[test]
    fn attribute_and_text_changes_become_sets_by_id_or_kind_path() {
        let ops = assert_diff_reaches(
            BASE,
            r##"<recast v="3"><background><gradient><stop at="0" color="#111111"/><stop at="1" color="#ffffff"/></gradient></background><screen id="scr"><zooms><zoom id="z1" at="1" dur="3" scale="2"/><zoom id="z2" at="5"/></zooms></screen><annotations><text id="t1" at="1" dur="1">bye</text></annotations></recast>"##,
        );
        assert!(ops.iter().any(|o| matches!(o, Op::Set { id, attr, .. } if id == "/background/gradient/stop" && attr == "color")));
        assert!(ops.iter().any(
            |o| matches!(o, Op::Set { id, attr, value: None } if id == "z2" && attr == "dur")
        ));
        assert!(ops
            .iter()
            .any(|o| matches!(o, Op::SetText { id, text } if id == "t1" && text == "bye")));
        assert_eq!(ops.len(), 5);
    }

    #[test]
    fn reorders_removals_and_inserts_land_as_moves_removes_and_inserts() {
        let ops = assert_diff_reaches(
            BASE,
            r##"<recast v="3"><background><gradient><stop at="1" color="#ffffff"/></gradient></background><screen id="scr"><zooms><zoom id="z3" at="9" dur="1"/><zoom id="z2" at="5" dur="2"/></zooms><shadow blur="40"/></screen><annotations><text id="t1" at="1" dur="1">hi</text></annotations></recast>"##,
        );
        assert!(ops
            .iter()
            .any(|o| matches!(o, Op::Remove { id } if id == "z1")));
        assert!(ops
            .iter()
            .any(|o| matches!(o, Op::Insert { parent, index: 0, .. } if parent == "scr/zooms")));
        assert!(ops.iter().any(|o| matches!(o, Op::Insert { parent, node, .. } if parent == "scr" && node.kind == "shadow")));
    }

    #[test]
    fn a_node_that_moves_between_parents_is_moved_not_recreated() {
        let ops = assert_diff_reaches(
            r##"<recast v="3"><timeline><clip id="k1" at="0" dur="1"/><clip id="k2" at="1" dur="1"><enter kind="fade"/></clip></timeline></recast>"##,
            r##"<recast v="3"><timeline><clip id="k2" at="1" dur="1"/><clip id="k1" at="0" dur="1"><enter kind="fade"/></clip></timeline></recast>"##,
        );
        assert!(ops.iter().any(|o| matches!(o, Op::Move { .. })));
        assert!(!ops
            .iter()
            .any(|o| matches!(o, Op::Insert { node, .. } if node.kind == "clip")));
    }

    #[test]
    fn swapped_nesting_still_reaches_the_target() {
        assert_diff_reaches(
            r##"<recast v="3"><annotations><text id="a" at="0" dur="1"><text id="b" at="0" dur="1">x</text></text></annotations></recast>"##,
            r##"<recast v="3"><annotations><text id="b" at="0" dur="1"><text id="a" at="0" dur="1">x</text></text></annotations></recast>"##,
        );
    }

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next() % n.max(1) as u64) as usize
        }
    }

    fn random_edit(rng: &mut Rng, doc: &Document, n: usize) -> Op {
        let ids: Vec<String> = doc.ids().iter().map(ToString::to_string).collect();
        let pick = |rng: &mut Rng| {
            if ids.is_empty() {
                "/".to_owned()
            } else {
                ids[rng.below(ids.len())].clone()
            }
        };
        match rng.below(5) {
            0 => Op::Set {
                id: pick(rng),
                attr: ["at", "dur", "scale"][rng.below(3)].into(),
                value: (rng.below(3) > 0).then(|| rng.below(9).to_string()),
            },
            1 => Op::Insert {
                parent: if rng.below(4) == 0 {
                    "/".into()
                } else {
                    pick(rng)
                },
                index: rng.below(4),
                node: NodeSpec {
                    kind: ["zoom", "shadow", "clip"][rng.below(3)].into(),
                    attrs: [("id".to_owned(), format!("n{n}"))]
                        .into_iter()
                        .filter(|_| rng.below(3) > 0)
                        .collect(),
                    text: None,
                    children: vec![],
                },
            },
            2 => Op::Remove { id: pick(rng) },
            3 => Op::Move {
                id: pick(rng),
                parent: pick(rng),
                index: rng.below(4),
            },
            _ => Op::Set {
                id: "/".into(),
                attr: "fps".into(),
                value: Some(rng.below(60).to_string()),
            },
        }
    }

    #[test]
    fn random_edit_sequences_are_recovered_by_diff() {
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        let base = doc(BASE);
        for round in 0..300 {
            let mut to = base.clone();
            let mut n = 0;
            while n < 6 {
                let op = random_edit(&mut rng, &to, round * 10 + n);
                if let Ok(next) = apply_all(&to, &[op]) {
                    to = next;
                }
                n += 1;
            }
            let ops = diff(&base, &to).unwrap();
            let reached = apply_all(&base, &ops).unwrap();
            assert_eq!(
                serialize(&reached),
                serialize(&to),
                "round {round}: {ops:#?}"
            );
            assert!(diff(&to, &reached).unwrap().is_empty());
        }
    }
}
