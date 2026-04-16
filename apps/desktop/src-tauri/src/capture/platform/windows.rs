use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use xcap::Monitor;

use crate::capture::CaptureSource;
use crate::recording::CaptureTarget;

pub fn create_source(target: &CaptureTarget) -> Result<Box<dyn CaptureSource>> {
    if let Ok(source) = DxgiSource::new(target) {
        return Ok(Box::new(source));
    }
    let fallback = XCapSource::new(target)?;
    Ok(Box::new(fallback))
}

// ---------------------------------------------------------------------------
// XCap fallback
// ---------------------------------------------------------------------------

struct XCapSource {
    monitor: Monitor,
    width: u32,
    height: u32,
}

impl XCapSource {
    fn new(target: &CaptureTarget) -> Result<Self> {
        let monitor = Monitor::all()?
            .into_iter()
            .find(|candidate| {
                candidate.x().ok() == Some(target.source.x)
                    && candidate.y().ok() == Some(target.source.y)
                    && candidate.width().ok() == Some(target.source.width)
                    && candidate.height().ok() == Some(target.source.height)
            })
            .context("unable to locate source monitor for fallback capture")?;

        Ok(Self {
            monitor,
            width: target.source.width,
            height: target.source.height,
        })
    }
}

// SAFETY: XCapSource contains xcap::Monitor which holds an HMONITOR (*mut c_void).
// HMONITOR is a system-wide handle that is safe to use from any thread.
unsafe impl Send for XCapSource {}

impl CaptureSource for XCapSource {
    fn capture_next(&mut self, _timeout: Duration) -> Result<Option<Vec<u8>>> {
        let image = self.monitor.capture_image()?;
        Ok(Some(image.into_raw()))
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }
}

// ---------------------------------------------------------------------------
// DXGI hardware capture
// ---------------------------------------------------------------------------

struct DxgiSource {
    duplication: ::windows::Win32::Graphics::Dxgi::IDXGIOutputDuplication,
    device_context: ::windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
    staging_texture: ::windows::Win32::Graphics::Direct3D11::ID3D11Texture2D,
    width: u32,
    height: u32,
}

impl DxgiSource {
    fn new(target: &CaptureTarget) -> Result<Self> {
        use ::windows::core::Interface;
        use ::windows::Win32::Foundation::RECT;
        use ::windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
        use ::windows::Win32::Graphics::Direct3D11::{
            D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, D3D11_CPU_ACCESS_READ,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
            D3D11_USAGE_STAGING,
        };
        use ::windows::Win32::Graphics::Dxgi::Common::{
            DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
        };
        use ::windows::Win32::Graphics::Dxgi::{
            CreateDXGIFactory1, IDXGIAdapter, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput,
            IDXGIOutput1,
        };

        let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1()? };
        let target_rect = RECT {
            left: target.source.x,
            top: target.source.y,
            right: target.source.x + target.source.width as i32,
            bottom: target.source.y + target.source.height as i32,
        };

        let mut adapter_index = 0;
        loop {
            let adapter: IDXGIAdapter1 = match unsafe { factory.EnumAdapters1(adapter_index) } {
                Ok(adapter) => adapter,
                Err(_) => break,
            };

            let adapter_base: IDXGIAdapter = adapter.cast()?;
            let mut device = None;
            let mut context = None;

            unsafe {
                D3D11CreateDevice(
                    Some(&adapter_base),
                    D3D_DRIVER_TYPE_UNKNOWN,
                    None,
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    None,
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    None,
                    Some(&mut context),
                )?;
            }

            let device: ID3D11Device = device.context("dxgi device was not created")?;
            let context: ID3D11DeviceContext =
                context.context("dxgi device context was not created")?;

            let mut output_index = 0;
            loop {
                let output: IDXGIOutput = match unsafe { adapter.EnumOutputs(output_index) } {
                    Ok(output) => output,
                    Err(_) => break,
                };
                let desc = unsafe { output.GetDesc()? };
                if desc.DesktopCoordinates.left == target_rect.left
                    && desc.DesktopCoordinates.top == target_rect.top
                    && desc.DesktopCoordinates.right == target_rect.right
                    && desc.DesktopCoordinates.bottom == target_rect.bottom
                {
                    let output1: IDXGIOutput1 = output.cast()?;
                    let duplication = unsafe { output1.DuplicateOutput(&device)? };
                    let texture_desc = D3D11_TEXTURE2D_DESC {
                        Width: target.source.width,
                        Height: target.source.height,
                        MipLevels: 1,
                        ArraySize: 1,
                        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Usage: D3D11_USAGE_STAGING,
                        BindFlags: 0,
                        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                        MiscFlags: 0,
                    };
                    let mut staging_texture = None;
                    unsafe {
                        device.CreateTexture2D(&texture_desc, None, Some(&mut staging_texture))?;
                    }
                    let staging_texture =
                        staging_texture.context("dxgi staging texture was not created")?;
                    return Ok(Self {
                        duplication,
                        device_context: context,
                        staging_texture,
                        width: target.source.width,
                        height: target.source.height,
                    });
                }
                output_index += 1;
            }

            adapter_index += 1;
        }

        Err(anyhow!("no DXGI output matched the requested display"))
    }
}

impl CaptureSource for DxgiSource {
    fn capture_next(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>> {
        use ::windows::core::Interface;
        use ::windows::Win32::Graphics::Direct3D11::{
            ID3D11Resource, ID3D11Texture2D, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ,
        };
        use ::windows::Win32::Graphics::Dxgi::{
            IDXGIResource, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
        };

        let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource = None;

        let acquire = unsafe {
            self.duplication.AcquireNextFrame(
                timeout.as_millis() as u32,
                &mut frame_info,
                &mut resource,
            )
        };

        if let Err(error) = acquire {
            if error.code() == DXGI_ERROR_WAIT_TIMEOUT {
                return Ok(None);
            }
            return Err(error.into());
        }

        let resource: IDXGIResource = resource.context("dxgi frame resource missing")?;
        let frame_texture: ID3D11Texture2D = resource.cast()?;
        let staging_resource: ID3D11Resource = self.staging_texture.cast()?;
        let frame_resource: ID3D11Resource = frame_texture.cast()?;

        unsafe {
            self.device_context
                .CopyResource(Some(&staging_resource), Some(&frame_resource));
        }

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.device_context.Map(
                Some(&staging_resource),
                0,
                D3D11_MAP_READ,
                0,
                Some(&mut mapped),
            )?;
        }

        let row_pitch = mapped.RowPitch as usize;
        let frame_stride = self.width as usize * 4;
        let mut bytes = vec![0u8; frame_stride * self.height as usize];

        unsafe {
            let source = std::slice::from_raw_parts(
                mapped.pData as *const u8,
                row_pitch * self.height as usize,
            );
            for row in 0..self.height as usize {
                let start = row * row_pitch;
                let end = start + frame_stride;
                let dest = row * frame_stride;
                bytes[dest..dest + frame_stride].copy_from_slice(&source[start..end]);
            }

            self.device_context.Unmap(Some(&staging_resource), 0);
            self.duplication.ReleaseFrame()?;
        }

        Ok(Some(bytes))
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }
}
