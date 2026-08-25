struct Sprite {
    // xy = top-left in canvas px, zw = size in canvas px.
    rect: vec4<f32>,
    // xy = canvas size, z = alpha, w = 1 when the texture is sRGB-encoded.
    frame: vec4<f32>,
}

@group(0) @binding(0) var<uniform> sprite: Sprite;
@group(0) @binding(1) var sprite_tex: texture_2d<f32>;
@group(0) @binding(2) var sprite_sampler: sampler;

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

/// Premultiplied output, because the pass blends with ONE / ONE_MINUS_SRC_ALPHA.
@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = (frag.xy - sprite.rect.xy) / max(sprite.rect.zw, vec2<f32>(1e-4));
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        discard;
    }
    let texel = textureSample(sprite_tex, sprite_sampler, uv);
    var rgb = texel.rgb;
    if (sprite.frame.w > 0.5) {
        rgb = srgb_to_linear(rgb);
    }
    let alpha = texel.a * sprite.frame.z;
    return vec4<f32>(rgb * alpha, alpha);
}
