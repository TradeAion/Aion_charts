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
    @location(5) r: f32,
    @location(6) g: f32,
    @location(7) b: f32,
    @location(8) a: f32,
    @location(9) _pad: f32,
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
        case 0u: { corner_x = seg.x1; corner_y = seg.y1; }      // v0 top-left
        case 1u: { corner_x = seg.x1; corner_y = seg.bottom; }  // v2 bottom-left
        case 2u: { corner_x = seg.x2; corner_y = seg.y2; }      // v1 top-right
        case 3u: { corner_x = seg.x2; corner_y = seg.y2; }      // v1 top-right
        case 4u: { corner_x = seg.x1; corner_y = seg.bottom; }  // v2 bottom-left
        case 5u: { corner_x = seg.x2; corner_y = seg.bottom; }  // v3 bottom-right
        default: { corner_x = 0.0; corner_y = 0.0; }
    }

    // Pixel → NDC
    let ndc_x = (corner_x / vp.width) * 2.0 - 1.0;
    let ndc_y = 1.0 - (corner_y / vp.height) * 2.0;

    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = vec4<f32>(seg.r, seg.g, seg.b, seg.a);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Premultiplied alpha
    let a = in.color.a;
    return vec4<f32>(in.color.r * a, in.color.g * a, in.color.b * a, a);
}
