use recast_captions::{
    break_into_lines, chunk_words, karaoke_centiseconds, spoken_word_count, CaptionAnimation,
    TranscriptWord,
};
use serde::Deserialize;

/// The fixture @recast/captions asserts against, so the shared crate cannot
/// drift from the TypeScript the DOM overlay still uses.
const FIXTURE: &str =
    include_str!("../../../packages/captions/src/__fixtures__/caption-parity.json");

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    name: String,
    words: Vec<TranscriptWord>,
    animation: serde_json::Value,
    max_chars_per_line: u32,
    max_lines: u32,
    expected: Expected,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Expected {
    chunks: Vec<Vec<usize>>,
    lines: Vec<Vec<usize>>,
    karaoke_cs: Vec<Vec<i64>>,
    spoken_at: Vec<SpokenAt>,
}

#[derive(Deserialize)]
struct SpokenAt {
    t: f64,
    count: usize,
}

/// The fixture carries partial animations, the way `resolveCaptionAnimation`
/// merges them over the default.
fn animation(overrides: &serde_json::Value) -> CaptionAnimation {
    let mut merged = serde_json::to_value(CaptionAnimation::default()).expect("default animation");
    if let (Some(base), Some(over)) = (merged.as_object_mut(), overrides.as_object()) {
        for (key, value) in over {
            base.insert(key.clone(), value.clone());
        }
    }
    serde_json::from_value(merged).expect("merged animation")
}

fn cases() -> Vec<Case> {
    serde_json::from_str::<Fixture>(FIXTURE)
        .expect("parity fixture")
        .cases
}

#[test]
fn chunking_matches_the_shared_fixture() {
    for case in cases() {
        let anim = animation(&case.animation);
        let runs = chunk_words(&case.words, &anim);
        let mut offset = 0;
        let indices: Vec<Vec<usize>> = runs
            .iter()
            .map(|run| {
                let group = (offset..offset + run.len()).collect();
                offset += run.len();
                group
            })
            .collect();
        assert_eq!(indices, case.expected.chunks, "{}", case.name);
    }
}

#[test]
fn line_breaking_matches_the_shared_fixture() {
    for case in cases() {
        let lines = break_into_lines(&case.words, case.max_chars_per_line, case.max_lines);
        assert_eq!(lines, case.expected.lines, "{}", case.name);
    }
}

#[test]
fn karaoke_timings_match_the_shared_fixture() {
    for case in cases() {
        let anim = animation(&case.animation);
        let runs = chunk_words(&case.words, &anim);
        let cs: Vec<Vec<i64>> = runs
            .iter()
            .map(|run| karaoke_centiseconds(run, run[0].start))
            .collect();
        assert_eq!(cs, case.expected.karaoke_cs, "{}", case.name);
    }
}

#[test]
fn spoken_counts_match_the_shared_fixture() {
    for case in cases() {
        for sample in &case.expected.spoken_at {
            assert_eq!(
                spoken_word_count(&case.words, sample.t),
                sample.count,
                "{} at t={}",
                case.name,
                sample.t
            );
        }
    }
}
