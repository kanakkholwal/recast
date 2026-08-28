use core::time::Duration;
use std::sync::Arc;

use capturekit_core::{
    AudioDesc, AudioDeviceId, AudioDirection, AudioFormat, CaptureError, Result,
};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{define_class, msg_send, AllocAnyThread, DefinedClass};
use objc2_av_foundation::{
    AVCaptureAudioDataOutput, AVCaptureAudioDataOutputSampleBufferDelegate, AVCaptureConnection,
    AVCaptureDevice, AVCaptureDeviceInput, AVCaptureOutput, AVCaptureSession, AVMediaType,
    AVMediaTypeAudio,
};
use objc2_core_media::CMSampleBuffer;
use objc2_foundation::{NSObject, NSObjectProtocol, NSString};

use super::audio::accept;
use crate::backend::{AudioSource, RawAudio};
use crate::deliver::AudioQueue;

pub(super) const BACKEND: &str = "avfoundation-audio";

/// How long to wait for the first buffer before deciding the device is wedged.
///
/// A microphone that has been granted and is not muted delivers within a few
/// buffer periods; five seconds is a stall, not slow hardware.
const FIRST_BUFFER: Duration = Duration::from_secs(5);

fn failed(message: String) -> CaptureError {
    CaptureError::backend(BACKEND, std::io::Error::other(message))
}

fn unsupported(operation: &'static str) -> CaptureError {
    CaptureError::Unsupported {
        backend: BACKEND,
        operation,
    }
}

/// `AVMediaTypeAudio`, which the framework declares as a nullable global.
fn audio_media_type() -> Result<&'static AVMediaType> {
    unsafe { AVMediaTypeAudio }.ok_or_else(|| unsupported("name the audio media type"))
}

/// The device to open: the one named, else the system default input.
fn device_for(id: Option<&AudioDeviceId>) -> Result<Retained<AVCaptureDevice>> {
    match id {
        Some(id) => {
            // An AVFoundation audio device's uniqueID IS its CoreAudio UID, so a picker's id opens here untranslated.
            let uid = NSString::from_str(&id.0);
            unsafe { AVCaptureDevice::deviceWithUniqueID(&uid) }.ok_or_else(|| {
                CaptureError::NotFoundNamed {
                    kind: "audio input",
                    id: id.0.clone(),
                }
            })
        }
        None => unsafe { AVCaptureDevice::defaultDeviceWithMediaType(audio_media_type()?) }
            .ok_or_else(|| unsupported("find a default audio input, and the system named none")),
    }
}

define_class!(
    #[unsafe(super(NSObject))]
    #[ivars = Arc<AudioQueue>]
    struct MicOutput;

    unsafe impl NSObjectProtocol for MicOutput {}

    unsafe impl AVCaptureAudioDataOutputSampleBufferDelegate for MicOutput {
        #[unsafe(method(captureOutput:didOutputSampleBuffer:fromConnection:))]
        fn did_output(
            &self,
            _output: &AVCaptureOutput,
            sample: &CMSampleBuffer,
            _connection: &AVCaptureConnection,
        ) {
            accept(self.ivars(), sample);
        }
    }
);

impl MicOutput {
    fn new(queue: Arc<AudioQueue>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(queue);
        unsafe { msg_send![super(this), init] }
    }
}

/// A microphone or line input, through an `AVCaptureSession`.
///
/// ScreenCaptureKit can only tap the system default input and refuses any other
/// name, so a picked device would fail the track outright. AVFoundation opens
/// the device the user chose, and takes the Microphone TCC grant rather than the
/// Screen Recording one, which is the honest permission for a microphone.
pub(crate) struct AvfMicSource {
    session: Retained<AVCaptureSession>,
    _output: Retained<AVCaptureAudioDataOutput>,
    _delegate: Retained<MicOutput>,
    queue: Arc<AudioQueue>,
    desc: AudioDesc,
    current: Vec<u8>,
    stopped: bool,
}

// SAFETY: these are thread-safe Objective-C objects and the queue is synchronised.
unsafe impl Send for AvfMicSource {}

impl AvfMicSource {
    pub(crate) fn open(id: Option<&AudioDeviceId>) -> Result<Self> {
        let device = device_for(id)?;
        let uid = unsafe { device.uniqueID() }.to_string();
        let input = unsafe {
            AVCaptureDeviceInput::initWithDevice_error(AVCaptureDeviceInput::alloc(), &device)
        }
        .map_err(|error| failed(error.localizedDescription().to_string()))?;

        let session = unsafe { AVCaptureSession::new() };
        if !unsafe { session.canAddInput(&input) } {
            return Err(unsupported("open an input another session already holds"));
        }
        unsafe { session.addInput(&input) };

        let queue = Arc::new(AudioQueue::default());
        let delegate = MicOutput::new(Arc::clone(&queue));
        let output = unsafe { AVCaptureAudioDataOutput::new() };
        if !unsafe { session.canAddOutput(&output) } {
            return Err(unsupported("read samples from this input"));
        }
        unsafe { session.addOutput(&output) };

        // Serial: two deliveries racing would reorder the samples.
        let dispatch = dispatch2::DispatchQueue::new(
            "com.capturekit.microphone",
            dispatch2::DispatchQueueAttr::SERIAL,
        );
        unsafe {
            output.setSampleBufferDelegate_queue(
                Some(ProtocolObject::from_ref(&*delegate)),
                Some(&dispatch),
            );
            session.startRunning();
        }

        let mut source = Self {
            session,
            _output: output,
            _delegate: delegate,
            queue,
            desc: AudioDesc {
                // Replaced by what the first buffer carries; a guess would misresample.
                format: AudioFormat::STEREO_48K,
                device: AudioDeviceId(uid),
                direction: AudioDirection::Input,
                backend: BACKEND,
            },
            current: Vec::new(),
            stopped: false,
        };

        // `startRunning` returns before the device produces, and only a real buffer names the format.
        source.next_buffer(FIRST_BUFFER)?;
        Ok(source)
    }
}

impl Drop for AvfMicSource {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

impl AudioSource for AvfMicSource {
    fn describe(&self) -> &AudioDesc {
        &self.desc
    }

    fn next_buffer(&mut self, timeout: Duration) -> Result<RawAudio<'_>> {
        let (pts, discontinuous) = self.queue.take(timeout, &mut self.current)?;
        self.queue.report_drops(BACKEND);
        // Read off the samples: AVFoundation delivers whatever the hardware runs at.
        if let Some(format) = self.queue.format() {
            self.desc.format = format;
        }
        Ok(RawAudio {
            pts,
            bytes: &self.current,
            // Sampled continuously, so a quiet room is real silence, not absent buffers.
            silence: false,
            discontinuous,
        })
    }

    fn stop(&mut self) -> Result<()> {
        if self.stopped {
            return Ok(());
        }
        self.stopped = true;
        self.queue.report_drops(BACKEND);
        unsafe { self.session.stopRunning() };
        self.queue.end();
        Ok(())
    }
}
