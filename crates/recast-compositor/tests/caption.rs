use recast_captions::{CaptionAnimation, CaptionStyle, TranscriptWord};
use recast_compositor::{layout_caption, CaptionFrame, VideoRect};
use recast_text::{resolve_face, FontFace, GlyphAtlas};

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

fn lay(style: &CaptionStyle, words: &[TranscriptWord], t: f64) -> CaptionFrame {
    let Some(face) = face() else {
        return CaptionFrame::default();
    };
    let mut atlas = GlyphAtlas::new(512, 2048);
    layout_caption(style, words, t, video(), CANVAS, &face, 0, &mut atlas)
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
    let frame = layout_caption(&style, &long, 1.5, video(), CANVAS, &face, 0, &mut atlas);
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
