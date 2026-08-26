use windows::core::Interface;
use windows::Win32::Foundation::{CloseHandle, GENERIC_ALL, HANDLE, HMODULE, TRUE};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_NV12, DXGI_RATIONAL, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::IDXGIResource1;
use windows::Win32::Media::MediaFoundation::{IMFDXGIDeviceManager, MFCreateDXGIDeviceManager};

use crate::encoder::EncodeError;

/// Full-range RGB in. `Nominal_Range` is bits 4 and 5, and 2 is 0-255.
const RGB_FULL: u32 = 2 << 4;
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
    /// The NT handle to hand to the other API. Borrowed: this surface still
    /// closes it, so the importer must duplicate rather than take it.
    pub fn handle(&self) -> isize {
        self.handle.0 as isize
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

/// The NV12 surface the encoder is fed. Kept alongside the processor that
/// filled it, since the two are created against the same content description.
pub struct Nv12Surface {
    texture: ID3D11Texture2D,
    processor: ID3D11VideoProcessor,
    enumerator: ID3D11VideoProcessorEnumerator,
    width: u32,
    height: u32,
}

impl Nv12Surface {
    pub fn texture(&self) -> &ID3D11Texture2D {
        &self.texture
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
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
        // SAFETY: setting a flag on the device's own context.
        unsafe { multithread.SetMultithreadProtected(true) };

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
        unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut texture))? };
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

    /// An NV12 surface plus the video processor that converts into it.
    ///
    /// The dimensions are the encoder's, and both are even: NV12 carries chroma
    /// at half resolution in each direction and has nowhere to put an odd row.
    pub fn nv12_surface(
        &self,
        width: u32,
        height: u32,
        frame_rate: (u32, u32),
    ) -> Result<Nv12Surface, EncodeError> {
        let (width, height) = (width & !1, height & !1);
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
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
        // SAFETY: as in `shared_surface`.
        unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut texture))? };
        let texture = texture.ok_or_else(|| EncodeError::Media(missing("NV12 texture")))?;

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

        Ok(Nv12Surface {
            texture,
            processor,
            enumerator,
            width,
            height,
        })
    }

    /// Converts a BGRA surface into an NV12 one, entirely on the GPU.
    pub fn convert(
        &self,
        source: &ID3D11Texture2D,
        target: &Nv12Surface,
    ) -> Result<(), EncodeError> {
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
                &target.enumerator,
                &input_desc,
                Some(&mut input),
            )?;
            let mut output = None;
            self.video.CreateVideoProcessorOutputView(
                &target.texture,
                &target.enumerator,
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
                &target.processor,
                output.as_ref(),
                0,
                std::slice::from_ref(&stream),
            );
            drop(std::mem::ManuallyDrop::take(&mut stream.pInputSurface));
            drop(std::mem::ManuallyDrop::take(&mut stream.pInputSurfaceRight));
            result?;
            // The encoder reads on its own threads, so the blt has to be handed
            // to the driver before the sample is queued rather than sitting in
            // the deferred command list.
            self.context.Flush();
        }
        Ok(())
    }
}

fn missing(what: &str) -> windows::core::Error {
    windows::core::Error::new(windows::Win32::Foundation::E_FAIL, what.to_string())
}
