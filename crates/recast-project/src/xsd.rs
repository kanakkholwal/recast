//! The schema table as an XSD, so an editor completes and validates `project.rcx` without knowing Recast.
//! Generated rather than written: `schema.rs` stays the only description of the format.

use std::fmt::Write as _;

use crate::schema::{self, AttrSpec, AttrType, ElementSpec, IdRule};

/// Any attribute may hold a whole-value `$name` reference instead of a literal, so every typed attribute unions this in.
const VAR_REF: &str = "varRef";

/// The XSD for the document vocabulary, generated from the schema table.
#[must_use]
pub fn xsd() -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    out.push_str(
        "<!-- Generated from crates/recast-project/src/schema.rs. Write a copy with `recast project schema`. -->\n",
    );
    out.push_str("<xs:schema xmlns:xs=\"http://www.w3.org/2001/XMLSchema\">\n");
    write_doc(&mut out, 1, "A Recast project document, project.rcx.");
    write_var_ref(&mut out);
    for spec in schema::schema() {
        write_element(&mut out, spec);
    }
    out.push_str("</xs:schema>\n");
    out
}

fn write_var_ref(out: &mut String) {
    let _ = writeln!(out, "  <xs:simpleType name=\"{VAR_REF}\">");
    write_doc(
        out,
        2,
        "A whole-value reference to a <var>, such as $accent.",
    );
    out.push_str("    <xs:restriction base=\"xs:string\">\n");
    out.push_str("      <xs:pattern value=\"\\$[A-Za-z0-9_-]+\"/>\n");
    out.push_str("    </xs:restriction>\n");
    out.push_str("  </xs:simpleType>\n");
}

fn write_element(out: &mut String, spec: &ElementSpec) {
    let _ = writeln!(out, "  <xs:element name=\"{}\">", spec.kind);
    write_doc(out, 2, spec.doc);
    let mixed = if spec.text { " mixed=\"true\"" } else { "" };
    let _ = writeln!(out, "    <xs:complexType{mixed}>");
    if !spec.children.is_empty() {
        out.push_str("      <xs:choice minOccurs=\"0\" maxOccurs=\"unbounded\">\n");
        for kid in &spec.children {
            let _ = writeln!(out, "        <xs:element ref=\"{kid}\"/>");
        }
        out.push_str("      </xs:choice>\n");
    }
    for attr in attrs_of(spec) {
        write_attr(out, &attr);
    }
    if spec.open_attrs {
        out.push_str("      <xs:anyAttribute namespace=\"##any\" processContents=\"skip\"/>\n");
    }
    out.push_str("    </xs:complexType>\n");
    out.push_str("  </xs:element>\n");
}

/// `id` is a rule on the element rather than a row in its attribute list, except where the list already carries it.
fn attrs_of(spec: &ElementSpec) -> Vec<AttrSpec> {
    let mut out: Vec<AttrSpec> = Vec::with_capacity(spec.attrs.len() + 1);
    if spec.id != IdRule::None && !spec.attrs.iter().any(|a| a.name == "id") {
        out.push(AttrSpec {
            name: "id",
            ty: AttrType::Id,
            required: spec.id == IdRule::Required,
            doc: "",
        });
    }
    for attr in &spec.attrs {
        if !out.iter().any(|seen| seen.name == attr.name) {
            out.push(*attr);
        }
    }
    out
}

fn write_attr(out: &mut String, attr: &AttrSpec) {
    let required = if attr.required {
        " use=\"required\""
    } else {
        ""
    };
    let shape = shape(attr.ty);
    let named = match &shape {
        Shape::Named(ty) => format!(" type=\"{ty}\""),
        Shape::Union { .. } => String::new(),
    };
    if attr.doc.is_empty() && matches!(shape, Shape::Named(_)) {
        let _ = writeln!(
            out,
            "      <xs:attribute name=\"{}\"{named}{required}/>",
            attr.name
        );
        return;
    }
    let _ = writeln!(
        out,
        "      <xs:attribute name=\"{}\"{named}{required}>",
        attr.name
    );
    write_doc(out, 4, attr.doc);
    if let Shape::Union { base, body } = shape {
        write_union(out, base, &body);
    }
    out.push_str("      </xs:attribute>\n");
}

fn write_union(out: &mut String, base: &str, body: &str) {
    out.push_str("        <xs:simpleType>\n");
    if body.is_empty() {
        let _ = writeln!(
            out,
            "          <xs:union memberTypes=\"{base} {VAR_REF}\"/>"
        );
    } else {
        let _ = writeln!(out, "          <xs:union memberTypes=\"{VAR_REF}\">");
        out.push_str("            <xs:simpleType>\n");
        let _ = writeln!(out, "              <xs:restriction base=\"{base}\">");
        out.push_str(body);
        out.push_str("              </xs:restriction>\n");
        out.push_str("            </xs:simpleType>\n");
        out.push_str("          </xs:union>\n");
    }
    out.push_str("        </xs:simpleType>\n");
}

enum Shape {
    Named(&'static str),
    Union { base: &'static str, body: String },
}

fn shape(ty: AttrType) -> Shape {
    match ty {
        AttrType::Id
        | AttrType::Ref(_)
        | AttrType::Text
        | AttrType::Color
        | AttrType::ColorOrNone
        | AttrType::Ease
        | AttrType::Src => Shape::Named("xs:string"),
        AttrType::Seconds => union("xs:decimal", String::new()),
        AttrType::Int => union("xs:integer", String::new()),
        AttrType::Number { min, max } => union("xs:decimal", bounds(min, max)),
        AttrType::Fraction => union("xs:decimal", bounds(Some(0.0), Some(1.0))),
        AttrType::Percent => union("xs:decimal", bounds(Some(0.0), Some(100.0))),
        AttrType::Bool => union("xs:token", enumeration(&["true", "false"])),
        AttrType::Enum(values) => union("xs:token", enumeration(values)),
    }
}

fn union(base: &'static str, body: String) -> Shape {
    Shape::Union { base, body }
}

fn bounds(min: Option<f64>, max: Option<f64>) -> String {
    let mut out = String::new();
    if let Some(min) = min {
        let _ = writeln!(out, "                <xs:minInclusive value=\"{min}\"/>");
    }
    if let Some(max) = max {
        let _ = writeln!(out, "                <xs:maxInclusive value=\"{max}\"/>");
    }
    out
}

fn enumeration(values: &[&str]) -> String {
    let mut out = String::new();
    for value in values {
        let _ = writeln!(
            out,
            "                <xs:enumeration value=\"{}\"/>",
            esc(value)
        );
    }
    out
}

fn write_doc(out: &mut String, depth: usize, doc: &str) {
    if doc.is_empty() {
        return;
    }
    let pad = "  ".repeat(depth);
    let _ = writeln!(out, "{pad}<xs:annotation>");
    let _ = writeln!(
        out,
        "{pad}  <xs:documentation>{}</xs:documentation>",
        esc(doc)
    );
    let _ = writeln!(out, "{pad}</xs:annotation>");
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn block_for(kind: &str, xsd: &str) -> String {
        let head = format!("  <xs:element name=\"{kind}\">");
        let start = xsd
            .find(&head)
            .unwrap_or_else(|| panic!("{kind} is not declared"));
        let rest = &xsd[start + head.len()..];
        let end = rest.find("\n  <xs:element name=\"").unwrap_or(rest.len());
        rest[..end].to_owned()
    }

    #[test]
    fn every_element_in_the_table_is_declared() {
        let xsd = xsd();

        let missing: Vec<&str> = schema::schema()
            .iter()
            .map(|spec| spec.kind)
            .filter(|kind| !xsd.contains(&format!("<xs:element name=\"{kind}\">")))
            .collect();

        assert_eq!(missing, Vec::<&str>::new());
    }

    #[test]
    fn every_attribute_is_declared_on_its_element() {
        let xsd = xsd();
        let mut missing = Vec::new();

        for spec in schema::schema() {
            let block = block_for(spec.kind, &xsd);
            for attr in attrs_of(spec) {
                if !block.contains(&format!("<xs:attribute name=\"{}\"", attr.name)) {
                    missing.push(format!("{}/{}", spec.kind, attr.name));
                }
            }
        }

        assert_eq!(missing, Vec::<String>::new());
    }

    #[test]
    fn a_child_the_table_allows_is_reachable_in_the_schema() {
        let xsd = xsd();
        let mut missing = Vec::new();

        for spec in schema::schema() {
            let block = block_for(spec.kind, &xsd);
            for kid in &spec.children {
                if !block.contains(&format!("<xs:element ref=\"{kid}\"/>")) {
                    missing.push(format!("{}>{}", spec.kind, kid));
                }
            }
        }

        assert_eq!(missing, Vec::<String>::new());
    }

    #[test]
    fn an_enum_attribute_offers_its_values() {
        let block = block_for("arrow", &xsd());

        assert!(
            block.contains("<xs:enumeration value=\"dashed\"/>"),
            "{block}"
        );
    }

    #[test]
    fn a_typed_attribute_also_accepts_a_variable_reference() {
        let block = block_for("zoom", &xsd());

        assert!(
            block.contains(&format!("memberTypes=\"xs:decimal {VAR_REF}\"")),
            "{block}"
        );
    }

    #[test]
    fn an_element_with_open_attributes_allows_unknown_ones() {
        let block = block_for("graphic", &xsd());

        assert!(block.contains("<xs:anyAttribute"), "{block}");
    }

    #[test]
    fn the_generated_schema_is_well_formed_xml() {
        let xsd = xsd();
        let mut reader = quick_xml::Reader::from_str(&xsd);
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Eof) => break,
                Ok(_) => buf.clear(),
                Err(e) => panic!("not well formed: {e}"),
            }
        }
    }

    #[test]
    fn the_checked_in_schema_matches_the_table() {
        let generated = xsd();
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("schema/project.xsd");
        if std::env::var("RECAST_WRITE_SCHEMA").is_ok() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &generated).unwrap();
        }

        let checked_in = std::fs::read_to_string(&path)
            .unwrap()
            .replace("\r\n", "\n");

        assert_eq!(
            checked_in, generated,
            "schema/project.xsd is stale; regenerate with RECAST_WRITE_SCHEMA=1"
        );
    }
}
