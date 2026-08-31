//! Caption font resolution for the export burn-in, in pure Rust so it builds everywhere.
//! libass scales by `Fontsize / (ascent + descent)` and matches the LEGACY family name, so raw CSS px and family both render wrong.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// The resolved face's bytes, kept so a caption line can be shaped/measured
/// without re-reading the file per chunk.
#[derive(Debug, Clone)]
pub struct MeasureFace {
    pub data: Arc<Vec<u8>>,
    pub index: u32,
    /// Font units per em (glyph advances are in these units).
    pub upem: f64,
}

/// What the burn-in needs about the resolved face.
#[derive(Debug, Clone)]
pub struct FontMatch {
    /// The family name to write as ASS `Fontname` (legacy name, ID 1).
    pub ass_name: String,
    /// ASS `Fontsize` = `css_px * ass_scale`, so libass renders glyphs at the
    /// same pixel height as the preview. `= (winAscent - winDescent) / upem`.
    pub ass_scale: f64,
    /// The face bytes for measuring line widths (the rounded pill).
    pub measure: MeasureFace,
}

/// Rendered width in px of `text` at `css_px`, shaped with rustybuzz (a HarfBuzz
/// port, so kerning/ligatures match libass). `spacing_px` is the per-glyph ASS
/// Spacing (letter spacing), added after each glyph as libass does. Used to size
/// the rounded pill; a few px of error is absorbed by the pill padding.
pub fn measure_line_width(face: &MeasureFace, css_px: f64, text: &str, spacing_px: f64) -> f64 {
    let Some(rb) = rustybuzz::Face::from_slice(&face.data, face.index) else {
        return 0.0;
    };
    let mut buf = rustybuzz::UnicodeBuffer::new();
    buf.push_str(text);
    buf.guess_segment_properties();
    let glyphs = rustybuzz::shape(&rb, &[], buf);
    let advance: i32 = glyphs.glyph_positions().iter().map(|p| p.x_advance).sum();
    let scale = if face.upem > 0.0 {
        css_px / face.upem
    } else {
        0.0
    };
    advance as f64 * scale + spacing_px * glyphs.glyph_infos().len() as f64
}

struct Db {
    db: fontdb::Database,
    loaded_dirs: HashSet<PathBuf>,
}

fn db() -> &'static Mutex<Db> {
    static DB: OnceLock<Mutex<Db>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        // Scanning the OS font dirs is the slow part; do it once behind the lock.
        db.load_system_fonts();
        Mutex::new(Db {
            db,
            loaded_dirs: HashSet::new(),
        })
    })
}

/// Resolve the face libass will use for `family` at `weight`, reading the size
/// correction and the match name off the font file. `custom_dir` is the pack /
/// download directory to also search (the same dir handed to libass via
/// `fontsdir`); pass `None` for system families. Returns `None` when no face
/// matches (caller falls back to the CSS name + no correction).
pub fn resolve_font(family: &str, weight: u32, custom_dir: Option<&Path>) -> Option<FontMatch> {
    let mut guard = db().lock().ok()?;
    if let Some(dir) = custom_dir {
        if !guard.loaded_dirs.contains(dir) {
            guard.db.load_fonts_dir(dir);
            guard.loaded_dirs.insert(dir.to_path_buf());
        }
    }

    let query = fontdb::Query {
        families: &[fontdb::Family::Name(family)],
        weight: fontdb::Weight(weight as u16),
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };
    let id = guard.db.query(&query)?;

    guard.db.with_face_data(id, |data, index| {
        let face = ttf_parser::Face::parse(data, index).ok()?;
        let upem = face.units_per_em() as f64;
        if upem <= 0.0 {
            return None;
        }
        // The denominator libass uses; ttf-parser reports the windows descender negative, so this is a subtraction.
        let (win_asc, win_desc) = match face.tables().os2 {
            Some(os2) => (
                os2.windows_ascender() as f64,
                os2.windows_descender() as f64,
            ),
            // No OS/2 table: fall back to the em box (no correction).
            None => (upem, 0.0),
        };
        let denom = win_asc - win_desc;
        let ass_scale = if denom > 0.0 { denom / upem } else { 1.0 };

        // The name libass matches on: legacy family (name ID 1), else the query.
        let ass_name = face
            .names()
            .into_iter()
            .find(|n| n.name_id == 1)
            .and_then(|n| n.to_string())
            .unwrap_or_else(|| family.to_string());

        Some(FontMatch {
            ass_name,
            ass_scale,
            measure: MeasureFace {
                data: Arc::new(data.to_vec()),
                index,
                upem,
            },
        })
    })?
}
