// Area segment shader — draws filled trapezoids for smooth area charts.
//
// Each instance is an AreaSegment representing a trapezoid:
// - Top edge: diagonal from (x1, y1) to (x2, y2) following the line
// - Bottom edge: horizontal at y = bottom
// 6 vertices per instance (TriangleList: 2 triangles = 1 trapezoid).

struct Viewport {
    width: f32,
    height: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0)
var<uniform> vp: Viewport;

struct AreaSegment {
    @location(0) x1: f32,      // left x
    @location(1) y1: f32,      // top-left y (close price at bar i)
    @location(2) x2: f32,      // right x
    @location(3) y2: f32,      // top-right y (close price at bar i+1)
    @location(4) bottom: f32,  // bottom y (same for both sides)
    @location(5) top_r: f32,
    @location(6) top_g: f32,
    @location(7) top_b: f32,
    @location(8) top_a: f32,
    @location(9) bottom_r: f32,
    @location(10) bottom_g: f32,
    @location(11) bottom_b: f32,
    @location(12) bottom_a: f32,
    @location(13) gradient_top: f32,
    @location(14) _pad1: f32,
    @location(15) _pad2: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    seg: AreaSegment,
    @builtin(vertex_index) vi: u32,
) -> VertexOutput {
    var out: VertexOutput;

    // Trapezoid corners:
    // v0: top-left (x1, y1)
    // v1: top-right (x2, y2)
    // v2: bottom-left (x1, bottom)
    // v3: bottom-right (x2, bottom)
    var corner_x: f32;
    var corner_y: f32;

    // TriangleList: v0, v2, v1, v1, v2, v3
    // First triangle: top-left, bottom-left, top-right
    // Second triangle: top-right, bottom-left, bottom-right
    switch (vi) {
        case 0u: { // v0 top-left
            corner_x = seg.x1;
            corner_y = seg.y1;
        }
        case 1u: { // v2 bottom-left
            corner_x = seg.x1;
            corner_y = seg.bottom;
        }
        case 2u: { // v1 top-right
            corner_x = seg.x2;
            corner_y = seg.y2;
        }
        case 3u: { // v1 top-right
            corner_x = seg.x2;
            corner_y = seg.y2;
        }
        case 4u: { // v2 bottom-left
            corner_x = seg.x1;
            corner_y = seg.bottom;
        }
        case 5u: { // v3 bottom-right
            corner_x = seg.x2;
            corner_y = seg.bottom;
        }
        default: {
            corner_x = 0.0;
            corner_y = 0.0;
        }
    }

    // Pixel → NDC
    let ndc_x = (corner_x / vp.width) * 2.0 - 1.0;
    let ndc_y = 1.0 - (corner_y / vp.height) * 2.0;

    // Global vertical gradient (constant across x) to avoid per-segment facets.
    let grad_h = max(seg.bottom - seg.gradient_top, 1.0);
    let t = clamp((corner_y - seg.gradient_top) / grad_h, 0.0, 1.0);
    let top_color = vec4<f32>(seg.top_r, seg.top_g, seg.top_b, seg.top_a);
    let bottom_color = vec4<f32>(seg.bottom_r, seg.bottom_g, seg.bottom_b, seg.bottom_a);

    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = mix(top_color, bottom_color, t);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Premultiplied alpha
    let a = in.color.a;
    return vec4<f32>(in.color.r * a, in.color.g * a, in.color.b * a, a);
}
