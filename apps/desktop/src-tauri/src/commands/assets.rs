use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tokio::fs;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetEntry {
    pub id: String,
    pub filename: String,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub thumb_filename: Option<String>,
    #[serde(default)]
    pub thumb_url: Option<String>,
    #[serde(default)]
    pub thumb_sha256: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    #[serde(default)]
    pub version: Option<String>,
    pub assets: Vec<AssetEntry>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AssetInstallResult {
    pub installed: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<FailedAsset>,
    pub cache_dir: String,
    pub hydrated: Vec<HydratedAsset>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailedAsset {
    pub id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HydratedAsset {
    pub id: String,
    pub path: Option<String>,
    pub thumb_path: Option<String>,
}

fn assets_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir unavailable: {e}"))?;
    Ok(base.join("assets"))
}

async fn file_sha256(path: &Path) -> std::io::Result<String> {
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

/// Download `url` to `final_path`, streaming into a sibling `.tmp` file while
/// hashing. Verifies against `expected_sha256` before atomically renaming.
async fn download_verified(
    client: &reqwest::Client,
    url: &str,
    expected_sha256: &str,
    final_path: &Path,
) -> Result<(), String> {
    let tmp_path = final_path.with_extension("tmp");
    let resp = client
        .get(url)
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
    if !got.eq_ignore_ascii_case(expected_sha256) {
        let _ = fs::remove_file(&tmp_path).await;
        return Err(format!(
            "sha256 mismatch (expected {}, got {})",
            expected_sha256, got
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

/// Ensure `target` exists on disk with content matching `expected_sha256`. If
/// it already does, return `true` (skipped). Otherwise download + verify.
async fn ensure_one(
    client: &reqwest::Client,
    url: &str,
    expected_sha256: &str,
    target: &Path,
) -> Result<bool, String> {
    if target.exists() {
        if let Ok(h) = file_sha256(target).await {
            if h.eq_ignore_ascii_case(expected_sha256) {
                return Ok(true);
            }
        }
        let _ = fs::remove_file(target).await;
    }
    download_verified(client, url, expected_sha256, target).await?;
    Ok(false)
}

fn read_lock(dir: &Path) -> Option<Manifest> {
    let bytes = std::fs::read(dir.join("manifest.lock.json")).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn hydrate_from_lock(dir: &Path) -> Vec<HydratedAsset> {
    let Some(manifest) = read_lock(dir) else {
        return Vec::new();
    };
    manifest
        .assets
        .into_iter()
        .map(|entry| {
            let full = dir.join(&entry.filename);
            let thumb = entry
                .thumb_filename
                .as_ref()
                .map(|name| dir.join(name))
                .filter(|p| p.exists());
            HydratedAsset {
                id: entry.id,
                path: full
                    .exists()
                    .then(|| full.to_string_lossy().to_string()),
                thumb_path: thumb.map(|p| p.to_string_lossy().to_string()),
            }
        })
        .collect()
}

/// No-network command. Reads the persisted lock file and returns which assets
/// (full-res + thumb) are already on disk. Call at startup before attempting
/// a network install so the UI can hydrate from cache even when offline.
#[tauri::command]
pub fn hydrate_cached_assets(app: AppHandle) -> Result<Vec<HydratedAsset>, String> {
    let dir = assets_dir(&app)?;
    Ok(hydrate_from_lock(&dir))
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

    // Thumbs first — tiny, finish quickly, unblock the picker UI before the
    // multi-megabyte full-res downloads run.
    for entry in manifest.assets.iter() {
        if let (Some(thumb_name), Some(thumb_url), Some(thumb_hash)) = (
            entry.thumb_filename.as_ref(),
            entry.thumb_url.as_ref(),
            entry.thumb_sha256.as_ref(),
        ) {
            let target = dir.join(thumb_name);
            if let Err(reason) = ensure_one(&client, thumb_url, thumb_hash, &target).await {
                result.failed.push(FailedAsset {
                    id: format!("{}#thumb", entry.id),
                    reason,
                });
            }
        }
    }

    for entry in manifest.assets.iter() {
        let target = dir.join(&entry.filename);
        match ensure_one(&client, &entry.url, &entry.sha256, &target).await {
            Ok(true) => result.skipped.push(entry.id.clone()),
            Ok(false) => result.installed.push(entry.id.clone()),
            Err(reason) => result.failed.push(FailedAsset {
                id: entry.id.clone(),
                reason,
            }),
        }
    }

    // Persist the resolved manifest so subsequent launches can hydrate from
    // disk without a network round-trip.
    let lock_path = dir.join("manifest.lock.json");
    if let Ok(json) = serde_json::to_vec_pretty(&manifest) {
        let _ = fs::write(&lock_path, json).await;
    }

    result.hydrated = hydrate_from_lock(&dir);
    Ok(result)
}

#[tauri::command]
pub fn get_cached_asset_path(app: AppHandle, id: String) -> Option<String> {
    let dir = assets_dir(&app).ok()?;
    let manifest = read_lock(&dir)?;
    let entry = manifest.assets.iter().find(|a| a.id == id)?;
    let path = dir.join(&entry.filename);
    path.exists()
        .then(|| path.to_string_lossy().to_string())
}
