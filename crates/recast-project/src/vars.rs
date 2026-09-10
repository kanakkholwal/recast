//! `<vars>`: typed values elements reference as `$name`, resolved on the way to the engine and never stored resolved.
//! The document keeps `$name` verbatim so a variable survives a round trip; only the copy the reader walks is substituted.

use std::collections::BTreeMap;

use crate::document::{Document, Node};
use crate::value;

pub use recast_scene::vars::{VarSpec as Var, VarType};

/// The rules a declaration obeys. An extension trait because the type itself travels to the engine and the editor, while these belong to the parser.
pub trait VarRules {
    fn resolved(&self) -> String;
    fn is_ill_typed(&self) -> bool;
    fn is_out_of_range(&self) -> bool;
}

impl VarRules for Var {
    /// What a reference expands to: the declared value, clamped for a numeric type with a range.
    fn resolved(&self) -> String {
        if !self.ty.is_numeric() {
            return self.value.clone();
        }
        let Ok(n) = value::parse_num(&self.value) else {
            return self.value.clone();
        };
        let clamped = clamp(n, self.min, self.max);
        if clamped == n {
            return self.value.clone();
        }
        match self.ty {
            VarType::Int => value::fmt_num(clamped.round()),
            _ => value::fmt_num(clamped),
        }
    }

    /// True when the declared value does not read as its declared type.
    fn is_ill_typed(&self) -> bool {
        match self.ty {
            VarType::Number | VarType::Angle => value::parse_num(&self.value).is_err(),
            VarType::Int => value::parse_int(&self.value).is_err(),
            VarType::Bool => value::parse_bool(&self.value).is_err(),
            VarType::Vec2 => parse_vec2(&self.value).is_none(),
            VarType::Select => !self.options.iter().any(|o| o == &self.value),
            VarType::Color | VarType::Text | VarType::Font | VarType::Asset => false,
        }
    }

    /// True when a numeric value sits outside its own range, which resolution then clamps.
    fn is_out_of_range(&self) -> bool {
        if !self.ty.is_numeric() {
            return false;
        }
        value::parse_num(&self.value).is_ok_and(|n| clamp(n, self.min, self.max) != n)
    }
}

fn clamp(v: f64, min: Option<f64>, max: Option<f64>) -> f64 {
    let v = min.map_or(v, |m| v.max(m));
    max.map_or(v, |m| v.min(m))
}

fn parse_vec2(text: &str) -> Option<(f64, f64)> {
    let (x, y) = text.split_once(',')?;
    Some((value::parse_num(x).ok()?, value::parse_num(y).ok()?))
}

/// The document's declared variables, in document order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Vars {
    order: Vec<Var>,
}

impl Vars {
    #[must_use]
    pub fn from_document(doc: &Document) -> Self {
        Self::from_root(&doc.root)
    }

    #[must_use]
    pub fn from_root(root: &Node) -> Self {
        let order = root
            .child("vars")
            .into_iter()
            .flat_map(|vars| vars.children_of("var"))
            .filter_map(var_from_node)
            .collect();
        Self { order }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Var> {
        self.order.iter()
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Var> {
        self.order.iter().find(|v| v.name == name)
    }

    /// Names declared more than once; the first declaration is the one that resolves.
    #[must_use]
    pub fn duplicates(&self) -> Vec<&str> {
        let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
        for var in &self.order {
            *seen.entry(var.name.as_str()).or_default() += 1;
        }
        seen.into_iter()
            .filter(|&(_, n)| n > 1)
            .map(|(name, _)| name)
            .collect()
    }

    /// The value a `$name` reference stands for, or `None` when the text is not a reference.
    #[must_use]
    pub fn expand(&self, raw: &str) -> Option<String> {
        let name = reference(raw)?;
        self.get(name).map(Var::resolved)
    }
}

fn var_from_node(node: &Node) -> Option<Var> {
    let name = node.attr("name")?.to_owned();
    let ty = VarType::parse(node.attr("type")?)?;
    Some(Var {
        name,
        ty,
        value: node.attr("value").unwrap_or_default().to_owned(),
        path: node.attr("path").map(str::to_owned),
        min: node.attr("min").and_then(|v| value::parse_num(v).ok()),
        max: node.attr("max").and_then(|v| value::parse_num(v).ok()),
        step: node.attr("step").and_then(|v| value::parse_num(v).ok()),
        options: node
            .attr("options")
            .map(|o| o.split(',').map(|s| s.trim().to_owned()).collect())
            .unwrap_or_default(),
    })
}

/// The variable an attribute value names, if it is a whole reference.
/// Only a whole value is a reference; `$a$b` and `pre$a` are literal text, which keeps the grammar free of escaping.
#[must_use]
pub fn reference(raw: &str) -> Option<&str> {
    let name = raw.strip_prefix('$')?;
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    valid.then_some(name)
}

/// Every `$name` in the tree that no `<var>` declares, with the element that names it.
#[must_use]
pub fn unresolved(root: &Node, vars: &Vars) -> Vec<(String, String)> {
    let mut out = Vec::new();
    root.walk(&mut |node: &Node| {
        if node.kind == "var" {
            return;
        }
        for (attr, raw) in &node.attrs {
            if let Some(name) = reference(raw) {
                if vars.get(name).is_none() {
                    out.push((node.kind.clone(), format!("{attr}=\"{raw}\"")));
                }
            }
        }
    });
    out
}

/// A copy of the document with every `$name` replaced by its value, for readers that want values rather than references.
/// Returns the document unchanged when it declares no variables, so the common case does not pay for the walk.
#[must_use]
pub fn resolve(doc: &Document) -> std::borrow::Cow<'_, Document> {
    let vars = Vars::from_document(doc);
    if vars.is_empty() {
        return std::borrow::Cow::Borrowed(doc);
    }
    let mut resolved = doc.clone();
    substitute(&mut resolved.root, &vars);
    std::borrow::Cow::Owned(resolved)
}

fn substitute(node: &mut Node, vars: &Vars) {
    if node.kind != "var" {
        for value in node.attrs.values_mut() {
            if let Some(expanded) = vars.expand(value) {
                *value = expanded;
            }
        }
        if let Some(text) = node.text.as_mut() {
            if let Some(expanded) = vars.expand(text) {
                *text = expanded;
            }
        }
    }
    for child in &mut node.children {
        substitute(child, vars);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn doc(body: &str) -> Document {
        parse(&format!("<recast v=\"3\">{body}</recast>")).unwrap()
    }

    #[test]
    fn a_reference_is_the_whole_value_or_it_is_literal_text() {
        assert_eq!(reference("$brand"), Some("brand"));
        assert_eq!(reference("$corner-2"), Some("corner-2"));
        assert_eq!(reference("$"), None);
        assert_eq!(reference("pre$brand"), None);
        assert_eq!(reference("$a.b"), None);
        assert_eq!(reference("plain"), None);
    }

    #[test]
    fn resolution_replaces_references_and_leaves_the_declarations_alone() {
        let d = doc(concat!(
            "<vars><var name=\"tint\" type=\"color\" value=\"#ff0000\"/></vars>",
            "<background><solid color=\"$tint\"/></background>"
        ));

        let resolved = resolve(&d);

        let solid = resolved.root.child("background").unwrap().child("solid");
        assert_eq!(solid.unwrap().attr("color"), Some("#ff0000"));
        let declared = resolved.root.child("vars").unwrap().child("var");
        assert_eq!(declared.unwrap().attr("value"), Some("#ff0000"));
        assert_eq!(
            d.root
                .child("background")
                .unwrap()
                .child("solid")
                .unwrap()
                .attr("color"),
            Some("$tint"),
            "the document itself still holds the reference"
        );
    }

    #[test]
    fn a_document_without_variables_is_not_copied() {
        let d = doc("<background><solid color=\"#000000\"/></background>");

        assert!(matches!(resolve(&d), std::borrow::Cow::Borrowed(_)));
    }

    #[test]
    fn a_numeric_value_outside_its_range_resolves_clamped_and_is_reported() {
        let d = doc(
            "<vars><var name=\"pad\" type=\"number\" value=\"140\" min=\"0\" max=\"100\"/></vars>",
        );
        let vars = Vars::from_document(&d);

        let pad = vars.get("pad").unwrap();
        assert_eq!(pad.resolved(), "100");
        assert!(pad.is_out_of_range());
        assert!(!pad.is_ill_typed());
    }

    #[test]
    fn a_value_that_does_not_read_as_its_type_is_ill_typed() {
        let d = doc(concat!(
            "<vars>",
            "<var name=\"n\" type=\"number\" value=\"wide\"/>",
            "<var name=\"flag\" type=\"bool\" value=\"yes\"/>",
            "<var name=\"pt\" type=\"vec2\" value=\"0.5\"/>",
            "<var name=\"pick\" type=\"select\" value=\"c\" options=\"a, b\"/>",
            "<var name=\"label\" type=\"text\" value=\"anything\"/>",
            "</vars>"
        ));
        let vars = Vars::from_document(&d);

        let ill: Vec<&str> = vars
            .iter()
            .filter(|v| v.is_ill_typed())
            .map(|v| v.name.as_str())
            .collect();
        assert_eq!(ill, ["n", "flag", "pt", "pick"]);
    }

    #[test]
    fn an_undeclared_reference_is_reported_with_the_element_that_names_it() {
        let d = doc(concat!(
            "<vars><var name=\"tint\" type=\"color\" value=\"#fff\"/></vars>",
            "<background><solid color=\"$missing\"/></background>"
        ));

        let found = unresolved(&d.root, &Vars::from_document(&d));

        assert_eq!(
            found,
            [("solid".to_owned(), "color=\"$missing\"".to_owned())]
        );
    }

    #[test]
    fn a_reference_that_is_declared_twice_resolves_to_the_first() {
        let d = doc(concat!(
            "<vars>",
            "<var name=\"tint\" type=\"color\" value=\"#111\"/>",
            "<var name=\"tint\" type=\"color\" value=\"#222\"/>",
            "</vars>"
        ));
        let vars = Vars::from_document(&d);

        assert_eq!(vars.expand("$tint").as_deref(), Some("#111"));
        assert_eq!(vars.duplicates(), ["tint"]);
    }
}
