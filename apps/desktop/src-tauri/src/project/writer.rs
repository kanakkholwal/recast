use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;
use zip::{ZipArchive, ZipWriter};

use crate::project::ProjectMetadata;

pub struct ProjectWriteRequest {
    pub output_path: PathBuf,
    pub metadata: ProjectMetadata,
    pub recording_path: PathBuf,
    pub cursor_path: PathBuf,
    pub audio_path: PathBuf,
    pub microphone_path: Option<PathBuf>,
    pub camera_path: Option<PathBuf>,
    pub edits_json: String,
}

/// Write a .recast project file atomically.
/// Writes to a temporary file first, then renames to the final path.
/// This prevents corrupted project files if the process crashes mid-write.
pub fn write_project(request: ProjectWriteRequest) -> Result<PathBuf> {
    let temp_path = request.output_path.with_extension("recast.tmp");

    // Write to temporary file.
    let result = write_project_inner(&temp_path, &request);

    match result {
        Ok(()) => {
            // Atomic rename: on Windows this is a replace operation.
            // If the target exists, we overwrite it.
            if request.output_path.exists() {
                fs::remove_file(&request.output_path)
                    .context("failed to remove old project file before atomic rename")?;
            }
            fs::rename(&temp_path, &request.output_path)
                .context("failed to atomically rename project file")?;
            Ok(request.output_path)
        }
        Err(e) => {
            // Clean up the temp file on failure.
            let _ = fs::remove_file(&temp_path);
            Err(e)
        }
    }
}

fn write_project_inner(path: &Path, request: &ProjectWriteRequest) -> Result<()> {
    let file = File::create(path)?;
    let mut writer = ZipWriter::new(file);
    let deflated = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    // Use Stored for media files — H.264/PCM don't benefit from Deflate.
    let stored = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .unix_permissions(0o644);

    writer.start_file("metadata.json", deflated)?;
    writer.write_all(serde_json::to_string_pretty(&request.metadata)?.as_bytes())?;

    writer.start_file("cursor.json", deflated)?;
    copy_file(&request.cursor_path, &mut writer)?;

    writer.start_file("audio.wav", stored)?;
    copy_file(&request.audio_path, &mut writer)?;

    writer.start_file("edits.json", deflated)?;
    writer.write_all(request.edits_json.as_bytes())?;

    writer.start_file("recording.mp4", stored)?;
    copy_file(&request.recording_path, &mut writer)?;

    if let Some(ref mic_path) = request.microphone_path {
        writer.start_file("microphone.wav", stored)?;
        copy_file(mic_path, &mut writer)?;
    }

    if let Some(ref cam_path) = request.camera_path {
        writer.start_file("camera.mp4", stored)?;
        copy_file(cam_path, &mut writer)?;
    }

    writer.finish()?;
    Ok(())
}

/// Rewrite the `edits.json` entry inside an existing `.recast` archive in place,
/// preserving all other entries (recording.mp4, audio.wav, etc.) by raw-copying
/// their compressed bytes — no decode/re-encode of media.
///
/// The write is atomic: a sibling `.recast.tmp` is produced first and only
/// renamed over the original on success.
pub fn update_project_edits(project_path: &Path, edits_json: &str) -> Result<()> {
    let temp_path = project_path.with_extension("recast.tmp");

    let result = update_project_edits_inner(project_path, &temp_path, edits_json);

    match result {
        Ok(()) => {
            if project_path.exists() {
                fs::remove_file(project_path)
                    .context("failed to remove old project file before atomic rename")?;
            }
            fs::rename(&temp_path, project_path)
                .context("failed to atomically rename project file")?;
            Ok(())
        }
        Err(e) => {
            let _ = fs::remove_file(&temp_path);
            Err(e)
        }
    }
}

fn update_project_edits_inner(
    project_path: &Path,
    temp_path: &Path,
    edits_json: &str,
) -> Result<()> {
    let src = File::open(project_path)
        .with_context(|| format!("failed to open project at {}", project_path.display()))?;
    let mut archive = ZipArchive::new(src).context("failed to read project archive")?;

    let dst = File::create(temp_path)?;
    let mut writer = ZipWriter::new(dst);
    let deflated = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);

    for i in 0..archive.len() {
        let entry = archive.by_index_raw(i)?;
        if entry.name() == "edits.json" {
            continue;
        }
        writer.raw_copy_file(entry)?;
    }

    writer.start_file("edits.json", deflated)?;
    writer.write_all(edits_json.as_bytes())?;

    writer.finish()?;
    Ok(())
}

fn copy_file(path: &Path, writer: &mut ZipWriter<File>) -> Result<()> {
    let mut file = File::open(path)?;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read])?;
    }
    Ok(())
}
