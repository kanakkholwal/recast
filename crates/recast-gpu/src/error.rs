use thiserror::Error;

#[derive(Debug, Error)]
pub enum GpuError {
    #[error("no GPU adapter available for the requested backend")]
    NoAdapter,
    #[error("adapter {name} is a software rasteriser and hardware was required")]
    SoftwareAdapterRejected { name: String },
    #[error("device request failed: {0}")]
    DeviceRequest(String),
    #[error("shared texture import failed: {0}")]
    Import(String),
    #[error("{0} is not supported on this backend")]
    Unsupported(&'static str),
}
