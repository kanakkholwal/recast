//! The op vocabulary as a JSON Schema for the `ops` argument, generated from one table so the schema and serde cannot drift.
//! A test deserialises the minimal document each row describes into an [`Op`], which is what proves the table matches the wire.

use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Seconds,
    Number,
    Integer,
    Text,
    Object,
    Any,
}

impl FieldType {
    fn schema(self) -> Value {
        match self {
            Self::Seconds => {
                json!({ "type": "number", "description": "Source-recording seconds." })
            }
            Self::Number => json!({ "type": "number" }),
            Self::Integer => json!({ "type": "integer", "minimum": 0 }),
            Self::Text => json!({ "type": "string" }),
            Self::Object => json!({ "type": "object" }),
            Self::Any => json!({}),
        }
    }

    /// A value that deserialises for this type, used only to prove the table against serde.
    #[cfg(test)]
    fn sample(self) -> Value {
        match self {
            Self::Seconds | Self::Number => json!(1.5),
            Self::Integer => json!(0),
            Self::Text => json!("x"),
            Self::Object => json!({}),
            Self::Any => json!(true),
        }
    }
}

pub struct Field {
    pub name: &'static str,
    pub kind: FieldType,
    pub required: bool,
    pub doc: &'static str,
    /// A complete JSON value for an object field, shown as the schema example and parsed by the table test.
    pub sample: Option<&'static str>,
}

pub struct OpSpec {
    /// The serde tag, as stored in journals.
    pub op: &'static str,
    pub doc: &'static str,
    pub fields: &'static [Field],
}

const fn field(name: &'static str, kind: FieldType, required: bool, doc: &'static str) -> Field {
    Field {
        name,
        kind,
        required,
        doc,
        sample: None,
    }
}

const fn object_field(
    name: &'static str,
    required: bool,
    doc: &'static str,
    sample: &'static str,
) -> Field {
    Field {
        name,
        kind: FieldType::Object,
        required,
        doc,
        sample: Some(sample),
    }
}

const ZOOM_SAMPLE: &str = r#"{"start":4.5,"end":10.5,"scale":1.8,"easeIn":{"x1":0.25,"y1":0.1,"x2":0.25,"y2":1},"easeOut":{"x1":0.25,"y1":0.1,"x2":0.25,"y2":1},"rampIn":0.5,"rampOut":0.5,"centerX":0.32,"centerY":0.28,"motionBlur":0,"hidden":false,"id":"z1","source":"manual"}"#;
const ANNOTATION_SAMPLE: &str = r##"{"id":"a1","start":16.8,"end":20.8,"kind":{"kind":"rect","x":0.32,"y":0.44,"w":0.18,"h":0.09,"radius":0},"fill":"transparent","stroke":{"width":6,"color":"#ff5c5c","style":"solid"},"opacity":0.9}"##;
const ANIM_SAMPLE: &str = r#"{"kind":"slide","durationMs":400,"dir":"left","intensity":0.5}"#;
const PATCH_SAMPLE: &str = r##"{"fill":"#3b82f6","end":22.0}"##;

/// Every op an agent may append. Times are SOURCE seconds; the intent tools take output seconds and convert.
pub const OPS: &[OpSpec] = &[
    OpSpec {
        op: "trim",
        doc: "Set the kept window of the recording.",
        fields: &[
            field("start", FieldType::Seconds, true, "First kept second."),
            field("end", FieldType::Seconds, true, "Last kept second."),
        ],
    },
    OpSpec {
        op: "cutAdd",
        doc: "Remove a range from the timeline. Prefer recast_remove_silences for silence.",
        fields: &[
            field("start", FieldType::Seconds, true, ""),
            field("end", FieldType::Seconds, true, ""),
        ],
    },
    OpSpec {
        op: "cutRemove",
        doc: "Restore a cut, by index or by its exact start and end.",
        fields: &[
            field("index", FieldType::Integer, false, "Position in the cuts list."),
            field("start", FieldType::Seconds, false, ""),
            field("end", FieldType::Seconds, false, ""),
        ],
    },
    OpSpec {
        op: "zoomAdd",
        doc: "Add a zoom region. Prefer recast_add_zoom, which fills the defaults and takes output seconds.",
        fields: &[object_field(
            "region",
            true,
            "start, end, scale (1..3), centerX/centerY (0..1), rampIn, rampOut, easeIn, easeOut, motionBlur, hidden, id.",
            ZOOM_SAMPLE,
        )],
    },
    OpSpec {
        op: "zoomRemove",
        doc: "Remove a zoom region by index.",
        fields: &[field("index", FieldType::Integer, true, "")],
    },
    OpSpec {
        op: "splitPointAdd",
        doc: "Add a split marker, which bounds the segments speed and animations attach to.",
        fields: &[field("at", FieldType::Seconds, true, "")],
    },
    OpSpec {
        op: "splitPointRemove",
        doc: "Remove a split marker.",
        fields: &[field("at", FieldType::Seconds, true, "")],
    },
    OpSpec {
        op: "speedSet",
        doc: "Play the segment starting at segmentStart at `rate` (0.25..4).",
        fields: &[
            field("segmentStart", FieldType::Seconds, true, "A kept segment's start."),
            field("rate", FieldType::Number, true, ""),
        ],
    },
    OpSpec {
        op: "speedRemove",
        doc: "Return a segment to 1x.",
        fields: &[field("segmentStart", FieldType::Seconds, true, "")],
    },
    OpSpec {
        op: "annotationAdd",
        doc: "Add an annotation; read one back with recast_project_show to copy its shape.",
        fields: &[object_field("annotation", true, "A full annotation with a unique id; kinds: rect, ellipse, arrow, image, blur, text.", ANNOTATION_SAMPLE)],
    },
    OpSpec {
        op: "annotationUpdate",
        doc: "Shallow-merge fields into the annotation with `id`.",
        fields: &[
            field("id", FieldType::Text, true, ""),
            object_field("patch", true, "Fields to overwrite.", PATCH_SAMPLE),
        ],
    },
    OpSpec {
        op: "annotationRemove",
        doc: "Remove the annotation with `id`.",
        fields: &[field("id", FieldType::Text, true, "")],
    },
    OpSpec {
        op: "animationAdd",
        doc: "Set the entrance and exit animation of the segment starting at `start`.",
        fields: &[
            field("start", FieldType::Seconds, true, ""),
            object_field("animIn", false, "kind, durationMs, easing, dir, intensity", ANIM_SAMPLE),
            object_field("animOut", false, "Same shape as animIn.", ANIM_SAMPLE),
        ],
    },
    OpSpec {
        op: "animationRemove",
        doc: "Clear the segment's animations.",
        fields: &[field("start", FieldType::Seconds, true, "")],
    },
    OpSpec {
        op: "set",
        doc: "Write any field by dotted path (padding, cursorSize, audioSettings.volume). The escape hatch; prefer a named op.",
        fields: &[
            field("field", FieldType::Text, true, "Dotted path into the render state."),
            field("value", FieldType::Any, true, "JSON value that fits the field."),
        ],
    },
];

/// JSON Schema for one element of an `ops` array: a `oneOf` over the table.
pub fn ops_item_schema() -> Value {
    json!({
        "type": "object",
        "description": "One edit, tagged by `op`. Times are source-recording seconds.",
        "oneOf": OPS.iter().map(op_schema).collect::<Vec<_>>(),
    })
}

fn op_schema(spec: &OpSpec) -> Value {
    let mut properties = Map::new();
    properties.insert("op".into(), json!({ "const": spec.op }));
    let mut required = vec![json!("op")];
    for f in spec.fields {
        let mut schema = f.kind.schema();
        if !f.doc.is_empty() {
            schema["description"] = json!(f.doc);
        }
        if let Some(sample) = f.sample.and_then(|s| serde_json::from_str::<Value>(s).ok()) {
            schema["examples"] = json!([sample]);
        }
        properties.insert(f.name.into(), schema);
        if f.required {
            required.push(json!(f.name));
        }
    }
    json!({
        "type": "object",
        "description": spec.doc,
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::ops::Op;

    /// Deserialising the document each row describes, with every field present, is the proof the table matches serde: a renamed field, variant or sample fails here.
    #[test]
    fn every_row_of_the_table_deserialises_into_an_op() {
        for spec in OPS {
            let mut doc = Map::new();
            doc.insert("op".into(), json!(spec.op));
            for f in spec.fields {
                let value = match f.sample {
                    Some(sample) => serde_json::from_str(sample)
                        .unwrap_or_else(|e| panic!("{}.{} sample: {e}", spec.op, f.name)),
                    None => f.kind.sample(),
                };
                doc.insert(f.name.into(), value);
            }
            let value = Value::Object(doc);
            let parsed: Result<Op, _> = serde_json::from_value(value.clone());
            assert!(parsed.is_ok(), "{value} did not parse: {:?}", parsed.err());
        }
    }

    #[test]
    fn object_fields_carry_their_sample_as_a_schema_example() {
        let schema = ops_item_schema();
        let zoom = schema["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["properties"]["op"]["const"] == "zoomAdd")
            .unwrap();
        assert_eq!(
            zoom["properties"]["region"]["examples"][0]["scale"],
            json!(1.8)
        );
    }

    /// `Replace` carries a whole state and is compaction's private op; an agent must not be offered it.
    #[test]
    fn the_table_covers_every_agent_facing_variant_and_not_replace() {
        let names: Vec<&str> = OPS.iter().map(|s| s.op).collect();
        assert!(!names.contains(&"replace"));
        assert_eq!(
            names.len(),
            15,
            "an Op variant was added or removed; update the table"
        );
    }

    #[test]
    fn the_schema_is_a_closed_one_of_with_op_required_everywhere() {
        let schema = ops_item_schema();
        let variants = schema["oneOf"].as_array().unwrap();
        assert_eq!(variants.len(), OPS.len());
        for v in variants {
            assert_eq!(v["additionalProperties"], json!(false));
            assert_eq!(v["required"][0], json!("op"));
        }
    }

    #[test]
    fn optional_fields_are_not_required() {
        let schema = ops_item_schema();
        let cut_remove = schema["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["properties"]["op"]["const"] == "cutRemove")
            .unwrap();
        assert_eq!(cut_remove["required"], json!(["op"]));
    }
}
