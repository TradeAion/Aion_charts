//! Converts the anti-aliased geometry subset of the Prim IR (`Polyline` / `AreaFill` / `Circle`)
//! into triangle-mesh vertices for the wgpu tri pipeline.
//!
//! The crisp-rect subset goes through [`prims_to_instances`](crate::prims_to_instances); this is
//! its companion for tessellated geometry. Together they let both backends consume one shared prim
//! list — wgpu here, the Canvas2D executor in `aion_render` (roadmap Phase D2). Previously the live
//! line/area builders tessellated straight to tris, so the Canvas2D fallback had nothing to render.
//!
//! The shared point pool holds **device-space** points (the builders already baked the DPR in), so
//! tessellation runs with identity pixel ratios — byte-identical to the old direct-to-tri path.

use aion_render::draw_list::{LineType, Prim};
use aion_render::line::{
    build_area_fill, build_disc, build_line_stroke, AreaMesh, LinePoint, LineParams, LineVertex,
    StrokeMesh,
};

use crate::tri_pipeline::TriVertex;

fn tri(v: &LineVertex) -> TriVertex {
    TriVertex { pos: [v.x, v.y], color: v.color }
}

/// Slice a `[first, first+count)` window of the shared device-space pool into `LinePoint`s.
fn pool_slice(points: &[[f32; 2]], first: u32, count: u32) -> Vec<LinePoint> {
    let (a, b) = (first as usize, (first + count) as usize);
    points
        .get(a..b)
        .unwrap_or(&[])
        .iter()
        .map(|p| LinePoint { x: p[0] as f64, y: p[1] as f64 })
        .collect()
}

/// Identity `LineParams` — the pool already carries the DPR, so tessellation must not re-scale.
fn identity(line_width: f64, line_type: LineType) -> LineParams {
    LineParams { horizontal_pixel_ratio: 1.0, vertical_pixel_ratio: 1.0, line_width, line_type }
}

/// Tessellate the geometry prims into `fill` (area fills, drawn first/below) and `stroke` (line
/// strokes + filled discs). Rects, text, and unhandled prims are ignored — they render elsewhere.
pub fn geom_prims_to_tris(
    prims: &[Prim],
    points: &[[f32; 2]],
    fill: &mut Vec<TriVertex>,
    stroke: &mut Vec<TriVertex>,
) {
    for prim in prims {
        match prim {
            Prim::AreaFill { first_point, point_count, base_y, line_type, gradient } => {
                let pts = pool_slice(points, *first_point, *point_count);
                let mut mesh = AreaMesh::default();
                build_area_fill(
                    &pts,
                    *base_y as f64,
                    gradient.top,
                    gradient.bottom,
                    &identity(0.0, *line_type),
                    &mut mesh,
                );
                fill.extend(mesh.vertices.iter().map(tri));
            }
            Prim::Polyline { first_point, point_count, width, line_type, color, .. } => {
                let pts = pool_slice(points, *first_point, *point_count);
                let mut mesh = StrokeMesh::default();
                build_line_stroke(&pts, *color, &identity(*width as f64, *line_type), &mut mesh);
                stroke.extend(mesh.vertices.iter().map(tri));
            }
            Prim::Circle { cx, cy, radius, fill: f, .. } => {
                let mut disc = Vec::new();
                build_disc([*cx, *cy], *radius, *f, &mut disc);
                stroke.extend(disc.iter().map(tri));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aion_render::color::Color;
    use aion_render::draw_list::Gradient;

    #[test]
    fn polyline_tessellates_to_stroke_only() {
        let points = [[0.0f32, 0.0], [10.0, 10.0], [20.0, 0.0]];
        let prims = [Prim::Polyline {
            first_point: 0,
            point_count: 3,
            width: 2.0,
            style: aion_render::draw_list::LineStyle::Solid,
            line_type: LineType::Simple,
            color: Color::rgb(0, 0, 0xFF),
        }];
        let (mut fill, mut stroke) = (Vec::new(), Vec::new());
        geom_prims_to_tris(&prims, &points, &mut fill, &mut stroke);
        assert!(fill.is_empty());
        assert!(!stroke.is_empty(), "two segments + a join tessellate to tris");
    }

    #[test]
    fn area_fill_tessellates_to_fill_only() {
        let points = [[0.0f32, 10.0], [20.0, 4.0]];
        let prims = [Prim::AreaFill {
            first_point: 0,
            point_count: 2,
            base_y: 40.0,
            line_type: LineType::Simple,
            gradient: Gradient { top: Color::rgb(0, 0, 0xFF), bottom: Color::rgba(0, 0, 0xFF, 0) },
        }];
        let (mut fill, mut stroke) = (Vec::new(), Vec::new());
        geom_prims_to_tris(&prims, &points, &mut fill, &mut stroke);
        assert!(stroke.is_empty());
        assert_eq!(fill.len(), 6, "one quad -> two tris -> six vertices");
    }

    #[test]
    fn circle_tessellates_to_stroke() {
        let prims = [Prim::Circle {
            cx: 5.0,
            cy: 5.0,
            radius: 3.0,
            fill: Color::rgb(0xFF, 0, 0),
            stroke_width: 0.0,
            stroke: Color::rgb(0, 0, 0),
        }];
        let (mut fill, mut stroke) = (Vec::new(), Vec::new());
        geom_prims_to_tris(&prims, &[], &mut fill, &mut stroke);
        assert!(fill.is_empty());
        assert!(!stroke.is_empty());
    }

    #[test]
    fn rects_and_text_ignored() {
        use aion_render::draw_list::IRect;
        let prims = [
            Prim::Rect { rect: IRect { x: 0, y: 0, w: 5, h: 5 }, color: Color::rgb(0, 0, 0) },
            Prim::Text { run_id: 0, x: 0.0, y: 0.0, color: Color::rgb(0, 0, 0) },
        ];
        let (mut fill, mut stroke) = (Vec::new(), Vec::new());
        geom_prims_to_tris(&prims, &[], &mut fill, &mut stroke);
        assert!(fill.is_empty() && stroke.is_empty());
    }
}
