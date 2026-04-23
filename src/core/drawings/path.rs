//! Multi-click Path drawing — TradingView-style arrowed path finished explicitly.

use super::drawing::{
    generate_anchor_circles, next_drawing_id, point_to_bitmap, point_to_css, Drawing,
};
use super::hit_test;
use super::types::*;
use crate::core::renderer::draw_list::ColoredLine;
use crate::core::viewport::Viewport;
use crate::impl_drawing_accessors;

const ARROW_HEAD_LENGTH_CSS: f64 = 10.0;
const ARROW_HEAD_WIDTH_CSS: f64 = 5.5;

#[derive(Debug)]
pub struct PathDrawing {
    id: u64,
    state: DrawingState,
    locked: bool,
    style: DrawingStyle,
    /// All committed vertices plus a trailing preview vertex while creating.
    anchors: Vec<AnchorPoint>,
}

impl PathDrawing {
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
        }
    }

    fn same_point(a: DrawingPoint, b: DrawingPoint) -> bool {
        (a.bar_index - b.bar_index).abs() <= 1e-9
            && (a.price - b.price).abs() <= 1e-9
            && a.timestamp == b.timestamp
    }

    fn arrow_head_points(
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        arrow_length: f64,
        arrow_width: f64,
    ) -> Option<[(f64, f64); 2]> {
        let dx = end_x - start_x;
        let dy = end_y - start_y;
        let len = (dx * dx + dy * dy).sqrt();
        if len <= f64::EPSILON {
            return None;
        }

        let ux = dx / len;
        let uy = dy / len;
        let px = -uy;
        let py = ux;
        let base_x = end_x - ux * arrow_length;
        let base_y = end_y - uy * arrow_length;

        Some([
            (base_x + px * arrow_width, base_y + py * arrow_width),
            (base_x - px * arrow_width, base_y - py * arrow_width),
        ])
    }

    fn last_segment_css(
        &self,
        vp: &Viewport,
        pw: f64,
        ph: f64,
    ) -> Option<((f64, f64), (f64, f64))> {
        self.anchors.windows(2).rev().find_map(|segment| {
            let start = point_to_css(&segment[0].point, vp, pw, ph);
            let end = point_to_css(&segment[1].point, vp, pw, ph);
            let dx = end.0 - start.0;
            let dy = end.1 - start.1;
            if (dx * dx + dy * dy).sqrt() <= f64::EPSILON {
                None
            } else {
                Some((start, end))
            }
        })
    }

    fn push_line(
        lines: &mut Vec<ColoredLine>,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        width: f32,
        color: [f32; 4],
        dash: f32,
        gap: f32,
    ) {
        lines.push(ColoredLine {
            x0: x0 as f32,
            y0: y0 as f32,
            x1: x1 as f32,
            y1: y1 as f32,
            width,
            r: color[0],
            g: color[1],
            b: color[2],
            a: color[3],
            dash,
            gap,
        });
    }
}

impl Drawing for PathDrawing {
    impl_drawing_accessors!(DrawingTool::Path);

    fn required_anchors(&self) -> usize {
        2
    }

    fn hit_test(&self, cx: f64, cy: f64, vp: &Viewport, pw: f64, ph: f64) -> HitResult {
        if self.anchors.is_empty() {
            return HitResult::miss();
        }

        for (idx, anchor) in self.anchors.iter().enumerate() {
            let (ax, ay) = point_to_css(&anchor.point, vp, pw, ph);
            let distance = hit_test::point_to_circle_distance(cx, cy, ax, ay);
            if distance <= hit_test::ANCHOR_HIT_THRESHOLD_CSS {
                return HitResult::hit(HitPart::Anchor(idx), distance);
            }
        }

        let mut best_distance = f64::MAX;
        for segment in self.anchors.windows(2) {
            let (x0, y0) = point_to_css(&segment[0].point, vp, pw, ph);
            let (x1, y1) = point_to_css(&segment[1].point, vp, pw, ph);
            let distance = hit_test::point_to_segment_distance(cx, cy, x0, y0, x1, y1);
            best_distance = best_distance.min(distance);
        }

        if let Some(((x0, y0), (x1, y1))) = self.last_segment_css(vp, pw, ph) {
            if let Some([left, right]) =
                Self::arrow_head_points(x0, y0, x1, y1, ARROW_HEAD_LENGTH_CSS, ARROW_HEAD_WIDTH_CSS)
            {
                best_distance = best_distance.min(hit_test::point_to_segment_distance(
                    cx, cy, left.0, left.1, x1, y1,
                ));
                best_distance = best_distance.min(hit_test::point_to_segment_distance(
                    cx, cy, right.0, right.1, x1, y1,
                ));
            }
        }

        if best_distance <= hit_test::HIT_THRESHOLD_CSS {
            HitResult::hit(HitPart::Body, best_distance)
        } else {
            HitResult::miss()
        }
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

        let color = self.style.color;
        let avg_ratio = (h_pixel_ratio + v_pixel_ratio) * 0.5;
        let line_width = (self.style.line_width * avg_ratio).floor().max(1.0) as f32;
        let dash = self.style.dash.map_or(0.0, |d| (d[0] * avg_ratio) as f32);
        let gap = self.style.dash.map_or(0.0, |d| (d[1] * avg_ratio) as f32);
        let snap_to_pixel = true;
        let mut last_segment = None;

        for segment in self.anchors.windows(2) {
            let (x0, y0) = point_to_bitmap(
                &segment[0].point,
                vp,
                pw,
                ph,
                h_pixel_ratio,
                v_pixel_ratio,
                snap_to_pixel,
            );
            let (x1, y1) = point_to_bitmap(
                &segment[1].point,
                vp,
                pw,
                ph,
                h_pixel_ratio,
                v_pixel_ratio,
                snap_to_pixel,
            );
            if ((x1 - x0) * (x1 - x0) + (y1 - y0) * (y1 - y0)).sqrt() > f64::EPSILON {
                last_segment = Some(((x0, y0), (x1, y1)));
            }
            Self::push_line(
                &mut geom.lines,
                x0,
                y0,
                x1,
                y1,
                line_width,
                color,
                dash,
                gap,
            );
        }

        if let Some(((x0, y0), (x1, y1))) = last_segment {
            if let Some([left, right]) = Self::arrow_head_points(
                x0,
                y0,
                x1,
                y1,
                ARROW_HEAD_LENGTH_CSS * avg_ratio,
                ARROW_HEAD_WIDTH_CSS * avg_ratio,
            ) {
                Self::push_line(
                    &mut geom.lines,
                    left.0,
                    left.1,
                    x1,
                    y1,
                    line_width,
                    color,
                    0.0,
                    0.0,
                );
                Self::push_line(
                    &mut geom.lines,
                    right.0,
                    right.1,
                    x1,
                    y1,
                    line_width,
                    color,
                    0.0,
                    0.0,
                );
            }
        }

        if show_anchors {
            geom.anchors = generate_anchor_circles(
                &self.anchors,
                vp,
                pw,
                ph,
                h_pixel_ratio,
                v_pixel_ratio,
                &color,
                snap_to_pixel,
            );
        }

        geom
    }

    fn add_creation_point(&mut self, bar_index: f64, price: f64) -> bool {
        if !matches!(self.state, DrawingState::Creating { .. }) {
            return true;
        }

        if self.anchors.is_empty() {
            self.anchors.push(AnchorPoint::new(bar_index, price));
            self.anchors.push(AnchorPoint::new(bar_index, price));
        } else {
            let last_idx = self.anchors.len() - 1;
            self.anchors[last_idx].point = DrawingPoint::new(bar_index, price);
            self.anchors.push(AnchorPoint::new(bar_index, price));
        }

        self.set_state(DrawingState::Creating { step: 1 });
        false
    }

    fn completes_on_pointer_up(&self) -> bool {
        false
    }

    fn complete_creation(&mut self) -> bool {
        if !matches!(self.state, DrawingState::Creating { .. }) {
            return false;
        }

        if self.anchors.len() >= 2 {
            let last = self.anchors[self.anchors.len() - 1].point;
            let prev = self.anchors[self.anchors.len() - 2].point;
            if Self::same_point(last, prev) {
                self.anchors.pop();
            }
        }

        if self.anchors.len() < 2 {
            return false;
        }

        self.set_state(DrawingState::Idle);
        true
    }

    fn update_creation_preview(&mut self, bar_index: f64, price: f64) {
        if !matches!(self.state, DrawingState::Creating { .. }) {
            return;
        }

        if self.anchors.is_empty() {
            self.anchors.push(AnchorPoint::new(bar_index, price));
        } else if let Some(last) = self.anchors.last_mut() {
            last.point = DrawingPoint::new(bar_index, price);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_viewport() -> Viewport {
        let mut vp = Viewport::new(1000, 600);
        vp.start_bar = 0.0;
        vp.end_bar = 100.0;
        vp.price_min = 0.0;
        vp.price_max = 100.0;
        vp
    }

    #[test]
    fn path_geometry_adds_arrow_cap_on_last_segment() {
        let mut drawing = PathDrawing::new(10.0, 50.0);
        assert!(!drawing.add_creation_point(20.0, 60.0));
        drawing.update_creation_preview(30.0, 55.0);
        assert!(drawing.complete_creation());

        let geom = drawing.generate_geometry(&test_viewport(), 1000.0, 600.0, 1.0, 1.0, 1.0, false);

        assert_eq!(
            geom.lines.len(),
            4,
            "two shaft segments plus two arrow-cap wings"
        );
        let tip = point_to_bitmap(
            &drawing.anchors()[2].point,
            &test_viewport(),
            1000.0,
            600.0,
            1.0,
            1.0,
            true,
        );
        assert_eq!(geom.lines[2].x1, tip.0 as f32);
        assert_eq!(geom.lines[2].y1, tip.1 as f32);
        assert_eq!(geom.lines[3].x1, tip.0 as f32);
        assert_eq!(geom.lines[3].y1, tip.1 as f32);
    }

    #[test]
    fn arrow_head_is_hit_testable() {
        let mut drawing = PathDrawing::new(10.0, 50.0);
        drawing.update_creation_preview(30.0, 50.0);
        assert!(drawing.complete_creation());

        let vp = test_viewport();
        let ((x0, y0), (x1, y1)) = drawing
            .last_segment_css(&vp, 1000.0, 600.0)
            .expect("segment");
        let [left, _right] = PathDrawing::arrow_head_points(
            x0,
            y0,
            x1,
            y1,
            ARROW_HEAD_LENGTH_CSS,
            ARROW_HEAD_WIDTH_CSS,
        )
        .expect("arrow head");
        let probe_x = left.0 * 0.8 + x1 * 0.2;
        let probe_y = left.1 * 0.8 + y1 * 0.2;

        let hit = drawing.hit_test(probe_x, probe_y, &vp, 1000.0, 600.0);

        assert_eq!(hit.part, HitPart::Body);
    }
}
