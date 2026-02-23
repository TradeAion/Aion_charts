// Line segment shader — draws crisp line segments as rotated quads.
//
// Each instance is a LineSegment: { x1, y1, x2, y2, width, r, g, b, a }
// The shader computes perpendicular offsets to create a properly rotated quad.
// 6 vertices per instance (TriangleList: 2 triangles = 1 quad).
//
// For crisp 1px lines, we use pixel-perfect coordinates without AA.
// For thicker lines, minimal edge smoothing is applied.

struct Viewport {
    width: f32,
    height: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0)
var<uniform> vp: Viewport;

struct LineSegment {
    @location(0) x1: f32,
    @location(1) y1: f32,
    @location(2) x2: f32,
    @location(3) y2: f32,
    @location(4) line_width: f32,
    @location(5) r: f32,
    @location(6) g: f32,
    @location(7) b: f32,
    @location(8) a: f32,
    @location(9) _pad: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_y: f32,      // Position within line width (-0.5 to 0.5)
    @location(2) half_width: f32,   // Half line width for AA calculation
};

@vertex
fn vs_main(
    seg: LineSegment,
    @builtin(vertex_index) vi: u32,
) -> VertexOutput {
    var out: VertexOutput;

    // Direction vector
    let dx = seg.x2 - seg.x1;
    let dy = seg.y2 - seg.y1;
    let len = sqrt(dx * dx + dy * dy);
    
    // Avoid division by zero for degenerate segments
    if len < 0.001 {
        out.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        out.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        out.local_y = 0.0;
        out.half_width = 0.0;
        return out;
    }

    // Normalized direction
    let nx = dx / len;
    let ny = dy / len;

    // Perpendicular (rotated 90 degrees)
    let px = -ny;
    let py = nx;

    // Half-width for the quad (add 0.5px for AA feathering on thick lines)
    let hw = seg.line_width * 0.5;
    let aa_extend = select(0.0, 0.5, seg.line_width > 1.5);
    let total_hw = hw + aa_extend;
    
    let ox = px * total_hw;
    let oy = py * total_hw;

    // Four corner positions:
    // v0: start - perpendicular offset (bottom-left of line)
    // v1: start + perpendicular offset (top-left of line)
    // v2: end - perpendicular offset (bottom-right of line)
    // v3: end + perpendicular offset (top-right of line)
    var corner_x: f32;
    var corner_y: f32;
    var local: f32;  // -0.5 at one edge, +0.5 at other edge

    // TriangleList: v0, v2, v1, v1, v2, v3
    switch (vi) {
        case 0u: { corner_x = seg.x1 - ox; corner_y = seg.y1 - oy; local = -total_hw; } // v0
        case 1u: { corner_x = seg.x2 - ox; corner_y = seg.y2 - oy; local = -total_hw; } // v2
        case 2u: { corner_x = seg.x1 + ox; corner_y = seg.y1 + oy; local = total_hw; }  // v1
        case 3u: { corner_x = seg.x1 + ox; corner_y = seg.y1 + oy; local = total_hw; }  // v1
        case 4u: { corner_x = seg.x2 - ox; corner_y = seg.y2 - oy; local = -total_hw; } // v2
        case 5u: { corner_x = seg.x2 + ox; corner_y = seg.y2 + oy; local = total_hw; }  // v3
        default: { corner_x = 0.0; corner_y = 0.0; local = 0.0; }
    }

    // Pixel → NDC
    let ndc_x = (corner_x / vp.width) * 2.0 - 1.0;
    let ndc_y = 1.0 - (corner_y / vp.height) * 2.0;

    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = vec4<f32>(seg.r, seg.g, seg.b, seg.a);
    out.local_y = local;
    out.half_width = hw;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Distance from line center (in pixels)
    let dist = abs(in.local_y);
    let hw = in.half_width;
    
    // For thin lines (<=1.5px), render solid without AA for crispness
    // For thicker lines, apply 1px edge smoothing
    var alpha: f32;
    if hw <= 0.75 {
        // Thin line: solid fill, no AA (crisp)
        alpha = in.color.a * step(dist, hw);
    } else {
        // Thicker line: smooth edge over 1px
        alpha = in.color.a * (1.0 - smoothstep(hw - 0.5, hw + 0.5, dist));
    }
    
    // Premultiplied alpha output
    return vec4<f32>(in.color.rgb * alpha, alpha);
}
