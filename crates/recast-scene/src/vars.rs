//! Declared variables as they travel: the document parses them, the inspector edits them, the engine never sees one.
//! The type is here rather than in `recast-project` because the render state carries the declarations through a whole-state save.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VarType {
    Color,
    Number,
    Int,
    Bool,
    Text,
    Select,
    Font,
    Vec2,
    Angle,
    Asset,
}

impl VarType {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "color" => Self::Color,
            "number" => Self::Number,
            "int" => Self::Int,
            "bool" => Self::Bool,
            "text" => Self::Text,
            "select" => Self::Select,
            "font" => Self::Font,
            "vec2" => Self::Vec2,
            "angle" => Self::Angle,
            "asset" => Self::Asset,
            _ => return None,
        })
    }

    /// How the document spells it, which is also how the wire does.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::Number => "number",
            Self::Int => "int",
            Self::Bool => "bool",
            Self::Text => "text",
            Self::Select => "select",
            Self::Font => "font",
            Self::Vec2 => "vec2",
            Self::Angle => "angle",
            Self::Asset => "asset",
        }
    }

    /// Numeric types carry a range and are the only ones `min`, `max` and `step` apply to.
    #[must_use]
    pub fn is_numeric(self) -> bool {
        matches!(self, Self::Number | Self::Int | Self::Angle)
    }
}

/// One declaration. `value` stays text, because that is what an attribute holds and what a reference expands to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VarSpec {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: VarType,
    pub value: String,
    /// Inspector grouping, so a brand preset can arrange itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_type_round_trips_through_the_spelling_the_document_uses() {
        for ty in [
            VarType::Color,
            VarType::Number,
            VarType::Int,
            VarType::Bool,
            VarType::Text,
            VarType::Select,
            VarType::Font,
            VarType::Vec2,
            VarType::Angle,
            VarType::Asset,
        ] {
            assert_eq!(VarType::parse(ty.as_str()), Some(ty));
            let json = serde_json::to_value(ty).expect("serialize");
            assert_eq!(
                json,
                serde_json::json!(ty.as_str()),
                "the wire matches the file"
            );
        }
    }

    /// The inspector edits these, so the key names are the contract with `VarSpec` in `render-state.ts`.
    #[test]
    fn the_wire_shape_is_the_one_the_inspector_edits() {
        let var = VarSpec {
            name: "accent".into(),
            ty: VarType::Number,
            value: "42".into(),
            path: Some("Brand".into()),
            min: Some(0.0),
            max: Some(100.0),
            step: Some(1.0),
            options: Vec::new(),
        };

        let json = serde_json::to_value(&var).expect("serialize");

        assert_eq!(
            json,
            serde_json::json!({
                "name": "accent",
                "type": "number",
                "value": "42",
                "path": "Brand",
                "min": 0.0,
                "max": 100.0,
                "step": 1.0
            })
        );
        assert_eq!(
            serde_json::from_value::<VarSpec>(json).expect("deserialize"),
            var
        );
    }
}
