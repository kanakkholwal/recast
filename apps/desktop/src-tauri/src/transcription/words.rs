//! Word-timestamp post-processing for captions.
//!
//! transcribe.cpp hands us a `Transcript` that may carry segments, per-word
//! timing, both, or (for a text-only model) neither. Animated captions need
//! clean, per-word timing in every case, so this module:
//!   - normalizes word times (monotonic, non-overlapping, a minimum on-screen
//!     duration so single-frame flicker doesn't read as a glitch),
//!   - groups a flat word stream into display-line segments,
//!   - synthesizes approximate word times for segments that arrived without any,
//!     and
//!   - maps the ggml result (`build_segments`) through the fallbacks above.
//!
//! Pure functions over the transcript types — compiled and unit-tested
//! regardless of the on-device engine feature.

use super::{TranscriptSegment, TranscriptWord};

/// Shortest time a word may stay on screen (seconds). Below this, word-by-word
/// styles flicker.
const MIN_WORD_DUR: f64 = 0.06;
/// Word count that forces a new caption line.
/// Word-stream grouping (below) is only reached through the ggml engine and the
/// tests; in a `--no-default-features` build it's dead but must still compile
/// (`words` is always built), so silence dead-code there rather than gate it out.
#[cfg_attr(not(feature = "ggml"), allow(dead_code))]
const MAX_WORDS_PER_LINE: usize = 7;
/// A silent gap (seconds) between words that forces a new caption line.
#[cfg_attr(not(feature = "ggml"), allow(dead_code))]
const LINE_GAP: f64 = 0.6;

/// A token that is only punctuation — glued onto the prior word, not shown alone.
#[cfg_attr(not(feature = "ggml"), allow(dead_code))]
fn is_punctuation(text: &str) -> bool {
    let t = text.trim();
    !t.is_empty()
        && t.chars()
            .all(|c| matches!(c, '.' | ',' | '!' | '?' | ';' | ':' | '…' | '—'))
}

/// True when the token ends a sentence (forces a line break after it).
#[cfg_attr(not(feature = "ggml"), allow(dead_code))]
fn ends_sentence(text: &str) -> bool {
    matches!(
        text.trim_end().chars().last(),
        Some('.') | Some('!') | Some('?') | Some('…')
    )
}

/// Normalize a word list in place: clamp to non-negative, force monotonic
/// non-decreasing starts, remove overlaps (a word never outlives the next word's
/// start), and give each word a minimum on-screen duration where there's room.
/// Idempotent.
pub(crate) fn clean_word_times(words: &mut [TranscriptWord]) {
    let n = words.len();
    for i in 0..n {
        if words[i].start < 0.0 {
            words[i].start = 0.0;
        }
        if i > 0 && words[i].start < words[i - 1].start {
            words[i].start = words[i - 1].start;
        }
        if words[i].end < words[i].start {
            words[i].end = words[i].start;
        }
    }
    for i in 0..n {
        let next_start = (i + 1 < n).then(|| words[i + 1].start);
        if let Some(ns) = next_start {
            if words[i].end > ns {
                words[i].end = ns;
            }
        }
        // Extend to the minimum duration, but never past the next word's start.
        let want = words[i].start + MIN_WORD_DUR;
        let cap = next_start.unwrap_or(f64::INFINITY);
        if words[i].end < want {
            words[i].end = want.min(cap).max(words[i].start);
        }
    }
}

/// Glue pure-punctuation tokens onto the preceding word (Parakeet can emit a
/// trailing `.`/`,` as its own token).
#[cfg_attr(not(feature = "ggml"), allow(dead_code))]
fn glue_punctuation(words: Vec<TranscriptWord>) -> Vec<TranscriptWord> {
    let mut out: Vec<TranscriptWord> = Vec::with_capacity(words.len());
    for w in words {
        if is_punctuation(&w.text) {
            if let Some(prev) = out.last_mut() {
                prev.text.push_str(w.text.trim());
                prev.end = prev.end.max(w.end);
                continue;
            }
        }
        out.push(w);
    }
    out
}

#[cfg_attr(not(feature = "ggml"), allow(dead_code))]
fn push_segment(
    segments: &mut Vec<TranscriptSegment>,
    cur: &mut Vec<TranscriptWord>,
    idx: &mut usize,
) {
    if cur.is_empty() {
        return;
    }
    let start = cur.first().unwrap().start;
    let end = cur.last().unwrap().end;
    let text = cur
        .iter()
        .map(|w| w.text.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    segments.push(TranscriptSegment {
        id: format!("seg-{}", *idx),
        start,
        end,
        text,
        words: std::mem::take(cur),
    });
    *idx += 1;
}

/// Group a flat, time-ordered word stream into display-line segments, breaking
/// on sentence punctuation, a long pause, or a max word count. Times are cleaned
/// first, so the returned segments carry normalized per-word timing.
#[cfg_attr(not(feature = "ggml"), allow(dead_code))]
pub(crate) fn group_words_into_segments(words: Vec<TranscriptWord>) -> Vec<TranscriptSegment> {
    let mut words = glue_punctuation(words);
    clean_word_times(&mut words);

    let mut segments: Vec<TranscriptSegment> = Vec::new();
    let mut cur: Vec<TranscriptWord> = Vec::new();
    let mut idx = 0usize;

    for i in 0..words.len() {
        let gap_before = if i > 0 {
            words[i].start - words[i - 1].end
        } else {
            0.0
        };
        if !cur.is_empty() && (gap_before > LINE_GAP || cur.len() >= MAX_WORDS_PER_LINE) {
            push_segment(&mut segments, &mut cur, &mut idx);
        }
        let breaks = ends_sentence(&words[i].text);
        cur.push(words[i].clone());
        if breaks {
            push_segment(&mut segments, &mut cur, &mut idx);
        }
    }
    push_segment(&mut segments, &mut cur, &mut idx);
    segments
}

/// Approximate per-word timing for a segment that arrived with none, splitting
/// its span across whitespace tokens weighted by character length. Lower
/// accuracy than real word timestamps, but lets animation work on any engine.
pub(crate) fn synthesize_words(seg: &TranscriptSegment) -> Vec<TranscriptWord> {
    let tokens: Vec<&str> = seg.text.split_whitespace().collect();
    if tokens.is_empty() {
        return Vec::new();
    }
    let total: usize = tokens.iter().map(|t| t.chars().count().max(1)).sum();
    let span = (seg.end - seg.start).max(0.0);
    let mut words = Vec::with_capacity(tokens.len());
    let mut t = seg.start;
    for tok in tokens {
        let frac = tok.chars().count().max(1) as f64 / total as f64;
        let start = t;
        let end = (t + span * frac).min(seg.end);
        words.push(TranscriptWord {
            start,
            end,
            text: tok.to_string(),
        });
        t = end;
    }
    clean_word_times(&mut words);
    words
}

/// One caption block spanning `[0, end_secs]`, used when an engine returns text
/// but no timing. Synthesizes per-word timing so animation still has something
/// to drive. Empty text yields no segment.
pub(crate) fn whole_clip_segment(text: &str, end_secs: f64) -> Vec<TranscriptSegment> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Vec::new();
    }
    let mut seg = TranscriptSegment {
        id: "seg-0".into(),
        start: 0.0,
        end: end_secs.max(0.0),
        text,
        words: Vec::new(),
    };
    seg.words = synthesize_words(&seg);
    vec![seg]
}

/// Fill per-word timing for any segment missing it (in place).
pub(crate) fn fill_segment_words(segments: &mut [TranscriptSegment]) {
    for seg in segments.iter_mut() {
        if seg.words.is_empty() {
            seg.words = synthesize_words(seg);
        }
    }
}

// - ggml result mapping -
//
// transcribe.cpp returns flat `segments` and `words` in milliseconds; segments
// index into the word list via `first_word`/`n_words`. These plain structs let
// the mapping (`build_segments`) be unit-tested without loading a model. Used
// only by the ggml engine, so they're dead in a `--no-default-features` build.

/// A transcribe.cpp segment, reduced to what caption mapping needs. Times in ms.
#[cfg_attr(not(feature = "ggml"), allow(dead_code))]
pub(crate) struct RawSeg {
    pub t0_ms: i64,
    pub t1_ms: i64,
    /// Index of this segment's first word in the flat word list (-1 = none).
    pub first_word: i64,
    pub n_words: i64,
    pub text: String,
}

/// A transcribe.cpp word, reduced to what caption mapping needs. Times in ms.
#[cfg_attr(not(feature = "ggml"), allow(dead_code))]
pub(crate) struct RawWord {
    pub t0_ms: i64,
    pub t1_ms: i64,
    pub text: String,
}

#[cfg_attr(not(feature = "ggml"), allow(dead_code))]
fn ms_to_secs(v: i64) -> f64 {
    v as f64 / 1000.0
}

/// Map a segment's `[first_word, first_word + n_words)` slice of the flat word
/// list into caption words, dropping empties. Bounds-checked: an out-of-range or
/// negative index yields no words (the caller then synthesizes them).
#[cfg_attr(not(feature = "ggml"), allow(dead_code))]
fn slice_words(words: &[RawWord], first: i64, n: i64) -> Vec<TranscriptWord> {
    if first < 0 || n <= 0 || first as usize >= words.len() {
        return Vec::new();
    }
    let start = first as usize;
    let end = ((first + n) as usize).min(words.len());
    words[start..end]
        .iter()
        .filter_map(|w| {
            let text = w.text.trim().to_string();
            (!text.is_empty()).then(|| TranscriptWord {
                start: ms_to_secs(w.t0_ms).max(0.0),
                end: ms_to_secs(w.t1_ms),
                text,
            })
        })
        .collect()
}

/// Turn a transcribe.cpp result into caption segments, richest-shape-first:
///   1. real segments -> map each, attaching its real word timing (synthesizing
///      per-word timing only where a segment carried none),
///   2. no segments but a flat word stream -> group words into display lines,
///   3. text only -> one block spanning the clip with synthesized word timing.
#[cfg_attr(not(feature = "ggml"), allow(dead_code))]
pub(crate) fn build_segments(
    full_text: &str,
    total_secs: f64,
    segs: &[RawSeg],
    words: &[RawWord],
) -> Vec<TranscriptSegment> {
    if !segs.is_empty() {
        let mut out: Vec<TranscriptSegment> = Vec::with_capacity(segs.len());
        for (i, s) in segs.iter().enumerate() {
            let text = s.text.trim().to_string();
            let mut seg_words = slice_words(words, s.first_word, s.n_words);
            if text.is_empty() && seg_words.is_empty() {
                continue;
            }
            let start = ms_to_secs(s.t0_ms).max(0.0);
            let end = ms_to_secs(s.t1_ms).max(start);
            clean_word_times(&mut seg_words);
            let mut seg = TranscriptSegment {
                id: format!("seg-{i}"),
                start,
                end,
                text,
                words: seg_words,
            };
            if seg.words.is_empty() {
                seg.words = synthesize_words(&seg);
            }
            out.push(seg);
        }
        if !out.is_empty() {
            return out;
        }
    }

    if !words.is_empty() {
        let flat: Vec<TranscriptWord> = words
            .iter()
            .filter_map(|w| {
                let text = w.text.trim().to_string();
                (!text.is_empty()).then(|| TranscriptWord {
                    start: ms_to_secs(w.t0_ms).max(0.0),
                    end: ms_to_secs(w.t1_ms),
                    text,
                })
            })
            .collect();
        if !flat.is_empty() {
            return group_words_into_segments(flat);
        }
    }

    let mut segments = whole_clip_segment(full_text, total_secs);
    fill_segment_words(&mut segments);
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(start: f64, end: f64, text: &str) -> TranscriptWord {
        TranscriptWord {
            start,
            end,
            text: text.into(),
        }
    }

    #[test]
    fn clean_removes_overlap_and_enforces_min_duration() {
        let mut words = vec![w(0.0, 0.5, "a"), w(0.4, 0.45, "b"), w(0.45, 1.0, "c")];
        clean_word_times(&mut words);
        // a.end clamped to b.start (no overlap).
        assert!((words[0].end - 0.4).abs() < 1e-9);
        // b is shorter than the floor but capped by c.start, so it can't grow past it.
        assert!(words[1].end <= words[2].start + 1e-9);
        // monotonic, non-overlapping throughout.
        for i in 1..words.len() {
            assert!(words[i].start >= words[i - 1].start - 1e-9);
            assert!(words[i - 1].end <= words[i].start + 1e-9);
        }
    }

    #[test]
    fn clean_extends_short_word_when_there_is_room() {
        let mut words = vec![w(0.0, 0.01, "hi"), w(2.0, 2.5, "there")];
        clean_word_times(&mut words);
        assert!((words[0].end - MIN_WORD_DUR).abs() < 1e-9);
    }

    #[test]
    fn clean_fixes_negative_and_backwards_times() {
        let mut words = vec![w(-1.0, -0.5, "x"), w(0.2, 0.1, "y")];
        clean_word_times(&mut words);
        assert!(words[0].start >= 0.0);
        assert!(words[1].end >= words[1].start);
        assert!(words[1].start >= words[0].start);
    }

    #[test]
    fn group_breaks_on_sentence_punctuation() {
        let words = vec![
            w(0.0, 0.3, "hello"),
            w(0.3, 0.6, "world."),
            w(0.7, 1.0, "next"),
        ];
        let segs = group_words_into_segments(words);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].text, "hello world.");
        assert_eq!(segs[1].text, "next");
        assert_eq!(segs[0].words.len(), 2);
    }

    #[test]
    fn group_breaks_on_long_pause() {
        let words = vec![w(0.0, 0.3, "a"), w(0.3, 0.6, "b"), w(2.0, 2.3, "c")];
        let segs = group_words_into_segments(words);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[1].text, "c");
    }

    #[test]
    fn group_caps_words_per_line() {
        let words: Vec<_> = (0..16)
            .map(|i| w(i as f64 * 0.2, i as f64 * 0.2 + 0.15, "w"))
            .collect();
        let segs = group_words_into_segments(words);
        assert!(segs.iter().all(|s| s.words.len() <= MAX_WORDS_PER_LINE));
        assert!(segs.len() >= 3);
    }

    #[test]
    fn group_glues_standalone_punctuation() {
        let words = vec![
            w(0.0, 0.3, "hello"),
            w(0.3, 0.32, ","),
            w(0.4, 0.7, "there"),
        ];
        let segs = group_words_into_segments(words);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].words.len(), 2);
        assert_eq!(segs[0].words[0].text, "hello,");
    }

    #[test]
    fn synthesize_splits_by_char_weight_and_stays_in_bounds() {
        let seg = TranscriptSegment {
            id: "s".into(),
            start: 1.0,
            end: 2.0,
            text: "a longword".into(),
            words: vec![],
        };
        let words = synthesize_words(&seg);
        assert_eq!(words.len(), 2);
        assert!(words[0].start >= 1.0 - 1e-9 && words.last().unwrap().end <= 2.0 + 1e-9);
        // "longword" (8 chars) gets more time than "a" (1 char).
        assert!(words[1].end - words[1].start > words[0].end - words[0].start);
    }

    #[test]
    fn fill_only_touches_empty_segments() {
        let mut segs = vec![
            TranscriptSegment {
                id: "0".into(),
                start: 0.0,
                end: 1.0,
                text: "one two".into(),
                words: vec![],
            },
            TranscriptSegment {
                id: "1".into(),
                start: 1.0,
                end: 2.0,
                text: "kept".into(),
                words: vec![w(1.0, 1.5, "kept")],
            },
        ];
        fill_segment_words(&mut segs);
        assert_eq!(segs[0].words.len(), 2);
        assert_eq!(segs[1].words.len(), 1); // untouched
    }

    fn rw(t0: i64, t1: i64, text: &str) -> RawWord {
        RawWord {
            t0_ms: t0,
            t1_ms: t1,
            text: text.into(),
        }
    }

    #[test]
    fn build_uses_real_segment_and_word_timing() {
        // Two segments indexing into a flat 3-word list.
        let words = vec![
            rw(0, 300, "hello"),
            rw(300, 600, "world"),
            rw(700, 1000, "next"),
        ];
        let segs = vec![
            RawSeg {
                t0_ms: 0,
                t1_ms: 600,
                first_word: 0,
                n_words: 2,
                text: "hello world".into(),
            },
            RawSeg {
                t0_ms: 700,
                t1_ms: 1000,
                first_word: 2,
                n_words: 1,
                text: "next".into(),
            },
        ];
        let out = build_segments("hello world next", 1.0, &segs, &words);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].words.len(), 2);
        assert!((out[0].start - 0.0).abs() < 1e-9);
        assert!((out[0].words[1].start - 0.3).abs() < 1e-9); // real ms -> secs
        assert_eq!(out[1].text, "next");
        assert_eq!(out[1].words.len(), 1);
    }

    #[test]
    fn build_synthesizes_words_for_a_segment_without_any() {
        // Segment present but no word rows (first_word = -1) -> synthesize.
        let segs = vec![RawSeg {
            t0_ms: 0,
            t1_ms: 1000,
            first_word: -1,
            n_words: 0,
            text: "one two three".into(),
        }];
        let out = build_segments("one two three", 1.0, &segs, &[]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].words.len(), 3);
    }

    #[test]
    fn build_groups_a_flat_word_stream_when_no_segments() {
        let words = vec![
            rw(0, 300, "hello"),
            rw(300, 600, "world."),
            rw(700, 1000, "next"),
        ];
        let out = build_segments("hello world. next", 1.0, &[], &words);
        // Sentence punctuation on "world." forces a line break (grouping path).
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn build_falls_back_to_whole_clip_on_text_only() {
        let out = build_segments("just some text", 2.0, &[], &[]);
        assert_eq!(out.len(), 1);
        assert!((out[0].end - 2.0).abs() < 1e-9);
        assert!(!out[0].words.is_empty());
    }

    #[test]
    fn build_yields_nothing_for_empty_input() {
        assert!(build_segments("", 1.0, &[], &[]).is_empty());
    }
}
