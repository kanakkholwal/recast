use anyhow::{bail, Result};
// Every `.context()` call site is Windows-gated; the stub main is not.
#[cfg(windows)]
use anyhow::Context;

#[cfg(not(windows))]
fn main() -> Result<()> {
    bail!("this spike is Windows-only")
}

#[cfg(windows)]
const WIDTH: u32 = 256;
#[cfg(windows)]
const HEIGHT: u32 = 128;

#[cfg(windows)]
fn expected_pixel(x: u32, y: u32) -> [u8; 4] {
    [(x % 256) as u8, (y % 256) as u8, 0x40, 0xff]
}

#[cfg(windows)]
fn main() -> Result<()> {
    println!("== S-1: D3D11 shared NT handle -> wgpu DX12 import ==");
    let mut instance_desc = wgpu::InstanceDescriptor::new_without_display_handle();
    instance_desc.backends = wgpu::Backends::DX12;
    let instance = wgpu::Instance::new(instance_desc);

    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::DX12));
    if adapters.is_empty() {
        bail!("no DX12 adapters found");
    }

    let mut results = Vec::new();
    for adapter in adapters {
        let info = adapter.get_info();
        let label = format!("{} [{:?}]", info.name, info.device_type);
        println!();
        println!(
            "---- {label} (vendor {:#06x} device {:#06x}) ----",
            info.vendor, info.device
        );
        match run_adapter(&adapter, &info) {
            Ok(per_frame_ms) => {
                println!("PASS  ({per_frame_ms:.3} ms/frame fence round trip)");
                results.push((label, Ok(per_frame_ms)));
            }
            Err(e) => {
                println!("FAIL  {e}");
                results.push((label, Err(e.to_string())));
            }
        }
    }

    println!();
    println!("== summary ==");
    let mut any_pass = false;
    for (label, outcome) in &results {
        match outcome {
            Ok(ms) => {
                any_pass = true;
                println!("  PASS  {label}  ({ms:.3} ms/frame)");
            }
            Err(e) => println!("  FAIL  {label}  {e}"),
        }
    }
    if !any_pass {
        bail!("zero-copy interop failed on every adapter");
    }
    Ok(())
}

#[cfg(windows)]
fn run_adapter(adapter: &wgpu::Adapter, info: &wgpu::AdapterInfo) -> Result<f64> {
    use windows::core::Interface;
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Direct3D12::ID3D12Resource;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::*;

    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

    // The D3D11 device must sit on the same physical adapter or OpenSharedHandle
    // fails; matching by DXGI vendor/device id is what makes a hybrid GPU work.
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1()? };
    let mut chosen: Option<IDXGIAdapter1> = None;
    for index in 0.. {
        let Ok(candidate) = (unsafe { factory.EnumAdapters1(index) }) else {
            break;
        };
        let desc = unsafe { candidate.GetDesc1()? };
        if desc.VendorId == info.vendor && desc.DeviceId == info.device {
            chosen = Some(candidate);
            break;
        }
    }
    let dxgi_adapter = chosen.context("no DXGI adapter matched the wgpu adapter")?;

    let mut d3d11: Option<ID3D11Device> = None;
    let mut context: Option<ID3D11DeviceContext> = None;
    unsafe {
        D3D11CreateDevice(
            Some(&dxgi_adapter.cast::<IDXGIAdapter>()?),
            D3D_DRIVER_TYPE_UNKNOWN,
            Default::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut d3d11),
            None,
            Some(&mut context),
        )?;
    }
    let d3d11 = d3d11.context("D3D11 device was not created")?;

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
    let mut shared_texture: Option<ID3D11Texture2D> = None;
    unsafe { d3d11.CreateTexture2D(&desc, Some(&initial), Some(&mut shared_texture))? };
    let shared_texture = shared_texture.context("shared texture was not created")?;
    // Without this the D3D12 device reads the texture before D3D11's upload has
    // landed and every pixel comes back zero. Cross-device sharing is not implicitly ordered.
    let d3d11_context = context.clone().context("no d3d11 context")?;
    unsafe { d3d11_context.Flush() };
    println!("d3d11: created {WIDTH}x{HEIGHT} BGRA shared texture (context flushed)");

    let resource1: IDXGIResource1 = shared_texture.cast()?;
    let handle = unsafe { resource1.CreateSharedHandle(None, DXGI_SHARED_RESOURCE_READ.0, None)? };
    println!("d3d11: shared NT handle = {:?}", handle.0);

    let imported = unsafe {
        let hal_device = device
            .as_hal::<wgpu::hal::api::Dx12>()
            .context("wgpu device is not DX12")?;
        let mut opened: Option<ID3D12Resource> = None;
        hal_device
            .raw_device()
            .OpenSharedHandle(handle, &mut opened)?;
        let d3d12_resource = opened.context("OpenSharedHandle returned nothing")?;
        println!("d3d12: OpenSharedHandle OK");

        let hal_texture = wgpu::hal::dx12::Device::texture_from_raw(
            d3d12_resource,
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureDimension::D2,
            wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            1,
            1,
        );
        drop(hal_device);
        device.create_texture_from_hal::<wgpu::hal::api::Dx12>(
            hal_texture,
            &wgpu::TextureDescriptor {
                label: Some("imported-d3d11"),
                size: wgpu::Extent3d {
                    width: WIDTH,
                    height: HEIGHT,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
            wgpu::TextureUses::COPY_SRC,
        )
    };
    println!("wgpu: imported as a wgpu::Texture with no host copy");

    let linear = render_through_shader(&device, &queue, &imported)?;
    verify(&linear)?;

    let per_frame_ms = steady_state(&d3d11, &d3d11_context, &device, &queue, &imported)?;

    unsafe { windows::Win32::Foundation::CloseHandle(handle)? };
    Ok(per_frame_ms)
}

#[cfg(windows)]
const SHADER: &str = r#"
@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -3.0), vec2(-1.0, 1.0), vec2(3.0, 1.0));
    return vec4<f32>(p[i], 0.0, 1.0);
}

@group(0) @binding(0) var src: texture_2d<f32>;

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(src, vec2<i32>(pos.xy), 0);
}
"#;

/// Samples the imported texture into an Rgba16Float target, which is the working
/// format the compositor will use, then reads that target back.
#[cfg(windows)]
fn render_through_shader(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    imported: &wgpu::Texture,
) -> Result<Vec<u8>> {
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("linear-target"),
        size: wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            targets: &[Some(wgpu::TextureFormat::Rgba16Float.into())],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(
                &imported.create_view(&Default::default()),
            ),
        }],
    });

    let bytes_per_row = WIDTH * 8;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (bytes_per_row * HEIGHT) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let view = target.create_view(&Default::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
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
    queue.submit([encoder.finish()]);

    let slice = readback.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device.poll(wgpu::PollType::wait_indefinitely())?;
    rx.recv()??;
    let data = slice.get_mapped_range()?.to_vec();
    readback.unmap();
    Ok(data)
}

#[cfg(windows)]
fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits >> 15) & 1) as u32;
    let exp = ((bits >> 10) & 0x1f) as u32;
    let frac = (bits & 0x3ff) as u32;
    let out = match exp {
        0 if frac == 0 => sign << 31,
        0 => {
            let mut e = -1i32;
            let mut f = frac;
            while f & 0x400 == 0 {
                f <<= 1;
                e -= 1;
            }
            (sign << 31) | (((127 + e - 14) as u32) << 23) | ((f & 0x3ff) << 13)
        }
        0x1f => (sign << 31) | 0x7f80_0000 | (frac << 13),
        _ => (sign << 31) | ((exp + 112) << 23) | (frac << 13),
    };
    f32::from_bits(out)
}

#[cfg(windows)]
fn verify(linear: &[u8]) -> Result<()> {
    let mut checked = 0;
    for &(x, y) in &[
        (0u32, 0u32),
        (1, 0),
        (255, 0),
        (0, 127),
        (128, 64),
        (200, 100),
    ] {
        let offset = ((y * WIDTH + x) * 8) as usize;
        let read = |i: usize| {
            f16_to_f32(u16::from_le_bytes([
                linear[offset + i * 2],
                linear[offset + i * 2 + 1],
            ]))
        };
        let want = expected_pixel(x, y);
        // The imported texture is BGRA, so channel 0 of the shader load is blue.
        let got = [
            (read(0) * 255.0).round() as u8,
            (read(1) * 255.0).round() as u8,
            (read(2) * 255.0).round() as u8,
        ];
        let want_rgb = [want[2], want[1], want[0]];
        if got != want_rgb {
            bail!("pixel ({x},{y}): imported {got:?}, expected {want_rgb:?}");
        }
        checked += 1;
    }
    println!("verify: {checked} sampled pixels match the D3D11 source exactly");
    Ok(())
}

/// A per-frame `Flush()` is a full CPU/GPU sync, so the steady-state path needs a
/// shared fence: D3D11 signals, wgpu's D3D12 queue waits GPU-side. D3D12 has no
/// keyed-mutex support, so a fence is the only cross-API option.
#[cfg(windows)]
fn steady_state(
    d3d11: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
    context: &windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    imported: &wgpu::Texture,
) -> Result<f64> {
    use windows::core::Interface;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Direct3D12::ID3D12Fence;

    const GENERIC_ALL: u32 = 0x1000_0000;

    println!();
    println!("== steady state: shared fence, no per-frame flush ==");

    let device5: ID3D11Device5 = d3d11.cast()?;
    let context4: ID3D11DeviceContext4 = context.cast()?;
    let mut fence_slot: Option<ID3D11Fence> = None;
    unsafe { device5.CreateFence(0, D3D11_FENCE_FLAG_SHARED, &mut fence_slot)? };
    let fence11 = fence_slot.context("d3d11 fence was not created")?;
    let fence_handle = unsafe { fence11.CreateSharedHandle(None, GENERIC_ALL, None)? };

    let fence12: ID3D12Fence = unsafe {
        let hal_device = device
            .as_hal::<wgpu::hal::api::Dx12>()
            .context("wgpu device is not DX12")?;
        let mut opened: Option<ID3D12Fence> = None;
        hal_device
            .raw_device()
            .OpenSharedHandle(fence_handle, &mut opened)?;
        opened.context("fence OpenSharedHandle returned nothing")?
    };
    println!("fence: shared D3D11 -> D3D12 fence opened");

    let scratch = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scratch"),
        size: wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8Unorm,
        usage: wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    const FRAMES: u64 = 120;
    let start = std::time::Instant::now();
    for frame in 1..=FRAMES {
        unsafe { context4.Signal(&fence11, frame)? };
        unsafe {
            let hal_device = device
                .as_hal::<wgpu::hal::api::Dx12>()
                .context("wgpu device is not DX12")?;
            hal_device.raw_queue().Wait(&fence12, frame)?;
        }
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: imported,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &scratch,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
    }
    device.poll(wgpu::PollType::wait_indefinitely())?;
    let elapsed = start.elapsed();

    unsafe { windows::Win32::Foundation::CloseHandle(fence_handle)? };
    println!(
        "fence: {FRAMES} signal/wait round trips in {:.2}ms ({:.3}ms per frame)",
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_secs_f64() * 1000.0 / FRAMES as f64
    );
    Ok(elapsed.as_secs_f64() * 1000.0 / FRAMES as f64)
}
