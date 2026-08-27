use core::time::Duration;
use std::time::Instant;

use capturekit_core::{
    CaptureError, ColorSpace, DirtyRects, LostReason, Rect, Result, Rotation, SourceDesc,
    Timestamp, WindowId,
};
use windows::core::{factory, Interface};
use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;

use crate::backend::{RawFrame, ScreenBackend};
use crate::platform::windows::d3d::{self, Readback};
use crate::platform::OpenOptions;
use crate::shot::CursorMode;

const BACKEND: &str = "wgc";

/// Whether this build of Windows has Graphics Capture at all (10 2004+ and 11).
pub(crate) fn is_supported() -> bool {
    GraphicsCaptureSession::IsSupported().unwrap_or(false)
}

/// Per-window capture, which Desktop Duplication cannot do: a maximised or
/// overlapped window yields only its own pixels, with no other window bleeding in.
pub(crate) struct WgcSource {
    frame_pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    readback: Readback,
    desc: SourceDesc,
    region: Option<Rect>,
    /// Minimum gap between readbacks. Graphics Capture delivers on every window
    /// repaint, far above any encode rate, and each readback maps GPU memory and
    /// so stalls the GPU. Surplus frames are closed without being read.
    min_readback_gap: Duration,
    next_readback_at: Instant,
    closed: bool,
}

// SAFETY: the frame pool is created free-threaded, and every object here is used
// only from the thread that owns the source. The bound satisfies `ScreenBackend`.
unsafe impl Send for WgcSource {}

impl WgcSource {
    pub(crate) fn open(window: WindowId, opts: &OpenOptions) -> Result<Self> {
        if !is_supported() {
            return Err(CaptureError::Unsupported {
                backend: BACKEND,
                operation: "capture a window on this build of Windows",
            });
        }
        // WinRT activation needs COM on this thread. Idempotent: S_FALSE and
        // RPC_E_CHANGED_MODE both mean it was already initialised, and the
        // apartment is not ours to own.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }

        let hwnd = HWND(window.0 as isize as *mut core::ffi::c_void);
        let (device, context) = d3d::create_device(None)?;
        let dxgi_device: IDXGIDevice = device.cast::<IDXGIDevice>().map_err(d3d::err)?;
        let inspectable =
            unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device) }.map_err(d3d::err)?;
        let d3d_device: IDirect3DDevice =
            inspectable.cast::<IDirect3DDevice>().map_err(d3d::err)?;

        let interop: IGraphicsCaptureItemInterop =
            factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().map_err(d3d::err)?;
        let item: GraphicsCaptureItem =
            unsafe { interop.CreateForWindow(hwnd) }.map_err(|error: windows::core::Error| {
                match error.code().0 {
                    // The window went away between enumeration and capture.
                    code if code == windows::Win32::Foundation::E_INVALIDARG.0 => {
                        CaptureError::NotFound {
                            kind: "window",
                            id: window.0,
                        }
                    }
                    _ => d3d::err(error),
                }
            })?;
        let item_size = item.Size().map_err(d3d::err)?;
        let surface = Rect::from_size(
            item_size.Width.max(0) as u32,
            item_size.Height.max(0) as u32,
        );

        let region =
            match opts.region {
                Some(region) => Some(region.fit_inside(&surface).ok_or(
                    CaptureError::Unsupported {
                        backend: BACKEND,
                        operation: "crop to a region outside the window",
                    },
                )?),
                None => surface.fit_inside(&surface),
            };
        let staged = region.unwrap_or(surface);

        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &d3d_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            item_size,
        )
        .map_err(d3d::err)?;
        let session = frame_pool.CreateCaptureSession(&item).map_err(d3d::err)?;
        // Both are Windows 11 setters; older builds simply lack them, so a
        // failure here is not a reason to abandon the capture.
        let _ = session.SetIsBorderRequired(false);
        let _ = session.SetIsCursorCaptureEnabled(opts.cursor == CursorMode::Include);
        session.StartCapture().map_err(d3d::err)?;

        let readback = Readback::new(
            &device,
            context,
            staged.width,
            staged.height,
            DXGI_FORMAT_B8G8R8A8_UNORM,
        )?;

        Ok(Self {
            frame_pool,
            session,
            readback,
            desc: SourceDesc {
                width: staged.width,
                height: staged.height,
                format: readback_format(),
                color_space: ColorSpace::SRGB,
                rotation: Rotation::None,
                scale_factor: 1.0,
                frame_rate: opts.frame_rate(),
                backend: BACKEND,
            },
            region: opts.region.map(|_| staged),
            min_readback_gap: readback_gap(opts.frame_rate()),
            next_readback_at: Instant::now(),
            closed: false,
        })
    }

    /// The newest frame the pool holds, with the ones it supersedes closed.
    ///
    /// Closing returns a buffer to the pool, which capture needs to keep flowing,
    /// but does not map GPU memory, so draining is cheap where reading is not.
    fn newest_frame(&self) -> Option<Direct3D11CaptureFrame> {
        let mut newest: Option<Direct3D11CaptureFrame> = None;
        while let Ok(frame) = self.frame_pool.TryGetNextFrame() {
            if let Some(superseded) = newest.replace(frame) {
                let _ = superseded.Close();
            }
        }
        newest
    }
}

const fn readback_format() -> capturekit_core::PixelFormat {
    capturekit_core::PixelFormat::Bgra8
}

/// Read back slightly faster than the encode rate so a fresh frame is usually
/// waiting when the pacer ticks, without falling back to once per repaint.
fn readback_gap(frame_rate: Option<u32>) -> Duration {
    let fps = u64::from(frame_rate.unwrap_or(60).max(1));
    Duration::from_nanos(800_000_000 / fps)
}

impl Drop for WgcSource {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

impl ScreenBackend for WgcSource {
    fn describe(&self) -> &SourceDesc {
        &self.desc
    }

    fn region(&self) -> Option<Rect> {
        self.region
    }

    fn next_frame(&mut self, timeout: Duration) -> Result<RawFrame<'_>> {
        self.readback.unmap();
        let deadline = Instant::now() + timeout;
        let mut pts = Timestamp::ZERO;
        let mut got_frame = false;

        while !got_frame {
            if let Some(frame) = self.newest_frame() {
                let now = Instant::now();
                if now >= self.next_readback_at {
                    self.next_readback_at = now + self.min_readback_gap;
                    let surface = frame.Surface().map_err(d3d::err)?;
                    let access: IDirect3DDxgiInterfaceAccess = surface
                        .cast::<IDirect3DDxgiInterfaceAccess>()
                        .map_err(d3d::err)?;
                    let texture: ID3D11Texture2D =
                        unsafe { access.GetInterface() }.map_err(d3d::err)?;
                    self.readback.copy_from(&texture, self.region)?;
                    pts = frame
                        .SystemRelativeTime()
                        .map(|time| Timestamp::from_nanos(time.Duration * 100))
                        .unwrap_or(Timestamp::ZERO);
                    got_frame = true;
                }
                let _ = frame.Close();
            }
            if !got_frame {
                if Instant::now() >= deadline {
                    return Err(CaptureError::Timeout(timeout));
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        let (bytes, stride) = self.readback.map()?;
        Ok(RawFrame {
            pts,
            bytes,
            stride,
            // Graphics Capture reports no damage, so every frame is fully dirty.
            dirty: DirtyRects::unknown(),
        })
    }

    fn stop(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        self.readback.unmap();
        self.session
            .Close()
            .map_err(|error| CaptureError::Lost(loss_of(error)))?;
        let _ = self.frame_pool.Close();
        Ok(())
    }
}

fn loss_of(_error: windows::core::Error) -> LostReason {
    LostReason::WindowClosed
}
