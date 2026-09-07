use recast_gpu::{GpuContext, GpuOptions, TextureDesc, WORKING_FORMAT};

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

#[test]
fn a_device_comes_up_on_the_platform_backend() {
    let Some(ctx) = context() else { return };
    let info = ctx.info();
    eprintln!(
        "adapter: {} [{:?}] on {:?}",
        info.name, info.device_type, info.backend
    );
    if cfg!(windows) {
        assert_eq!(info.backend, wgpu::Backend::Dx12);
        assert!(ctx.supports_zero_copy_import());
    }
}

#[test]
fn the_pool_allocates_real_textures_and_recycles_them() {
    let Some(ctx) = context() else { return };
    let mut pool = ctx.texture_pool("test");

    let desc = TextureDesc::new(1920, 1080, WORKING_FORMAT);
    let first = pool.acquire(desc);
    assert_eq!(first.texture.width(), 1920);
    assert_eq!(first.texture.format(), WORKING_FORMAT);
    pool.release(first);

    let second = pool.acquire(desc);
    pool.release(second);
    assert_eq!(pool.stats().created, 1);
    assert_eq!(pool.stats().reused, 1);
}

#[test]
fn requiring_hardware_rejects_a_software_adapter() {
    let Ok(ctx) = GpuContext::new_blocking(GpuOptions::default()) else {
        return;
    };
    assert!(!ctx.is_software());
}
