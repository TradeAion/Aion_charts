// Line segment shader — draws anti-aliased line segments as rotated quads.
//
// Each instance is a LineSegment: { x1, y1, x2, y2, width, r, g, b, a }
// The shader computes perpendicular offsets to create a properly rotated quad.
// 6 vertices per instance (TriangleList: 2 triangles = 1 quad).

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
    @location(1) edge_dist: f32,  // Distance from center for AA
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
        out.edge_dist = 0.0;
        return out;
    }

    // Normalized direction
    let nx = dx / len;
    let ny = dy / len;

    // Perpendicular (rotated 90 degrees)
    let px = -ny;
    let py = nx;

    // Half-width offset
    let hw = seg.line_width * 0.5;
    let ox = px * hw;
    let oy = py * hw;

    // Four corner positions:
    // v0: start - perpendicular offset (bottom-left of line)
    // v1: start + perpendicular offset (top-left of line)
    // v2: end - perpendicular offset (bottom-right of line)
    // v3: end + perpendicular offset (top-right of line)
    var corner_x: f32;
    var corner_y: f32;
    var edge: f32;

    // TriangleList: v0, v2, v1, v1, v2, v3
    switch (vi) {
        case 0u: { corner_x = seg.x1 - ox; corner_y = seg.y1 - oy; edge = -1.0; } // v0
        case 1u: { corner_x = seg.x2 - ox; corner_y = seg.y2 - oy; edge = -1.0; } // v2
        case 2u: { corner_x = seg.x1 + ox; corner_y = seg.y1 + oy; edge = 1.0; }  // v1
        case 3u: { corner_x = seg.x1 + ox; corner_y = seg.y1 + oy; edge = 1.0; }  // v1
        case 4u: { corner_x = seg.x2 - ox; corner_y = seg.y2 - oy; edge = -1.0; } // v2
        case 5u: { corner_x = seg.x2 + ox; corner_y = seg.y2 + oy; edge = 1.0; }  // v3
        default: { corner_x = 0.0; corner_y = 0.0; edge = 0.0; }
    }

    // Pixel → NDC
    let ndc_x = (corner_x / vp.width) * 2.0 - 1.0;
    let ndc_y = 1.0 - (corner_y / vp.height) * 2.0;

    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = vec4<f32>(seg.r, seg.g, seg.b, seg.a);
    out.edge_dist = edge;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Simple anti-aliasing: fade out at edges
    // edge_dist goes from -1 to 1 across the line width
    let dist = abs(in.edge_dist);
    let aa_width = 0.7; // Pixels for AA smoothing
    let alpha = in.color.a * smoothstep(1.0, 1.0 - aa_width, dist);
    
    // Premultiplied alpha
    return vec4<f32>(in.color.r * alpha, in.color.g * alpha, in.color.b * alpha, alpha);
}
