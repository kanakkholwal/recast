//! ocrs model paths and download-on-first-use, under an `ocrs` subfolder of the caption models root.
//! These are ocrs's own default URLs, fetched through the caption pipeline's verified streaming downloader.

use std::path::PathBuf;

use tauri::AppHandle;

use crate::transcription::{download_file, models_dir};

/// ocrs default text-detection model (pure-Rust rten). Weights are CC-BY-SA;
/// attribution is owed and a sha256 pin is owed before release.
const DETECTION_URL: &str = "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten";
/// ocrs default text-recognition model.
const RECOGNITION_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten";

const SUBDIR: &str = "ocrs";
const DETECTION_FILE: &str = "text-detection.rten";
const RECOGNITION_FILE: &str = "text-recognition.rten";

/// On-disk paths to the detection and recognition model files.
pub struct OcrModelPaths {
    pub detection: PathBuf,
    pub recognition: PathBuf,
}

fn dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(models_dir(app)?.join(SUBDIR))
}

pub fn model_paths(app: &AppHandle) -> Result<OcrModelPaths, String> {
    let dir = dir(app)?;
    Ok(OcrModelPaths {
        detection: dir.join(DETECTION_FILE),
        recognition: dir.join(RECOGNITION_FILE),
    })
}

/// Whether both models are already on disk. Lets a caller skip announcing a
/// download phase on the overwhelmingly common run where there is nothing to fetch.
pub fn models_present(app: &AppHandle) -> bool {
    model_paths(app).is_ok_and(|p| p.detection.exists() && p.recognition.exists())
}

/// Ensure both model files exist locally, downloading any that are missing.
/// `on_progress(downloaded, total)` reports per-chunk download bytes.
pub async fn ensure_models(
    app: &AppHandle,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<OcrModelPaths, String> {
    let paths = model_paths(app)?;
    let client = reqwest::Client::new();
    if !paths.detection.exists() {
        download_file(
            &client,
            DETECTION_URL,
            None,
            &paths.detection,
            &mut on_progress,
        )
        .await?;
    }
    if !paths.recognition.exists() {
        download_file(
            &client,
            RECOGNITION_URL,
            None,
            &paths.recognition,
            &mut on_progress,
        )
        .await?;
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guards against the URL and the on-disk filename drifting apart, which would
    /// silently save a model under the wrong name and re-download it forever.
    #[test]
    fn urls_and_filenames_agree() {
        assert!(DETECTION_URL.ends_with(DETECTION_FILE));
        assert!(RECOGNITION_URL.ends_with(RECOGNITION_FILE));
        assert_ne!(DETECTION_FILE, RECOGNITION_FILE);
    }
}
