#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
mod fallback;

use anyhow::Result;

use super::CaptureSource;
use crate::recording::CaptureTarget;

pub fn create_source(target: &CaptureTarget) -> Result<Box<dyn CaptureSource>> {
    #[cfg(windows)]
    {
        windows::create_source(target)
    }
    #[cfg(not(windows))]
    {
        fallback::create_source(target)
    }
}
