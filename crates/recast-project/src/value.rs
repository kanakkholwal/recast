//! Attribute value grammar: how each type is spelled in the file and read back.
//! Times are `seconds.millis` with exactly three decimals so two writers of the same value produce the same bytes.

use std::fmt::Write as _;

/// Seconds to the canonical three-decimal spelling.
#[must_use]
pub fn fmt_secs(v: f64) -> String {
    format!("{:.3}", quantize_ms(v))
}

/// Millisecond grid, which is what three decimals can carry.
#[must_use]
pub fn quantize_ms(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// A general number: up to six decimals, trailing zeros trimmed, never scientific notation.
#[must_use]
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let mut s = format!("{v:.6}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

/// # Errors When the text is not a finite number.
pub fn parse_num(text: &str) -> Result<f64, ValueError> {
    let v: f64 = text
        .trim()
        .parse()
        .map_err(|_| ValueError::NotANumber(text.to_owned()))?;
    if !v.is_finite() {
        return Err(ValueError::NotFinite(text.to_owned()));
    }
    Ok(v)
}

/// # Errors When the text is not an integer.
pub fn parse_int(text: &str) -> Result<i64, ValueError> {
    text.trim()
        .parse()
        .map_err(|_| ValueError::NotAnInteger(text.to_owned()))
}

/// `true`/`false` only; a present attribute with any other spelling is an error, not a truthy value.
/// # Errors On any other spelling.
pub fn parse_bool(text: &str) -> Result<bool, ValueError> {
    match text.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(ValueError::NotABool(other.to_owned())),
    }
}

/// `#rrggbb` or `#rrggbbaa`, lower case on output.
/// # Errors On any other spelling.
pub fn parse_color(text: &str) -> Result<String, ValueError> {
    let t = text.trim();
    let hex = t.strip_prefix('#').unwrap_or("");
    let ok = matches!(hex.len(), 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit());
    if !ok {
        return Err(ValueError::NotAColor(text.to_owned()));
    }
    Ok(t.to_ascii_lowercase())
}

/// A cubic bezier easing, spelled as one of the named curves or four numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ease {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

const NAMED_EASES: &[(&str, Ease)] = &[
    (
        "linear",
        Ease {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        },
    ),
    (
        "ease",
        Ease {
            x1: 0.25,
            y1: 0.1,
            x2: 0.25,
            y2: 1.0,
        },
    ),
    (
        "easeIn",
        Ease {
            x1: 0.42,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        },
    ),
    (
        "easeOut",
        Ease {
            x1: 0.0,
            y1: 0.0,
            x2: 0.58,
            y2: 1.0,
        },
    ),
    (
        "easeInOut",
        Ease {
            x1: 0.42,
            y1: 0.0,
            x2: 0.58,
            y2: 1.0,
        },
    ),
    (
        "easeOutCubic",
        Ease {
            x1: 0.33,
            y1: 1.0,
            x2: 0.68,
            y2: 1.0,
        },
    ),
    (
        "easeInOutCubic",
        Ease {
            x1: 0.65,
            y1: 0.0,
            x2: 0.35,
            y2: 1.0,
        },
    ),
];

/// # Errors When the text is neither a known name nor four finite numbers.
pub fn parse_ease(text: &str) -> Result<Ease, ValueError> {
    let t = text.trim();
    if let Some((_, ease)) = NAMED_EASES.iter().find(|(name, _)| *name == t) {
        return Ok(*ease);
    }
    let parts: Vec<f64> = t
        .split_whitespace()
        .map(parse_num)
        .collect::<Result<_, _>>()
        .map_err(|_| ValueError::NotAnEase(text.to_owned()))?;
    let [x1, y1, x2, y2] = parts[..] else {
        return Err(ValueError::NotAnEase(text.to_owned()));
    };
    Ok(Ease {
        x1: x1 as f32,
        y1: y1 as f32,
        x2: x2 as f32,
        y2: y2 as f32,
    })
}

/// The name when the curve is a named one, otherwise the four numbers.
#[must_use]
pub fn fmt_ease(ease: Ease) -> String {
    let near = |a: f32, b: f32| (a - b).abs() < 1e-4;
    if let Some((name, _)) = NAMED_EASES.iter().find(|(_, e)| {
        near(e.x1, ease.x1) && near(e.y1, ease.y1) && near(e.x2, ease.x2) && near(e.y2, ease.y2)
    }) {
        return (*name).to_owned();
    }
    let mut s = String::new();
    for (i, v) in [ease.x1, ease.y1, ease.x2, ease.y2].into_iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        let _ = write!(s, "{}", fmt_num(f64::from(v)));
    }
    s
}

/// Where a `src` points. The document never holds a URL; the host resolves the form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Src {
    /// Relative to the project directory (`media/recording.mp4`).
    Relative(String),
    /// `ext:<extension-id>/<asset-id>`, an installed extension asset.
    Extension { extension: String, asset: String },
    /// `asset:<id>`, the built-in downloadable catalogue.
    Catalogue(String),
    /// `file:///...`, a linked file outside the project. Non-portable.
    Linked(String),
    /// `gen:<hash>`, a declared generation, reserved.
    Generated(String),
}

/// # Errors On an absolute path that is not `file:///`, a traversal, or an empty reference.
pub fn parse_src(text: &str) -> Result<Src, ValueError> {
    let t = text.trim();
    if t.is_empty() {
        return Err(ValueError::EmptySrc);
    }
    if let Some(rest) = t.strip_prefix("ext:") {
        let (extension, asset) = rest
            .split_once('/')
            .ok_or_else(|| ValueError::BadSrc(text.to_owned()))?;
        return Ok(Src::Extension {
            extension: extension.to_owned(),
            asset: asset.to_owned(),
        });
    }
    if let Some(id) = t.strip_prefix("asset:") {
        return Ok(Src::Catalogue(id.to_owned()));
    }
    if let Some(path) = t.strip_prefix("file:///") {
        return Ok(Src::Linked(path.to_owned()));
    }
    if let Some(hash) = t.strip_prefix("gen:") {
        return Ok(Src::Generated(hash.to_owned()));
    }
    let traverses = t.split(['/', '\\']).any(|part| part == "..");
    let absolute = t.starts_with('/')
        || t.starts_with('\\')
        || t.get(1..3) == Some(":/")
        || t.get(1..3) == Some(":\\");
    if traverses || absolute {
        return Err(ValueError::BadSrc(text.to_owned()));
    }
    Ok(Src::Relative(t.to_owned()))
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValueError {
    #[error("'{0}' is not a number")]
    NotANumber(String),
    #[error("'{0}' is not finite")]
    NotFinite(String),
    #[error("'{0}' is not an integer")]
    NotAnInteger(String),
    #[error("'{0}' is not true or false")]
    NotABool(String),
    #[error("'{0}' is not a #rrggbb or #rrggbbaa colour")]
    NotAColor(String),
    #[error("'{0}' is not a named easing or four numbers")]
    NotAnEase(String),
    #[error("src is empty")]
    EmptySrc,
    #[error("'{0}' is not a valid src: use a path relative to the project, ext:<ext>/<asset>, asset:<id>, or file:///")]
    BadSrc(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_always_carry_three_decimals() {
        assert_eq!(fmt_secs(4.5), "4.500");
        assert_eq!(fmt_secs(12.34567), "12.346");
        assert_eq!(fmt_secs(0.0), "0.000");
    }

    #[test]
    fn numbers_trim_trailing_zeros_and_never_go_scientific() {
        assert_eq!(fmt_num(1.5), "1.5");
        assert_eq!(fmt_num(2.0), "2");
        assert_eq!(fmt_num(0.000001), "0.000001");
        assert_eq!(fmt_num(1e-7), "0");
        assert_eq!(fmt_num(1920.0), "1920");
    }

    #[test]
    fn a_bool_is_only_true_or_false() {
        assert_eq!(parse_bool("true"), Ok(true));
        assert_eq!(parse_bool(" false "), Ok(false));
        assert!(parse_bool("1").is_err());
        assert!(parse_bool("yes").is_err());
    }

    #[test]
    fn colours_are_hex_only_and_lowercased() {
        assert_eq!(parse_color("#FF5C5C"), Ok("#ff5c5c".into()));
        assert_eq!(parse_color("#ff5c5c80"), Ok("#ff5c5c80".into()));
        assert!(parse_color("rgba(1,2,3,0.5)").is_err());
        assert!(parse_color("#fff").is_err());
        assert!(parse_color("transparent").is_err());
    }

    #[test]
    fn easing_round_trips_by_name_and_by_numbers() {
        let named = parse_ease("easeOutCubic").unwrap();
        assert_eq!(fmt_ease(named), "easeOutCubic");
        let custom = parse_ease("0.4 0 0.2 1").unwrap();
        assert_eq!(fmt_ease(custom), "0.4 0 0.2 1");
        assert!(parse_ease("bouncy").is_err());
        assert!(parse_ease("0.4 0 0.2").is_err());
    }

    #[test]
    fn src_forms_are_recognised_and_escapes_refused() {
        assert_eq!(
            parse_src("media/recording.mp4"),
            Ok(Src::Relative("media/recording.mp4".into()))
        );
        assert_eq!(
            parse_src("ext:classic-cursors/macos"),
            Ok(Src::Extension {
                extension: "classic-cursors".into(),
                asset: "macos".into()
            })
        );
        assert_eq!(
            parse_src("asset:wallpaper1"),
            Ok(Src::Catalogue("wallpaper1".into()))
        );
        assert_eq!(
            parse_src("file:///C:/x.png"),
            Ok(Src::Linked("C:/x.png".into()))
        );
        assert!(parse_src("../x.png").is_err());
        assert!(parse_src("C:/x.png").is_err());
        assert!(parse_src("/etc/passwd").is_err());
        assert!(parse_src("").is_err());
        assert!(parse_src("ext:noslash").is_err());
    }
}
