//! Rectangle drawing — 2-anchor filled rectangle with border.

use super::drawing::{
    generate_anchor_circles, next_drawing_id, point_to_bitmap, point_to_css, Drawing,
};
use super::hit_test;
use super::types::*;
use crate::core::renderer::draw_list::{ColoredLine, ColoredRect};
use crate::core::viewport::Viewport;

#[derive(Debug)]
pub struct RectangleDrawing {
    id: u64,
    state: DrawingState,
    style: DrawingStyle,
    anchors: Vec<AnchorPoint>,
}

impl RectangleDrawing {
    pub fn new(bar_index: f64, price: f64) -> Self {
        let id = next_drawing_id();
        Self {
            id,
            state: DrawingState::Creating { step: 1 },
            style: DrawingStyle::rectangle_from_theme(
                &crate::core::renderer::theme::ThemeConfig::default(),
            ),
            anchors: vec![
                AnchorPoint::new(bar_index, price),
                AnchorPoint::new(bar_index, price),
            ],
        }
    }
}

impl Drawing for RectangleDrawing {
    fn id(&self) -> u64 {
        self.id
    }
    fn tool(&self) -> DrawingTool {
        DrawingTool::Rectangle
    }
    fn state(&self) -> DrawingState {
        self.state
    }
    fn set_state(&mut self, state: DrawingState) {
        self.state = state;
    }
    fn style(&self) -> &DrawingStyle {
        &self.style
    }
    fn style_mut(&mut self) -> &mut DrawingStyle {
        &mut self.style
    }
    fn anchors(&self) -> &[AnchorPoint] {
        &self.anchors
    }
    fn anchors_mut(&mut self) -> &mut Vec<AnchorPoint> {
        &mut self.anchors
    }
    fn required_anchors(&self) -> usize {
        2
    }

    fn hit_test(&self, cx: f64, cy: f64, vp: &Viewport, pw: f64, ph: f64) -> HitResult {
        if self.anchors.len() < 2 {
            return HitResult::miss();
        }

        let (x0, y0) = point_to_css(&self.anchors[0].point, vp, pw, ph);
        let (x1, y1) = point_to_css(&self.anchors[1].point, vp, pw, ph);

        // Check anchors first
        for (i, a) in self.anchors.iter().enumerate() {
            let (ax, ay) = point_to_css(&a.point, vp, pw, ph);
            let d = hit_test::point_to_circle_distance(cx, cy, ax, ay);
            if d <= hit_test::ANCHOR_HIT_THRESHOLD_CSS {
                return HitResult::hit(HitPart::Anchor(i), d);
            }
        }

        // Check if inside the rectangle fill
        if hit_test::point_in_rect(cx, cy, x0, y0, x1, y1) {
            let d = hit_test::point_to_rect_edge_distance(cx, cy, x0, y0, x1, y1);
            // Near the edge → Edge (draggable), deep interior → Body (pan pass-through)
            if d <= hit_test::HIT_THRESHOLD_CSS {
                return HitResult::hit(HitPart::Edge, d);
            }
            return HitResult::hit(HitPart::Body, d);
        }

        // Check edges from outside (within threshold)
        let d = hit_test::point_to_rect_edge_distance(cx, cy, x0, y0, x1, y1);
        if d <= hit_test::HIT_THRESHOLD_CSS {
            return HitResult::hit(HitPart::Edge, d);
        }

        HitResult::miss()
    }

    fn generate_geometry(
        &self,
        vp: &Viewport,
        pw: f64,
        ph: f64,
        _dpr: f64,
        h_pixel_ratio: f64,
        v_pixel_ratio: f64,
        show_anchors: bool,
    ) -> DrawingGeometry {
        let mut geom = DrawingGeometry::new();
        if self.anchors.len() < 2 {
            return geom;
        }

        let (bx0, by0) = point_to_bitmap(
            &self.anchors[0].point,
            vp,
            pw,
            ph,
            h_pixel_ratio,
            v_pixel_ratio,
        );
        let (bx1, by1) = point_to_bitmap(
            &self.anchors[1].point,
            vp,
            pw,
            ph,
            h_pixel_ratio,
            v_pixel_ratio,
        );

        let px0 = bx0.min(bx1) as f32;
        let py0 = by0.min(by1) as f32;
        let px1 = bx0.max(bx1) as f32;
        let py1 = by0.max(by1) as f32;
        let w = px1 - px0;
        let h = py1 - py0;

        // Fill
        if let Some(fc) = self.style.fill_color {
            geom.rects.push(ColoredRect {
                x: px0,
                y: py0,
                w,
                h,
                r: fc[0],
                g: fc[1],
                b: fc[2],
                a: fc[3],
            });
        }

        // Border (4 edge lines)
        let c = &self.style.color;
        let avg_ratio = (h_pixel_ratio + v_pixel_ratio) * 0.5;
        let lw = (self.style.line_width * avg_ratio).floor().max(1.0) as f32;
        let d = self.style.dash.map_or(0.0, |d| (d[0] * avg_ratio) as f32);
        let g = self.style.dash.map_or(0.0, |d| (d[1] * avg_ratio) as f32);

        // Top edge
        geom.lines.push(ColoredLine {
            x0: px0,
            y0: py0,
            x1: px1,
            y1: py0,
            width: lw,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash: d,
            gap: g,
        });
        // Bottom edge
        geom.lines.push(ColoredLine {
            x0: px0,
            y0: py1,
            x1: px1,
            y1: py1,
            width: lw,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash: d,
            gap: g,
        });
        // Left edge
        geom.lines.push(ColoredLine {
            x0: px0,
            y0: py0,
            x1: px0,
            y1: py1,
            width: lw,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash: d,
            gap: g,
        });
        // Right edge
        geom.lines.push(ColoredLine {
            x0: px1,
            y0: py0,
            x1: px1,
            y1: py1,
            width: lw,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash: d,
            gap: g,
        });

        if show_anchors {
            geom.anchors =
                generate_anchor_circles(&self.anchors, vp, pw, ph, h_pixel_ratio, v_pixel_ratio, c);
        }

        geom
    }
}
