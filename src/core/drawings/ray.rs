//! Ray drawing — two-anchor line extending to visible area edges.
//!
//! Like a trend line but extends infinitely in both directions.

use crate::core::drawings::types::{
    AnchorCircle, AnchorPoint, DrawingGeometry, DrawingState, DrawingStyle, HitPart, HitResult,
};
use crate::core::renderer::draw_list::ColoredLine;
use crate::core::viewport::Viewport;

/// A ray (extended line) drawing with two anchor points.
pub struct Ray {
    /// Two anchor points defining the line.
    pub anchors: [AnchorPoint; 2],
    pub style: DrawingStyle,
    pub state: DrawingState,
}

impl Ray {
    pub fn new(bar0: f64, price0: f64, bar1: f64, price1: f64, style: DrawingStyle) -> Self {
        Self {
            anchors: [
                AnchorPoint::new(bar0, price0),
                AnchorPoint::new(bar1, price1),
            ],
            style,
            state: DrawingState::Creating { step: 0 },
        }
    }

    /// Number of anchor points needed (2 for ray).
    pub fn anchor_count() -> usize {
        2
    }

    /// Set anchor at creation step.
    pub fn set_anchor(&mut self, step: usize, bar_index: f64, price: f64) {
        if step < 2 {
            self.anchors[step].point.bar_index = bar_index;
            self.anchors[step].point.price = price;
        }
    }

    /// Move the entire drawing by delta.
    pub fn translate(&mut self, delta_bar: f64, delta_price: f64) {
        for anchor in &mut self.anchors {
            anchor.point.bar_index += delta_bar;
            anchor.point.price += delta_price;
        }
    }

    /// Move a specific anchor.
    pub fn move_anchor(&mut self, idx: usize, bar: f64, price: f64) {
        if idx < 2 {
            self.anchors[idx].point.bar_index = bar;
            self.anchors[idx].point.price = price;
        }
    }

    /// Generate pixel-space geometry for rendering.
    pub fn generate_geometry(
        &self,
        vp: &Viewport,
        pane_css_w: f64,
        pane_css_h: f64,
        dpr: f64,
        _h_ratio: f64,
        _v_ratio: f64,
    ) -> DrawingGeometry {
        let mut geom = DrawingGeometry::new();

        // Convert anchor points to pixel coordinates
        let visible_bars = (vp.end_bar - vp.start_bar).max(1.0);

        let x0_css =
            (self.anchors[0].point.bar_index - vp.start_bar + 0.5) / visible_bars * pane_css_w;
        let y0_css = vp.price_to_css_y(self.anchors[0].point.price, pane_css_h);

        let x1_css =
            (self.anchors[1].point.bar_index - vp.start_bar + 0.5) / visible_bars * pane_css_w;
        let y1_css = vp.price_to_css_y(self.anchors[1].point.price, pane_css_h);

        // Extend line to pane edges
        let (ext_x0, ext_y0, ext_x1, ext_y1) = extend_line_to_rect(
            x0_css, y0_css, x1_css, y1_css, 0.0, 0.0, pane_css_w, pane_css_h,
        );

        let ext_x0_phys = ext_x0 * dpr;
        let ext_y0_phys = ext_y0 * dpr;
        let ext_x1_phys = ext_x1 * dpr;
        let ext_y1_phys = ext_y1 * dpr;

        let line_w = (self.style.line_width * dpr).max(1.0);

        // Draw extended line
        let (dash, gap) = self
            .style
            .dash
            .map_or((0.0, 0.0), |d| (d[0] as f32, d[1] as f32));
        geom.lines.push(ColoredLine {
            x0: ext_x0_phys as f32,
            y0: ext_y0_phys as f32,
            x1: ext_x1_phys as f32,
            y1: ext_y1_phys as f32,
            width: line_w as f32,
            r: self.style.color[0],
            g: self.style.color[1],
            b: self.style.color[2],
            a: self.style.color[3],
            dash,
            gap,
        });

        // Draw anchor circles if selected
        if matches!(
            self.state,
            DrawingState::Selected | DrawingState::Dragging { .. }
        ) {
            let anchor_r = 5.0 * dpr;
            let pane_pw = pane_css_w * dpr;
            let pane_ph = pane_css_h * dpr;

            for (_i, anchor) in self.anchors.iter().enumerate() {
                let ax = (anchor.point.bar_index - vp.start_bar + 0.5) / visible_bars * pane_pw;
                let ay = vp.price_to_css_y(anchor.point.price, pane_css_h) * dpr;

                // Only draw if anchor is in visible area
                if ax >= -anchor_r
                    && ax <= pane_pw + anchor_r
                    && ay >= -anchor_r
                    && ay <= pane_ph + anchor_r
                {
                    geom.anchors.push(AnchorCircle {
                        cx: ax,
                        cy: ay,
                        radius: anchor_r,
                        fill: super::default_anchor_color(),
                        border: self.style.color,
                        border_width: 2.0 * dpr,
                    });
                }
            }
        }

        geom
    }

    /// Hit-test the ray.
    pub fn hit_test(
        &self,
        x_css: f64,
        y_css: f64,
        vp: &Viewport,
        pane_css_w: f64,
        pane_css_h: f64,
    ) -> HitResult {
        let visible_bars = (vp.end_bar - vp.start_bar).max(1.0);

        // Check anchor hits first
        for (i, anchor) in self.anchors.iter().enumerate() {
            let ax = (anchor.point.bar_index - vp.start_bar + 0.5) / visible_bars * pane_css_w;
            let ay = vp.price_to_css_y(anchor.point.price, pane_css_h);
            let dist = ((x_css - ax).powi(2) + (y_css - ay).powi(2)).sqrt();
            if dist <= anchor.hit_radius {
                return HitResult::hit(HitPart::Anchor(i), dist);
            }
        }

        // Check line body hit
        let x0 = (self.anchors[0].point.bar_index - vp.start_bar + 0.5) / visible_bars * pane_css_w;
        let y0 = vp.price_to_css_y(self.anchors[0].point.price, pane_css_h);
        let x1 = (self.anchors[1].point.bar_index - vp.start_bar + 0.5) / visible_bars * pane_css_w;
        let y1 = vp.price_to_css_y(self.anchors[1].point.price, pane_css_h);

        let dist = point_to_line_distance(x_css, y_css, x0, y0, x1, y1);
        if dist <= 7.0 {
            return HitResult::hit(HitPart::Body, dist);
        }

        HitResult::miss()
    }
}

/// Extend a line defined by two points to the edges of a rectangle.
/// Returns (x0, y0, x1, y1) of the extended line segment.
fn extend_line_to_rect(
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    rect_x: f64,
    rect_y: f64,
    rect_w: f64,
    rect_h: f64,
) -> (f64, f64, f64, f64) {
    let dx = x1 - x0;
    let dy = y1 - y0;

    // Handle degenerate cases
    if dx.abs() < 1e-10 && dy.abs() < 1e-10 {
        return (x0, y0, x1, y1);
    }

    // Find intersections with all four edges
    let mut t_values: Vec<f64> = Vec::new();

    // Left edge (x = rect_x)
    if dx.abs() > 1e-10 {
        let t = (rect_x - x0) / dx;
        let y = y0 + t * dy;
        if y >= rect_y && y <= rect_y + rect_h {
            t_values.push(t);
        }
    }

    // Right edge (x = rect_x + rect_w)
    if dx.abs() > 1e-10 {
        let t = (rect_x + rect_w - x0) / dx;
        let y = y0 + t * dy;
        if y >= rect_y && y <= rect_y + rect_h {
            t_values.push(t);
        }
    }

    // Top edge (y = rect_y)
    if dy.abs() > 1e-10 {
        let t = (rect_y - y0) / dy;
        let x = x0 + t * dx;
        if x >= rect_x && x <= rect_x + rect_w {
            t_values.push(t);
        }
    }

    // Bottom edge (y = rect_y + rect_h)
    if dy.abs() > 1e-10 {
        let t = (rect_y + rect_h - y0) / dy;
        let x = x0 + t * dx;
        if x >= rect_x && x <= rect_x + rect_w {
            t_values.push(t);
        }
    }

    if t_values.len() < 2 {
        return (x0, y0, x1, y1);
    }

    // Sort and take min/max
    t_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let t_min = t_values[0];
    let t_max = t_values[t_values.len() - 1];

    let new_x0 = x0 + t_min * dx;
    let new_y0 = y0 + t_min * dy;
    let new_x1 = x0 + t_max * dx;
    let new_y1 = y0 + t_max * dy;

    (new_x0, new_y0, new_x1, new_y1)
}

/// Calculate distance from point (px, py) to infinite line through (x0, y0) and (x1, y1).
fn point_to_line_distance(px: f64, py: f64, x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len_sq = dx * dx + dy * dy;

    if len_sq < 1e-10 {
        // Degenerate line (two points are same)
        return ((px - x0).powi(2) + (py - y0).powi(2)).sqrt();
    }

    // Distance to infinite line
    ((dy * px - dx * py + x1 * y0 - y1 * x0).abs()) / len_sq.sqrt()
}
