use std::path::PathBuf;

use anyhow::{anyhow, Result};

use crate::camera::CameraCaptureConfig;

/// Fallback camera session for non-Windows platforms.
/// Camera capture is not yet supported outside Windows.
pub struct PlatformCameraSession;

impl PlatformCameraSession {
    pub fn start(config: CameraCaptureConfig) -> Result<Self> {
        let _ = (&config.output_path, config.device_name.as_deref());
        Err(anyhow!("camera capture is not supported on this platform"))
    }

    pub fn stop(self) -> Result<PathBuf> {
        unreachable!()
    }
}
