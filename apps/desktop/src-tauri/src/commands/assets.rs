use std::path::PathBuf;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tokio::fs;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssetEntry {
    pub id: String,
    pub filename: String,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    #[serde(default)]
    pub version: Option<String>,
    pub assets: Vec<AssetEntry>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct AssetInstallResult {
    pub installed: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<FailedAsset>,
    pub cache_dir: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailedAsset {
    pub id: String,
    pub reason: String,
}

fn assets_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir unavailable: {e}"))?;
    Ok(base.join("assets"))
}

async fn file_sha256(path: &PathBuf) -> std::io::Result<String> {
    use tokio::io::AsyncReadExt;
    let mut file = fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

async fn download_verified(
    client: &reqwest::Client,
    entry: &AssetEntry,
    final_path: &PathBuf,
) -> Result<(), String> {
    let tmp_path = final_path.with_extension("tmp");
    let resp = client
        .get(&entry.url)
        .send()
        .await
        .map_err(|e| format!("request: {e}"))?
        .error_for_status()
        .map_err(|e| format!("http: {e}"))?;

    let mut hasher = Sha256::new();
    let mut file = fs::File::create(&tmp_path)
        .await
        .map_err(|e| format!("create tmp: {e}"))?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("stream: {e}"))?;
        hasher.update(&bytes);
        file.write_all(&bytes)
            .await
            .map_err(|e| format!("write: {e}"))?;
    }
    file.flush().await.map_err(|e| format!("flush: {e}"))?;
    drop(file);

    let got = hex::encode(hasher.finalize());
    if !got.eq_ignore_ascii_case(&entry.sha256) {
        let _ = fs::remove_file(&tmp_path).await;
        return Err(format!(
            "sha256 mismatch (expected {}, got {})",
            entry.sha256, got
        ));
    }

    if final_path.exists() {
        let _ = fs::remove_file(&final_path).await;
    }
    fs::rename(&tmp_path, &final_path)
        .await
        .map_err(|e| format!("rename: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn ensure_assets_installed(
    app: AppHandle,
    manifest_url: String,
) -> Result<AssetInstallResult, String> {
    let dir = assets_dir(&app)?;
    fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("create dir: {e}"))?;

    let client = reqwest::Client::builder()
        .user_agent("recast-desktop")
        .build()
        .map_err(|e| format!("client: {e}"))?;

    let manifest: Manifest = client
        .get(&manifest_url)
        .send()
        .await
        .map_err(|e| format!("manifest request: {e}"))?
        .error_for_status()
        .map_err(|e| format!("manifest http: {e}"))?
        .json()
        .await
        .map_err(|e| format!("manifest parse: {e}"))?;

    let mut result = AssetInstallResult {
        cache_dir: dir.to_string_lossy().to_string(),
        ..Default::default()
    };

    for entry in manifest.assets.iter() {
        let target = dir.join(&entry.filename);
        if target.exists() {
            match file_sha256(&target).await {
                Ok(h) if h.eq_ignore_ascii_case(&entry.sha256) => {
                    result.skipped.push(entry.id.clone());
                    continue;
                }
                _ => {
                    let _ = fs::remove_file(&target).await;
                }
            }
        }
        match download_verified(&client, entry, &target).await {
            Ok(()) => result.installed.push(entry.id.clone()),
            Err(reason) => result.failed.push(FailedAsset {
                id: entry.id.clone(),
                reason,
            }),
        }
    }

    let lock_path = dir.join("manifest.lock.json");
    if let Ok(json) = serde_json::to_vec_pretty(&manifest) {
        let _ = fs::write(&lock_path, json).await;
    }

    Ok(result)
}

#[tauri::command]
pub fn get_cached_asset_path(app: AppHandle, id: String) -> Option<String> {
    let dir = assets_dir(&app).ok()?;
    let lock_path = dir.join("manifest.lock.json");
    let bytes = std::fs::read(&lock_path).ok()?;
    let manifest: Manifest = serde_json::from_slice(&bytes).ok()?;
    let entry = manifest.assets.iter().find(|a| a.id == id)?;
    let path = dir.join(&entry.filename);
    if path.exists() {
        Some(path.to_string_lossy().to_string())
    } else {
        None
    }
}

