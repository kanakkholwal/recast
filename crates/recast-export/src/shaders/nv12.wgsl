// RGBA to packed NV12, in the SAME fixed point the CPU encoder uses: float
// maths here would drift a code value and break the byte-identical goldens.

struct Params {
    // Rows of `encode_matrix` scaled by 2^16, one per vec4 so std140 padding is explicit.
    row0: vec4<i32>,
    row1: vec4<i32>,
    row2: vec4<i32>,
    // x,y,z are the per-channel offsets; w is the frame width in pixels.
    offsets: vec4<i32>,
    // x is height, the rest pads to 16 bytes.
    size: vec4<u32>,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> out: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

const FRACTION_BITS: u32 = 16u;
const HALF: i32 = 32768;

fn to_code(scaled: i32) -> u32 {
    return u32(clamp((scaled + HALF) >> FRACTION_BITS, 0, 255));
}

/// The three code values for one pixel, still scaled by 2^16.
fn encode(at: vec2<i32>) -> vec3<i32> {
    // Rgba8Unorm hands back byte/255, and the CPU works on the raw byte.
    let texel = textureLoad(source, at, 0);
    let rgb = vec3<i32>(round(texel.rgb * 255.0));
    return vec3<i32>(
        params.row0.x * rgb.r + params.row0.y * rgb.g + params.row0.z * rgb.b + params.offsets.x,
        params.row1.x * rgb.r + params.row1.y * rgb.g + params.row1.z * rgb.b + params.offsets.y,
        params.row2.x * rgb.r + params.row2.y * rgb.g + params.row2.z * rgb.b + params.offsets.z,
    );
}

// One invocation per 4x2 block: four luma bytes make one aligned u32 per row,
// and the block's two chroma pairs make exactly one more.
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let width = u32(params.offsets.w);
    let height = params.size.x;
    let blocks_x = width / 4u;
    let blocks_y = height / 2u;
    if id.x >= blocks_x || id.y >= blocks_y {
        return;
    }

    let x0 = i32(id.x * 4u);
    let y0 = i32(id.y * 2u);

    var luma: array<vec4<i32>, 2>;
    var chroma_sum: array<vec2<i32>, 2>;
    for (var row = 0u; row < 2u; row = row + 1u) {
        var codes: vec4<i32>;
        for (var col = 0u; col < 4u; col = col + 1u) {
            let code = encode(vec2<i32>(x0 + i32(col), y0 + i32(row)));
            codes[col] = code.x;
            // Columns 0,1 average into the first chroma pair and 2,3 into the second.
            let pair = col / 2u;
            chroma_sum[pair] = chroma_sum[pair] + code.yz;
        }
        luma[row] = codes;
    }

    let row_u32 = width / 4u;
    for (var row = 0u; row < 2u; row = row + 1u) {
        let at = (u32(y0) + row) * row_u32 + id.x;
        let c = luma[row];
        out[at] = to_code(c.x) | (to_code(c.y) << 8u) | (to_code(c.z) << 16u) | (to_code(c.w) << 24u);
    }

    // The chroma plane follows the whole luma plane, and each block writes one u32 of it.
    let chroma_base = width * height / 4u;
    let first = chroma_sum[0] / 4;
    let second = chroma_sum[1] / 4;
    out[chroma_base + id.y * row_u32 + id.x] =
        to_code(first.x) | (to_code(first.y) << 8u) | (to_code(second.x) << 16u)
        | (to_code(second.y) << 24u);
}
