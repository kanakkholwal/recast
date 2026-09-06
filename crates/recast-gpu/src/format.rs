/// The linear-light working format the whole graph composites in.
pub const WORKING_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// What a driver presents or reads back: 8-bit sRGB-encoded.
pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Single-channel masks (shadow, blur alpha, luma planes).
pub const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

pub fn is_linear_float(format: wgpu::TextureFormat) -> bool {
    matches!(
        format,
        wgpu::TextureFormat::Rgba16Float | wgpu::TextureFormat::Rgba32Float
    )
}

/// True when sampling the format applies the sRGB EOTF in hardware. Sampling one
/// of these into the linear working space would double-decode.
pub fn is_srgb_encoded(format: wgpu::TextureFormat) -> bool {
    format.is_srgb()
}

/// Bytes one row of `width` pixels occupies, rounded up to the 256-byte
/// alignment `copy_texture_to_buffer` requires.
pub fn aligned_bytes_per_row(width: u32, format: wgpu::TextureFormat) -> u32 {
    let unpadded = width * format.block_copy_size(None).unwrap_or(4);
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    unpadded.div_ceil(align) * align
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_working_space_is_linear_float() {
        assert!(is_linear_float(WORKING_FORMAT));
        assert!(!is_linear_float(OUTPUT_FORMAT));
    }

    #[test]
    fn srgb_encoded_formats_are_recognised_so_they_are_not_double_decoded() {
        assert!(is_srgb_encoded(wgpu::TextureFormat::Rgba8UnormSrgb));
        assert!(is_srgb_encoded(wgpu::TextureFormat::Bgra8UnormSrgb));
        assert!(!is_srgb_encoded(wgpu::TextureFormat::Rgba8Unorm));
        assert!(!is_srgb_encoded(WORKING_FORMAT));
    }

    #[test]
    fn row_padding_meets_the_copy_alignment() {
        for width in [1u32, 63, 64, 100, 1920, 3840] {
            let padded = aligned_bytes_per_row(width, OUTPUT_FORMAT);
            assert_eq!(padded % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT, 0);
            assert!(padded >= width * 4);
        }
    }

    #[test]
    fn an_already_aligned_row_is_not_padded_further() {
        assert_eq!(aligned_bytes_per_row(64, OUTPUT_FORMAT), 256);
        assert_eq!(aligned_bytes_per_row(1920, OUTPUT_FORMAT), 7680);
    }
}
