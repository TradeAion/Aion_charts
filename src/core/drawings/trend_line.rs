//! Trend Line drawing — 2-anchor line segment.

use super::drawing::{
    generate_anchor_circles, line_label_placement, line_middle_gap_range, next_drawing_id,
    point_to_bitmap, point_to_css, prepare_text_block, push_line_with_gap_range,
    push_rotated_text_block, Drawing, TEXT_DRAWING_GAP_CSS,
};
use super::hit_test;
use super::types::*;
use crate::core::viewport::Viewport;
use crate::impl_drawing_accessors;

#[derive(Debug)]
pub struct TrendLineDrawing {
    id: u64,
    state: DrawingState,
    style: DrawingStyle,
    anchors: Vec<AnchorPoint>,
    text: DrawingText,
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
        // Keep live preview crisp while creating/dragging too.
        let snap_to_pixel = true;

        let (bx0, by0) = point_to_bitmap(
            &self.anchors[0].point,
            vp,
            pw,
            ph,
            h_pixel_ratio,
            v_pixel_ratio,
            snap_to_pixel,
        );
        let (bx1, by1) = point_to_bitmap(
            &self.anchors[1].point,
            vp,
            pw,
            ph,
            h_pixel_ratio,
            v_pixel_ratio,
            snap_to_pixel,
        );

        let c = &self.style.color;
        let avg_ratio = (h_pixel_ratio + v_pixel_ratio) * 0.5;
        let line_w = (self.style.line_width * avg_ratio).floor().max(1.0);
        let text_color = self.text.style.resolved_color(*c);
        let fs = (self.text.style.resolved_font_size(self.style.font_size) * avg_ratio) as f32;
        let dash = self.style.dash.map_or(0.0, |d| (d[0] * avg_ratio) as f32);
        let gap = self.style.dash.map_or(0.0, |d| (d[1] * avg_ratio) as f32);
        let mut line_gap_range = None;

        if let Some(block) = prepare_text_block(&self.text.value, fs) {
            // Universal 2px shape↔text spacing for both end inset and
            // perpendicular gap from the trend line.
            let inset = TEXT_DRAWING_GAP_CSS * avg_ratio;
            let gap = TEXT_DRAWING_GAP_CSS * avg_ratio;
            let placement = line_label_placement(
                bx0,
                by0,
                bx1,
                by1,
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
            bx0,
            by0,
            bx1,
            by1,
            line_w as f32,
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
}
