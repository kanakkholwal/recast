use capturekit_core::Timestamp;
use objc2_core_media::CMSampleBuffer;
use objc2_core_video::{
    CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight,
    CVPixelBufferGetPixelFormatType, CVPixelBufferGetWidth, CVPixelBufferLockBaseAddress,
    CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
};

use crate::deliver::{Delivered, FrameSlot};

/// `kCVPixelFormatType_32BGRA`, the only pixel format either macOS source asks
/// for.
pub(super) const BGRA: u32 = u32::from_be_bytes(*b"BGRA");

/// Copy one delivered pixel buffer into the slot.
///
/// Shared by ScreenCaptureKit and by the AVFoundation camera: both hand over a
/// `CMSampleBuffer` wrapping a `CVPixelBuffer`, and both must not hold it
/// locked across the consumer's work, which would starve a pool only a few
/// frames deep. The copy is into a reused buffer, so it costs no allocation.
pub(super) fn accept_video(slot: &FrameSlot, sample: &CMSampleBuffer) {
    let Some(image) = (unsafe { sample.image_buffer() }) else {
        return;
    };
    let pixels = image.as_ref();
    if CVPixelBufferGetPixelFormatType(pixels) != BGRA {
        return;
    }

    let locked = unsafe { CVPixelBufferLockBaseAddress(pixels, CVPixelBufferLockFlags::ReadOnly) };
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
            // The host time clock every macOS source stamps, so a session lines its tracks up by subtracting one origin.
            scale => Timestamp::from_ticks(time.value, i64::from(scale)),
        };
        let len = stride * height;
        // SAFETY: the buffer is locked, so `base` covers stride times height readable bytes until the unlock below.
        let source = unsafe { core::slice::from_raw_parts(base.cast::<u8>(), len) };
        slot.publish(
            Delivered {
                pts,
                stride: stride as u32,
                width: width as u32,
                height: height as u32,
            },
            source,
        );
    }
    unsafe { CVPixelBufferUnlockBaseAddress(pixels, CVPixelBufferLockFlags::ReadOnly) };
}
