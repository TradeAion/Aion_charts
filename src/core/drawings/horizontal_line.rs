//! HorizontalLine drawing — single-anchor line spanning full pane width.
//!
//! Like price lines but as a drawing tool that can be selected, dragged, deleted.
//! Completes on the first click (1 anchor). The line extends across the full
//! pane width at the anchor's price level.

use super::drawing::{generate_anchor_circles, next_drawing_id, point_to_css, Drawing};
use super::hit_test;
use super::types::*;
use crate::core::renderer::draw_list::ColoredLine;
use crate::core::viewport::Viewport;
use crate::impl_drawing_accessors;

#[derive(Debug)]
pub struct HorizontalLineDrawing {
    id: u64,
    state: DrawingState,
    style: DrawingStyle,
    anchors: Vec<AnchorPoint>,
}

impl HorizontalLineDrawing {
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

impl Drawing for HorizontalLineDrawing {
    impl_drawing_accessors!(DrawingTool::HorizontalLine);
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
        // Line body — full-width horizontal at anchor price
        let line_y = vp.price_to_css_y(self.anchors[0].point.price, ph);
        let d = (cy - line_y).abs();
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
        let y = (vp.price_to_css_y(self.anchors[0].point.price, ph) * v_pixel_ratio).round() as f32;
        let pane_pw = (pw * h_pixel_ratio).round() as f32;

        let (dash, gap) = self.style.dash.map_or((0.0, 0.0), |d| {
            ((d[0] * avg_ratio) as f32, (d[1] * avg_ratio) as f32)
        });

        geom.lines.push(ColoredLine {
            x0: 0.0,
            y0: y,
            x1: pane_pw,
            y1: y,
            width: lw,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash,
            gap,
        });

        if show_anchors {
            geom.anchors =
                generate_anchor_circles(&self.anchors, vp, pw, ph, h_pixel_ratio, v_pixel_ratio, c);
        }
        geom
    }

    /// Only vertical movement matters for a horizontal line.
    fn move_by(&mut self, _delta_bar: f64, delta_price: f64) {
        for a in self.anchors.iter_mut() {
            a.point.price += delta_price;
        }
    }

    fn move_anchor(&mut self, index: usize, _bar_index: f64, price: f64) {
        if let Some(a) = self.anchors.get_mut(index) {
            a.point.price = price;
        }
    }
}
