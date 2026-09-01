use core::time::Duration;
use std::sync::Arc;

use capturekit_core::{
    CaptureError, ColorSpace, DirtyRects, DisplayId, PixelFormat, Rect, Result, Rotation,
    SourceDesc, WindowId,
};
use dispatch2::{DispatchQueue, DispatchQueueAttr};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{define_class, msg_send, AllocAnyThread, DefinedClass};
use objc2_core_media::CMSampleBuffer;
use objc2_foundation::{NSArray, NSObject, NSObjectProtocol};
use objc2_screen_capture_kit::{
    SCContentFilter, SCStream, SCStreamConfiguration, SCStreamDelegate, SCStreamOutput,
    SCStreamOutputType,
};

use crate::backend::{FrameSource, RawFrame};
use crate::deliver::{Endable, FrameSlot};
use crate::platform::macos::content::{self, BACKEND};
use crate::platform::macos::sample::{accept_video, BGRA};
use crate::platform::OpenOptions;
use crate::shot::CursorMode;

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
                accept_video(self.ivars(), sample);
            }
        }
    }
);

define_class!(
    #[unsafe(super(NSObject))]
    #[ivars = Arc<dyn Endable>]
    pub(super) struct StreamStopped;

    unsafe impl NSObjectProtocol for StreamStopped {}

    /// The daemon stops a stream when the grant is revoked, the display goes
    /// away or the window closes. Without this the consumer only sees frames
    /// stop arriving and waits out its timeout over and over.
    unsafe impl SCStreamDelegate for StreamStopped {
        #[unsafe(method(stream:didStopWithError:))]
        fn did_stop(&self, _stream: &SCStream, error: &objc2_foundation::NSError) {
            log::warn!("screencapturekit stopped the stream: {}", error.localizedDescription());
            self.ivars().end();
        }
    }
);

impl StreamStopped {
    pub(super) fn new(slot: Arc<dyn Endable>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(slot);
        // SAFETY: the ivars are set before `init`, which is the order `define_class!` requires.
        unsafe { msg_send![super(this), init] }
    }
}

impl StreamOutput {
    fn new(slot: Arc<FrameSlot>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(slot);
        // SAFETY: the ivars are set before `init`, which is the order `define_class!` requires.
        unsafe { msg_send![super(this), init] }
    }
}

/// Display, region and window capture through ScreenCaptureKit.
pub(crate) struct SckSource {
    stream: Retained<SCStream>,
    _output: Retained<StreamOutput>,
    /// Held only to keep the delegate alive; the stream does not retain it.
    _stopped: Retained<StreamStopped>,
    slot: Arc<FrameSlot>,
    desc: SourceDesc,
    region: Option<Rect>,
    /// The sequence the last returned frame carried, so a slow consumer is not
    /// handed the same frame twice as though it were new.
    seen: u64,
    /// The frame handed to the caller, swapped out of the slot so the delivery
    /// thread cannot reallocate it while the caller reads it.
    current: Vec<u8>,
    stopped: bool,
}

// SAFETY: `SCStream` and the output object are thread-safe ObjC objects, and the frame slot is explicitly synchronised.
unsafe impl Send for SckSource {}

fn configuration(
    size: Rect,
    source_rect: Option<Rect>,
    opts: &OpenOptions,
) -> Retained<SCStreamConfiguration> {
    // SAFETY: a plain allocation, taking no arguments to get wrong.
    let config = unsafe { SCStreamConfiguration::new() };
    // SAFETY: setters on the configuration just allocated, each taking a plain value.
    unsafe {
        config.setWidth(size.width as usize);
        config.setHeight(size.height as usize);
        config.setPixelFormat(BGRA);
        config.setShowsCursor(opts.cursor == CursorMode::Include);
        // Two is the smallest depth that lets the stream keep producing while one frame is copied out.
        config.setQueueDepth(3);
        if let Some(rect) = source_rect {
            // The crop ScreenCaptureKit applies while compositing, so pixels outside it are never rendered, let alone copied.
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
        if let Some(fps) = opts.frame_rate().filter(|fps| *fps > 0) {
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
        let stopped = StreamStopped::new(Arc::clone(&slot) as Arc<dyn Endable>);

        // SAFETY: filter, configuration and delegate are all live for the call, and the stream retains what it keeps.
        let stream = unsafe {
            SCStream::initWithFilter_configuration_delegate(
                SCStream::alloc(),
                &filter,
                &config,
                Some(ProtocolObject::from_ref(&*stopped)),
            )
        };
        // Serial: frames must reach the slot in daemon order, and a concurrent queue would let two deliveries race the swap.
        let queue = DispatchQueue::new("com.capturekit.frames", DispatchQueueAttr::SERIAL);
        let protocol = ProtocolObject::from_ref(&*output);
        // SAFETY: the output object and the queue outlive the stream, which retains both.
        unsafe {
            stream.addStreamOutput_type_sampleHandlerQueue_error(
                protocol,
                SCStreamOutputType::Screen,
                Some(&queue),
            )
        }
        .map_err(|error| {
            CaptureError::backend(
                BACKEND,
                std::io::Error::other(error.localizedDescription().to_string()),
            )
        })?;

        start_capture(&stream)?;

        Ok(Self {
            stream,
            _output: output,
            _stopped: stopped,
            slot,
            desc: SourceDesc {
                width: staged.width,
                height: staged.height,
                format: PixelFormat::Bgra8,
                color_space: ColorSpace::SRGB,
                rotation,
                scale_factor,
                frame_rate: opts.frame_rate(),
                backend: BACKEND,
            },
            region,
            seen: 0,
            current: Vec::new(),
            stopped: false,
        })
    }

    pub(crate) fn open_display(display: DisplayId, opts: &OpenOptions) -> Result<Self> {
        let (sc, described) = content::sc_display(display)?;
        let surface = Rect::from_size(described.bounds.width, described.bounds.height);
        let region = fit_region(opts.region, &surface)?;
        // SAFETY: `sc` is a live display from the shareable-content query and the exclusion list is a fresh empty array.
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
        // SAFETY: `sc` is a live window from the shareable-content query.
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
pub(super) fn start_capture(stream: &SCStream) -> Result<()> {
    let (sender, receiver) = std::sync::mpsc::channel::<Option<String>>();
    let handler = block2::RcBlock::new(move |error: *mut objc2_foundation::NSError| {
        let message = (!error.is_null()).then(|| {
            // SAFETY: non-null and live for the length of the handler.
            unsafe { &*error }.localizedDescription().to_string()
        });
        let _ = sender.send(message);
    });
    // SAFETY: the handler is retained by the block and the channel it sends on outlives the wait below.
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

impl FrameSource for SckSource {
    fn describe(&self) -> &SourceDesc {
        &self.desc
    }

    fn region(&self) -> Option<Rect> {
        self.region
    }

    fn next_frame(&mut self, timeout: Duration) -> Result<RawFrame<'_>> {
        let meta = self.slot.take(timeout, &mut self.seen, &mut self.current)?;
        // The stream can renegotiate size on a display-mode change, and the description must follow or consumers read wrong geometry.
        self.desc.width = meta.width;
        self.desc.height = meta.height;
        Ok(RawFrame {
            pts: meta.pts,
            bytes: &self.current,
            stride: meta.stride,
            // ScreenCaptureKit reports no damage rectangles.
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
        let handler = block2::RcBlock::new(|_error: *mut objc2_foundation::NSError| {});
        // SAFETY: the handler ignores its argument and borrows nothing, so it is safe to outlive this call.
        unsafe { self.stream.stopCaptureWithCompletionHandler(Some(&handler)) };
        Ok(())
    }
}
