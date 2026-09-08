//! `Document` to canonical text. Attribute order comes from the schema, values are re-spelled by type, indentation is fixed.
//! Two writers of the same tree produce the same bytes, which is what `hash` and git diffs rely on.

use std::fmt::Write as _;

use crate::document::{Document, Node};
use crate::schema::{self, AttrType};
use crate::value;

const INDENT: &str = "  ";

#[must_use]
pub fn serialize(doc: &Document) -> String {
    let mut out = String::new();
    write_node(&mut out, &doc.root, 0);
    out
}

fn write_node(out: &mut String, node: &Node, depth: usize) {
    for _ in 0..depth {
        out.push_str(INDENT);
    }
    out.push('<');
    out.push_str(&node.kind);
    for (name, raw) in ordered_attrs(node) {
        let _ = write!(
            out,
            " {name}=\"{}\"",
            escape(&canonical(&node.kind, name, raw))
        );
    }
    match (&node.text, node.children.is_empty()) {
        (None, true) => out.push_str("/>\n"),
        (Some(text), true) => {
            let _ = writeln!(out, ">{}</{}>", escape(text), node.kind);
        }
        (text, false) => {
            out.push_str(">\n");
            if let Some(text) = text {
                for _ in 0..=depth {
                    out.push_str(INDENT);
                }
                out.push_str(&escape(text));
                out.push('\n');
            }
            for child in &node.children {
                write_node(out, child, depth + 1);
            }
            for _ in 0..depth {
                out.push_str(INDENT);
            }
            let _ = writeln!(out, "</{}>", node.kind);
        }
    }
}

/// Schema order first, then anything the schema does not know, alphabetically. `id` always leads.
fn ordered_attrs(node: &Node) -> Vec<(&str, &str)> {
    let mut out: Vec<(&str, &str)> = Vec::with_capacity(node.attrs.len());
    if let Some(id) = node.attr("id") {
        out.push(("id", id));
    }
    if let Some(spec) = schema::element(&node.kind) {
        for attr in &spec.attrs {
            if attr.name != "id" {
                if let Some(v) = node.attr(attr.name) {
                    out.push((attr.name, v));
                }
            }
        }
    }
    for (name, v) in &node.attrs {
        if name != "id" && !out.iter().any(|(n, _)| n == name) {
            out.push((name.as_str(), v.as_str()));
        }
    }
    out
}

/// Re-spells a value in its canonical form when the schema knows its type; unknown attributes pass through.
fn canonical(kind: &str, name: &str, raw: &str) -> String {
    let Some(ty) = schema::attr_type(kind, name) else {
        return raw.to_owned();
    };
    let spelled = match ty {
        AttrType::Seconds => value::parse_num(raw).map(value::fmt_secs),
        AttrType::Number { .. } | AttrType::Fraction | AttrType::Percent => {
            value::parse_num(raw).map(value::fmt_num)
        }
        AttrType::Color => value::parse_color(raw),
        AttrType::Ease => value::parse_ease(raw).map(value::fmt_ease),
        AttrType::Bool => value::parse_bool(raw).map(|b| b.to_string()),
        _ => Ok(raw.to_owned()),
    };
    spelled.unwrap_or_else(|_| raw.to_owned())
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    #[test]
    fn output_is_indented_ordered_and_re_spelled() {
        let src = "<recast v=\"3\"><screen id=\"scr\"><zoom scale=\"1.80\" id=\"z1\" at=\"4.5\" cx=\"0.320\"/></screen></recast>";
        let out = serialize(&parse(src).unwrap());
        assert_eq!(
            out,
            "<recast v=\"3\">\n  <screen id=\"scr\">\n    <zoom id=\"z1\" at=\"4.500\" scale=\"1.8\" cx=\"0.32\"/>\n  </screen>\n</recast>\n"
        );
    }

    #[test]
    fn serialize_then_parse_is_identity_on_the_tree() {
        let src = "<recast v=\"3\"><annotations><text id=\"t1\" at=\"1\" dur=\"2\">a &lt; b &amp; \"c\"</text></annotations><background><solid color=\"#FF5C5C\"/></background></recast>";
        let doc = parse(src).unwrap();
        let again = parse(&serialize(&doc)).unwrap();
        let strip = |mut d: Document| {
            d.root.walk(&mut |_| {});
            fn clear(n: &mut Node) {
                n.at = None;
                n.children.iter_mut().for_each(clear);
            }
            clear(&mut d.root);
            d
        };
        let (a, b) = (strip(doc), strip(again));
        assert_eq!(serialize(&a), serialize(&b));
        assert_eq!(b.find("t1").unwrap().text.as_deref(), Some("a < b & \"c\""));
        assert_eq!(
            b.root
                .child("background")
                .unwrap()
                .child("solid")
                .unwrap()
                .attr("color"),
            Some("#ff5c5c")
        );
    }

    #[test]
    fn unknown_attributes_survive_after_the_known_ones() {
        let src = "<recast v=\"3\"><screen id=\"scr\" zzz=\"1\" pad=\"40\"/></recast>";
        let out = serialize(&parse(src).unwrap());
        assert!(
            out.contains("<screen id=\"scr\" pad=\"40\" zzz=\"1\"/>"),
            "{out}"
        );
    }
}
