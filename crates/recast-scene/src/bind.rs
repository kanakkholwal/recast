//! The relational mapping: a property driven by a signal through a closed map. Typed data, no code, one hop.
//! Pure and allocation-free per sample, so the evaluator can run it per frame in wasm and native alike.

use crate::v1::easing::Easing;

/// What a binding reads. Closed: an unknown source is a validation error, never a runtime lookup.
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    /// Seconds since the binding's window opened.
    Time,
    CursorX,
    CursorY,
    CursorPressed,
    CursorIdle,
    ZoomScale,
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
            "cursor.x" => Self::CursorX,
            "cursor.y" => Self::CursorY,
            "cursor.pressed" => Self::CursorPressed,
            "cursor.idle" => Self::CursorIdle,
            "zoom.scale" => Self::ZoomScale,
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
#[derive(Debug, Clone, PartialEq)]
pub struct Key {
    pub at: f64,
    pub value: f64,
    /// Easing INTO this key from the previous one.
    pub ease: Easing,
}

/// The closed map set. Composite behaviours are ONE named map, never a composition rule.
#[derive(Debug, Clone, PartialEq)]
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
    /// Exponential lag toward the signal with time constant `tau`; the state is the caller's, see `smooth_step`.
    Smooth {
        tau: f64,
    },
    /// `from` until the signal crosses `0.5`, then `to`.
    Step {
        from: f64,
        to: f64,
    },
    /// The camera-dodge rule: push away from the signal by `strength` when it comes within `radius` of the property's rest value.
    Dodge {
        strength: f64,
        radius: f64,
    },
    Clamp {
        from: f64,
        to: f64,
    },
    /// Follow the signal at `strength`, resting at the static value when the signal is at rest (0.5).
    Follow {
        strength: f64,
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
#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    pub prop: String,
    pub signal: Signal,
    pub map: Map,
    /// Window on the layer's clock, seconds; `None` is the whole layer.
    pub window: Option<(f64, f64)>,
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
            Signal::CursorX => s.cursor.0,
            Signal::CursorY => s.cursor.1,
            Signal::CursorPressed => f64::from(u8::from(s.cursor_pressed)),
            Signal::CursorIdle => f64::from(u8::from(s.cursor_idle)),
            Signal::ZoomScale => s.zoom_scale,
            Signal::ZoomCenterX => s.zoom_center.0,
            Signal::ZoomCenterY => s.zoom_center.1,
            Signal::AudioLevel(_) => s.audio_level,
            Signal::WordActive => f64::from(u8::from(s.word_active)),
            Signal::LayerProp { .. } => s.layer_prop,
        }
    }

    /// The bound value at `t`, or `None` outside the window (the static value stands). `rest` is the static value;
    /// `previous` is the last bound value, which only `smooth` needs.
    pub fn sample(
        &self,
        t: f64,
        s: &Signals,
        rest: f64,
        previous: Option<f64>,
        dt: f64,
    ) -> Option<f64> {
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
            Map::Smooth { tau } => smooth_step(previous.unwrap_or(rest), x, *tau, dt),
            Map::Step { from, to } => {
                if x < 0.5 {
                    *from
                } else {
                    *to
                }
            }
            Map::Dodge { strength, radius } => {
                let d = x - rest;
                if d.abs() >= *radius || *radius <= 0.0 {
                    rest
                } else {
                    let push = (1.0 - d.abs() / radius) * strength;
                    if d >= 0.0 {
                        rest - push
                    } else {
                        rest + push
                    }
                }
            }
            Map::Clamp { from, to } => x.clamp(from.min(*to), from.max(*to)),
            Map::Follow { strength } => rest + (x - 0.5) * strength,
        })
    }
}

/// Exponential lag: the fraction of the gap closed in `dt` seconds with time constant `tau`.
pub fn smooth_step(previous: f64, target: f64, tau: f64, dt: f64) -> f64 {
    if tau <= 0.0 || dt <= 0.0 {
        return target;
    }
    let k = 1.0 - (-dt / tau).exp();
    previous + (target - previous) * k
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
        assert_eq!(b.sample(0.5, &signals(), 7.0, None, 0.016), None);
        assert_eq!(
            b.sample(3.0, &signals(), 7.0, None, 0.016),
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
                None,
                0.016,
            )
            .unwrap();
        assert!((near_end - 9.9975).abs() < 1e-9);
        assert_eq!(b.sample(5.0, &signals(), 7.0, None, 0.016), None);
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
        assert_eq!(lin.sample(3.0, &s, 0.0, None, 0.0), Some(4.0));
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
                .sample(3.0, &Signals { time: 3.25, ..s }, 0.0, None, 0.0)
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
            (wave
                .sample(2.0, &Signals { time: 2.0, ..s }, 0.0, None, 0.0)
                .unwrap()
                - 1.0)
                .abs()
                < 1e-9,
            "a quarter period is the crest"
        );
        let step = bind(Signal::CursorPressed, Map::Step { from: 1.0, to: 1.2 });
        assert_eq!(step.sample(3.0, &s, 1.0, None, 0.0), Some(1.0));
        assert_eq!(
            step.sample(
                3.0,
                &Signals {
                    cursor_pressed: true,
                    ..s
                },
                1.0,
                None,
                0.0
            ),
            Some(1.2)
        );
        let clamp = bind(Signal::ZoomScale, Map::Clamp { from: 1.0, to: 1.5 });
        assert_eq!(clamp.sample(3.0, &s, 0.0, None, 0.0), Some(1.5));
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

    #[test]
    fn smooth_closes_a_fixed_fraction_of_the_gap_per_time_constant() {
        let after_one_tau = smooth_step(0.0, 1.0, 0.5, 0.5);
        assert!((after_one_tau - (1.0 - (-1.0f64).exp())).abs() < 1e-12);
        assert_eq!(
            smooth_step(0.0, 1.0, 0.0, 0.5),
            1.0,
            "no lag without a time constant"
        );
        let b = bind(Signal::CursorX, Map::Smooth { tau: 0.2 });
        let first = b.sample(3.0, &signals(), 0.5, None, 0.1).unwrap();
        assert!(
            first > 0.5 && first < 0.75,
            "moves toward the cursor without arriving: {first}"
        );
    }

    #[test]
    fn dodge_pushes_away_inside_the_radius_and_follow_rides_the_signal() {
        let dodge = bind(
            Signal::CursorX,
            Map::Dodge {
                strength: 0.2,
                radius: 0.3,
            },
        );
        let pushed = dodge.sample(3.0, &signals(), 0.9, None, 0.0).unwrap();
        assert!(
            (pushed - 1.0).abs() < 1e-9,
            "cursor at 0.75 is 0.15 of 0.3 away: pushed up by half the strength: {pushed}"
        );
        let above = dodge
            .sample(
                3.0,
                &Signals {
                    cursor: (0.95, 0.0),
                    ..signals()
                },
                0.9,
                None,
                0.0,
            )
            .unwrap();
        assert!(above < 0.9, "cursor above the rest pushes it down: {above}");
        assert_eq!(
            dodge.sample(3.0, &signals(), 0.2, None, 0.0),
            Some(0.2),
            "far away: at rest"
        );
        let follow = bind(Signal::CursorY, Map::Follow { strength: 0.4 });
        assert_eq!(follow.sample(3.0, &signals(), 0.5, None, 0.0), Some(0.4));
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
