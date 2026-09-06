use core::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use capturekit_core::{AudioDesc, AudioDeviceId, AudioDirection, AudioFormat, Result, Timestamp};

use crate::backend::AudioSource;
use crate::capturer::Flow;
use crate::platform::os;

/// One delivery of interleaved samples.
pub struct AudioBuffer<'a> {
    pts: Timestamp,
    bytes: &'a [u8],
    desc: &'a AudioDesc,
    silence: bool,
    discontinuous: bool,
}

impl AudioBuffer<'_> {
    /// When these samples were captured, on the source's clock.
    #[must_use]
    pub const fn pts(&self) -> Timestamp {
        self.pts
    }

    /// Interleaved samples, in the described format.
    #[must_use]
    pub const fn bytes(&self) -> &[u8] {
        self.bytes
    }

    /// What the backend negotiated.
    #[must_use]
    pub const fn desc(&self) -> &AudioDesc {
        self.desc
    }

    /// The format these samples are in.
    #[must_use]
    pub const fn format(&self) -> AudioFormat {
        self.desc.format
    }

    /// Sample frames in this buffer.
    #[must_use]
    pub fn frames(&self) -> usize {
        self.desc.format.frames_in(self.bytes.len())
    }

    /// How long this buffer lasts.
    #[must_use]
    pub fn duration(&self) -> Duration {
        self.desc.format.duration_of(self.frames() as u64)
    }

    /// Whether the device reported a break in the stream before these samples.
    /// Distinct from inserted silence: this is the driver saying it lost data, not capturekit covering a gap it measured.
    #[must_use]
    pub const fn is_discontinuous(&self) -> bool {
        self.discontinuous
    }

    /// Whether this buffer is silence the backend inserted to cover a gap the device ran through without delivering anything.
    /// A loopback device with nothing playing delivers no buffers at all, so without this the track would come out short and everything in it early.
    #[must_use]
    pub const fn is_inserted_silence(&self) -> bool {
        self.silence
    }
}

/// Configures an audio capture before it opens.
#[derive(Debug, Clone)]
pub struct AudioCapturerBuilder {
    device: Option<AudioDeviceId>,
    direction: AudioDirection,
}

impl AudioCapturerBuilder {
    pub(crate) const fn new(direction: AudioDirection) -> Self {
        Self {
            device: None,
            direction,
        }
    }

    /// Capture a specific device instead of the system default.
    #[must_use]
    pub fn device(mut self, device: AudioDeviceId) -> Self {
        self.device = Some(device);
        self
    }

    /// Open the device.
    pub fn build(self) -> Result<AudioCapturer> {
        let backend = os::open_audio(self.device.as_ref(), self.direction)?;
        let desc = backend.describe().clone();
        Ok(AudioCapturer { backend, desc })
    }
}

/// A live audio capture, held open.
pub struct AudioCapturer {
    backend: Box<dyn AudioSource>,
    desc: AudioDesc,
}

impl AudioCapturer {
    /// Wrap a scripted source, so a recorder's own loop can be driven with no device: its timeout and error arms are the ones a real device will not exercise on demand.
    #[cfg(any(test, feature = "mock"))]
    #[must_use]
    pub fn scripted(source: crate::mock::MockAudioSource) -> Self {
        let desc = crate::backend::AudioSource::describe(&source).clone();
        Self {
            backend: Box::new(source),
            desc,
        }
    }

    /// What the backend negotiated, which is not always what was asked for.
    #[must_use]
    pub const fn describe(&self) -> &AudioDesc {
        &self.desc
    }

    /// Wait for the next buffer.
    pub fn next_buffer(&mut self, timeout: Duration) -> Result<AudioBuffer<'_>> {
        let raw = self.backend.next_buffer(timeout)?;
        Ok(AudioBuffer {
            pts: raw.pts,
            bytes: raw.bytes,
            desc: &self.desc,
            silence: raw.silence,
            discontinuous: raw.discontinuous,
        })
    }

    /// Release the device.
    pub fn stop(&mut self) -> Result<()> {
        self.backend.stop()
    }

    /// Run `handler` on a capture thread until it returns [`Flow::Stop`].
    pub fn start<H>(mut self, timeout: Duration, mut handler: H) -> AudioHandle
    where
        H: FnMut(AudioBuffer<'_>) -> Flow + Send + 'static,
    {
        let stopping = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stopping);
        let join = std::thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                match self.next_buffer(timeout) {
                    Ok(buffer) => {
                        if handler(buffer) == Flow::Stop {
                            break;
                        }
                    }
                    // A timeout is a quiet device, not a failure.
                    Err(err) if err.is_recoverable() => continue,
                    Err(err) => {
                        let _ = self.stop();
                        return Err(err);
                    }
                }
            }
            self.stop()
        });
        AudioHandle {
            stopping,
            join: Some(join),
        }
    }
}

/// A running audio capture thread.
pub struct AudioHandle {
    stopping: Arc<AtomicBool>,
    join: Option<JoinHandle<Result<()>>>,
}

impl AudioHandle {
    /// Ask the capture to stop and wait for it.
    pub fn stop(mut self) -> Result<()> {
        self.stopping.store(true, Ordering::Relaxed);
        match self.join.take() {
            Some(join) => join.join().unwrap_or(Ok(())),
            None => Ok(()),
        }
    }

    /// Whether the capture thread has already finished.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.join.as_ref().is_some_and(JoinHandle::is_finished)
    }
}

impl Drop for AudioHandle {
    /// Stops rather than detaching, so a dropped handle cannot leave a device
    /// open.
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
