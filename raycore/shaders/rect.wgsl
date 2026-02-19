// Unified rect shader — trivially draws a colored rectangle.
//
// Each instance is a ColoredRect: { x, y, w, h, r, g, b, a } in physical pixels.
// The shader converts pixel coords to NDC using viewport dimensions.
// 6 vertices per instance (TriangleList: 2 triangles = 1 quad).

struct Viewport {
    width: f32,
    height: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0)
var<uniform> vp: Viewport;

struct RectInstance {
    @location(0) x: f32,
    @location(1) y: f32,
    @location(2) w: f32,
    @location(3) h: f32,
    @location(4) r: f32,
    @location(5) g: f32,
    @location(6) b: f32,
    @location(7) a: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    instance: RectInstance,
    @builtin(vertex_index) vi: u32,
) -> VertexOutput {
    var out: VertexOutput;

    // Quad corners in pixel space
    var px: f32;
    var py: f32;

    // TriangleList: BL, BR, TL, TL, BR, TR
    switch (vi) {
        case 0u: { px = instance.x;              py = instance.y + instance.h; }
        case 1u: { px = instance.x + instance.w; py = instance.y + instance.h; }
        case 2u: { px = instance.x;              py = instance.y; }
        case 3u: { px = instance.x;              py = instance.y; }
        case 4u: { px = instance.x + instance.w; py = instance.y + instance.h; }
        case 5u: { px = instance.x + instance.w; py = instance.y; }
        default: { px = 0.0; py = 0.0; }
    }

    // Pixel → NDC:  x_ndc = (px / width) * 2 - 1,  y_ndc = 1 - (py / height) * 2
    let ndc_x = (px / vp.width) * 2.0 - 1.0;
    let ndc_y = 1.0 - (py / vp.height) * 2.0;

    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = vec4<f32>(instance.r, instance.g, instance.b, instance.a);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Output premultiplied alpha — required for CompositeAlphaMode::PreMultiplied.
    // The browser composites this canvas over the grid canvas; transparent pixels
    // let the grid show through.
    let a = in.color.a;
    return vec4<f32>(in.color.r * a, in.color.g * a, in.color.b * a, a);
}
