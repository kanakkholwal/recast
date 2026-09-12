//! Where things live inside a `Name.recast/` directory, and how to tell a v3 directory from the zips that came before it.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const DOCUMENT: &str = "project.rcx";
pub const CAPTURE: &str = "capture.json";
pub const MEDIA_DIR: &str = "media";
pub const TRACKS_DIR: &str = "tracks";
pub const BRANCHES_DIR: &str = "branches";
pub const CACHE_DIR: &str = ".cache";

pub const RECORDING: &str = "media/recording.mp4";
pub const CAMERA: &str = "media/camera.mp4";
pub const MIC: &str = "media/mic.wav";
pub const SYSTEM: &str = "media/system.wav";
pub const CURSOR_TRACK: &str = "tracks/cursor.json";
pub const WORDS_TRACK: &str = "tracks/words.json";

pub const GITATTRIBUTES: &str = ".gitattributes";
pub const GITIGNORE: &str = ".gitignore";

const GITATTRIBUTES_TEXT: &str =
    "# A Recast project. The document is XML text; the media are binary.
project.rcx text eol=lf linguist-language=XML diff=html
*.json text eol=lf
media/** binary
";

const GITIGNORE_TEXT: &str =
    "# Derived from the document: the write-ahead log, caches and thumbnails.
.cache/
";

/// Writes the git hints a fresh project wants, keeping any file the user already has.
/// # Errors When the filesystem fails.
pub fn write_git_hints(root: &Path) -> std::io::Result<()> {
    for (name, text) in [
        (GITATTRIBUTES, GITATTRIBUTES_TEXT),
        (GITIGNORE, GITIGNORE_TEXT),
    ] {
        let path = root.join(name);
        if !path.exists() {
            std::fs::write(path, text)?;
        }
    }
    Ok(())
}

/// Paths under one project directory. Every path here is relative to `root`, spelled with forward slashes in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    root: PathBuf,
}

impl Layout {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn document(&self) -> PathBuf {
        self.root.join(DOCUMENT)
    }

    #[must_use]
    pub fn capture(&self) -> PathBuf {
        self.root.join(CAPTURE)
    }

    /// A document-relative `src` resolved under the root. Only relative forms resolve here; the host maps the others.
    #[must_use]
    pub fn resolve(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    #[must_use]
    pub fn branches(&self) -> PathBuf {
        self.root.join(BRANCHES_DIR)
    }

    #[must_use]
    pub fn cache(&self) -> PathBuf {
        self.root.join(CACHE_DIR)
    }

    #[must_use]
    pub fn wal(&self) -> PathBuf {
        self.cache().join("wal.log")
    }

    #[must_use]
    pub fn store_state(&self) -> PathBuf {
        self.cache().join("store.json")
    }
}

/// What a path names, decided by shape alone so a caller can refuse or migrate before reading anything large.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Located {
    /// A v3 directory holding `project.rcx`.
    V3Dir(PathBuf),
    /// A zip of a v3 directory: the interchange form.
    Packaged(PathBuf),
    /// A v2 bundle: `project.json` manifest plus `edits/` sections.
    V2Zip(PathBuf),
    /// A v1 bundle: a flat `edits.json`.
    V1Zip(PathBuf),
}

#[derive(Debug, thiserror::Error)]
pub enum LocateError {
    #[error("{0} does not exist")]
    Missing(PathBuf),
    #[error("{0} is a directory without a {DOCUMENT}")]
    NotAProjectDir(PathBuf),
    #[error("{0} is neither a project directory nor a project archive")]
    Unrecognised(PathBuf),
    #[error("could not read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// # Errors When the path is missing, a directory without a document, or an archive of an unknown shape.
pub fn locate(path: &Path) -> Result<Located, LocateError> {
    if !path.exists() {
        return Err(LocateError::Missing(path.to_path_buf()));
    }
    if path.is_dir() {
        return if path.join(DOCUMENT).is_file() {
            Ok(Located::V3Dir(path.to_path_buf()))
        } else {
            Err(LocateError::NotAProjectDir(path.to_path_buf()))
        };
    }
    let io = |source| LocateError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut magic = [0u8; 4];
    let mut file = File::open(path).map_err(io)?;
    let read = file.read(&mut magic).map_err(io)?;
    if read < 4 || &magic[..2] != b"PK" {
        return Err(LocateError::Unrecognised(path.to_path_buf()));
    }
    let names = entry_names(path).map_err(io)?;
    let has = |name: &str| names.iter().any(|n| n == name);
    Ok(if has(DOCUMENT) {
        Located::Packaged(path.to_path_buf())
    } else if has("project.json") {
        Located::V2Zip(path.to_path_buf())
    } else if has("edits.json") {
        Located::V1Zip(path.to_path_buf())
    } else {
        return Err(LocateError::Unrecognised(path.to_path_buf()));
    })
}

/// Entry names from the central directory only; nothing is inflated.
pub(crate) fn entry_names(path: &Path) -> std::io::Result<Vec<String>> {
    let file = File::open(path)?;
    let archive = zip::ZipArchive::new(file).map_err(std::io::Error::other)?;
    Ok(archive.file_names().map(str::to_owned).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip_with(dir: &Path, name: &str, entries: &[&str]) -> PathBuf {
        let path = dir.join(name);
        let file = File::create(&path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        for entry in entries {
            writer
                .start_file(*entry, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"{}").unwrap();
        }
        writer.finish().unwrap();
        path
    }

    #[test]
    fn every_shape_is_told_apart_from_its_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let v1 = zip_with(tmp.path(), "a.recast", &["edits.json", "recording.mp4"]);
        let v2 = zip_with(
            tmp.path(),
            "b.recast",
            &["project.json", "edits/frame.json"],
        );
        let packed = zip_with(tmp.path(), "c.recast", &[DOCUMENT, "media/recording.mp4"]);
        assert_eq!(locate(&v1).unwrap(), Located::V1Zip(v1.clone()));
        assert_eq!(locate(&v2).unwrap(), Located::V2Zip(v2.clone()));
        assert_eq!(locate(&packed).unwrap(), Located::Packaged(packed.clone()));
        let dir = tmp.path().join("d.recast");
        std::fs::create_dir(&dir).unwrap();
        assert!(matches!(
            locate(&dir).unwrap_err(),
            LocateError::NotAProjectDir(_)
        ));
        std::fs::write(dir.join(DOCUMENT), "<recast v=\"3\"/>").unwrap();
        assert_eq!(locate(&dir).unwrap(), Located::V3Dir(dir));
    }

    #[test]
    fn a_non_archive_file_and_a_missing_path_are_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let text = tmp.path().join("x.recast");
        std::fs::write(&text, "hello").unwrap();
        assert!(matches!(
            locate(&text).unwrap_err(),
            LocateError::Unrecognised(_)
        ));
        assert!(matches!(
            locate(&tmp.path().join("nope")).unwrap_err(),
            LocateError::Missing(_)
        ));
    }

    #[test]
    fn the_layout_places_every_file_under_the_root() {
        let layout = Layout::new("/p/Name.recast");
        assert!(layout.document().ends_with(DOCUMENT));
        assert!(layout.resolve(RECORDING).ends_with("media/recording.mp4"));
        assert!(layout.wal().ends_with(".cache/wal.log"));
    }

    #[test]
    fn a_fresh_project_gets_git_hints() {
        let tmp = tempfile::tempdir().unwrap();

        write_git_hints(tmp.path()).unwrap();

        assert!(std::fs::read_to_string(tmp.path().join(GITATTRIBUTES))
            .unwrap()
            .contains("linguist-language=XML"));
    }

    #[test]
    fn a_hint_file_the_user_edited_is_left_alone() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(GITIGNORE), "mine").unwrap();

        write_git_hints(tmp.path()).unwrap();

        assert_eq!(
            std::fs::read_to_string(tmp.path().join(GITIGNORE)).unwrap(),
            "mine"
        );
    }
}
