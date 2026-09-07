use crate::error::GpuError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PowerPreference {
    #[default]
    HighPerformance,
    LowPower,
}

#[derive(Debug, Clone)]
pub struct GpuOptions {
    pub power: PowerPreference,
    pub label: &'static str,
    /// Fail rather than silently falling back to a software adapter. Export and
    /// recording want this; a preview surface may prefer WARP over nothing.
    pub require_hardware: bool,
    /// `None` picks the platform default. The wasm preview overrides it to pin
    /// WebGPU or WebGL2.
    pub backends: Option<wgpu::Backends>,
}

impl Default for GpuOptions {
    fn default() -> Self {
        Self {
            power: PowerPreference::HighPerformance,
            label: "recast",
            require_hardware: true,
            backends: None,
        }
    }
}

pub struct GpuContext {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl GpuContext {
    pub async fn new(options: GpuOptions) -> Result<Self, GpuError> {
        let instance = Self::instance_for(&options);
        Self::from_instance(instance, options, None).await
    }

    /// Split from `from_instance` because a surface must be created from the
    /// same instance the device comes from, and the caller owns the canvas.
    pub fn instance_for(options: &GpuOptions) -> wgpu::Instance {
        let mut instance_desc = wgpu::InstanceDescriptor::new_without_display_handle();
        instance_desc.backends = options.backends.unwrap_or_else(preferred_backends);
        wgpu::Instance::new(instance_desc)
    }

    pub async fn from_instance(
        instance: wgpu::Instance,
        options: GpuOptions,
        compatible_surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, GpuError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: match options.power {
                    PowerPreference::HighPerformance => wgpu::PowerPreference::HighPerformance,
                    PowerPreference::LowPower => wgpu::PowerPreference::LowPower,
                },
                force_fallback_adapter: false,
                compatible_surface,
                ..Default::default()
            })
            .await
            .map_err(|_| GpuError::NoAdapter)?;

        let info = adapter.get_info();
        if options.require_hardware && info.device_type == wgpu::DeviceType::Cpu {
            return Err(GpuError::SoftwareAdapterRejected {
                name: info.name.clone(),
            });
        }

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some(options.label),
                required_features: wgpu::Features::empty(),
                required_limits: adapter.limits(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
                ..Default::default()
            })
            .await
            .map_err(|e| GpuError::DeviceRequest(e.to_string()))?;

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_blocking(options: GpuOptions) -> Result<Self, GpuError> {
        pollster::block_on(Self::new(options))
    }

    pub fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }

    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn info(&self) -> wgpu::AdapterInfo {
        self.adapter.get_info()
    }

    pub fn is_software(&self) -> bool {
        self.adapter.get_info().device_type == wgpu::DeviceType::Cpu
    }

    pub fn texture_pool(&self, label: &'static str) -> crate::pool::GpuTexturePool {
        crate::pool::TexturePool::new(crate::pool::WgpuAllocator::new(self.device.clone(), label))
    }

    /// True when this adapter can import an OS shared texture handle without a host copy. Only DX12 is proven so far (S-1); the others gate their own paths as they land.
    pub fn supports_zero_copy_import(&self) -> bool {
        matches!(self.adapter.get_info().backend, wgpu::Backend::Dx12)
    }
}

fn preferred_backends() -> wgpu::Backends {
    if cfg!(windows) {
        wgpu::Backends::DX12
    } else if cfg!(target_os = "macos") {
        wgpu::Backends::METAL
    } else if cfg!(target_arch = "wasm32") {
        browser_backends()
    } else {
        wgpu::Backends::VULKAN
    }
}

/// Only what this artifact compiled in: the WebGL2 backend is a separate build
/// because it costs roughly 1.1 MB gzipped.
fn browser_backends() -> wgpu::Backends {
    let mut backends = wgpu::Backends::empty();
    if cfg!(feature = "webgpu") {
        backends |= wgpu::Backends::BROWSER_WEBGPU;
    }
    if cfg!(feature = "webgl2") {
        backends |= wgpu::Backends::GL;
    }
    backends
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backend_choice_follows_the_platform_the_plan_committed_to() {
        let backends = preferred_backends();
        if cfg!(windows) {
            assert_eq!(backends, wgpu::Backends::DX12);
        } else if cfg!(target_os = "macos") {
            assert_eq!(backends, wgpu::Backends::METAL);
        }
    }

    /// Pins that the override reaches the instance. Dropping it would leave the
    /// wasm preview silently on whatever the platform default picked.
    #[test]
    fn an_explicit_backend_override_is_honoured_rather_than_ignored() {
        let options = GpuOptions {
            backends: Some(wgpu::Backends::empty()),
            require_hardware: false,
            ..Default::default()
        };
        let instance = GpuContext::instance_for(&options);
        let result = pollster::block_on(GpuContext::from_instance(instance, options, None));
        assert!(matches!(result, Err(GpuError::NoAdapter)));
    }

    #[test]
    fn export_defaults_to_refusing_a_software_adapter() {
        assert!(GpuOptions::default().require_hardware);
    }
}
