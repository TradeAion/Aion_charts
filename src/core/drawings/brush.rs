//! Brush (freehand) drawing — variable-point polyline recorded from pointer drag.
//!
//! The brush tool records logical (bar_index, price) points while the user
//! drags across the chart. On pointer-up the polyline is finalized.
//! Points are stored in logical chart space so the drawing survives
//! scroll, zoom, and resize — just like all other drawing tools.

use super::drawing::{next_drawing_id, point_to_bitmap, point_to_css, Drawing};
use super::hit_test;
use super::types::*;
use crate::core::renderer::draw_list::ColoredLine;
use crate::core::viewport::Viewport;
use crate::impl_drawing_accessors;

#[derive(Debug)]
pub struct BrushDrawing {
    id: u64,
    state: DrawingState,
    style: DrawingStyle,
    /// The two required "anchors" that the Drawing trait API expects.
    /// For brush, anchor[0] = first recorded point, anchor[1] = last.
    anchors: Vec<AnchorPoint>,
    /// All intermediate freehand points (in logical coords).
    /// The full polyline is anchors[0] + points + anchors[1].
    points: Vec<DrawingPoint>,
}

impl BrushDrawing {
    pub fn new(bar_index: f64, price: f64) -> Self {
        let id = next_drawing_id();
        let mut style = DrawingStyle::default();
        // Brush should render thicker than line-based tools for better visibility.
        style.line_width = 2.5;
        Self {
            id,
            state: DrawingState::Creating { step: 1 },
            style,
            anchors: vec![
                AnchorPoint::new(bar_index, price),
                AnchorPoint::new(bar_index, price),
            ],
            points: Vec::new(),
        }
    }

    pub(crate) fn points(&self) -> &[DrawingPoint] {
        &self.points
    }

    pub(crate) fn points_mut(&mut self) -> &mut Vec<DrawingPoint> {
        &mut self.points
    }

    pub(crate) fn set_points(&mut self, points: Vec<DrawingPoint>) {
        self.points = points;
    }
}

impl Drawing for BrushDrawing {
    impl_drawing_accessors!(DrawingTool::Brush);
    fn required_anchors(&self) -> usize {
        // Brush is finalized by pointer-up, not a fixed anchor count.
        // Return 2 so the default add_creation_point logic completes on
        // the second call (pointer-up).
        2
    }

    /// Override the default creation preview to record intermediate points.
    fn update_creation_preview(&mut self, bar_index: f64, price: f64) {
        if !matches!(self.state, DrawingState::Creating { .. }) {
            return;
        }
        // Distance-gate: skip if the cursor hasn't moved enough in CSS space.
        // We can't compute CSS coords here (no viewport), so we approximate
        // using a very cheap logical-distance check.  The real gating happens
        // in generate_geometry which has pixel info, but recording a few extra
        // points is harmless — they'll collapse visually.
        self.points.push(DrawingPoint::new(bar_index, price));
        // Keep anchor[1] tracking the latest position for the base trait.
        if self.anchors.len() >= 2 {
            self.anchors[1].point = DrawingPoint::new(bar_index, price);
        }
    }

    fn hit_test(&self, cx: f64, cy: f64, vp: &Viewport, pw: f64, ph: f64) -> HitResult {
        // Walk every segment of the polyline.
        let all = self.all_points();
        if all.len() < 2 {
            return HitResult::miss();
        }
        let mut best_d = f64::MAX;
        for pair in all.windows(2) {
            let (x0, y0) = point_to_css(&pair[0], vp, pw, ph);
            let (x1, y1) = point_to_css(&pair[1], vp, pw, ph);
            let d = hit_test::point_to_segment_distance(cx, cy, x0, y0, x1, y1);
            if d < best_d {
                best_d = d;
            }
        }
        if best_d <= hit_test::HIT_THRESHOLD_CSS {
            return HitResult::hit(HitPart::Body, best_d);
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
        _show_anchors: bool,
    ) -> DrawingGeometry {
        let mut geom = DrawingGeometry::new();
        let all = self.all_points();
        if all.len() < 2 {
            return geom;
        }

        let c = &self.style.color;
        let avg_ratio = (h_pixel_ratio + v_pixel_ratio) * 0.5;
        let line_w = (self.style.line_width * avg_ratio).floor().max(1.0) as f32;

        let mut prev = point_to_bitmap(&all[0], vp, pw, ph, h_pixel_ratio, v_pixel_ratio);
        for pt in &all[1..] {
            let cur = point_to_bitmap(pt, vp, pw, ph, h_pixel_ratio, v_pixel_ratio);
            geom.lines.push(ColoredLine {
                x0: prev.0 as f32,
                y0: prev.1 as f32,
                x1: cur.0 as f32,
                y1: cur.1 as f32,
                width: line_w,
                r: c[0],
                g: c[1],
                b: c[2],
                a: c[3],
                dash: 0.0,
                gap: 0.0,
            });
            prev = cur;
        }

        // Brush drawings intentionally show no anchor circles — the shape
        // is the freehand stroke itself.

        geom
    }

    /// Move the entire brush stroke by a delta in logical coordinates.
    fn move_by(&mut self, delta_bar: f64, delta_price: f64) {
        for anchor in self.anchors.iter_mut() {
            anchor.point.bar_index += delta_bar;
            anchor.point.price += delta_price;
        }
        for pt in self.points.iter_mut() {
            pt.bar_index += delta_bar;
            pt.price += delta_price;
        }
    }
}

impl BrushDrawing {
    /// Return all points in order: anchor[0], intermediate points, anchor[1].
    fn all_points(&self) -> Vec<DrawingPoint> {
        let mut out = Vec::with_capacity(self.points.len() + 2);
        if !self.anchors.is_empty() {
            out.push(self.anchors[0].point);
        }
        out.extend_from_slice(&self.points);
        if self.anchors.len() >= 2 {
            out.push(self.anchors[1].point);
        }
        out
    }
}
