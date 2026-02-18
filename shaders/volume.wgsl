// Volume shader — instanced histogram bars.
//
// Each instance: x, volume, bar_width, is_bullish.
// Generates 4 vertices per instance (triangle strip rectangle from 0 to volume).

struct ViewportUniforms {
    projection: mat4x4<f32>,
    width_px: f32,
    height_px: f32,
    visible_bars: f32,
    _pad: f32,
};

@group(0) @binding(0)
var<uniform> viewport: ViewportUniforms;

struct VertexInput {
    @location(0) x: f32,
    @location(1) volume: f32,
    @location(2) bar_width: f32,
    @location(3) is_bullish: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

const BULLISH_VOL: vec4<f32> = vec4<f32>(0.102, 0.737, 0.612, 0.35);
const BEARISH_VOL: vec4<f32> = vec4<f32>(0.906, 0.298, 0.235, 0.35);

@vertex
fn vs_main(
    instance: VertexInput,
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;

    let half_w = instance.bar_width * 0.5;
    var pos: vec2<f32>;

    // Triangle strip: BL, BR, TL, TR
    switch (vertex_index) {
        case 0u: { pos = vec2<f32>(instance.x - half_w, 0.0); }
        case 1u: { pos = vec2<f32>(instance.x + half_w, 0.0); }
        case 2u: { pos = vec2<f32>(instance.x - half_w, instance.volume); }
        case 3u: { pos = vec2<f32>(instance.x + half_w, instance.volume); }
        default: { pos = vec2<f32>(0.0, 0.0); }
    }

    out.position = viewport.projection * vec4<f32>(pos, 0.0, 1.0);
    out.color = select(BEARISH_VOL, BULLISH_VOL, instance.is_bullish > 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
