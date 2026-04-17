//! VerticalLine drawing — single-anchor line spanning full pane height.
//!
//! Positioned at a bar index, extends from top to bottom of the chart.
//! Completes on the first click (1 anchor).

use super::drawing::{
    generate_anchor_circles, next_drawing_id, point_to_bitmap, point_to_css, Drawing,
};
use super::hit_test;
use super::types::*;
use crate::core::renderer::draw_list::ColoredLine;
use crate::core::viewport::Viewport;
use crate::impl_drawing_accessors;

#[derive(Debug)]
pub struct VerticalLineDrawing {
    id: u64,
    state: DrawingState,
    style: DrawingStyle,
    anchors: Vec<AnchorPoint>,
}

impl VerticalLineDrawing {
    pub fn new(bar_index: f64, price: f64) -> Self {
        let id = next_drawing_id();
        Self {
            id,
            // Single-anchor tool: start at step 0 so finalize_creation_step()
            // completes without creating a phantom second anchor.
            state: DrawingState::Creating { step: 0 },
            style: DrawingStyle::default(),
            anchors: vec![AnchorPoint::new(bar_index, price)],
        }
    }
}

impl Drawing for VerticalLineDrawing {
    impl_drawing_accessors!(DrawingTool::VerticalLine);
    fn required_anchors(&self) -> usize {
        1
    }

    fn hit_test(&self, cx: f64, cy: f64, vp: &Viewport, pw: f64, ph: f64) -> HitResult {
        if self.anchors.is_empty() {
            return HitResult::miss();
        }
        // Anchor hit
        let (ax, ay) = point_to_css(&self.anchors[0].point, vp, pw, ph);
        let ad = hit_test::point_to_circle_distance(cx, cy, ax, ay);
        if ad <= hit_test::ANCHOR_HIT_THRESHOLD_CSS {
            return HitResult::hit(HitPart::Anchor(0), ad);
        }
        // Line body — full-height vertical at anchor bar_index
        let line_x = ax;
        let d = (cx - line_x).abs();
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
        if self.anchors.is_empty() {
            return geom;
        }

        let c = &self.style.color;
        let avg_ratio = (h_pixel_ratio + v_pixel_ratio) * 0.5;
        let lw = (self.style.line_width * avg_ratio).floor().max(1.0) as f32;
        let snap_to_pixel = !matches!(
            self.state,
            DrawingState::Dragging { .. } | DrawingState::Creating { .. }
        );
        let (x, _y) = point_to_bitmap(
            &self.anchors[0].point,
            vp,
            pw,
            ph,
            h_pixel_ratio,
            v_pixel_ratio,
            snap_to_pixel,
        );
        let x = x as f32;
        let pane_ph = (ph * v_pixel_ratio).round() as f32;

        let (dash, gap) = self.style.dash.map_or((0.0, 0.0), |d| {
            ((d[0] * avg_ratio) as f32, (d[1] * avg_ratio) as f32)
        });

        geom.lines.push(ColoredLine {
            x0: x,
            y0: 0.0,
            x1: x,
            y1: pane_ph,
            width: lw,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash,
            gap,
        });

        if show_anchors {
            geom.anchors = generate_anchor_circles(
                &self.anchors,
                vp,
                pw,
                ph,
                h_pixel_ratio,
                v_pixel_ratio,
                c,
                snap_to_pixel,
            );
        }
        geom
    }

    /// Only horizontal movement matters for a vertical line.
    fn move_by(&mut self, delta_bar: f64, _delta_price: f64) {
        for a in self.anchors.iter_mut() {
            a.point.bar_index += delta_bar;
        }
    }

    fn move_anchor(&mut self, index: usize, bar_index: f64, _price: f64) {
        if let Some(a) = self.anchors.get_mut(index) {
            a.point.bar_index = bar_index;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_viewport() -> Viewport {
        let mut vp = Viewport::new(1000, 600);
        vp.start_bar = 10.0;
        vp.end_bar = 20.0;
        vp.price_min = 90.0;
        vp.price_max = 110.0;
        vp
    }

    #[test]
    fn vertical_line_hit_test_uses_shared_x_alignment() {
        let vp = test_viewport();
        let drawing = VerticalLineDrawing::new(14.5, 100.0);

        let (expected_x, _expected_y) = point_to_css(&drawing.anchors[0].point, &vp, 1000.0, 600.0);
        let hit = drawing.hit_test(expected_x, 200.0, &vp, 1000.0, 600.0);

        assert!(hit.is_hit());
    }

    #[test]
    fn vertical_line_geometry_uses_shared_bitmap_alignment() {
        let vp = test_viewport();
        let mut drawing = VerticalLineDrawing::new(14.5, 100.0);
        drawing.set_state(DrawingState::Idle);

        let geom = drawing.generate_geometry(&vp, 1000.0, 600.0, 1.0, 1.0, 1.0, false);
        let (expected_x, _expected_y) =
            point_to_bitmap(&drawing.anchors[0].point, &vp, 1000.0, 600.0, 1.0, 1.0, true);

        assert_eq!(geom.lines.len(), 1);
        assert!((geom.lines[0].x0 as f64 - expected_x).abs() < 1e-9);
        assert!((geom.lines[0].x1 as f64 - expected_x).abs() < 1e-9);
    }
}
