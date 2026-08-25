struct Card {
    // Destination rect in canvas pixels: xy = origin, zw = size.
    rect: vec4<f32>,
    // Canvas size in pixels, then the source affine's first row is in `affine_a`.
    canvas: vec4<f32>,
    // sx, shx, tx, shy
    affine_a: vec4<f32>,
    // sy, ty, opacity, corner radius as a fraction of the shorter card edge
    affine_b: vec4<f32>,
    // x = 1 when the source needs the sRGB EOTF applied in the shader,
    // y = rotation in radians about the card centre,
    // z = dolly-blur streak length in source UV (already folded with velocity),
    // w unused.
    flags: vec4<f32>,
    // xy = zoom focus in source UV, zw unused.
    focus: vec4<f32>,
}

@group(0) @binding(0) var<uniform> card: Card;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(1.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0),
    );
    let corner = corners[i];
    let half_size = card.rect.zw * 0.5;
    let local = corner * card.rect.zw - half_size;
    let angle = card.flags.y;
    let rotated = vec2<f32>(
        local.x * cos(angle) - local.y * sin(angle),
        local.x * sin(angle) + local.y * cos(angle),
    );
    let pixel = card.rect.xy + half_size + rotated;
    let ndc = vec2<f32>(
        pixel.x / card.canvas.x * 2.0 - 1.0,
        1.0 - pixel.y / card.canvas.y * 2.0,
    );

    var out: VsOut;
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = corner;
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let cutoff = step(c, vec3<f32>(0.04045));
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return mix(high, low, cutoff);
}

fn sample_source(uv: vec2<f32>) -> vec4<f32> {
    var colour = textureSampleLevel(src, samp, clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0);
    if (card.flags.x > 0.5) {
        colour = vec4<f32>(srgb_to_linear(colour.rgb), colour.a);
    }
    return colour;
}

const BLUR_TAPS: i32 = 11;

/// Radial streak along the line from the zoom focus, which is what a dolly
/// actually smears. Averaged in LINEAR light, so a bright edge does not darken
/// the way an sRGB-space average would.
fn dolly_blur(uv: vec2<f32>, streak: f32) -> vec4<f32> {
    let direction = uv - card.focus.xy;
    var total = vec4<f32>(0.0);
    for (var i: i32 = 0; i < BLUR_TAPS; i = i + 1) {
        let t = (f32(i) / f32(BLUR_TAPS - 1) - 0.5) * 2.0;
        total = total + sample_source(uv + direction * streak * t);
    }
    return total / f32(BLUR_TAPS);
}

/// Signed distance to a rounded rectangle centred on the origin. Negative inside.
fn rounded_box_sdf(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(radius);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let sx = card.affine_a.x;
    let shx = card.affine_a.y;
    let tx = card.affine_a.z;
    let shy = card.affine_a.w;
    let sy = card.affine_b.x;
    let ty = card.affine_b.y;
    let opacity = card.affine_b.z;
    let radius_fraction = card.affine_b.w;

    let source_uv = vec2<f32>(
        sx * in.uv.x + shx * in.uv.y + tx,
        shy * in.uv.x + sy * in.uv.y + ty,
    );
    var colour = sample_source(source_uv);
    let streak = card.flags.z;
    if (streak > 0.0) {
        colour = dolly_blur(source_uv, streak);
    }

    var alpha = colour.a * opacity;
    if (radius_fraction > 0.0) {
        let size = card.rect.zw;
        let half_size = size * 0.5;
        let radius = radius_fraction * min(size.x, size.y);
        let d = rounded_box_sdf(in.uv * size - half_size, half_size, radius);
        // One-pixel feather, so the corner is antialiased rather than stepped.
        alpha = alpha * (1.0 - smoothstep(-1.0, 0.0, d));
    }

    return vec4<f32>(colour.rgb * alpha, alpha);
}
