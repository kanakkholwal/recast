use core::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, OnceLock};
use std::thread::JoinHandle;

use capturekit_core::{
    Camera, CameraFormat, CameraId, CaptureError, ColorSpace, DirtyRects, PixelFormat, Rect,
    Result, Rotation, SourceDesc,
};
use windows::core::{Interface, GUID, HRESULT, PWSTR};
use windows::Win32::Foundation::S_OK;
use windows::Win32::Media::MediaFoundation::{
    IMF2DBuffer, IMFActivate, IMFAttributes, IMFMediaSource, IMFMediaType, IMFSourceReader,
    MFCreateAttributes, MFCreateMediaType, MFCreateSourceReaderFromMediaSource,
    MFEnumDeviceSources, MFMediaType_Video, MFShutdown, MFStartup, MFVideoFormat_RGB32,
    MFSTARTUP_NOSOCKET, MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MF_MT_DEFAULT_STRIDE,
    MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
    MF_SOURCE_READERF_ENDOFSTREAM, MF_SOURCE_READERF_ERROR,
    MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, MF_SOURCE_READER_FIRST_VIDEO_STREAM,
    MF_VERSION,
};
use windows::Win32::System::Com::CoTaskMemFree;

use super::com::ComScope;
use crate::backend::{FrameSource, RawFrame};
use crate::deliver::{Delivered, FrameSlot};
use crate::platform::OpenOptions;

const BACKEND: &str = "mediafoundation";
/// The stream index every device video source publishes its frames on.
const VIDEO_STREAM: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
/// What the reader is asked for when the caller names no size.
const DEFAULT_SIZE: (u32, u32) = (1280, 720);

fn err(source: windows::core::Error) -> CaptureError {
    CaptureError::backend(BACKEND, source)
}

fn unsupported(operation: &'static str) -> CaptureError {
    CaptureError::Unsupported {
        backend: BACKEND,
        operation,
    }
}

/// Starts Media Foundation once for the process, and never shuts it down.
/// `MFShutdown` is refcounted against `MFStartup`, but a library cannot know when the host is done, and balancing them would tear MF down under a camera still held open.
fn ensure_started() -> Result<()> {
    static STARTED: OnceLock<HRESULT> = OnceLock::new();
    // SAFETY: starts Media Foundation once per process, which the `OnceLock` guarantees.
    let hr = *STARTED.get_or_init(|| unsafe {
        MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).map_or_else(|error| error.code(), |()| S_OK)
    });
    if hr.is_ok() {
        Ok(())
    } else {
        Err(err(windows::core::Error::from(hr)))
    }
}

/// A `PWSTR` the callee allocated, freed however this function leaves.
fn take_string(value: PWSTR) -> String {
    if value.is_null() {
        return String::new();
    }
    // SAFETY: `value` is a live NUL-terminated wide string until the free below.
    let text = unsafe { value.to_string() }.unwrap_or_default();
    // SAFETY: frees the CoTaskMem string the caller allocated, exactly once.
    unsafe { CoTaskMemFree(Some(value.as_ptr().cast())) };
    text
}

fn attribute_string(activate: &IMFActivate, key: &GUID) -> Option<String> {
    let mut value = PWSTR::null();
    let mut len = 0u32;
    // SAFETY: both out-parameters are live locals; the string is freed by the caller.
    unsafe { activate.GetAllocatedString(key, &mut value, &mut len) }.ok()?;
    Some(take_string(value))
}

/// Width and height out of the `UINT64` MF packs them into.
fn unpack_pair(value: u64) -> (u32, u32) {
    ((value >> 32) as u32, (value & 0xffff_ffff) as u32)
}

fn pack_pair(high: u32, low: u32) -> u64 {
    (u64::from(high) << 32) | u64::from(low)
}

/// Every video capture device the system offers.
/// The array and each activation object in it are owned by the caller of `MFEnumDeviceSources`, so both are released here whatever happens next.
fn activations() -> Result<Vec<IMFActivate>> {
    ensure_started()?;
    let mut attributes: Option<IMFAttributes> = None;
    // SAFETY: writes the new attribute store into a live out-slot.
    unsafe { MFCreateAttributes(&mut attributes, 1) }.map_err(err)?;
    let attributes = attributes.ok_or_else(|| unsupported("describe a device query"))?;
    // SAFETY: a setter on the attribute store just created.
    unsafe {
        attributes.SetGUID(
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        )
    }
    .map_err(err)?;

    let mut raw: *mut Option<IMFActivate> = core::ptr::null_mut();
    let mut count = 0u32;
    // SAFETY: the array pointer and count are live locals; the array is freed below.
    unsafe { MFEnumDeviceSources(&attributes, &mut raw, &mut count) }.map_err(err)?;
    if raw.is_null() {
        return Ok(Vec::new());
    }
    let mut found = Vec::with_capacity(count as usize);
    for index in 0..count as usize {
        // SAFETY: `index` is below the reported count, and the read takes the array's own reference.
        if let Some(activate) = unsafe { raw.add(index).read() } {
            found.push(activate);
        }
    }
    // SAFETY: frees the array allocated above, after every element was taken out of it.
    unsafe { CoTaskMemFree(Some(raw.cast())) };
    Ok(found)
}

/// The symbolic link that identifies a device across reboots and USB ports.
fn symbolic_link(activate: &IMFActivate) -> Option<String> {
    attribute_string(
        activate,
        &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
    )
}

pub(crate) fn cameras() -> Result<Vec<Camera>> {
    let _com = ComScope::mta();
    let found = activations()?;
    let mut cameras = Vec::with_capacity(found.len());
    for (index, activate) in found.iter().enumerate() {
        let Some(id) = symbolic_link(activate) else {
            continue;
        };
        let name = attribute_string(activate, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME)
            .unwrap_or_else(|| format!("Camera {}", index + 1));
        cameras.push(Camera {
            id: CameraId(id),
            name,
            // Media Foundation enumerates in class-installer registration order, and the first is what a camera app opens.
            is_default: index == 0,
            formats: modes(activate).unwrap_or_default(),
        });
    }
    Ok(cameras)
}

/// Shuts a media source down when it drops. An early `?` past a manual
/// `Shutdown` left the camera powered on and held until the process exited,
/// and enumeration activates every device it lists.
struct ActiveSource(IMFMediaSource);

impl Drop for ActiveSource {
    fn drop(&mut self) {
        // SAFETY: shuts down the source this owns, exactly once.
        let _ = unsafe { self.0.Shutdown() };
    }
}

/// The modes one device advertises, deduplicated and largest first.
/// Reported as what capturekit will deliver rather than the device's own subtype, since the reader converts and a webcam lists the same geometry over MJPG, YUY2 and NV12.
fn modes(activate: &IMFActivate) -> Result<Vec<CameraFormat>> {
    // SAFETY: activates the device this activation object describes; shut down below.
    let source = ActiveSource(unsafe { activate.ActivateObject() }.map_err(err)?);
    let reader = reader_for(&source.0)?;
    let mut modes: Vec<CameraFormat> = Vec::new();
    let mut index = 0u32;
    // SAFETY: the reader is live, and an index past the end is reported as an error.
    while let Ok(media_type) = unsafe { reader.GetNativeMediaType(VIDEO_STREAM, index) } {
        index += 1;
        // SAFETY: a documented UINT64 attribute on the live media type above.
        let Ok(size) = (unsafe { media_type.GetUINT64(&MF_MT_FRAME_SIZE) }) else {
            continue;
        };
        let (width, height) = unpack_pair(size);
        if width == 0 || height == 0 {
            continue;
        }
        // SAFETY: a documented UINT64 attribute on the same live media type.
        let frame_rate = unsafe { media_type.GetUINT64(&MF_MT_FRAME_RATE) }
            .ok()
            .and_then(|packed| {
                let (numerator, denominator) = unpack_pair(packed);
                (denominator != 0).then(|| numerator as f32 / denominator as f32)
            });
        let mode = CameraFormat {
            width,
            height,
            pixel_format: PixelFormat::Bgra8,
            frame_rate,
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
    Ok(modes)
}

fn reader_for(source: &IMFMediaSource) -> Result<IMFSourceReader> {
    let mut attributes: Option<IMFAttributes> = None;
    // SAFETY: writes the new attribute store into a live out-slot.
    unsafe { MFCreateAttributes(&mut attributes, 1) }.map_err(err)?;
    let attributes = attributes.ok_or_else(|| unsupported("describe a reader"))?;
    // SAFETY: a setter on the store just created; without it the reader refuses BGRA, which no webcam produces natively.
    unsafe {
        attributes.SetUINT32(
            &MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING,
            u32::from(true),
        )
    }
    .map_err(err)?;
    // SAFETY: both the source and the attributes are live for the call.
    unsafe { MFCreateSourceReaderFromMediaSource(source, &attributes) }.map_err(err)
}

/// Open the device whose symbolic link is `id`.
fn activate_by_id(id: &CameraId) -> Result<IMFMediaSource> {
    let found = activations()?;
    let activate = found
        .iter()
        .find(|activate| symbolic_link(activate).as_deref() == Some(id.0.as_str()))
        .ok_or_else(|| CaptureError::NotFoundNamed {
            kind: "camera",
            id: id.0.clone(),
        })?;
    // SAFETY: activates the device this activation object describes.
    unsafe { activate.ActivateObject() }.map_err(err)
}

/// Ask the reader for BGRA at `size`, and report what it settled on.
fn negotiate(reader: &IMFSourceReader, size: (u32, u32)) -> Result<(u32, u32)> {
    // SAFETY: a plain allocation, taking no arguments to get wrong.
    let wanted: IMFMediaType = unsafe { MFCreateMediaType() }.map_err(err)?;
    // SAFETY: setters on the media type just created.
    unsafe {
        wanted
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .and_then(|()| wanted.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32))
            .and_then(|()| wanted.SetUINT64(&MF_MT_FRAME_SIZE, pack_pair(size.0, size.1)))
            // RGB32 defaults to bottom-up, which is how a camera preview ends up upside down; a positive stride gets top-down.
            .and_then(|()| wanted.SetUINT32(&MF_MT_DEFAULT_STRIDE, size.0.saturating_mul(4)))
    }
    .map_err(err)?;
    // SAFETY: both the reader and the requested type are live for the call.
    unsafe { reader.SetCurrentMediaType(VIDEO_STREAM, None, &wanted) }.map_err(err)?;

    // SAFETY: reads back the type the reader settled on above.
    let settled = unsafe { reader.GetCurrentMediaType(VIDEO_STREAM) }.map_err(err)?;
    // SAFETY: a documented UINT64 attribute on that live type.
    let packed = unsafe { settled.GetUINT64(&MF_MT_FRAME_SIZE) }.map_err(err)?;
    let (width, height) = unpack_pair(packed);
    if width == 0 || height == 0 {
        return Err(unsupported("open a camera that reports no frame size"));
    }
    Ok((width, height))
}

/// What the worker thread reports back once the device is open, or why it is not.
type Opened = Result<SourceDesc>;

/// A camera stream read on a thread of its own, exposed to the caller only as a [`FrameSlot`].
/// `ReadSample` blocks until the device produces a frame and nothing else may touch the reader meanwhile, so the reader is created on the worker and never leaves it.
pub(crate) struct MfCameraSource {
    desc: SourceDesc,
    slot: Arc<FrameSlot>,
    stopping: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    /// The buffer handed to the caller, swapped out of the slot so the worker
    /// cannot reallocate it while the caller reads it.
    current: Vec<u8>,
    seen: u64,
    stride: u32,
}

impl MfCameraSource {
    pub(crate) fn open(id: &CameraId, opts: &OpenOptions) -> Result<Self> {
        ensure_started()?;
        let size = opts
            .region
            .map_or(DEFAULT_SIZE, |region| (region.width, region.height));
        let slot = Arc::new(FrameSlot::default());
        let stopping = Arc::new(AtomicBool::new(false));
        let (report, opened) = mpsc::channel::<Opened>();

        let worker = {
            let id = id.clone();
            let slot = Arc::clone(&slot);
            let stopping = Arc::clone(&stopping);
            let frame_rate = opts.frame_rate();
            std::thread::Builder::new()
                .name("capturekit-camera".into())
                .spawn(move || run(&id, size, frame_rate, &report, &slot, &stopping))
                .map_err(|error| CaptureError::backend(BACKEND, error))?
        };

        // A worker that dies before reporting drops the sender, so this ends instead of waiting on a device that never opens.
        let desc = match opened.recv() {
            Ok(result) => result,
            Err(_) => Err(unsupported("start a camera capture thread")),
        };
        match desc {
            Ok(desc) => Ok(Self {
                stride: desc.width.saturating_mul(4),
                desc,
                slot,
                stopping,
                worker: Some(worker),
                current: Vec::new(),
                seen: 0,
            }),
            Err(failure) => {
                stopping.store(true, Ordering::Relaxed);
                let _ = worker.join();
                Err(failure)
            }
        }
    }
}

/// Open the device, report the result, then pump frames until asked to stop.
fn run(
    id: &CameraId,
    size: (u32, u32),
    frame_rate: Option<u32>,
    report: &mpsc::Sender<Opened>,
    slot: &FrameSlot,
    stopping: &AtomicBool,
) {
    // Declared first so COM outlives every object created under it.
    let _com = ComScope::mta();
    let opened = activate_by_id(id).and_then(|source| {
        // Guarded from here on: a reader or negotiation failure below must not leave the device streaming.
        let source = ActiveSource(source);
        let reader = reader_for(&source.0)?;
        let (width, height) = negotiate(&reader, size)?;
        Ok((source, reader, width, height))
    });
    let (source, reader, width, height) = match opened {
        Ok((source, reader, width, height)) => {
            let desc = SourceDesc {
                width,
                height,
                format: PixelFormat::Bgra8,
                // Webcams deliver limited-range BT.601, already expanded to full-range RGB by the reader's video processor.
                color_space: ColorSpace::SRGB,
                rotation: Rotation::None,
                scale_factor: 1.0,
                frame_rate,
                backend: BACKEND,
            };
            let _ = report.send(Ok(desc));
            (source, reader, width, height)
        }
        Err(failure) => {
            slot.end();
            let _ = report.send(Err(failure));
            return;
        }
    };

    let mut scratch = Vec::new();
    while !stopping.load(Ordering::Relaxed) {
        match read_one(&reader, width, height, slot, &mut scratch) {
            Ok(true) => {}
            // End of stream: unplugged and taken-by-another-process both surface this way, and Windows doesn't say which.
            Ok(false) => {
                log::info!("camera stream ended");
                break;
            }
            Err(error) => {
                log::warn!("camera read failed: {error}");
                break;
            }
        }
    }
    slot.end();
    drop(source);
}

/// Read one sample, publishing it. `false` means the stream ended.
fn read_one(
    reader: &IMFSourceReader,
    width: u32,
    height: u32,
    slot: &FrameSlot,
    scratch: &mut Vec<u8>,
) -> Result<bool, windows::core::Error> {
    let mut flags = 0u32;
    let mut sample = None;
    // SAFETY: every out-parameter is a live local.
    unsafe {
        reader.ReadSample(
            VIDEO_STREAM,
            0,
            None,
            Some(&mut flags),
            None,
            Some(&mut sample),
        )
    }?;
    if flags & MF_SOURCE_READERF_ERROR.0 as u32 != 0
        || flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0
    {
        return Ok(false);
    }
    // A null sample with no end-of-stream flag means 'nothing yet'; it happens around a format change.
    let Some(sample) = sample else {
        return Ok(true);
    };
    // SAFETY: flattens the live sample above into one buffer.
    let buffer = unsafe { sample.ConvertToContiguousBuffer() }?;

    // A 2D buffer knows its own pitch; a plain one is packed at the width.
    let (base, stride) = match buffer.cast::<IMF2DBuffer>() {
        Ok(flat) => {
            let mut scanline = core::ptr::null_mut();
            let mut pitch = 0i32;
            // SAFETY: both out-parameters are live locals, and the matching unlock runs below.
            unsafe { flat.Lock2D(&mut scanline, &mut pitch) }?;
            (scanline, pitch)
        }
        Err(_) => {
            let mut data = core::ptr::null_mut();
            let mut length = 0u32;
            // SAFETY: both out-parameters are live locals, and the matching unlock runs below.
            unsafe { buffer.Lock(&mut data, None, Some(&mut length)) }?;
            (data, width.saturating_mul(4) as i32)
        }
    };

    if !base.is_null() && stride != 0 {
        let row_bytes = width.saturating_mul(4) as usize;
        // SAFETY: the buffer stays locked until the unlock below, so it addresses `height` rows of the reported pitch.
        unsafe { gather_rows(scratch, base, stride, row_bytes, height) };
        slot.publish(
            Delivered {
                // The shared clock, not the camera's own sample time, which counts from when its stream started.
                pts: super::now(),
                stride: row_bytes as u32,
                width,
                height,
            },
            scratch,
        );
    }

    match buffer.cast::<IMF2DBuffer>() {
        // SAFETY: pairs with the 2D lock taken above.
        Ok(flat) => unsafe { flat.Unlock2D() }?,
        // SAFETY: pairs with the plain lock taken above.
        Err(_) => unsafe { buffer.Unlock() }?,
    }
    Ok(true)
}

/// Copies `height` scanlines top row first, whichever way MF laid them out; `Lock2D` returns the FIRST scanline, not the lowest address.
/// # Safety: `scanline0` must address `height` rows of `row_bytes` readable bytes, each `pitch` from the last.
unsafe fn gather_rows(
    out: &mut Vec<u8>,
    scanline0: *const u8,
    pitch: i32,
    row_bytes: usize,
    height: u32,
) {
    out.clear();
    out.reserve(row_bytes.saturating_mul(height as usize));
    for row in 0..height as isize {
        // SAFETY: the caller guarantees `height` rows of `pitch` bytes are addressable.
        let line = unsafe { scanline0.offset(row * pitch as isize) };
        // SAFETY: each row holds at least `row_bytes`, which the caller derived from the width.
        out.extend_from_slice(unsafe { core::slice::from_raw_parts(line, row_bytes) });
    }
}

impl Drop for MfCameraSource {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

impl FrameSource for MfCameraSource {
    fn describe(&self) -> &SourceDesc {
        &self.desc
    }

    fn region(&self) -> Option<Rect> {
        // The reader scales to the requested size rather than cropping, so a region is a resolution request.
        None
    }

    fn next_frame(&mut self, timeout: Duration) -> Result<RawFrame<'_>> {
        let meta = self.slot.take(timeout, &mut self.seen, &mut self.current)?;
        self.desc.width = meta.width;
        self.desc.height = meta.height;
        self.stride = meta.stride;
        Ok(RawFrame {
            pts: meta.pts,
            bytes: &self.current,
            stride: self.stride,
            // A camera repaints its whole sensor every frame.
            dirty: DirtyRects::unknown(),
            cursor: None,
            gpu: None,
        })
    }

    fn stop(&mut self) -> Result<()> {
        self.stopping.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        Ok(())
    }
}

/// Kept so the process can release Media Foundation deliberately, which only a
/// host shutting down should ever do.
#[allow(dead_code)]
pub(crate) fn shutdown() {
    // SAFETY: pairs with the one-time startup above, at process exit.
    let _ = unsafe { MFShutdown() };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_packed_pair_round_trips_through_the_uint64_mf_stores_it_in() {
        assert_eq!(unpack_pair(pack_pair(1920, 1080)), (1920, 1080));
    }

    /// The half that silently truncates if the low word is masked wrong: 4K is
    /// past the point where a signed or narrowed shift starts losing bits.
    #[test]
    fn a_frame_size_larger_than_a_signed_word_survives_unpacking() {
        assert_eq!(unpack_pair(pack_pair(3840, 2160)), (3840, 2160));
        assert_eq!(unpack_pair(pack_pair(0, u32::MAX)), (0, u32::MAX));
    }

    #[test]
    fn a_null_string_attribute_reads_as_empty_rather_than_panicking() {
        assert_eq!(take_string(PWSTR::null()), "");
    }

    /// Rows numbered top to bottom, laid out in that order.
    const ROWS: usize = 4;
    const ROW_BYTES: usize = 3;

    fn ladder() -> Vec<u8> {
        (0..ROWS as u8).flat_map(|row| [row; ROW_BYTES]).collect()
    }

    #[test]
    fn a_positive_pitch_reads_the_rows_in_the_order_memory_holds_them() {
        let source = ladder();
        let mut out = Vec::new();
        // SAFETY: the fixture holds exactly ROWS rows of ROW_BYTES, which is what is passed.
        unsafe {
            gather_rows(
                &mut out,
                source.as_ptr(),
                ROW_BYTES as i32,
                ROW_BYTES,
                ROWS as u32,
            );
        };
        assert_eq!(out, source);
    }

    /// The bug this exists to prevent: a bottom-up frame handed back in memory
    /// order is an upside-down camera preview that passes every size check.
    #[test]
    fn a_negative_pitch_reads_the_rows_back_into_top_down_order() {
        let source = ladder();
        // SAFETY: the fixture holds ROWS rows, so the last one, which Lock2D reports first for a negative pitch, starts inside it.
        let first = unsafe { source.as_ptr().add((ROWS - 1) * ROW_BYTES) };
        let mut out = Vec::new();
        // SAFETY: a negative pitch walks back from the last row, which is where `first` points.
        unsafe {
            gather_rows(&mut out, first, -(ROW_BYTES as i32), ROW_BYTES, ROWS as u32);
        };
        let expected: Vec<u8> = (0..ROWS as u8)
            .rev()
            .flat_map(|row| [row; ROW_BYTES])
            .collect();
        assert_eq!(out, expected);
    }

    /// A stride wider than the row: the padding must not travel with the pixels.
    #[test]
    fn padding_between_rows_is_left_behind() {
        let padded: Vec<u8> = (0..ROWS as u8)
            .flat_map(|row| [row, row, row, 0xFF, 0xFF])
            .collect();
        let mut out = Vec::new();
        // SAFETY: the fixture holds ROWS rows of five bytes, which is the stride passed.
        unsafe { gather_rows(&mut out, padded.as_ptr(), 5, ROW_BYTES, ROWS as u32) };
        assert_eq!(out, ladder());
    }
}
