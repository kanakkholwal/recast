use core::time::Duration;
use std::sync::{Arc, Condvar, Mutex};

use capturekit_core::{
    CaptureError, ColorSpace, DirtyRects, DisplayId, PixelFormat, Rect, Result, Rotation,
    SourceDesc, Timestamp, WindowId,
};
use dispatch2::{DispatchQueue, DispatchQueueAttr};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{define_class, msg_send, AllocAnyThread, DefinedClass};
use objc2_core_media::CMSampleBuffer;
use objc2_core_video::{
    CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight,
    CVPixelBufferGetPixelFormatType, CVPixelBufferGetWidth, CVPixelBufferLockBaseAddress,
    CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
};
use objc2_foundation::{NSArray, NSObject, NSObjectProtocol};
use objc2_screen_capture_kit::{
    SCContentFilter, SCStream, SCStreamConfiguration, SCStreamOutput, SCStreamOutputType,
};

use crate::backend::{RawFrame, ScreenBackend};
use crate::platform::macos::content::{self, BACKEND};
use crate::platform::OpenOptions;
use crate::shot::CursorMode;

/// `kCVPixelFormatType_32BGRA`, the only format this backend asks for.
const BGRA: u32 = u32::from_be_bytes(*b"BGRA");

/// One delivered frame, already copied out of the pixel buffer.
///
/// Copied rather than retained because holding an `IOSurface` locked across the
/// consumer's work starves the stream's own pool, which is only `queue_depth`
/// frames deep. The copy is into a reused buffer, so it costs no allocation.
#[derive(Default)]
struct Delivered {
    bytes: Vec<u8>,
    stride: u32,
    width: u32,
    height: u32,
    pts: Timestamp,
    /// Bumped per delivery so a waiter can tell a new frame from the last one.
    sequence: u64,
}

#[derive(Default)]
struct FrameSlot {
    frame: Mutex<Delivered>,
    arrived: Condvar,
}

impl FrameSlot {
    /// Copy a delivered pixel buffer into the slot and wake the waiter.
    fn accept(&self, sample: &CMSampleBuffer) {
        let Some(image) = (unsafe { sample.image_buffer() }) else {
            return;
        };
        let pixels = image.as_ref();
        if CVPixelBufferGetPixelFormatType(pixels) != BGRA {
            return;
        }

        let locked =
            unsafe { CVPixelBufferLockBaseAddress(pixels, CVPixelBufferLockFlags::ReadOnly) };
        if locked != 0 {
            return;
        }
        let base = CVPixelBufferGetBaseAddress(pixels);
        let stride = CVPixelBufferGetBytesPerRow(pixels);
        let width = CVPixelBufferGetWidth(pixels);
        let height = CVPixelBufferGetHeight(pixels);

        if !base.is_null() && stride > 0 && height > 0 {
            let time = unsafe { sample.presentation_time_stamp() };
            let pts = match time.timescale {
                0 => Timestamp::ZERO,
                scale => Timestamp::from_ticks(time.value, i64::from(scale)),
            };
            if let Ok(mut slot) = self.frame.lock() {
                let len = stride * height;
                slot.bytes.clear();
                slot.bytes.reserve(len);
                // SAFETY: the buffer is locked, so `base` points at
                // `stride * height` readable bytes until the unlock below.
                let source = unsafe { core::slice::from_raw_parts(base.cast::<u8>(), len) };
                slot.bytes.extend_from_slice(source);
                slot.stride = stride as u32;
                slot.width = width as u32;
                slot.height = height as u32;
                slot.pts = pts;
                slot.sequence = slot.sequence.wrapping_add(1);
                self.arrived.notify_all();
            }
        }
        unsafe { CVPixelBufferUnlockBaseAddress(pixels, CVPixelBufferLockFlags::ReadOnly) };
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and this type has no Drop.
    #[unsafe(super(NSObject))]
    #[name = "CapturekitStreamOutput"]
    #[ivars = Arc<FrameSlot>]
    struct StreamOutput;

    unsafe impl NSObjectProtocol for StreamOutput {}

    unsafe impl SCStreamOutput for StreamOutput {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        fn did_output(
            &self,
            _stream: &SCStream,
            sample: &CMSampleBuffer,
            kind: SCStreamOutputType,
        ) {
            if kind == SCStreamOutputType::Screen {
                self.ivars().accept(sample);
            }
        }
    }
);

impl StreamOutput {
    fn new(slot: Arc<FrameSlot>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(slot);
        unsafe { msg_send![super(this), init] }
    }
}

/// Display, region and window capture through ScreenCaptureKit.
pub(crate) struct SckSource {
    stream: Retained<SCStream>,
    _output: Retained<StreamOutput>,
    slot: Arc<FrameSlot>,
    desc: SourceDesc,
    region: Option<Rect>,
    /// The sequence the last returned frame carried, so a slow consumer is not
    /// handed the same frame twice as though it were new.
    last_sequence: u64,
    /// The frame handed to the caller, swapped out of the slot so the delivery
    /// thread cannot reallocate it while the caller reads it.
    current: Vec<u8>,
    stopped: bool,
}

// SAFETY: `SCStream` and the output object are thread-safe Objective-C objects;
// the frame slot is explicitly synchronised. The bound satisfies `ScreenBackend`.
unsafe impl Send for SckSource {}

fn configuration(size: Rect, source_rect: Option<Rect>, opts: &OpenOptions) -> Retained<SCStreamConfiguration> {
    let config = unsafe { SCStreamConfiguration::new() };
    unsafe {
        config.setWidth(size.width as usize);
        config.setHeight(size.height as usize);
        config.setPixelFormat(BGRA);
        config.setShowsCursor(opts.cursor == CursorMode::Include);
        // Two is the smallest depth that lets the stream keep producing while
        // one frame is being copied out.
        config.setQueueDepth(3);
        if let Some(rect) = source_rect {
            // The crop ScreenCaptureKit applies while compositing, so the pixels
            // outside it are never rendered, let alone copied.
            config.setSourceRect(objc2_core_foundation::CGRect {
                origin: objc2_core_foundation::CGPoint {
                    x: f64::from(rect.x),
                    y: f64::from(rect.y),
                },
                size: objc2_core_foundation::CGSize {
                    width: f64::from(rect.width),
                    height: f64::from(rect.height),
                },
            });
        }
        if let Some(fps) = opts.frame_rate.filter(|fps| *fps > 0) {
            config.setMinimumFrameInterval(objc2_core_media::CMTime {
                value: 1,
                timescale: fps as i32,
                flags: objc2_core_media::CMTimeFlags::Valid,
                epoch: 0,
            });
        }
    }
    config
}

impl SckSource {
    fn start(
        filter: Retained<SCContentFilter>,
        size: Rect,
        region: Option<Rect>,
        scale_factor: f32,
        rotation: Rotation,
        opts: &OpenOptions,
    ) -> Result<Self> {
        let staged = region.unwrap_or(size);
        let config = configuration(staged, region, opts);
        let slot = Arc::new(FrameSlot::default());
        let output = StreamOutput::new(Arc::clone(&slot));

        let stream = unsafe {
            SCStream::initWithFilter_configuration_delegate(SCStream::alloc(), &filter, &config, None)
        };
        // Serial: frames must reach the slot in the order the daemon produced
        // them, and a concurrent queue would let two deliveries race the swap.
        let queue = DispatchQueue::new("com.capturekit.frames", DispatchQueueAttr::SERIAL);
        let protocol = ProtocolObject::from_ref(&*output);
        unsafe {
            stream.addStreamOutput_type_sampleHandlerQueue_error(
                protocol,
                SCStreamOutputType::Screen,
                Some(&queue),
            )
        }
        .map_err(|error| {
            CaptureError::backend(BACKEND, std::io::Error::other(error.localizedDescription().to_string()))
        })?;

        start_capture(&stream)?;

        Ok(Self {
            stream,
            _output: output,
            slot,
            desc: SourceDesc {
                width: staged.width,
                height: staged.height,
                format: PixelFormat::Bgra8,
                color_space: ColorSpace::SRGB,
                rotation,
                scale_factor,
                frame_rate: opts.frame_rate,
                backend: BACKEND,
            },
            region,
            last_sequence: 0,
            current: Vec::new(),
            stopped: false,
        })
    }

    pub(crate) fn open_display(display: DisplayId, opts: &OpenOptions) -> Result<Self> {
        let (sc, described) = content::sc_display(display)?;
        let surface = Rect::from_size(described.bounds.width, described.bounds.height);
        let region = fit_region(opts.region, &surface)?;
        let filter = unsafe {
            SCContentFilter::initWithDisplay_excludingWindows(
                SCContentFilter::alloc(),
                &sc,
                &NSArray::new(),
            )
        };
        Self::start(
            filter,
            surface,
            region,
            described.scale_factor,
            described.rotation,
            opts,
        )
    }

    pub(crate) fn open_window(window: WindowId, opts: &OpenOptions) -> Result<Self> {
        let (sc, described) = content::sc_window(window)?;
        let surface = Rect::from_size(described.bounds.width, described.bounds.height);
        let region = fit_region(opts.region, &surface)?;
        let filter = unsafe {
            SCContentFilter::initWithDesktopIndependentWindow(SCContentFilter::alloc(), &sc)
        };
        Self::start(filter, surface, region, 1.0, Rotation::None, opts)
    }
}

fn fit_region(region: Option<Rect>, surface: &Rect) -> Result<Option<Rect>> {
    match region {
        None => Ok(None),
        Some(region) => Ok(Some(region.fit_inside(surface).ok_or(
            CaptureError::Unsupported {
                backend: BACKEND,
                operation: "crop to a region outside the source",
            },
        )?)),
    }
}

/// Start the stream and wait for the daemon to say whether it did.
fn start_capture(stream: &SCStream) -> Result<()> {
    let (sender, receiver) = std::sync::mpsc::channel::<Option<String>>();
    let handler = block2::RcBlock::new(move |error: *mut objc2_foundation::NSError| {
        let message = (!error.is_null()).then(|| {
            // SAFETY: non-null and live for the length of the handler.
            unsafe { &*error }.localizedDescription().to_string()
        });
        let _ = sender.send(message);
    });
    unsafe { stream.startCaptureWithCompletionHandler(Some(&handler)) };

    match receiver.recv_timeout(Duration::from_secs(5)) {
        Ok(None) => Ok(()),
        Ok(Some(message)) => Err(CaptureError::backend(
            BACKEND,
            std::io::Error::other(message),
        )),
        Err(_) => Err(CaptureError::Timeout(Duration::from_secs(5))),
    }
}

impl Drop for SckSource {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

impl ScreenBackend for SckSource {
    fn describe(&self) -> &SourceDesc {
        &self.desc
    }

    fn region(&self) -> Option<Rect> {
        self.region
    }

    fn next_frame(&mut self, timeout: Duration) -> Result<RawFrame<'_>> {
        let mut slot = self
            .slot
            .frame
            .lock()
            .map_err(|_| CaptureError::Lost(capturekit_core::LostReason::AccessLost))?;
        // Guard against a spurious wake handing back the frame already returned.
        while slot.sequence == self.last_sequence {
            let (next, waited) = self
                .slot
                .arrived
                .wait_timeout(slot, timeout)
                .map_err(|_| CaptureError::Lost(capturekit_core::LostReason::AccessLost))?;
            slot = next;
            if waited.timed_out() && slot.sequence == self.last_sequence {
                return Err(CaptureError::Timeout(timeout));
            }
        }
        self.last_sequence = slot.sequence;

        // The stream can renegotiate its size on a display mode change, and the
        // description has to follow or every consumer reads the wrong geometry.
        self.desc.width = slot.width;
        self.desc.height = slot.height;
        let pts = slot.pts;
        let stride = slot.stride;

        // Swap rather than copy: the caller gets a buffer this source owns, and
        // the delivery thread gets the previous one back to refill. Nothing is
        // shared, so it cannot reallocate under a reader, and after the first
        // frame neither side allocates again.
        core::mem::swap(&mut self.current, &mut slot.bytes);
        drop(slot);

        Ok(RawFrame {
            pts,
            bytes: &self.current,
            stride,
            // ScreenCaptureKit reports no damage rectangles.
            dirty: DirtyRects::unknown(),
        })
    }

    fn stop(&mut self) -> Result<()> {
        if self.stopped {
            return Ok(());
        }
        self.stopped = true;
        let handler = block2::RcBlock::new(|_error: *mut objc2_foundation::NSError| {});
        unsafe { self.stream.stopCaptureWithCompletionHandler(Some(&handler)) };
        Ok(())
    }
}
