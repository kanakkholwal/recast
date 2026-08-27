//! The slice of `linux/videodev2.h` this backend uses: structs, ioctl codes and
//! FourCCs.
//!
//! Transcribed rather than generated, so building the crate needs no kernel
//! headers and no bindgen. Every struct size and ioctl code is pinned by the
//! tests below against what a real `videodev2.h` produces, because a wrong size
//! silently changes the ioctl number and the driver answers ENOTTY.

use core::mem::size_of;
use std::io;
use std::os::unix::io::RawFd;

/// Structs that are plain data, so an all-zero one is a valid one.
///
/// V4L2 requires unused fields to be zero, so every ioctl below starts from a
/// zeroed struct rather than filling each field.
///
/// # Safety
///
/// The implementor must be inhabited by all-zero bytes: plain data, with no
/// reference, `NonNull` or enum field.
pub(super) unsafe trait Zeroable: Sized {
    fn zeroed() -> Self {
        // SAFETY: the implementor promises all-zero is a valid value.
        unsafe { core::mem::zeroed() }
    }
}

const READ: u32 = 2;
const WRITE: u32 = 1;

/// `_IOC` from `asm-generic/ioctl.h`, which is what every architecture this
/// ships on uses; PowerPC, MIPS and SPARC order the direction bits differently.
const fn code(dir: u32, nr: u32, size: usize) -> u32 {
    (dir << 30) | ((size as u32) << 16) | ((b'V' as u32) << 8) | nr
}

const fn ior<T>(nr: u32) -> u32 {
    code(READ, nr, size_of::<T>())
}

const fn iow<T>(nr: u32) -> u32 {
    code(WRITE, nr, size_of::<T>())
}

const fn iowr<T>(nr: u32) -> u32 {
    code(READ | WRITE, nr, size_of::<T>())
}

pub(super) const QUERYCAP: u32 = ior::<Capability>(0);
pub(super) const ENUM_FMT: u32 = iowr::<FmtDesc>(2);
pub(super) const S_FMT: u32 = iowr::<Format>(5);
pub(super) const REQBUFS: u32 = iowr::<RequestBuffers>(8);
pub(super) const QUERYBUF: u32 = iowr::<Buffer>(9);
pub(super) const QBUF: u32 = iowr::<Buffer>(15);
pub(super) const DQBUF: u32 = iowr::<Buffer>(17);
pub(super) const STREAMON: u32 = iow::<i32>(18);
pub(super) const STREAMOFF: u32 = iow::<i32>(19);
pub(super) const S_PARM: u32 = iowr::<StreamParm>(22);
pub(super) const ENUM_FRAMESIZES: u32 = iowr::<FrameSizeEnum>(74);
pub(super) const ENUM_FRAMEINTERVALS: u32 = iowr::<FrameIvalEnum>(75);

pub(super) const CAP_VIDEO_CAPTURE: u32 = 0x0000_0001;
pub(super) const CAP_STREAMING: u32 = 0x0400_0000;
pub(super) const CAP_DEVICE_CAPS: u32 = 0x8000_0000;
pub(super) const CAP_TIMEPERFRAME: u32 = 0x0000_1000;

pub(super) const BUF_TYPE_VIDEO_CAPTURE: u32 = 1;
pub(super) const MEMORY_MMAP: u32 = 1;
pub(super) const FIELD_NONE: u32 = 1;

pub(super) const BUF_FLAG_ERROR: u32 = 0x0000_0040;
pub(super) const BUF_FLAG_TIMESTAMP_MASK: u32 = 0x0000_e000;
pub(super) const BUF_FLAG_TIMESTAMP_MONOTONIC: u32 = 0x0000_2000;

pub(super) const FRMSIZE_TYPE_DISCRETE: u32 = 1;
pub(super) const FRMIVAL_TYPE_DISCRETE: u32 = 1;

/// Full range rather than the 16-235 luma a webcam defaults to.
pub(super) const QUANTIZATION_FULL_RANGE: u32 = 1;

/// A FourCC as V4L2 packs it: little-endian over the four characters.
pub(super) const fn fourcc(code: &[u8; 4]) -> u32 {
    (code[0] as u32) | ((code[1] as u32) << 8) | ((code[2] as u32) << 16) | ((code[3] as u32) << 24)
}

/// A FourCC back as the four characters a driver author would recognise.
pub(super) fn fourcc_name(value: u32) -> String {
    value
        .to_le_bytes()
        .iter()
        .map(|byte| char::from(*byte & 0x7f))
        .filter(|c| !c.is_control())
        .collect()
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Capability {
    pub driver: [u8; 16],
    pub card: [u8; 32],
    pub bus_info: [u8; 32],
    pub version: u32,
    pub capabilities: u32,
    pub device_caps: u32,
    pub reserved: [u32; 3],
}
// SAFETY: plain data, and V4L2 wants the reserved fields zeroed.
unsafe impl Zeroable for Capability {}

impl Capability {
    /// What THIS node can do, which is not what the physical device can do.
    ///
    /// A UVC webcam exposes a capture node and a metadata node; only the
    /// per-node `device_caps` tells them apart, and it holds a real answer only
    /// when the driver sets the flag that says so.
    pub fn node_caps(&self) -> u32 {
        if self.capabilities & CAP_DEVICE_CAPS == 0 {
            self.capabilities
        } else {
            self.device_caps
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct FmtDesc {
    pub index: u32,
    pub ty: u32,
    pub flags: u32,
    pub description: [u8; 32],
    pub pixelformat: u32,
    pub reserved: [u32; 4],
}
// SAFETY: plain data.
unsafe impl Zeroable for FmtDesc {}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct FrameSizeEnum {
    pub index: u32,
    pub pixel_format: u32,
    pub ty: u32,
    /// The size union: `(width, height)` when discrete, and `(min_width,
    /// max_width, step_width, min_height, max_height, step_height)` otherwise.
    pub size: [u32; 6],
    pub reserved: [u32; 2],
}
// SAFETY: plain data.
unsafe impl Zeroable for FrameSizeEnum {}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct FrameIvalEnum {
    pub index: u32,
    pub pixel_format: u32,
    pub width: u32,
    pub height: u32,
    pub ty: u32,
    /// The interval union: `(numerator, denominator)` when discrete, and three
    /// such fractions (min, max, step) otherwise.
    pub interval: [u32; 6],
    pub reserved: [u32; 2],
}
// SAFETY: plain data.
unsafe impl Zeroable for FrameIvalEnum {}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct PixFormat {
    pub width: u32,
    pub height: u32,
    pub pixelformat: u32,
    pub field: u32,
    pub bytesperline: u32,
    pub sizeimage: u32,
    pub colorspace: u32,
    pub private: u32,
    pub flags: u32,
    pub ycbcr_enc: u32,
    pub quantization: u32,
    pub xfer_func: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Format {
    pub ty: u32,
    /// The `fmt` union holds pointers, so it is pointer-aligned; this pins that
    /// alignment without hard-coding a word size.
    _align: [libc::c_ulong; 0],
    pub pix: PixFormat,
    _rest: [u8; 200 - size_of::<PixFormat>()],
}
// SAFETY: plain data.
unsafe impl Zeroable for Format {}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct RequestBuffers {
    pub count: u32,
    pub ty: u32,
    pub memory: u32,
    pub capabilities: u32,
    pub flags: u8,
    pub reserved: [u8; 3],
}
// SAFETY: plain data.
unsafe impl Zeroable for RequestBuffers {}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Buffer {
    pub index: u32,
    pub ty: u32,
    pub bytesused: u32,
    pub flags: u32,
    pub field: u32,
    pub timestamp: libc::timeval,
    pub timecode: [u32; 4],
    pub sequence: u32,
    pub memory: u32,
    /// First member of the `m` union, which is all an mmap buffer ever uses.
    pub offset: u32,
    _m_rest: [u8; size_of::<usize>() - size_of::<u32>()],
    pub length: u32,
    pub reserved2: u32,
    pub request_fd: i32,
}
// SAFETY: plain data.
unsafe impl Zeroable for Buffer {}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct StreamParm {
    pub ty: u32,
    pub capability: u32,
    pub capturemode: u32,
    pub numerator: u32,
    pub denominator: u32,
    pub extendedmode: u32,
    pub readbuffers: u32,
    pub reserved: [u32; 4],
    _rest: [u8; 160],
}
// SAFETY: plain data.
unsafe impl Zeroable for StreamParm {}

/// Run one ioctl, retrying the signal interruption a blocking DQBUF invites.
pub(super) fn ioctl<T>(fd: RawFd, request: u32, arg: &mut T) -> io::Result<()> {
    loop {
        // SAFETY: `arg` is a live struct of exactly the size `request` encodes.
        let rc = unsafe {
            libc::ioctl(
                fd,
                request as _,
                core::ptr::from_mut(arg).cast::<libc::c_void>(),
            )
        };
        if rc >= 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        if err.kind() == io::ErrorKind::Interrupted {
            continue;
        }
        return Err(err);
    }
}

/// A NUL-terminated fixed-size driver string as Rust text.
pub(super) fn cstr(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ioctl NUMBER is the thing under test: it encodes the struct size, so
    /// a struct one field wrong stops matching the driver's ioctl entirely and
    /// every call answers ENOTTY.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn the_ioctl_codes_match_the_kernel_headers() {
        assert_eq!(QUERYCAP, 0x8068_5600);
        assert_eq!(ENUM_FMT, 0xc040_5602);
        assert_eq!(S_FMT, 0xc0d0_5605);
        assert_eq!(REQBUFS, 0xc014_5608);
        assert_eq!(QUERYBUF, 0xc058_5609);
        assert_eq!(QBUF, 0xc058_560f);
        assert_eq!(DQBUF, 0xc058_5611);
        assert_eq!(STREAMON, 0x4004_5612);
        assert_eq!(STREAMOFF, 0x4004_5613);
        assert_eq!(S_PARM, 0xc0cc_5616);
        assert_eq!(ENUM_FRAMESIZES, 0xc02c_564a);
        assert_eq!(ENUM_FRAMEINTERVALS, 0xc034_564b);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn the_struct_layout_matches_the_kernel_headers() {
        assert_eq!(size_of::<Capability>(), 104);
        assert_eq!(size_of::<FmtDesc>(), 64);
        assert_eq!(size_of::<FrameSizeEnum>(), 44);
        assert_eq!(size_of::<FrameIvalEnum>(), 52);
        assert_eq!(size_of::<PixFormat>(), 48);
        assert_eq!(size_of::<Format>(), 208);
        assert_eq!(size_of::<RequestBuffers>(), 20);
        assert_eq!(size_of::<Buffer>(), 88);
        assert_eq!(size_of::<StreamParm>(), 204);
    }

    /// A size that is right by accident can still put fields in the wrong place,
    /// which the driver reads as a different request.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn the_fields_sit_where_the_kernel_expects_them() {
        let format = Format::zeroed();
        let base = core::ptr::from_ref(&format) as usize;
        assert_eq!(core::ptr::from_ref(&format.pix) as usize - base, 8);
        assert_eq!(
            core::ptr::from_ref(&format.pix.bytesperline) as usize - base,
            8 + 16
        );

        let buffer = Buffer::zeroed();
        let base = core::ptr::from_ref(&buffer) as usize;
        assert_eq!(core::ptr::from_ref(&buffer.timestamp) as usize - base, 24);
        assert_eq!(core::ptr::from_ref(&buffer.sequence) as usize - base, 56);
        assert_eq!(core::ptr::from_ref(&buffer.offset) as usize - base, 64);
        assert_eq!(core::ptr::from_ref(&buffer.length) as usize - base, 72);

        let parm = StreamParm::zeroed();
        let base = core::ptr::from_ref(&parm) as usize;
        assert_eq!(core::ptr::from_ref(&parm.capability) as usize - base, 4);
        assert_eq!(core::ptr::from_ref(&parm.numerator) as usize - base, 12);

        let sizes = FrameSizeEnum::zeroed();
        let base = core::ptr::from_ref(&sizes) as usize;
        assert_eq!(core::ptr::from_ref(&sizes.size) as usize - base, 12);

        let intervals = FrameIvalEnum::zeroed();
        let base = core::ptr::from_ref(&intervals) as usize;
        assert_eq!(core::ptr::from_ref(&intervals.interval) as usize - base, 20);
    }

    #[test]
    fn a_fourcc_packs_the_way_videodev2_spells_it() {
        assert_eq!(fourcc(b"YUYV"), 0x5659_5559);
        assert_eq!(fourcc(b"NV12"), 0x3231_564e);
        assert_eq!(fourcc(b"MJPG"), 0x4750_4a4d);
        assert_eq!(fourcc_name(fourcc(b"YUYV")), "YUYV");
    }

    #[test]
    fn a_driver_string_stops_at_its_nul_rather_than_its_padding() {
        let mut card = [0u8; 32];
        card[..3].copy_from_slice(b"Cam");
        assert_eq!(cstr(&card), "Cam");
    }

    #[test]
    fn a_node_reports_its_own_caps_only_when_it_says_it_has_them() {
        let mut caps = Capability::zeroed();
        caps.capabilities = CAP_VIDEO_CAPTURE;
        caps.device_caps = 0;
        assert_eq!(caps.node_caps(), CAP_VIDEO_CAPTURE);
        caps.capabilities = CAP_VIDEO_CAPTURE | CAP_DEVICE_CAPS;
        caps.device_caps = CAP_STREAMING;
        assert_eq!(caps.node_caps(), CAP_STREAMING);
    }
}
