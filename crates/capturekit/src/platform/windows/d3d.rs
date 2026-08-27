use capturekit_core::{CaptureError, PixelFormat, Rect, Result};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_UNKNOWN};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
    D3D11_BOX, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE,
    D3D11_MAP_READ, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::IDXGIAdapter;

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
    // A device built against an explicit adapter must not also name a driver
    // type; D3D11CreateDevice rejects the pair.
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
    format: PixelFormat,
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
        let pixel_format = pixel_format(format).ok_or(CaptureError::Unsupported {
            backend: BACKEND,
            operation: "deliver frames in this surface format",
        })?;
        let staging = staging_texture(device, width, height, format)?;
        let resource: ID3D11Resource = staging.cast::<ID3D11Resource>().map_err(err)?;
        Ok(Self {
            context,
            resource,
            height,
            format: pixel_format,
            mapped: None,
        })
    }

    pub(crate) const fn format(&self) -> PixelFormat {
        self.format
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
        let clipped = region
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
        let mapped = self.mapped.ok_or_else(|| missing("map a staging texture"))?;
        let stride = mapped.RowPitch;
        let len = if mapped.DepthPitch > 0 {
            mapped.DepthPitch as usize
        } else {
            stride as usize * self.height as usize
        };
        // SAFETY: `Map` succeeded, so `pData` points at `DepthPitch` readable
        // bytes of this subresource, and the mapping stays open until `unmap`,
        // which takes `&mut self` and so cannot run while this borrow lives.
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
