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
    // xy = zoom focus in source UV, z = 1 to cover-fit, w unused.
    focus: vec4<f32>,
    // The tilted card: corners 0,1 then 2,3 in canvas pixels (top-left, top-right, bottom-right, bottom-left),
    // and each corner's homogeneous w. Read only when flags.w > 0.5; the flat path never touches them.
    plane_a: vec4<f32>,
    plane_b: vec4<f32>,
    plane_w: vec4<f32>,
    // contact, rim, rim width (share of the shorter side), spare. All zero is unlit.
    material: vec4<f32>,
}

@group(0) @binding(0) var<uniform> card: Card;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    /// This corner's depth over the card's mean, so the contact term reads how
    /// far away the pixel is without a light or a normal. 1 on the flat path.
    @location(1) shade: f32,
}

fn plane_corner(index: u32) -> vec2<f32> {
    switch index {
        case 0u: { return card.plane_a.xy; }
        case 1u: { return card.plane_a.zw; }
        case 2u: { return card.plane_b.xy; }
        default: { return card.plane_b.zw; }
    }
}

fn plane_depth(index: u32) -> f32 {
    switch index {
        case 0u: { return card.plane_w.x; }
        case 1u: { return card.plane_w.y; }
        case 2u: { return card.plane_w.z; }
        default: { return card.plane_w.w; }
    }
}

/// Signed distance from a pixel to the projected quad's boundary, positive outside; the coverage edge (06, D-2).
fn plane_distance(p: vec2<f32>) -> f32 {
    let c0 = plane_corner(0u);
    let c1 = plane_corner(1u);
    let c2 = plane_corner(2u);
    let c3 = plane_corner(3u);
    // Orientation from the signed area, so the edge normals point outward whichever way the corners wind.
    let area = (c1.x - c0.x) * (c2.y - c0.y) - (c2.x - c0.x) * (c1.y - c0.y);
    let sign = select(1.0, -1.0, area < 0.0);
    var d = -1e9;
    var a = c3;
    for (var i: u32 = 0u; i < 4u; i = i + 1u) {
        let b = plane_corner(i);
        let e = b - a;
        let n = normalize(vec2<f32>(e.y, -e.x)) * sign;
        d = max(d, dot(p - a, n));
        a = b;
    }
    return d;
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
    var pixel = card.rect.xy + half_size + rotated;
    var w = 1.0;
    if (card.flags.w > 0.5) {
        // corner uv to card order: (0,0)=0, (1,0)=1, (1,1)=2, (0,1)=3
        let index = select(select(0u, 1u, corner.x > 0.5), select(3u, 2u, corner.x > 0.5), corner.y > 0.5);
        pixel = plane_corner(index);
        w = plane_depth(index);
    }
    let ndc = vec2<f32>(
        pixel.x / card.canvas.x * 2.0 - 1.0,
        1.0 - pixel.y / card.canvas.y * 2.0,
    );

    var out: VsOut;
    // Scaled by w so the divide the rasteriser does restores ndc and interpolates uv perspective-correctly.
    out.pos = vec4<f32>(ndc * w, 0.0, w);
    out.uv = corner;
    let mean_w = dot(card.plane_w, vec4<f32>(0.25));
    out.shade = select(1.0, w / max(mean_w, 1e-6), card.flags.w > 0.5);
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

/// Darkens what lies further away and lifts the card's own edge. Not a light
/// model: two numbers over the card's own depth and its distance to the border.
fn lit(colour: vec3<f32>, uv: vec2<f32>, shade: f32) -> vec3<f32> {
    let contact = card.material.x;
    let rim = card.material.y;
    if (contact <= 0.0 && rim <= 0.0) {
        return colour;
    }
    var out = colour;
    if (contact > 0.0) {
        // Depth over the mean, so a flat card is untouched and a tilted one
        // darkens only where it recedes.
        let away = clamp(shade - 1.0, 0.0, 1.0);
        out = out * (1.0 - away * contact);
    }
    if (rim > 0.0) {
        let size = card.rect.zw;
        let px = uv * size;
        let edge = min(min(px.x, px.y), min(size.x - px.x, size.y - px.y));
        let width = max(card.material.z * min(size.x, size.y), 1.0);
        out = out + vec3<f32>(rim * (1.0 - smoothstep(0.0, width, edge)));
    }
    return out;
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

    var source_uv = vec2<f32>(
        sx * in.uv.x + shx * in.uv.y + tx,
        shy * in.uv.x + sy * in.uv.y + ty,
    );
    // Cover-fit acts in SOURCE uv, after the affine, so a camera bubble crops
    // its own centre rather than stretching a 16:9 sensor into a square. The
    // size comes from the texture, since a decoded frame carries none of its own
    // through `LayerInput`. Mirroring is NOT here: `bubble_transform` already
    // flips the affine, and doing both cancels out.
    if (card.focus.z > 0.5) {
        let dims = vec2<f32>(textureDimensions(src));
        let source_aspect = dims.x / max(dims.y, 1.0);
        let dest_aspect = card.rect.z / max(card.rect.w, 1.0);
        var fit = vec2<f32>(1.0, 1.0);
        if (source_aspect > dest_aspect) {
            fit.x = dest_aspect / source_aspect;
        } else {
            fit.y = source_aspect / dest_aspect;
        }
        source_uv = (source_uv - vec2<f32>(0.5)) * fit + vec2<f32>(0.5);
    }
    let streak = card.flags.z;
    // else, not an overwrite: the plain sample is a wasted fetch under blur.
    var colour: vec4<f32>;
    if (streak > 0.0) {
        colour = dolly_blur(source_uv, streak);
    } else {
        colour = sample_source(source_uv);
    }

    var alpha = colour.a * opacity;
    if (card.flags.w > 0.5) {
        // The quad's own edge as coverage: one pixel of feather on a diagonal edge instead of a stair.
        alpha = alpha * clamp(0.5 - plane_distance(in.pos.xy), 0.0, 1.0);
    }
    if (radius_fraction > 0.0) {
        let size = card.rect.zw;
        let half_size = size * 0.5;
        let radius = radius_fraction * min(size.x, size.y);
        let d = rounded_box_sdf(in.uv * size - half_size, half_size, radius);
        // One-pixel feather, so the corner is antialiased rather than stepped.
        alpha = alpha * (1.0 - smoothstep(-1.0, 0.0, d));
    }

    return vec4<f32>(lit(colour.rgb, in.uv, in.shade) * alpha, alpha);
}
