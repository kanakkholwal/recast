use std::path::{Path, PathBuf};

use recast_color::{parse_css_color, parse_gradient, Srgba};
use serde::Deserialize;

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/editor/src/lib/editor/__fixtures__/color-parity.json")
}

#[derive(Deserialize)]
struct ColorCase {
    name: String,
    input: String,
    expected: Option<[u8; 4]>,
}

#[derive(Deserialize)]
struct StopCase {
    color: [u8; 4],
    pos: f64,
}

#[derive(Deserialize)]
struct GradientCase {
    name: String,
    input: String,
    angle: f64,
    stops: Vec<StopCase>,
}

#[derive(Deserialize)]
struct Fixture {
    colors: Vec<ColorCase>,
    gradients: Vec<GradientCase>,
}

fn load() -> Fixture {
    let path = fixture_path();
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn as_bytes(c: Srgba) -> [u8; 4] {
    [c.r, c.g, c.b, c.a]
}

#[test]
fn colour_parsing_matches_the_shared_fixtures() {
    let fixture = load();
    assert!(!fixture.colors.is_empty());

    for case in fixture.colors {
        let parsed = parse_css_color(&case.input).map(as_bytes);
        assert_eq!(
            parsed, case.expected,
            "{}: {:?} parsed to {:?}",
            case.name, case.input, parsed
        );
    }
}

#[test]
fn gradient_parsing_matches_the_shared_fixtures() {
    let fixture = load();
    assert!(!fixture.gradients.is_empty());

    for case in fixture.gradients {
        let parsed = parse_gradient(&case.input);
        assert!(
            (parsed.angle - case.angle).abs() < 1e-6,
            "{}: angle {} != {}",
            case.name,
            parsed.angle,
            case.angle
        );
        assert_eq!(
            parsed.stops.len(),
            case.stops.len(),
            "{}: stop count",
            case.name
        );
        for (i, (got, want)) in parsed.stops.iter().zip(&case.stops).enumerate() {
            assert_eq!(
                as_bytes(got.color),
                want.color,
                "{}: stop {i} colour",
                case.name
            );
            assert!(
                (got.pos - want.pos).abs() < 1e-6,
                "{}: stop {i} position {} != {}",
                case.name,
                got.pos,
                want.pos
            );
        }
    }
}
