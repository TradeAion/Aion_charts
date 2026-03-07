struct Viewport {
    width: f32,
    height: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0)
var<uniform> vp: Viewport;

struct GradientRectInstance {
    @location(0) x: f32,
    @location(1) y: f32,
    @location(2) w: f32,
    @location(3) h: f32,
    @location(4) left_r: f32,
    @location(5) left_g: f32,
    @location(6) left_b: f32,
    @location(7) left_a: f32,
    @location(8) right_r: f32,
    @location(9) right_g: f32,
    @location(10) right_b: f32,
    @location(11) right_a: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    instance: GradientRectInstance,
    @builtin(vertex_index) vi: u32,
) -> VertexOutput {
    var out: VertexOutput;
    var px: f32;
    var py: f32;
    var color: vec4<f32>;

    switch (vi) {
        case 0u: {
            px = instance.x;
            py = instance.y + instance.h;
            color = vec4<f32>(instance.left_r, instance.left_g, instance.left_b, instance.left_a);
        }
        case 1u: {
            px = instance.x + instance.w;
            py = instance.y + instance.h;
            color = vec4<f32>(instance.right_r, instance.right_g, instance.right_b, instance.right_a);
        }
        case 2u: {
            px = instance.x;
            py = instance.y;
            color = vec4<f32>(instance.left_r, instance.left_g, instance.left_b, instance.left_a);
        }
        case 3u: {
            px = instance.x;
            py = instance.y;
            color = vec4<f32>(instance.left_r, instance.left_g, instance.left_b, instance.left_a);
        }
        case 4u: {
            px = instance.x + instance.w;
            py = instance.y + instance.h;
            color = vec4<f32>(instance.right_r, instance.right_g, instance.right_b, instance.right_a);
        }
        case 5u: {
            px = instance.x + instance.w;
            py = instance.y;
            color = vec4<f32>(instance.right_r, instance.right_g, instance.right_b, instance.right_a);
        }
        default: {
            px = 0.0;
            py = 0.0;
            color = vec4<f32>(0.0);
        }
    }

    let ndc_x = (px / vp.width) * 2.0 - 1.0;
    let ndc_y = 1.0 - (py / vp.height) * 2.0;

    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let a = in.color.a;
    return vec4<f32>(in.color.r * a, in.color.g * a, in.color.b * a, a);
}
