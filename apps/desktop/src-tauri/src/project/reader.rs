use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use zip::ZipArchive;

use crate::project::ProjectMetadata;

#[derive(Debug, Clone)]
pub struct ProjectOpenResult {
    pub metadata: ProjectMetadata,
    pub recording_path: PathBuf,
    pub cursor_path: PathBuf,
    pub edits_path: PathBuf,
    pub audio_path: Option<PathBuf>,
    pub microphone_path: Option<PathBuf>,
    pub camera_path: Option<PathBuf>,
}

pub fn open_project(path: &Path) -> Result<ProjectOpenResult> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;

    let metadata: ProjectMetadata = {
        let mut metadata_entry = archive.by_name("metadata.json")?;
        let mut bytes = Vec::new();
        metadata_entry.read_to_end(&mut bytes)?;
        serde_json::from_slice(&bytes)?
    };

    let cache_dir = cache_dir_for(path)?;
    fs::create_dir_all(&cache_dir)?;

    let recording_path = extract_entry(
        &mut archive,
        "recording.mp4",
        &cache_dir.join("recording.mp4"),
    )?;
    let cursor_path = extract_entry(&mut archive, "cursor.json", &cache_dir.join("cursor.json"))?;
    let edits_path = extract_entry(&mut archive, "edits.json", &cache_dir.join("edits.json"))?;

    // Optional entries — v1 archives may not have these.
    let audio_path = try_extract_entry(&mut archive, "audio.wav", &cache_dir.join("audio.wav"));
    let microphone_path = try_extract_entry(
        &mut archive,
        "microphone.wav",
        &cache_dir.join("microphone.wav"),
    );
    let camera_path = try_extract_entry(&mut archive, "camera.mp4", &cache_dir.join("camera.mp4"));

    Ok(ProjectOpenResult {
        metadata,
        recording_path,
        cursor_path,
        edits_path,
        audio_path,
        microphone_path,
        camera_path,
    })
}

fn cache_dir_for(project_path: &Path) -> Result<PathBuf> {
    let metadata = fs::metadata(project_path)?;
    let stem = project_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("project");
    Ok(env::temp_dir()
        .join("recast-cache")
        .join(format!("{stem}-{}", metadata.len())))
}

fn extract_entry(archive: &mut ZipArchive<File>, name: &str, path: &Path) -> Result<PathBuf> {
    let mut entry = archive
        .by_name(name)
        .with_context(|| format!("missing {name} in project"))?;
    let mut output = File::create(path)?;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = entry.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read])?;
    }
    Ok(path.to_path_buf())
}

/// Try to extract an optional entry from the archive. Returns None if the entry doesn't exist.
fn try_extract_entry(archive: &mut ZipArchive<File>, name: &str, path: &Path) -> Option<PathBuf> {
    let mut entry = archive.by_name(name).ok()?;
    let mut output = File::create(path).ok()?;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = entry.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read]).ok()?;
    }
    Some(path.to_path_buf())
}
