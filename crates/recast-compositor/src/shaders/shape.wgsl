const KIND_RECT: u32 = 0u;
const KIND_ELLIPSE: u32 = 1u;
const KIND_ARROW: u32 = 2u;

struct Shape {
    // Rect: xy = origin, zw = size. Ellipse: xy = centre, zw = radii.
    // Arrow: xy = tail, zw = head.
    geom: vec4<f32>,
    // kind, corner radius or arrow head fraction, stroke width, master alpha
    params: vec4<f32>,
    fill: vec4<f32>,
    stroke: vec4<f32>,
}

@group(0) @binding(0) var<uniform> shape: Shape;

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

/// Exact for a circle, a close approximation for an ellipse: the gradient
/// correction keeps the antialiased edge one pixel wide at any aspect.
fn ellipse_sdf(p: vec2<f32>, radii: vec2<f32>) -> f32 {
    let r = max(radii, vec2<f32>(0.0001));
    let k1 = length(p / r);
    let k2 = length(p / (r * r));
    if (k2 <= 0.0) {
        return -min(r.x, r.y);
    }
    return (k1 - 1.0) / k2;
}

fn segment_sdf(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let denom = max(dot(ba, ba), 0.0001);
    let h = clamp(dot(pa, ba) / denom, 0.0, 1.0);
    return length(pa - ba * h);
}

/// Isoceles triangle head at `b`, pointing along `b - a`.
fn arrow_head_sdf(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, head_len: f32) -> f32 {
    let dir = normalize(b - a);
    let normal = vec2<f32>(-dir.y, dir.x);
    let local = p - b;
    let along = dot(local, dir);
    let across = abs(dot(local, normal));
    let half_width = head_len * 0.5;
    // Distance to the wedge spanning [-head_len, 0] along the axis.
    let taper = half_width * clamp(-along / max(head_len, 0.0001), 0.0, 1.0);
    let outside_axis = max(along, -along - head_len);
    return max(outside_axis, across - taper);
}

@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let kind = u32(shape.params.x);
    let detail = shape.params.y;
    let stroke_width = shape.params.z;
    let alpha = shape.params.w;

    var sd = 1e9;
    if (kind == KIND_RECT) {
        let half_size = shape.geom.zw * 0.5;
        sd = rounded_box_sdf(frag.xy - shape.geom.xy - half_size, half_size, detail);
    } else if (kind == KIND_ELLIPSE) {
        sd = ellipse_sdf(frag.xy - shape.geom.xy, shape.geom.zw);
    } else {
        let a = shape.geom.xy;
        let b = shape.geom.zw;
        let head_len = length(b - a) * detail;
        let shaft = segment_sdf(frag.xy, a, b) - max(stroke_width, 1.0) * 0.5;
        sd = min(shaft, arrow_head_sdf(frag.xy, a, b, head_len));
    }

    let fill_coverage = clamp(1.0 - smoothstep(-0.5, 0.5, sd), 0.0, 1.0);
    var colour = srgb_to_linear(shape.fill.rgb);
    var coverage = fill_coverage * shape.fill.a;

    // The arrow is stroke-only: its silhouette IS the stroke, so a second
    // outline band would double it.
    if (stroke_width > 0.0 && kind != KIND_ARROW) {
        let band = abs(sd) - stroke_width * 0.5;
        let stroke_coverage =
            clamp(1.0 - smoothstep(-0.5, 0.5, band), 0.0, 1.0) * shape.stroke.a;
        let stroke_colour = srgb_to_linear(shape.stroke.rgb);
        colour = mix(colour, stroke_colour, stroke_coverage);
        coverage = max(coverage, stroke_coverage);
    } else if (kind == KIND_ARROW) {
        colour = srgb_to_linear(shape.stroke.rgb);
        coverage = fill_coverage * shape.stroke.a;
    }

    let out_alpha = coverage * alpha;
    return vec4<f32>(colour * out_alpha, out_alpha);
}
