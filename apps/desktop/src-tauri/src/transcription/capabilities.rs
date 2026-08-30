//! Device capability probe for caption-model gating.
//!
//! Cheap, best-effort detection of OS / arch / RAM / GPU so the UI can disable
//! models a device can't run and warn about ones that'll be slow. Deliberately
//! conservative on GPU: we only *confirm* an accelerator we can prove (Metal on
//! macOS, CUDA via `nvidia-smi`); everything else reports CPU mode rather than
//! overpromising.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub available: bool,
    /// "metal" | "cuda" | None (CPU mode).
    pub backend: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCapabilities {
    /// "macos" | "windows" | "linux" | …
    pub os: String,
    /// "aarch64" | "x86_64" | …
    pub arch: String,
    pub total_ram_bytes: Option<u64>,
    pub gpu: GpuInfo,
    /// Whether the on-device caption engine (ggml / transcribe.cpp) is compiled
    /// into this build. False in a `--no-default-features` build, where the UI
    /// offers remote endpoints instead of the on-device generator.
    pub captions_available: bool,
}

pub fn detect() -> DeviceCapabilities {
    DeviceCapabilities {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        total_ram_bytes: total_ram(),
        gpu: detect_gpu(),
        captions_available: cfg!(feature = "ggml"),
    }
}

fn detect_gpu() -> GpuInfo {
    #[cfg(target_os = "macos")]
    {
        // Every supported Mac has a Metal GPU (Apple Silicon or Intel/AMD).
        GpuInfo {
            available: true,
            backend: Some("metal".into()),
            name: None,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Only claim a GPU we can prove: `nvidia-smi` confirms CUDA, and anything else reports CPU without more native probing.
        match nvidia_name() {
            Some(name) => GpuInfo {
                available: true,
                backend: Some("cuda".into()),
                name: Some(name),
            },
            None => GpuInfo {
                available: false,
                backend: None,
                name: None,
            },
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn nvidia_name() -> Option<String> {
    let mut cmd = std::process::Command::new("nvidia-smi");
    cmd.args(["--query-gpu=name", "--format=csv,noheader"]);
    crate::ffmpeg::configure_silent_command(&mut cmd); // no console flash on Windows
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()?
        .trim()
        .to_string();
    (!name.is_empty()).then_some(name)
}

#[cfg(target_os = "windows")]
fn total_ram() -> Option<u64> {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    unsafe { GlobalMemoryStatusEx(&mut status).ok()? };
    Some(status.ullTotalPhys)
}

#[cfg(target_os = "macos")]
fn total_ram() -> Option<u64> {
    let out = std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

#[cfg(target_os = "linux")]
fn total_ram() -> Option<u64> {
    let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in meminfo.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let kb: u64 = rest.trim().trim_end_matches("kB").trim().parse().ok()?;
            return Some(kb * 1024);
        }
    }
    None
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn total_ram() -> Option<u64> {
    None
}
