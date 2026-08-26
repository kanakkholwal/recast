#![deny(unsafe_op_in_unsafe_fn)]

mod context;
mod error;
mod format;
pub mod interop;
mod pool;

pub use context::{GpuContext, GpuOptions, PowerPreference};
pub use error::GpuError;
pub use format::{
    aligned_bytes_per_row, is_linear_float, is_srgb_encoded, MASK_FORMAT, OUTPUT_FORMAT,
    WORKING_FORMAT,
};
pub use interop::{
    import_shared_fence, import_shared_texture, SharedFence, SharedFormat, SharedHandle,
    SharedTexture, SharedTextureDesc, SharedUse,
};
pub use pool::{
    GpuTexturePool, Lease, PoolStats, TextureAllocator, TextureDesc, TexturePool, WgpuAllocator,
    DEFAULT_MAX_IDLE_BYTES, DEFAULT_MAX_UNUSED_FRAMES,
};
