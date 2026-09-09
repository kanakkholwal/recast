struct Shadow {
    // centre.xy, half extent .zw, all in canvas pixels
    rect: vec4<f32>,
    // blur, spread, offset_y, corner radius
    shape: vec4<f32>,
    // sRGB-encoded rgb, then opacity 0..1
    tint: vec4<f32>,
    // Rows of the inverse tilt (canvas to flat card); warp2.w = 1 when it applies.
    warp0: vec4<f32>,
    warp1: vec4<f32>,
    warp2: vec4<f32>,
}

@group(0) @binding(0) var<uniform> shadow: Shadow;

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


/// Canvas pixel to the flat card's pixel through the inverse of the card's tilt, when there is one (w > 0.5).
/// Every distance below is then measured on the plane, so a corner radius or a blur is the same size at both edges.
fn unwarp(p: vec2<f32>, r0: vec4<f32>, r1: vec4<f32>, r2: vec4<f32>) -> vec2<f32> {
    if (r2.w < 0.5) {
        return p;
    }
    let w = r2.x * p.x + r2.y * p.y + r2.z;
    let safe = select(w, 1e-6, abs(w) < 1e-6);
    return vec2<f32>(r0.x * p.x + r0.y * p.y + r0.z, r1.x * p.x + r1.y * p.y + r1.z) / safe;
}

@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let blur = shadow.shape.x;
    let spread = shadow.shape.y;
    let offset_y = shadow.shape.z;
    let radius = shadow.shape.w;

    let flat = unwarp(frag.xy, shadow.warp0, shadow.warp1, shadow.warp2);
    let p = flat - shadow.rect.xy - vec2<f32>(0.0, offset_y);
    let half_extent = shadow.rect.zw + vec2<f32>(spread);
    let q = abs(p) - half_extent + vec2<f32>(radius);
    let sd = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;

    let coverage = clamp(1.0 - smoothstep(0.0, blur, sd), 0.0, 1.0);
    let alpha = coverage * shadow.tint.a;
    return vec4<f32>(srgb_to_linear(shadow.tint.rgb) * alpha, alpha);
}
