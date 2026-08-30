use capturekit_core::{CaptureError, PixelFormat, Rect, Result};
use windows::core::Interface;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_UNKNOWN};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11Device5, ID3D11DeviceContext, ID3D11DeviceContext4,
    ID3D11Fence, ID3D11Resource, ID3D11Texture2D, D3D11_BIND_RENDER_TARGET,
    D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_FENCE_FLAG_SHARED, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_RESOURCE_MISC_SHARED,
    D3D11_RESOURCE_MISC_SHARED_NTHANDLE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{IDXGIAdapter, IDXGIResource1, DXGI_SHARED_RESOURCE_READ};

pub(crate) const BACKEND: &str = "d3d11";

pub(crate) fn err(source: windows::core::Error) -> CaptureError {
    CaptureError::backend(BACKEND, source)
}

/// A failure the API reported through a null out-parameter rather than an HRESULT.
fn missing(what: &'static str) -> CaptureError {
    CaptureError::Unsupported {
        backend: BACKEND,
        operation: what,
    }
}

/// The pixel format a DXGI surface maps onto, or `None` for one capturekit does
/// not model. Anything else would be handed out mislabelled.
pub(crate) fn pixel_format(format: DXGI_FORMAT) -> Option<PixelFormat> {
    match format {
        DXGI_FORMAT_B8G8R8A8_UNORM => Some(PixelFormat::Bgra8),
        DXGI_FORMAT_R16G16B16A16_FLOAT => Some(PixelFormat::Rgba16Float),
        _ => None,
    }
}

/// A hardware device with BGRA support, which both Desktop Duplication and the
/// Graphics Capture frame pool require.
pub(crate) fn create_device(
    adapter: Option<&IDXGIAdapter>,
) -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    let mut device = None;
    let mut context = None;
    // A device built against an explicit adapter must not also name a driver type; D3D11CreateDevice rejects the pair.
    let driver_type = if adapter.is_some() {
        D3D_DRIVER_TYPE_UNKNOWN
    } else {
        D3D_DRIVER_TYPE_HARDWARE
    };
    unsafe {
        D3D11CreateDevice(
            adapter,
            driver_type,
            None,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .map_err(err)?;
    }
    match (device, context) {
        (Some(device), Some(context)) => Ok((device, context)),
        _ => Err(missing("create a Direct3D 11 device")),
    }
}

fn staging_texture(
    device: &ID3D11Device,
    width: u32,
    height: u32,
    format: DXGI_FORMAT,
) -> Result<ID3D11Texture2D> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
    };
    let mut texture = None;
    unsafe {
        device
            .CreateTexture2D(&desc, None, Some(&mut texture))
            .map_err(err)?;
    }
    texture.ok_or_else(|| missing("create a staging texture"))
}

/// A CPU-readable copy of a GPU surface, plus the mapping that exposes it.
///
/// Holds the mapping open across the borrow of the frame it produced, so a
/// consumer reads the driver's rows in place. Nothing is repacked to a tight
/// stride: [`capturekit_core::PixelFormat::buffer_len`] already understands
/// padding, and repacking every frame is what a capture path can least afford.
pub(crate) struct Readback {
    context: ID3D11DeviceContext,
    resource: ID3D11Resource,
    height: u32,
    mapped: Option<D3D11_MAPPED_SUBRESOURCE>,
}

impl Readback {
    pub(crate) fn new(
        device: &ID3D11Device,
        context: ID3D11DeviceContext,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
    ) -> Result<Self> {
        let staging = staging_texture(device, width, height, format)?;
        let resource: ID3D11Resource = staging.cast::<ID3D11Resource>().map_err(err)?;
        Ok(Self {
            context,
            resource,
            height,
            mapped: None,
        })
    }

    /// Copy `source` into the staging texture, cropping on the GPU when `region`
    /// is set, so the pixels outside it are never read back at all.
    pub(crate) fn copy_from(
        &mut self,
        source: &ID3D11Texture2D,
        region: Option<Rect>,
    ) -> Result<()> {
        self.unmap();
        let source_resource: ID3D11Resource = source.cast::<ID3D11Resource>().map_err(err)?;
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { source.GetDesc(&mut desc) };

        let surface = Rect::from_size(desc.Width, desc.Height);
        let clipped =
            region
                .unwrap_or(surface)
                .fit_inside(&surface)
                .ok_or(CaptureError::Unsupported {
                    backend: BACKEND,
                    operation: "crop to a region outside the captured surface",
                })?;
        let source_box = D3D11_BOX {
            left: clipped.x.max(0) as u32,
            top: clipped.y.max(0) as u32,
            front: 0,
            right: clipped.right().max(0) as u32,
            bottom: clipped.bottom().max(0) as u32,
            back: 1,
        };
        unsafe {
            self.context.CopySubresourceRegion(
                &self.resource,
                0,
                0,
                0,
                0,
                &source_resource,
                0,
                Some(&source_box),
            );
        }
        Ok(())
    }

    /// The staged pixels and their row pitch, valid until the next copy or drop.
    pub(crate) fn map(&mut self) -> Result<(&[u8], u32)> {
        if self.mapped.is_none() {
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                self.context
                    .Map(&self.resource, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                    .map_err(err)?;
            }
            self.mapped = Some(mapped);
        }
        let mapped = self
            .mapped
            .ok_or_else(|| missing("map a staging texture"))?;
        let stride = mapped.RowPitch;
        let len = if mapped.DepthPitch > 0 {
            mapped.DepthPitch as usize
        } else {
            stride as usize * self.height as usize
        };
        // SAFETY: `Map` succeeded, so `pData` covers `DepthPitch` bytes, and `unmap` takes `&mut self` so it can't run under this borrow.
        let bytes = unsafe { core::slice::from_raw_parts(mapped.pData.cast::<u8>(), len) };
        Ok((bytes, stride))
    }

    pub(crate) fn unmap(&mut self) {
        if self.mapped.take().is_some() {
            unsafe { self.context.Unmap(&self.resource, 0) };
        }
    }
}

impl Drop for Readback {
    fn drop(&mut self) {
        self.unmap();
    }
}

/// `CreateSharedHandle` on a fence accepts only this access mask; `SYNCHRONIZE`
/// alone comes back `E_ACCESSDENIED`.
const GENERIC_ALL: u32 = 0x1000_0000;

/// A GPU-resident copy of the frame, plus the fence that says when it is ready.
///
/// The pixels never reach host memory: a consumer opens the texture on its own
/// device and reads it there. Cross-device sharing is NOT implicitly ordered,
/// so a consumer that samples before the fence reaches this frame's value reads
/// zeroes, with no error raised anywhere.
pub(crate) struct SharedSurface {
    texture: ID3D11Texture2D,
    context: ID3D11DeviceContext4,
    fence: ID3D11Fence,
    /// Signalled by the CONSUMER when it has finished with a frame. One texture
    /// is reused for every frame, so without this the next copy overwrites the
    /// picture while the consumer is still reading it.
    release: ID3D11Fence,
    texture_handle: isize,
    fence_handle: isize,
    release_handle: isize,
    /// Incremented per copy, so a consumer waits for its own frame.
    signalled: u64,
}

impl SharedSurface {
    pub(crate) fn new(
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
    ) -> Result<Self> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
            CPUAccessFlags: 0,
            // NT handle, no keyed mutex: D3D12 has none, so the fence orders it.
            MiscFlags: (D3D11_RESOURCE_MISC_SHARED.0 | D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0)
                as u32,
        };
        let mut texture = None;
        unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }.map_err(err)?;
        let texture = texture.ok_or_else(|| missing("create a shared texture"))?;

        let device5: ID3D11Device5 = device.cast().map_err(err)?;
        let context4: ID3D11DeviceContext4 = context.cast().map_err(err)?;
        let mut fence_slot: Option<ID3D11Fence> = None;
        unsafe { device5.CreateFence(0, D3D11_FENCE_FLAG_SHARED, &mut fence_slot) }.map_err(err)?;
        let fence = fence_slot.ok_or_else(|| missing("create a shared fence"))?;
        let mut release_slot: Option<ID3D11Fence> = None;
        unsafe { device5.CreateFence(0, D3D11_FENCE_FLAG_SHARED, &mut release_slot) }
            .map_err(err)?;
        let release = release_slot.ok_or_else(|| missing("create a release fence"))?;

        let texture_handle = unsafe {
            texture
                .cast::<IDXGIResource1>()
                .map_err(err)?
                .CreateSharedHandle(None, DXGI_SHARED_RESOURCE_READ.0, None)
        }
        .map_err(err)?;
        let fence_handle =
            unsafe { fence.CreateSharedHandle(None, GENERIC_ALL, None) }.map_err(err)?;
        let release_handle =
            unsafe { release.CreateSharedHandle(None, GENERIC_ALL, None) }.map_err(err)?;

        Ok(Self {
            texture,
            context: context4,
            fence,
            release,
            texture_handle: texture_handle.0 as isize,
            fence_handle: fence_handle.0 as isize,
            release_handle: release_handle.0 as isize,
            signalled: 0,
        })
    }

    /// Copy `source` in and signal the fence, returning the value to wait for.
    ///
    /// Queues a wait for the consumer to release the PREVIOUS frame first. This
    /// is a GPU-side wait, so the CPU does not block, but it does mean a
    /// consumer that takes a handle and never signals `release` stalls the
    /// capture. That is the contract [`GpuHandle`] states.
    pub(crate) fn copy_from(
        &mut self,
        source: &ID3D11Texture2D,
        region: Option<Rect>,
    ) -> Result<u64> {
        let source_box = clip_box(source, region)?;
        let destination: ID3D11Resource = self.texture.cast().map_err(err)?;
        let source_resource: ID3D11Resource = source.cast().map_err(err)?;
        unsafe {
            // Zero on the first copy, which a fence starting at zero passes.
            self.context
                .Wait(&self.release, self.signalled)
                .map_err(err)?;
            self.context.CopySubresourceRegion(
                &destination,
                0,
                0,
                0,
                0,
                &source_resource,
                0,
                Some(&source_box),
            );
        }
        self.signalled += 1;
        unsafe { self.context.Signal(&self.fence, self.signalled) }.map_err(err)?;
        Ok(self.signalled)
    }

    /// The texture, the producer's fence and the consumer's release fence.
    pub(crate) const fn handles(&self) -> (isize, isize, isize) {
        (self.texture_handle, self.fence_handle, self.release_handle)
    }
}

impl Drop for SharedSurface {
    fn drop(&mut self) {
        // Duplicated into the consumer, so closing ours does not invalidate theirs.
        for handle in [self.texture_handle, self.fence_handle, self.release_handle] {
            if handle != 0 {
                unsafe {
                    let _ = CloseHandle(HANDLE(handle as *mut core::ffi::c_void));
                }
            }
        }
    }
}

/// The source rectangle to copy, clipped to the surface it is taken from.
fn clip_box(source: &ID3D11Texture2D, region: Option<Rect>) -> Result<D3D11_BOX> {
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { source.GetDesc(&mut desc) };
    let surface = Rect::from_size(desc.Width, desc.Height);
    let clipped =
        region
            .unwrap_or(surface)
            .fit_inside(&surface)
            .ok_or(CaptureError::Unsupported {
                backend: BACKEND,
                operation: "crop to a region outside the captured surface",
            })?;
    Ok(D3D11_BOX {
        left: clipped.x.max(0) as u32,
        top: clipped.y.max(0) as u32,
        front: 0,
        right: clipped.right().max(0) as u32,
        bottom: clipped.bottom().max(0) as u32,
        back: 1,
    })
}
