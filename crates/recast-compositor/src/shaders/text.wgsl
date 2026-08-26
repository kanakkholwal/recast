struct Canvas {
    // xy = canvas size in px, zw unused
    size: vec4<f32>,
}

@group(0) @binding(0) var<uniform> canvas: Canvas;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct Instance {
    // xy = top-left in canvas px, zw = size in px
    @location(0) rect: vec4<f32>,
    // u0, v0, u1, v1 in atlas uv
    @location(1) uv: vec4<f32>,
    // sRGB rgb, master alpha in a
    @location(2) colour: vec4<f32>,
}

struct Varying {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) colour: vec4<f32>,
}

@vertex
fn vs(@builtin(vertex_index) i: u32, instance: Instance) -> Varying {
    var corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
    );
    let corner = corners[i];
    let px = instance.rect.xy + corner * instance.rect.zw;
    let ndc = vec2<f32>(
        px.x / max(canvas.size.x, 1.0) * 2.0 - 1.0,
        1.0 - px.y / max(canvas.size.y, 1.0) * 2.0,
    );

    var out: Varying;
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = mix(instance.uv.xy, instance.uv.zw, corner);
    out.colour = instance.colour;
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let cutoff = step(c, vec3<f32>(0.04045));
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return mix(high, low, cutoff);
}

@fragment
fn fs(in: Varying) -> @location(0) vec4<f32> {
    let coverage = textureSample(atlas, atlas_sampler, in.uv).r;
    let alpha = coverage * in.colour.a;
    return vec4<f32>(srgb_to_linear(in.colour.rgb) * alpha, alpha);
}
