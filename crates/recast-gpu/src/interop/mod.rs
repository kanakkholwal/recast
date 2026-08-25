#[cfg(windows)]
mod dx12;
#[cfg(not(windows))]
mod unsupported;

#[cfg(windows)]
use dx12 as backend;
#[cfg(windows)]
pub use dx12::FENCE_SHARED_ACCESS;
#[cfg(not(windows))]
use unsupported as backend;

use crate::{GpuContext, GpuError};

/// A raw OS handle to a texture or fence shared by another API or process.
/// Carried as `isize` so the public surface never names a `windows` crate type:
/// wgpu-hal pins that version, and exposing it would make every consumer pin it
/// too (S-1 finding F1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedHandle(pub isize);

impl SharedHandle {
    pub fn is_null(self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedFormat {
    Bgra8Unorm,
    Rgba8Unorm,
    Rgba16Float,
}

impl SharedFormat {
    pub fn wgpu(self) -> wgpu::TextureFormat {
        match self {
            Self::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
            Self::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            Self::Rgba16Float => wgpu::TextureFormat::Rgba16Float,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedTextureDesc {
    pub width: u32,
    pub height: u32,
    pub format: SharedFormat,
}

impl SharedTextureDesc {
    pub fn new(width: u32, height: u32, format: SharedFormat) -> Self {
        Self {
            width,
            height,
            format,
        }
    }
}

/// A texture owned by another API, sampled in place. Dropping it closes this
/// side's duplicated handle; the producer still owns the surface.
pub struct SharedTexture {
    texture: wgpu::Texture,
    _handle: backend::OwnedHandle,
}

impl SharedTexture {
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    pub fn view(&self) -> wgpu::TextureView {
        self.texture.create_view(&Default::default())
    }
}

/// Imports a shared texture handle with no host copy.
///
/// Cross-device sharing is NOT implicitly ordered: without a fence the importing
/// device can read before the producer's writes land, and every pixel comes back
/// zero with no error. Pair this with [`SharedFence`].
pub fn import_shared_texture(
    ctx: &GpuContext,
    handle: SharedHandle,
    desc: SharedTextureDesc,
) -> Result<SharedTexture, GpuError> {
    if handle.is_null() {
        return Err(GpuError::Import("null shared handle".into()));
    }
    if desc.width == 0 || desc.height == 0 {
        return Err(GpuError::Import(format!(
            "degenerate shared texture {}x{}",
            desc.width, desc.height
        )));
    }
    backend::import_texture(ctx, handle, desc)
}

/// A fence shared with the producing device. D3D12 has no keyed-mutex support,
/// so a shared fence is the only cross-API ordering primitive available.
pub struct SharedFence {
    inner: backend::Fence,
    _handle: backend::OwnedHandle,
}

impl SharedFence {
    /// Makes this device's queue wait, GPU-side, until the producer signals
    /// `value`. Does not block the CPU.
    pub fn queue_wait(&self, ctx: &GpuContext, value: u64) -> Result<(), GpuError> {
        backend::queue_wait(ctx, &self.inner, value)
    }
}

pub fn import_shared_fence(
    ctx: &GpuContext,
    handle: SharedHandle,
) -> Result<SharedFence, GpuError> {
    if handle.is_null() {
        return Err(GpuError::Import("null fence handle".into()));
    }
    backend::import_fence(ctx, handle)
}
