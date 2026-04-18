//! HorizontalLine drawing — single-anchor line spanning full pane width.
//!
//! Like price lines but as a drawing tool that can be selected, dragged, deleted.
//! Completes on the first click (1 anchor). The line extends across the full
//! pane width at the anchor's price level.

use super::drawing::{
    generate_anchor_circles, line_label_placement, line_middle_gap_range, next_drawing_id,
    point_to_css, prepare_text_block, push_line_with_gap_range, push_rotated_text_block, Drawing,
    TEXT_DRAWING_GAP_CSS,
};
use super::hit_test;
use super::types::*;
use crate::core::viewport::Viewport;
use crate::impl_drawing_accessors;

#[derive(Debug)]
pub struct HorizontalLineDrawing {
    id: u64,
    state: DrawingState,
    style: DrawingStyle,
    anchors: Vec<AnchorPoint>,
    text: DrawingText,
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
        let text_color = self.text.style.resolved_color(*c);
        let fs = (self.text.style.resolved_font_size(self.style.font_size) * avg_ratio) as f32;
        // Keep live preview crisp while creating/dragging too.
        let snap_to_pixel = true;
        let y = {
            let value = vp.price_to_css_y(self.anchors[0].point.price, ph) * v_pixel_ratio;
            if snap_to_pixel {
                value.round()
            } else {
                value
            }
        } as f32;
        let pane_pw = (pw * h_pixel_ratio).round() as f32;

        let (dash, gap) = self.style.dash.map_or((0.0, 0.0), |d| {
            ((d[0] * avg_ratio) as f32, (d[1] * avg_ratio) as f32)
        });
        let mut line_gap_range = None;

        if let Some(block) = prepare_text_block(&self.text.value, fs) {
            // Inset (horizontal padding from pane edge) and gap (perpendicular
            // distance from the line to the text baseline) both use the
            // universal 2px shape↔text spacing.
            let inset = TEXT_DRAWING_GAP_CSS * avg_ratio;
            let gap = TEXT_DRAWING_GAP_CSS * avg_ratio;
            let placement = line_label_placement(
                0.0,
                y as f64,
                pane_pw as f64,
                y as f64,
                self.text.horizontal_align,
                self.text.vertical_align,
                &block,
                fs,
                inset,
                gap,
            );
            if self.text.vertical_align
                == crate::core::renderer::draw_list::TextVerticalAlign::Middle
            {
                line_gap_range = line_middle_gap_range(&placement, &block, avg_ratio as f32);
            }
            push_rotated_text_block(
                &mut geom.texts,
                &block,
                placement.anchor_x,
                placement.anchor_y,
                placement.top_local_y,
                fs,
                600,
                self.text.style.italic,
                text_color,
                placement.align,
                placement.rotation_rad,
            );
        }

        push_line_with_gap_range(
            &mut geom.lines,
            0.0,
            y as f64,
            pane_pw as f64,
            y as f64,
            lw,
            *c,
            dash,
            gap,
            line_gap_range,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::renderer::draw_list::{TextAlign, TextVerticalAlign};

    fn test_viewport() -> Viewport {
        let mut vp = Viewport::new(1000, 600);
        vp.start_bar = 10.0;
        vp.end_bar = 20.0;
        vp.price_min = 90.0;
        vp.price_max = 110.0;
        vp
    }

    #[test]
    fn centered_middle_text_splits_horizontal_line_geometry() {
        let vp = test_viewport();
        let mut drawing = HorizontalLineDrawing::new(14.5, 100.0);
        drawing.set_state(DrawingState::Idle);
        drawing.text_mut().value = "Dev".to_string();
        drawing.text_mut().horizontal_align = TextAlign::Center;
        drawing.text_mut().vertical_align = TextVerticalAlign::Middle;

        let geom = drawing.generate_geometry(&vp, 1000.0, 600.0, 1.0, 1.0, 1.0, false);

        assert_eq!(geom.lines.len(), 2);
    }
}
