use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;
use zip::{ZipArchive, ZipWriter};

use crate::project::format::{self, ProjectManifest};
use crate::project::ProjectMetadata;

pub struct ProjectWriteRequest {
    pub output_path: PathBuf,
    pub metadata: ProjectMetadata,
    pub recording_path: PathBuf,
    pub cursor_path: PathBuf,
    pub audio_path: Option<PathBuf>,
    pub microphone_path: Option<PathBuf>,
    pub camera_path: Option<PathBuf>,
    pub edits_json: String,
}

/// Write a .recast project file atomically. Writes to a temporary file first, then renames to the final path. This prevents corrupted project files if the process crashes mid-write.
pub fn write_project(request: ProjectWriteRequest) -> Result<PathBuf> {
    let temp_path = request.output_path.with_extension("recast.tmp");

    // Write to temporary file.
    let result = write_project_inner(&temp_path, &request);

    match result {
        Ok(()) => {
            // `fs::rename` already replaces the target atomically; deleting first would leave a window where only the .tmp exists.
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

    let manifest = ProjectManifest::new(
        request.metadata.created_at_unix_ms,
        request.audio_path.is_some(),
        request.microphone_path.is_some(),
        request.camera_path.is_some(),
        true,
    );
    writer.start_file(format::MANIFEST_NAME, deflated)?;
    writer.write_all(format::to_canonical_string(&serde_json::to_value(&manifest)?)?.as_bytes())?;

    writer.start_file(format::METADATA_NAME, deflated)?;
    writer.write_all(serde_json::to_string_pretty(&request.metadata)?.as_bytes())?;

    writer.start_file(format::ASSET_CURSOR_TRACK, deflated)?;
    copy_file(&request.cursor_path, &mut writer)?;

    if let Some(ref audio_path) = request.audio_path {
        writer.start_file(format::ASSET_AUDIO, stored)?;
        copy_file(audio_path, &mut writer)?;
    }

    write_edit_sections(&mut writer, deflated, &request.edits_json)?;

    writer.start_file(format::ASSET_VIDEO, stored)?;
    copy_file(&request.recording_path, &mut writer)?;

    if let Some(ref mic_path) = request.microphone_path {
        writer.start_file(format::ASSET_MICROPHONE, stored)?;
        copy_file(mic_path, &mut writer)?;
    }

    if let Some(ref cam_path) = request.camera_path {
        writer.start_file(format::ASSET_CAMERA, stored)?;
        copy_file(cam_path, &mut writer)?;
    }

    // Flush before the caller renames: without it, power loss can leave a renamed-into-place file that is partial.
    writer.finish()?.sync_all()?;
    Ok(())
}

/// Split a flat edits payload into per-section files, each canonically
/// serialized so the bundle diffs cleanly. The single place edits are written.
fn write_edit_sections(
    writer: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    edits_json: &str,
) -> Result<()> {
    let flat: serde_json::Value =
        serde_json::from_str(edits_json).context("edits payload is not valid JSON")?;
    for (section, value) in format::split_edits(&flat) {
        writer.start_file(format::section_path(section), options)?;
        writer.write_all(format::to_canonical_string(&value)?.as_bytes())?;
    }
    Ok(())
}

/// Rewrites the `edits/` sections of a v2 `.recast` in place, raw-copying every other entry's compressed bytes so media is never re-encoded.
/// Atomic via a sibling `.recast.tmp` renamed on success, and it errors on a v1 archive so a save can never produce a hybrid bundle.
pub fn update_project_edits(project_path: &Path, edits_json: &str) -> Result<()> {
    let temp_path = project_path.with_extension("recast.tmp");

    let result = update_project_edits_inner(project_path, &temp_path, edits_json);

    match result {
        Ok(()) => {
            // Replace in one step; see `write_project` for why the old file isn't deleted first.
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

    let names: Vec<String> = archive.file_names().map(str::to_string).collect();
    if !format::is_v2(&names) {
        bail!("cannot save edits: project is not format v2 (migrate first)");
    }

    let dst = File::create(temp_path)?;
    let mut writer = ZipWriter::new(dst);
    let deflated = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);

    for i in 0..archive.len() {
        let entry = archive.by_index_raw(i)?;
        if entry.name().starts_with("edits/") {
            continue;
        }
        writer.raw_copy_file(entry)?;
    }

    write_edit_sections(&mut writer, deflated, edits_json)?;

    writer.finish()?.sync_all()?;
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
