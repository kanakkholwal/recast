//! V4L2 camera capture over memory-mapped streaming: the only path with per-frame timestamps, and it copies once instead of twice.
//! Frames convert to BGRA to match the other platforms; compressed modes are refused, since decoding belongs in a codec.

mod convert;
mod uapi;

use core::time::Duration;
use std::ffi::CString;
use std::io;
use std::os::unix::io::RawFd;
use std::time::Instant;

use capturekit_core::{
    Camera, CameraFormat, CameraId, CaptureError, ColorSpace, DirtyRects, LostReason, PixelFormat,
    Rect, Result, Rotation, SourceDesc, Timestamp,
};

use crate::backend::{FrameSource, RawFrame};
use crate::platform::OpenOptions;
use convert::{layout_of, Layout, SUPPORTED};
use uapi::{fourcc_name, ioctl, Zeroable};

pub(super) const BACKEND: &str = "v4l2";

/// What is asked for when the caller names no size.
const DEFAULT_SIZE: (u32, u32) = (1280, 720);

/// Buffers the driver fills while the consumer holds one.
/// Four is the usual choice: enough that a slow consumer does not starve the driver, few enough that a stalled consumer cannot build a second of latency.
const BUFFER_COUNT: u32 = 4;

/// The rate a mode has to reach before its extra pixels are worth taking.
/// Webcams commonly offer 1280x720 at 10fps and 640x480 at 30fps in the same uncompressed format. Ranking purely by size picks the judder.
const MIN_USEFUL_FPS: f32 = 15.0;

/// Cap on every `VIDIOC_ENUM_*` walk, so a driver that never returns EINVAL
/// cannot spin here forever.
const MAX_ENUM: u32 = 128;

/// An owned file descriptor for a video node.
struct Fd(RawFd);

impl Drop for Fd {
    fn drop(&mut self) {
        // SAFETY: this type is the only owner of the descriptor.
        unsafe { libc::close(self.0) };
    }
}

/// One mmap'd capture buffer.
struct Mapping {
    ptr: *mut libc::c_void,
    len: usize,
}

impl Mapping {
    /// The first `used` bytes the driver reported as filled.
    fn filled(&self, used: usize) -> &[u8] {
        // SAFETY: the mapping is `len` bytes and outlives the slice, and the read is clamped.
        unsafe { core::slice::from_raw_parts(self.ptr.cast::<u8>(), used.min(self.len)) }
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: this type owns the mapping it is dropping.
        unsafe { libc::munmap(self.ptr, self.len) };
    }
}

fn io_failed(path: &str, action: &str, err: &io::Error) -> CaptureError {
    CaptureError::backend(
        BACKEND,
        io::Error::new(err.kind(), format!("{path}: {action}: {err}")),
    )
}

/// A device error, classified so the caller knows whether to retry.
/// EBUSY is recoverable because it is nearly always the previous holder letting go; a missing node never is, so an unplugged camera fails at once.
fn device_error(path: &str, action: &str, err: &io::Error) -> CaptureError {
    match err.raw_os_error() {
        Some(libc::EBUSY) => CaptureError::Lost(LostReason::AccessLost),
        Some(libc::ENODEV | libc::ENXIO) => CaptureError::NotFoundNamed {
            kind: "camera",
            id: path.to_string(),
        },
        // The fix is a group membership, not a system prompt, so the hint beats the type.
        Some(libc::EACCES) => io_failed(
            path,
            &format!("{action} (is this user in the \"video\" group?)"),
            err,
        ),
        _ => io_failed(path, action, err),
    }
}

fn open_node(path: &str) -> Result<Fd> {
    let name = CString::new(path).map_err(|_| CaptureError::NotFoundNamed {
        kind: "camera",
        id: path.to_string(),
    })?;
    // Non-blocking so a dequeue can drain the queue; the waiting is a poll instead.
    let flags = libc::O_RDWR | libc::O_NONBLOCK | libc::O_CLOEXEC;
    // SAFETY: `name` is a live NUL-terminated path.
    let fd = unsafe { libc::open(name.as_ptr(), flags) };
    if fd < 0 {
        return Err(device_error(path, "open", &io::Error::last_os_error()));
    }
    Ok(Fd(fd))
}

/// Every `/dev/videoN` on the system, in node order.
fn video_nodes() -> Vec<String> {
    let Ok(entries) = std::fs::read_dir("/dev") else {
        return Vec::new();
    };
    let mut nodes: Vec<(u32, String)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let index = name.strip_prefix("video")?.parse().ok()?;
            Some((index, format!("/dev/{name}")))
        })
        .collect();
    nodes.sort_unstable();
    nodes.into_iter().map(|(_, path)| path).collect()
}

/// The FourCCs a node offers for video capture, in the driver's own order.
fn offered_formats(fd: RawFd) -> Vec<u32> {
    let mut found = Vec::new();
    for index in 0..MAX_ENUM {
        let mut desc = uapi::FmtDesc::zeroed();
        desc.index = index;
        desc.ty = uapi::BUF_TYPE_VIDEO_CAPTURE;
        if ioctl(fd, uapi::ENUM_FMT, &mut desc).is_err() {
            break;
        }
        found.push(desc.pixelformat);
    }
    found
}

/// The discrete sizes a format offers, or the largest of a stepwise range.
fn frame_sizes(fd: RawFd, pixel_format: u32) -> Vec<(u32, u32)> {
    let mut sizes = Vec::new();
    for index in 0..MAX_ENUM {
        let mut probe = uapi::FrameSizeEnum::zeroed();
        probe.index = index;
        probe.pixel_format = pixel_format;
        if ioctl(fd, uapi::ENUM_FRAMESIZES, &mut probe).is_err() {
            break;
        }
        if probe.ty == uapi::FRMSIZE_TYPE_DISCRETE {
            sizes.push((probe.size[0], probe.size[1]));
            continue;
        }
        // A stepwise range describes thousands of sizes; only its largest is worth listing.
        sizes.push((probe.size[1], probe.size[4]));
        break;
    }
    sizes.retain(|(w, h)| *w > 0 && *h > 0);
    sizes
}

/// The highest frame rate a mode reports, where the driver reports any.
fn best_frame_rate(fd: RawFd, pixel_format: u32, width: u32, height: u32) -> Option<f32> {
    let mut best: Option<f32> = None;
    for index in 0..MAX_ENUM {
        let mut probe = uapi::FrameIvalEnum::zeroed();
        probe.index = index;
        probe.pixel_format = pixel_format;
        probe.width = width;
        probe.height = height;
        if ioctl(fd, uapi::ENUM_FRAMEINTERVALS, &mut probe).is_err() {
            break;
        }
        // Both shapes put their fastest interval first, so one read serves each.
        let (numerator, denominator) = (probe.interval[0], probe.interval[1]);
        if numerator == 0 || denominator == 0 {
            break;
        }
        let fps = denominator as f32 / numerator as f32;
        best = Some(best.map_or(fps, |best: f32| best.max(fps)));
        if probe.ty != uapi::FRMIVAL_TYPE_DISCRETE {
            break;
        }
    }
    best
}

/// The modes one node advertises that this backend can actually read, largest
/// first, reported as the BGRA it delivers rather than the device's own FourCC.
fn modes(fd: RawFd) -> Vec<CameraFormat> {
    let mut modes: Vec<CameraFormat> = Vec::new();
    for pixel_format in offered_formats(fd) {
        if layout_of(pixel_format).is_none() {
            continue;
        }
        for (width, height) in frame_sizes(fd, pixel_format) {
            let mode = CameraFormat {
                width,
                height,
                pixel_format: PixelFormat::Bgra8,
                frame_rate: best_frame_rate(fd, pixel_format, width, height),
            };
            if !modes.contains(&mode) {
                modes.push(mode);
            }
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

/// Whether a node is one this backend would open, by its own reported caps.
fn is_capture_node(caps: &uapi::Capability) -> bool {
    let node = caps.node_caps();
    node & uapi::CAP_VIDEO_CAPTURE != 0 && node & uapi::CAP_STREAMING != 0
}

pub(crate) fn cameras() -> Result<Vec<Camera>> {
    let mut cameras: Vec<Camera> = Vec::new();
    let mut buses: Vec<String> = Vec::new();
    for path in video_nodes() {
        let Ok(fd) = open_node(&path) else {
            continue;
        };
        let mut caps = uapi::Capability::zeroed();
        if ioctl(fd.0, uapi::QUERYCAP, &mut caps).is_err() || !is_capture_node(&caps) {
            continue;
        }
        // One camera can expose several nodes sharing a bus address; only the first streams.
        let bus = uapi::cstr(&caps.bus_info);
        if !bus.is_empty() && buses.contains(&bus) {
            continue;
        }
        buses.push(bus);
        cameras.push(Camera {
            id: CameraId(path),
            name: uapi::cstr(&caps.card),
            is_default: false,
            formats: modes(fd.0),
        });
    }
    if let Some(first) = cameras.first_mut() {
        first.is_default = true;
    }
    Ok(cameras)
}

/// Whether the calling user can open a camera at all.
/// Linux gates cameras on file permissions rather than a prompt, so there is nothing to request: a node the user cannot read is a `video` group problem.
pub(crate) fn permission() -> capturekit_core::Permission {
    use capturekit_core::Permission;
    let nodes = video_nodes();
    if nodes.is_empty() {
        return Permission::NotRequired;
    }
    let readable = nodes.iter().any(|path| {
        let Ok(name) = CString::new(path.as_str()) else {
            return false;
        };
        // SAFETY: `name` is a live NUL-terminated path.
        unsafe { libc::access(name.as_ptr(), libc::R_OK | libc::W_OK) == 0 }
    });
    if readable {
        Permission::NotRequired
    } else {
        Permission::Denied
    }
}

/// The mode to open, preferring the largest that still reaches the target rate: a webcam offering 720p10 and 480p30 offers the second for a reason.
/// Falls back to the fastest that fits, then the smallest available, so a device whose every mode exceeds the request still opens.
fn choose_size(
    sizes: &[((u32, u32), Option<f32>)],
    want: (u32, u32),
    fps: f32,
) -> Option<(u32, u32)> {
    let fits: Vec<_> = sizes
        .iter()
        .filter(|((w, h), _)| *w <= want.0 && *h <= want.1)
        .collect();
    let fast = fits
        .iter()
        .filter(|(_, rate)| rate.is_none_or(|rate| rate >= fps))
        .max_by_key(|((w, h), _)| u64::from(*w) * u64::from(*h));
    if let Some(((w, h), _)) = fast {
        return Some((*w, *h));
    }
    let best_rate = fits.iter().max_by(|a, b| {
        a.1.unwrap_or_default()
            .total_cmp(&b.1.unwrap_or_default())
            .then(
                (u64::from(a.0 .0) * u64::from(a.0 .1))
                    .cmp(&(u64::from(b.0 .0) * u64::from(b.0 .1))),
            )
    });
    if let Some(((w, h), _)) = best_rate {
        return Some((*w, *h));
    }
    sizes
        .iter()
        .min_by_key(|((w, h), _)| u64::from(*w) * u64::from(*h))
        .map(|((w, h), _)| (*w, *h))
}

/// A subsampled format has no way to spell an odd edge, so the odd row or column
/// is dropped rather than half-read off the end of a line.
fn even_size(layout: Layout, width: u32, height: u32) -> (u32, u32) {
    if layout.is_subsampled() {
        (width & !1, height & !1)
    } else {
        (width, height)
    }
}

fn no_readable_format(path: &str, offered: &[u32]) -> CaptureError {
    let offered = offered
        .iter()
        .map(|code| fourcc_name(*code))
        .collect::<Vec<_>>()
        .join(", ");
    let readable = SUPPORTED
        .iter()
        .map(|(code, _)| fourcc_name(*code))
        .collect::<Vec<_>>()
        .join(", ");
    CaptureError::backend(
        BACKEND,
        io::Error::other(format!(
            "{path} offers only {offered}; this backend reads uncompressed formats ({readable})"
        )),
    )
}

/// What the driver settled on after `VIDIOC_S_FMT`.
struct Negotiated {
    layout: Layout,
    width: u32,
    height: u32,
    stride: u32,
    full_range: bool,
    image_len: usize,
}

fn negotiate(fd: RawFd, path: &str, want: (u32, u32), fps: f32) -> Result<Negotiated> {
    let offered = offered_formats(fd);
    let pixel_format = SUPPORTED
        .iter()
        .map(|(code, _)| *code)
        .find(|code| offered.contains(code))
        .ok_or_else(|| no_readable_format(path, &offered))?;

    let sizes: Vec<_> = frame_sizes(fd, pixel_format)
        .into_iter()
        .map(|size| (size, best_frame_rate(fd, pixel_format, size.0, size.1)))
        .collect();
    let (width, height) = choose_size(&sizes, want, fps).unwrap_or(want);

    let mut format = uapi::Format::zeroed();
    format.ty = uapi::BUF_TYPE_VIDEO_CAPTURE;
    format.pix.width = width;
    format.pix.height = height;
    format.pix.pixelformat = pixel_format;
    format.pix.field = uapi::FIELD_NONE;
    ioctl(fd, uapi::S_FMT, &mut format).map_err(|e| device_error(path, "set the format", &e))?;

    // S_FMT answers with what the driver ACTUALLY set, so everything below reads it back.
    let pix = format.pix;
    let layout =
        layout_of(pix.pixelformat).ok_or_else(|| no_readable_format(path, &[pix.pixelformat]))?;
    let (width, height) = even_size(layout, pix.width, pix.height);
    let stride = pix.bytesperline.max(layout.min_stride(width));
    Ok(Negotiated {
        layout,
        width,
        height,
        stride,
        full_range: pix.quantization == uapi::QUANTIZATION_FULL_RANGE,
        image_len: pix.sizeimage as usize,
    })
}

/// Ask for a frame interval, which most drivers honour and some ignore.
fn set_frame_rate(fd: RawFd, fps: u32) {
    if fps == 0 {
        return;
    }
    let mut parm = uapi::StreamParm::zeroed();
    parm.ty = uapi::BUF_TYPE_VIDEO_CAPTURE;
    parm.numerator = 1;
    parm.denominator = fps;
    if ioctl(fd, uapi::S_PARM, &mut parm).is_err() {
        return;
    }
    if parm.capability & uapi::CAP_TIMEPERFRAME == 0 {
        log::debug!("{BACKEND}: device does not support setting a frame interval");
    }
}

/// A camera stream over memory-mapped V4L2 buffers.
pub(crate) struct V4l2CameraSource {
    path: String,
    fd: Fd,
    buffers: Vec<Mapping>,
    layout: Layout,
    stride: u32,
    full_range: bool,
    desc: SourceDesc,
    frame: Vec<u8>,
    streaming: bool,
}

// SAFETY: the mappings and descriptor are owned here and only touched through `&mut self`.
unsafe impl Send for V4l2CameraSource {}

impl V4l2CameraSource {
    pub(crate) fn open(id: &CameraId, opts: &OpenOptions) -> Result<Self> {
        let path = id.0.clone();
        let fd = open_node(&path)?;
        let mut caps = uapi::Capability::zeroed();
        ioctl(fd.0, uapi::QUERYCAP, &mut caps)
            .map_err(|e| device_error(&path, "query the device", &e))?;
        if !is_capture_node(&caps) {
            return Err(CaptureError::NotFoundNamed {
                kind: "camera",
                id: path,
            });
        }

        let want = opts
            .region
            .map_or(DEFAULT_SIZE, |region| (region.width, region.height));
        let fps = opts.frame_rate();
        let format = negotiate(
            fd.0,
            &path,
            want,
            fps.map_or(MIN_USEFUL_FPS, |fps| fps as f32),
        )?;
        if let Some(fps) = fps {
            set_frame_rate(fd.0, fps);
        }
        log::info!(
            "{BACKEND}: {path} opened at {}x{} as {:?}",
            format.width,
            format.height,
            format.layout
        );

        let mut source = Self {
            path,
            fd,
            buffers: Vec::new(),
            layout: format.layout,
            stride: format.stride,
            full_range: format.full_range,
            desc: SourceDesc {
                width: format.width,
                height: format.height,
                format: PixelFormat::Bgra8,
                color_space: ColorSpace::SRGB,
                rotation: Rotation::None,
                scale_factor: 1.0,
                frame_rate: fps,
                backend: BACKEND,
            },
            frame: Vec::new(),
            streaming: false,
        };
        source.start(format.image_len)?;
        Ok(source)
    }

    /// Map the driver's buffers, hand them all back, and start the stream.
    fn start(&mut self, image_len: usize) -> Result<()> {
        let mut request = uapi::RequestBuffers::zeroed();
        request.count = BUFFER_COUNT;
        request.ty = uapi::BUF_TYPE_VIDEO_CAPTURE;
        request.memory = uapi::MEMORY_MMAP;
        ioctl(self.fd.0, uapi::REQBUFS, &mut request)
            .map_err(|e| device_error(&self.path, "request buffers", &e))?;
        if request.count == 0 {
            return Err(io_failed(
                &self.path,
                "request buffers",
                &io::Error::other("the driver granted none"),
            ));
        }

        for index in 0..request.count {
            let mut buffer = uapi::Buffer::zeroed();
            buffer.index = index;
            buffer.ty = uapi::BUF_TYPE_VIDEO_CAPTURE;
            buffer.memory = uapi::MEMORY_MMAP;
            ioctl(self.fd.0, uapi::QUERYBUF, &mut buffer)
                .map_err(|e| device_error(&self.path, "query a buffer", &e))?;
            let len = (buffer.length as usize).max(image_len);
            // SAFETY: the driver just gave this offset and length; a failed map is never used.
            let ptr = unsafe {
                libc::mmap(
                    core::ptr::null_mut(),
                    len,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_SHARED,
                    self.fd.0,
                    i64::from(buffer.offset),
                )
            };
            if ptr == libc::MAP_FAILED {
                return Err(device_error(
                    &self.path,
                    "map a buffer",
                    &io::Error::last_os_error(),
                ));
            }
            self.buffers.push(Mapping { ptr, len });
            self.enqueue(index)?;
        }

        let mut ty = uapi::BUF_TYPE_VIDEO_CAPTURE as i32;
        ioctl(self.fd.0, uapi::STREAMON, &mut ty)
            .map_err(|e| device_error(&self.path, "start streaming", &e))?;
        self.streaming = true;
        Ok(())
    }

    fn enqueue(&self, index: u32) -> Result<()> {
        let mut buffer = uapi::Buffer::zeroed();
        buffer.index = index;
        buffer.ty = uapi::BUF_TYPE_VIDEO_CAPTURE;
        buffer.memory = uapi::MEMORY_MMAP;
        ioctl(self.fd.0, uapi::QBUF, &mut buffer)
            .map_err(|e| device_error(&self.path, "queue a buffer", &e))
    }

    /// Take one filled buffer, or `None` when the driver has nothing ready.
    fn dequeue(&self) -> Result<Option<uapi::Buffer>> {
        let mut buffer = uapi::Buffer::zeroed();
        buffer.ty = uapi::BUF_TYPE_VIDEO_CAPTURE;
        buffer.memory = uapi::MEMORY_MMAP;
        match ioctl(self.fd.0, uapi::DQBUF, &mut buffer) {
            Ok(()) => Ok(Some(buffer)),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(err) => Err(device_error(&self.path, "dequeue a buffer", &err)),
        }
    }

    /// The newest filled buffer, returning the older ones to the driver.
    /// A consumer slower than the device would otherwise walk a queue of stale frames, each one a frame further behind than the last.
    fn dequeue_newest(&self, first: uapi::Buffer) -> Result<uapi::Buffer> {
        let mut newest = first;
        loop {
            match self.dequeue() {
                Ok(Some(next)) => {
                    self.enqueue(newest.index)?;
                    newest = next;
                }
                Ok(None) => return Ok(newest),
                // The buffer goes back, or the driver is one short from here on.
                Err(e) => {
                    let _ = self.enqueue(newest.index);
                    return Err(e);
                }
            }
        }
    }

    fn wait_readable(&self, timeout: Duration) -> Result<()> {
        let mut poll = libc::pollfd {
            fd: self.fd.0,
            events: libc::POLLIN,
            revents: 0,
        };
        let millis = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
        loop {
            // SAFETY: one live pollfd, and the count matches.
            let ready = unsafe { libc::poll(core::ptr::from_mut(&mut poll), 1, millis) };
            if ready < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(device_error(&self.path, "wait for a frame", &err));
            }
            if ready == 0 {
                return Err(CaptureError::Timeout(timeout));
            }
            // A video node reports an unplugged device as a poll error, not a failing ioctl.
            if poll.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                return Err(CaptureError::Lost(LostReason::AccessLost));
            }
            return Ok(());
        }
    }
}

/// When the driver says the frame was captured, on `CLOCK_MONOTONIC`.
/// Older drivers stamp buffers with wall-clock time, which cannot be compared with anything else in a session; those fall back to the moment of dequeue.
// `time_t` is 32-bit on some Linux targets, where this widening is not useless.
#[allow(clippy::useless_conversion)]
fn stamp(buffer: &uapi::Buffer) -> Timestamp {
    if buffer.flags & uapi::BUF_FLAG_TIMESTAMP_MASK != uapi::BUF_FLAG_TIMESTAMP_MONOTONIC {
        return super::now();
    }
    let seconds = i64::from(buffer.timestamp.tv_sec);
    let micros = i64::from(buffer.timestamp.tv_usec);
    Timestamp::from_micros(seconds.saturating_mul(1_000_000) + micros)
}

impl Drop for V4l2CameraSource {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

impl FrameSource for V4l2CameraSource {
    fn describe(&self) -> &SourceDesc {
        &self.desc
    }

    fn region(&self) -> Option<Rect> {
        // A camera is opened in a mode rather than cropped, so a region was a size request.
        None
    }

    fn next_frame(&mut self, timeout: Duration) -> Result<RawFrame<'_>> {
        let deadline = Instant::now() + timeout;
        let buffer = loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(CaptureError::Timeout(timeout));
            }
            self.wait_readable(left)?;
            let Some(first) = self.dequeue()? else {
                continue;
            };
            let newest = self.dequeue_newest(first)?;
            // A buffer flagged as an error holds no usable pixels, so it goes straight back.
            if newest.flags & uapi::BUF_FLAG_ERROR != 0 {
                self.enqueue(newest.index)?;
                continue;
            }
            break newest;
        };

        let Self {
            buffers,
            frame,
            desc,
            ..
        } = self;
        let mapping = buffers
            .get(buffer.index as usize)
            .ok_or(CaptureError::Lost(LostReason::AccessLost))?;
        let result = convert::to_bgra(
            self.layout,
            mapping.filled(buffer.bytesused as usize),
            desc.width,
            desc.height,
            self.stride,
            self.full_range,
            frame,
        );
        // The buffer goes back whatever the conversion did, or the driver runs dry.
        self.enqueue(buffer.index)?;
        result?;

        Ok(RawFrame {
            pts: stamp(&buffer),
            bytes: &self.frame,
            stride: self.desc.width * 4,
            // A camera repaints its whole sensor every frame.
            dirty: DirtyRects::unknown(),
            cursor: None,
            gpu: None,
        })
    }

    fn stop(&mut self) -> Result<()> {
        if self.streaming {
            self.streaming = false;
            let mut ty = uapi::BUF_TYPE_VIDEO_CAPTURE as i32;
            // STREAMOFF first: unmapping a buffer the driver still owns is a use-after-free.
            let _ = ioctl(self.fd.0, uapi::STREAMOFF, &mut ty);
        }
        self.buffers.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modes() -> Vec<((u32, u32), Option<f32>)> {
        vec![
            ((1280, 720), Some(10.0)),
            ((640, 480), Some(30.0)),
            ((320, 240), Some(30.0)),
        ]
    }

    /// The whole point of the rate filter: 720p at 10fps judders, and the device
    /// offers 480p at 30 for exactly this reason.
    #[test]
    fn a_larger_mode_is_passed_over_when_it_cannot_reach_the_rate() {
        assert_eq!(choose_size(&modes(), (1280, 720), 30.0), Some((640, 480)));
    }

    #[test]
    fn the_largest_fitting_mode_wins_when_every_mode_reaches_the_rate() {
        assert_eq!(choose_size(&modes(), (1280, 720), 10.0), Some((1280, 720)));
    }

    /// Nothing reaches 60, so the fastest that fits is better than nothing.
    #[test]
    fn an_unreachable_rate_falls_back_to_the_fastest_mode_that_fits() {
        assert_eq!(choose_size(&modes(), (1280, 720), 60.0), Some((640, 480)));
    }

    #[test]
    fn a_device_whose_every_mode_is_too_large_still_opens_at_its_smallest() {
        let sizes = [((1920, 1080), Some(30.0)), ((3840, 2160), Some(30.0))];
        assert_eq!(choose_size(&sizes, (640, 480), 30.0), Some((1920, 1080)));
    }

    /// A driver that reports no rate is not a driver that is slow.
    #[test]
    fn a_mode_with_no_reported_rate_is_not_treated_as_a_slow_one() {
        let sizes = [((1280, 720), None), ((640, 480), Some(30.0))];
        assert_eq!(choose_size(&sizes, (1280, 720), 30.0), Some((1280, 720)));
    }

    #[test]
    fn a_device_with_no_modes_at_all_chooses_nothing() {
        assert_eq!(choose_size(&[], (1280, 720), 30.0), None);
    }

    /// The message has to name what the device offered and what would work, or
    /// it tells the user nothing they can act on.
    /// A driver is free to answer S_FMT with an odd size, which a 4:2:2 row
    /// cannot hold: the last macropixel would read two bytes past the line.
    #[test]
    fn a_subsampled_format_is_pulled_back_to_an_even_size() {
        let yuyv = layout_of(uapi::fourcc(b"YUYV")).expect("a known layout");
        assert_eq!(even_size(yuyv, 641, 481), (640, 480));
        let bgra = layout_of(uapi::fourcc(b"AR24")).expect("a known layout");
        assert_eq!(even_size(bgra, 641, 481), (641, 481));
    }

    #[test]
    fn the_unreadable_format_error_names_both_sides() {
        let err = no_readable_format("/dev/video0", &[uapi::fourcc(b"MJPG")]).to_string();
        let source =
            std::error::Error::source(&no_readable_format("/dev/video0", &[uapi::fourcc(b"MJPG")]))
                .map(|e| e.to_string())
                .unwrap_or_default();
        assert!(err.contains(BACKEND), "{err}");
        assert!(source.contains("MJPG"), "{source}");
        assert!(source.contains("YUYV"), "{source}");
        assert!(source.contains("/dev/video0"), "{source}");
    }

    /// EBUSY has to stay recoverable: it is the previous holder letting go, and
    /// the caller's reopen is what fixes it.
    #[test]
    fn a_busy_device_is_recoverable_and_a_missing_one_is_not() {
        let busy = device_error(
            "/dev/video0",
            "open",
            &io::Error::from_raw_os_error(libc::EBUSY),
        );
        assert!(busy.is_recoverable(), "{busy}");
        let gone = device_error(
            "/dev/video0",
            "open",
            &io::Error::from_raw_os_error(libc::ENODEV),
        );
        assert!(!gone.is_recoverable(), "{gone}");
        assert!(gone.to_string().contains("/dev/video0"), "{gone}");
    }

    #[test]
    fn a_permission_failure_says_which_group_to_join() {
        let err = device_error(
            "/dev/video0",
            "open",
            &io::Error::from_raw_os_error(libc::EACCES),
        );
        let source = std::error::Error::source(&err)
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(source.contains("video"), "{source}");
    }

    #[test]
    fn a_node_that_only_carries_metadata_is_not_a_camera() {
        let mut caps = uapi::Capability::zeroed();
        caps.capabilities = uapi::CAP_VIDEO_CAPTURE | uapi::CAP_STREAMING;
        assert!(is_capture_node(&caps));
        caps.capabilities |= uapi::CAP_DEVICE_CAPS;
        caps.device_caps = uapi::CAP_STREAMING;
        assert!(!is_capture_node(&caps));
    }

    /// A monotonic stamp is the driver's; anything else is not comparable with
    /// the rest of the session and has to be replaced.
    #[test]
    fn only_a_monotonic_driver_stamp_is_trusted() {
        let mut buffer = uapi::Buffer::zeroed();
        buffer.flags = uapi::BUF_FLAG_TIMESTAMP_MONOTONIC;
        buffer.timestamp.tv_sec = 2;
        buffer.timestamp.tv_usec = 500_000;
        assert_eq!(stamp(&buffer).as_nanos(), 2_500_000_000);
        buffer.flags = 0;
        assert_ne!(stamp(&buffer).as_nanos(), 2_500_000_000);
    }
}
