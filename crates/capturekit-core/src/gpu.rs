/// A frame left on the GPU, carrying raw handles so capturekit needs no graphics-API dependency.
/// Ordering is explicit in both directions: wait on `fence` for `ready_at`, then signal `release`, or the shared surface tears silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuHandle {
    /// Windows: an NT handle to a shared `ID3D11Texture2D`, openable by a D3D11
    /// or D3D12 device on the SAME adapter.
    pub texture: isize,
    /// Windows: an NT handle to a shared `ID3D11Fence` the producer signals.
    pub fence: isize,
    /// Windows: an NT handle to a shared `ID3D11Fence` the CONSUMER signals with `ready_at` once its read of the texture is queued. The producer waits on it before reusing the surface.
    pub release: isize,
    /// The fence value that means this frame's copy has landed, and the value to
    /// signal back on `release` when the read is done.
    pub ready_at: u64,
    /// Width of the shared texture in pixels.
    pub width: u32,
    /// Height of the shared texture in pixels.
    pub height: u32,
}
