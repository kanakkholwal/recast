@group(0) @binding(0) var working: texture_2d<f32>;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -3.0), vec2(-1.0, 1.0), vec2(3.0, 1.0));
    return vec4<f32>(p[i], 0.0, 1.0);
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let cutoff = step(c, vec3<f32>(0.0031308));
    let low = c * 12.92;
    let high = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return mix(high, low, cutoff);
}

/// SDR output: clamp, un-premultiply, then apply the sRGB OETF. No filmic curve
/// yet; HDR tonemapping is a separate pass when the HDR scope is decided.
@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let texel = textureLoad(working, vec2<i32>(frag.xy), 0);
    let alpha = clamp(texel.a, 0.0, 1.0);
    var linear = texel.rgb;
    if (alpha > 0.0) {
        linear = linear / alpha;
    }
    return vec4<f32>(linear_to_srgb(clamp(linear, vec3<f32>(0.0), vec3<f32>(1.0))), alpha);
}
