struct Region {
    // xy = origin, zw = size, all in canvas pixels.
    rect: vec4<f32>,
    // corner radius, master alpha, fit (0 fill, 1 cover, 2 contain), unused
    params: vec4<f32>,
    tint: vec4<f32>,
    // Canvas to flat-rect, for an image riding a tilted card; warp2.w = 0 is flat.
    warp0: vec4<f32>,
    warp1: vec4<f32>,
    warp2: vec4<f32>,
}

@group(0) @binding(0) var<uniform> region: Region;
@group(0) @binding(1) var image: texture_2d<f32>;
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

fn unwarp(p: vec2<f32>, r0: vec4<f32>, r1: vec4<f32>, r2: vec4<f32>) -> vec2<f32> {
    if (r2.w < 0.5) {
        return p;
    }
    let w = r2.x * p.x + r2.y * p.y + r2.z;
    let safe = select(w, 1e-6, abs(w) < 1e-6);
    return vec2<f32>(r0.x * p.x + r0.y * p.y + r0.z, r1.x * p.x + r1.y * p.y + r1.z) / safe;
}

fn rounded_box_sdf(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let r = min(radius, min(half_size.x, half_size.y));
    let q = abs(p) - half_size + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

/// Reshapes the sampled uv so the source keeps its aspect inside the box.
/// `cover` crops the long edge, `contain` leaves the remainder clear.
fn fitted(uv: vec2<f32>, box_size: vec2<f32>) -> vec2<f32> {
    let mode = region.params.z;
    if (mode < 0.5) {
        return uv;
    }
    let source = vec2<f32>(textureDimensions(image));
    let source_aspect = source.x / max(source.y, 1.0);
    let box_aspect = box_size.x / max(box_size.y, 1.0);
    var scale = vec2<f32>(1.0, 1.0);
    let wider = source_aspect > box_aspect;
    // Cover shrinks the sampled window on the long edge; contain grows it, which letterboxes.
    if ((mode < 1.5) == wider) {
        scale.x = box_aspect / source_aspect;
    } else {
        scale.y = source_aspect / box_aspect;
    }
    return (uv - vec2<f32>(0.5)) * scale + vec2<f32>(0.5);
}

/// An annotation stretches to its rect, matching `paintImage`'s
/// `drawImage(img, x, y, w, h)`: the editor's resize handles are what set the
/// aspect. A composition item picks its own fit.
@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let px = unwarp(frag.xy, region.warp0, region.warp1, region.warp2);
    let half_size = region.rect.zw * 0.5;
    let sd = rounded_box_sdf(px - region.rect.xy - half_size, half_size, region.params.x);
    let coverage = clamp(1.0 - smoothstep(-0.5, 0.5, sd), 0.0, 1.0);
    if (coverage <= 0.0) {
        discard;
    }

    let uv = fitted((px - region.rect.xy) / max(region.rect.zw, vec2<f32>(1.0)), region.rect.zw);
    if (region.params.z > 1.5 && (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0)) {
        discard;
    }
    let texel = textureSample(image, image_sampler, uv);
    // Uploaded straight from an ImageBitmap, so the samples are sRGB-encoded
    // and un-premultiplied.
    let alpha = clamp(texel.a, 0.0, 1.0) * coverage * region.params.y;
    return vec4<f32>(srgb_to_linear(texel.rgb) * alpha, alpha);
}
