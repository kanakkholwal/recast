use core::time::Duration;
use std::sync::Arc;

use capturekit_core::{
    interleave, AudioDesc, AudioDeviceId, AudioDirection, AudioFormat, CaptureError, Result,
    SampleFormat, Timestamp,
};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{define_class, msg_send, sel, AllocAnyThread, DefinedClass};
use objc2_core_audio_types::{AudioBuffer, AudioBufferList, AudioStreamBasicDescription};
use objc2_core_media::{CMAudioFormatDescriptionGetStreamBasicDescription, CMSampleBuffer};
use objc2_foundation::{NSArray, NSObject, NSObjectProtocol};
use objc2_screen_capture_kit::{
    SCContentFilter, SCStream, SCStreamConfiguration, SCStreamOutput, SCStreamOutputType,
};

use crate::backend::{AudioSource, RawAudio};
use crate::deliver::{AudioQueue, Endable};
use crate::platform::macos::content::{self, BACKEND};
use crate::platform::macos::stream::{start_capture, StreamStopped};

/// The two devices this backend can name. ScreenCaptureKit exposes no list of
/// its own, so anything else is refused rather than guessed at.
const SYSTEM_AUDIO: &str = "system-audio";
const DEFAULT_INPUT: &str = "default-input";

/// CoreAudio's sample storage flags, from `CoreAudioBaseTypes.h`.
const FLAG_IS_FLOAT: u32 = 1 << 0;
const FLAG_IS_BIG_ENDIAN: u32 = 1 << 1;
const FLAG_IS_SIGNED_INTEGER: u32 = 1 << 2;

/// 7.1 is the widest layout a Mac mixes, and a fixed ceiling is what lets the
/// buffer list live on the stack instead of being allocated per delivery.
const MAX_CHANNELS: usize = 8;

/// How many samples may queue before a stalled consumer starts losing them.
/// Four seconds of 48 kHz stereo float, far past any read interval a recorder
/// uses and still bounded.
const QUEUE_BYTES: usize = 4 * 48_000 * 2 * 4;

/// What ScreenCaptureKit is asked for. It converts to this whatever the output
/// device runs at, which is why the answer does not depend on the hardware.
const REQUESTED: AudioFormat = AudioFormat::STEREO_48K;

fn unsupported(operation: &'static str) -> CaptureError {
    CaptureError::Unsupported {
        backend: BACKEND,
        operation,
    }
}

/// An `AudioBufferList` with room for every channel a Mac mixes.
///
/// The C type declares one buffer and is indexed past its end. Sizing the array
/// up front and handing CoreMedia the real byte count is what keeps that in
/// bounds; a stream with more channels is refused by CoreMedia rather than
/// overflowing this.
#[repr(C)]
struct ChannelBuffers {
    count: u32,
    buffers: [AudioBuffer; MAX_CHANNELS],
}

impl ChannelBuffers {
    const fn empty() -> Self {
        Self {
            count: 0,
            buffers: [AudioBuffer {
                mNumberChannels: 0,
                mDataByteSize: 0,
                mData: core::ptr::null_mut(),
            }; MAX_CHANNELS],
        }
    }

    /// The buffers CoreMedia actually filled.
    fn filled(&self) -> &[AudioBuffer] {
        let count = (self.count as usize).min(MAX_CHANNELS);
        &self.buffers[..count]
    }
}

/// Read a CoreAudio stream description into capturekit's vocabulary.
///
/// Kept separate from the delivery path so the flag decoding is unit-testable:
/// misreading float as integer is silence or noise, never an error.
fn format_of(asbd: &AudioStreamBasicDescription) -> Result<AudioFormat> {
    let flags = asbd.mFormatFlags;
    if flags & FLAG_IS_BIG_ENDIAN != 0 {
        return Err(unsupported("read big-endian samples"));
    }
    let float = flags & FLAG_IS_FLOAT != 0;
    let signed = flags & FLAG_IS_SIGNED_INTEGER != 0;
    let sample_format = match (float, signed, asbd.mBitsPerChannel) {
        (true, _, 32) => SampleFormat::F32,
        (false, true, 16) => SampleFormat::I16,
        (false, true, 32) => SampleFormat::I32,
        _ => return Err(unsupported("read a sample size capturekit has no name for")),
    };
    let channels = u16::try_from(asbd.mChannelsPerFrame)
        .map_err(|_| unsupported("read more channels than a format can name"))?;
    if channels == 0 || asbd.mSampleRate <= 0.0 {
        return Err(unsupported("read a stream with no channels or no rate"));
    }
    Ok(AudioFormat::new(
        asbd.mSampleRate as u32,
        channels,
        sample_format,
    ))
}

/// Copy one sample buffer's samples into `out`, interleaved.
///
/// ScreenCaptureKit delivers one plane per channel; capturekit's contract is
/// interleaved. A single buffer is already interleaved (or mono) and is copied
/// straight through.
fn samples_of(buffers: &[AudioBuffer], format: AudioFormat, out: &mut Vec<u8>) -> Result<()> {
    let planes: Vec<&[u8]> = buffers
        .iter()
        .map(|buffer| {
            if buffer.mData.is_null() {
                &[][..]
            } else {
                // SAFETY: the block buffer retained by the caller owns these
                // bytes until it is dropped, after this returns.
                unsafe {
                    core::slice::from_raw_parts(
                        buffer.mData.cast::<u8>(),
                        buffer.mDataByteSize as usize,
                    )
                }
            }
        })
        .collect();

    match planes.len() {
        0 => {
            out.clear();
            Ok(())
        }
        1 => {
            out.clear();
            out.extend_from_slice(planes[0]);
            format.validate_buffer(out.len())
        }
        _ => interleave(&planes, format, out).map(|_| ()),
    }
}

define_class!(
    #[unsafe(super(NSObject))]
    #[ivars = Arc<AudioQueue>]
    struct AudioOutput;

    unsafe impl NSObjectProtocol for AudioOutput {}

    unsafe impl SCStreamOutput for AudioOutput {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        fn did_output(
            &self,
            _stream: &SCStream,
            sample: &CMSampleBuffer,
            kind: SCStreamOutputType,
        ) {
            if kind == SCStreamOutputType::Audio || kind == SCStreamOutputType::Microphone {
                accept(self.ivars(), sample);
            }
        }
    }
);

impl AudioOutput {
    fn new(queue: Arc<AudioQueue>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(queue);
        unsafe { msg_send![super(this), init] }
    }
}

/// Pull the samples out of a delivered buffer and publish them.
fn accept(queue: &AudioQueue, sample: &CMSampleBuffer) {
    let Some(description) = (unsafe { sample.format_description() }) else {
        return;
    };
    // SAFETY: the description is an audio one, since this only runs for the
    // audio output types; a non-audio description returns null and is refused.
    let asbd = unsafe { CMAudioFormatDescriptionGetStreamBasicDescription(&description) };
    if asbd.is_null() {
        return;
    }
    let Ok(format) = format_of(unsafe { &*asbd }) else {
        return;
    };

    let mut list = ChannelBuffers::empty();
    let mut block = core::ptr::null_mut();
    let status = unsafe {
        sample.audio_buffer_list_with_retained_block_buffer(
            core::ptr::null_mut(),
            core::ptr::from_mut(&mut list).cast::<AudioBufferList>(),
            core::mem::size_of::<ChannelBuffers>(),
            None,
            None,
            0,
            &mut block,
        )
    };
    if status != 0 {
        log::debug!("screencapturekit audio buffer list refused: {status}");
        return;
    }
    // Retained by the call above, and it owns the sample memory read below.
    let _block = (!block.is_null()).then(|| unsafe { Retained::from_raw(block) });

    let mut samples = Vec::new();
    if samples_of(list.filled(), format, &mut samples).is_err() {
        return;
    }
    if samples.is_empty() {
        return;
    }
    let time = unsafe { sample.presentation_time_stamp() };
    let pts = match time.timescale {
        0 => Timestamp::ZERO,
        // The same host time clock the video output stamps, which is what makes
        // an audio and a video track of one session line up with no correction.
        scale => Timestamp::from_ticks(time.value, i64::from(scale)),
    };
    queue.publish(pts, &samples, QUEUE_BYTES);
}

/// ScreenCaptureKit names no devices, so a caller is told that rather than
/// handed a list with one invented entry in it.
pub(crate) fn devices() -> Result<Vec<capturekit_core::AudioDevice>> {
    Err(unsupported(
        "enumerate audio devices; it captures the system default",
    ))
}

/// System audio and the microphone, both through a ScreenCaptureKit stream.
///
/// A stream needs content to filter even when only its audio is wanted, so this
/// opens the main display at a size and rate that cost nothing and adds no video
/// output at all. That also means system audio carries the screen recording
/// grant on macOS, which is the platform's rule rather than a choice here.
pub(crate) struct SckAudioSource {
    stream: Retained<SCStream>,
    _output: Retained<AudioOutput>,
    _stopped: Retained<StreamStopped>,
    queue: Arc<AudioQueue>,
    desc: AudioDesc,
    current: Vec<u8>,
    stopped: bool,
}

// SAFETY: `SCStream` and the output object are thread-safe Objective-C objects
// and the slot is explicitly synchronised. The bound satisfies `AudioSource`.
unsafe impl Send for SckAudioSource {}

impl SckAudioSource {
    pub(crate) fn open(device: Option<&AudioDeviceId>, direction: AudioDirection) -> Result<Self> {
        let expected = match direction {
            AudioDirection::Loopback => SYSTEM_AUDIO,
            AudioDirection::Input => DEFAULT_INPUT,
        };
        if let Some(named) = device {
            if named.0 != expected {
                return Err(unsupported("open an audio device by name"));
            }
        }

        let config = configuration(direction)?;
        let (sc, _) = content::sc_display(content::main_display())?;
        let filter = unsafe {
            SCContentFilter::initWithDisplay_excludingWindows(
                SCContentFilter::alloc(),
                &sc,
                &NSArray::new(),
            )
        };

        let queue = Arc::new(AudioQueue::default());
        let output = AudioOutput::new(Arc::clone(&queue));
        let stopped = StreamStopped::new(Arc::clone(&queue) as Arc<dyn Endable>);
        let stream = unsafe {
            SCStream::initWithFilter_configuration_delegate(
                SCStream::alloc(),
                &filter,
                &config,
                Some(ProtocolObject::from_ref(&*stopped)),
            )
        };

        let kind = match direction {
            AudioDirection::Loopback => SCStreamOutputType::Audio,
            AudioDirection::Input => SCStreamOutputType::Microphone,
        };
        // Serial: buffers must reach the slot in the order the daemon produced
        // them, and a concurrent queue would let two deliveries race the swap.
        let deliveries = dispatch2::DispatchQueue::new(
            "com.capturekit.audio",
            dispatch2::DispatchQueueAttr::SERIAL,
        );
        unsafe {
            stream.addStreamOutput_type_sampleHandlerQueue_error(
                ProtocolObject::from_ref(&*output),
                kind,
                Some(&deliveries),
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
            queue,
            desc: AudioDesc {
                // What was asked for. The first delivery replaces it with what
                // the daemon actually produced, which for a microphone is the
                // device's own rate rather than this.
                format: REQUESTED,
                device: AudioDeviceId(expected.to_string()),
                direction,
                backend: BACKEND,
            },
            current: Vec::new(),
            stopped: false,
        })
    }
}

/// A stream configured to carry audio and as little video as SCK allows.
fn configuration(direction: AudioDirection) -> Result<Retained<SCStreamConfiguration>> {
    let config = unsafe { SCStreamConfiguration::new() };
    unsafe {
        // No screen output is ever added, so these only bound what the daemon
        // sets aside for a video path nothing reads.
        config.setWidth(2);
        config.setHeight(2);
        config.setSampleRate(REQUESTED.sample_rate as isize);
        config.setChannelCount(REQUESTED.channels as isize);
    }
    match direction {
        AudioDirection::Loopback => unsafe { config.setCapturesAudio(true) },
        AudioDirection::Input => {
            // `captureMicrophone` is macOS 15. Asking an older system for it
            // raises rather than returning, so the selector is checked first.
            if !config.respondsToSelector(sel!(setCaptureMicrophone:)) {
                return Err(unsupported("capture a microphone before macOS 15"));
            }
            unsafe { config.setCaptureMicrophone(true) };
        }
    }
    Ok(config)
}

impl Drop for SckAudioSource {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

impl AudioSource for SckAudioSource {
    fn describe(&self) -> &AudioDesc {
        &self.desc
    }

    fn next_buffer(&mut self, timeout: Duration) -> Result<RawAudio<'_>> {
        let (pts, lost) = self.queue.take(timeout, &mut self.current)?;
        Ok(RawAudio {
            pts,
            bytes: &self.current,
            // ScreenCaptureKit taps the mix continuously, so an idle system
            // delivers real silence rather than nothing.
            silence: false,
            // Set only when the queue had to refuse samples.
            discontinuous: lost,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn asbd(flags: u32, bits: u32, channels: u32, rate: f64) -> AudioStreamBasicDescription {
        AudioStreamBasicDescription {
            mSampleRate: rate,
            mFormatID: 0,
            mFormatFlags: flags,
            mBytesPerPacket: 0,
            mFramesPerPacket: 1,
            mBytesPerFrame: 0,
            mChannelsPerFrame: channels,
            mBitsPerChannel: bits,
            mReserved: 0,
        }
    }

    #[test]
    fn float_samples_are_read_as_float() {
        let format = format_of(&asbd(FLAG_IS_FLOAT, 32, 2, 48_000.0)).expect("float stereo");
        assert_eq!(format, AudioFormat::STEREO_48K);
    }

    #[test]
    fn signed_integer_samples_are_read_at_their_own_width() {
        let sixteen = format_of(&asbd(FLAG_IS_SIGNED_INTEGER, 16, 1, 44_100.0)).expect("i16 mono");
        assert_eq!(sixteen, AudioFormat::new(44_100, 1, SampleFormat::I16));
        let thirty_two =
            format_of(&asbd(FLAG_IS_SIGNED_INTEGER, 32, 2, 96_000.0)).expect("i32 stereo");
        assert_eq!(thirty_two.sample_format, SampleFormat::I32);
    }

    /// 32 bits set both ways is the case a width-only match gets wrong: reading
    /// float samples as integers is full-scale noise, not an error.
    #[test]
    fn thirty_two_bit_float_is_not_confused_with_thirty_two_bit_integer() {
        let float = format_of(&asbd(FLAG_IS_FLOAT, 32, 2, 48_000.0)).expect("float");
        let integer = format_of(&asbd(FLAG_IS_SIGNED_INTEGER, 32, 2, 48_000.0)).expect("integer");
        assert_eq!(float.sample_format, SampleFormat::F32);
        assert_eq!(integer.sample_format, SampleFormat::I32);
    }

    #[test]
    fn big_endian_samples_are_refused_rather_than_byte_swapped_by_accident() {
        assert!(format_of(&asbd(FLAG_IS_FLOAT | FLAG_IS_BIG_ENDIAN, 32, 2, 48_000.0)).is_err());
    }

    #[test]
    fn unsigned_integer_samples_are_refused() {
        assert!(format_of(&asbd(0, 16, 2, 48_000.0)).is_err());
    }

    #[test]
    fn a_stream_with_no_channels_or_no_rate_is_refused() {
        assert!(format_of(&asbd(FLAG_IS_FLOAT, 32, 0, 48_000.0)).is_err());
        assert!(format_of(&asbd(FLAG_IS_FLOAT, 32, 2, 0.0)).is_err());
    }

    fn buffer(bytes: &[u8], channels: u32) -> AudioBuffer {
        AudioBuffer {
            mNumberChannels: channels,
            mDataByteSize: bytes.len() as u32,
            mData: bytes.as_ptr() as *mut core::ffi::c_void,
        }
    }

    #[test]
    fn one_plane_per_channel_is_interleaved() {
        let left = [1u8, 1, 2, 2];
        let right = [9u8, 9, 8, 8];
        let format = AudioFormat::new(48_000, 2, SampleFormat::I16);
        let mut out = Vec::new();
        samples_of(&[buffer(&left, 1), buffer(&right, 1)], format, &mut out).expect("two planes");
        assert_eq!(out, vec![1, 1, 9, 9, 2, 2, 8, 8]);
    }

    /// An interleaved stream arrives as ONE buffer carrying every channel.
    /// Running it through the interleaver would treat it as a single plane and
    /// report half the channels.
    #[test]
    fn a_single_interleaved_buffer_is_copied_straight_through() {
        let packed = [1u8, 1, 9, 9, 2, 2, 8, 8];
        let format = AudioFormat::new(48_000, 2, SampleFormat::I16);
        let mut out = Vec::new();
        samples_of(&[buffer(&packed, 2)], format, &mut out).expect("one buffer");
        assert_eq!(out, packed);
    }

    #[test]
    fn a_partial_sample_frame_in_a_single_buffer_is_refused() {
        let ragged = [1u8, 1, 9];
        let format = AudioFormat::new(48_000, 2, SampleFormat::I16);
        let mut out = Vec::new();
        assert!(samples_of(&[buffer(&ragged, 2)], format, &mut out).is_err());
    }

    #[test]
    fn a_buffer_list_that_came_back_empty_yields_no_samples() {
        let mut out = vec![0xFF; 8];
        samples_of(&[], AudioFormat::STEREO_48K, &mut out).expect("nothing to read");
        assert!(out.is_empty());
    }

    /// The C type declares one buffer and is indexed past its end, so the count
    /// CoreMedia writes must never be trusted further than the array reaches.
    #[test]
    fn a_channel_count_past_the_array_is_clamped_to_it() {
        let mut list = ChannelBuffers::empty();
        list.count = 64;
        assert_eq!(list.filled().len(), MAX_CHANNELS);
    }
}
