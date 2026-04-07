use std::path::{Path, PathBuf};

use anyhow::Result;

/// Abstraction over where project files are stored.
///
/// The initial implementation is `LocalStorage` (filesystem).
/// Future implementations can add:
/// - Loom-style hosted upload
/// - Custom S3-compatible bucket
/// - Self-hosted storage
pub trait StorageBackend: Send + Sync {
    /// Store a file and return a URI or path that can be used to retrieve it.
    fn store(&self, local_path: &Path, remote_key: &str) -> Result<String>;

    /// Retrieve a file by its URI/key and return the local path.
    fn retrieve(&self, remote_key: &str, local_path: &Path) -> Result<PathBuf>;

    /// Check if a remote key exists.
    fn exists(&self, remote_key: &str) -> Result<bool>;

    /// Delete a remote key.
    fn delete(&self, remote_key: &str) -> Result<()>;

    /// Human-readable name for this backend.
    fn name(&self) -> &str;
}

/// Local filesystem storage — the default for offline-first operation.
pub struct LocalStorage {
    pub root_dir: PathBuf,
}

impl LocalStorage {
    pub fn new(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }
}

impl StorageBackend for LocalStorage {
    fn store(&self, local_path: &Path, remote_key: &str) -> Result<String> {
        let dest = self.root_dir.join(remote_key);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(local_path, &dest)?;
        Ok(dest.to_string_lossy().to_string())
    }

    fn retrieve(&self, remote_key: &str, local_path: &Path) -> Result<PathBuf> {
        let src = self.root_dir.join(remote_key);
        std::fs::copy(&src, local_path)?;
        Ok(local_path.to_path_buf())
    }

    fn exists(&self, remote_key: &str) -> Result<bool> {
        Ok(self.root_dir.join(remote_key).exists())
    }

    fn delete(&self, remote_key: &str) -> Result<()> {
        let path = self.root_dir.join(remote_key);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "local"
    }
}
