use core::time::Duration;
use std::time::Instant;

use capturekit_core::{
    CaptureError, ColorSpace, CursorSample, CursorShape, CursorShapeKind, DirtyRects, DisplayId,
    GpuHandle, LostReason, Rect, Result, Rotation, SourceDesc, Timestamp,
};
use windows::core::{Interface, HRESULT};
use windows::Win32::Foundation::{E_ACCESSDENIED, E_INVALIDARG, RECT};
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_MODE_ROTATION, DXGI_MODE_ROTATION_ROTATE180, DXGI_MODE_ROTATION_ROTATE270,
    DXGI_MODE_ROTATION_ROTATE90,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput, IDXGIOutput1,
    IDXGIOutputDuplication, IDXGIResource, DXGI_ERROR_ACCESS_DENIED, DXGI_ERROR_ACCESS_LOST,
    DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_DEVICE_RESET, DXGI_ERROR_INVALID_CALL,
    DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_DESC, DXGI_OUTDUPL_FRAME_INFO,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_OUTDUPL_POINTER_SHAPE_INFO, DXGI_OUTDUPL_POINTER_SHAPE_TYPE_COLOR,
    DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MASKED_COLOR, DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME,
};

use crate::backend::{FrameSource, RawFrame};
use crate::platform::windows::d3d::{self, Readback};
use crate::platform::OpenOptions;

const BACKEND: &str = "dxgi";
const REACQUIRE_BACKOFF: Duration = Duration::from_millis(250);

/// Desktop Duplication revokes the session on a mode change, fullscreen-exclusive
/// entry, the UAC secure desktop, a driver reset and undock. All are recoverable
/// by rebuilding, which is why they never reach the caller as a hard failure.
fn loss_reason(code: HRESULT) -> Option<LostReason> {
    match code {
        DXGI_ERROR_ACCESS_LOST | DXGI_ERROR_ACCESS_DENIED | DXGI_ERROR_INVALID_CALL => {
            Some(LostReason::AccessLost)
        }
        DXGI_ERROR_DEVICE_REMOVED | DXGI_ERROR_DEVICE_RESET => Some(LostReason::DeviceLost),
        _ => None,
    }
}

fn rotation_of(rotation: DXGI_MODE_ROTATION) -> Rotation {
    match rotation {
        DXGI_MODE_ROTATION_ROTATE90 => Rotation::Cw90,
        DXGI_MODE_ROTATION_ROTATE180 => Rotation::Cw180,
        DXGI_MODE_ROTATION_ROTATE270 => Rotation::Cw270,
        _ => Rotation::None,
    }
}

/// Ticks per second of the clock `LastPresentTime` counts.
fn qpc_frequency() -> i64 {
    use windows::Win32::System::Performance::QueryPerformanceFrequency;
    let mut freq = 0i64;
    let _ = unsafe { QueryPerformanceFrequency(&mut freq) };
    freq
}

/// The output whose monitor handle is `display`, and the adapter driving it.
fn find_output(display: DisplayId) -> Result<(IDXGIAdapter, IDXGIOutput1, IDXGIOutput)> {
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.map_err(d3d::err)?;
    let mut adapter_index = 0;
    while let Ok(adapter) = unsafe { factory.EnumAdapters1(adapter_index) } {
        let adapter: IDXGIAdapter1 = adapter;
        let mut output_index = 0;
        while let Ok(output) = unsafe { adapter.EnumOutputs(output_index) } {
            let desc = unsafe { output.GetDesc() }.map_err(d3d::err)?;
            if desc.Monitor.0 as u64 == display.0 {
                let output1: IDXGIOutput1 = output.cast().map_err(d3d::err)?;
                let base: IDXGIAdapter = adapter.cast().map_err(d3d::err)?;
                return Ok((base, output1, output));
            }
            output_index += 1;
        }
        adapter_index += 1;
    }
    Err(CaptureError::NotFound {
        kind: "display",
        id: display.0,
    })
}

/// Whole-display and region capture through Desktop Duplication.
pub(crate) struct DxgiSource {
    duplication: IDXGIOutputDuplication,
    /// Held only to outlive the duplication, which the device owns.
    _device: ID3D11Device,
    /// `None` when the caller asked for GPU handles: the staging texture is the
    /// host copy that mode exists to avoid, and at 4K it is 33 MB of it.
    readback: Option<Readback>,
    desc: SourceDesc,
    display: DisplayId,
    region: Option<Rect>,
    dirty: DirtyRects,
    dirty_scratch: Vec<RECT>,
    /// The cursor, carried across frames because Desktop Duplication reports a
    /// position only when it moves and a shape only when it changes.
    cursor: Cursor,
    /// Present only when the caller asked for GPU handles; the readback is then
    /// skipped entirely, which is the point of the mode.
    shared: Option<d3d::SharedSurface>,
    qpc_frequency: i64,
    holding_frame: bool,
    lost_since: Option<Instant>,
    next_retry_at: Option<Instant>,
}

// SAFETY: every COM object is created and used on one thread; a `DxgiSource` moves to its capture thread before any call.
unsafe impl Send for DxgiSource {}

/// Release the mapped staging texture, where the capture has one at all.
fn unmap(readback: Option<&mut Readback>) {
    if let Some(readback) = readback {
        readback.unmap();
    }
}

impl DxgiSource {
    pub(crate) fn open(display: DisplayId, opts: &OpenOptions) -> Result<Self> {
        // Desktop Duplication never composites the cursor, so `Include` is impossible; the pointer is reported per frame instead.
        if opts.cursor == crate::shot::CursorMode::Include {
            return Err(CaptureError::Unsupported {
                backend: BACKEND,
                operation:
                    "composite the cursor into a display capture; read Frame::cursor instead",
            });
        }
        let (adapter, output1, output) = find_output(display)?;
        let (device, context) = d3d::create_device(Some(&adapter))?;
        let output_desc = unsafe { output.GetDesc() }.map_err(d3d::err)?;
        let duplication = unsafe { output1.DuplicateOutput(&device) }.map_err(|error| {
            // Duplication is one per output per process, and a second open reports only 'the parameter is incorrect'.
            match error.code() {
                E_INVALIDARG | E_ACCESSDENIED => CaptureError::AlreadyCaptured {
                    kind: "display",
                    id: display.0,
                },
                _ => d3d::err(error),
            }
        })?;

        let dupl_desc: DXGI_OUTDUPL_DESC = unsafe { duplication.GetDesc() };
        let surface = Rect::from_size(dupl_desc.ModeDesc.Width, dupl_desc.ModeDesc.Height);
        let region =
            match opts.region {
                Some(region) => Some(region.fit_inside(&surface).ok_or(
                    CaptureError::Unsupported {
                        backend: BACKEND,
                        operation: "crop to a region outside the display",
                    },
                )?),
                None => None,
            };
        let staged = region.unwrap_or(surface);

        // One or the other: GPU mode is zero-copy because no staging texture exists.
        let format =
            d3d::pixel_format(dupl_desc.ModeDesc.Format).ok_or(CaptureError::Unsupported {
                backend: BACKEND,
                operation: "deliver this display's pixel format",
            })?;
        let (readback, shared) = if opts.gpu_handles {
            let shared = d3d::SharedSurface::new(
                &device,
                &context,
                staged.width,
                staged.height,
                dupl_desc.ModeDesc.Format,
            )?;
            (None, Some(shared))
        } else {
            let readback = Readback::new(
                &device,
                context,
                staged.width,
                staged.height,
                dupl_desc.ModeDesc.Format,
            )?;
            (Some(readback), None)
        };

        let desc = SourceDesc {
            width: staged.width,
            height: staged.height,
            format,
            // The compositor hands out full-range sRGB whatever the monitor carries.
            color_space: ColorSpace::SRGB,
            rotation: rotation_of(output_desc.Rotation),
            scale_factor: 1.0,
            frame_rate: opts.frame_rate(),
            backend: BACKEND,
        };

        Ok(Self {
            duplication,
            _device: device,
            readback,
            desc,
            display,
            region,
            dirty: DirtyRects::unknown(),
            dirty_scratch: Vec::new(),
            cursor: Cursor::default(),
            shared,
            qpc_frequency: qpc_frequency(),
            holding_frame: false,
            lost_since: None,
            next_retry_at: None,
        })
    }

    fn release_frame(&mut self) {
        if self.holding_frame {
            self.holding_frame = false;
            let _ = unsafe { self.duplication.ReleaseFrame() };
        }
    }

    /// Rebuild the duplication after a recoverable loss, at most once per backoff.
    fn reacquire(&mut self, reason: LostReason) -> Result<()> {
        let now = Instant::now();
        if self.lost_since.is_none() {
            self.lost_since = Some(now);
            log::warn!("dxgi duplication lost ({reason}); reacquiring");
        }
        if self.next_retry_at.is_some_and(|at| now < at) {
            return Err(CaptureError::Lost(reason));
        }
        self.next_retry_at = Some(now + REACQUIRE_BACKOFF);

        let opts = OpenOptions {
            region: self.region,
            pacing: match self.desc.frame_rate {
                Some(fps) => capturekit_core::Pacing::Constant { fps },
                None => capturekit_core::Pacing::Passthrough,
            },
            ..OpenOptions::default()
        };
        match Self::open(self.display, &opts) {
            Ok(fresh) => {
                let held = self.lost_since.map(|at| at.elapsed()).unwrap_or_default();
                self.release_frame();
                // Replaced wholesale rather than field by field: the old value owns COM handles that its `Drop` releases.
                *self = fresh;
                log::info!("dxgi duplication reacquired after {}ms", held.as_millis());
                Ok(())
            }
            Err(err) => {
                log::debug!("dxgi reacquire failed: {err}");
                Err(CaptureError::Lost(reason))
            }
        }
    }

    /// Damage as the driver reports it, or unknown when it reports none.
    fn read_dirty(&mut self, info: &DXGI_OUTDUPL_FRAME_INFO) -> DirtyRects {
        if info.TotalMetadataBufferSize == 0 {
            return DirtyRects::unknown();
        }
        let slot = core::mem::size_of::<RECT>();
        self.dirty_scratch.clear();
        self.dirty_scratch.resize(
            info.TotalMetadataBufferSize as usize / slot + 1,
            RECT::default(),
        );
        let mut required = 0u32;
        let bytes = (self.dirty_scratch.len() * slot) as u32;
        let read = unsafe {
            self.duplication.GetFrameDirtyRects(
                bytes,
                self.dirty_scratch.as_mut_ptr(),
                &mut required,
            )
        };
        if read.is_err() {
            return DirtyRects::unknown();
        }
        let origin = self.region.unwrap_or_default();
        DirtyRects::from_rects(
            self.dirty_scratch
                .iter()
                .take(required as usize / slot)
                .map(|r| {
                    Rect::new(
                        r.left - origin.x,
                        r.top - origin.y,
                        (r.right - r.left).max(0) as u32,
                        (r.bottom - r.top).max(0) as u32,
                    )
                }),
        )
    }
}

/// Cursor state accumulated across frames, because Desktop Duplication is edge-triggered on both halves.
/// `PointerPosition` means nothing unless `LastMouseUpdateTime` is set and a shape arrives only on change, so a stateless read jumps the cursor to the origin when it holds still.
#[derive(Default)]
struct Cursor {
    position: Option<(i32, i32)>,
    visible: bool,
    shape: Option<CursorShape>,
    shape_id: u64,
    scratch: Vec<u8>,
}

impl Cursor {
    fn shape_kind(raw: DXGI_OUTDUPL_POINTER_SHAPE_INFO) -> Option<CursorShapeKind> {
        match raw.Type as i32 {
            t if t == DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME.0 => {
                Some(CursorShapeKind::Monochrome)
            }
            t if t == DXGI_OUTDUPL_POINTER_SHAPE_TYPE_COLOR.0 => Some(CursorShapeKind::Color),
            t if t == DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MASKED_COLOR.0 => {
                Some(CursorShapeKind::MaskedColor)
            }
            _ => None,
        }
    }

    /// Take in one frame's pointer metadata.
    fn update(&mut self, duplication: &IDXGIOutputDuplication, info: &DXGI_OUTDUPL_FRAME_INFO) {
        if info.LastMouseUpdateTime != 0 {
            self.visible = info.PointerPosition.Visible.as_bool();
            self.position = Some((
                info.PointerPosition.Position.x,
                info.PointerPosition.Position.y,
            ));
        }
        if info.PointerShapeBufferSize == 0 {
            return;
        }

        self.scratch.clear();
        self.scratch.resize(info.PointerShapeBufferSize as usize, 0);
        let mut required = 0u32;
        let mut shape_info = DXGI_OUTDUPL_POINTER_SHAPE_INFO::default();
        let read = unsafe {
            duplication.GetFramePointerShape(
                self.scratch.len() as u32,
                self.scratch.as_mut_ptr().cast(),
                &mut required,
                &mut shape_info,
            )
        };
        if read.is_err() {
            return;
        }
        let Some(kind) = Self::shape_kind(shape_info) else {
            return;
        };
        self.scratch.truncate(required as usize);
        self.shape = Some(CursorShape {
            width: shape_info.Width,
            height: shape_info.Height,
            stride: shape_info.Pitch,
            hotspot_x: shape_info.HotSpot.x.max(0) as u32,
            hotspot_y: shape_info.HotSpot.y.max(0) as u32,
            kind,
            bytes: core::mem::take(&mut self.scratch),
        });
        self.shape_id = self.shape_id.wrapping_add(1);
    }

    /// The sample for a frame at `pts`, in the captured surface's coordinates.
    fn sample(&self, pts: Timestamp, region: Option<Rect>) -> CursorSample {
        let origin = region.unwrap_or_default();
        CursorSample {
            pts,
            position: self.position.map(|(x, y)| (x - origin.x, y - origin.y)),
            visible: self.visible,
            // Duplication has no button state; read the source the pointer path uses.
            buttons: super::pointer::buttons(),
            shape_id: self.shape_id,
        }
    }
}

impl Drop for DxgiSource {
    fn drop(&mut self) {
        self.release_frame();
    }
}

impl FrameSource for DxgiSource {
    fn describe(&self) -> &SourceDesc {
        &self.desc
    }

    fn cursor_shape(&self) -> Option<&CursorShape> {
        self.cursor.shape.as_ref()
    }

    fn region(&self) -> Option<Rect> {
        self.region
    }

    fn next_frame(&mut self, timeout: Duration) -> Result<RawFrame<'_>> {
        unmap(self.readback.as_mut());
        self.release_frame();

        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource = None;
        let acquired = unsafe {
            self.duplication
                .AcquireNextFrame(timeout.as_millis() as u32, &mut info, &mut resource)
        };

        if let Err(error) = acquired {
            if error.code() == DXGI_ERROR_WAIT_TIMEOUT {
                return Err(CaptureError::Timeout(timeout));
            }
            return match loss_reason(error.code()) {
                Some(reason) => {
                    self.reacquire(reason)?;
                    Err(CaptureError::Timeout(timeout))
                }
                None => Err(d3d::err(error)),
            };
        }
        self.holding_frame = true;

        let resource: IDXGIResource = resource.ok_or(CaptureError::Unsupported {
            backend: BACKEND,
            operation: "deliver a frame without a surface",
        })?;
        let texture: ID3D11Texture2D = resource.cast().map_err(d3d::err)?;
        let gpu = match self.shared.as_mut() {
            Some(shared) => {
                let ready_at = shared.copy_from(&texture, self.region)?;
                let (texture_handle, fence, release) = shared.handles();
                Some(GpuHandle {
                    texture: texture_handle,
                    fence,
                    release,
                    ready_at,
                    width: self.desc.width,
                    height: self.desc.height,
                })
            }
            None => {
                let readback = self.readback.as_mut().ok_or(CaptureError::Unsupported {
                    backend: BACKEND,
                    operation: "read back a capture opened for GPU handles",
                })?;
                readback.copy_from(&texture, self.region)?;
                None
            }
        };
        self.dirty = self.read_dirty(&info);

        // A zero `LastPresentTime` is a cursor-only update, and reporting the origin keeps warmup honest for a screenshot.
        let pts = Timestamp::from_ticks(info.LastPresentTime, self.qpc_frequency);
        self.cursor.update(&self.duplication, &info);
        let dirty = self.dirty.clone();
        let cursor = Some(self.cursor.sample(pts, self.region));
        let (bytes, stride) = match self.readback.as_mut() {
            Some(readback) => readback.map()?,
            None => (&[][..], 0),
        };
        Ok(RawFrame {
            pts,
            bytes,
            stride,
            dirty,
            cursor,
            gpu,
        })
    }

    fn stop(&mut self) -> Result<()> {
        unmap(self.readback.as_mut());
        self.release_frame();
        Ok(())
    }
}
