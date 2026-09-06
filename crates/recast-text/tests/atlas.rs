use recast_text::{resolve_face, shape_line, FontFace, GlyphAtlas};

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

fn glyph_id(face: &FontFace, ch: char) -> u16 {
    shape_line(face, 32.0, &ch.to_string(), 0.0).glyphs[0].id
}

fn ink(atlas: &GlyphAtlas) -> usize {
    atlas.coverage().iter().filter(|p| **p > 0).count()
}

#[test]
fn a_packed_glyph_lands_inside_the_atlas_with_its_ink() {
    let Some(face) = face() else { return };
    let mut atlas = GlyphAtlas::new(256, 1024);
    let placed = atlas
        .insert(0, &face, glyph_id(&face, 'M'), 48.0)
        .expect("M should rasterise");
    let (width, height) = atlas.size();
    assert!(placed.x + placed.width <= width);
    assert!(placed.y + placed.height <= height);
    assert!(ink(&atlas) > 0, "the mask was never blitted");
}

#[test]
fn the_same_glyph_at_the_same_size_is_packed_once() {
    let Some(face) = face() else { return };
    let mut atlas = GlyphAtlas::new(256, 1024);
    let id = glyph_id(&face, 'M');
    let first = atlas.insert(0, &face, id, 48.0).unwrap();
    let painted = ink(&atlas);
    let second = atlas.insert(0, &face, id, 48.0).unwrap();
    assert_eq!(first, second);
    assert_eq!(ink(&atlas), painted, "the glyph was blitted a second time");
}

/// The face id is the caller's numbering, so two fonts must not collide on it.
#[test]
fn the_same_glyph_id_from_a_different_face_gets_its_own_slot() {
    let Some(face) = face() else { return };
    let mut atlas = GlyphAtlas::new(256, 1024);
    let id = glyph_id(&face, 'M');
    let first = atlas.insert(0, &face, id, 48.0).unwrap();
    let second = atlas.insert(1, &face, id, 48.0).unwrap();
    assert_ne!((first.x, first.y), (second.x, second.y));
}

#[test]
fn a_bigger_size_is_a_different_entry() {
    let Some(face) = face() else { return };
    let mut atlas = GlyphAtlas::new(512, 1024);
    let id = glyph_id(&face, 'M');
    let small = atlas.insert(0, &face, id, 24.0).unwrap();
    let large = atlas.insert(0, &face, id, 96.0).unwrap();
    assert!(large.width > small.width && large.height > small.height);
}

/// Sizes are snapped to quarter pixels, so a resize sweep reuses entries
/// instead of minting one per frame.
#[test]
fn sizes_inside_one_quarter_pixel_share_an_entry_and_wider_ones_do_not() {
    let Some(face) = face() else { return };
    let mut atlas = GlyphAtlas::new(512, 1024);
    let id = glyph_id(&face, 'M');
    let first = atlas.insert(0, &face, id, 48.0).unwrap();
    assert_eq!(atlas.insert(0, &face, id, 48.05).unwrap(), first);
    let stepped = atlas.insert(0, &face, id, 48.3).unwrap();
    assert_ne!((stepped.x, stepped.y), (first.x, first.y));
}

/// Linear sampling reaches half a texel past the quad, so packed glyphs need a
/// transparent gutter or a neighbour's ink bleeds into the edge.
#[test]
fn packed_glyphs_keep_a_gutter_between_them() {
    let Some(face) = face() else { return };
    let mut atlas = GlyphAtlas::new(512, 1024);
    let mut placed = Vec::new();
    for ch in "MMMMMMMM".chars().zip("abcdefgh".chars()).map(|(_, c)| c) {
        if let Some(spot) = atlas.insert(0, &face, glyph_id(&face, ch), 40.0) {
            placed.push(spot);
        }
    }
    assert!(placed.len() > 4);
    for (i, a) in placed.iter().enumerate() {
        for b in &placed[i + 1..] {
            let rows_overlap = a.y < b.y + b.height && b.y < a.y + a.height;
            if rows_overlap {
                let gap = a.x.max(b.x) - (a.x.min(b.x) + if a.x < b.x { a.width } else { b.width });
                assert!(gap >= 1, "{a:?} and {b:?} touch");
            }
        }
    }
}

#[test]
fn packed_glyphs_do_not_overlap() {
    let Some(face) = face() else { return };
    let mut atlas = GlyphAtlas::new(128, 1024);
    let mut placed = Vec::new();
    for ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz".chars() {
        if let Some(spot) = atlas.insert(0, &face, glyph_id(&face, ch), 40.0) {
            placed.push(spot);
        }
    }
    assert!(placed.len() > 20, "too few glyphs to be a real test");
    for (i, a) in placed.iter().enumerate() {
        for b in &placed[i + 1..] {
            let apart = a.x + a.width <= b.x
                || b.x + b.width <= a.x
                || a.y + a.height <= b.y
                || b.y + b.height <= a.y;
            assert!(apart, "{a:?} overlaps {b:?}");
        }
    }
}

#[test]
fn a_space_has_no_ink_and_is_not_packed() {
    let Some(face) = face() else { return };
    let mut atlas = GlyphAtlas::new(256, 1024);
    let space = shape_line(&face, 32.0, " ", 0.0).glyphs[0].id;
    assert!(atlas.insert(0, &face, space, 48.0).is_none());
    assert_eq!(ink(&atlas), 0);
}

#[test]
fn the_atlas_grows_instead_of_refusing_a_glyph_that_would_not_fit() {
    let Some(face) = face() else { return };
    let mut atlas = GlyphAtlas::new(256, 2048);
    let (_, start) = atlas.size();
    let generation = atlas.generation();
    for ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZ".chars() {
        assert!(atlas.insert(0, &face, glyph_id(&face, ch), 90.0).is_some());
    }
    let (_, grown) = atlas.size();
    assert!(grown > start, "still {grown} rows after 26 large glyphs");
    assert!(atlas.generation() > generation, "growth must be observable");
    assert!(!atlas.overflowed());
}

#[test]
fn a_glyph_that_cannot_fit_at_all_is_refused_rather_than_packed_out_of_bounds() {
    let Some(face) = face() else { return };
    let mut atlas = GlyphAtlas::new(16, 16);
    assert!(atlas
        .insert(0, &face, glyph_id(&face, 'M'), 200.0)
        .is_none());
    assert!(atlas.overflowed());
}

/// A refusal must not be cached: it would outlive the reset that frees room.
#[test]
fn a_refused_glyph_is_packed_after_a_reset_gives_it_room() {
    let Some(face) = face() else { return };
    let mut atlas = GlyphAtlas::new(256, 256);
    let mut refused = None;
    for ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789".chars() {
        let id = glyph_id(&face, ch);
        if atlas.insert(0, &face, id, 80.0).is_none() && atlas.overflowed() {
            refused = Some(id);
            break;
        }
    }
    let Some(id) = refused else {
        panic!("the atlas never filled up");
    };
    atlas.reset();
    assert!(atlas.insert(0, &face, id, 80.0).is_some());
}

#[test]
fn the_dirty_range_covers_the_rows_a_new_glyph_wrote() {
    let Some(face) = face() else { return };
    let mut atlas = GlyphAtlas::new(256, 1024);
    let placed = atlas.insert(0, &face, glyph_id(&face, 'M'), 48.0).unwrap();
    let (from, to) = atlas.take_dirty().expect("a packed glyph is dirty");
    assert!(from <= placed.y && to >= placed.y + placed.height);
    assert!(atlas.take_dirty().is_none(), "the range was not consumed");
}

#[test]
fn a_cache_hit_dirties_nothing() {
    let Some(face) = face() else { return };
    let mut atlas = GlyphAtlas::new(256, 1024);
    let id = glyph_id(&face, 'M');
    atlas.insert(0, &face, id, 48.0);
    atlas.take_dirty();
    atlas.insert(0, &face, id, 48.0);
    assert!(atlas.take_dirty().is_none());
}

#[test]
fn a_reset_clears_the_ink_and_the_packing() {
    let Some(face) = face() else { return };
    let mut atlas = GlyphAtlas::new(256, 1024);
    let id = glyph_id(&face, 'M');
    let before = atlas.insert(0, &face, id, 48.0).unwrap();
    let generation = atlas.generation();
    atlas.reset();
    assert_eq!(ink(&atlas), 0);
    assert!(atlas.generation() > generation);
    let after = atlas.insert(0, &face, id, 48.0).unwrap();
    assert_eq!((before.x, before.y), (after.x, after.y));
}
