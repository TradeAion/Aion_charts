//! Trend Line drawing — 2-anchor line segment.

use super::drawing::{
    generate_anchor_circles, next_drawing_id, point_to_bitmap, point_to_css, Drawing,
};
use super::hit_test;
use super::types::*;
use crate::core::renderer::draw_list::ColoredLine;
use crate::core::viewport::Viewport;
use crate::impl_drawing_accessors;

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
            style: DrawingStyle::default(),
            anchors: vec![
                AnchorPoint::new(bar_index, price),
                AnchorPoint::new(bar_index, price), // preview anchor
            ],
        }
    }
}

impl Drawing for TrendLineDrawing {
    impl_drawing_accessors!(DrawingTool::TrendLine);
    fn required_anchors(&self) -> usize {
        2
    }

    fn hit_test(&self, cx: f64, cy: f64, vp: &Viewport, pw: f64, ph: f64) -> HitResult {
        if self.anchors.len() < 2 {
            return HitResult::miss();
        }

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

        let c = &self.style.color;
        let avg_ratio = (h_pixel_ratio + v_pixel_ratio) * 0.5;
        let line_w = (self.style.line_width * avg_ratio).floor().max(1.0);

        geom.lines.push(ColoredLine {
            x0: bx0 as f32,
            y0: by0 as f32,
            x1: bx1 as f32,
            y1: by1 as f32,
            width: line_w as f32,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash: self.style.dash.map_or(0.0, |d| (d[0] * avg_ratio) as f32),
            gap: self.style.dash.map_or(0.0, |d| (d[1] * avg_ratio) as f32),
        });

        if show_anchors {
            geom.anchors =
                generate_anchor_circles(&self.anchors, vp, pw, ph, h_pixel_ratio, v_pixel_ratio, c);
        }

        geom
    }
}
