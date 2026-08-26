use std::path::Path;

use windows::core::HSTRING;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Variant::{VT_I8, VT_UI8};

use crate::windows_mf::ensure_started;

/// What the file says its video stream is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    /// Numerator and denominator, as stored.
    pub frame_rate: (u32, u32),
    /// 100 ns units, or zero when the file does not say.
    pub duration: i64,
}

/// One decoded frame, NV12, tightly packed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFrame {
    pub data: Vec<u8>,
    /// 100 ns units on the file's own timeline.
    pub timestamp: i64,
}

#[derive(Debug)]
pub enum DecodeError {
    /// Media Foundation would not start, or the file has no video.
    Unsupported,
    Media(windows::core::Error),
}

impl From<windows::core::Error> for DecodeError {
    fn from(value: windows::core::Error) -> Self {
        Self::Media(value)
    }
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => write!(f, "no decodable video stream"),
            Self::Media(e) => write!(f, "media foundation: {e}"),
        }
    }
}

impl std::error::Error for DecodeError {}

const VIDEO_STREAM: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;

/// Reads a file's video stream as NV12 frames.
pub struct VideoReader {
    reader: IMFSourceReader,
    info: VideoInfo,
}

impl VideoReader {
    pub fn open(path: &Path) -> Result<Self, DecodeError> {
        if !ensure_started() {
            return Err(DecodeError::Unsupported);
        }
        // SAFETY: the attribute store and the URL both outlive the call.
        let reader = unsafe {
            let mut attributes = None;
            MFCreateAttributes(&mut attributes, 1)?;
            let attributes = attributes.ok_or(DecodeError::Unsupported)?;
            // Lets the reader insert a converter, which is what makes asking
            // for NV12 work whatever the file actually holds.
            attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)?;
            MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), &attributes)?
        };

        // SAFETY: stream selection and type negotiation on the reader we own.
        unsafe {
            reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
            reader.SetStreamSelection(VIDEO_STREAM, true)?;

            let output = MFCreateMediaType()?;
            output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            output.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
            reader.SetCurrentMediaType(VIDEO_STREAM, None, &output)?;
        }

        let info = Self::read_info(&reader)?;
        Ok(Self { reader, info })
    }

    fn read_info(reader: &IMFSourceReader) -> Result<VideoInfo, DecodeError> {
        // SAFETY: reading the type the reader settled on, plus one descriptor.
        unsafe {
            let media = reader.GetCurrentMediaType(VIDEO_STREAM)?;
            let (width, height) = unpack(media.GetUINT64(&MF_MT_FRAME_SIZE)?);
            let frame_rate = media
                .GetUINT64(&MF_MT_FRAME_RATE)
                .map(unpack)
                .unwrap_or((0, 1));
            let duration = reader
                .GetPresentationAttribute(
                    MF_SOURCE_READER_MEDIASOURCE.0 as u32,
                    &MF_PD_DURATION,
                )
                .ok()
                // Media Foundation stores the duration unsigned, but the
                // signed form is legal and some sources use it.
                .and_then(|value| match value.Anonymous.Anonymous.vt {
                    VT_UI8 => Some(value.Anonymous.Anonymous.Anonymous.uhVal as i64),
                    VT_I8 => Some(value.Anonymous.Anonymous.Anonymous.hVal),
                    _ => None,
                })
                .unwrap_or(0);
            Ok(VideoInfo {
                width,
                height,
                frame_rate,
                duration,
            })
        }
    }

    pub fn info(&self) -> VideoInfo {
        self.info
    }

    /// The next frame, or `None` at end of stream.
    ///
    /// A read can legitimately return neither a frame nor the end: a format
    /// change or a gap produces an empty sample, so the caller loops rather
    /// than treating that as the end.
    pub fn next_frame(&mut self) -> Result<Option<DecodedFrame>, DecodeError> {
        loop {
            let mut flags = 0u32;
            let mut timestamp = 0i64;
            let mut sample = None;
            // SAFETY: the out-params live for the call.
            unsafe {
                self.reader.ReadSample(
                    VIDEO_STREAM,
                    0,
                    None,
                    Some(&mut flags),
                    Some(&mut timestamp),
                    Some(&mut sample),
                )?;
            }
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                return Ok(None);
            }
            // The reader can renegotiate mid-file, and the size is what every
            // caller sized its buffers from.
            if flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0 {
                self.info = Self::read_info(&self.reader)?;
            }
            let Some(sample) = sample else { continue };
            return Ok(Some(DecodedFrame {
                data: contiguous_bytes(&sample)?,
                timestamp,
            }));
        }
    }

    /// Moves to `timestamp` in 100 ns units. Decoding resumes from the keyframe
    /// at or before it, which is what every seek in a video editor means.
    pub fn seek(&mut self, timestamp: i64) -> Result<(), DecodeError> {
        let position = windows::Win32::System::Com::StructuredStorage::PROPVARIANT::from(timestamp);
        // SAFETY: the position outlives the call.
        unsafe { self.reader.SetCurrentPosition(&windows::core::GUID::zeroed(), &position) }?;
        Ok(())
    }
}

fn contiguous_bytes(sample: &IMFSample) -> Result<Vec<u8>, DecodeError> {
    // SAFETY: converting to one buffer, then reading it under a lock.
    unsafe {
        let buffer = sample.ConvertToContiguousBuffer()?;
        let mut start = std::ptr::null_mut();
        let mut length = 0u32;
        buffer.Lock(&mut start, None, Some(&mut length))?;
        let data = std::slice::from_raw_parts(start, length as usize).to_vec();
        buffer.Unlock()?;
        Ok(data)
    }
}

/// Media Foundation packs paired values high half first.
fn unpack(value: u64) -> (u32, u32) {
    ((value >> 32) as u32, value as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_packed_pair_reads_back_high_half_first() {
        assert_eq!(unpack((1920u64 << 32) | 1080), (1920, 1080));
        assert_eq!(unpack((30_000u64 << 32) | 1001), (30_000, 1001));
    }
}
