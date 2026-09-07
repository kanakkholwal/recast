use super::{SharedFence, SharedHandle, SharedTexture, SharedTextureDesc};
use crate::{GpuContext, GpuError};

pub struct OwnedHandle;
pub struct Fence;

pub fn import_texture(
    _ctx: &GpuContext,
    _handle: SharedHandle,
    _desc: SharedTextureDesc,
) -> Result<SharedTexture, GpuError> {
    Err(GpuError::Unsupported("shared texture import"))
}

pub fn import_fence(_ctx: &GpuContext, _handle: SharedHandle) -> Result<SharedFence, GpuError> {
    Err(GpuError::Unsupported("shared fence import"))
}

pub fn queue_wait(_ctx: &GpuContext, _fence: &Fence, _value: u64) -> Result<(), GpuError> {
    Err(GpuError::Unsupported("shared fence wait"))
}

pub fn queue_signal(_ctx: &GpuContext, _fence: &Fence, _value: u64) -> Result<(), GpuError> {
    Err(GpuError::Unsupported("shared fence signal"))
}
