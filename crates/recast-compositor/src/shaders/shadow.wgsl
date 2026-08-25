struct Shadow {
    // centre.xy, half extent .zw, all in canvas pixels
    rect: vec4<f32>,
    // blur, spread, offset_y, corner radius
    shape: vec4<f32>,
    // sRGB-encoded rgb, then opacity 0..1
    tint: vec4<f32>,
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

@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let blur = shadow.shape.x;
    let spread = shadow.shape.y;
    let offset_y = shadow.shape.z;
    let radius = shadow.shape.w;

    let p = frag.xy - shadow.rect.xy - vec2<f32>(0.0, offset_y);
    let half_extent = shadow.rect.zw + vec2<f32>(spread);
    let q = abs(p) - half_extent + vec2<f32>(radius);
    let sd = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;

    let coverage = clamp(1.0 - smoothstep(0.0, blur, sd), 0.0, 1.0);
    let alpha = coverage * shadow.tint.a;
    return vec4<f32>(srgb_to_linear(shadow.tint.rgb) * alpha, alpha);
}
