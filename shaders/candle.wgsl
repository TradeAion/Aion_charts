// Candle shader — instanced rendering of OHLC candles.
//
// Each instance provides: x, open, high, low, close, bar_width (legacy, unused).
// The vertex shader generates 24 vertices per instance (4 quads × 6 verts each):
//   quad 0 (verts 0-5):   upper wick  (high → body_top)
//   quad 1 (verts 6-11):  lower wick  (body_bottom → low)
//   quad 2 (verts 12-17): border rect (outer body)
//   quad 3 (verts 18-23): body fill   (inner body, inset by border_width)
//
// Pixel-exact sizing: all widths come from uniforms in physical pixels,
// converted to bar-index units via px_per_bar.

struct ViewportUniforms {
    projection: mat4x4<f32>,
    width_px: f32,
    height_px: f32,
    visible_bars: f32,
    px_per_bar: f32,
    bar_width_px: f32,
    wick_width_px: f32,
    border_width_px: f32,
    draw_body: f32,
};

@group(0) @binding(0)
var<uniform> viewport: ViewportUniforms;

struct VertexInput {
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

// Colors — match LWC defaults
const BULLISH_BODY: vec4<f32> = vec4<f32>(0.102, 0.737, 0.612, 1.0);
const BEARISH_BODY: vec4<f32> = vec4<f32>(0.906, 0.298, 0.235, 1.0);
const BULLISH_WICK: vec4<f32> = vec4<f32>(0.102, 0.737, 0.612, 1.0);
const BEARISH_WICK: vec4<f32> = vec4<f32>(0.906, 0.298, 0.235, 1.0);

// Build a quad from 6 vertices (two triangles: TriangleList).
// vi: 0..5 within quad.
fn quad_vertex(vi: u32, half_w: f32, bottom: f32, top: f32) -> vec2<f32> {
    // Triangle 1: BL, BR, TL.  Triangle 2: TL, BR, TR.
    switch (vi) {
        case 0u: { return vec2<f32>(-half_w, bottom); }
        case 1u: { return vec2<f32>( half_w, bottom); }
        case 2u: { return vec2<f32>(-half_w, top); }
        case 3u: { return vec2<f32>(-half_w, top); }
        case 4u: { return vec2<f32>( half_w, bottom); }
        case 5u: { return vec2<f32>( half_w, top); }
        default: { return vec2<f32>(0.0, 0.0); }
    }
}

@vertex
fn vs_main(
    instance: VertexInput,
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;

    let is_bullish = instance.close >= instance.open;
    let body_top = select(instance.open, instance.close, is_bullish);
    let body_bottom = select(instance.close, instance.open, is_bullish);

    // Pixel widths → bar-index-space widths
    let ppb = max(viewport.px_per_bar, 0.001);
    let bar_half_w = (viewport.bar_width_px / ppb) * 0.5;
    let wick_half_w = (viewport.wick_width_px / ppb) * 0.5;

    // Convert border_width from pixels to price units.
    // projection[1][1] = 2 / (price_max - price_min) for orthographic_rh.
    // height_px = candle viewport height in physical pixels (set by backend).
    // 1 pixel in price units = price_range / height_px.
    let price_range = 2.0 / abs(viewport.projection[1][1]);
    let candle_h_px = viewport.height_px;
    let border_price = viewport.border_width_px * price_range / candle_h_px;

    // Minimum body height: 1 pixel in price units
    let min_body = price_range / candle_h_px;
    let raw_body = body_top - body_bottom;
    let actual_top = select(body_top, body_bottom + min_body, raw_body < min_body);

    let quad_idx = vertex_index / 6u;
    let vi = vertex_index % 6u;

    var pos: vec2<f32>;
    var color: vec4<f32>;

    let wick_color = select(BEARISH_WICK, BULLISH_WICK, is_bullish);
    let body_color = select(BEARISH_BODY, BULLISH_BODY, is_bullish);

    if (quad_idx == 0u) {
        // Upper wick: from body_top to high
        let offs = quad_vertex(vi, wick_half_w, actual_top, instance.high);
        pos = vec2<f32>(instance.x + offs.x, offs.y);
        color = wick_color;
    } else if (quad_idx == 1u) {
        // Lower wick: from low to body_bottom
        let offs = quad_vertex(vi, wick_half_w, instance.low, body_bottom);
        pos = vec2<f32>(instance.x + offs.x, offs.y);
        color = wick_color;
    } else if (quad_idx == 2u) {
        // Border rect (outer body)
        let offs = quad_vertex(vi, bar_half_w, body_bottom, actual_top);
        pos = vec2<f32>(instance.x + offs.x, offs.y);
        color = wick_color; // border color = wick color (like LWC)
    } else {
        // Body fill (inset by border)
        if (viewport.draw_body > 0.5) {
            let inner_half_w = bar_half_w - (viewport.border_width_px / ppb);
            let inner_bottom = body_bottom + border_price;
            let inner_top = actual_top - border_price;
            if (inner_half_w > 0.0 && inner_top > inner_bottom) {
                let offs = quad_vertex(vi, inner_half_w, inner_bottom, inner_top);
                pos = vec2<f32>(instance.x + offs.x, offs.y);
            } else {
                pos = vec2<f32>(instance.x, body_bottom);
            }
        } else {
            pos = vec2<f32>(instance.x, body_bottom);
        }
        color = body_color;
    }

    out.position = viewport.projection * vec4<f32>(pos, 0.0, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
