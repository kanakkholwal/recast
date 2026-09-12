//! The component registry: vetted render recipes with typed parameters, named and versioned by the document.
//! First-party and in-repo by construction. A document names a component and its parameters; it never carries code, so nothing here is compiled at render time.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// What an instance pins: major and minor only, because patch is ours to move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pin {
    pub major: u16,
    pub minor: u16,
}

impl fmt::Display for Pin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParamType {
    Color,
    Number {
        min: f64,
        max: f64,
    },
    Fraction,
    Angle,
    Bool,
    /// Any string. What a title says is not the format's business.
    Text,
}

#[derive(Debug, Clone, Copy)]
pub struct ParamSpec {
    pub name: &'static str,
    pub ty: ParamType,
    /// Spelled as the document would, so a missing parameter and a written default read the same way.
    pub default: &'static str,
    pub doc: &'static str,
}

/// Where a recipe draws: its own layer, or over the screen card it is attached to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Surface {
    #[default]
    Overlay,
    Screen,
}

pub struct Manifest {
    pub name: &'static str,
    pub version: Version,
    pub surface: Surface,
    /// The recipe the compositor selects on; the registry owns these numbers.
    pub recipe: u32,
    /// Whether the recipe puts pixels through the component pass. A text-only
    /// one does not, and skipping it saves a draw call that would only discard.
    pub shape: bool,
    pub doc: &'static str,
    pub params: &'static [ParamSpec],
}

impl Manifest {
    /// The value to render a parameter with: what the instance wrote, else the declared default.
    #[must_use]
    pub fn value<'a>(&self, params: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str>
    where
        'static: 'a,
    {
        params
            .get(name)
            .map(String::as_str)
            .or_else(|| self.param(name).map(|p| p.default))
    }

    #[must_use]
    pub fn param(&self, name: &str) -> Option<&'static ParamSpec> {
        self.params.iter().find(|p| p.name == name)
    }
}

const fn num(
    name: &'static str,
    min: f64,
    max: f64,
    default: &'static str,
    doc: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Number { min, max },
        default,
        doc,
    }
}

const fn frac(name: &'static str, default: &'static str, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Fraction,
        default,
        doc,
    }
}

const fn text(name: &'static str, default: &'static str, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Text,
        default,
        doc,
    }
}

const fn colour(name: &'static str, default: &'static str, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Color,
        default,
        doc,
    }
}

const fn v(major: u16, minor: u16, patch: u16) -> Version {
    Version {
        major,
        minor,
        patch,
    }
}

/// Recipe 0 is the placeholder every unresolved instance falls back to, so a component that cannot be found still draws something named.
pub const PLACEHOLDER: u32 = 0;

pub static REGISTRY: &[Manifest] = &[
    Manifest {
        name: "spotlight",
        version: v(1, 0, 0),
        shape: true,
        surface: Surface::Overlay,
        recipe: 1,
        doc: "Darkens everything outside a soft ellipse, to put the eye somewhere.",
        params: &[
            frac("cx", "0.5", "Centre, fraction of the canvas width."),
            frac("cy", "0.5", "Centre, fraction of the canvas height."),
            frac(
                "radius",
                "0.3",
                "Clear radius, fraction of the shorter canvas edge.",
            ),
            frac(
                "feather",
                "0.25",
                "How far the edge fades, as a share of the radius.",
            ),
            frac("strength", "0.6", "How dark it gets outside."),
            colour("tint", "#000000", "What the outside is washed towards."),
        ],
    },
    Manifest {
        name: "shape-reveal",
        version: v(1, 0, 0),
        shape: true,
        surface: Surface::Overlay,
        recipe: 2,
        doc: "A rounded panel that wipes in along an angle over its own duration.",
        params: &[
            colour("fill", "#ffffff", "Panel colour."),
            frac(
                "radius",
                "0.08",
                "Corner radius, fraction of the shorter edge.",
            ),
            num(
                "angle",
                -360.0,
                360.0,
                "0",
                "Wipe direction in degrees; 0 wipes left to right.",
            ),
            frac("softness", "0.08", "Width of the wipe's soft edge."),
            frac(
                "hold",
                "0.2",
                "Share of the duration the panel stays fully in before wiping out.",
            ),
            frac("x", "0.1", "Left edge, fraction of the canvas."),
            frac("y", "0.4", "Top edge, fraction of the canvas."),
            frac("w", "0.8", "Width, fraction of the canvas."),
            frac("h", "0.2", "Height, fraction of the canvas."),
        ],
    },
    Manifest {
        name: "mesh",
        version: v(1, 0, 0),
        shape: true,
        surface: Surface::Overlay,
        recipe: 4,
        doc: "Two soft colour blooms drifting over each other, the gradient backdrop.",
        params: &[
            colour("a", "#7c3aed", "First bloom."),
            colour("b", "#06b6d4", "Second bloom."),
            frac("spread", "0.55", "How wide each bloom reaches."),
            num(
                "cycles",
                0.0,
                60.0,
                "1",
                "How many times they orbit over the duration.",
            ),
            frac("strength", "0.85", "How opaque the blooms are."),
        ],
    },
    Manifest {
        name: "sweep",
        version: v(1, 0, 0),
        shape: true,
        surface: Surface::Screen,
        recipe: 3,
        doc: "A gloss band travelling across the screen card, the hero shader.",
        params: &[
            colour("tint", "#ffffff", "Band colour."),
            frac("width", "0.25", "Band width as a share of the card."),
            num("angle", -360.0, 360.0, "35", "Band angle in degrees."),
            num(
                "cycles",
                0.0,
                60.0,
                "1",
                "How many times it crosses over the duration.",
            ),
            frac("strength", "0.35", "Peak brightness it adds."),
        ],
    },
    Manifest {
        name: "title-card",
        version: v(1, 0, 0),
        shape: true,
        surface: Surface::Overlay,
        recipe: 5,
        doc: "A panel with a title and a subtitle, wiping in and out over its own duration.",
        params: &[
            text("title", "Title", "The headline."),
            text(
                "subtitle",
                "",
                "A second line under it; empty leaves it out.",
            ),
            text(
                "font",
                "",
                "Family to draw with; empty takes the project's own.",
            ),
            colour("fill", "#101014", "Panel colour."),
            colour("color", "#ffffff", "Text colour."),
            frac("size", "0.12", "Title size, share of the frame height."),
            frac(
                "subSize",
                "0.05",
                "Subtitle size, share of the frame height.",
            ),
            num("align", -1.0, 1.0, "0", "-1 left, 0 centre, 1 right."),
            frac(
                "radius",
                "0.04",
                "Corner radius, fraction of the shorter edge.",
            ),
            frac("softness", "0.06", "Width of the wipe's soft edge."),
            frac(
                "hold",
                "0.6",
                "Share of the duration the panel stays fully in.",
            ),
            frac("x", "0.12", "Left edge, fraction of the canvas."),
            frac("y", "0.3", "Top edge, fraction of the canvas."),
            frac("w", "0.76", "Width, fraction of the canvas."),
            frac("h", "0.4", "Height, fraction of the canvas."),
        ],
    },
    Manifest {
        name: "lower-third",
        version: v(1, 0, 0),
        shape: true,
        surface: Surface::Overlay,
        recipe: 6,
        doc: "A name and a role on a bar in the lower left, the broadcast introduction.",
        params: &[
            text("name", "Name", "Who this is."),
            text("role", "", "What they do; empty leaves it out."),
            text(
                "font",
                "",
                "Family to draw with; empty takes the project's own.",
            ),
            colour("fill", "#101014", "Bar colour."),
            colour("color", "#ffffff", "Text colour."),
            frac("size", "0.06", "Name size, share of the frame height."),
            frac("subSize", "0.035", "Role size, share of the frame height."),
            frac(
                "radius",
                "0.02",
                "Corner radius, fraction of the shorter edge.",
            ),
            frac("softness", "0.05", "Width of the wipe's soft edge."),
            frac(
                "hold",
                "0.7",
                "Share of the duration the bar stays fully in.",
            ),
            frac("x", "0.06", "Left edge, fraction of the canvas."),
            frac("y", "0.72", "Top edge, fraction of the canvas."),
            frac("w", "0.45", "Width, fraction of the canvas."),
            frac("h", "0.16", "Height, fraction of the canvas."),
        ],
    },
    Manifest {
        name: "counter",
        version: v(1, 0, 0),
        shape: false,
        surface: Surface::Overlay,
        recipe: 7,
        doc: "A number counting from one value to another over the instance's duration.",
        params: &[
            num("from", -1.0e9, 1.0e9, "0", "Where it starts."),
            num("to", -1.0e9, 1.0e9, "100", "Where it lands."),
            num("decimals", 0.0, 6.0, "0", "Digits after the point."),
            text("prefix", "", "Written before the number."),
            text("suffix", "", "Written after it."),
            text(
                "font",
                "",
                "Family to draw with; empty takes the project's own.",
            ),
            colour("color", "#ffffff", "Text colour."),
            frac("size", "0.14", "Share of the frame height."),
            num("align", -1.0, 1.0, "0", "-1 left, 0 centre, 1 right."),
            frac("x", "0.1", "Left edge, fraction of the canvas."),
            frac("y", "0.4", "Top edge, fraction of the canvas."),
            frac("w", "0.8", "Width, fraction of the canvas."),
            frac("h", "0.2", "Height, fraction of the canvas."),
        ],
    },
];

#[must_use]
pub fn manifest(name: &str) -> Option<&'static Manifest> {
    REGISTRY.iter().find(|m| m.name == name)
}

/// What a `name@major.minor` reference resolved to, and why, so `check` can say something useful and the renderer can still draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// The registry has this major and at least this minor.
    Ready {
        name: &'static str,
        recipe: u32,
        surface: Surface,
        /// True when the registry moved past the pin and adopted it, which patch and minor both allow.
        adopted: bool,
    },
    /// The name is known but the registry is older than the pin, so a parameter the document uses may not exist.
    TooNew {
        name: String,
        have: Version,
        want: Pin,
    },
    /// A different major: additive rules do not apply, so this needs an explicit migration.
    MajorMismatch {
        name: String,
        have: Version,
        want: Pin,
    },
    Unknown {
        name: String,
    },
    Malformed {
        spec: String,
    },
}

impl Resolution {
    #[must_use]
    pub fn recipe(&self) -> u32 {
        match self {
            Self::Ready { recipe, .. } => *recipe,
            _ => PLACEHOLDER,
        }
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    /// What `check` tells the user, or `None` when the instance resolved cleanly.
    #[must_use]
    pub fn complaint(&self) -> Option<String> {
        match self {
            Self::Ready { .. } => None,
            Self::TooNew { name, have, want } => Some(format!(
                "{name}@{want} needs a newer Recast; this one has {have}"
            )),
            Self::MajorMismatch { name, have, want } => Some(format!(
                "{name}@{want} is a different major version to {have}; it needs an explicit migration"
            )),
            Self::Unknown { name } => Some(format!("no component named '{name}'")),
            Self::Malformed { spec } => {
                Some(format!("'{spec}' is not a component@major.minor reference"))
            }
        }
    }
}

/// Resolves `name@major.minor` against the registry.
/// Patch and minor are ours to move forward (a minor only adds parameters, which take their defaults), so a newer registry adopts an older pin.
#[must_use]
pub fn resolve(spec: &str) -> Resolution {
    let Some(pin) = parse_pin(spec) else {
        return Resolution::Malformed {
            spec: spec.to_owned(),
        };
    };
    let (name, want) = pin;
    let Some(found) = manifest(name) else {
        return Resolution::Unknown {
            name: name.to_owned(),
        };
    };
    against(found, want)
}

/// The version rule on its own: same major, and a registry at or past the pinned minor.
fn against(found: &'static Manifest, want: Pin) -> Resolution {
    if found.version.major != want.major {
        return Resolution::MajorMismatch {
            name: found.name.to_owned(),
            have: found.version,
            want,
        };
    }
    if found.version.minor < want.minor {
        return Resolution::TooNew {
            name: found.name.to_owned(),
            have: found.version,
            want,
        };
    }
    Resolution::Ready {
        name: found.name,
        recipe: found.recipe,
        surface: found.surface,
        adopted: found.version.minor > want.minor,
    }
}

fn parse_pin(spec: &str) -> Option<(&str, Pin)> {
    let (name, version) = spec.split_once('@')?;
    if name.is_empty() {
        return None;
    }
    let (major, minor) = version.split_once('.')?;
    Some((
        name,
        Pin {
            major: major.parse().ok()?,
            minor: minor.parse().ok()?,
        },
    ))
}

/// A component instance: what to draw, with what, and when.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphicSpec {
    pub id: String,
    /// `name@major.minor` as the document spelled it, kept verbatim so a round trip cannot rewrite the pin.
    pub component: String,
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub duration: f64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
    /// Where the element that declared this wants it drawn, used when the component does not resolve.
    #[serde(default)]
    pub fallback_surface: Surface,
}

impl GraphicSpec {
    #[must_use]
    pub fn resolution(&self) -> Resolution {
        resolve(&self.component)
    }

    /// Progress through the instance's own window, 0 to 1; a zero duration is always at its end.
    #[must_use]
    pub fn progress(&self, time: f64) -> f64 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        ((time - self.start) / self.duration).clamp(0.0, 1.0)
    }

    #[must_use]
    pub fn covers(&self, time: f64) -> bool {
        self.duration <= 0.0 || (time >= self.start && time <= self.start + self.duration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pin_resolves_when_the_registry_is_at_or_past_it() {
        assert!(resolve("spotlight@1.0").is_ready());
        assert_eq!(
            resolve("spotlight@1.0"),
            Resolution::Ready {
                name: "spotlight",
                recipe: 1,
                surface: Surface::Overlay,
                adopted: false,
            }
        );
    }

    static AHEAD: Manifest = Manifest {
        name: "ahead",
        version: v(2, 4, 1),
        shape: true,
        surface: Surface::Overlay,
        recipe: 9,
        doc: "",
        params: &[],
    };

    fn pin(major: u16, minor: u16) -> Pin {
        Pin { major, minor }
    }

    #[test]
    fn a_registry_ahead_on_the_minor_adopts_an_older_pin_because_a_minor_only_adds() {
        assert_eq!(
            against(&AHEAD, pin(2, 1)),
            Resolution::Ready {
                name: "ahead",
                recipe: 9,
                surface: Surface::Overlay,
                adopted: true,
            }
        );
    }

    #[test]
    fn a_patch_moves_underneath_a_pin_without_being_called_an_adoption() {
        match against(&AHEAD, pin(2, 4)) {
            Resolution::Ready { adopted, .. } => {
                assert!(!adopted, "2.4.1 is what 2.4 asked for; the patch is ours");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_pin_past_the_registrys_minor_is_refused_rather_than_guessed_at() {
        assert_eq!(
            against(&AHEAD, pin(2, 9)),
            Resolution::TooNew {
                name: "ahead".to_owned(),
                have: v(2, 4, 1),
                want: pin(2, 9),
            }
        );
        assert!(matches!(
            against(&AHEAD, pin(1, 0)),
            Resolution::MajorMismatch { .. }
        ));
    }

    #[test]
    fn a_pin_the_registry_cannot_meet_still_names_the_component_and_draws_the_placeholder() {
        let newer = resolve("spotlight@1.7");
        let major = resolve("spotlight@2.0");
        let missing = resolve("nope@1.0");
        let junk = resolve("spotlight");

        for case in [&newer, &major, &missing, &junk] {
            assert_eq!(case.recipe(), PLACEHOLDER, "{case:?}");
            assert!(case.complaint().is_some(), "{case:?}");
        }
        assert!(newer.complaint().unwrap().contains("newer Recast"));
        assert!(major.complaint().unwrap().contains("explicit migration"));
    }

    #[test]
    fn a_parameter_falls_back_to_its_declared_default() {
        let m = manifest("spotlight").unwrap();
        let mut params = BTreeMap::new();
        params.insert("radius".to_owned(), "0.1".to_owned());

        assert_eq!(m.value(&params, "radius"), Some("0.1"));
        assert_eq!(m.value(&params, "cx"), Some("0.5"));
        assert_eq!(m.value(&params, "nonsense"), None);
    }

    #[test]
    fn every_registered_recipe_number_is_unique_and_not_the_placeholder() {
        let mut seen = std::collections::BTreeSet::new();
        for m in REGISTRY {
            assert_ne!(m.recipe, PLACEHOLDER, "{} took the placeholder", m.name);
            assert!(seen.insert(m.recipe), "{} reused a recipe number", m.name);
        }
    }

    #[test]
    fn every_declared_default_reads_as_its_own_type() {
        for m in REGISTRY {
            for p in m.params {
                let ok = match p.ty {
                    ParamType::Color => p.default.starts_with('#'),
                    ParamType::Bool => matches!(p.default, "true" | "false"),
                    ParamType::Fraction => p
                        .default
                        .parse::<f64>()
                        .is_ok_and(|v| (0.0..=1.0).contains(&v)),
                    ParamType::Number { min, max } => p
                        .default
                        .parse::<f64>()
                        .is_ok_and(|v| (min..=max).contains(&v)),
                    ParamType::Angle => p.default.parse::<f64>().is_ok(),
                    ParamType::Text => true,
                };
                assert!(ok, "{}.{} default '{}'", m.name, p.name, p.default);
            }
        }
    }

    #[test]
    fn an_instance_reports_its_own_window() {
        let g = GraphicSpec {
            id: "g1".into(),
            component: "spotlight@1.0".into(),
            start: 2.0,
            duration: 4.0,
            params: BTreeMap::new(),
            fallback_surface: Surface::Overlay,
        };

        assert!((g.progress(2.0) - 0.0).abs() < 1e-9);
        assert!((g.progress(4.0) - 0.5).abs() < 1e-9);
        assert!((g.progress(9.0) - 1.0).abs() < 1e-9, "clamped past the end");
        assert!(!g.covers(1.0));
        assert!(g.covers(3.0));
    }

    /// The editor carries these through a save without understanding them, so
    /// the key names are the contract with `GraphicSpec` in `render-state.ts`.
    #[test]
    fn the_wire_shape_is_the_one_the_editor_carries() {
        let g = GraphicSpec {
            id: "g1".into(),
            component: "sweep@1.0".into(),
            start: 1.5,
            duration: 4.0,
            params: [("angle".to_owned(), "20".to_owned())]
                .into_iter()
                .collect(),
            fallback_surface: Surface::Screen,
        };

        let json = serde_json::to_value(&g).expect("serialize");

        assert_eq!(
            json,
            serde_json::json!({
                "id": "g1",
                "component": "sweep@1.0",
                "start": 1.5,
                "duration": 4.0,
                "params": { "angle": "20" },
                "fallbackSurface": "screen"
            })
        );
        let back: GraphicSpec = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, g);
    }

    #[test]
    fn an_instance_with_no_duration_is_always_on() {
        let g = GraphicSpec {
            id: "g1".into(),
            component: "sweep@1.0".into(),
            start: 0.0,
            duration: 0.0,
            params: BTreeMap::new(),
            fallback_surface: Surface::Screen,
        };

        assert!(g.covers(1000.0));
        assert!((g.progress(1000.0) - 1.0).abs() < 1e-9);
    }
}
