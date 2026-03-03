// Candle shader — instanced rendering of OHLC candlesticks.
//
// Architecture: the CPU maps f64 OHLCV data to f32 pixel-space coordinates
// relative to the viewport origin (solves the f64→f32 precision trap for
// large timestamps / prices). The shader generates 24 vertices per instance
// (4 quads × 6 verts each):
//
//   quad 0 (verts 0–5):   upper wick  (high_y → body_top_y)
//   quad 1 (verts 6–11):  lower wick  (body_bottom_y → low_y)
//   quad 2 (verts 12–17): border rect (outer body)
//   quad 3 (verts 18–23): body fill   (inner body, inset by border_width)
//
// All coordinates are in physical pixels. The shader converts to NDC.
// Colors come from the uniform buffer, not hardcoded, so the Rust style
// system is the single source of truth.
//
// IMPORTANT — edge computation for odd/even pixel widths:
// We receive FULL widths (not half), then compute edges asymmetrically:
//   left  = floor(center - floor(w / 2))
//   right = left + w
// This guarantees the rect is exactly `w` pixels wide regardless of
// odd/even width or fractional center position. `floor(w/2)` for
// even widths splits perfectly; for odd widths (e.g. wick_width=1),
// the extra pixel goes to the right, matching LWC's convention.

struct CandleUniforms {
    // Viewport dimensions (physical pixels) — for pixel→NDC conversion.
    width: f32,
    height: f32,
    // Candle sizing (physical pixels) — FULL widths, NOT halves.
    bar_width: f32,
    wick_width: f32,
    border_width: f32,
    // 1.0 = draw inner body fill; 0.0 = skip (bar too narrow).
    draw_body: f32,
    _pad0: f32,
    _pad1: f32,
    // Colors — [r, g, b, a] packed as vec4.
    bullish_body: vec4<f32>,
    bearish_body: vec4<f32>,
    bullish_wick: vec4<f32>,
    bearish_wick: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> u: CandleUniforms;

// Per-instance data. All values are in physical pixel space.
// CPU has already done: f64 world → f32 pixel (viewport-relative).
struct CandleInstance {
    @location(0) center_x: f32,    // bar center X in pixels
    @location(1) open_y: f32,      // open price → pixel Y (top-down: 0=top)
    @location(2) high_y: f32,      // high price → pixel Y
    @location(3) low_y: f32,       // low price → pixel Y
    @location(4) close_y: f32,     // close price → pixel Y
    @location(5) state: f32,       // 1.0 = bullish, 0.0 = bearish
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

// Emit a quad corner. vi ∈ [0, 5].
// TriangleList winding: BL, BR, TL, TL, BR, TR.
// left/right/top/bottom in pixel coords (Y-down).
fn quad_corner(vi: u32, left: f32, right: f32, top: f32, bottom: f32) -> vec2<f32> {
    switch (vi) {
        case 0u: { return vec2<f32>(left,  bottom); }
        case 1u: { return vec2<f32>(right, bottom); }
        case 2u: { return vec2<f32>(left,  top); }
        case 3u: { return vec2<f32>(left,  top); }
        case 4u: { return vec2<f32>(right, bottom); }
        case 5u: { return vec2<f32>(right, top); }
        default: { return vec2<f32>(0.0, 0.0); }
    }
}

// Pixel → NDC: x_ndc = (px / width) * 2 - 1, y_ndc = 1 - (py / height) * 2
fn px_to_ndc(px: vec2<f32>) -> vec4<f32> {
    let ndc_x = (px.x / u.width) * 2.0 - 1.0;
    let ndc_y = 1.0 - (px.y / u.height) * 2.0;
    return vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
}

// Asymmetric edge computation: exact pixel width for both odd and even sizes.
//   left  = floor(center) - floor(w / 2)
//   right = left + w
fn edges(center: f32, w: f32) -> vec2<f32> {
    let l = floor(center) - floor(w * 0.5);
    let r = l + w;
    return vec2<f32>(l, r);
}

@vertex
fn vs_main(
    inst: CandleInstance,
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;

    let is_bull = inst.state > 0.5;

    // Body top/bottom in pixel Y (Y-down: smaller Y = higher price).
    // Bullish: close < open in pixel-Y (close is higher price = smaller Y).
    let body_top = floor(min(inst.open_y, inst.close_y));
    let body_bottom_raw = floor(max(inst.open_y, inst.close_y));
    // Ensure minimum 1px body height.
    let body_bottom = max(body_bottom_raw, body_top + 1.0);

    // Asymmetric edge computation for body and wick.
    let bar_edges  = edges(inst.center_x, u.bar_width);
    let wick_edges = edges(inst.center_x, u.wick_width);

    let quad_idx = vertex_index / 6u;
    let vi = vertex_index % 6u;

    let wick_color = select(u.bearish_wick, u.bullish_wick, is_bull);
    let body_color = select(u.bearish_body, u.bullish_body, is_bull);

    var px: vec2<f32>;
    var color: vec4<f32>;

    if (quad_idx == 0u) {
        // Upper wick: from high_y (top) down to body_top.
        px = quad_corner(vi, wick_edges.x, wick_edges.y, floor(inst.high_y), body_top);
        color = wick_color;
    } else if (quad_idx == 1u) {
        // Lower wick: from body_bottom down to low_y.
        px = quad_corner(vi, wick_edges.x, wick_edges.y, body_bottom, floor(inst.low_y));
        color = wick_color;
    } else if (quad_idx == 2u) {
        // Border / outer body rect.
        px = quad_corner(vi, bar_edges.x, bar_edges.y, body_top, body_bottom);
        color = wick_color; // border uses wick color (LWC convention)
    } else {
        // Body fill — inset by border_width.
        let bw = u.border_width;
        let inner_left  = bar_edges.x + bw;
        let inner_right = bar_edges.y - bw;
        let inner_top    = body_top + bw;
        let inner_bottom = body_bottom - bw;
        if (u.draw_body > 0.5 && inner_right > inner_left && inner_bottom > inner_top) {
            px = quad_corner(vi, inner_left, inner_right, inner_top, inner_bottom);
        } else {
            // Degenerate — collapse to zero-area point.
            px = vec2<f32>(inst.center_x, body_top);
        }
        color = body_color;
    }

    out.position = px_to_ndc(px);
    // Pre-multiply alpha for CompositeAlphaMode::PreMultiplied.
    let a = color.a;
    out.color = vec4<f32>(color.r * a, color.g * a, color.b * a, a);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
