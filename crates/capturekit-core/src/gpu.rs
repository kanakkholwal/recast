/// A frame left on the GPU, addressed by platform handles.
///
/// The pixels never reach host memory. Importing one is inherently unsafe and
/// platform-specific, which is why this carries raw handles rather than a typed
/// texture: capturekit will not depend on a graphics API to hand one over.
///
/// **Ordering is not implicit, in both directions.** Cross-device sharing on
/// Windows carries no barrier. A consumer that samples the texture before
/// `fence` reaches `ready_at` reads zeroes, and no error is raised anywhere;
/// a consumer that never signals `release` lets the producer overwrite the
/// picture mid-read, which is the same corruption from the other side. One
/// surface is reused for every frame, so both halves are mandatory: wait on
/// `fence` for `ready_at`, then signal `release` with `ready_at` once the read
/// is queued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuHandle {
    /// Windows: an NT handle to a shared `ID3D11Texture2D`, openable by a D3D11
    /// or D3D12 device on the SAME adapter.
    pub texture: isize,
    /// Windows: an NT handle to a shared `ID3D11Fence` the producer signals.
    pub fence: isize,
    /// Windows: an NT handle to a shared `ID3D11Fence` the CONSUMER signals with
    /// `ready_at` once its read of the texture is queued. The producer waits on
    /// it before reusing the surface.
    pub release: isize,
    /// The fence value that means this frame's copy has landed, and the value to
    /// signal back on `release` when the read is done.
    pub ready_at: u64,
    /// Width of the shared texture in pixels.
    pub width: u32,
    /// Height of the shared texture in pixels.
    pub height: u32,
}
