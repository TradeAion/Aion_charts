//! VerticalLine drawing — single-anchor line spanning full pane height.
//!
//! Positioned at a bar index, extends from top to bottom of the chart.
//! Completes on the first click (1 anchor).

use super::drawing::{
    generate_anchor_circles, next_drawing_id, optical_middle_top, point_to_bitmap, point_to_css,
    prepare_text_block, push_line_with_gap, push_text_block, text_block_bounds, Drawing,
    TEXT_DRAWING_GAP_CSS,
};
use super::hit_test;
use super::types::*;
use crate::core::renderer::draw_list::{TextAlign, TextVerticalAlign};
use crate::core::viewport::Viewport;
use crate::impl_drawing_accessors;

#[derive(Debug)]
pub struct VerticalLineDrawing {
    id: u64,
    state: DrawingState,
    locked: bool,
    style: DrawingStyle,
    anchors: Vec<AnchorPoint>,
    text: DrawingText,
}

impl VerticalLineDrawing {
    pub fn new(bar_index: f64, price: f64) -> Self {
        let id = next_drawing_id();
        Self {
            id,
            // Single-anchor tool: start at step 0 so finalize_creation_step()
            // completes without creating a phantom second anchor.
            state: DrawingState::Creating { step: 0 },
            locked: false,
            style: DrawingStyle::default(),
            anchors: vec![AnchorPoint::new(bar_index, price)],
            text: DrawingText::default(),
        }
    }

    pub fn text(&self) -> &DrawingText {
        &self.text
    }

    pub fn text_mut(&mut self) -> &mut DrawingText {
        &mut self.text
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
        let text_color = self.text.style.resolved_color(*c);
        let fs = (self.text.style.resolved_font_size(self.style.font_size) * avg_ratio) as f32;
        // Keep live preview crisp while creating/dragging too.
        let snap_to_pixel = true;
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

        let mut line_gap_bounds = None;

        if let Some(block) = prepare_text_block(&self.text.value, fs) {
            // Universal 2px shape↔text spacing for both the horizontal offset
            // from the vertical line and the vertical padding from the pane edges.
            let gap = TEXT_DRAWING_GAP_CSS * avg_ratio;
            let pad_y = TEXT_DRAWING_GAP_CSS * avg_ratio;
            let (text_x, text_align) = match self.text.horizontal_align {
                TextAlign::Left => (x as f64 - gap, TextAlign::Right),
                TextAlign::Center => (x as f64, TextAlign::Center),
                TextAlign::Right => (x as f64 + gap, TextAlign::Left),
            };
            let top_y = match self.text.vertical_align {
                TextVerticalAlign::Top => pad_y as f32,
                TextVerticalAlign::Middle => optical_middle_top(pane_ph * 0.5, &block, fs),
                TextVerticalAlign::Bottom => pane_ph - pad_y as f32 - block.total_height,
            };
            if self.text.horizontal_align == TextAlign::Center {
                line_gap_bounds = Some(text_block_bounds(&block, text_x as f32, top_y, text_align));
            }
            push_text_block(
                &mut geom.texts,
                &block,
                text_x as f32,
                top_y,
                fs,
                600,
                self.text.style.italic,
                text_color,
                text_align,
            );
        }

        push_line_with_gap(
            &mut geom.lines,
            x as f64,
            0.0,
            x as f64,
            pane_ph as f64,
            lw,
            *c,
            dash,
            gap,
            line_gap_bounds,
            (2.0 * avg_ratio) as f32,
        );

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
        let (expected_x, _expected_y) = point_to_bitmap(
            &drawing.anchors[0].point,
            &vp,
            1000.0,
            600.0,
            1.0,
            1.0,
            true,
        );

        assert_eq!(geom.lines.len(), 1);
        assert!((geom.lines[0].x0 as f64 - expected_x).abs() < 1e-9);
        assert!((geom.lines[0].x1 as f64 - expected_x).abs() < 1e-9);
    }
}
