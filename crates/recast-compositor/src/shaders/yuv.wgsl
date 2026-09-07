struct Yuv {
    // Rows of the folded Y'CbCr to R'G'B' matrix, w holding that row's bias.
    decode0: vec4<f32>,
    decode1: vec4<f32>,
    decode2: vec4<f32>,
    gamut0: vec4<f32>,
    gamut1: vec4<f32>,
    gamut2: vec4<f32>,
    // xy = chroma texel offset, zw unused.
    chroma: vec4<f32>,
    // x = plane layout, y = transfer function, zw unused.
    codes: vec4<u32>,
}

@group(0) @binding(0) var<uniform> yuv: Yuv;
@group(0) @binding(1) var plane0: texture_2d<f32>;
@group(0) @binding(2) var plane1: texture_2d<f32>;
@group(0) @binding(3) var plane2: texture_2d<f32>;
@group(0) @binding(4) var samp: sampler;

const LAYOUT_NV12: u32 = 0u;

const TRANSFER_SRGB: u32 = 0u;
const TRANSFER_LINEAR: u32 = 1u;
const TRANSFER_GAMMA22: u32 = 2u;
const TRANSFER_REC709: u32 = 3u;
const TRANSFER_PQ: u32 = 4u;
const TRANSFER_HLG: u32 = 5u;

const PQ_M1: f32 = 0.15930176;
const PQ_M2: f32 = 78.84375;
const PQ_C1: f32 = 0.8359375;
const PQ_C2: f32 = 18.851562;
const PQ_C3: f32 = 18.6875;

const HLG_A: f32 = 0.17883277;
const HLG_B: f32 = 0.28466892;
const HLG_C: f32 = 0.5599107;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -3.0), vec2(-1.0, 1.0), vec2(3.0, 1.0));
    return vec4<f32>(p[i], 0.0, 1.0);
}

/// `den` bottoms out near 0.16 for input on the unit interval, which the
/// fragment has already clamped to, so this needs no divide-by-zero guard.
fn pq_to_linear(c: f32) -> f32 {
    let e = pow(c, 1.0 / PQ_M2);
    let num = max(e - PQ_C1, 0.0);
    let den = PQ_C2 - PQ_C3 * e;
    return pow(num / den, 1.0 / PQ_M1);
}

fn hlg_to_linear(c: f32) -> f32 {
    if (c <= 0.5) {
        return c * c / 3.0;
    }
    return (exp((c - HLG_C) / HLG_A) + HLG_B) / 12.0;
}

/// Kept in step with `recast_color::TransferFunction` by the parity test, which
/// renders a ramp through here and compares it to the Rust side.
fn to_linear(code: u32, c: f32) -> f32 {
    switch (code) {
        case TRANSFER_LINEAR: {
            return c;
        }
        case TRANSFER_GAMMA22: {
            return pow(c, 2.2);
        }
        case TRANSFER_REC709: {
            if (c < 0.081) {
                return c / 4.5;
            }
            return pow((c + 0.099) / 1.099, 1.0 / 0.45);
        }
        case TRANSFER_PQ: {
            return pq_to_linear(c);
        }
        case TRANSFER_HLG: {
            return hlg_to_linear(c);
        }
        default: {
            if (c <= 0.04045) {
                return c / 12.92;
            }
            return pow((c + 0.055) / 1.055, 2.4);
        }
    }
}

@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let luma_dims = vec2<f32>(textureDimensions(plane0));
    let uv = frag.xy / luma_dims;
    let chroma_dims = vec2<f32>(textureDimensions(plane1));
    let chroma_uv = uv + yuv.chroma.xy / max(chroma_dims, vec2<f32>(1.0));

    let y = textureSampleLevel(plane0, samp, uv, 0.0).r;
    var cb: f32;
    var cr: f32;
    if (yuv.codes.x == LAYOUT_NV12) {
        let pair = textureSampleLevel(plane1, samp, chroma_uv, 0.0);
        cb = pair.r;
        cr = pair.g;
    } else {
        cb = textureSampleLevel(plane1, samp, chroma_uv, 0.0).r;
        cr = textureSampleLevel(plane2, samp, chroma_uv, 0.0).r;
    }

    let ycc = vec3<f32>(y, cb, cr);
    // Clamped because limited-range footroom and headroom decode outside 0..1,
    // and the transfer functions are only defined on the unit interval.
    let encoded = clamp(
        vec3<f32>(
            dot(yuv.decode0.xyz, ycc) + yuv.decode0.w,
            dot(yuv.decode1.xyz, ycc) + yuv.decode1.w,
            dot(yuv.decode2.xyz, ycc) + yuv.decode2.w,
        ),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );

    let transfer = yuv.codes.y;
    let light = vec3<f32>(
        to_linear(transfer, encoded.r),
        to_linear(transfer, encoded.g),
        to_linear(transfer, encoded.b),
    );

    return vec4<f32>(
        dot(yuv.gamut0.xyz, light),
        dot(yuv.gamut1.xyz, light),
        dot(yuv.gamut2.xyz, light),
        1.0,
    );
}
