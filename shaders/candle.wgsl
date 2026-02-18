// Candle shader — instanced rendering of OHLC candles (body + wicks).
//
// Each instance provides: x, open, high, low, close, bar_width.
// The vertex shader generates 12 vertices per instance:
//   0-3:  candle body (triangle strip quad)
//   4-7:  upper wick (thin quad)
//   8-11: lower wick (thin quad)

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
    // Instance attributes
    @location(0) x: f32,
    @location(1) open: f32,
    @location(2) high: f32,
    @location(3) low: f32,
    @location(4) close: f32,
    @location(5) bar_width: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

// Colors
const BULLISH_BODY: vec4<f32> = vec4<f32>(0.102, 0.737, 0.612, 1.0);   // #1ABC9C
const BEARISH_BODY: vec4<f32> = vec4<f32>(0.906, 0.298, 0.235, 1.0);   // #E74C3C
const BULLISH_WICK: vec4<f32> = vec4<f32>(0.102, 0.737, 0.612, 0.9);
const BEARISH_WICK: vec4<f32> = vec4<f32>(0.906, 0.298, 0.235, 0.9);

@vertex
fn vs_main(
    instance: VertexInput,
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;

    let is_bullish = instance.close >= instance.open;
    let body_top = select(instance.open, instance.close, is_bullish);
    let body_bottom = select(instance.close, instance.open, is_bullish);

    let half_w = instance.bar_width * 0.5;
    // Wick is 1/8th the width of the body, minimum 1px equivalent
    let wick_half_w = max(instance.bar_width * 0.0625, 0.02);

    var pos: vec2<f32>;
    var color: vec4<f32>;

    let part = vertex_index / 4u;   // 0=body, 1=upper_wick, 2=lower_wick
    let vi = vertex_index % 4u;     // vertex within quad

    if (part == 0u) {
        // Body quad: triangle strip order: BL, BR, TL, TR
        let body_min_height = (body_top - body_bottom);
        let actual_top = select(body_top, body_bottom + 0.001, body_min_height < 0.001);
        switch (vi) {
            case 0u: { pos = vec2<f32>(instance.x - half_w, body_bottom); }
            case 1u: { pos = vec2<f32>(instance.x + half_w, body_bottom); }
            case 2u: { pos = vec2<f32>(instance.x - half_w, actual_top); }
            case 3u: { pos = vec2<f32>(instance.x + half_w, actual_top); }
            default: { pos = vec2<f32>(0.0, 0.0); }
        }
        color = select(BEARISH_BODY, BULLISH_BODY, is_bullish);
    } else if (part == 1u) {
        // Upper wick: from body_top to high
        switch (vi) {
            case 0u: { pos = vec2<f32>(instance.x - wick_half_w, body_top); }
            case 1u: { pos = vec2<f32>(instance.x + wick_half_w, body_top); }
            case 2u: { pos = vec2<f32>(instance.x - wick_half_w, instance.high); }
            case 3u: { pos = vec2<f32>(instance.x + wick_half_w, instance.high); }
            default: { pos = vec2<f32>(0.0, 0.0); }
        }
        color = select(BEARISH_WICK, BULLISH_WICK, is_bullish);
    } else {
        // Lower wick: from low to body_bottom
        switch (vi) {
            case 0u: { pos = vec2<f32>(instance.x - wick_half_w, instance.low); }
            case 1u: { pos = vec2<f32>(instance.x + wick_half_w, instance.low); }
            case 2u: { pos = vec2<f32>(instance.x - wick_half_w, body_bottom); }
            case 3u: { pos = vec2<f32>(instance.x + wick_half_w, body_bottom); }
            default: { pos = vec2<f32>(0.0, 0.0); }
        }
        color = select(BEARISH_WICK, BULLISH_WICK, is_bullish);
    }

    out.position = viewport.projection * vec4<f32>(pos, 0.0, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
