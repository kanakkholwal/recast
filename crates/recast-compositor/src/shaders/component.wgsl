struct Component {
    // The surface this instance draws in: xy = origin, zw = size, canvas pixels.
    rect: vec4<f32>,
    // canvas width, canvas height, progress through the instance's window, recipe.
    canvas: vec4<f32>,
    // Recipe parameters; the meaning is the recipe's.
    p0: vec4<f32>,
    // More parameters; w is always the surface's corner radius in pixels.
    p1: vec4<f32>,
    // Linear-light colour the recipe paints with.
    tint: vec4<f32>,
    // A second colour, for the recipes that blend two.
    tint2: vec4<f32>,
    // Canvas to flat-surface, for an instance riding a tilted card; warp2.w = 0 is flat.
    warp0: vec4<f32>,
    warp1: vec4<f32>,
    warp2: vec4<f32>,
}

@group(0) @binding(0) var<uniform> component: Component;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -3.0), vec2(-1.0, 1.0), vec2(3.0, 1.0));
    return vec4<f32>(p[i], 0.0, 1.0);
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

/// Alternating diagonal stripes: what an instance draws when its component
/// cannot be resolved. Loud on purpose, and never nothing.
fn placeholder(uv: vec2<f32>, size: vec2<f32>) -> f32 {
    let px = uv * size;
    let stripe = fract((px.x + px.y) / 24.0);
    let inside = step(0.5, stripe) * 0.55;
    let border = 1.0 - step(3.0, min(min(px.x, px.y), min(size.x - px.x, size.y - px.y)));
    return max(inside, border);
}

/// Darkens outside a soft ellipse. p0 = centre xy, radius, feather; p1.x = strength.
fn spotlight(uv: vec2<f32>, size: vec2<f32>) -> f32 {
    let aspect = size.x / max(size.y, 1.0);
    let here = vec2<f32>(uv.x * aspect, uv.y);
    let centre = vec2<f32>(component.p0.x * aspect, component.p0.y);
    let radius = max(component.p0.z, 1e-4);
    let inner = radius * (1.0 - clamp(component.p0.w, 0.0, 1.0));
    return smoothstep(inner, radius, distance(here, centre)) * component.p1.x;
}

/// A rounded panel wiping in and out along an angle.
/// p0 = corner radius (fraction), angle (radians), softness, and the wipe front,
/// which the CPU computes so the panel and any words on it cannot drift apart.
fn shape_reveal(uv: vec2<f32>, size: vec2<f32>) -> f32 {
    let half_size = size * 0.5;
    let radius = component.p0.x * min(size.x, size.y);
    let sd = rounded_box_sdf(uv * size - half_size, half_size, radius);
    let coverage = clamp(1.0 - smoothstep(-0.5, 0.5, sd), 0.0, 1.0);

    let angle = component.p0.y;
    let axis = vec2<f32>(cos(angle), sin(angle));
    // Projected onto the wipe axis and renormalised, so the front crosses the
    // whole panel whatever the angle.
    let extent = abs(axis.x) + abs(axis.y);
    let t = (dot(uv - vec2<f32>(0.5), axis) / max(extent, 1e-4)) + 0.5;

    let softness = max(component.p0.z, 1e-3);
    let front = component.p0.w;
    return coverage * (1.0 - smoothstep(front - softness, front, t));
}

/// A gloss band travelling across the surface.
/// p0 = width, angle (radians), cycles, strength.
fn sweep(uv: vec2<f32>, size: vec2<f32>) -> f32 {
    let half_size = size * 0.5;
    let sd = rounded_box_sdf(uv * size - half_size, half_size, component.p1.w);
    let coverage = clamp(1.0 - smoothstep(-0.5, 0.5, sd), 0.0, 1.0);

    let angle = component.p0.y;
    let axis = vec2<f32>(cos(angle), sin(angle));
    let extent = abs(axis.x) + abs(axis.y);
    let t = dot(uv - vec2<f32>(0.5), axis) / max(extent, 1e-4);

    let head = fract(component.canvas.z * component.p0.z) * 2.0 - 1.0;
    let width = max(component.p0.x, 1e-3);
    let band = exp(-pow((t - head) / width, 2.0) * 4.0);
    return coverage * band * component.p0.w;
}

/// Two soft blooms orbiting each other. p0 = spread, cycles, strength.
/// The only recipe that mixes a colour per pixel, so it returns one.
fn mesh(uv: vec2<f32>, size: vec2<f32>) -> vec4<f32> {
    let aspect = size.x / max(size.y, 1.0);
    let here = vec2<f32>(uv.x * aspect, uv.y);
    let turn = component.canvas.z * component.p0.y * 6.2831853;
    let spread = max(component.p0.x, 1e-3);

    let a_centre = vec2<f32>((0.5 + 0.26 * cos(turn)) * aspect, 0.5 + 0.26 * sin(turn));
    let b_centre = vec2<f32>((0.5 - 0.26 * cos(turn * 0.8)) * aspect, 0.5 - 0.26 * sin(turn * 0.8));
    let a_weight = exp(-pow(distance(here, a_centre) / spread, 2.0) * 2.0);
    let b_weight = exp(-pow(distance(here, b_centre) / spread, 2.0) * 2.0);

    let total = a_weight + b_weight;
    let colour = mix(component.tint.rgb, component.tint2.rgb, b_weight / max(total, 1e-4));
    return vec4<f32>(colour, clamp(total, 0.0, 1.0) * component.p0.z);
}

@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let px = unwarp(frag.xy, component.warp0, component.warp1, component.warp2);
    let size = max(component.rect.zw, vec2<f32>(1.0));
    let uv = (px - component.rect.xy) / size;
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        discard;
    }

    let recipe = u32(component.canvas.w + 0.5);
    var colour = component.tint.rgb;
    var alpha = 0.0;
    switch recipe {
        case 1u: { alpha = spotlight(uv, size); }
        case 2u: { alpha = shape_reveal(uv, size); }
        case 3u: { alpha = sweep(uv, size); }
        case 4u: {
            let bloom = mesh(uv, size);
            colour = bloom.rgb;
            alpha = bloom.a;
        }
        // A title card and a lower third are the same panel; only their words and their box differ.
        case 5u, 6u: { alpha = shape_reveal(uv, size); }
        default: { alpha = placeholder(uv, size); }
    }

    alpha = clamp(alpha, 0.0, 1.0) * component.tint.a;
    if (alpha <= 0.0) {
        discard;
    }
    return vec4<f32>(colour * alpha, alpha);
}
