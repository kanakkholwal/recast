use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use xcap::Monitor;

use crate::capture::CaptureSource;
use crate::recording::{CaptureKind, CaptureTarget};

pub fn create_source(target: &CaptureTarget) -> Result<Box<dyn CaptureSource>> {
    // Window targets use true per-window capture (Windows Graphics Capture) so a
    // maximized or overlapped app records only its own pixels — the monitor
    // duplication path below can't isolate a window. `resolve_window_target`
    // matches this on the same predicate, sizing `source` to the window.
    if target.kind == CaptureKind::Window && wgc_window_capture_supported() {
        match WgcSource::new(target) {
            Ok(source) => return Ok(Box::new(source)),
            Err(e) => log::warn!(
                "WGC window capture failed ({e:#}); falling back to window screenshot loop"
            ),
        }
        // Fallback for the rare per-window WGC init failure. Still window-sized,
        // so it stays consistent with the encoder dimensions. The monitor path
        // is NOT usable here (source is the window, not a display).
        return Ok(Box::new(WindowXCapSource::new(target)?));
    }

    if let Ok(source) = DxgiSource::new(target) {
        return Ok(Box::new(source));
    }
    let fallback = XCapSource::new(target)?;
    Ok(Box::new(fallback))
}

//
// XCap fallback
//

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

//
// DXGI hardware capture
//

struct DxgiSource {
    duplication: ::windows::Win32::Graphics::Dxgi::IDXGIOutputDuplication,
    device_context: ::windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
    staging_texture: ::windows::Win32::Graphics::Direct3D11::ID3D11Texture2D,
    width: u32,
    height: u32,
}

impl DxgiSource {
    fn new(target: &CaptureTarget) -> Result<Self> {
        use windows::core::Interface;
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
        use windows::Win32::Graphics::Direct3D11::{
            D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, D3D11_CPU_ACCESS_READ,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
            D3D11_USAGE_STAGING,
        };
        use windows::Win32::Graphics::Dxgi::Common::{
            DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
        };
        use windows::Win32::Graphics::Dxgi::{
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
        use windows::core::Interface;
        use windows::Win32::Graphics::Direct3D11::{
            ID3D11Resource, ID3D11Texture2D, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ,
        };
        use windows::Win32::Graphics::Dxgi::{
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

//
// Windows Graphics Capture (WGC): true per-window capture
//

/// True when this OS supports Windows Graphics Capture (Windows 10 2004+ and
/// Windows 11). Window targets then capture only the window's own surface —
/// following it, excluding overlapping windows and the taskbar — instead of the
/// monitor-plus-crop path, which cannot isolate a maximized window.
pub fn wgc_window_capture_supported() -> bool {
    use windows::Graphics::Capture::GraphicsCaptureSession;
    GraphicsCaptureSession::IsSupported().unwrap_or(false)
}

/// A live WGC session bound to a single window's HWND. Unlike DXGI (which
/// duplicates a whole monitor), this delivers frames of just the window's
/// surface. The pool + session are kept alive for the capture's lifetime and
/// `capture_next` polls the pool, mirroring the DXGI poll model.
struct WgcSource {
    context: windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
    frame_pool: windows::Graphics::Capture::Direct3D11CaptureFramePool,
    session: windows::Graphics::Capture::GraphicsCaptureSession,
    staging: windows::Win32::Graphics::Direct3D11::ID3D11Texture2D,
    _device: windows::Win32::Graphics::Direct3D11::ID3D11Device,
    width: u32,
    height: u32,
}

// SAFETY: every COM object is created and used only on the capture thread. The
// frame pool is created free-threaded (agile). WgcSource never actually crosses
// threads after construction; the Send bound only satisfies the trait, matching
// DxgiSource above.
unsafe impl Send for WgcSource {}

impl WgcSource {
    fn new(target: &CaptureTarget) -> Result<Self> {
        use windows::core::{factory, Interface};
        use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem};
        use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
        use windows::Graphics::DirectX::DirectXPixelFormat;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Dxgi::IDXGIDevice;
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
        use windows::Win32::System::WinRT::Direct3D11::CreateDirect3D11DeviceFromDXGIDevice;
        use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;

        // WinRT activation on this bare capture thread needs COM initialized.
        // Idempotent: S_FALSE / RPC_E_CHANGED_MODE just mean it was already set,
        // and we don't own the apartment lifetime, so ignore the result.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }

        // Even dims: libx264/NVENC require them. The encoder is configured from
        // `target.source`, which resolve already made even; clamp defensively.
        let width = (target.source.width & !1).max(2);
        let height = (target.source.height & !1).max(2);
        // xcap's window id IS the HWND: `Window::id()` returns `hwnd.0 as u32`.
        // Win32 handles are 32-bit values sign-extended to pointer width, so
        // this round-trips the pointer exactly.
        let hwnd = HWND(target.id as i32 as isize as *mut core::ffi::c_void);

        let (device, context) = create_d3d11_device()?;
        let dxgi_device: IDXGIDevice = device.cast()?;
        let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device)? };
        let d3d_device: IDirect3DDevice = inspectable.cast()?;

        let interop: IGraphicsCaptureItemInterop =
            factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        let item: GraphicsCaptureItem = unsafe { interop.CreateForWindow(hwnd)? };
        let item_size = item.Size()?;

        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &d3d_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            item_size,
        )?;
        let session = frame_pool.CreateCaptureSession(&item)?;
        // Win11: drop the yellow capture border and the hardware cursor (we
        // composite our own cursor track). Older builds lack these setters, so
        // ignore failures rather than aborting the capture.
        let _ = session.SetIsBorderRequired(false);
        let _ = session.SetIsCursorCaptureEnabled(false);
        session.StartCapture()?;
        log::info!(
            "WGC per-window capture started ({width}x{height}, hwnd {:#x})",
            target.id
        );

        let staging = create_staging_texture(&device, width, height)?;

        Ok(Self {
            context,
            frame_pool,
            session,
            staging,
            _device: device,
            width,
            height,
        })
    }

    /// Copy the frame's surface texture into the CPU-readable staging texture and
    /// read out BGRA8 rows. Clamps to the fixed output size so a frame that
    /// differs from the window bounds (DPI/border quirks) never overruns.
    fn extract(
        &mut self,
        frame: &windows::Graphics::Capture::Direct3D11CaptureFrame,
    ) -> Result<Vec<u8>> {
        use windows::core::Interface;
        use windows::Win32::Graphics::Direct3D11::{
            ID3D11Resource, ID3D11Texture2D, D3D11_BOX, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ,
            D3D11_TEXTURE2D_DESC,
        };
        use windows::Win32::System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess;

        let surface = frame.Surface()?;
        let access: IDirect3DDxgiInterfaceAccess = surface.cast()?;
        let src_texture: ID3D11Texture2D = unsafe { access.GetInterface()? };
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { src_texture.GetDesc(&mut desc) };

        let copy_w = desc.Width.min(self.width);
        let copy_h = desc.Height.min(self.height);
        let src_res: ID3D11Resource = src_texture.cast()?;
        let dst_res: ID3D11Resource = self.staging.cast()?;
        let region = D3D11_BOX {
            left: 0,
            top: 0,
            front: 0,
            right: copy_w,
            bottom: copy_h,
            back: 1,
        };
        unsafe {
            self.context
                .CopySubresourceRegion(&dst_res, 0, 0, 0, 0, &src_res, 0, Some(&region));
        }

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.context
                .Map(&dst_res, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        }

        let row_pitch = mapped.RowPitch as usize;
        let stride = self.width as usize * 4;
        let mut bytes = vec![0u8; stride * self.height as usize];
        let copy_bytes = (copy_w as usize * 4).min(stride);
        unsafe {
            let source = std::slice::from_raw_parts(
                mapped.pData as *const u8,
                row_pitch * self.height as usize,
            );
            for row in 0..copy_h as usize {
                let s = row * row_pitch;
                let d = row * stride;
                bytes[d..d + copy_bytes].copy_from_slice(&source[s..s + copy_bytes]);
            }
            self.context.Unmap(&dst_res, 0);
        }
        Ok(bytes)
    }
}

impl Drop for WgcSource {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.frame_pool.Close();
    }
}

impl CaptureSource for WgcSource {
    fn capture_next(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>> {
        // TryGetNextFrame returns Err when the pool is empty (no new frame since
        // the last pull), so treat any error as "nothing yet" and retry until
        // the timeout, then yield None — the same contract as DXGI's poll.
        let deadline = std::time::Instant::now() + timeout;
        loop {
            match self.frame_pool.TryGetNextFrame() {
                Ok(frame) => {
                    let bytes = self.extract(&frame)?;
                    let _ = frame.Close();
                    return Ok(Some(bytes));
                }
                Err(_) => {
                    if std::time::Instant::now() >= deadline {
                        return Ok(None);
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }
}

/// Create a hardware D3D11 device + immediate context with BGRA support, as
/// both the WGC frame pool and the staging copy need one.
fn create_d3d11_device() -> Result<(
    windows::Win32::Graphics::Direct3D11::ID3D11Device,
    windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
)> {
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11CreateDevice, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
    };

    let mut device = None;
    let mut context = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            windows::Win32::Foundation::HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )?;
    }
    let device = device.context("D3D11 device was not created")?;
    let context = context.context("D3D11 device context was not created")?;
    Ok((device, context))
}

fn create_staging_texture(
    device: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
    width: u32,
    height: u32,
) -> Result<windows::Win32::Graphics::Direct3D11::ID3D11Texture2D> {
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CPU_ACCESS_READ, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    };
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};

    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
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
    let mut texture = None;
    unsafe {
        device.CreateTexture2D(&desc, None, Some(&mut texture))?;
    }
    texture.context("staging texture was not created")
}

//
// xcap window-capture fallback (used only if WGC init fails on a window)
//

/// Per-frame window screenshot via xcap (WGC one-shot with a GDI PrintWindow
/// fallback). Slower than the persistent `WgcSource`, but window-sized like it,
/// so it stays consistent with the encoder. Only reached when `WgcSource::new`
/// fails on a specific window.
struct WindowXCapSource {
    window: xcap::Window,
    width: u32,
    height: u32,
}

// SAFETY: xcap::Window wraps an HWND (a system-wide handle); used only on the
// capture thread.
unsafe impl Send for WindowXCapSource {}

impl WindowXCapSource {
    fn new(target: &CaptureTarget) -> Result<Self> {
        let window = xcap::Window::all()?
            .into_iter()
            .find(|candidate| candidate.id().ok() == Some(target.id))
            .context("window target not found for capture")?;
        Ok(Self {
            window,
            width: (target.source.width & !1).max(2),
            height: (target.source.height & !1).max(2),
        })
    }
}

impl CaptureSource for WindowXCapSource {
    fn capture_next(&mut self, _timeout: Duration) -> Result<Option<Vec<u8>>> {
        // xcap returns RGBA; the pipeline wants BGRA, so swap R/B per pixel.
        let image = self.window.capture_image()?;
        let (iw, ih) = (image.width(), image.height());
        let raw = image.into_raw();
        let stride = self.width as usize * 4;
        let src_stride = iw as usize * 4;
        let copy_w = (iw.min(self.width)) as usize;
        let copy_h = (ih.min(self.height)) as usize;
        let mut bytes = vec![0u8; stride * self.height as usize];
        for row in 0..copy_h {
            for col in 0..copy_w {
                let s = row * src_stride + col * 4;
                let d = row * stride + col * 4;
                bytes[d] = raw[s + 2];
                bytes[d + 1] = raw[s + 1];
                bytes[d + 2] = raw[s];
                bytes[d + 3] = raw[s + 3];
            }
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
