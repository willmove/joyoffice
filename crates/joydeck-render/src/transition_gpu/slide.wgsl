// Original projected edges are evaluated in the fragment shader so neighbouring tiles
// have the same coverage even on hardware with a coarse rasterizer subpixel grid.
struct Vertex {
    @builtin(position) position: vec4f,
    @location(0) @interpolate(flat) edge0: vec4f,
    @location(1) @interpolate(flat) edge1: vec4f,
    @location(2) @interpolate(flat) edge2: vec4f,
    @location(3) @interpolate(flat) factors: vec3f,
    @location(4) @interpolate(flat) flags: u32,
    @location(5) @interpolate(flat) attr0: vec3f,
    @location(6) @interpolate(flat) attr1: vec3f,
    @location(7) @interpolate(flat) attr2: vec3f,
    @location(8) @interpolate(flat) origins: vec3f,
}

@vertex
fn vertex(@builtin(vertex_index) index: u32, @location(0) box: vec4f,
          @location(1) a: vec4f, @location(2) b: vec4f, @location(3) c: vec4f,
          @location(4) origins: vec3f, @location(5) factors: vec3f, @location(6) flags: u32,
          @location(7) attr0: vec3f, @location(8) attr1: vec3f, @location(9) attr2: vec3f) -> Vertex {
    let pos = vec2f(select(box.x, box.z, index == 1u || index == 2u || index == 4u),
                    select(box.y, box.w, index == 2u || index == 4u || index == 5u));
    return Vertex(vec4f(pos * vec2f(2, -2) + vec2f(-1, 1), 0, 1),
                  a, b, c, factors, flags, attr0, attr1, attr2, origins);
}

@group(0) @binding(0) var slides: texture_2d_array<f32>;

// The independent coordinate is x for a shallow edge and y for a steep one.
fn distance(pixel: vec2f, coefficients: vec4f, origin: f32, swap: bool) -> f32 {
    let at = select(pixel, pixel.yx, swap);
    return fma(-coefficients.x, at.y - origin, at.x - coefficients.z) -
           fma(coefficients.y, at.y - origin, coefficients.w);
}

@fragment
fn fragment(v: Vertex) -> @location(0) vec4f {
    let pixel = floor(v.position.xy);
    let weights = v.factors * vec3f(distance(pixel, v.edge0, v.origins.x, (v.flags & 8u) != 0u),
        distance(pixel, v.edge1, v.origins.y, (v.flags & 16u) != 0u), distance(pixel, v.edge2, v.origins.z, (v.flags & 32u) != 0u));
    if any(weights < vec3f(0)) { discard; }
    let attributes = v.attr0 * weights.x + v.attr1 * weights.y + v.attr2 * weights.z;
    let source = v.flags & 7u;
    if source == 6u { return vec4f(0, 0, 0, attributes.z); }
    if source == 7u { return vec4f(attributes.z); }
    let last = vec2i(textureDimensions(slides)) - vec2i(1);
    let at = clamp(attributes.xy * vec2f(last + 1) - 0.5, vec2f(0), vec2f(last));
    let lo = vec2i(floor(at));
    let hi = min(lo + 1, last);
    let f = fract(at);
    let top = mix(textureLoad(slides, lo, i32(source), 0),
                  textureLoad(slides, vec2i(hi.x, lo.y), i32(source), 0), f.x);
    let bottom = mix(textureLoad(slides, vec2i(lo.x, hi.y), i32(source), 0),
                     textureLoad(slides, hi, i32(source), 0), f.x);
    return mix(top, bottom, f.y) * attributes.z;
}
