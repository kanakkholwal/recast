//! The zero-copy path, proven by reading the shared texture back.
//!
//! A shared texture that is never waited on reads as zeroes with no error
//! raised anywhere, so asserting the handle is non-null proves nothing. These
//! open it on a SECOND D3D11 device, wait on the fence, and read the pixels.

#![cfg(windows)]

use std::time::Duration;

use capturekit::{capturer, DisplayId, GpuHandle, Pacing, Target};
use windows::core::Interface;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11Device1, ID3D11Device5, ID3D11DeviceContext,
    ID3D11DeviceContext4, ID3D11Fence, ID3D11Resource, ID3D11Texture2D, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};

const TIMEOUT: Duration = Duration::from_millis(500);

fn primary() -> Option<DisplayId> {
    let displays = capturekit::displays().ok()?;
    displays
        .iter()
        .find(|d| d.is_primary)
        .or(displays.first())
        .map(|d| d.id)
}

fn second_device() -> (ID3D11Device, ID3D11DeviceContext) {
    let mut device = None;
    let mut context = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            None,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .expect("a second D3D11 device");
    }
    (device.unwrap(), context.unwrap())
}

/// Open a fence the producer shared.
unsafe fn open_fence(device: &ID3D11Device, handle: isize) -> ID3D11Fence {
    let device5: ID3D11Device5 = device.cast().expect("ID3D11Device5");
    let mut fence: Option<ID3D11Fence> = None;
    device5
        .OpenSharedFence(HANDLE(handle as *mut core::ffi::c_void), &mut fence)
        .expect("the shared fence opens");
    fence.expect("a fence")
}

/// Open the shared frame on `device`, wait for the fence, read it back, and
/// release the surface.
///
/// This is the consumer an encoder would be, and both halves are mandatory. The
/// wait orders this read after the producer's copy; the release lets the
/// producer reuse the one surface it copies every frame into.
fn read_shared(
    device: &ID3D11Device,
    context: &ID3D11DeviceContext,
    handle: &GpuHandle,
) -> Vec<u8> {
    unsafe {
        let device1: ID3D11Device1 = device.cast().expect("ID3D11Device1");
        let shared: ID3D11Texture2D = device1
            .OpenSharedResource1(HANDLE(handle.texture as *mut core::ffi::c_void))
            .expect("the shared texture opens on another device");

        let ready = open_fence(device, handle.fence);
        let release = open_fence(device, handle.release);
        let context4: ID3D11DeviceContext4 = context.cast().expect("ID3D11DeviceContext4");
        context4
            .Wait(&ready, handle.ready_at)
            .expect("the wait is queued");

        let mut desc = D3D11_TEXTURE2D_DESC::default();
        shared.GetDesc(&mut desc);
        let staging_desc = D3D11_TEXTURE2D_DESC {
            Width: desc.Width,
            Height: desc.Height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut staging = None;
        device
            .CreateTexture2D(&staging_desc, None, Some(&mut staging))
            .expect("a staging texture");
        let staging: ID3D11Texture2D = staging.unwrap();

        let dst: ID3D11Resource = staging.cast().unwrap();
        let src: ID3D11Resource = shared.cast().unwrap();
        context.CopyResource(&dst, &src);

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        context
            .Map(&dst, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .expect("the staging texture maps");
        let rows = desc.Height as usize;
        let pitch = mapped.RowPitch as usize;
        let out = core::slice::from_raw_parts(mapped.pData.cast::<u8>(), rows * pitch).to_vec();
        context.Unmap(&dst, 0);

        context4
            .Signal(&release, handle.ready_at)
            .expect("the release is queued");
        context.Flush();
        out
    }
}

/// The whole claim: pixels reach another device without passing through host
/// memory on the producer's side. All-zero here is what an unwaited or unshared
/// texture looks like, and it raises no error anywhere.
#[test]
#[ignore = "live: needs a real display and a D3D11 device"]
fn a_shared_frame_carries_real_pixels_to_another_device() {
    let Some(display) = primary() else { return };
    let mut capture = capturer(Target::Display(display))
        .gpu_handles(true)
        .pacing(Pacing::Passthrough)
        .build()
        .expect("the display opens");
    let (device, context) = second_device();

    let frame = capture.next_frame(TIMEOUT).expect("a frame arrives");
    let handle = *frame.gpu_handle().expect("gpu handles were asked for");
    assert!(handle.ready_at > 0, "the fence was never signalled");
    assert!(handle.release != 0, "no release fence was shared");
    assert!(
        frame.bytes().is_empty(),
        "gpu mode still read {} bytes back",
        frame.bytes().len()
    );
    drop(frame);

    let pixels = read_shared(&device, &context, &handle);
    assert!(!pixels.is_empty(), "nothing was read back");
    let distinct: std::collections::HashSet<_> = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| [p[0], p[1], p[2]])
        .collect();
    assert!(
        distinct.len() > 1,
        "the shared texture is a single colour ({} distinct), which is what an \
         unwaited or unshared surface reads as",
        distinct.len()
    );
}

/// Without `gpu_handles`, nothing pays for a shared surface it never asked for.
#[test]
#[ignore = "live: needs a real display"]
fn the_readback_path_reports_no_gpu_handle() {
    let Some(display) = primary() else { return };
    let mut capture = capturer(Target::Display(display))
        .pacing(Pacing::Passthrough)
        .build()
        .expect("the display opens");
    let frame = capture.next_frame(TIMEOUT).expect("a frame arrives");
    assert!(frame.gpu_handle().is_none());
    assert!(!frame.bytes().is_empty());
}

/// One surface is reused for every frame, so the producer waits for the
/// consumer to release the previous one before overwriting it.
///
/// Verified by mutation: dropping the consumer's release signal deadlocks both
/// devices and this HANGS rather than asserting, because the stall is inside a
/// GPU wait that no timeout of ours bounds. Run it with one.
///
/// Needs a desktop that is actually changing: under `Passthrough` an idle
/// display produces nothing at all, which is not what this is measuring.
#[test]
#[ignore = "live: needs a real display with moving content"]
fn frames_keep_arriving_while_a_consumer_reads_and_releases_each_one() {
    let Some(display) = primary() else { return };
    let mut capture = capturer(Target::Display(display))
        .gpu_handles(true)
        .pacing(Pacing::Passthrough)
        .build()
        .expect("the display opens");
    let (device, context) = second_device();

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut seen = Vec::new();
    while seen.len() < 6 && std::time::Instant::now() < deadline {
        let Ok(frame) = capture.next_frame(TIMEOUT) else {
            continue;
        };
        let handle = *frame.gpu_handle().expect("gpu handles were asked for");
        drop(frame);
        let pixels = read_shared(&device, &context, &handle);
        assert!(!pixels.is_empty());
        seen.push(handle.ready_at);
    }

    assert!(
        seen.len() >= 3,
        "only {} frames in 10s — the producer is stalled, or the desktop is not changing",
        seen.len()
    );
    assert!(
        seen.windows(2).all(|pair| pair[1] > pair[0]),
        "each copy owes a fresh fence value, got {seen:?}"
    );
}
