struct Blur {
    // xy = the UV step between adjacent taps, z = taps per side,
    // w = sigma expressed in those steps.
    params: vec4<f32>,
}

@group(0) @binding(0) var<uniform> blur: Blur;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var src_sampler: sampler;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -3.0), vec2(-1.0, 1.0), vec2(3.0, 1.0));
    return vec4<f32>(p[i], 0.0, 1.0);
}

/// One axis of a separable Gaussian, normalised by the weight actually
/// accumulated so a truncated tail cannot brighten the result.
@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = frag.xy / vec2<f32>(textureDimensions(src));
    let taps = i32(blur.params.z);
    let sigma = max(blur.params.w, 1e-4);

    var total = textureSample(src, src_sampler, uv);
    var weight_sum = 1.0;
    for (var i: i32 = 1; i <= taps; i = i + 1) {
        let t = f32(i) / sigma;
        let weight = exp(-0.5 * t * t);
        let offset = blur.params.xy * f32(i);
        total = total + textureSample(src, src_sampler, uv + offset) * weight;
        total = total + textureSample(src, src_sampler, uv - offset) * weight;
        weight_sum = weight_sum + 2.0 * weight;
    }
    return total / weight_sum;
}
