// Each tile visits its intersecting primitives in paint order. Both coverage
// and the 8-bit intermediate rounding follow the CPU compositor.
struct Primitive {
    step_x: vec3f,
    flags: u32,
    first_row: u32,
    y_start: u32,
    y_end: u32,
    padding: u32,
}

struct Row {
    attributes: vec3f,
    span: u32,
}

@group(0) @binding(0) var slides: texture_2d_array<f32>;
@group(0) @binding(1) var output: texture_storage_2d<rgba8unorm, write>;
@group(1) @binding(0) var<storage, read> primitives: array<Primitive>;
@group(1) @binding(1) var<storage, read> rows: array<Row>;
@group(1) @binding(2) var<storage, read> tiles: array<vec2u>;
@group(1) @binding(3) var<storage, read> references: array<u32>;

fn texel(at: vec2i, source: i32) -> vec4f {
    return round(textureLoad(slides, at, source, 0) * 255.0);
}

fn sample_slide(at: vec2f, source: i32, rect: bool) -> vec4f {
    if source == 6 { return vec4f(0., 0., 0., 255.); }
    if source == 7 { return vec4f(255.); }
    if rect { return texel(vec2i(at), source); }
    let last = vec2i(textureDimensions(slides)) - vec2i(1);
    let p = clamp(at, vec2f(0), vec2f(last));
    let lo = vec2i(floor(p));
    let hi = min(lo + 1, last);
    let f = fract(p);
    let a = texel(lo, source);
    let b = texel(vec2i(hi.x, lo.y), source);
    let c = texel(vec2i(lo.x, hi.y), source);
    let d = texel(hi, source);
    let top = a + (b - a) * f.x;
    let bottom = c + (d - c) * f.x;
    return top + (bottom - top) * f.y;
}

@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3u, @builtin(workgroup_id) tile_id: vec3u) {
    let size = textureDimensions(output);
    if any(id.xy >= size) { return; }
    let tiles_x = (size.x + 7u) / 8u;
    let bin = tiles[tile_id.y * tiles_x + tile_id.x];
    var dst = vec4f(0., 0., 0., 255.);
    for (var i = bin.x; i < bin.x + bin.y; i++) {
        let primitive = primitives[references[i]];
        if id.y < primitive.y_start || id.y >= primitive.y_end { continue; }
        let row = rows[primitive.first_row + id.y - primitive.y_start];
        let start = row.span & 65535u;
        if id.x < start || id.x >= (row.span >> 16u) { continue; }
        let attributes = row.attributes + primitive.step_x * f32(id.x - start);
        let opacity = attributes.z;
        if opacity <= 0. { continue; }
        let source = i32(primitive.flags & 7u);
        let rect = (primitive.flags & 8u) != 0u;
        let src = sample_slide(attributes.xy, source, rect);
        if rect && source < 6 && src.a == 255. {
            let alpha = u32(floor(opacity * 65535. + 0.5));
            dst = vec4f((vec4u(src) * alpha + vec4u(dst) * (65535u - alpha) + 32767u) / 65535u);
        } else {
            let keep = 1. - src.a * opacity / 255.;
            dst = clamp(floor(src * opacity + dst * keep + 0.5), vec4f(0.), vec4f(255.));
        }
    }
    textureStore(output, vec2i(id.xy), dst / 255.);
}
