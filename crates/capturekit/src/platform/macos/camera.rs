use core::time::Duration;
use std::sync::Arc;

use capturekit_core::{
    Camera, CameraFormat, CameraId, CaptureError, ColorSpace, DirtyRects, PixelFormat, Rect,
    Result, Rotation, SourceDesc,
};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{define_class, msg_send, AllocAnyThread, DefinedClass};
use objc2_av_foundation::{
    AVCaptureConnection, AVCaptureDevice, AVCaptureDeviceDiscoverySession, AVCaptureDeviceFormat,
    AVCaptureDeviceInput, AVCaptureDevicePosition, AVCaptureDeviceType,
    AVCaptureDeviceTypeBuiltInWideAngleCamera, AVCaptureDeviceTypeExternal, AVCaptureOutput,
    AVCaptureSession, AVCaptureVideoDataOutput, AVCaptureVideoDataOutputSampleBufferDelegate,
    AVMediaTypeVideo,
};
use objc2_core_media::{CMSampleBuffer, CMVideoFormatDescriptionGetDimensions};
use objc2_core_video::kCVPixelBufferPixelFormatTypeKey;
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSObject, NSObjectProtocol, NSString};

use crate::backend::{FrameSource, RawFrame};
use crate::deliver::FrameSlot;
use crate::platform::macos::sample::{accept_video, BGRA};
use crate::platform::OpenOptions;

pub(super) const BACKEND: &str = "avfoundation";

/// What the session is asked for when the caller names no size.
const DEFAULT_SIZE: (u32, u32) = (1280, 720);
/// How long to wait for the first frame before deciding the device is wedged.
const FIRST_FRAME: Duration = Duration::from_secs(5);

fn unsupported(operation: &'static str) -> CaptureError {
    CaptureError::Unsupported {
        backend: BACKEND,
        operation,
    }
}

fn failed(message: String) -> CaptureError {
    CaptureError::backend(BACKEND, std::io::Error::other(message))
}

/// `AVMediaTypeVideo`, which the framework declares as a nullable global.
fn video_media_type() -> Option<&'static NSString> {
    unsafe { AVMediaTypeVideo }
}

/// Every video capture device AVFoundation offers.
///
/// A discovery session rather than the deprecated `devicesWithMediaType`, which
/// stops reporting external cameras on recent systems.
fn discover() -> Retained<NSArray<AVCaptureDevice>> {
    let mut kinds = Vec::with_capacity(2);
    // Read as raw pointers, never as the `&'static` the binding declares.
    // `AVCaptureDeviceTypeExternal` is macOS 14 and weak-linked, so on macOS 13
    // the symbol resolves to null and taking a reference to it is undefined.
    for slot in [
        core::ptr::addr_of!(AVCaptureDeviceTypeBuiltInWideAngleCamera),
        core::ptr::addr_of!(AVCaptureDeviceTypeExternal),
    ] {
        // SAFETY: reading the pointer the symbol holds, without forming a
        // reference to whatever it points at until it is known non-null.
        let raw = unsafe { slot.cast::<*const AVCaptureDeviceType>().read() };
        if !raw.is_null() {
            kinds.push(unsafe { &*raw });
        }
    }
    let types = NSArray::from_slice(&kinds);
    let session = unsafe {
        AVCaptureDeviceDiscoverySession::discoverySessionWithDeviceTypes_mediaType_position(
            &types,
            video_media_type(),
            AVCaptureDevicePosition::Unspecified,
        )
    };
    unsafe { session.devices() }
}

/// The modes one device advertises, largest first.
///
/// Reported as what capturekit delivers rather than as the device's own
/// subtype: the session converts every format to BGRA on the way out.
fn modes(device: &AVCaptureDevice) -> Vec<CameraFormat> {
    let mut modes: Vec<CameraFormat> = Vec::new();
    for format in unsafe { device.formats() }.iter() {
        let Some(mode) = mode_of(&format) else {
            continue;
        };
        if !modes.contains(&mode) {
            modes.push(mode);
        }
    }
    modes.sort_by(|a, b| {
        b.area().cmp(&a.area()).then(
            b.frame_rate
                .unwrap_or_default()
                .total_cmp(&a.frame_rate.unwrap_or_default()),
        )
    });
    modes
}

fn mode_of(format: &AVCaptureDeviceFormat) -> Option<CameraFormat> {
    let description = unsafe { format.formatDescription() };
    let size = unsafe { CMVideoFormatDescriptionGetDimensions(&description) };
    let width = u32::try_from(size.width).ok()?;
    let height = u32::try_from(size.height).ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    let frame_rate = unsafe { format.videoSupportedFrameRateRanges() }
        .iter()
        .map(|range| unsafe { range.maxFrameRate() })
        .fold(None::<f64>, |best, rate| {
            Some(best.map_or(rate, |best: f64| best.max(rate)))
        });
    Some(CameraFormat {
        width,
        height,
        pixel_format: PixelFormat::Bgra8,
        frame_rate: frame_rate.map(|rate| rate as f32),
    })
}

pub(crate) fn cameras() -> Result<Vec<Camera>> {
    let devices = discover();
    let default = video_media_type()
        .and_then(|media| unsafe { AVCaptureDevice::defaultDeviceWithMediaType(media) })
        .map(|device| unsafe { device.uniqueID() }.to_string());
    Ok(devices
        .iter()
        .map(|device| {
            let id = unsafe { device.uniqueID() }.to_string();
            Camera {
                is_default: default.as_ref() == Some(&id),
                name: unsafe { device.localizedName() }.to_string(),
                formats: modes(&device),
                id: CameraId(id),
            }
        })
        .collect())
}

fn device_by_id(id: &CameraId) -> Result<Retained<AVCaptureDevice>> {
    discover()
        .iter()
        .find(|device| unsafe { device.uniqueID() }.to_string() == id.0)
        .ok_or_else(|| CaptureError::NotFoundNamed {
            kind: "camera",
            id: id.0.clone(),
        })
}

/// Pin the device to the mode closest to `size` without exceeding it.
///
/// Left alone when nothing fits, so the session keeps whatever the device came
/// up in rather than being forced into a mode the caller never asked for.
fn pin_format(device: &AVCaptureDevice, size: (u32, u32)) {
    let formats = unsafe { device.formats() };
    let chosen = formats
        .iter()
        .filter_map(|format| mode_of(&format).map(|mode| (format, mode)))
        .filter(|(_, mode)| mode.width <= size.0 && mode.height <= size.1)
        .max_by_key(|(_, mode)| mode.area());
    let Some((format, _)) = chosen else {
        return;
    };
    if unsafe { device.lockForConfiguration() }.is_err() {
        return;
    }
    unsafe { device.setActiveFormat(&format) };
    unsafe { device.unlockForConfiguration() };
}

/// Ask the output for BGRA rather than the device's native `420v`, so every
/// source in this crate delivers one pixel format.
///
/// The key is `kCVPixelBufferPixelFormatTypeKey`, a `CFString` that is toll-free
/// bridged to the `NSString` this dictionary wants.
fn bgra_settings() -> Retained<NSDictionary<NSString, AnyObject>> {
    let key: &NSString = unsafe { &*core::ptr::from_ref(kCVPixelBufferPixelFormatTypeKey).cast() };
    let value = NSNumber::new_u32(BGRA);
    NSDictionary::from_slices(&[key], &[&*value as &AnyObject])
}

define_class!(
    #[unsafe(super(NSObject))]
    #[ivars = Arc<FrameSlot>]
    struct CameraOutput;

    unsafe impl NSObjectProtocol for CameraOutput {}

    unsafe impl AVCaptureVideoDataOutputSampleBufferDelegate for CameraOutput {
        #[unsafe(method(captureOutput:didOutputSampleBuffer:fromConnection:))]
        fn did_output(
            &self,
            _output: &AVCaptureOutput,
            sample: &CMSampleBuffer,
            _connection: &AVCaptureConnection,
        ) {
            accept_video(self.ivars(), sample);
        }

        /// A dropped frame is the device outrunning the consumer, which the slot
        /// already handles by keeping only the newest. Worth counting, not worth
        /// failing.
        #[unsafe(method(captureOutput:didDropSampleBuffer:fromConnection:))]
        fn did_drop(
            &self,
            _output: &AVCaptureOutput,
            _sample: &CMSampleBuffer,
            _connection: &AVCaptureConnection,
        ) {
            log::trace!("avfoundation dropped a late frame");
        }
    }
);

impl CameraOutput {
    fn new(slot: Arc<FrameSlot>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(slot);
        unsafe { msg_send![super(this), init] }
    }
}

/// A camera stream through an `AVCaptureSession`.
pub(crate) struct AvfCameraSource {
    session: Retained<AVCaptureSession>,
    _output: Retained<AVCaptureVideoDataOutput>,
    _delegate: Retained<CameraOutput>,
    slot: Arc<FrameSlot>,
    desc: SourceDesc,
    current: Vec<u8>,
    seen: u64,
    stopped: bool,
}

// SAFETY: `AVCaptureSession` and its output are thread-safe Objective-C objects
// and the slot is explicitly synchronised. The bound satisfies `FrameSource`.
unsafe impl Send for AvfCameraSource {}

impl AvfCameraSource {
    pub(crate) fn open(id: &CameraId, opts: &OpenOptions) -> Result<Self> {
        let device = device_by_id(id)?;
        let size = opts
            .region
            .map_or(DEFAULT_SIZE, |region| (region.width, region.height));
        pin_format(&device, size);

        let input = unsafe {
            AVCaptureDeviceInput::initWithDevice_error(AVCaptureDeviceInput::alloc(), &device)
        }
        .map_err(|error| failed(error.localizedDescription().to_string()))?;

        let session = unsafe { AVCaptureSession::new() };
        if !unsafe { session.canAddInput(&input) } {
            return Err(unsupported("open a camera another session already holds"));
        }
        unsafe { session.addInput(&input) };

        let slot = Arc::new(FrameSlot::default());
        let delegate = CameraOutput::new(Arc::clone(&slot));
        let output = unsafe { AVCaptureVideoDataOutput::new() };
        unsafe {
            // BGRA rather than the device's native '420v', so every source in
            // this crate delivers one pixel format.
            output.setVideoSettings(Some(&bgra_settings()));
            // The slot keeps only the newest buffer, so holding late ones back
            // would add latency and nothing else.
            output.setAlwaysDiscardsLateVideoFrames(true);
        }
        if !unsafe { session.canAddOutput(&output) } {
            return Err(unsupported("read frames from this camera"));
        }
        unsafe { session.addOutput(&output) };

        // Serial: buffers must reach the slot in the order the device produced
        // them, and a concurrent queue would let two deliveries race the swap.
        let queue = dispatch2::DispatchQueue::new(
            "com.capturekit.camera",
            dispatch2::DispatchQueueAttr::SERIAL,
        );
        unsafe {
            output.setSampleBufferDelegate_queue(
                Some(ProtocolObject::from_ref(&*delegate)),
                Some(&queue),
            );
            session.startRunning();
        }

        let mut source = Self {
            session,
            _output: output,
            _delegate: delegate,
            slot,
            desc: SourceDesc {
                width: size.0,
                height: size.1,
                format: PixelFormat::Bgra8,
                // AVFoundation converts to full-range sRGB on the way to BGRA.
                color_space: ColorSpace::SRGB,
                rotation: Rotation::None,
                scale_factor: 1.0,
                frame_rate: opts.frame_rate(),
                backend: BACKEND,
            },
            current: Vec::new(),
            seen: 0,
            stopped: false,
        };

        // `startRunning` returns before the device is producing, and the size it
        // settles on is only knowable from a real frame. Waiting for one here
        // means `describe()` is right before the caller ever reads it.
        source.next_frame(FIRST_FRAME)?;
        Ok(source)
    }
}

impl Drop for AvfCameraSource {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

impl FrameSource for AvfCameraSource {
    fn describe(&self) -> &SourceDesc {
        &self.desc
    }

    fn region(&self) -> Option<Rect> {
        // The session picks a device mode rather than cropping to a rectangle,
        // so a region is a resolution request and not a crop.
        None
    }

    fn next_frame(&mut self, timeout: Duration) -> Result<RawFrame<'_>> {
        let meta = self.slot.take(timeout, &mut self.seen, &mut self.current)?;
        self.desc.width = meta.width;
        self.desc.height = meta.height;
        Ok(RawFrame {
            pts: meta.pts,
            bytes: &self.current,
            stride: meta.stride,
            // A camera repaints its whole sensor every frame.
            dirty: DirtyRects::unknown(),
            cursor: None,
            gpu: None,
        })
    }

    fn stop(&mut self) -> Result<()> {
        if self.stopped {
            return Ok(());
        }
        self.stopped = true;
        unsafe { self.session.stopRunning() };
        Ok(())
    }
}
