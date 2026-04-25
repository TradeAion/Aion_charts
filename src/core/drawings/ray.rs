//! Ray drawing — 2-anchor line that starts at anchor[0], passes through anchor[1],
//! and extends infinitely in that direction to the pane boundary.

use super::drawing::{
    generate_anchor_circles, line_label_placement, next_drawing_id, point_to_bitmap, point_to_css,
    prepare_text_block, push_line_with_gap_range, push_rotated_text_block, Drawing,
    LineLabelPlacement, PreparedTextBlock, TEXT_DRAWING_GAP_CSS, TEXT_LABEL_CLEARANCE_CSS,
};
use super::hit_test;
use super::types::*;
use crate::core::renderer::draw_list::TextAlign;
use crate::core::viewport::Viewport;
use crate::impl_drawing_accessors;

#[derive(Debug)]
pub struct RayDrawing {
    id: u64,
    state: DrawingState,
    locked: bool,
    style: DrawingStyle,
    anchors: Vec<AnchorPoint>,
    text: DrawingText,
}

impl RayDrawing {
    pub fn new(bar_index: f64, price: f64) -> Self {
        let id = next_drawing_id();
        Self {
            id,
            state: DrawingState::Creating { step: 1 },
            locked: false,
            style: DrawingStyle::default(),
            anchors: vec![
                AnchorPoint::new(bar_index, price),
                AnchorPoint::new(bar_index, price),
            ],
            text: DrawingText::default(),
        }
    }

    /// Compute the far endpoint where the ray exits the pane rectangle.
    /// Returns the bitmap-space endpoint extending from p0 through p1.
    fn ray_far_point(x0: f64, y0: f64, x1: f64, y1: f64, pane_pw: f64, pane_ph: f64) -> (f64, f64) {
        let dx = x1 - x0;
        let dy = y1 - y0;
        if dx.abs() < 1e-12 && dy.abs() < 1e-12 {
            return (x1, y1);
        }
        // Find the largest t such that (x0 + t*dx, y0 + t*dy) is still in bounds.
        let mut t_max = f64::MAX;
        if dx.abs() > 1e-12 {
            let t = if dx > 0.0 {
                (pane_pw - x0) / dx
            } else {
                -x0 / dx
            };
            if t > 0.0 {
                t_max = t_max.min(t);
            }
        }
        if dy.abs() > 1e-12 {
            let t = if dy > 0.0 {
                (pane_ph - y0) / dy
            } else {
                -y0 / dy
            };
            if t > 0.0 {
                t_max = t_max.min(t);
            }
        }
        // Ensure we at least reach p1
        if t_max < 1.0 {
            t_max = 1.0;
        }
        (x0 + t_max * dx, y0 + t_max * dy)
    }

    fn middle_text_gap_range_on_ray(
        ray_start_x: f64,
        ray_start_y: f64,
        ray_end_x: f64,
        ray_end_y: f64,
        placement: &LineLabelPlacement,
        block: &PreparedTextBlock,
        padding: f32,
    ) -> Option<(f64, f64)> {
        let ray_dx = ray_end_x - ray_start_x;
        let ray_dy = ray_end_y - ray_start_y;
        let ray_len = (ray_dx * ray_dx + ray_dy * ray_dy).sqrt();
        if ray_len <= f64::EPSILON {
            return None;
        }

        let ux = ray_dx / ray_len;
        let uy = ray_dy / ray_len;
        let anchor_t = (placement.anchor_x as f64 - ray_start_x) * ux
            + (placement.anchor_y as f64 - ray_start_y) * uy;

        let width = block.max_width as f64;
        let local_left_x = match placement.align {
            TextAlign::Left => 0.0,
            TextAlign::Center => -width * 0.5,
            TextAlign::Right => -width,
        };
        let local_right_x = local_left_x + width;

        let (sin_theta, cos_theta) = placement.rotation_rad.sin_cos();
        let orientation = if ux * cos_theta as f64 + uy * sin_theta as f64 >= 0.0 {
            1.0
        } else {
            -1.0
        };
        let projected_a = anchor_t + orientation * local_left_x;
        let projected_b = anchor_t + orientation * local_right_x;
        let padding = padding as f64;
        Some((
            projected_a.min(projected_b) - padding,
            projected_a.max(projected_b) + padding,
        ))
    }

    fn text_label_placement(
        &self,
        anchor_start_x: f64,
        anchor_start_y: f64,
        anchor_end_x: f64,
        anchor_end_y: f64,
        block: &PreparedTextBlock,
        font_size: f32,
        avg_ratio: f64,
    ) -> LineLabelPlacement {
        let inset = TEXT_DRAWING_GAP_CSS * avg_ratio;
        let gap = TEXT_LABEL_CLEARANCE_CSS * avg_ratio;
        line_label_placement(
            anchor_start_x,
            anchor_start_y,
            anchor_end_x,
            anchor_end_y,
            self.text.horizontal_align,
            self.text.vertical_align,
            block,
            font_size,
            inset,
            gap,
        )
    }

    pub fn text(&self) -> &DrawingText {
        &self.text
    }

    pub fn text_mut(&mut self) -> &mut DrawingText {
        &mut self.text
    }
}

impl Drawing for RayDrawing {
    impl_drawing_accessors!(DrawingTool::Ray);
    fn required_anchors(&self) -> usize {
        2
    }

    fn hit_test(&self, cx: f64, cy: f64, vp: &Viewport, pw: f64, ph: f64) -> HitResult {
        if self.anchors.len() < 2 {
            return HitResult::miss();
        }
        // Anchor hit-test first
        for (i, a) in self.anchors.iter().enumerate() {
            let (ax, ay) = point_to_css(&a.point, vp, pw, ph);
            let d = hit_test::point_to_circle_distance(cx, cy, ax, ay);
            if d <= hit_test::ANCHOR_HIT_THRESHOLD_CSS {
                return HitResult::hit(HitPart::Anchor(i), d);
            }
        }
        // Ray body — from anchor[0] through anchor[1], extending infinitely
        let (x0, y0) = point_to_css(&self.anchors[0].point, vp, pw, ph);
        let (x1, y1) = point_to_css(&self.anchors[1].point, vp, pw, ph);
        let d = hit_test::point_to_ray_distance(cx, cy, x0, y0, x1, y1);
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

        let c = &self.style.color;
        let avg_ratio = (h_pixel_ratio + v_pixel_ratio) * 0.5;
        let lw = (self.style.line_width * avg_ratio).floor().max(1.0) as f32;
        let text_color = self.text.style.resolved_color(*c);
        let fs = (self.text.style.resolved_font_size(self.style.font_size) * avg_ratio) as f32;
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

        let pane_pw = pw * h_pixel_ratio;
        let pane_ph = ph * v_pixel_ratio;
        let (far_x, far_y) = Self::ray_far_point(bx0, by0, bx1, by1, pane_pw, pane_ph);
        let mut line_gap_range = None;

        if let Some(block) = prepare_text_block(&self.text.value, fs) {
            // Ray labels are anchored against the finite anchor span, not the
            // infinite body. That keeps "left/center/right" placements within
            // the user-defined anchor region instead of drifting to the pane edge.
            let placement = self.text_label_placement(bx0, by0, bx1, by1, &block, fs, avg_ratio);
            if self.text.vertical_align
                == crate::core::renderer::draw_list::TextVerticalAlign::Middle
            {
                line_gap_range = Self::middle_text_gap_range_on_ray(
                    bx0,
                    by0,
                    far_x,
                    far_y,
                    &placement,
                    &block,
                    (TEXT_LABEL_CLEARANCE_CSS * avg_ratio) as f32,
                );
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
            far_x,
            far_y,
            lw,
            *c,
            0.0,
            0.0,
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
    fn left_aligned_ray_text_anchor_stays_between_anchors_for_leftward_rays() {
        let vp = test_viewport();
        let mut drawing = RayDrawing::new(18.0, 100.0);
        drawing.set_state(DrawingState::Idle);
        drawing.move_anchor(1, 14.0, 96.0);
        drawing.text_mut().value = "Ray".to_string();
        drawing.text_mut().horizontal_align = TextAlign::Left;
        drawing.text_mut().vertical_align = TextVerticalAlign::Top;

        let font_size = drawing
            .text()
            .style
            .resolved_font_size(drawing.style().font_size) as f32;
        let block = prepare_text_block(&drawing.text().value, font_size).expect("text block");
        let (ax0, ay0) = point_to_bitmap(
            &drawing.anchors[0].point,
            &vp,
            1000.0,
            600.0,
            1.0,
            1.0,
            true,
        );
        let (ax1, ay1) = point_to_bitmap(
            &drawing.anchors[1].point,
            &vp,
            1000.0,
            600.0,
            1.0,
            1.0,
            true,
        );
        let placement = drawing.text_label_placement(ax0, ay0, ax1, ay1, &block, font_size, 1.0);
        let anchor_x = placement.anchor_x as f64;

        assert!(
            anchor_x >= ax0.min(ax1),
            "anchor_x={anchor_x}, anchors=({ax0}, {ax1})"
        );
        assert!(
            anchor_x <= ax0.max(ax1),
            "anchor_x={anchor_x}, anchors=({ax0}, {ax1})"
        );
    }
}
