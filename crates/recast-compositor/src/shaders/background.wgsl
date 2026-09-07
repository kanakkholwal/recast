const MAX_STOPS: u32 = 8u;

struct Background {
    // x = stop count (0 means solid), y = angle in radians, zw = canvas size.
    header: vec4<f32>,
    solid: vec4<f32>,
    // x = 1 when an image is bound, yz = cover-fit UV scale, w = sRGB decode.
    image: vec4<f32>,
    // rgb = the stop colour in sRGB-encoded 0..1, a = its position in 0..1.
    stops: array<vec4<f32>, MAX_STOPS>,
}

@group(0) @binding(0) var<uniform> bg: Background;
@group(0) @binding(1) var image_tex: texture_2d<f32>;
@group(0) @binding(2) var image_sampler: sampler;

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

/// CSS gradient geometry: 0deg points to the top and the angle turns clockwise.
/// The line is centred on the box and long enough to cover the far corners.
fn gradient_t(pos: vec2<f32>, size: vec2<f32>, angle: f32) -> f32 {
    let dir = vec2<f32>(sin(angle), -cos(angle));
    let half_size = size * 0.5;
    let centred = pos - half_size;
    let extent = abs(half_size.x * dir.x) + abs(half_size.y * dir.y);
    if (extent <= 0.0) {
        return 0.0;
    }
    return clamp((dot(centred, dir) + extent) / (2.0 * extent), 0.0, 1.0);
}

/// Stops are interpolated in sRGB, matching CSS and the editor preview, and the
/// result is converted to linear once. Interpolating in linear would be a
/// different gradient from the one the user authored.
fn sample_gradient(t: f32, count: u32) -> vec3<f32> {
    var result = bg.stops[0].rgb;
    if (t <= bg.stops[0].a) {
        return result;
    }
    for (var i: u32 = 1u; i < count; i = i + 1u) {
        let a = bg.stops[i - 1u];
        let b = bg.stops[i];
        if (t <= b.a) {
            let span = b.a - a.a;
            var f = 0.0;
            if (span > 0.0) {
                f = (t - a.a) / span;
            }
            return mix(a.rgb, b.rgb, f);
        }
        result = b.rgb;
    }
    return result;
}

@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    if (bg.image.x > 0.5) {
        // Cover fit, centred: the scale is below 1 on the axis being cropped.
        let uv = (frag.xy / bg.header.zw - 0.5) * bg.image.yz + 0.5;
        let texel = textureSample(image_tex, image_sampler, uv);
        var rgb = texel.rgb;
        if (bg.image.w > 0.5) {
            rgb = srgb_to_linear(rgb);
        }
        return vec4<f32>(rgb, 1.0);
    }

    let count = u32(bg.header.x);
    if (count == 0u) {
        return vec4<f32>(srgb_to_linear(bg.solid.rgb) * bg.solid.a, bg.solid.a);
    }
    let t = gradient_t(frag.xy, bg.header.zw, bg.header.y);
    let encoded = sample_gradient(t, count);
    return vec4<f32>(srgb_to_linear(encoded), 1.0);
}
