use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use crate::face::FontFace;

/// A face plus the name libass will match it on. The LEGACY name (ID 1) is what
/// libass looks up, and for a non-RIBBI weight like Inter-600 that is
/// "Inter SemiBold" rather than "Inter", so passing the CSS family through
/// silently falls back to a system face.
#[derive(Debug, Clone)]
pub struct ResolvedFace {
    pub face: FontFace,
    pub ass_name: String,
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

/// Resolves `family` at `weight`, also searching `extra_dir` (the pack or
/// download directory handed to libass as `fontsdir`). `None` when nothing
/// matches, which the caller answers by falling back to the CSS name.
pub fn resolve_face(family: &str, weight: u16, extra_dir: Option<&Path>) -> Option<ResolvedFace> {
    let mut guard = db().lock().ok()?;
    if let Some(dir) = extra_dir {
        if !guard.loaded_dirs.contains(dir) {
            guard.db.load_fonts_dir(dir);
            guard.loaded_dirs.insert(dir.to_path_buf());
        }
    }
    let query = fontdb::Query {
        families: &[fontdb::Family::Name(family)],
        weight: fontdb::Weight(weight),
        ..Default::default()
    };
    let id = guard.db.query(&query)?;
    let (source, index) = guard.db.face_source(id)?;
    let ass_name = guard
        .db
        .face(id)
        .and_then(|info| info.families.first().map(|(name, _)| name.clone()))
        .unwrap_or_else(|| family.to_string());
    let data = match source {
        fontdb::Source::Binary(bytes) => Arc::new(bytes.as_ref().as_ref().to_vec()),
        fontdb::Source::File(path) => Arc::new(std::fs::read(path).ok()?),
        fontdb::Source::SharedFile(_, bytes) => Arc::new(bytes.as_ref().as_ref().to_vec()),
    };
    Some(ResolvedFace {
        face: FontFace::from_bytes(data, index)?,
        ass_name,
    })
}
