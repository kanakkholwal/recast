use std::path::{Path, PathBuf};

use recast_time::{
    build_time_map, clamp_speed, derive_segments, normalize_cuts, original_to_output,
    original_to_output_cuts, time_map_from_segments, ClipShape, Cut, SegmentSpeed, TimeSpan,
};
use serde::Deserialize;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/editor/src/lib/timeline/__fixtures__")
}

fn load<T: for<'de> Deserialize<'de>>(name: &str) -> T {
    let path = fixtures_dir().join(name);
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CutCase {
    name: String,
    trim_start: f64,
    trim_end: f64,
    cuts: Vec<[f64; 2]>,
    expected_kept_duration: f64,
}

#[derive(Deserialize)]
struct CutFixture {
    cases: Vec<CutCase>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpeedCase {
    name: String,
    trim_end: f64,
    duration: f64,
    cuts: Vec<[f64; 2]>,
    split_points: Vec<f64>,
    segment_speeds: Vec<[f64; 2]>,
    expected_output_duration: f64,
}

#[derive(Deserialize)]
struct SpeedFixture {
    cases: Vec<SpeedCase>,
}

fn to_cuts(pairs: &[[f64; 2]]) -> Vec<Cut> {
    pairs.iter().map(|p| Cut::new(p[0], p[1])).collect()
}

#[test]
fn speed_one_reduces_exactly_to_the_cut_translation_map() {
    let fixture: CutFixture = load("cut-parity.json");
    assert!(!fixture.cases.is_empty());

    for case in fixture.cases {
        let cuts = to_cuts(&case.cuts);
        let segments = derive_segments(&ClipShape {
            trim_start: case.trim_start,
            trim_end: case.trim_end,
            cuts: cuts.clone(),
            split_points: Vec::new(),
        });
        let map = time_map_from_segments(&segments, |_| 1.0);

        assert!(
            (map.output_duration - case.expected_kept_duration).abs() < 1e-6,
            "{}: kept duration {} != {}",
            case.name,
            map.output_duration,
            case.expected_kept_duration
        );

        let offset = original_to_output_cuts(&cuts, case.trim_start);
        for seg in &segments {
            for t in [seg.start, (seg.start + seg.end) / 2.0, seg.end] {
                let general = original_to_output(&map, t);
                let cut_only = original_to_output_cuts(&cuts, t) - offset;
                assert!(
                    (general - cut_only).abs() < 1e-6,
                    "{}: at t={t} the general map says {general} and the cut map says {cut_only}",
                    case.name
                );
            }
        }
    }
}

#[test]
fn the_display_axis_reduces_to_the_full_duration_cut_map_at_one_times() {
    const DURATION: f64 = 12.0;
    let fixture: CutFixture = load("cut-parity.json");

    for case in fixture.cases.iter().filter(|c| c.trim_end <= DURATION) {
        let cuts = to_cuts(&case.cuts);
        let segments = derive_segments(&ClipShape {
            trim_start: case.trim_start,
            trim_end: case.trim_end,
            cuts: cuts.clone(),
            split_points: Vec::new(),
        });
        let map = recast_time::display_time_map(
            &recast_time::DisplayAxis {
                trim_start: case.trim_start,
                trim_end: case.trim_end,
                duration_secs: DURATION,
                segments,
                cuts: cuts.clone(),
            },
            |_| 1.0,
        );

        assert!(
            (map.output_duration - original_to_output_cuts(&cuts, DURATION)).abs() < 1e-6,
            "{}: display duration mismatch",
            case.name
        );

        let mut t = 0.0;
        while t <= DURATION {
            assert!(
                (original_to_output(&map, t) - original_to_output_cuts(&cuts, t)).abs() < 1e-6,
                "{}: display axis diverged at t={t}",
                case.name
            );
            t += 0.37;
        }
    }
}

#[test]
fn warped_output_duration_matches_the_shared_speed_fixtures() {
    let fixture: SpeedFixture = load("speed-parity.json");
    assert!(!fixture.cases.is_empty());

    for case in fixture.cases {
        let overrides: Vec<SegmentSpeed> = case
            .segment_speeds
            .iter()
            .map(|p| SegmentSpeed {
                start: p[0],
                speed: p[1],
            })
            .collect();
        let segments = derive_segments(&ClipShape {
            trim_start: 0.0,
            trim_end: case.trim_end,
            cuts: to_cuts(&case.cuts),
            split_points: case.split_points.clone(),
        });
        let map = time_map_from_segments(&segments, |index| {
            let start = segments
                .iter()
                .find(|s| s.index == index)
                .map(|s| s.start)
                .unwrap_or(0.0);
            clamp_speed(recast_time::segment_speed_at(&overrides, start))
        });

        assert!(
            (map.output_duration - case.expected_output_duration).abs() < 1e-6,
            "{}: output duration {} != {}",
            case.name,
            map.output_duration,
            case.expected_output_duration
        );
        assert!(case.duration >= case.trim_end);
    }
}

#[test]
fn the_wire_span_shape_matches_the_editor_payload() {
    let spans: Vec<TimeSpan> = serde_json::from_str(
        r#"[{"origStart":0,"origEnd":4,"speed":2},{"origStart":6,"origEnd":10}]"#,
    )
    .expect("parse wire spans");
    assert_eq!(spans[0].speed, 2.0);
    assert_eq!(spans[1].speed, 1.0);

    let map = build_time_map(&spans);
    assert!((map.output_duration - 6.0).abs() < 1e-9);

    let round_tripped = serde_json::to_string(&spans[0]).expect("serialize");
    assert!(round_tripped.contains("origStart"));
}

#[test]
fn overlapping_cuts_are_merged_before_any_duration_is_derived() {
    let cuts = to_cuts(&[[11.0, 13.0], [12.0, 15.0]]);
    assert_eq!(normalize_cuts(&cuts).len(), 1);
    let segments = derive_segments(&ClipShape {
        trim_start: 10.0,
        trim_end: 20.0,
        cuts,
        split_points: Vec::new(),
    });
    let map = time_map_from_segments(&segments, |_| 1.0);
    assert!((map.output_duration - 6.0).abs() < 1e-9);
}
