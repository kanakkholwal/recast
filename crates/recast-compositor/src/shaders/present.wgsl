@group(0) @binding(0) var working: texture_2d<f32>;
@group(0) @binding(1) var working_sampler: sampler;

struct Varying {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

/// UV comes from the triangle rather than from `frag.xy / textureDimensions`,
/// because the target is not always the working size: the preview presents a
/// full-resolution composition into a smaller canvas, and dividing by the
/// SOURCE dimensions there would crop the top-left corner instead of scaling.
@vertex
fn vs(@builtin(vertex_index) i: u32) -> Varying {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -3.0), vec2(-1.0, 1.0), vec2(3.0, 1.0));
    let clip = p[i];
    return Varying(
        vec4<f32>(clip, 0.0, 1.0),
        vec2<f32>(clip.x * 0.5 + 0.5, 0.5 - clip.y * 0.5),
    );
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
fn fs(in: Varying) -> @location(0) vec4<f32> {
    let texel = textureSample(working, working_sampler, in.uv);
    let alpha = clamp(texel.a, 0.0, 1.0);
    var linear = texel.rgb;
    if (alpha > 0.0) {
        linear = linear / alpha;
    }
    return vec4<f32>(linear_to_srgb(clamp(linear, vec3<f32>(0.0), vec3<f32>(1.0))), alpha);
}
