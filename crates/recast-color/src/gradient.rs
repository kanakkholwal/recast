#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::css::{parse_hex, Srgba};

pub const DEFAULT_GRADIENT_ANGLE: f64 = 135.0;
pub const DEFAULT_GRADIENT_STOPS: [Srgba; 2] = [
    Srgba::opaque(0x63, 0x66, 0xf1),
    Srgba::opaque(0xd9, 0x46, 0xef),
];

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GradientStop {
    pub color: Srgba,
    /// Percent along the gradient line, 0..100.
    pub pos: f64,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Gradient {
    /// Degrees, normalised to 0..360, CSS convention (0 = to top).
    pub angle: f64,
    pub stops: Vec<GradientStop>,
}

impl Default for Gradient {
    fn default() -> Self {
        Self {
            angle: DEFAULT_GRADIENT_ANGLE,
            stops: vec![
                GradientStop {
                    color: DEFAULT_GRADIENT_STOPS[0],
                    pos: 0.0,
                },
                GradientStop {
                    color: DEFAULT_GRADIENT_STOPS[1],
                    pos: 100.0,
                },
            ],
        }
    }
}

fn normalize_angle(deg: f64) -> f64 {
    if !deg.is_finite() {
        return DEFAULT_GRADIENT_ANGLE;
    }
    ((deg % 360.0) + 360.0) % 360.0
}

fn scan_angle(value: &str) -> Option<f64> {
    let bytes = value.as_bytes();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        if !value.is_char_boundary(i) || !value[i..].starts_with("deg") {
            i += 1;
            continue;
        }
        let mut start = i;
        while start > 0 && matches!(bytes[start - 1], b'0'..=b'9' | b'.' | b'-') {
            start -= 1;
        }
        if start < i {
            if let Ok(v) = value[start..i].parse::<f64>() {
                return Some(v);
            }
        }
        i += 1;
    }
    None
}

fn hex_run_len(bytes: &[u8], from: usize) -> usize {
    bytes[from..]
        .iter()
        .take_while(|b| b.is_ascii_hexdigit())
        .count()
}

/// Mirrors the editor regex: 8, then 6, then a greedy 3-4. A 5-digit run yields
/// a 4-digit colour, exactly as the JavaScript alternation does.
fn hex_token_len(run: usize) -> Option<usize> {
    match run {
        r if r >= 8 => Some(8),
        r if r >= 6 => Some(6),
        r if r >= 4 => Some(4),
        r if r >= 3 => Some(3),
        _ => None,
    }
}

fn scan_position(rest: &str) -> Option<f64> {
    let trimmed = rest.trim_start();
    if trimmed.len() == rest.len() {
        return None;
    }
    let bytes = trimmed.as_bytes();
    let mut end = 0;
    while end < bytes.len() && matches!(bytes[end], b'0'..=b'9' | b'.' | b'-') {
        end += 1;
    }
    if end == 0 || bytes.get(end) != Some(&b'%') {
        return None;
    }
    trimmed[..end].parse::<f64>().ok()
}

/// Tolerant `linear-gradient(...)` reader: a missing angle defaults to 135
/// degrees, missing stop positions distribute evenly, and the result always has
/// at least two stops so every renderer sees a well-formed spec.
pub fn parse_gradient(value: &str) -> Gradient {
    let angle = scan_angle(value)
        .map(normalize_angle)
        .unwrap_or(DEFAULT_GRADIENT_ANGLE);

    let bytes = value.as_bytes();
    let mut raw: Vec<(Srgba, Option<f64>)> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'#' {
            i += 1;
            continue;
        }
        let Some(len) = hex_token_len(hex_run_len(bytes, i + 1)) else {
            i += 1;
            continue;
        };
        let token = &value[i..i + 1 + len];
        let after = i + 1 + len;
        match parse_hex(token) {
            Some(color) => {
                let pos = scan_position(&value[after..]).map(|p| p.clamp(0.0, 100.0));
                raw.push((color, pos));
            }
            None => {
                i += 1;
                continue;
            }
        }
        i = after;
    }

    match raw.len() {
        0 => Gradient {
            angle,
            ..Default::default()
        },
        1 => Gradient {
            angle,
            stops: vec![
                GradientStop {
                    color: raw[0].0,
                    pos: 0.0,
                },
                GradientStop {
                    color: raw[0].0,
                    pos: 100.0,
                },
            ],
        },
        n => Gradient {
            angle,
            stops: raw
                .iter()
                .enumerate()
                .map(|(i, (color, pos))| GradientStop {
                    color: *color,
                    pos: pos.unwrap_or(i as f64 / (n - 1) as f64 * 100.0),
                })
                .collect(),
        },
    }
}

pub fn serialize_gradient(gradient: &Gradient) -> String {
    let angle = normalize_angle(gradient.angle.round());
    let mut stops = gradient.stops.clone();
    stops.sort_by(|a, b| a.pos.total_cmp(&b.pos));
    let body: Vec<String> = stops
        .iter()
        .map(|s| format!("{} {}%", s.color.to_hex(), s.pos.clamp(0.0, 100.0).round()))
        .collect();
    format!("linear-gradient({}deg, {})", angle, body.join(", "))
}

impl Gradient {
    /// Colour at `t` in 0..1 along the gradient line, interpolated in sRGB the
    /// way CSS does. The compositor converts to linear after sampling.
    pub fn sample(&self, t: f64) -> Srgba {
        let Some(first) = self.stops.first() else {
            return Srgba::default();
        };
        let pos = t.clamp(0.0, 1.0) * 100.0;
        if pos <= first.pos {
            return first.color;
        }
        for pair in self.stops.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if pos <= b.pos {
                let span = b.pos - a.pos;
                let f = if span.abs() < f64::EPSILON {
                    0.0
                } else {
                    (pos - a.pos) / span
                };
                return mix(a.color, b.color, f);
            }
        }
        self.stops.last().map(|s| s.color).unwrap_or_default()
    }
}

fn mix(a: Srgba, b: Srgba, f: f64) -> Srgba {
    let lerp = |x: u8, y: u8| {
        (x as f64 + (y as f64 - x as f64) * f)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Srgba::new(
        lerp(a.r, b.r),
        lerp(a.g, b.g),
        lerp(a.b, b.b),
        lerp(a.a, b.a),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_gradient_parses_angle_and_stops() {
        let g = parse_gradient("linear-gradient(45deg, #ff0000 0%, #0000ff 100%)");
        assert_eq!(g.angle, 45.0);
        assert_eq!(g.stops.len(), 2);
        assert_eq!(g.stops[0].color, Srgba::opaque(255, 0, 0));
        assert_eq!(g.stops[1].pos, 100.0);
    }

    #[test]
    fn a_missing_angle_defaults_to_135_degrees() {
        assert_eq!(
            parse_gradient("linear-gradient(#ff0000, #0000ff)").angle,
            DEFAULT_GRADIENT_ANGLE
        );
    }

    #[test]
    fn a_negative_or_over_turn_angle_normalises() {
        assert_eq!(parse_gradient("linear-gradient(-90deg, #fff)").angle, 270.0);
        assert_eq!(parse_gradient("linear-gradient(450deg, #fff)").angle, 90.0);
    }

    #[test]
    fn missing_positions_distribute_evenly() {
        let g = parse_gradient("linear-gradient(#f00, #0f0, #00f)");
        assert_eq!(g.stops[0].pos, 0.0);
        assert_eq!(g.stops[1].pos, 50.0);
        assert_eq!(g.stops[2].pos, 100.0);
    }

    #[test]
    fn a_single_stop_is_widened_to_a_flat_two_stop_gradient() {
        let g = parse_gradient("linear-gradient(90deg, #123456)");
        assert_eq!(g.stops.len(), 2);
        assert_eq!(g.stops[0].color, g.stops[1].color);
        assert_eq!((g.stops[0].pos, g.stops[1].pos), (0.0, 100.0));
    }

    #[test]
    fn a_string_with_no_colour_falls_back_to_the_default_pair() {
        let g = parse_gradient("linear-gradient(20deg, nonsense)");
        assert_eq!(g.angle, 20.0);
        assert_eq!(g.stops[0].color, DEFAULT_GRADIENT_STOPS[0]);
        assert_eq!(g.stops[1].color, DEFAULT_GRADIENT_STOPS[1]);
    }

    #[test]
    fn eight_digit_stops_keep_their_alpha() {
        let g = parse_gradient("linear-gradient(0deg, #ff000080 0%, #0000ffff 100%)");
        assert_eq!(g.stops[0].color.a, 0x80);
        assert_eq!(g.stops[1].color.a, 0xff);
    }

    #[test]
    fn short_hex_stops_expand() {
        let g = parse_gradient("linear-gradient(#abc 0%, #def 100%)");
        assert_eq!(g.stops[0].color, Srgba::opaque(0xaa, 0xbb, 0xcc));
    }

    #[test]
    fn a_five_digit_run_takes_four_digits_the_way_the_editor_regex_does() {
        let g = parse_gradient("linear-gradient(#abcde, #012345)");
        assert_eq!(g.stops[0].color, Srgba::new(0xaa, 0xbb, 0xcc, 0xdd));
    }

    #[test]
    fn out_of_range_positions_clamp() {
        let g = parse_gradient("linear-gradient(#f00 -30%, #00f 300%)");
        assert_eq!(g.stops[0].pos, 0.0);
        assert_eq!(g.stops[1].pos, 100.0);
    }

    #[test]
    fn a_position_must_be_separated_by_whitespace_to_bind_to_its_stop() {
        let g = parse_gradient("linear-gradient(#f00, #00f)");
        assert_eq!(g.stops[0].pos, 0.0);
        assert_eq!(g.stops[1].pos, 100.0);
    }

    #[test]
    fn serialising_produces_a_string_that_parses_back_unchanged() {
        let original = parse_gradient("linear-gradient(217deg, #0f172a 10%, #6366f1 90%)");
        let round_tripped = parse_gradient(&serialize_gradient(&original));
        assert_eq!(round_tripped, original);
    }

    #[test]
    fn serialising_sorts_the_stops() {
        let g = Gradient {
            angle: 30.0,
            stops: vec![
                GradientStop {
                    color: Srgba::opaque(0, 0, 255),
                    pos: 80.0,
                },
                GradientStop {
                    color: Srgba::opaque(255, 0, 0),
                    pos: 20.0,
                },
            ],
        };
        let text = serialize_gradient(&g);
        assert!(
            text.starts_with("linear-gradient(30deg, #ff0000 20%"),
            "{text}"
        );
    }

    #[test]
    fn sampling_interpolates_between_the_bracketing_stops() {
        let g = parse_gradient("linear-gradient(0deg, #000000 0%, #ffffff 100%)");
        assert_eq!(g.sample(0.0), Srgba::opaque(0, 0, 0));
        assert_eq!(g.sample(1.0), Srgba::opaque(255, 255, 255));
        assert_eq!(g.sample(0.5), Srgba::opaque(128, 128, 128));
    }

    #[test]
    fn sampling_clamps_outside_the_first_and_last_stop() {
        let g = parse_gradient("linear-gradient(0deg, #ff0000 25%, #0000ff 75%)");
        assert_eq!(g.sample(0.0), Srgba::opaque(255, 0, 0));
        assert_eq!(g.sample(1.0), Srgba::opaque(0, 0, 255));
    }
}
