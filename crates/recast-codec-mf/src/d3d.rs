use windows::core::Interface;
use windows::Win32::Foundation::{
    CloseHandle, DuplicateHandle, DUPLICATE_SAME_ACCESS, GENERIC_ALL, HANDLE, HMODULE, TRUE,
};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_NV12, DXGI_RATIONAL, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::IDXGIResource1;
use windows::Win32::Media::MediaFoundation::{IMFDXGIDeviceManager, MFCreateDXGIDeviceManager};
use windows::Win32::System::Threading::GetCurrentProcess;

use crate::encoder::EncodeError;

/// Full-range RGB in.
///
/// Two fields say this and the driver reads the OLDER one: `RGB_Range` is bit 1,
/// where 0 is full, and `Nominal_Range` is bits 4 and 5, where 2 is 0-255.
/// Setting only the newer field leaves the behaviour resting on bit 1 happening
/// to default to the value we want, so both are stated.
#[allow(clippy::identity_op)] // The zero IS the statement: bit 1 is full-range RGB.
const RGB_FULL: u32 = (0 << 1) | (2 << 4);
/// BT.709 studio-range YCbCr out: `YCbCr_Matrix` is bit 2, and nominal range 1
/// is 16-235. This is what an H.264 stream is read as unless it says otherwise,
/// so writing full-range luma here would come back washed out everywhere.
const YCBCR_709_LIMITED: u32 = (1 << 2) | (1 << 4);

/// A D3D11 device the encoder shares, plus the video processor that turns the
/// compositor's BGRA into the NV12 an H.264 transform wants.
///
/// One device for both, because a texture cannot cross devices without a shared
/// handle and the whole point here is to not copy.
pub struct D3dContext {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    manager: IMFDXGIDeviceManager,
    video: ID3D11VideoDevice,
    video_context: ID3D11VideoContext,
}

/// A BGRA render target that another API can open by handle. The compositor
/// draws into it; the encoder reads it without it ever reaching system memory.
pub struct SharedSurface {
    texture: ID3D11Texture2D,
    handle: HANDLE,
    width: u32,
    height: u32,
}

impl SharedSurface {
    /// An NT handle for the other API to take ownership of. Duplicated, because
    /// this surface closes its own; an importer given the original would close
    /// it a second time.
    pub fn duplicate_handle(&self) -> Result<isize, EncodeError> {
        duplicate(self.handle)
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn texture(&self) -> &ID3D11Texture2D {
        &self.texture
    }
}

impl Drop for SharedSurface {
    fn drop(&mut self) {
        if !self.handle.0.is_null() {
            // SAFETY: created by `CreateSharedHandle` here and closed once,
            // because `SharedSurface` is neither Copy nor Clone.
            let _ = unsafe { CloseHandle(self.handle) };
        }
    }
}

/// A fence the producing API signals and this device waits on.
///
/// D3D12, which is the backend wgpu uses here, has no keyed-mutex support, so a
/// shared fence is the only ordering primitive available between the two.
pub struct SyncFence {
    fence: ID3D11Fence,
    handle: HANDLE,
}

impl SyncFence {
    /// An NT handle for the producing API to take ownership of. Duplicated, for
    /// the same reason as [`SharedSurface::duplicate_handle`].
    pub fn duplicate_handle(&self) -> Result<isize, EncodeError> {
        duplicate(self.handle)
    }
}

fn duplicate(handle: HANDLE) -> Result<isize, EncodeError> {
    let mut copy = HANDLE::default();
    // SAFETY: `handle` is ours and stays open; the duplicate belongs to the
    // caller, who closes it.
    unsafe {
        let process = GetCurrentProcess();
        DuplicateHandle(
            process,
            handle,
            process,
            &mut copy,
            0,
            false,
            DUPLICATE_SAME_ACCESS,
        )?;
    }
    Ok(copy.0 as isize)
}

impl Drop for SyncFence {
    fn drop(&mut self) {
        if !self.handle.0.is_null() {
            // SAFETY: created here and closed once; `SyncFence` is not Copy.
            let _ = unsafe { CloseHandle(self.handle) };
        }
    }
}

/// The video processor that turns BGRA into NV12, and the frames it fills.
///
/// The processor is created once; the frames are not, because a hardware
/// encoder is ASYNCHRONOUS and holds a frame after `ProcessInput` returns.
/// Converting into one surface over and over hands it a picture that the next
/// conversion has already overwritten.
pub struct Nv12Converter {
    device: ID3D11Device,
    processor: ID3D11VideoProcessor,
    enumerator: ID3D11VideoProcessorEnumerator,
    width: u32,
    height: u32,
}

impl Nv12Converter {
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// A fresh surface for one frame.
    ///
    /// Allocated per frame rather than pooled: the sample handed to the encoder
    /// holds a reference to the texture, so COM keeps it alive exactly as long
    /// as the encoder needs it and frees it the moment it does not. Reusing one
    /// would need a way to observe that release, which the API does not offer.
    pub fn frame(&self) -> Result<Nv12Frame, EncodeError> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: self.width,
            Height: self.height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_NV12,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            // The video processor writes through a render-target view, and the
            // encoder reads it as a shader resource.
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut texture = None;
        // SAFETY: the description is fully initialised and the out parameter is
        // checked below.
        unsafe {
            self.device
                .CreateTexture2D(&desc, None, Some(&mut texture))?
        };
        let texture = texture.ok_or_else(|| EncodeError::Media(missing("NV12 texture")))?;
        Ok(Nv12Frame { texture })
    }
}

/// One frame's worth of NV12, owned until the encoder is done with it.
pub struct Nv12Frame {
    texture: ID3D11Texture2D,
}

impl Nv12Frame {
    pub(crate) fn texture(&self) -> &ID3D11Texture2D {
        &self.texture
    }
}

impl D3dContext {
    pub fn new() -> Result<Self, EncodeError> {
        let mut device = None;
        let mut context = None;
        // SAFETY: out parameters are freshly declared and checked below.
        // VIDEO_SUPPORT is what makes `ID3D11VideoDevice` available at all;
        // BGRA_SUPPORT is what lets the compositor's format be a render target.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
        }
        let device = device.ok_or_else(|| EncodeError::Media(missing("D3D11 device")))?;
        let context = context.ok_or_else(|| EncodeError::Media(missing("D3D11 context")))?;

        // Media Foundation calls into this device from its own threads. Without
        // multithread protection that is a data race the driver will not report.
        let multithread: ID3D11Multithread = context.cast()?;
        // SAFETY: setting a flag on the device's own context. The return is
        // the PREVIOUS setting, not a status.
        let _ = unsafe { multithread.SetMultithreadProtected(true) };

        let mut token = 0u32;
        let mut manager = None;
        // SAFETY: out parameters are freshly declared and checked below.
        unsafe { MFCreateDXGIDeviceManager(&mut token, &mut manager)? };
        let manager: IMFDXGIDeviceManager =
            manager.ok_or_else(|| EncodeError::Media(missing("DXGI device manager")))?;
        // SAFETY: the token is the one the manager just handed out.
        unsafe { manager.ResetDevice(&device, token)? };

        let video: ID3D11VideoDevice = device.cast()?;
        let video_context: ID3D11VideoContext = context.cast()?;
        Ok(Self {
            device,
            context,
            manager,
            video,
            video_context,
        })
    }

    pub(crate) fn manager(&self) -> &IMFDXGIDeviceManager {
        &self.manager
    }

    /// A BGRA surface the compositor can render into and another API can open.
    pub fn shared_surface(&self, width: u32, height: u32) -> Result<SharedSurface, EncodeError> {
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
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            CPUAccessFlags: 0,
            // NTHANDLE is what makes the handle openable by D3D12, which is the
            // backend wgpu uses here. The plain SHARED flag alone is D3D11 only.
            MiscFlags: (D3D11_RESOURCE_MISC_SHARED.0 | D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0)
                as u32,
        };
        let mut texture = None;
        // SAFETY: the description is fully initialised and the out parameter is
        // checked; no initial data means an undefined but allocated surface.
        unsafe {
            self.device
                .CreateTexture2D(&desc, None, Some(&mut texture))?
        };
        let texture = texture.ok_or_else(|| EncodeError::Media(missing("shared texture")))?;

        let resource: IDXGIResource1 = texture.cast()?;
        // SAFETY: the resource was created with the sharing flags this needs.
        let handle = unsafe { resource.CreateSharedHandle(None, GENERIC_ALL.0, None)? };
        Ok(SharedSurface {
            texture,
            handle,
            width,
            height,
        })
    }

    /// The converter the encoder's frames come from.
    ///
    /// The dimensions are the encoder's, and both are even: NV12 carries chroma
    /// at half resolution in each direction and has nowhere to put an odd row.
    pub fn nv12_converter(
        &self,
        width: u32,
        height: u32,
        frame_rate: (u32, u32),
    ) -> Result<Nv12Converter, EncodeError> {
        let (width, height) = (width & !1, height & !1);
        let rate = DXGI_RATIONAL {
            Numerator: frame_rate.0.max(1),
            Denominator: frame_rate.1.max(1),
        };
        let content = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
            InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
            InputFrameRate: rate,
            InputWidth: width,
            InputHeight: height,
            OutputFrameRate: rate,
            OutputWidth: width,
            OutputHeight: height,
            Usage: D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
        };
        // SAFETY: the description is fully initialised; both calls return owned
        // interfaces.
        let (enumerator, processor) = unsafe {
            let enumerator = self.video.CreateVideoProcessorEnumerator(&content)?;
            let processor = self.video.CreateVideoProcessor(&enumerator, 0)?;
            (enumerator, processor)
        };

        // SAFETY: configuring the processor we just created. Both colour spaces
        // are set explicitly: the default guess differs by driver, which is the
        // classic source of an export that is fine on one machine and washed
        // out on another.
        unsafe {
            self.video_context.VideoProcessorSetStreamColorSpace(
                &processor,
                0,
                &D3D11_VIDEO_PROCESSOR_COLOR_SPACE {
                    _bitfield: RGB_FULL,
                },
            );
            self.video_context.VideoProcessorSetOutputColorSpace(
                &processor,
                &D3D11_VIDEO_PROCESSOR_COLOR_SPACE {
                    _bitfield: YCBCR_709_LIMITED,
                },
            );
        }

        Ok(Nv12Converter {
            device: self.device.clone(),
            processor,
            enumerator,
            width,
            height,
        })
    }

    /// A fence for the producing API to signal when its drawing has landed.
    pub fn shared_fence(&self) -> Result<SyncFence, EncodeError> {
        let device: ID3D11Device5 = self.device.cast()?;
        let mut created = None;
        // SAFETY: creating a fence on our own device, out parameter checked
        // below. SHARED is what lets the other API open it at all.
        unsafe { device.CreateFence(0, D3D11_FENCE_FLAG_SHARED, &mut created)? };
        let fence: ID3D11Fence = created.ok_or_else(|| EncodeError::Media(missing("fence")))?;
        // SAFETY: the fence was created with the sharing flag this needs.
        // GENERIC_ALL rather than SYNCHRONIZE: the narrower right is accepted
        // here and then rejected at the other end's OpenSharedHandle.
        let handle = unsafe { fence.CreateSharedHandle(None, GENERIC_ALL.0, None)? };
        Ok(SyncFence { fence, handle })
    }

    /// Holds this device's work until the producer signals `value`. GPU-side:
    /// the CPU does not block.
    ///
    /// Without it the conversion below reads whatever was in the surface before
    /// the producer drew, and every pixel comes back wrong with no error.
    pub fn wait_for(&self, fence: &SyncFence, value: u64) -> Result<(), EncodeError> {
        let context: ID3D11DeviceContext4 = self.context.cast()?;
        // SAFETY: enqueueing a wait on our own context.
        unsafe { context.Wait(&fence.fence, value) }?;
        Ok(())
    }

    /// Signals `value` once this device's queued work has run, and hands the
    /// queue to the driver.
    ///
    /// The other half of the handshake. `wait_for` orders our read AFTER the
    /// producer's draw; this orders the producer's NEXT draw after our read.
    /// One shared surface with only the first half is a race: the producer
    /// overwrites the picture before the conversion has taken it.
    pub fn signal(&self, fence: &SyncFence, value: u64) -> Result<(), EncodeError> {
        let context: ID3D11DeviceContext4 = self.context.cast()?;
        // SAFETY: enqueueing a signal on our own context, then flushing.
        //
        // The flush is contract, not something our tests prove: a signal sitting
        // in an unsubmitted command buffer is a fence the other API waits on
        // forever. D3D11 flushes on its own eventually, which is why removing
        // this still passes here and is no reason to rely on it.
        unsafe {
            context.Signal(&fence.fence, value)?;
            context.Flush();
        }
        Ok(())
    }

    /// Converts a BGRA surface into an NV12 frame, entirely on the GPU.
    pub fn convert(
        &self,
        source: &SharedSurface,
        converter: &Nv12Converter,
        target: &Nv12Frame,
    ) -> Result<(), EncodeError> {
        let source = &source.texture;
        let input_desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
            FourCC: 0,
            ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_VPIV {
                    MipSlice: 0,
                    ArraySlice: 0,
                },
            },
        };
        let output_desc = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
            ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_VPOV { MipSlice: 0 },
            },
        };

        // SAFETY: both views are created against the enumerator the processor
        // came from, which is what makes the blt below legal. The stream struct
        // hands ownership of the input view to `ManuallyDrop`, and it is taken
        // back before returning so the view is released exactly once.
        unsafe {
            let mut input = None;
            self.video.CreateVideoProcessorInputView(
                source,
                &converter.enumerator,
                &input_desc,
                Some(&mut input),
            )?;
            let mut output = None;
            self.video.CreateVideoProcessorOutputView(
                &target.texture,
                &converter.enumerator,
                &output_desc,
                Some(&mut output),
            )?;

            let mut stream = D3D11_VIDEO_PROCESSOR_STREAM {
                Enable: TRUE,
                OutputIndex: 0,
                InputFrameOrField: 0,
                PastFrames: 0,
                FutureFrames: 0,
                ppPastSurfaces: std::ptr::null_mut(),
                pInputSurface: std::mem::ManuallyDrop::new(input),
                ppFutureSurfaces: std::ptr::null_mut(),
                ppPastSurfacesRight: std::ptr::null_mut(),
                pInputSurfaceRight: std::mem::ManuallyDrop::new(None),
                ppFutureSurfacesRight: std::ptr::null_mut(),
            };
            let result = self.video_context.VideoProcessorBlt(
                &converter.processor,
                output.as_ref(),
                0,
                std::slice::from_ref(&stream),
            );
            drop(std::mem::ManuallyDrop::take(&mut stream.pInputSurface));
            drop(std::mem::ManuallyDrop::take(&mut stream.pInputSurfaceRight));
            result?;
        }
        Ok(())
    }
}

fn missing(what: &str) -> windows::core::Error {
    windows::core::Error::new(windows::Win32::Foundation::E_FAIL, what)
}
