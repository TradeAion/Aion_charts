//! Ray drawing — 2-anchor line that starts at anchor[0], passes through anchor[1],
//! and extends infinitely in that direction to the pane boundary.

use super::drawing::{
    generate_anchor_circles, next_drawing_id, point_to_bitmap, point_to_css, Drawing,
};
use super::hit_test;
use super::types::*;
use crate::core::renderer::draw_list::ColoredLine;
use crate::core::viewport::Viewport;
use crate::impl_drawing_accessors;

#[derive(Debug)]
pub struct RayDrawing {
    id: u64,
    state: DrawingState,
    style: DrawingStyle,
    anchors: Vec<AnchorPoint>,
}

impl RayDrawing {
    pub fn new(bar_index: f64, price: f64) -> Self {
        let id = next_drawing_id();
        Self {
            id,
            state: DrawingState::Creating { step: 1 },
            style: DrawingStyle::default(),
            anchors: vec![
                AnchorPoint::new(bar_index, price),
                AnchorPoint::new(bar_index, price),
            ],
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

        let pane_pw = pw * h_pixel_ratio;
        let pane_ph = ph * v_pixel_ratio;
        let (far_x, far_y) = Self::ray_far_point(bx0, by0, bx1, by1, pane_pw, pane_ph);

        geom.lines.push(ColoredLine {
            x0: bx0 as f32,
            y0: by0 as f32,
            x1: far_x as f32,
            y1: far_y as f32,
            width: lw,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash: 0.0,
            gap: 0.0,
        });

        if show_anchors {
            geom.anchors =
                generate_anchor_circles(&self.anchors, vp, pw, ph, h_pixel_ratio, v_pixel_ratio, c);
        }
        geom
    }
}
