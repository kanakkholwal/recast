//! How a card catches light, as a small block of numbers rather than a light model (decision D-6 in `06-perspective-and-motion-graphics.md`).
//! Two terms, not the three D-6 named: the specular sweep it listed is now the `sweep` component, which composes and animates, so repeating it here would be two ways to do one thing.

use serde::{Deserialize, Serialize};

/// Per-layer shading. All zero is no shading at all, which is what every
/// project made before this had, so it must render byte-identically.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    /// How much the parts of a tilted card that lie further away darken. The
    /// one term a component cannot reach, since only the card knows its depth.
    #[serde(default)]
    pub contact: f64,
    /// How much the card's own edge lifts, the rim light.
    #[serde(default)]
    pub rim: f64,
    /// How far in from the edge the rim reaches, as a share of the shorter side.
    #[serde(default = "default_rim_width")]
    pub rim_width: f64,
}

fn default_rim_width() -> f64 {
    0.04
}

/// Hand-written rather than derived: the derive would give a zero width, and a
/// skipped-because-unlit material would come back different from the one written.
impl Default for Material {
    fn default() -> Self {
        Self::NONE
    }
}

impl Material {
    pub const NONE: Self = Self {
        contact: 0.0,
        rim: 0.0,
        rim_width: 0.04,
    };

    /// True when the block is exactly the unlit default, so the writer can leave
    /// it out. A width on its own still counts as written: dropping it would
    /// change what an author typed the next time they turn the rim up.
    #[must_use]
    pub fn is_none(&self) -> bool {
        self == &Self::NONE
    }

    /// True when it puts nothing on screen, whatever it says.
    #[must_use]
    pub fn is_unlit(&self) -> bool {
        self.contact <= 0.0 && self.rim <= 0.0
    }

    /// The four lanes the card uniform carries. The fourth is spare, kept so a
    /// third term does not change the uniform's size later.
    #[must_use]
    pub fn lanes(&self) -> [f32; 4] {
        [
            self.contact.clamp(0.0, 1.0) as f32,
            self.rim.clamp(0.0, 1.0) as f32,
            self.rim_width.clamp(0.0, 0.5) as f32,
            0.0,
        ]
    }
}

/// A material by the layer it belongs to, as the document declares it on `<screen>` and `<camera>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerMaterial {
    #[serde(flatten)]
    pub layer: crate::bind::LayerRef,
    #[serde(flatten)]
    pub material: Material,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_that_changes_nothing_reads_as_none_whatever_its_width_says() {
        assert!(Material::NONE.is_none());
        assert!(Material::default().is_none());
        let width_only = Material {
            rim_width: 0.3,
            ..Material::NONE
        };
        assert!(!width_only.is_none(), "a written width is not the default");
        assert!(width_only.is_unlit(), "but it still lights nothing");

        assert!(!Material {
            contact: 0.2,
            ..Material::NONE
        }
        .is_none());
        assert!(!Material {
            rim: 0.2,
            ..Material::NONE
        }
        .is_none());
    }

    #[test]
    fn the_lanes_are_clamped_so_a_written_value_cannot_blow_the_shader_out() {
        let wild = Material {
            contact: 4.0,
            rim: -1.0,
            rim_width: 9.0,
        };

        assert_eq!(wild.lanes(), [1.0, 0.0, 0.5, 0.0]);
    }

    #[test]
    fn the_wire_shape_flattens_onto_the_layer_it_belongs_to() {
        let entry = LayerMaterial {
            layer: crate::bind::LayerRef::Screen,
            material: Material {
                contact: 0.3,
                rim: 0.2,
                rim_width: 0.05,
            },
        };

        let json = serde_json::to_value(&entry).expect("serialize");

        assert_eq!(json["layer"], "screen");
        assert_eq!(json["contact"], 0.3);
        assert_eq!(json["rimWidth"], 0.05);
        assert_eq!(
            serde_json::from_value::<LayerMaterial>(json).expect("deserialize"),
            entry
        );
    }
}
