struct Region {
    // xy = origin, zw = size, all in canvas pixels.
    rect: vec4<f32>,
    // corner radius, unused, unused, unused
    params: vec4<f32>,
    // Wash over the blurred pixels; a = 0 leaves them clear.
    tint: vec4<f32>,
}

@group(0) @binding(0) var<uniform> region: Region;
@group(0) @binding(1) var blurred: texture_2d<f32>;
@group(0) @binding(2) var blurred_sampler: sampler;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -3.0), vec2(-1.0, 1.0), vec2(3.0, 1.0));
    return vec4<f32>(p[i], 0.0, 1.0);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let cutoff = step(c, vec3<f32>(0.04045));
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return mix(high, low, cutoff);
}

fn rounded_box_sdf(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let r = min(radius, min(half_size.x, half_size.y));
    let q = abs(p) - half_size + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

/// Replaces the rect with the pre-blurred copy of what was underneath, so the
/// source is sampled at the fragment's own position rather than through a
/// transform. Opacity does NOT fade the blur itself, matching `paintBlur`:
/// a half-faded redaction would leak what it is there to hide.
@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let half_size = region.rect.zw * 0.5;
    let sd = rounded_box_sdf(frag.xy - region.rect.xy - half_size, half_size, region.params.x);
    let coverage = clamp(1.0 - smoothstep(-0.5, 0.5, sd), 0.0, 1.0);
    if (coverage <= 0.0) {
        discard;
    }

    let texel = textureSample(blurred, blurred_sampler, frag.xy / vec2<f32>(textureDimensions(blurred)));
    // The working texture is premultiplied; un-premultiply to mix the wash.
    let alpha = clamp(texel.a, 0.0, 1.0);
    var colour = texel.rgb;
    if (alpha > 0.0) {
        colour = colour / alpha;
    }
    colour = mix(colour, srgb_to_linear(region.tint.rgb), region.tint.a);

    let out_alpha = coverage * max(alpha, region.tint.a);
    return vec4<f32>(colour * out_alpha, out_alpha);
}
