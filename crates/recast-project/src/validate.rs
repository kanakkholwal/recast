//! Meaning: types, ranges, enums, ids, references and timeline invariants, collected together rather than first-only.
//! Unknown elements and attributes are warnings, so a newer file opens in an older reader instead of not at all.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::document::{Document, Node};
use crate::ids::Id;
use crate::parse::Position;
use crate::schema::{self, AttrType, ElementSpec, IdRule};
use crate::value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub level: Level,
    /// Stable code an agent or a panel can branch on.
    pub code: &'static str,
    pub element: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attr: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<Position>,
}

impl Serialize for Position {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub issues: Vec<Issue>,
}

impl Report {
    pub fn errors(&self) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(|i| i.level == Level::Error)
    }

    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.errors().next().is_none()
    }

    fn push(
        &mut self,
        level: Level,
        code: &'static str,
        node: &Node,
        attr: Option<&str>,
        message: String,
    ) {
        self.issues.push(Issue {
            level,
            code,
            element: node.kind.clone(),
            id: node.id().map(str::to_owned),
            attr: attr.map(str::to_owned),
            message,
            at: node.at,
        });
    }
}

/// Runs every rule and returns everything found. `is_ok` is false only on errors.
#[must_use]
pub fn validate(doc: &Document) -> Report {
    let mut report = Report::default();
    let mut ids: BTreeMap<String, usize> = BTreeMap::new();
    let kinds_by_id = index_ids(doc, &mut ids);
    check_node(&doc.root, None, &kinds_by_id, &mut report);
    for (id, count) in ids.into_iter().filter(|(_, c)| *c > 1) {
        report.issues.push(Issue {
            level: Level::Error,
            code: "duplicate_id",
            element: String::new(),
            id: Some(id.clone()),
            attr: Some("id".into()),
            message: format!("id '{id}' is used {count} times"),
            at: None,
        });
    }
    check_timeline(doc, &mut report);
    report
}

fn index_ids(doc: &Document, counts: &mut BTreeMap<String, usize>) -> BTreeMap<String, String> {
    let mut kinds = BTreeMap::new();
    doc.root.walk(&mut |n| {
        if let Some(id) = n.id() {
            *counts.entry(id.to_owned()).or_insert(0) += 1;
            kinds.entry(id.to_owned()).or_insert_with(|| n.kind.clone());
        }
    });
    kinds
}

fn check_node(
    node: &Node,
    parent: Option<&ElementSpec>,
    kinds: &BTreeMap<String, String>,
    report: &mut Report,
) {
    let Some(spec) = schema::element(&node.kind) else {
        report.push(
            Level::Warning,
            "unknown_element",
            node,
            None,
            format!("<{}> is not in the schema; kept as is", node.kind),
        );
        return;
    };
    if let Some(parent) = parent {
        if !parent.children.contains(&spec.kind) {
            report.push(
                Level::Error,
                "misplaced_element",
                node,
                None,
                format!("<{}> may not sit inside <{}>", spec.kind, parent.kind),
            );
        }
    }
    check_id(node, spec, report);
    check_attrs(node, spec, kinds, report);
    if spec.text && node.text.is_none() && spec.kind == "text" {
        report.push(
            Level::Warning,
            "empty_text",
            node,
            None,
            "text annotation has no content".into(),
        );
    }
    check_binds(node, report);
    for child in &node.children {
        check_node(child, Some(spec), kinds, report);
    }
}

fn check_id(node: &Node, spec: &ElementSpec, report: &mut Report) {
    match (spec.id, node.id()) {
        (IdRule::Required, None) => report.push(
            Level::Error,
            "missing_id",
            node,
            Some("id"),
            format!("<{}> needs an id", spec.kind),
        ),
        (IdRule::None, Some(_)) => report.push(
            Level::Warning,
            "unexpected_id",
            node,
            Some("id"),
            format!("<{}> does not take an id", spec.kind),
        ),
        (_, Some(id)) => {
            if let Err(e) = Id::parse(id) {
                report.push(Level::Error, "bad_id", node, Some("id"), e.to_string());
            }
        }
        (_, None) => {}
    }
}

fn check_attrs(
    node: &Node,
    spec: &ElementSpec,
    kinds: &BTreeMap<String, String>,
    report: &mut Report,
) {
    for attr in spec.attrs.iter().filter(|a| a.required) {
        if node.attr(attr.name).is_none() {
            report.push(
                Level::Error,
                "missing_attr",
                node,
                Some(attr.name),
                format!("<{}> needs {}", spec.kind, attr.name),
            );
        }
    }
    for (name, raw) in &node.attrs {
        match spec.attrs.iter().find(|a| a.name == name) {
            Some(attr) => check_value(node, name, attr.ty, raw, kinds, report),
            None if spec.open_attrs || name == "id" => {}
            None => report.push(
                Level::Warning,
                "unknown_attr",
                node,
                Some(name),
                format!("{name} is not an attribute of <{}>; kept as is", spec.kind),
            ),
        }
    }
}

fn check_value(
    node: &Node,
    name: &str,
    ty: AttrType,
    raw: &str,
    kinds: &BTreeMap<String, String>,
    report: &mut Report,
) {
    let result: Result<(), String> = match ty {
        AttrType::Id | AttrType::Text => Ok(()),
        AttrType::Seconds => value::parse_num(raw).map(|_| ()).map_err(|e| e.to_string()),
        AttrType::Number { min, max } => value::parse_num(raw)
            .map_err(|e| e.to_string())
            .and_then(|v| in_range(v, min, max)),
        AttrType::Fraction => value::parse_num(raw)
            .map_err(|e| e.to_string())
            .and_then(|v| in_range(v, Some(0.0), Some(1.0))),
        AttrType::Percent => value::parse_num(raw)
            .map_err(|e| e.to_string())
            .and_then(|v| in_range(v, Some(0.0), Some(100.0))),
        AttrType::Int => value::parse_int(raw).map(|_| ()).map_err(|e| e.to_string()),
        AttrType::Bool => value::parse_bool(raw)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        AttrType::Color => value::parse_color(raw)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        AttrType::ColorOrNone if raw == "none" => Ok(()),
        AttrType::ColorOrNone => value::parse_color(raw)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        AttrType::Ease => value::parse_ease(raw)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        AttrType::Enum(options) if options.contains(&raw) => Ok(()),
        AttrType::Enum(options) => Err(format!("'{raw}' is not one of {}", options.join(", "))),
        AttrType::Src => value::parse_src(raw).map(|_| ()).map_err(|e| e.to_string()),
        AttrType::Ref(kind) => match kinds.get(raw) {
            Some(found) if found == kind => Ok(()),
            Some(found) => Err(format!("'{raw}' is a <{found}>, expected a <{kind}>")),
            None => Err(format!("no <{kind}> with id '{raw}'")),
        },
    };
    if let Err(message) = result {
        report.push(Level::Error, "bad_value", node, Some(name), message);
    }
}

fn in_range(v: f64, min: Option<f64>, max: Option<f64>) -> Result<(), String> {
    if min.is_some_and(|m| v < m) || max.is_some_and(|m| v > m) {
        return Err(format!(
            "{v} is outside {}..{}",
            min.map_or("-inf".into(), |m| m.to_string()),
            max.map_or("inf".into(), |m| m.to_string())
        ));
    }
    Ok(())
}

/// Maps that may stack on one property, applied in this order; the camera's keys-then-follow-then-dodge is the only composition the engine defines.
pub(crate) const STACKABLE: &[&str] = &["keys", "follow", "dodge"];

/// One driver per property, except the fixed camera stack: two binds naming the same prop is a conflict, not a composition.
/// The source must parse as a signal, and a property the engine cannot drive yet is a warning, not a refusal.
fn check_binds(node: &Node, report: &mut Report) {
    let mut seen: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for bind in node.children_of("bind") {
        let map = bind.attr("map").unwrap_or("");
        if let Some(src) = bind.attr("src") {
            if let Err(e) = recast_scene::bind::Signal::parse(src) {
                report.push(
                    Level::Error,
                    "unknown_signal",
                    bind,
                    Some("src"),
                    e.to_string(),
                );
            }
        }
        let camera_stack = node.kind == "camera" && STACKABLE.contains(&map);
        if !camera_stack && STACKABLE.contains(&map) {
            report.push(Level::Error, "camera_only_map", bind, Some("map"), format!("'{map}' is a camera placement rule; other elements take linear, wave, step or clamp"));
        }
        for prop in bind.attr("prop").unwrap_or("").split_whitespace() {
            if !camera_stack && !recast_scene::bind::BINDABLE.contains(&prop) {
                report.push(Level::Warning, "unbound_prop", bind, Some("prop"), format!("the engine does not drive '{prop}' yet; the binding is kept but has no effect"));
            }
        }
        for prop in bind.attr("prop").unwrap_or("").split_whitespace() {
            let maps = seen.entry(prop).or_default();
            let stacks =
                STACKABLE.contains(&map) && maps.iter().all(|m| STACKABLE.contains(m) && *m != map);
            if !maps.is_empty() && !stacks {
                report.push(Level::Error, "duplicate_driver", bind, Some("prop"), format!("'{prop}' already has a driver on this element; only keys, follow and dodge stack"));
            }
            maps.push(map);
        }
    }
}

fn check_timeline(doc: &Document, report: &mut Report) {
    let Some(tl) = doc.root.child("timeline") else {
        return;
    };
    let secs = |n: &Node, name: &str| n.attr(name).and_then(|v| value::parse_num(v).ok());
    let (in_, out) = (secs(tl, "in"), secs(tl, "out"));
    if let (Some(i), Some(o)) = (in_, out) {
        if o < i {
            report.push(
                Level::Error,
                "out_before_in",
                tl,
                Some("out"),
                format!("out {o} is before in {i}"),
            );
        }
    }
    let mut cuts: Vec<(f64, f64, &Node)> = Vec::new();
    if let Some(group) = tl.child("cuts") {
        for cut in group.children_of("cut") {
            if let (Some(at), Some(dur)) = (secs(cut, "at"), secs(cut, "dur")) {
                if dur <= 0.0 {
                    report.push(
                        Level::Error,
                        "empty_cut",
                        cut,
                        Some("dur"),
                        "a cut needs a positive duration".into(),
                    );
                }
                if in_.is_some_and(|i| at < i - 1e-4) || out.is_some_and(|o| at + dur > o + 1e-4) {
                    report.push(
                        Level::Error,
                        "cut_outside_trim",
                        cut,
                        None,
                        "cut lies outside in..out".into(),
                    );
                }
                cuts.push((at, at + dur, cut));
            }
        }
    }
    cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
    for pair in cuts.windows(2) {
        if pair[1].0 < pair[0].1 - 1e-4 {
            report.push(
                Level::Error,
                "cut_overlap",
                pair[1].2,
                None,
                "overlaps the previous cut".into(),
            );
        }
    }
    check_zooms(doc, in_, out, report);
}

fn check_zooms(doc: &Document, in_: Option<f64>, out: Option<f64>, report: &mut Report) {
    let secs = |n: &Node, name: &str| n.attr(name).and_then(|v| value::parse_num(v).ok());
    let Some(zooms) = doc.root.child("screen").and_then(|s| s.child("zooms")) else {
        return;
    };
    for zoom in zooms.children_of("zoom") {
        if let (Some(at), Some(dur)) = (secs(zoom, "at"), secs(zoom, "dur")) {
            if dur <= 0.0 {
                report.push(
                    Level::Error,
                    "empty_zoom",
                    zoom,
                    Some("dur"),
                    "a zoom needs a positive duration".into(),
                );
            }
            if in_.is_some_and(|i| at < i - 1e-4) || out.is_some_and(|o| at + dur > o + 1e-4) {
                report.push(
                    Level::Error,
                    "zoom_outside_trim",
                    zoom,
                    None,
                    "zoom lies outside in..out".into(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn codes(src: &str) -> Vec<&'static str> {
        validate(&parse(src).unwrap())
            .issues
            .into_iter()
            .map(|i| i.code)
            .collect()
    }

    #[test]
    fn a_well_formed_document_is_clean() {
        let src = r#"<recast v="3" aspect="16:9" pad="40">
  <media id="rec" kind="video" src="media/recording.mp4" dur="92.4"/>
  <track id="cur" kind="cursor" src="tracks/cursor.json" ro="true"/>
  <timeline src="rec" in="0.5" out="92.4"><cuts><cut id="c1" at="12.3" dur="2.5" source="silence"/></cuts><split id="s1" at="30"/><clip id="k1" at="30" speed="1.5"/></timeline>
  <screen id="scr" src="rec"><zooms><zoom id="z1" at="4.5" dur="6" scale="1.8" cx="0.32" cy="0.28"/></zooms></screen>
  <camera id="cam" x="0.78" y="0.72" w="0.18" h="0.18" space="canvas"><bind id="b1" prop="x y" src="cursor" map="dodge" strength="0.8"/></camera>
  <cursor id="cur1" track="cur" size="1.4"/>
</recast>"#;
        let report = validate(&parse(src).unwrap());
        assert!(report.issues.is_empty(), "{:?}", report.issues);
    }

    #[test]
    fn every_error_class_is_reported_together() {
        let src = r#"<recast v="3" aspect="4:3">
  <timeline in="10" out="5"><cuts><cut id="c1" at="1" dur="0"/><cut id="c1" at="0.5" dur="2"/></cuts></timeline>
  <screen id="scr" src="nope"><zooms><zoom at="1" dur="1" scale="9" cx="1.5"/></zooms></screen>
  <camera id="cam"><bind id="b1" prop="x" src="cursor" map="dodge"/><bind id="b2" prop="x y" src="time" map="wave"/></camera>
  <bogus/>
  <cursor id="cur1" zzz="1" highlightColor="red"/>
</recast>"#;
        let found = codes(src);
        for code in [
            "bad_value",
            "out_before_in",
            "empty_cut",
            "duplicate_id",
            "cut_overlap",
            "missing_id",
            "duplicate_driver",
            "unknown_element",
            "unknown_attr",
        ] {
            assert!(found.contains(&code), "missing {code} in {found:?}");
        }
        let report = validate(&parse(src).unwrap());
        assert!(!report.is_ok());
        let bad_ref = report
            .issues
            .iter()
            .find(|i| i.attr.as_deref() == Some("src") && i.element == "screen")
            .unwrap();
        assert!(bad_ref.message.contains("no <media> with id 'nope'"));
    }

    #[test]
    fn the_camera_stack_composes_but_a_second_wave_does_not() {
        let stack = r#"<recast v="3"><camera id="c"><bind id="k" prop="x y" src="time" map="keys"/><bind id="f" prop="x y" src="zoom.center" map="follow"/><bind id="d" prop="x y" src="cursor" map="dodge"/></camera></recast>"#;
        assert!(!codes(stack).contains(&"duplicate_driver"));
        let twice = r#"<recast v="3"><camera id="c"><bind id="a" prop="x" src="time" map="wave"/><bind id="b" prop="x" src="time" map="wave"/></camera></recast>"#;
        assert!(codes(twice).contains(&"duplicate_driver"));
    }

    #[test]
    fn unknowns_are_warnings_not_errors() {
        let report =
            validate(&parse("<recast v=\"3\"><zzz/><screen id=\"s\" q=\"1\"/></recast>").unwrap());
        assert!(report.is_ok());
        assert_eq!(report.issues.len(), 2);
    }

    #[test]
    fn a_misplaced_element_and_a_graphic_param_are_told_apart() {
        let found = codes("<recast v=\"3\"><zoom id=\"z\" at=\"1\" dur=\"1\"/><graphic id=\"g\" component=\"lower-third@1\" title=\"Hi\"/></recast>");
        assert!(found.contains(&"misplaced_element"));
        assert!(!found.contains(&"unknown_attr"));
    }

    #[test]
    fn a_reference_to_the_wrong_kind_is_named() {
        let report = validate(&parse("<recast v=\"3\"><media id=\"m\" kind=\"video\" src=\"media/a.mp4\"/><cursor id=\"c\" track=\"m\"/></recast>").unwrap());
        assert!(report
            .issues
            .iter()
            .any(|i| i.message.contains("is a <media>, expected a <track>")));
    }
}
