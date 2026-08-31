use recast_text::{rasterize, resolve_face, shape_line, FontFace};

/// Every assertion needs a real face. CI runs Windows, which always has Arial; a machine without it skips rather than failing, the same shape the GPU tests use for a missing adapter.
fn face() -> Option<FontFace> {
    for family in [
        "Arial",
        "Segoe UI",
        "Helvetica",
        "DejaVu Sans",
        "Liberation Sans",
    ] {
        if let Some(resolved) = resolve_face(family, 400, None) {
            return Some(resolved.face);
        }
    }
    eprintln!("skipping: no system font resolved");
    None
}

#[test]
fn a_line_advances_by_the_sum_of_its_glyph_advances() {
    let Some(face) = face() else { return };
    let one = shape_line(&face, 64.0, "n", 0.0);
    let two = shape_line(&face, 64.0, "nn", 0.0);
    assert_eq!(one.glyphs.len(), 1);
    assert_eq!(two.glyphs.len(), 2);
    // The second 'n' starts where the first one's advance ended.
    assert!((two.glyphs[1].x - one.width).abs() < 1e-6);
    assert!((two.width - one.width * 2.0).abs() < 1e-6);
}

/// ASS `Spacing` is added after EVERY glyph, including the last, which is what
/// makes the export's pill width match.
#[test]
fn letter_spacing_is_added_after_every_glyph() {
    let Some(face) = face() else { return };
    let plain = shape_line(&face, 40.0, "abc", 0.0);
    let spaced = shape_line(&face, 40.0, "abc", 5.0);
    assert!((spaced.width - plain.width - 15.0).abs() < 1e-6);
}

#[test]
fn the_line_scales_linearly_with_the_pixel_size() {
    let Some(face) = face() else { return };
    let small = shape_line(&face, 20.0, "Recast", 0.0);
    let large = shape_line(&face, 40.0, "Recast", 0.0);
    assert!(
        (large.width - small.width * 2.0).abs() < 1e-6,
        "{small:?} {large:?}"
    );
}

#[test]
fn an_empty_line_shapes_to_nothing_rather_than_failing() {
    let Some(face) = face() else { return };
    let line = shape_line(&face, 32.0, "", 0.0);
    assert!(line.glyphs.is_empty());
    assert_eq!(line.width, 0.0);
}

/// Clusters are what a per-word highlight keys on, so a glyph has to carry the
/// byte offset it came from.
#[test]
fn glyphs_carry_the_byte_offset_they_came_from() {
    let Some(face) = face() else { return };
    let line = shape_line(&face, 32.0, "ab", 0.0);
    assert_eq!(line.glyphs[0].cluster, 0);
    assert_eq!(line.glyphs[1].cluster, 1);
}

#[test]
fn a_rasterised_glyph_has_ink_in_it() {
    let Some(face) = face() else { return };
    let line = shape_line(&face, 64.0, "H", 0.0);
    let mask = rasterize(&face, line.glyphs[0].id, 64.0).expect("an outline for H");
    assert!(mask.width > 0 && mask.height > 0);
    assert_eq!(mask.coverage.len(), (mask.width * mask.height) as usize);
    assert!(
        mask.coverage.iter().any(|&c| c > 200),
        "no solid coverage anywhere in the mask"
    );
    // A capital H sits ABOVE the baseline, so its top is negative in y-down.
    assert!(mask.top < 0, "top {} is not above the baseline", mask.top);
}

#[test]
fn a_bigger_size_rasterises_a_bigger_mask() {
    let Some(face) = face() else { return };
    let id = shape_line(&face, 32.0, "H", 0.0).glyphs[0].id;
    let small = rasterize(&face, id, 32.0).expect("small");
    let large = rasterize(&face, id, 64.0).expect("large");
    assert!(large.height > small.height, "{small:?} {large:?}");
}

/// A space has no outline. Returning an empty mask instead of `None` would put
/// a zero-area quad in the atlas for every space in every caption.
#[test]
fn a_space_rasterises_to_nothing() {
    let Some(face) = face() else { return };
    let line = shape_line(&face, 64.0, " ", 0.0);
    assert_eq!(line.glyphs.len(), 1);
    assert!(rasterize(&face, line.glyphs[0].id, 64.0).is_none());
    // It still advances, or words would collide.
    assert!(line.width > 0.0);
}

#[test]
fn the_faces_line_height_covers_its_ascender_and_descender() {
    let Some(face) = face() else { return };
    let metrics = face.metrics();
    assert!(metrics.ascender > 0.0);
    assert!(metrics.descender < 0.0);
    assert!(metrics.line_height() >= metrics.ascender - metrics.descender);
}
