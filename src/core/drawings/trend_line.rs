//! Trend Line drawing — 2-anchor line segment.

use crate::core::viewport::Viewport;
use crate::core::renderer::draw_list::ColoredLine;
use super::types::*;
use super::drawing::{Drawing, next_drawing_id, point_to_css, generate_anchor_circles};
use super::hit_test;

#[derive(Debug)]
pub struct TrendLineDrawing {
    id: u64,
    state: DrawingState,
    style: DrawingStyle,
    anchors: Vec<AnchorPoint>,
}

impl TrendLineDrawing {
    pub fn new(bar_index: f64, price: f64) -> Self {
        let id = next_drawing_id();
        Self {
            id,
            state: DrawingState::Creating { step: 1 },
            style: DrawingStyle {
                color: [0.35, 0.55, 0.95, 1.0],
                line_width: 1.0,
                fill_color: None,
                dash: None,
                font_size: 11.0,
            },
            anchors: vec![
                AnchorPoint::new(bar_index, price),
                AnchorPoint::new(bar_index, price), // preview anchor
            ],
        }
    }
}

impl Drawing for TrendLineDrawing {
    fn id(&self) -> u64 { self.id }
    fn tool(&self) -> DrawingTool { DrawingTool::TrendLine }
    fn state(&self) -> DrawingState { self.state }
    fn set_state(&mut self, state: DrawingState) { self.state = state; }
    fn style(&self) -> &DrawingStyle { &self.style }
    fn style_mut(&mut self) -> &mut DrawingStyle { &mut self.style }
    fn anchors(&self) -> &[AnchorPoint] { &self.anchors }
    fn anchors_mut(&mut self) -> &mut Vec<AnchorPoint> { &mut self.anchors }
    fn required_anchors(&self) -> usize { 2 }

    fn hit_test(
        &self,
        cx: f64, cy: f64,
        vp: &Viewport, pw: f64, ph: f64,
    ) -> HitResult {
        if self.anchors.len() < 2 { return HitResult::miss(); }

        let (x0, y0) = point_to_css(&self.anchors[0].point, vp, pw, ph);
        let (x1, y1) = point_to_css(&self.anchors[1].point, vp, pw, ph);

        // Check anchors first (higher priority)
        for (i, a) in self.anchors.iter().enumerate() {
            let (ax, ay) = point_to_css(&a.point, vp, pw, ph);
            let d = hit_test::point_to_circle_distance(cx, cy, ax, ay);
            if d <= hit_test::ANCHOR_HIT_THRESHOLD_CSS {
                return HitResult::hit(HitPart::Anchor(i), d);
            }
        }

        // Check line body
        let d = hit_test::point_to_segment_distance(cx, cy, x0, y0, x1, y1);
        if d <= hit_test::HIT_THRESHOLD_CSS {
            return HitResult::hit(HitPart::Body, d);
        }

        HitResult::miss()
    }

    fn generate_geometry(
        &self,
        vp: &Viewport, pw: f64, ph: f64, dpr: f64,
        show_anchors: bool,
    ) -> DrawingGeometry {
        let mut geom = DrawingGeometry::new();
        if self.anchors.len() < 2 { return geom; }

        let (x0, y0) = point_to_css(&self.anchors[0].point, vp, pw, ph);
        let (x1, y1) = point_to_css(&self.anchors[1].point, vp, pw, ph);

        let c = &self.style.color;
        geom.lines.push(ColoredLine {
            x0: (x0 * dpr) as f32,
            y0: (y0 * dpr) as f32,
            x1: (x1 * dpr) as f32,
            y1: (y1 * dpr) as f32,
            width: (self.style.line_width * dpr) as f32,
            r: c[0], g: c[1], b: c[2], a: c[3],
            dash: self.style.dash.map_or(0.0, |d| (d[0] * dpr) as f32),
            gap: self.style.dash.map_or(0.0, |d| (d[1] * dpr) as f32),
        });

        if show_anchors {
            geom.anchors = generate_anchor_circles(&self.anchors, vp, pw, ph, dpr, c);
        }

        geom
    }
}
