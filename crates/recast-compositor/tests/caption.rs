use recast_captions::{CaptionAnimation, CaptionStyle, TranscriptWord};
use recast_compositor::{layout_caption, CaptionClock, CaptionFrame, VideoRect};
use recast_text::{resolve_face, FontFace, GlyphAtlas};
use recast_time::{MappedSpan, TimeMap};

const CANVAS: (u32, u32) = (1280, 720);

fn face() -> Option<FontFace> {
    for family in ["Arial", "Segoe UI", "Helvetica", "DejaVu Sans"] {
        if let Some(resolved) = resolve_face(family, 400, None) {
            return Some(resolved.face);
        }
    }
    eprintln!("skipping: no system font resolved");
    None
}

/// A 16:9 video filling most of the canvas, so there is padding to place a
/// caption into and the offset range is live.
fn video() -> VideoRect {
    VideoRect {
        x: 64.0,
        y: 36.0,
        w: 1152.0,
        h: 648.0,
    }
}

fn words(spec: &[(f64, f64, &str)]) -> Vec<TranscriptWord> {
    spec.iter()
        .map(|(start, end, text)| TranscriptWord {
            start: *start,
            end: *end,
            text: (*text).to_string(),
        })
        .collect()
}

fn sentence() -> Vec<TranscriptWord> {
    words(&[
        (0.0, 0.5, "the"),
        (0.5, 1.0, "quick"),
        (1.0, 1.5, "brown"),
        (1.5, 2.0, "fox"),
    ])
}

fn style() -> CaptionStyle {
    CaptionStyle {
        animation: Some(CaptionAnimation {
            chunk: "line".into(),
            highlight: Some("none".into()),
            ..CaptionAnimation::default()
        }),
        ..CaptionStyle::default()
    }
}

/// No retiming in these fixtures, so the two axes coincide.
fn clock(t: f64) -> CaptionClock<'static> {
    CaptionClock {
        source: t,
        output: t,
        time_map: None,
    }
}

fn lay(style: &CaptionStyle, words: &[TranscriptWord], t: f64) -> CaptionFrame {
    let Some(face) = face() else {
        return CaptionFrame::default();
    };
    let mut atlas = GlyphAtlas::new(512, 2048);
    layout_caption(style, words, clock(t), video(), CANVAS, &face, 0, &mut atlas)
}

#[test]
fn every_glyph_sits_inside_the_pill_that_backs_it() {
    let Some(_) = face() else { return };
    let frame = lay(&style(), &sentence(), 1.0);
    let pill = frame.pill.expect("the default style has a box background");
    assert!(!frame.glyphs.is_empty());
    for quad in &frame.glyphs {
        let (x, y, w, h) = (quad.rect[0], quad.rect[1], quad.rect[2], quad.rect[3]);
        assert!(
            x >= pill.x - 1.0
                && y >= pill.y - 1.0
                && x + w <= pill.x + pill.w + 1.0
                && y + h <= pill.y + pill.h + 1.0,
            "glyph {:?} escapes pill {:?}",
            quad.rect,
            (pill.x, pill.y, pill.w, pill.h)
        );
    }
}

#[test]
fn the_pill_hugs_the_text_rather_than_the_frame() {
    let Some(_) = face() else { return };
    let short = lay(&style(), &words(&[(0.0, 1.0, "hi")]), 0.5);
    let long = lay(&style(), &sentence(), 1.0);
    let (short, long) = (short.pill.unwrap(), long.pill.unwrap());
    assert!(
        long.w > short.w,
        "a longer line did not widen the pill: {} vs {}",
        long.w,
        short.w
    );
    assert!(long.w < CANVAS.0 as f32, "the pill spans the whole canvas");
}

#[test]
fn nothing_is_laid_out_before_the_first_word_starts() {
    let Some(_) = face() else { return };
    let words = words(&[(2.0, 2.5, "late")]);
    assert!(lay(&style(), &words, 0.0).is_empty());
    assert!(!lay(&style(), &words, 2.1).is_empty());
}

#[test]
fn a_disabled_style_lays_out_nothing() {
    let Some(_) = face() else { return };
    let disabled = CaptionStyle {
        enabled: false,
        ..style()
    };
    assert!(lay(&disabled, &sentence(), 1.0).is_empty());
}

#[test]
fn a_bottom_caption_sits_below_a_top_one() {
    let Some(_) = face() else { return };
    let bottom = lay(&style(), &sentence(), 1.0).pill.unwrap();
    let top = lay(
        &CaptionStyle {
            position: "top".into(),
            ..style()
        },
        &sentence(),
        1.0,
    )
    .pill
    .unwrap();
    assert!(bottom.y > top.y, "bottom {} was not below top {}", bottom.y, top.y);
}

/// Positive offset moves a bottom caption INWARD over the video, so it rises.
#[test]
fn a_positive_offset_lifts_a_bottom_caption_onto_the_video() {
    let Some(_) = face() else { return };
    let flush = lay(
        &CaptionStyle {
            offset_pct: 0.0,
            ..style()
        },
        &sentence(),
        1.0,
    )
    .pill
    .unwrap();
    let lifted = lay(
        &CaptionStyle {
            offset_pct: 12.0,
            ..style()
        },
        &sentence(),
        1.0,
    )
    .pill
    .unwrap();
    assert!(
        lifted.y < flush.y,
        "offset did not lift the caption: {} vs {}",
        lifted.y,
        flush.y
    );
}

#[test]
fn alignment_moves_the_pill_across_the_video() {
    let Some(_) = face() else { return };
    let at = |align: &str| {
        lay(
            &CaptionStyle {
                align: align.into(),
                ..style()
            },
            &sentence(),
            1.0,
        )
        .pill
        .unwrap()
        .x
    };
    let (left, centre, right) = (at("left"), at("center"), at("right"));
    assert!(left < centre && centre < right, "{left} {centre} {right}");
}

#[test]
fn uppercase_reshapes_the_text() {
    let Some(_) = face() else { return };
    let plain = lay(&style(), &sentence(), 1.0);
    let shouted = lay(
        &CaptionStyle {
            uppercase: true,
            ..style()
        },
        &sentence(),
        1.0,
    );
    assert_ne!(
        plain.glyphs.iter().map(|q| q.uv).collect::<Vec<_>>(),
        shouted.glyphs.iter().map(|q| q.uv).collect::<Vec<_>>(),
        "uppercase drew the same glyphs"
    );
}

/// Progressive highlight is the Loom look: spoken words in the base colour,
/// unspoken ones muted.
#[test]
fn progressive_highlight_mutes_the_words_not_yet_spoken() {
    let Some(_) = face() else { return };
    let progressive = CaptionStyle {
        color: "#ffffff".into(),
        muted_color: "#808080".into(),
        animation: Some(CaptionAnimation {
            chunk: "line".into(),
            highlight: Some("progressive".into()),
            ..CaptionAnimation::default()
        }),
        ..style()
    };
    let midway = lay(&progressive, &sentence(), 1.0);
    let colours: Vec<[f32; 4]> = midway.glyphs.iter().map(|q| q.colour).collect();
    assert!(
        colours.iter().any(|c| c[0] > 0.9) && colours.iter().any(|c| c[0] < 0.6),
        "expected both bright and muted words, got {colours:?}"
    );

    let finished = lay(&progressive, &sentence(), 5.0);
    assert!(
        finished.glyphs.iter().all(|q| q.colour[0] > 0.9),
        "every word should be bright once all are spoken"
    );
}

#[test]
fn colour_emphasis_accents_only_the_active_word() {
    let Some(_) = face() else { return };
    let accented = CaptionStyle {
        color: "#ffffff".into(),
        animation: Some(CaptionAnimation {
            chunk: "line".into(),
            emphasis: "color".into(),
            emphasis_color: "#ff0000".into(),
            highlight: Some("none".into()),
            ..CaptionAnimation::default()
        }),
        ..style()
    };
    let frame = lay(&accented, &sentence(), 0.7);
    let accent = frame.glyphs.iter().filter(|q| q.colour[1] < 0.5).count();
    assert!(accent > 0, "no word took the accent");
    assert!(
        accent < frame.glyphs.len(),
        "every word took the accent, so it is not per-word"
    );
}

#[test]
fn word_chunking_shows_one_word_at_a_time() {
    let Some(_) = face() else { return };
    let per_word = CaptionStyle {
        animation: Some(CaptionAnimation {
            chunk: "word".into(),
            ..CaptionAnimation::default()
        }),
        ..style()
    };
    let whole = lay(&style(), &sentence(), 1.2);
    let single = lay(&per_word, &sentence(), 1.2);
    assert!(
        single.glyphs.len() < whole.glyphs.len(),
        "word chunking drew {} glyphs against {} for the whole line",
        single.glyphs.len(),
        whole.glyphs.len()
    );
}

#[test]
fn a_line_too_long_for_one_row_wraps_and_the_pill_grows_taller() {
    let Some(_) = face() else { return };
    let long = words(&[
        (0.0, 0.4, "supercalifragilistic"),
        (0.4, 0.8, "expialidocious"),
        (0.8, 1.2, "antidisestablishment"),
    ]);
    let one_row = lay(
        &CaptionStyle {
            max_chars_per_line: 200,
            ..style()
        },
        &long,
        1.0,
    )
    .pill
    .unwrap();
    let wrapped = lay(
        &CaptionStyle {
            max_chars_per_line: 20,
            max_lines: 3,
            ..style()
        },
        &long,
        1.0,
    )
    .pill
    .unwrap();
    assert!(wrapped.h > one_row.h, "wrapping did not add a row");
    assert!(wrapped.w < one_row.w, "wrapping did not narrow the pill");
}

/// Packing a glyph can grow the atlas, which changes the height every uv is
/// divided by. Quads emitted before that growth must not keep the old scale.
#[test]
fn every_uv_matches_the_atlas_the_frame_ended_with() {
    let Some(face) = face() else { return };
    // Deliberately small, so laying out one caption forces at least one growth.
    let mut atlas = GlyphAtlas::new(256, 4096);
    let long = words(&[
        (0.0, 0.4, "abcdefghij"),
        (0.4, 0.8, "klmnopqrst"),
        (0.8, 1.2, "uvwxyzABCD"),
        (1.2, 1.6, "EFGHIJKLMN"),
    ]);
    let style = CaptionStyle {
        font_size_pct: 9.0,
        max_chars_per_line: 12,
        max_lines: 4,
        ..style()
    };
    let (_, before) = atlas.size();
    let frame = layout_caption(&style, &long, clock(1.5), video(), CANVAS, &face, 0, &mut atlas);
    let (width, height) = atlas.size();
    assert!(height > before, "the atlas never grew, so this proves nothing");
    assert!(!frame.glyphs.is_empty());

    for quad in &frame.glyphs {
        let [u0, v0, u1, v1] = quad.uv;
        assert!(
            (0.0..=1.0).contains(&v0) && (0.0..=1.0).contains(&v1) && v1 > v0,
            "uv {:?} is outside the atlas",
            quad.uv
        );
        // The quad is drawn at its packed pixel size, so the uv span has to be
        // that many texels of the FINAL atlas.
        let span_x = (u1 - u0) * width as f32;
        let span_y = (v1 - v0) * height as f32;
        assert!(
            (span_x - quad.rect[2]).abs() < 0.5 && (span_y - quad.rect[3]).abs() < 0.5,
            "uv span {span_x}x{span_y} does not match the {}x{} quad",
            quad.rect[2],
            quad.rect[3]
        );
    }
}

/// A wrapped caption centres each line inside the pill; leaving them left-flush
/// looks broken next to a full first line.
#[test]
fn a_short_wrapped_line_is_centred_rather_than_left_flush() {
    let Some(_) = face() else { return };
    let long = words(&[
        (0.0, 0.4, "aaaaaaaaaa"),
        (0.4, 0.8, "bbbbbbbbbb"),
        (0.8, 1.2, "cc"),
    ]);
    let frame = lay(
        &CaptionStyle {
            max_chars_per_line: 21,
            max_lines: 2,
            ..style()
        },
        &long,
        1.5,
    );
    // Grouped by the baseline (these letters have no descender), because glyph
    // TOPS differ within a row: 'b' has an ascender and 'a' does not.
    let mut rows: Vec<(i32, f32, f32)> = Vec::new();
    for quad in &frame.glyphs {
        let key = (quad.rect[1] + quad.rect[3]).round() as i32;
        match rows.iter_mut().find(|r| r.0 == key) {
            Some(row) => {
                row.1 = row.1.min(quad.rect[0]);
                row.2 = row.2.max(quad.rect[0] + quad.rect[2]);
            }
            None => rows.push((key, quad.rect[0], quad.rect[0] + quad.rect[2])),
        }
    }
    rows.sort_by_key(|r| r.0);
    assert_eq!(rows.len(), 2, "expected the caption to wrap to two rows");
    let (wide, narrow) = (rows[0], rows[1]);
    assert!(
        narrow.2 - narrow.1 < wide.2 - wide.1,
        "the second row is not the shorter one"
    );
    assert!(
        narrow.1 > wide.1 + 1.0,
        "the short row starts at {} against {}, so it was left-flush",
        narrow.1,
        wide.1
    );
}

// --- Entrance ---

fn entrance_style(kind: &str, ms: f64) -> CaptionStyle {
    CaptionStyle {
        animation: Some(CaptionAnimation {
            chunk: "line".into(),
            highlight: Some("none".into()),
            entrance: kind.into(),
            entrance_ms: ms,
            ..CaptionAnimation::default()
        }),
        ..style()
    }
}

fn lay_at(style: &CaptionStyle, words: &[TranscriptWord], clock: CaptionClock<'_>) -> CaptionFrame {
    let Some(face) = face() else {
        return CaptionFrame::default();
    };
    let mut atlas = GlyphAtlas::new(512, 2048);
    layout_caption(style, words, clock, video(), CANVAS, &face, 0, &mut atlas)
}

#[test]
fn a_fade_entrance_ramps_the_alpha_and_settles_opaque() {
    let Some(_) = face() else { return };
    let style = entrance_style("fade", 400.0);
    let alpha = |t: f64| lay(&style, &sentence(), t).glyphs[0].colour[3];
    // The chunk starts at 0.0, so t IS the elapsed entrance time.
    let (early, mid, settled) = (alpha(0.02), alpha(0.2), alpha(1.0));
    assert!(early < mid, "alpha did not ramp: {early} then {mid}");
    assert!(mid < settled, "alpha did not keep ramping: {mid} then {settled}");
    assert!((settled - 1.0).abs() < 1e-6, "settled at {settled}");
}

#[test]
fn no_entrance_is_opaque_from_the_first_frame() {
    let Some(_) = face() else { return };
    let style = entrance_style("none", 400.0);
    assert!((lay(&style, &sentence(), 0.01).glyphs[0].colour[3] - 1.0).abs() < 1e-6);
}

/// A zero duration is the same as no entrance, and must not divide by it.
#[test]
fn a_zero_length_entrance_is_inert() {
    let Some(_) = face() else { return };
    let style = entrance_style("fade", 0.0);
    assert!((lay(&style, &sentence(), 0.0).glyphs[0].colour[3] - 1.0).abs() < 1e-6);
}

#[test]
fn a_slide_entrance_starts_below_where_it_settles() {
    let Some(_) = face() else { return };
    let style = entrance_style("slide", 400.0);
    let top = |t: f64| lay(&style, &sentence(), t).glyphs[0].rect[1];
    let (early, settled) = (top(0.02), top(1.0));
    assert!(
        early > settled + 1.0,
        "the slide did not travel: {early} then {settled}"
    );
}

/// `pop` scales in place; travelling would make it a slide.
#[test]
fn a_pop_entrance_scales_without_travelling() {
    let Some(_) = face() else { return };
    let pop = entrance_style("pop", 400.0);
    let slide = entrance_style("slide", 400.0);
    let centre_shift = |style: &CaptionStyle| {
        let early = lay(style, &sentence(), 0.02).pill.unwrap();
        let settled = lay(style, &sentence(), 1.0).pill.unwrap();
        (early.y + early.h / 2.0) - (settled.y + settled.h / 2.0)
    };
    assert!(
        centre_shift(&pop).abs() < 0.5,
        "pop moved by {}",
        centre_shift(&pop)
    );
    assert!(centre_shift(&slide) > 1.0, "slide did not move");

    let early = lay(&pop, &sentence(), 0.02).pill.unwrap();
    let settled = lay(&pop, &sentence(), 1.0).pill.unwrap();
    assert!(early.w < settled.w, "pop did not scale up");
}

/// The pill scales about its own centre, so the glyphs must stay inside it
/// throughout the entrance rather than sliding out during the ramp.
#[test]
fn the_glyphs_stay_inside_the_pill_mid_entrance() {
    let Some(_) = face() else { return };
    let frame = lay(&entrance_style("pop", 400.0), &sentence(), 0.05);
    let pill = frame.pill.unwrap();
    for quad in &frame.glyphs {
        assert!(
            quad.rect[0] >= pill.x - 1.0
                && quad.rect[0] + quad.rect[2] <= pill.x + pill.w + 1.0,
            "glyph {:?} escaped pill {:?} mid-entrance",
            quad.rect,
            (pill.x, pill.w)
        );
    }
}

/// The entrance is clocked on the OUTPUT axis: on a 2x segment the viewer sees
/// it at the authored wall-clock rate, not at twice the speed.
#[test]
fn the_entrance_runs_on_the_output_axis_not_the_original_one() {
    let Some(_) = face() else { return };
    let style = entrance_style("fade", 400.0);
    // One span playing at 2x: 4 s of original become 2 s of output.
    let map = TimeMap {
        spans: vec![MappedSpan {
            orig_start: 0.0,
            orig_end: 4.0,
            speed: 2.0,
            out_start: 0.0,
            out_end: 2.0,
        }],
        output_duration: 2.0,
    };
    // Original 0.4 is output 0.2, which is half of a 400 ms entrance.
    let sped = lay_at(
        &style,
        &sentence(),
        CaptionClock {
            source: 0.4,
            output: 0.2,
            time_map: Some(&map),
        },
    );
    // The same OUTPUT elapsed with no retiming has to look identical.
    let plain = lay_at(
        &style,
        &sentence(),
        CaptionClock {
            source: 0.4,
            output: 0.2,
            time_map: None,
        },
    );
    assert!((sped.glyphs[0].colour[3] - plain.glyphs[0].colour[3]).abs() < 1e-6);
    // And it must NOT have finished, which is what clocking on source would do.
    assert!(
        sped.glyphs[0].colour[3] < 0.95,
        "the entrance was already over at {}",
        sped.glyphs[0].colour[3]
    );
}

/// A chunk that starts mid-clip has its entrance measured from ITS start, not
/// from zero, or every later chunk appears fully settled.
#[test]
fn a_later_chunks_entrance_is_measured_from_its_own_start() {
    let Some(_) = face() else { return };
    let style = CaptionStyle {
        animation: Some(CaptionAnimation {
            chunk: "word".into(),
            highlight: Some("none".into()),
            entrance: "fade".into(),
            entrance_ms: 400.0,
            ..CaptionAnimation::default()
        }),
        ..style()
    };
    // The second word starts at 0.5, so 0.52 is 20 ms into ITS entrance.
    let fresh = lay(&style, &sentence(), 0.52).glyphs[0].colour[3];
    let settled = lay(&style, &sentence(), 0.95).glyphs[0].colour[3];
    assert!(
        fresh < 0.5,
        "the second chunk started already settled at {fresh}"
    );
    assert!((settled - 1.0).abs() < 1e-6);
}
