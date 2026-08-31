#![cfg(windows)]

use recast_gpu::interop::FENCE_SHARED_ACCESS;
use recast_gpu::{
    import_shared_fence, import_shared_texture, GpuContext, GpuOptions, SharedFormat, SharedHandle,
    SharedTextureDesc,
};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;

const WIDTH: u32 = 256;
const HEIGHT: u32 = 128;

fn expected_pixel(x: u32, y: u32) -> [u8; 4] {
    [(x % 256) as u8, (y % 256) as u8, 0x40, 0xff]
}

fn context() -> Option<GpuContext> {
    match GpuContext::new_blocking(GpuOptions {
        require_hardware: false,
        ..Default::default()
    }) {
        Ok(ctx) => Some(ctx),
        Err(e) => {
            if std::env::var("RECAST_GPU_REQUIRE_ADAPTER").as_deref() == Ok("1") {
                panic!("RECAST_GPU_REQUIRE_ADAPTER=1 but no adapter: {e}");
            }
            eprintln!("skipping: no GPU adapter ({e})");
            None
        }
    }
}

struct Producer {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
}

/// A D3D11 device on the SAME physical adapter as `ctx`. `OpenSharedHandle` fails across adapters, which is what makes hybrid-GPU laptops fail if the match is skipped.
fn producer_on(ctx: &GpuContext) -> Option<Producer> {
    let info = ctx.info();
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.ok()?;
    let mut chosen: Option<IDXGIAdapter1> = None;
    for index in 0.. {
        let Ok(candidate) = (unsafe { factory.EnumAdapters1(index) }) else {
            break;
        };
        let desc = unsafe { candidate.GetDesc1() }.ok()?;
        if desc.VendorId == info.vendor && desc.DeviceId == info.device {
            chosen = Some(candidate);
            break;
        }
    }
    let adapter = chosen?;

    let mut device: Option<ID3D11Device> = None;
    let mut immediate: Option<ID3D11DeviceContext> = None;
    unsafe {
        D3D11CreateDevice(
            Some(&adapter.cast::<IDXGIAdapter>().ok()?),
            D3D_DRIVER_TYPE_UNKNOWN,
            Default::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut immediate),
        )
    }
    .ok()?;

    Some(Producer {
        device: device?,
        context: immediate?,
    })
}

fn create_shared_texture(producer: &Producer) -> (ID3D11Texture2D, SharedHandle) {
    let mut pixels = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let offset = ((y * WIDTH + x) * 4) as usize;
            pixels[offset..offset + 4].copy_from_slice(&expected_pixel(x, y));
        }
    }

    let desc = D3D11_TEXTURE2D_DESC {
        Width: WIDTH,
        Height: HEIGHT,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
        CPUAccessFlags: 0,
        MiscFlags: (D3D11_RESOURCE_MISC_SHARED.0 | D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0) as u32,
    };
    let initial = D3D11_SUBRESOURCE_DATA {
        pSysMem: pixels.as_ptr().cast(),
        SysMemPitch: WIDTH * 4,
        SysMemSlicePitch: 0,
    };
    let mut texture: Option<ID3D11Texture2D> = None;
    unsafe {
        producer
            .device
            .CreateTexture2D(&desc, Some(&initial), Some(&mut texture))
    }
    .expect("create shared texture");
    let texture = texture.expect("shared texture");

    unsafe { producer.context.Flush() };

    let resource: IDXGIResource1 = texture.cast().expect("IDXGIResource1");
    let handle = unsafe { resource.CreateSharedHandle(None, DXGI_SHARED_RESOURCE_READ.0, None) }
        .expect("CreateSharedHandle");
    (texture, SharedHandle(handle.0 as isize))
}

fn read_back_bgra(ctx: &GpuContext, source: &wgpu::Texture) -> Vec<u8> {
    let bytes_per_row = recast_gpu::aligned_bytes_per_row(WIDTH, wgpu::TextureFormat::Bgra8Unorm);
    let buffer = ctx.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (bytes_per_row * HEIGHT) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = ctx.device().create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: source,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(HEIGHT),
            },
        },
        wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
    );
    ctx.queue().submit([encoder.finish()]);

    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    ctx.device()
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");

    let mapped = slice.get_mapped_range().expect("map readback buffer");
    let mut out = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    for row in 0..HEIGHT {
        let start = (row * bytes_per_row) as usize;
        out.extend_from_slice(&mapped[start..start + (WIDTH * 4) as usize]);
    }
    drop(mapped);
    buffer.unmap();
    out
}

#[test]
fn a_d3d11_texture_imports_with_its_pixels_intact() {
    let Some(ctx) = context() else { return };
    if !ctx.supports_zero_copy_import() {
        eprintln!("skipping: adapter is not DX12");
        return;
    }
    let Some(producer) = producer_on(&ctx) else {
        eprintln!("skipping: no matching D3D11 adapter");
        return;
    };

    let (_keep_alive, handle) = create_shared_texture(&producer);
    let shared = import_shared_texture(
        &ctx,
        handle,
        SharedTextureDesc::new(WIDTH, HEIGHT, SharedFormat::Bgra8Unorm),
    )
    .expect("import");

    let pixels = read_back_bgra(&ctx, shared.texture());
    for &(x, y) in &[
        (0u32, 0u32),
        (1, 0),
        (255, 0),
        (0, 127),
        (128, 64),
        (200, 100),
    ] {
        let offset = ((y * WIDTH + x) * 4) as usize;
        assert_eq!(
            &pixels[offset..offset + 4],
            &expected_pixel(x, y),
            "pixel ({x},{y}) did not survive the import"
        );
    }
}

#[test]
fn a_shared_fence_imports_and_the_queue_wait_is_accepted() {
    let Some(ctx) = context() else { return };
    if !ctx.supports_zero_copy_import() {
        return;
    }
    let Some(producer) = producer_on(&ctx) else {
        return;
    };

    let device5: ID3D11Device5 = producer.device.cast().expect("ID3D11Device5");
    let context4: ID3D11DeviceContext4 = producer.context.cast().expect("ID3D11DeviceContext4");
    let mut fence: Option<ID3D11Fence> = None;
    unsafe { device5.CreateFence(0, D3D11_FENCE_FLAG_SHARED, &mut fence) }.expect("CreateFence");
    let fence = fence.expect("fence");

    let raw = unsafe { fence.CreateSharedHandle(None, FENCE_SHARED_ACCESS, None) }
        .expect("fence CreateSharedHandle");
    let imported = import_shared_fence(&ctx, SharedHandle(raw.0 as isize)).expect("import fence");

    unsafe { context4.Signal(&fence, 1) }.expect("signal");
    imported.queue_wait(&ctx, 1).expect("queue wait");

    let mut encoder = ctx.device().create_command_encoder(&Default::default());
    encoder.insert_debug_marker("after fence wait");
    ctx.queue().submit([encoder.finish()]);
    ctx.device()
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
}

/// S-1 recorded `SYNCHRONIZE` as rejected at `CreateSharedHandle`. It is not:
/// the handle is created and the IMPORT is what fails. Pinned here so the
/// constant is not "simplified" back to `SYNCHRONIZE` by someone reading the
/// Win32 docs, which describe it as the right access mask for a fence.
#[test]
fn a_synchronize_access_fence_handle_cannot_be_imported() {
    let Some(ctx) = context() else { return };
    if !ctx.supports_zero_copy_import() {
        return;
    }
    let Some(producer) = producer_on(&ctx) else {
        return;
    };
    const SYNCHRONIZE: u32 = 0x0010_0000;

    let device5: ID3D11Device5 = producer.device.cast().expect("ID3D11Device5");
    let mut fence: Option<ID3D11Fence> = None;
    unsafe { device5.CreateFence(0, D3D11_FENCE_FLAG_SHARED, &mut fence) }.expect("CreateFence");
    let fence = fence.expect("fence");

    let restricted = unsafe { fence.CreateSharedHandle(None, SYNCHRONIZE, None) }
        .expect("CreateSharedHandle itself accepts SYNCHRONIZE");
    let imported = import_shared_fence(&ctx, SharedHandle(restricted.0 as isize));
    assert!(
        imported.is_err(),
        "a SYNCHRONIZE-access fence handle imported; FENCE_SHARED_ACCESS could be narrowed"
    );

    let full = unsafe { fence.CreateSharedHandle(None, FENCE_SHARED_ACCESS, None) }
        .expect("GENERIC_ALL is accepted");
    import_shared_fence(&ctx, SharedHandle(full.0 as isize)).expect("GENERIC_ALL imports");
}

#[test]
fn a_null_or_degenerate_import_is_refused_before_touching_the_driver() {
    let Some(ctx) = context() else { return };
    assert!(import_shared_texture(
        &ctx,
        SharedHandle(0),
        SharedTextureDesc::new(64, 64, SharedFormat::Bgra8Unorm)
    )
    .is_err());
    assert!(import_shared_fence(&ctx, SharedHandle(0)).is_err());
    assert!(import_shared_texture(
        &ctx,
        SharedHandle(1),
        SharedTextureDesc::new(0, 64, SharedFormat::Bgra8Unorm)
    )
    .is_err());
}
