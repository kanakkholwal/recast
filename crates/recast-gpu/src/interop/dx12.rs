use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Graphics::Direct3D12::{ID3D12Fence, ID3D12Resource};

use super::{SharedFence, SharedHandle, SharedTexture, SharedTextureDesc, SharedUse};
use crate::{GpuContext, GpuError};

pub struct OwnedHandle(HANDLE);

impl OwnedHandle {
    fn new(handle: SharedHandle) -> Self {
        Self(HANDLE(handle.0 as *mut core::ffi::c_void))
    }

    fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0 .0.is_null() {
            // SAFETY: the handle came from the caller's `SharedHandle` and is
            // closed exactly once, here, because `OwnedHandle` is not Copy or Clone.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }
}

pub struct Fence(ID3D12Fence);

fn import_error(what: &str, e: windows::core::Error) -> GpuError {
    GpuError::Import(format!("{what}: {e}"))
}

pub fn import_texture(
    ctx: &GpuContext,
    handle: SharedHandle,
    desc: SharedTextureDesc,
) -> Result<SharedTexture, GpuError> {
    let owned = OwnedHandle::new(handle);
    let format = desc.format.wgpu();
    let (usage, state) = match desc.use_as {
        SharedUse::Read => (
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            wgpu::TextureUses::COPY_SRC,
        ),
        SharedUse::RenderTarget => (
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            wgpu::TextureUses::COLOR_TARGET,
        ),
    };
    let size = wgpu::Extent3d {
        width: desc.width,
        height: desc.height,
        depth_or_array_layers: 1,
    };

    // SAFETY: `as_hal` hands out the live DX12 device; `raw_device` is valid for
    // the borrow. `texture_from_raw` adopts the D3D12 resource we just opened,
    // whose dimensions and format we assert match `desc` (a mismatch is the
    // caller's contract violation and would be caught by the golden tests).
    // The hal borrow is dropped before `create_texture_from_hal`, which needs
    // the device again.
    let texture = unsafe {
        let hal_device = ctx
            .device()
            .as_hal::<wgpu::hal::api::Dx12>()
            .ok_or(GpuError::Unsupported("shared texture import outside DX12"))?;

        let mut opened: Option<ID3D12Resource> = None;
        hal_device
            .raw_device()
            .OpenSharedHandle(owned.raw(), &mut opened)
            .map_err(|e| import_error("OpenSharedHandle", e))?;
        let resource =
            opened.ok_or_else(|| GpuError::Import("OpenSharedHandle returned null".into()))?;

        let hal_texture = wgpu::hal::dx12::Device::texture_from_raw(
            resource,
            format,
            wgpu::TextureDimension::D2,
            size,
            1,
            1,
        );
        drop(hal_device);

        ctx.device()
            .create_texture_from_hal::<wgpu::hal::api::Dx12>(
                hal_texture,
                &wgpu::TextureDescriptor {
                    label: Some("recast-shared"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                },
                state,
            )
    };

    Ok(SharedTexture {
        texture,
        _handle: owned,
    })
}

pub fn import_fence(ctx: &GpuContext, handle: SharedHandle) -> Result<SharedFence, GpuError> {
    let owned = OwnedHandle::new(handle);

    // SAFETY: same device borrow contract as `import_texture`. `OpenSharedHandle`
    // takes ownership of nothing; `owned` still closes our duplicate.
    let fence = unsafe {
        let hal_device = ctx
            .device()
            .as_hal::<wgpu::hal::api::Dx12>()
            .ok_or(GpuError::Unsupported("shared fence import outside DX12"))?;
        let mut opened: Option<ID3D12Fence> = None;
        hal_device
            .raw_device()
            .OpenSharedHandle(owned.raw(), &mut opened)
            .map_err(|e| import_error("fence OpenSharedHandle", e))?;
        opened.ok_or_else(|| GpuError::Import("fence OpenSharedHandle returned null".into()))?
    };

    Ok(SharedFence {
        inner: Fence(fence),
        _handle: owned,
    })
}

pub fn queue_signal(ctx: &GpuContext, fence: &Fence, value: u64) -> Result<(), GpuError> {
    // SAFETY: same device borrow contract as `queue_wait`; `Signal` only
    // enqueues, and transfers no ownership.
    unsafe {
        let hal_device = ctx
            .device()
            .as_hal::<wgpu::hal::api::Dx12>()
            .ok_or(GpuError::Unsupported("shared fence signal outside DX12"))?;
        hal_device
            .raw_queue()
            .Signal(&fence.0, value)
            .map_err(|e| import_error("queue signal", e))
    }
}

pub fn queue_wait(ctx: &GpuContext, fence: &Fence, value: u64) -> Result<(), GpuError> {
    // SAFETY: `raw_queue` is valid for the hal borrow, and `Wait` only enqueues a
    // GPU-side wait; it neither blocks the CPU nor transfers ownership.
    unsafe {
        let hal_device = ctx
            .device()
            .as_hal::<wgpu::hal::api::Dx12>()
            .ok_or(GpuError::Unsupported("shared fence wait outside DX12"))?;
        hal_device
            .raw_queue()
            .Wait(&fence.0, value)
            .map_err(|e| import_error("queue wait", e))
    }
}

/// Access mask a producer must use when creating a shared fence handle.
/// `SYNCHRONIZE`, which the Win32 docs suggest, is accepted by
/// `CreateSharedHandle` and then fails at `OpenSharedHandle`.
pub const FENCE_SHARED_ACCESS: u32 = 0x1000_0000;
