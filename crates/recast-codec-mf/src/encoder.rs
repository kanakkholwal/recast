use windows::core::{Interface, GUID};
use windows::Win32::Media::MediaFoundation::*;

use recast_codec::{EncoderDescriptor, VideoCodec};

use crate::windows_mf::{activate_for, ensure_started};

/// What the caller asks the encoder for. Deliberately small: everything else an
/// H.264 encoder can be told is either a default we are happy with or a knob no
/// user of ours has needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodeConfig {
    pub width: u32,
    pub height: u32,
    /// Numerator and denominator, so 30000/1001 stays exact.
    pub frame_rate: (u32, u32),
    pub bitrate: u32,
}

/// One compressed access unit as the transform produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedSample {
    /// Annex B, which is what Media Foundation emits.
    pub data: Vec<u8>,
    /// 100 ns units, the Media Foundation clock.
    pub timestamp: i64,
    pub duration: i64,
    pub is_sync: bool,
}

#[derive(Debug)]
pub enum EncodeError {
    /// Nothing on this machine matched the descriptor.
    NotFound,
    /// The frame handed in is smaller than the configured size.
    ShortFrame,
    Media(windows::core::Error),
}

impl From<windows::core::Error> for EncodeError {
    fn from(value: windows::core::Error) -> Self {
        Self::Media(value)
    }
}

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "no matching Media Foundation encoder"),
            Self::ShortFrame => write!(f, "the frame is smaller than the configured size"),
            Self::Media(e) => write!(f, "media foundation: {e}"),
        }
    }
}

impl std::error::Error for EncodeError {}

/// Hardware transforms are asynchronous and event driven; the Microsoft
/// software one is synchronous and pulled. The protocols differ enough that the
/// mode is kept explicitly rather than probed per call.
enum Mode {
    Sync,
    Async {
        events: IMFMediaEventGenerator,
        /// Outstanding `METransformNeedInput` events. The transform asks for
        /// frames ahead of time, so a credit here means `ProcessInput` can be
        /// called without waiting for another event.
        credits: u32,
    },
}

/// An H.264 transform, fed NV12 and drained of Annex B.
pub struct H264Encoder {
    transform: IMFTransform,
    mode: Mode,
    config: EncodeConfig,
    frame_bytes: usize,
    draining: bool,
}

impl H264Encoder {
    /// Opens `descriptor`. The descriptor is matched by id against a fresh
    /// enumeration rather than being held open, so nothing keeps a hardware
    /// session reserved between the probe and the encode.
    pub fn open(
        descriptor: &EncoderDescriptor,
        config: EncodeConfig,
    ) -> Result<Self, EncodeError> {
        if descriptor.codec != VideoCodec::H264 {
            return Err(EncodeError::NotFound);
        }
        if !ensure_started() {
            return Err(EncodeError::NotFound);
        }
        let (activate, asynchronous) = activate_for(&descriptor.id).ok_or(EncodeError::NotFound)?;
        // SAFETY: the activate came from MFTEnumEx and is alive here.
        let transform: IMFTransform = unsafe { activate.ActivateObject() }?;

        let mode = match asynchronous {
            false => Mode::Sync,
            true => {
                // An async transform stays locked until the caller says it
                // understands the event protocol.
                // SAFETY: reading and writing the transform's own attributes.
                unsafe {
                    let attributes = transform.GetAttributes()?;
                    attributes.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1)?;
                }
                Mode::Async {
                    events: transform.cast()?,
                    credits: 0,
                }
            }
        };

        let mut encoder = Self {
            transform,
            mode,
            config,
            frame_bytes: nv12_len(config.width, config.height),
            draining: false,
        };
        encoder.configure()?;
        Ok(encoder)
    }

    fn configure(&mut self) -> Result<(), EncodeError> {
        // Output first: an H.264 transform will not accept an input type until
        // it knows what it is producing.
        let output = media_type(&[
            (MF_MT_MAJOR_TYPE, Value::Guid(MFMediaType_Video)),
            (MF_MT_SUBTYPE, Value::Guid(MFVideoFormat_H264)),
            (MF_MT_AVG_BITRATE, Value::U32(self.config.bitrate)),
            (
                MF_MT_INTERLACE_MODE,
                Value::U32(MFVideoInterlace_Progressive.0 as u32),
            ),
            (
                MF_MT_FRAME_SIZE,
                Value::U64(pack(self.config.width, self.config.height)),
            ),
            (
                MF_MT_FRAME_RATE,
                Value::U64(pack(self.config.frame_rate.0, self.config.frame_rate.1)),
            ),
            (MF_MT_PIXEL_ASPECT_RATIO, Value::U64(pack(1, 1))),
        ])?;
        // SAFETY: stream 0 is the only stream an H.264 encoder MFT exposes.
        unsafe { self.transform.SetOutputType(0, &output, 0) }?;

        let input = media_type(&[
            (MF_MT_MAJOR_TYPE, Value::Guid(MFMediaType_Video)),
            (MF_MT_SUBTYPE, Value::Guid(MFVideoFormat_NV12)),
            (
                MF_MT_INTERLACE_MODE,
                Value::U32(MFVideoInterlace_Progressive.0 as u32),
            ),
            (
                MF_MT_FRAME_SIZE,
                Value::U64(pack(self.config.width, self.config.height)),
            ),
            (
                MF_MT_FRAME_RATE,
                Value::U64(pack(self.config.frame_rate.0, self.config.frame_rate.1)),
            ),
            (MF_MT_PIXEL_ASPECT_RATIO, Value::U64(pack(1, 1))),
        ])?;
        // SAFETY: as above.
        unsafe { self.transform.SetInputType(0, &input, 0) }?;

        // SAFETY: the documented start-up message pair.
        unsafe {
            self.transform
                .ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            self.transform
                .ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
        }
        Ok(())
    }

    /// Bytes one NV12 frame of this size occupies.
    pub fn frame_bytes(&self) -> usize {
        self.frame_bytes
    }

    /// Feeds one NV12 frame and returns whatever came out. An encoder holds
    /// frames back to look ahead, so an empty result is normal.
    pub fn encode(
        &mut self,
        nv12: &[u8],
        timestamp: i64,
        duration: i64,
    ) -> Result<Vec<EncodedSample>, EncodeError> {
        if nv12.len() < self.frame_bytes {
            return Err(EncodeError::ShortFrame);
        }
        let sample = memory_sample(&nv12[..self.frame_bytes], timestamp, duration)?;
        match &self.mode {
            Mode::Sync => {
                // SAFETY: the sample lives for the call, and stream 0 is the
                // only one an encoder MFT exposes.
                unsafe { self.transform.ProcessInput(0, &sample, 0) }?;
                self.drain_available()
            }
            Mode::Async { .. } => self.encode_async(sample),
        }
    }

    /// Waits for the transform to ask for input, feeds it, then takes whatever
    /// is already waiting. Blocking on the ask is correct: the transform always
    /// raises one eventually, and returning without feeding would stall.
    fn encode_async(&mut self, sample: IMFSample) -> Result<Vec<EncodedSample>, EncodeError> {
        let mut out = Vec::new();
        loop {
            if let Mode::Async { credits, .. } = &mut self.mode {
                if *credits > 0 {
                    *credits -= 1;
                    // SAFETY: a credit means the transform is ready for input.
                    unsafe { self.transform.ProcessInput(0, &sample, 0) }?;
                    break;
                }
            }
            // Waiting for a credit is also when output arrives: the transform
            // interleaves the two events, so nothing has to be polled for.
            match self.pump()? {
                Some(produced) => out.push(produced),
                None => continue,
            }
        }
        Ok(out)
    }

    /// Handles one transform event, blocking for it. Blocking is always right
    /// here: the caller only pumps when it needs something from the transform,
    /// and the transform always raises an event eventually.
    fn pump(&mut self) -> Result<Option<EncodedSample>, EncodeError> {
        let Mode::Async { events, .. } = &self.mode else {
            return Ok(None);
        };
        let events = events.clone();
        // SAFETY: pulling from the transform's own event queue.
        let event = unsafe { events.GetEvent(MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS(0)) }?;
        // SAFETY: every event carries its type.
        let kind = unsafe { event.GetType() }?;
        if kind == METransformNeedInput.0 as u32 {
            if let Mode::Async { credits, .. } = &mut self.mode {
                *credits += 1;
            }
            return Ok(None);
        }
        if kind == METransformHaveOutput.0 as u32 {
            return self.next_output();
        }
        if kind == METransformDrainComplete.0 as u32 {
            self.draining = false;
        }
        Ok(None)
    }

    /// Tells the transform there is no more input and collects the tail.
    pub fn finish(&mut self) -> Result<Vec<EncodedSample>, EncodeError> {
        // SAFETY: the documented shutdown message pair.
        unsafe {
            self.transform
                .ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0)?;
            self.transform.ProcessMessage(MFT_MESSAGE_COMMAND_DRAIN, 0)?;
        }
        if matches!(self.mode, Mode::Sync) {
            return self.drain_available();
        }
        // `draining` is cleared by the drain-complete event, which is the only
        // signal that the transform has finished; a plain read loop would block
        // forever once it has.
        self.draining = true;
        let mut out = Vec::new();
        while self.draining {
            if let Some(produced) = self.pump()? {
                out.push(produced);
            }
        }
        Ok(out)
    }

    fn drain_available(&mut self) -> Result<Vec<EncodedSample>, EncodeError> {
        let mut out = Vec::new();
        loop {
            match self.next_output()? {
                Some(sample) => out.push(sample),
                None => return Ok(out),
            }
        }
    }

    fn next_output(&mut self) -> Result<Option<EncodedSample>, EncodeError> {
        // SAFETY: reading the stream info the transform advertises.
        let info = unsafe { self.transform.GetOutputStreamInfo(0) }?;
        // A transform that allocates its own samples wants a null one handed in.
        let provides_samples = info.dwFlags
            & (MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32
                | MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES.0 as u32)
            != 0;
        let mut buffers = [MFT_OUTPUT_DATA_BUFFER {
            dwStreamID: 0,
            pSample: std::mem::ManuallyDrop::new(match provides_samples {
                true => None,
                // SAFETY: allocating the buffer the transform asked us for.
                false => Some(unsafe { empty_sample(info.cbSize as usize) }?),
            }),
            dwStatus: 0,
            pEvents: std::mem::ManuallyDrop::new(None),
        }];
        let mut status = 0u32;
        // SAFETY: one buffer for the one stream.
        let result = unsafe { self.transform.ProcessOutput(0, &mut buffers, &mut status) };
        if let Err(error) = result {
            let sample = std::mem::ManuallyDrop::take_if_needed(&mut buffers[0].pSample);
            drop(sample);
            // Not an error: the transform simply has nothing for us yet.
            if error.code() == MF_E_TRANSFORM_NEED_MORE_INPUT {
                return Ok(None);
            }
            return Err(EncodeError::Media(error));
        }

        let sample = std::mem::ManuallyDrop::take_if_needed(&mut buffers[0].pSample);
        let Some(sample) = sample else {
            return Ok(None);
        };
        Ok(Some(read_sample(&sample)?))
    }
}

impl Drop for H264Encoder {
    fn drop(&mut self) {
        // Releasing without ending the stream leaves a hardware session held on
        // some drivers, which is the whole reason NVENC runs out of them.
        // SAFETY: a shutdown message on a live transform.
        unsafe {
            let _ = self
                .transform
                .ProcessMessage(MFT_MESSAGE_NOTIFY_END_STREAMING, 0);
        }
    }
}

/// Helper so the `ManuallyDrop` dance above reads once rather than three times.
trait TakeIfNeeded<T> {
    fn take_if_needed(slot: &mut std::mem::ManuallyDrop<T>) -> T;
}

impl<T> TakeIfNeeded<T> for std::mem::ManuallyDrop<T> {
    fn take_if_needed(slot: &mut std::mem::ManuallyDrop<T>) -> T {
        // SAFETY: every caller drops or returns the value, so it is taken once.
        unsafe { std::mem::ManuallyDrop::take(slot) }
    }
}

fn read_sample(sample: &IMFSample) -> Result<EncodedSample, EncodeError> {
    // SAFETY: reading the attributes and the contiguous buffer of a sample the
    // transform just handed us.
    unsafe {
        let timestamp = sample.GetSampleTime().unwrap_or(0);
        let duration = sample.GetSampleDuration().unwrap_or(0);
        let is_sync = sample
            .GetUINT32(&MFSampleExtension_CleanPoint)
            .map(|v| v != 0)
            .unwrap_or(true);
        let buffer = sample.ConvertToContiguousBuffer()?;
        let mut start = std::ptr::null_mut();
        let mut length = 0u32;
        buffer.Lock(&mut start, None, Some(&mut length))?;
        let data = std::slice::from_raw_parts(start, length as usize).to_vec();
        buffer.Unlock()?;
        Ok(EncodedSample {
            data,
            timestamp,
            duration,
            is_sync,
        })
    }
}

/// SAFETY: the caller must give a non-zero size the transform asked for.
unsafe fn empty_sample(size: usize) -> Result<IMFSample, windows::core::Error> {
    let sample = MFCreateSample()?;
    let buffer = MFCreateMemoryBuffer(size.max(1) as u32)?;
    sample.AddBuffer(&buffer)?;
    Ok(sample)
}

fn memory_sample(
    data: &[u8],
    timestamp: i64,
    duration: i64,
) -> Result<IMFSample, windows::core::Error> {
    // SAFETY: the buffer is sized to the slice and unlocked before use.
    unsafe {
        let sample = MFCreateSample()?;
        let buffer = MFCreateMemoryBuffer(data.len() as u32)?;
        let mut start = std::ptr::null_mut();
        buffer.Lock(&mut start, None, None)?;
        std::ptr::copy_nonoverlapping(data.as_ptr(), start, data.len());
        buffer.Unlock()?;
        buffer.SetCurrentLength(data.len() as u32)?;
        sample.AddBuffer(&buffer)?;
        sample.SetSampleTime(timestamp)?;
        sample.SetSampleDuration(duration)?;
        Ok(sample)
    }
}

enum Value {
    Guid(GUID),
    U32(u32),
    U64(u64),
}

fn media_type(attributes: &[(GUID, Value)]) -> Result<IMFMediaType, windows::core::Error> {
    // SAFETY: setting attributes on a type we just created.
    unsafe {
        let media = MFCreateMediaType()?;
        for (key, value) in attributes {
            match value {
                Value::Guid(v) => media.SetGUID(key, v)?,
                Value::U32(v) => media.SetUINT32(key, *v)?,
                Value::U64(v) => media.SetUINT64(key, *v)?,
            }
        }
        Ok(media)
    }
}

/// Media Foundation packs paired values into one 64-bit attribute, high half
/// first: frame size is width and height, frame rate is numerator and
/// denominator.
fn pack(high: u32, low: u32) -> u64 {
    ((high as u64) << 32) | low as u64
}

/// NV12 is a full-size luma plane plus a half-height interleaved chroma plane.
fn nv12_len(width: u32, height: u32) -> usize {
    (width as usize * height as usize) * 3 / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paired_attributes_pack_high_half_first() {
        assert_eq!(pack(1920, 1080), (1920u64 << 32) | 1080);
        assert_eq!(pack(30_000, 1001), (30_000u64 << 32) | 1001);
    }

    #[test]
    fn an_nv12_frame_is_one_and_a_half_planes() {
        assert_eq!(nv12_len(2, 2), 6);
        assert_eq!(nv12_len(1920, 1080), 1920 * 1080 * 3 / 2);
    }
}
