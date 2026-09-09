//! The relational mapping: a property driven by a signal through a closed map. Typed data, no code, one hop.
//! Pure and allocation-free per sample, so the evaluator can run it per frame in wasm and native alike.

use serde::{Deserialize, Serialize};

use crate::v1::easing::Easing;

/// What a binding reads. Closed: an unknown source is a validation error, never a runtime lookup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Signal {
    /// Seconds since the binding's window opened.
    Time,
    /// The pointer as a pair; a scalar map reads its x.
    Cursor,
    CursorX,
    CursorY,
    CursorPressed,
    CursorIdle,
    ZoomScale,
    /// The zoom focus as a pair; a scalar map reads its x.
    ZoomCenter,
    ZoomCenterX,
    ZoomCenterY,
    AudioLevel(String),
    WordActive,
    /// A property of a layer BELOW this one in z-order, by layer id and property name.
    LayerProp {
        layer: String,
        prop: String,
    },
}

impl Signal {
    /// Parses the `src` attribute grammar: `time`, `cursor.x`, `zoom.scale`, `audio.level(mic)`, `layer(k1).x`.
    pub fn parse(text: &str) -> Result<Self, BindError> {
        let bad = || BindError::UnknownSignal(text.to_owned());
        Ok(match text {
            "time" => Self::Time,
            "cursor" => Self::Cursor,
            "cursor.x" => Self::CursorX,
            "cursor.y" => Self::CursorY,
            "cursor.pressed" => Self::CursorPressed,
            "cursor.idle" => Self::CursorIdle,
            "zoom.scale" => Self::ZoomScale,
            "zoom.center" => Self::ZoomCenter,
            "zoom.center.x" => Self::ZoomCenterX,
            "zoom.center.y" => Self::ZoomCenterY,
            "word.active" => Self::WordActive,
            _ => {
                if let Some(track) = text
                    .strip_prefix("audio.level(")
                    .and_then(|r| r.strip_suffix(')'))
                {
                    return Ok(Self::AudioLevel(track.to_owned()));
                }
                let inner = text.strip_prefix("layer(").ok_or_else(bad)?;
                let (layer, prop) = inner.split_once(").").ok_or_else(bad)?;
                if layer.is_empty() || prop.is_empty() {
                    return Err(bad());
                }
                Self::LayerProp {
                    layer: layer.to_owned(),
                    prop: prop.to_owned(),
                }
            }
        })
    }
}

/// One keyframe of a `keys` map: the value the property holds at `at` seconds into the window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    pub at: f64,
    pub value: f64,
    /// Easing INTO this key from the previous one.
    pub ease: Easing,
}

/// The closed scalar map set. The camera's `keys`, `follow` and `dodge` are vector rules on its placement and stay in the
/// compositor; the validator refuses those names elsewhere. No lag map: the evaluator is frame-pure (a frame depends only
/// on its own time, so seeking and export agree), and a lag needs state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "map", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Map {
    /// Piecewise eased interpolation over time; holds the first key before it and the last after it.
    Keys(Vec<Key>),
    /// `from..to` over the signal's `0..1`; with `period`, the signal wraps every `period` and `loop` decides whether it restarts.
    Linear {
        from: f64,
        to: f64,
        period: Option<f64>,
        looped: bool,
    },
    /// A sine between `from` and `to` with the given period, phase from the signal.
    Wave {
        from: f64,
        to: f64,
        period: f64,
    },
    /// `from` until the signal crosses `0.5`, then `to`.
    Step {
        from: f64,
        to: f64,
    },
    Clamp {
        from: f64,
        to: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BindError {
    #[error("unknown signal '{0}'")]
    UnknownSignal(String),
    #[error("'{prop}' already has a driver on this element")]
    TwoDrivers { prop: String },
    #[error("'{layer}' is not below this layer, so it cannot be read (one hop, z-order only)")]
    NotBelow { layer: String },
    #[error("a binding cannot read its own layer")]
    SelfRead,
}

/// A property driven by a signal through a map inside a window; outside the window the static value stands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    pub prop: String,
    pub signal: Signal,
    /// Flattened: the map's name and its parameters sit beside `prop` and `signal` on the wire.
    #[serde(flatten)]
    pub map: Map,
    /// Window on the layer's clock, seconds; `None` is the whole layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<(f64, f64)>,
}

/// Which layer a binding sits on, as the v1 state names layers (it has no layer ids of its own).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "layer",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LayerRef {
    Screen,
    Camera,
    Annotation { id: String },
}

/// A binding with the layer it drives; what the v1 state carries and `migrate::to_scene` attaches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerBinding {
    #[serde(flatten)]
    pub layer: LayerRef,
    #[serde(flatten)]
    pub binding: Binding,
}

/// The camera's placement rules: the three `<bind>`s on `x y w h`, evaluated in this order and no other
/// (keys set the base, follow moves it with the zoom, dodge nudges it off the pointer). Vector maps, so they
/// live beside the scalar set rather than in it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "rule",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PlacementRule {
    Keys {
        keys: Vec<PlacementKey>,
        ease: Easing,
    },
    Follow {
        strength: f64,
        duration: f64,
        ease: Easing,
    },
    Dodge {
        strength: f64,
    },
}

/// A placement keyframe: where the bubble is at `at` seconds, in frame fractions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacementKey {
    pub at: f64,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl PlacementRule {
    /// The rules a camera's settings declare, in evaluation order. The settings stay the wire form the editor edits.
    pub fn from_settings(c: &crate::v1::nodes::CameraOverlaySettings) -> Vec<Self> {
        let mut rules = Vec::new();
        if !c.keyframes.is_empty() {
            rules.push(Self::Keys {
                keys: c
                    .keyframes
                    .iter()
                    .map(|k| PlacementKey {
                        at: k.at_sec,
                        x: k.placement.x,
                        y: k.placement.y,
                        w: k.placement.width,
                        h: k.placement.height,
                    })
                    .collect(),
                ease: c.keyframe_easing,
            });
        }
        if c.zoom_follow {
            rules.push(Self::Follow {
                strength: c.zoom_follow_strength,
                duration: c.zoom_follow_duration,
                ease: c.zoom_follow_easing,
            });
        }
        if c.cursor_dodge {
            rules.push(Self::Dodge {
                strength: c.cursor_dodge_strength,
            });
        }
        rules
    }
}

/// The properties the compositor can drive today; `check` warns on anything else.
pub const BINDABLE: &[&str] = &["opacity", "blur", "x", "y", "z", "rx", "ry", "rz", "scale"];

/// A layer's 3D transform, rendered as a homography (06, D-1): offsets in frame fractions (`z` in frame heights, away is
/// positive), rotations in degrees, `scale` about the anchor, `perspective` the focal length in frame heights.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Transform3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub rx: f64,
    pub ry: f64,
    pub rz: f64,
    pub scale: f64,
    pub anchor_x: f64,
    pub anchor_y: f64,
    pub perspective: f64,
}

impl Default for Transform3 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform3 {
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        rx: 0.0,
        ry: 0.0,
        rz: 0.0,
        scale: 1.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        perspective: 2.0,
    };

    /// True when the transform changes nothing, so the flat path (and its goldens) stays in force.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }

    /// The transform after a layer's bindings on its fields; `sample` reads each field's static value as the rest.
    #[must_use]
    pub fn bound(mut self, bindings: &[Binding], t: f64, signals: &Signals) -> Self {
        let fold = |prop: &str, rest: f64| {
            bindings
                .iter()
                .filter(|b| b.prop.split_whitespace().any(|p| p == prop))
                .fold(rest, |v, b| b.sample(t, signals, v).unwrap_or(v))
        };
        self.x = fold("x", self.x);
        self.y = fold("y", self.y);
        self.z = fold("z", self.z);
        self.rx = fold("rx", self.rx);
        self.ry = fold("ry", self.ry);
        self.rz = fold("rz", self.rz);
        self.scale = fold("scale", self.scale);
        self
    }
}

/// A transform with the layer it belongs to, as the v1 state carries it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerTransform {
    #[serde(flatten)]
    pub layer: LayerRef,
    #[serde(flatten)]
    pub transform: Transform3,
}

/// The signal values at one instant, as the evaluator resolves them before sampling. Missing signals read as rest.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Signals {
    pub time: f64,
    pub cursor: (f64, f64),
    pub cursor_pressed: bool,
    pub cursor_idle: bool,
    pub zoom_scale: f64,
    pub zoom_center: (f64, f64),
    pub audio_level: f64,
    pub word_active: bool,
    /// The one hop: the already-evaluated property of the layer this binding reads, when it reads one.
    pub layer_prop: f64,
}

impl Binding {
    /// Whether the window holds `t` (layer seconds).
    pub fn active(&self, t: f64) -> bool {
        self.window.is_none_or(|(s, e)| t >= s && t < e)
    }

    /// The signal's value as the map sees it: `0..1` for spatial and level signals, seconds for time.
    pub fn read(&self, s: &Signals) -> f64 {
        match &self.signal {
            Signal::Time => s.time - self.window.map_or(0.0, |w| w.0),
            Signal::Cursor | Signal::CursorX => s.cursor.0,
            Signal::CursorY => s.cursor.1,
            Signal::CursorPressed => f64::from(u8::from(s.cursor_pressed)),
            Signal::CursorIdle => f64::from(u8::from(s.cursor_idle)),
            Signal::ZoomScale => s.zoom_scale,
            Signal::ZoomCenter | Signal::ZoomCenterX => s.zoom_center.0,
            Signal::ZoomCenterY => s.zoom_center.1,
            Signal::AudioLevel(_) => s.audio_level,
            Signal::WordActive => f64::from(u8::from(s.word_active)),
            Signal::LayerProp { .. } => s.layer_prop,
        }
    }

    /// The bound value at `t`, or `None` outside the window (the static value stands). `rest` is the static value.
    pub fn sample(&self, t: f64, s: &Signals, rest: f64) -> Option<f64> {
        if !self.active(t) {
            return None;
        }
        let x = self.read(s);
        Some(match &self.map {
            Map::Keys(keys) => keys_at(keys, x, rest),
            Map::Linear {
                from,
                to,
                period,
                looped,
            } => {
                let u = match period {
                    Some(p) if *p > 0.0 => {
                        if *looped {
                            (x / p).rem_euclid(1.0)
                        } else {
                            (x / p).clamp(0.0, 1.0)
                        }
                    }
                    _ => x.clamp(0.0, 1.0),
                };
                from + (to - from) * u
            }
            Map::Wave { from, to, period } => {
                let phase = if *period > 0.0 { x / period } else { 0.0 };
                let mid = (from + to) / 2.0;
                mid + (to - from) / 2.0 * (phase * std::f64::consts::TAU).sin()
            }
            Map::Step { from, to } => {
                if x < 0.5 {
                    *from
                } else {
                    *to
                }
            }
            Map::Clamp { from, to } => x.clamp(from.min(*to), from.max(*to)),
        })
    }
}

/// Piecewise eased interpolation; holds the first key before it and the last after it; no keys means the rest value.
pub fn keys_at(keys: &[Key], t: f64, rest: f64) -> f64 {
    let Some(first) = keys.first() else {
        return rest;
    };
    if t <= first.at {
        return first.value;
    }
    for pair in keys.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if t < b.at {
            let span = (b.at - a.at).max(f64::EPSILON);
            let u = ((t - a.at) / span).clamp(0.0, 1.0);
            let eased = f64::from(b.ease.y(u as f32));
            return a.value + (b.value - a.value) * eased;
        }
    }
    keys.last().map_or(rest, |k| k.value)
}

/// The static checks `check` runs on a layer's bindings: one driver per property, one hop downward only.
/// `below` lists the ids of layers under this one in z-order; `own` is this layer's id.
pub fn validate(bindings: &[Binding], own: &str, below: &[&str]) -> Result<(), BindError> {
    let mut seen: Vec<&str> = Vec::new();
    for b in bindings {
        for prop in b.prop.split_whitespace() {
            if seen.contains(&prop) {
                return Err(BindError::TwoDrivers {
                    prop: prop.to_owned(),
                });
            }
            seen.push(prop);
        }
        if let Signal::LayerProp { layer, .. } = &b.signal {
            if layer == own {
                return Err(BindError::SelfRead);
            }
            if !below.contains(&layer.as_str()) {
                return Err(BindError::NotBelow {
                    layer: layer.clone(),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signals() -> Signals {
        Signals {
            time: 3.0,
            cursor: (0.75, 0.25),
            zoom_scale: 2.0,
            ..Signals::default()
        }
    }

    fn bind(signal: Signal, map: Map) -> Binding {
        Binding {
            prop: "x".into(),
            signal,
            map,
            window: Some((1.0, 5.0)),
        }
    }

    #[test]
    fn the_source_grammar_is_closed_and_names_every_signal() {
        assert_eq!(Signal::parse("cursor.x").unwrap(), Signal::CursorX);
        assert_eq!(
            Signal::parse("audio.level(mic)").unwrap(),
            Signal::AudioLevel("mic".into())
        );
        assert_eq!(
            Signal::parse("layer(k1).x").unwrap(),
            Signal::LayerProp {
                layer: "k1".into(),
                prop: "x".into()
            }
        );
        assert!(matches!(
            Signal::parse("mouse.x"),
            Err(BindError::UnknownSignal(_))
        ));
        assert!(matches!(
            Signal::parse("layer(k1)"),
            Err(BindError::UnknownSignal(_))
        ));
        assert_eq!(Signal::parse("cursor").unwrap(), Signal::Cursor);
        let wire: Binding = serde_json::from_str(
            r#"{"prop":"opacity","signal":"time","map":"wave","from":0.2,"to":1,"period":2}"#,
        )
        .unwrap();
        assert_eq!(
            wire.map,
            Map::Wave {
                from: 0.2,
                to: 1.0,
                period: 2.0
            }
        );
        assert_eq!(serde_json::to_value(&wire).unwrap()["map"], "wave");
        let placed: LayerBinding = serde_json::from_str(
            r#"{"layer":"annotation","id":"a1","prop":"opacity","signal":"cursorX","map":"clamp","from":0,"to":1}"#,
        )
        .unwrap();
        assert_eq!(placed.layer, LayerRef::Annotation { id: "a1".into() });
    }

    #[test]
    fn outside_the_window_the_static_value_stands_and_time_is_window_local() {
        let b = bind(
            Signal::Time,
            Map::Linear {
                from: 0.0,
                to: 10.0,
                period: Some(4.0),
                looped: false,
            },
        );
        assert_eq!(b.sample(0.5, &signals(), 7.0), None);
        assert_eq!(
            b.sample(3.0, &signals(), 7.0),
            Some(5.0),
            "2 s into a 4 s period is halfway"
        );
        let near_end = b
            .sample(
                4.999,
                &Signals {
                    time: 4.999,
                    ..signals()
                },
                7.0,
            )
            .unwrap();
        assert!((near_end - 9.9975).abs() < 1e-9);
        assert_eq!(b.sample(5.0, &signals(), 7.0), None);
    }

    #[test]
    fn linear_wave_step_and_clamp_follow_the_signal() {
        let s = signals();
        let lin = bind(
            Signal::CursorX,
            Map::Linear {
                from: -8.0,
                to: 8.0,
                period: None,
                looped: false,
            },
        );
        assert_eq!(lin.sample(3.0, &s, 0.0), Some(4.0));
        let looped = bind(
            Signal::Time,
            Map::Linear {
                from: 0.0,
                to: 1.0,
                period: Some(1.0),
                looped: true,
            },
        );
        assert!(
            (looped
                .sample(3.0, &Signals { time: 3.25, ..s }, 0.0)
                .unwrap()
                - 0.25)
                .abs()
                < 1e-9
        );
        let wave = bind(
            Signal::Time,
            Map::Wave {
                from: -1.0,
                to: 1.0,
                period: 4.0,
            },
        );
        assert!(
            (wave.sample(2.0, &Signals { time: 2.0, ..s }, 0.0).unwrap() - 1.0).abs() < 1e-9,
            "a quarter period is the crest"
        );
        let step = bind(Signal::CursorPressed, Map::Step { from: 1.0, to: 1.2 });
        assert_eq!(step.sample(3.0, &s, 1.0), Some(1.0));
        assert_eq!(
            step.sample(
                3.0,
                &Signals {
                    cursor_pressed: true,
                    ..s
                },
                1.0
            ),
            Some(1.2)
        );
        let clamp = bind(Signal::ZoomScale, Map::Clamp { from: 1.0, to: 1.5 });
        assert_eq!(clamp.sample(3.0, &s, 0.0), Some(1.5));
    }

    #[test]
    fn keys_hold_at_both_ends_and_ease_between() {
        let keys = vec![
            Key {
                at: 0.0,
                value: 0.0,
                ease: Easing::LINEAR,
            },
            Key {
                at: 2.0,
                value: 10.0,
                ease: Easing::LINEAR,
            },
            Key {
                at: 4.0,
                value: 4.0,
                ease: Easing::LINEAR,
            },
        ];
        assert_eq!(keys_at(&keys, -1.0, 99.0), 0.0);
        assert_eq!(keys_at(&keys, 1.0, 99.0), 5.0);
        assert_eq!(keys_at(&keys, 3.0, 99.0), 7.0);
        assert_eq!(keys_at(&keys, 9.0, 99.0), 4.0);
        assert_eq!(keys_at(&[], 1.0, 99.0), 99.0);
    }

    /// Step 9's second proof: a zoom's eased envelope is a `keys` binding on `scale` over its window, the exit ease reversed.
    #[test]
    fn a_zoom_ramp_is_a_keys_binding_on_scale() {
        let ease_in = Easing {
            x1: 0.42,
            y1: 0.0,
            x2: 0.58,
            y2: 1.0,
        };
        let ease_out = Easing {
            x1: 0.25,
            y1: 0.1,
            x2: 0.25,
            y2: 1.0,
        };
        let region = crate::v1::nodes::ZoomRegion {
            start: 2.0,
            end: 8.0,
            scale: 2.5,
            ease_in,
            ease_out,
            ramp_in: 1.0,
            ramp_out: 1.5,
            ..serde_json::from_str(r#"{"start":0,"end":1,"scale":1}"#).unwrap()
        };
        let keys = vec![
            Key {
                at: 2.0,
                value: 1.0,
                ease: Easing::LINEAR,
            },
            Key {
                at: 3.0,
                value: 2.5,
                ease: ease_in,
            },
            Key {
                at: 6.5,
                value: 2.5,
                ease: Easing::LINEAR,
            },
            Key {
                at: 8.0,
                value: 1.0,
                ease: ease_out.reversed(),
            },
        ];
        for i in 0..=120 {
            let t = 2.0 + f64::from(i) * 0.05;
            let engine = region.scale_at(t);
            let bound = if t >= 8.0 {
                1.0
            } else {
                keys_at(&keys, t, 1.0)
            };
            assert!(
                (engine - bound).abs() < 1e-4,
                "at {t}: engine {engine} vs keys {bound}"
            );
        }
    }

    #[test]
    fn one_driver_per_property_and_one_hop_downward_only() {
        let a = bind(Signal::Time, Map::Step { from: 0.0, to: 1.0 });
        let mut b = a.clone();
        b.prop = "x y".into();
        assert_eq!(
            validate(&[a.clone(), b], "k2", &["k1"]),
            Err(BindError::TwoDrivers { prop: "x".into() })
        );
        let reads_below = Binding {
            prop: "y".into(),
            signal: Signal::LayerProp {
                layer: "k1".into(),
                prop: "x".into(),
            },
            map: Map::Clamp { from: 0.0, to: 1.0 },
            window: None,
        };
        assert_eq!(
            validate(&[a.clone(), reads_below.clone()], "k2", &["k1"]),
            Ok(())
        );
        assert_eq!(
            validate(std::slice::from_ref(&reads_below), "k2", &["k3"]),
            Err(BindError::NotBelow { layer: "k1".into() })
        );
        assert_eq!(
            validate(&[reads_below], "k1", &[]),
            Err(BindError::SelfRead)
        );
    }
}
