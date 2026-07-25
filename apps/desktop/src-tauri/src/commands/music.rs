//! Download a remote music/audio asset (e.g. a Jamendo track) into the app's
//! persistent cache and hand back a local path. Keeps playback offline-first:
//! the frontend stores the returned path as the clip's asset, never the URL.

use tauri::{AppHandle, Manager};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use futures_util::StreamExt;

use super::error::{AppError, AppResult};

/// Download `url` to `<app_data>/music/<id>.mp3` (skipped if already cached) and
/// return the local path. `id` is sanitized to a safe filename.
#[tauri::command]
pub async fn download_music_asset(app: AppHandle, url: String, id: String) -> AppResult<String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::msg(format!("app_data_dir: {e}")))?
        .join("music");
    fs::create_dir_all(&dir)
        .await
        .map_err(|e| AppError::msg(format!("create music cache: {e}")))?;

    let safe: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if safe.is_empty() {
        return Err(AppError::msg("empty music id"));
    }
    let dest = dir.join(format!("{safe}.mp3"));
    if dest.exists() {
        return Ok(dest.to_string_lossy().to_string());
    }

    let tmp = dest.with_extension("tmp");
    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| AppError::msg(format!("http client: {e}")))?;
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::msg(format!("request: {e}")))?
        .error_for_status()
        .map_err(|e| AppError::msg(format!("http: {e}")))?;

    let mut file = fs::File::create(&tmp)
        .await
        .map_err(|e| AppError::msg(format!("create tmp: {e}")))?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| AppError::msg(format!("stream: {e}")))?;
        file.write_all(&bytes)
            .await
            .map_err(|e| AppError::msg(format!("write: {e}")))?;
    }
    file.flush()
        .await
        .map_err(|e| AppError::msg(format!("flush: {e}")))?;
    drop(file);

    if dest.exists() {
        let _ = fs::remove_file(&dest).await;
    }
    fs::rename(&tmp, &dest)
        .await
        .map_err(|e| AppError::msg(format!("rename: {e}")))?;
    Ok(dest.to_string_lossy().to_string())
}
