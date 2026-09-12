//! The interchange form: a v3 directory zipped for sharing, and back. Media is stored, not deflated; `.cache/` never travels.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

use crate::layout::{self, locate, Located};

#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error(transparent)]
    Locate(#[from] layout::LocateError),
    #[error("{0} is not a v3 project directory")]
    NotADirectory(PathBuf),
    #[error("{0} is not a packaged project")]
    NotPackaged(PathBuf),
    #[error("{0} exists; refusing to overwrite")]
    Exists(PathBuf),
    #[error("archive entry '{0}' escapes the target directory")]
    Escape(String),
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: io::Error,
    },
    #[error("archive: {0}")]
    Zip(#[from] zip::result::ZipError),
}

fn io_err(context: impl Into<String>) -> impl FnOnce(io::Error) -> PackageError {
    let context = context.into();
    move |source| PackageError::Io { context, source }
}

/// Zips `dir` into `file`. Media entries are stored (H.264 and PCM do not compress); everything else deflates.
/// # Errors When `dir` is not a v3 project, `file` exists, or the filesystem fails.
pub fn pack(dir: &Path, file: &Path) -> Result<(), PackageError> {
    if !matches!(locate(dir)?, Located::V3Dir(_)) {
        return Err(PackageError::NotADirectory(dir.to_path_buf()));
    }
    if file.exists() {
        return Err(PackageError::Exists(file.to_path_buf()));
    }
    let tmp = file.with_extension("packing");
    let outcome = pack_into(dir, &tmp);
    if outcome.is_err() {
        let _ = fs::remove_file(&tmp);
        return outcome;
    }
    fs::rename(&tmp, file).map_err(io_err("renaming the package into place"))
}

fn pack_into(dir: &Path, tmp: &Path) -> Result<(), PackageError> {
    let mut writer =
        zip::ZipWriter::new(File::create(tmp).map_err(io_err("creating the package"))?);
    let mut entries = Vec::new();
    collect(dir, dir, &mut entries)?;
    entries.sort();
    for relative in entries {
        let stored = relative.starts_with(layout::MEDIA_DIR);
        let options = SimpleFileOptions::default().compression_method(if stored {
            CompressionMethod::Stored
        } else {
            CompressionMethod::Deflated
        });
        writer.start_file(&relative, options)?;
        let mut source =
            File::open(dir.join(&relative)).map_err(io_err(format!("reading {relative}")))?;
        io::copy(&mut source, &mut writer).map_err(io_err(format!("packing {relative}")))?;
    }
    writer
        .finish()?
        .sync_all()
        .map_err(io_err("flushing the package"))
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), PackageError> {
    for entry in fs::read_dir(dir).map_err(io_err(format!("listing {}", dir.display())))? {
        let entry = entry.map_err(io_err("listing the project"))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != layout::CACHE_DIR {
                collect(root, &path, out)?;
            }
        } else if let Ok(relative) = path.strip_prefix(root) {
            out.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

/// Extracts a packaged project into a new directory `dir`.
/// # Errors When `file` is not a packaged project, `dir` exists, an entry would escape `dir`, or the filesystem fails.
pub fn unpack(file: &Path, dir: &Path) -> Result<(), PackageError> {
    if !matches!(locate(file)?, Located::Packaged(_)) {
        return Err(PackageError::NotPackaged(file.to_path_buf()));
    }
    if dir.exists() {
        return Err(PackageError::Exists(dir.to_path_buf()));
    }
    let staging = dir.with_extension("unpacking");
    let outcome = unpack_into(file, &staging);
    if outcome.is_err() {
        let _ = fs::remove_dir_all(&staging);
        return outcome;
    }
    fs::rename(&staging, dir).map_err(io_err("renaming the unpacked directory into place"))
}

fn unpack_into(file: &Path, staging: &Path) -> Result<(), PackageError> {
    let mut archive =
        zip::ZipArchive::new(File::open(file).map_err(io_err("opening the package"))?)?;
    fs::create_dir_all(staging).map_err(io_err("creating the target directory"))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(relative) = entry.enclosed_name() else {
            return Err(PackageError::Escape(entry.name().to_owned()));
        };
        let target = staging.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(io_err("creating a directory"))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(io_err("creating a directory"))?;
        }
        let mut out =
            File::create(&target).map_err(io_err(format!("creating {}", target.display())))?;
        io::copy(&mut entry, &mut out)
            .map_err(io_err(format!("extracting {}", target.display())))?;
    }
    layout::write_git_hints(staging).map_err(io_err("writing the git hints"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(dir: &Path) {
        for sub in [layout::MEDIA_DIR, layout::TRACKS_DIR, layout::CACHE_DIR] {
            fs::create_dir_all(dir.join(sub)).unwrap();
        }
        fs::write(dir.join(layout::DOCUMENT), "<recast v=\"3\"/>\n").unwrap();
        fs::write(dir.join(layout::RECORDING), b"MP4").unwrap();
        fs::write(dir.join(layout::CURSOR_TRACK), "{}").unwrap();
        fs::write(dir.join(layout::CACHE_DIR).join("thumb.jpg"), b"JPG").unwrap();
    }

    #[test]
    fn pack_and_unpack_round_trip_without_the_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("Take.recast");
        project(&dir);
        let file = tmp.path().join("Take.recast.zip");
        pack(&dir, &file).unwrap();
        assert_eq!(locate(&file).unwrap(), Located::Packaged(file.clone()));
        let names = layout::entry_names(&file).unwrap();
        assert!(names.iter().any(|n| n == layout::RECORDING));
        assert!(!names.iter().any(|n| n.starts_with(".cache")));
        let back = tmp.path().join("Back.recast");
        unpack(&file, &back).unwrap();
        assert_eq!(locate(&back).unwrap(), Located::V3Dir(back.clone()));
        assert_eq!(fs::read(back.join(layout::RECORDING)).unwrap(), b"MP4");
        assert!(!back.join(layout::CACHE_DIR).exists());
    }

    #[test]
    fn existing_targets_are_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("Take.recast");
        project(&dir);
        let file = tmp.path().join("busy.zip");
        fs::write(&file, b"x").unwrap();
        assert!(matches!(
            pack(&dir, &file).unwrap_err(),
            PackageError::Exists(_)
        ));
        assert!(matches!(
            pack(tmp.path(), &tmp.path().join("y.zip")).unwrap_err(),
            PackageError::Locate(_)
        ));
    }
}
