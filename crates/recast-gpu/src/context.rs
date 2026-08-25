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
}

impl Default for GpuOptions {
    fn default() -> Self {
        Self {
            power: PowerPreference::HighPerformance,
            label: "recast",
            require_hardware: true,
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
        let mut instance_desc = wgpu::InstanceDescriptor::new_without_display_handle();
        instance_desc.backends = preferred_backends();
        let instance = wgpu::Instance::new(instance_desc);

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: match options.power {
                    PowerPreference::HighPerformance => wgpu::PowerPreference::HighPerformance,
                    PowerPreference::LowPower => wgpu::PowerPreference::LowPower,
                },
                force_fallback_adapter: false,
                compatible_surface: None,
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

    /// True when this adapter can import an OS shared texture handle without a
    /// host copy. Only DX12 is proven so far (S-1); the others gate their own
    /// paths as they land.
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
        wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL
    } else {
        wgpu::Backends::VULKAN
    }
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

    #[test]
    fn export_defaults_to_refusing_a_software_adapter() {
        assert!(GpuOptions::default().require_hardware);
    }
}
