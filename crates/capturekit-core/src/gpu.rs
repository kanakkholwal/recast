/// A frame left on the GPU, addressed by platform handles.
///
/// The pixels never reach host memory. Importing one is inherently unsafe and
/// platform-specific, which is why this carries raw handles rather than a typed
/// texture: capturekit will not depend on a graphics API to hand one over.
///
/// **Ordering is not implicit.** Cross-device sharing on Windows carries no
/// barrier, so a consumer that samples the texture before `fence` reaches
/// `ready_at` reads zeroes, and no error is raised anywhere. Wait first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuHandle {
    /// Windows: an NT handle to a shared `ID3D11Texture2D`, openable by a D3D11
    /// or D3D12 device on the SAME adapter.
    pub texture: isize,
    /// Windows: an NT handle to a shared `ID3D11Fence`.
    pub fence: isize,
    /// The fence value that means this frame's copy has landed.
    pub ready_at: u64,
    /// Width of the shared texture in pixels.
    pub width: u32,
    /// Height of the shared texture in pixels.
    pub height: u32,
}
