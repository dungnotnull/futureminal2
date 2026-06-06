struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) bg_color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) bg_color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(in.position, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    out.bg_color = in.bg_color;
    return out;
}

@group(0) @binding(0)
var glyph_atlas: texture_2d<f32>;
@group(0) @binding(1)
var glyph_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // If UV is zero, we are rendering a background quad.
    let is_background = all(in.uv == vec2<f32>(0.0, 0.0));
    if (is_background) {
        return in.bg_color;
    }

    // Sample glyph alpha from atlas and multiply by foreground color.
    let sampled = textureSample(glyph_atlas, glyph_sampler, in.uv);
    let alpha = sampled.r; // R8_UNORM atlas stores alpha in red channel.
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
