#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod css;
mod gradient;
mod primaries;
mod transfer;

pub use css::{parse_css_color, parse_hex, LinearRgba, Srgba, TRANSPARENT};
pub use gradient::{
    parse_gradient, serialize_gradient, Gradient, GradientStop, DEFAULT_GRADIENT_ANGLE,
    DEFAULT_GRADIENT_STOPS,
};
pub use primaries::{
    apply, convert_matrix, invert, multiply, ColorRange, Mat3, MatrixCoefficients, Primaries,
    IDENTITY,
};
pub use transfer::{
    hlg_to_linear, linear_from_hlg, linear_to_pq, linear_to_rec709, linear_to_srgb, pq_to_linear,
    rec709_to_linear, srgb_to_linear, TransferFunction,
};
