#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::transfer::{linear_to_srgb, srgb_to_linear};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Srgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LinearRgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

pub const TRANSPARENT: Srgba = Srgba {
    r: 0,
    g: 0,
    b: 0,
    a: 0,
};

impl Srgba {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub fn alpha_f32(self) -> f32 {
        self.a as f32 / 255.0
    }

    pub fn to_linear(self) -> LinearRgba {
        LinearRgba {
            r: srgb_to_linear(self.r as f32 / 255.0),
            g: srgb_to_linear(self.g as f32 / 255.0),
            b: srgb_to_linear(self.b as f32 / 255.0),
            a: self.alpha_f32(),
        }
    }

    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }
}

impl LinearRgba {
    pub fn to_srgba(self) -> Srgba {
        Srgba {
            r: encode_channel(self.r),
            g: encode_channel(self.g),
            b: encode_channel(self.b),
            a: (self.a * 255.0).round().clamp(0.0, 255.0) as u8,
        }
    }

    pub fn premultiplied(self) -> Self {
        Self {
            r: self.r * self.a,
            g: self.g * self.a,
            b: self.b * self.a,
            a: self.a,
        }
    }
}

fn encode_channel(linear: f32) -> u8 {
    (linear_to_srgb(linear) * 255.0).round().clamp(0.0, 255.0) as u8
}

fn hex_value(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn hex_pair(bytes: &[u8], index: usize) -> Option<u8> {
    Some(hex_value(*bytes.get(index)?)? * 16 + hex_value(*bytes.get(index + 1)?)?)
}

fn expand_nibble(bytes: &[u8], index: usize) -> Option<u8> {
    let v = hex_value(*bytes.get(index)?)?;
    Some(v * 16 + v)
}

/// Accepts `#rgb`, `#rgba`, `#rrggbb` and `#rrggbbaa`, with or without the hash.
/// Length is the discriminator, so an over-long string is rejected rather than
/// silently truncated the way the two previous export-side parsers did.
pub fn parse_hex(value: &str) -> Option<Srgba> {
    let trimmed = value.trim().trim_start_matches('#');
    let bytes = trimmed.as_bytes();
    if !bytes.iter().all(|b| hex_value(*b).is_some()) {
        return None;
    }
    match bytes.len() {
        3 => Some(Srgba::opaque(
            expand_nibble(bytes, 0)?,
            expand_nibble(bytes, 1)?,
            expand_nibble(bytes, 2)?,
        )),
        4 => Some(Srgba::new(
            expand_nibble(bytes, 0)?,
            expand_nibble(bytes, 1)?,
            expand_nibble(bytes, 2)?,
            expand_nibble(bytes, 3)?,
        )),
        6 => Some(Srgba::opaque(
            hex_pair(bytes, 0)?,
            hex_pair(bytes, 2)?,
            hex_pair(bytes, 4)?,
        )),
        8 => Some(Srgba::new(
            hex_pair(bytes, 0)?,
            hex_pair(bytes, 2)?,
            hex_pair(bytes, 4)?,
            hex_pair(bytes, 6)?,
        )),
        _ => None,
    }
}

fn channel_component(part: &str) -> Option<u8> {
    let part = part.trim();
    if let Some(pct) = part.strip_suffix('%') {
        let v = pct.trim().parse::<f64>().ok()?;
        return Some((v / 100.0 * 255.0).round().clamp(0.0, 255.0) as u8);
    }
    Some(part.parse::<f64>().ok()?.round().clamp(0.0, 255.0) as u8)
}

fn alpha_component(part: &str) -> Option<u8> {
    let part = part.trim();
    let v = if let Some(pct) = part.strip_suffix('%') {
        pct.trim().parse::<f64>().ok()? / 100.0
    } else {
        part.parse::<f64>().ok()?
    };
    Some((v * 255.0).round().clamp(0.0, 255.0) as u8)
}

/// Hex, `rgb()`/`rgba()` in both comma and space syntax, and the `transparent`
/// keyword. Anything else returns `None` so the caller can fall back explicitly.
pub fn parse_css_color(value: &str) -> Option<Srgba> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value.eq_ignore_ascii_case("transparent") {
        return Some(TRANSPARENT);
    }
    if value.starts_with('#') {
        return parse_hex(value);
    }

    let lower = value.to_ascii_lowercase();
    let body = lower
        .strip_prefix("rgba(")
        .or_else(|| lower.strip_prefix("rgb("))?
        .strip_suffix(')')?;

    let (channels, alpha) = match body.split_once('/') {
        Some((c, a)) => (c, Some(a)),
        None => (body, None),
    };
    let mut parts: Vec<&str> = if channels.contains(',') {
        channels.split(',').map(str::trim).collect()
    } else {
        channels.split_whitespace().collect()
    };
    let alpha = match alpha {
        Some(a) => Some(a),
        None if parts.len() == 4 => parts.pop(),
        None => None,
    };
    if parts.len() != 3 {
        return None;
    }

    Some(Srgba::new(
        channel_component(parts[0])?,
        channel_component(parts[1])?,
        channel_component(parts[2])?,
        match alpha {
            Some(a) => alpha_component(a)?,
            None => 255,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_digit_hex_parses_opaque() {
        assert_eq!(parse_hex("#1e293b"), Some(Srgba::opaque(0x1e, 0x29, 0x3b)));
    }

    #[test]
    fn a_missing_hash_is_accepted() {
        assert_eq!(parse_hex("1e293b"), parse_hex("#1e293b"));
    }

    #[test]
    fn three_digit_hex_expands_each_nibble() {
        assert_eq!(parse_hex("#abc"), Some(Srgba::opaque(0xaa, 0xbb, 0xcc)));
    }

    #[test]
    fn four_digit_hex_carries_alpha() {
        assert_eq!(parse_hex("#abcd"), Some(Srgba::new(0xaa, 0xbb, 0xcc, 0xdd)));
    }

    #[test]
    fn eight_digit_hex_carries_alpha() {
        assert_eq!(
            parse_hex("#0f172a80"),
            Some(Srgba::new(0x0f, 0x17, 0x2a, 0x80))
        );
    }

    #[test]
    fn an_over_long_hex_is_rejected_not_truncated() {
        assert_eq!(parse_hex("#1e293bff00"), None);
        assert_eq!(parse_hex("#12345"), None);
    }

    #[test]
    fn a_non_hex_digit_is_rejected() {
        assert_eq!(parse_hex("#gg0011"), None);
    }

    #[test]
    fn uppercase_hex_parses() {
        assert_eq!(parse_hex("#1E293B"), parse_hex("#1e293b"));
    }

    #[test]
    fn transparent_is_a_colour_not_a_parse_failure() {
        assert_eq!(parse_css_color("transparent"), Some(TRANSPARENT));
        assert_eq!(parse_css_color("TRANSPARENT"), Some(TRANSPARENT));
    }

    #[test]
    fn comma_syntax_rgb_and_rgba_parse() {
        assert_eq!(
            parse_css_color("rgb(255, 128, 0)"),
            Some(Srgba::opaque(255, 128, 0))
        );
        assert_eq!(
            parse_css_color("rgba(255, 128, 0, 0.5)"),
            Some(Srgba::new(255, 128, 0, 128))
        );
    }

    #[test]
    fn space_and_slash_syntax_parses() {
        assert_eq!(
            parse_css_color("rgb(255 128 0 / 50%)"),
            Some(Srgba::new(255, 128, 0, 128))
        );
        assert_eq!(
            parse_css_color("rgb(255 128 0)"),
            Some(Srgba::opaque(255, 128, 0))
        );
    }

    #[test]
    fn percentage_channels_parse() {
        assert_eq!(
            parse_css_color("rgb(100%, 0%, 50%)"),
            Some(Srgba::opaque(255, 0, 128))
        );
    }

    #[test]
    fn out_of_range_channels_clamp() {
        assert_eq!(
            parse_css_color("rgb(300, -20, 0)"),
            Some(Srgba::opaque(255, 0, 0))
        );
    }

    #[test]
    fn an_unsupported_function_returns_none() {
        assert_eq!(parse_css_color("hsl(200 50% 50%)"), None);
        assert_eq!(parse_css_color(""), None);
    }

    #[test]
    fn hex_serialises_back_to_the_shortest_faithful_form() {
        assert_eq!(Srgba::opaque(0x1e, 0x29, 0x3b).to_hex(), "#1e293b");
        assert_eq!(Srgba::new(0x1e, 0x29, 0x3b, 0x80).to_hex(), "#1e293b80");
    }

    #[test]
    fn linear_round_trips_through_srgb() {
        for c in [0u8, 1, 17, 128, 200, 255] {
            let original = Srgba::opaque(c, c, c);
            assert_eq!(original.to_linear().to_srgba(), original);
        }
    }

    #[test]
    fn mid_grey_is_not_half_in_linear_light() {
        let linear = Srgba::opaque(128, 128, 128).to_linear();
        assert!(linear.r < 0.25, "{}", linear.r);
    }

    #[test]
    fn premultiplying_scales_colour_but_not_alpha() {
        let p = Srgba::new(255, 0, 0, 128).to_linear().premultiplied();
        assert!((p.r - 0.501_960_8).abs() < 1e-3);
        assert!((p.a - 0.501_960_8).abs() < 1e-3);
    }
}
