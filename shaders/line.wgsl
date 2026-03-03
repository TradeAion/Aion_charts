// Line segment shader — capsule-based anti-aliased strokes.
//
// Each instance is a LineSegment:
//   { x1, y1, x2, y2, width, r, g, b, a, _pad }
//
// Compared to a plain rotated-quad stroke, this shader computes alpha from
// signed distance to a line capsule (segment + round end caps), which avoids:
// - "broken"/stippled appearance on slight tilt
// - visible segment joints in brush/freehand strokes
//
// The quad geometry is expanded just enough to contain the capsule + AA band.

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
    // Local coordinates in line space:
    // x: distance along centerline (0 at start, len at end)
    // y: signed perpendicular distance from centerline
    @location(1) local_x: f32,
    @location(2) local_y: f32,
    @location(3) seg_len: f32,
    @location(4) half_width: f32,
    @location(5) aa_band: f32,
};

@vertex
fn vs_main(
    seg: LineSegment,
    @builtin(vertex_index) vi: u32,
) -> VertexOutput {
    var out: VertexOutput;

    let dx = seg.x2 - seg.x1;
    let dy = seg.y2 - seg.y1;
    let len = sqrt(dx * dx + dy * dy);

    if len < 0.001 {
        out.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        out.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        out.local_x = 0.0;
        out.local_y = 0.0;
        out.seg_len = 0.0;
        out.half_width = 0.0;
        out.aa_band = 0.0;
        return out;
    }

    let nx = dx / len;
    let ny = dy / len;
    let px = -ny;
    let py = nx;

    let hw = max(seg.line_width * 0.5, 0.5);
    let aa = 0.5;
    let extend = hw + aa;
    let total_hw = hw + aa;

    // Expand ends for round caps + AA.
    let sx = seg.x1 - nx * extend;
    let sy = seg.y1 - ny * extend;
    let ex = seg.x2 + nx * extend;
    let ey = seg.y2 + ny * extend;

    // Perpendicular offsets.
    let ox = px * total_hw;
    let oy = py * total_hw;

    // Local coordinates for capsule distance evaluation.
    // Start vertices use x = -extend, end vertices use x = len + extend.
    var corner_x: f32;
    var corner_y: f32;
    var lx: f32;
    var ly: f32;

    // TriangleList: v0, v2, v1, v1, v2, v3
    // v0 = start - perp, v1 = start + perp, v2 = end - perp, v3 = end + perp
    switch (vi) {
        case 0u: { corner_x = sx - ox; corner_y = sy - oy; lx = -extend;        ly = -total_hw; } // v0
        case 1u: { corner_x = ex - ox; corner_y = ey - oy; lx = len + extend;   ly = -total_hw; } // v2
        case 2u: { corner_x = sx + ox; corner_y = sy + oy; lx = -extend;        ly = total_hw;  } // v1
        case 3u: { corner_x = sx + ox; corner_y = sy + oy; lx = -extend;        ly = total_hw;  } // v1
        case 4u: { corner_x = ex - ox; corner_y = ey - oy; lx = len + extend;   ly = -total_hw; } // v2
        case 5u: { corner_x = ex + ox; corner_y = ey + oy; lx = len + extend;   ly = total_hw;  } // v3
        default: { corner_x = 0.0; corner_y = 0.0; lx = 0.0; ly = 0.0; }
    }

    let ndc_x = (corner_x / vp.width) * 2.0 - 1.0;
    let ndc_y = 1.0 - (corner_y / vp.height) * 2.0;

    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = vec4<f32>(seg.r, seg.g, seg.b, seg.a);
    out.local_x = lx;
    out.local_y = ly;
    out.seg_len = len;
    out.half_width = hw;
    out.aa_band = aa;
    return out;
}

fn capsule_signed_distance(local_x: f32, local_y: f32, len: f32, hw: f32) -> f32 {
    let y = abs(local_y);
    if (local_x < 0.0) {
        // Distance to start cap circle.
        return length(vec2<f32>(local_x, y)) - hw;
    }
    if (local_x > len) {
        // Distance to end cap circle.
        return length(vec2<f32>(local_x - len, y)) - hw;
    }
    // Distance to segment body.
    return y - hw;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sd = capsule_signed_distance(in.local_x, in.local_y, in.seg_len, in.half_width);
    let aaf = max(in.aa_band, 0.001);
    let edge_alpha = 1.0 - smoothstep(-aaf, aaf, sd);
    let alpha = in.color.a * edge_alpha;
    // Premultiplied-alpha output.
    return vec4<f32>(in.color.rgb * alpha, alpha);
}
