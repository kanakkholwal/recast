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

/// What the importing device will do with the surface. Sampling a foreign
/// texture and drawing into one are opposite roles, and the initial resource
/// state has to match or the first barrier is wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedUse {
    /// The other API produced the picture; we read it.
    Read,
    /// We produce the picture; the other API consumes it.
    RenderTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedTextureDesc {
    pub width: u32,
    pub height: u32,
    pub format: SharedFormat,
    pub use_as: SharedUse,
}

impl SharedTextureDesc {
    pub fn new(width: u32, height: u32, format: SharedFormat) -> Self {
        Self {
            width,
            height,
            format,
            use_as: SharedUse::Read,
        }
    }

    /// Imports the surface to draw into rather than to sample.
    pub fn as_render_target(mut self) -> Self {
        self.use_as = SharedUse::RenderTarget;
        self
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

    /// Makes this device's queue signal `value` once everything already submitted has finished. Does not block the CPU.
    /// The mirror of [`SharedFence::queue_wait`], for when we are the producer: without it the consuming API reads whatever was in the surface before, with no error anywhere.
    pub fn queue_signal(&self, ctx: &GpuContext, value: u64) -> Result<(), GpuError> {
        backend::queue_signal(ctx, &self.inner, value)
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
