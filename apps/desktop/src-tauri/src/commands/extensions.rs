//! Declarative asset-pack extensions (Tier 1).
//!
//! An extension is a *manifest + static assets* — no executable code, so there
//! is no privilege-escalation surface to sandbox. The trust model is therefore
//! validation, not isolation:
//!   - HTTPS-only manifest/asset URLs (localhost allowed for dev),
//!   - per-asset SHA256 verification (reuses [`crate::commands::assets::ensure_one`]),
//!   - strict manifest schema (`kind == "asset-pack"`, **no** permissions),
//!   - filename-traversal rejection (bare filenames only; Windows device names
//!     and drive prefixes blocked),
//!   - a reserved `signature` field + [`verify_signature`] seam for a future
//!     Ed25519 publisher-signing fast-follow.
//!
//! Installed packs live under `app_data_dir/extensions/<extId>/` with the
//! resolved manifest persisted as `extension.lock.json` and an enable flag in
//! `state.json`. The render boundary already accepts absolute file paths for
//! backgrounds and (rasterized) cursor sprites, so packs need **no** new render
//! code — only this installer.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tokio::fs;

use super::assets::{ensure_one, AssetEntry};
use super::error::{AppError, AppResult};

/// Manifest as published by a pack author / curated registry. `contributes` is
/// kept opaque (`serde_json::Value`) — the frontend interprets the per-kind
/// contribution shapes; Rust only validates the security-relevant envelope and
/// downloads `assets`.
fn empty_contributions() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: Option<String>,
    /// Must be `"asset-pack"` for now. Reserved for future `"plugin"` kinds.
    pub kind: String,
    /// Forward-compat hedge: asset-packs must declare an empty set. A non-empty
    /// list is rejected at install so a pack can never request capabilities.
    #[serde(default)]
    pub permissions: Vec<String>,
    /// Reserved for Ed25519 publisher signing (see [`verify_signature`]).
    #[serde(default)]
    pub signature: Option<String>,
    /// Opaque per-kind contributions, interpreted frontend-side. Defaults to an
    /// empty object, not `Value::Null`: TS declares `contributes` non-nullable
    /// (`ExtensionContributions`), so a bare `default` wrote `null` over the
    /// wire and every reader needed a `?? {}` guard.
    #[serde(default = "empty_contributions")]
    pub contributes: serde_json::Value,
    pub assets: Vec<AssetEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ExtState {
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_true() -> bool {
    true
}

/// Resolved on-disk asset location for one manifest-local asset id.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtAssetPath {
    pub id: String,
    pub path: Option<String>,
    pub thumb_path: Option<String>,
}

/// An installed pack + its hydrated asset paths, returned to the frontend which
/// maps `contributes` + these paths into registry entries.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledExtension {
    pub manifest: ExtensionManifest,
    pub enabled: bool,
    pub dir: String,
    pub assets: Vec<ExtAssetPath>,
}

fn extensions_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir unavailable: {e}"))?;
    Ok(base.join("extensions"))
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("recast-desktop")
        .build()
        .map_err(|e| format!("client: {e}"))
}

// ── Validation (the security gate) ──────────────────────────────────────────

/// Allow only `https://` (or `http://` to a loopback host for local dev/testing).
fn url_allowed(url: &str) -> bool {
    match reqwest::Url::parse(url) {
        Ok(u) => match u.scheme() {
            "https" => true,
            "http" => matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "::1")),
            _ => false,
        },
        Err(_) => false,
    }
}

/// Reserved Windows device names (stem, case-insensitive) that resolve to
/// devices regardless of extension.
const RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// A pack asset filename must be a *bare* filename written directly into the
/// extension's own directory — no path separators, parent refs, drive prefixes,
/// control chars, trailing dot/space (Windows trims them), or reserved device
/// names. This is the primary traversal defense.
pub(crate) fn is_safe_filename(name: &str) -> bool {
    if name.is_empty() || name.len() > 255 {
        return false;
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return false;
    }
    if name.chars().any(|c| c.is_control()) {
        return false;
    }
    // Drive prefix like `C:` (second byte is a colon).
    if name.as_bytes().get(1) == Some(&b':') {
        return false;
    }
    if name.starts_with('.') || name.starts_with(' ') || name.ends_with('.') || name.ends_with(' ')
    {
        return false;
    }
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    !RESERVED_NAMES.contains(&stem.as_str())
}

/// The extension id doubles as a directory name, so it must be a path-safe slug.
pub(crate) fn is_safe_ext_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id != "."
        && id != ".."
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

fn validate_manifest(m: &ExtensionManifest) -> Result<(), String> {
    if m.kind != "asset-pack" {
        return Err(format!(
            "unsupported extension kind '{}' (only 'asset-pack' is allowed)",
            m.kind
        ));
    }
    if !m.permissions.is_empty() {
        return Err("asset-pack extensions must not declare any permissions".into());
    }
    if !is_safe_ext_id(&m.id) {
        return Err(format!("unsafe extension id '{}'", m.id));
    }
    for a in &m.assets {
        if !is_safe_filename(&a.filename) {
            return Err(format!("unsafe asset filename '{}'", a.filename));
        }
        if let Some(tn) = &a.thumb_filename {
            if !is_safe_filename(tn) {
                return Err(format!("unsafe thumbnail filename '{tn}'"));
            }
        }
    }
    Ok(())
}

/// Reserved seam for Ed25519 publisher signing (a fast-follow). v1 trust is
/// anchored on HTTPS + per-asset SHA256 + schema validation, so this accepts.
fn verify_signature(_manifest_bytes: &[u8], _signature: Option<&str>) -> Result<(), String> {
    Ok(())
}

// ── State persistence ───────────────────────────────────────────────────────

async fn write_state(dir: &Path, enabled: bool) {
    if let Ok(json) = serde_json::to_vec_pretty(&ExtState { enabled }) {
        let _ = crate::commands::system::write_replace_async(&dir.join("state.json"), &json).await;
    }
}

fn read_state(dir: &Path) -> ExtState {
    std::fs::read(dir.join("state.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn read_lock(dir: &Path) -> Option<ExtensionManifest> {
    let bytes = std::fs::read(dir.join("extension.lock.json")).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Build the frontend-facing record from an installed pack's directory.
fn build_installed(dir: &Path, manifest: ExtensionManifest, enabled: bool) -> InstalledExtension {
    let assets = manifest
        .assets
        .iter()
        .map(|entry| {
            let full = dir.join(&entry.filename);
            let thumb = entry
                .thumb_filename
                .as_ref()
                .map(|n| dir.join(n))
                .filter(|p| p.exists());
            ExtAssetPath {
                id: entry.id.clone(),
                path: full.exists().then(|| full.to_string_lossy().to_string()),
                thumb_path: thumb.map(|p| p.to_string_lossy().to_string()),
            }
        })
        .collect();
    InstalledExtension {
        manifest,
        enabled,
        dir: dir.to_string_lossy().to_string(),
        assets,
    }
}

// ── Commands ────────────────────────────────────────────────────────────────

/// Install (or update) a pack from its manifest URL. Validates the envelope,
/// downloads + verifies every asset, persists the lock + enabled state, and
/// returns the hydrated record.
#[tauri::command]
pub async fn install_extension(
    app: AppHandle,
    manifest_url: String,
) -> AppResult<InstalledExtension> {
    if !url_allowed(&manifest_url) {
        return Err(AppError::from(
            "manifest URL must be https (localhost allowed for dev)",
        ));
    }
    let client = http_client()?;
    let manifest_bytes = client
        .get(&manifest_url)
        .send()
        .await
        .map_err(|e| AppError::msg(format!("manifest request: {e}")))?
        .error_for_status()
        .map_err(|e| AppError::msg(format!("manifest http: {e}")))?
        .bytes()
        .await
        .map_err(|e| AppError::msg(format!("manifest read: {e}")))?;

    let manifest: ExtensionManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| AppError::msg(format!("manifest parse: {e}")))?;
    validate_manifest(&manifest)?;
    verify_signature(&manifest_bytes, manifest.signature.as_deref())?;

    let dir = extensions_dir(&app)?.join(&manifest.id);
    fs::create_dir_all(&dir)
        .await
        .map_err(|e| AppError::msg(format!("create dir: {e}")))?;

    for entry in &manifest.assets {
        if !url_allowed(&entry.url) {
            return Err(AppError::msg(format!(
                "asset '{}' url must be https (localhost allowed for dev)",
                entry.id
            )));
        }
        let target = dir.join(&entry.filename);
        ensure_one(&client, &entry.url, &entry.sha256, &target)
            .await
            .map_err(|e| AppError::msg(format!("asset '{}': {e}", entry.id)))?;

        if let (Some(tn), Some(tu), Some(th)) = (
            entry.thumb_filename.as_ref(),
            entry.thumb_url.as_ref(),
            entry.thumb_sha256.as_ref(),
        ) {
            if url_allowed(tu) {
                let _ = ensure_one(&client, tu, th, &dir.join(tn)).await;
            }
        }
    }

    let lock_path = dir.join("extension.lock.json");
    if let Ok(json) = serde_json::to_vec_pretty(&manifest) {
        crate::commands::system::write_replace_async(&lock_path, &json)
            .await
            .map_err(|e| AppError::msg(format!("write lock: {e}")))?;
    }
    write_state(&dir, true).await;

    Ok(build_installed(&dir, manifest, true))
}

/// Manifests of installed **and enabled** packs, read from disk. Used by other
/// subsystems (e.g. caption models) that merge extension contributions into
/// their own registries without going through the frontend. Disabled packs and
/// unreadable dirs are silently skipped.
pub(crate) fn enabled_manifests(app: &AppHandle) -> Vec<ExtensionManifest> {
    let Ok(base) = extensions_dir(app) else {
        return Vec::new();
    };
    let Ok(read) = std::fs::read_dir(&base) else {
        return Vec::new();
    };
    let mut out: Vec<ExtensionManifest> = read
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|dir| read_state(dir).enabled)
        .filter_map(|dir| read_lock(&dir))
        .collect();
    // Stable order so a merged catalog's ordering doesn't depend on read_dir.
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// No-network: enumerate installed packs from disk (used for startup hydration).
#[tauri::command]
pub fn list_installed_extensions(app: AppHandle) -> AppResult<Vec<InstalledExtension>> {
    let base = extensions_dir(&app)?;
    let mut out = Vec::new();
    let Ok(read) = std::fs::read_dir(&base) else {
        return Ok(out); // dir absent → nothing installed yet
    };
    for entry in read.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        if let Some(manifest) = read_lock(&dir) {
            let enabled = read_state(&dir).enabled;
            out.push(build_installed(&dir, manifest, enabled));
        }
    }
    // `read_dir` has no defined order, so sort by id to keep hydration and picker order stable.
    out.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    Ok(out)
}

/// Toggle a pack's enabled flag (without deleting its files).
#[tauri::command]
pub async fn set_extension_enabled(app: AppHandle, ext_id: String, enabled: bool) -> AppResult<()> {
    if !is_safe_ext_id(&ext_id) {
        return Err(AppError::msg(format!("unsafe extension id '{ext_id}'")));
    }
    let dir = extensions_dir(&app)?.join(&ext_id);
    if !dir.is_dir() {
        return Err(AppError::msg(format!(
            "extension '{ext_id}' is not installed"
        )));
    }
    write_state(&dir, enabled).await;
    Ok(())
}

/// Remove a pack and all its files.
#[tauri::command]
pub async fn uninstall_extension(app: AppHandle, ext_id: String) -> AppResult<()> {
    if !is_safe_ext_id(&ext_id) {
        return Err(AppError::msg(format!("unsafe extension id '{ext_id}'")));
    }
    let dir = extensions_dir(&app)?.join(&ext_id);
    if dir.is_dir() {
        fs::remove_dir_all(&dir)
            .await
            .map_err(|e| AppError::msg(format!("remove extension: {e}")))?;
    }
    Ok(())
}

/// Fetch a curated registry *index* (no install). Returns the raw JSON for the
/// frontend gallery to render. The index is expected to list packs with their
/// manifest URLs + display metadata.
#[tauri::command]
pub async fn fetch_extension_registry(index_url: String) -> AppResult<serde_json::Value> {
    if !url_allowed(&index_url) {
        return Err(AppError::from(
            "registry index URL must be https (localhost allowed for dev)",
        ));
    }
    let client = http_client()?;
    client
        .get(&index_url)
        .send()
        .await
        .map_err(|e| AppError::msg(format!("registry request: {e}")))?
        .error_for_status()
        .map_err(|e| AppError::msg(format!("registry http: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::msg(format!("registry parse: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal_and_device_names() {
        assert!(is_safe_filename("cursor.svg"));
        assert!(is_safe_filename("wallpaper-01.png"));
        assert!(!is_safe_filename("../escape.svg"));
        assert!(!is_safe_filename("a/b.svg"));
        assert!(!is_safe_filename("a\\b.svg"));
        assert!(!is_safe_filename("C:evil.svg"));
        assert!(!is_safe_filename("CON"));
        assert!(!is_safe_filename("nul.png"));
        assert!(!is_safe_filename(".hidden"));
        assert!(!is_safe_filename("trailing. "));
        assert!(!is_safe_filename(""));
    }

    #[test]
    fn ext_id_must_be_path_safe_slug() {
        assert!(is_safe_ext_id("acme.cursor-pack_1"));
        assert!(!is_safe_ext_id(""));
        assert!(!is_safe_ext_id(".."));
        assert!(!is_safe_ext_id("../x"));
        assert!(!is_safe_ext_id("has space"));
        assert!(!is_safe_ext_id(".hidden"));
    }

    #[test]
    fn only_https_or_loopback_http() {
        assert!(url_allowed("https://example.com/m.json"));
        assert!(url_allowed("http://localhost:8080/m.json"));
        assert!(url_allowed("http://127.0.0.1/m.json"));
        assert!(!url_allowed("http://evil.com/m.json"));
        assert!(!url_allowed("file:///etc/passwd"));
        assert!(!url_allowed("not a url"));
    }

    #[test]
    fn manifest_rejects_permissions_and_wrong_kind() {
        let mut m = ExtensionManifest {
            id: "pack".into(),
            name: "Pack".into(),
            version: "1.0.0".into(),
            author: None,
            kind: "asset-pack".into(),
            permissions: vec![],
            signature: None,
            contributes: empty_contributions(),
            assets: vec![],
        };
        assert!(validate_manifest(&m).is_ok());
        m.permissions = vec!["fs:read".into()];
        assert!(validate_manifest(&m).is_err());
        m.permissions = vec![];
        m.kind = "plugin".into();
        assert!(validate_manifest(&m).is_err());
    }
}
