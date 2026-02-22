//! VerticalLine drawing — single-anchor line spanning full pane height.
//!
//! Positioned at a bar index, extends from top to bottom of the chart.

use crate::core::drawings::types::{
    AnchorCircle, AnchorPoint, DrawingGeometry, DrawingState, DrawingStyle, HitPart, HitResult,
};
use crate::core::renderer::draw_list::ColoredLine;
use crate::core::viewport::Viewport;

/// A vertical line drawing at a fixed bar index.
pub struct VerticalLine {
    /// Single anchor at the bar index.
    pub anchor: AnchorPoint,
    pub style: DrawingStyle,
    pub state: DrawingState,
}

impl VerticalLine {
    pub fn new(bar_index: f64, style: DrawingStyle) -> Self {
        Self {
            anchor: AnchorPoint::new(bar_index, 0.0), // price doesn't matter
            style,
            state: DrawingState::Creating { step: 0 },
        }
    }

    /// Number of anchor points needed (1 for vertical line).
    pub fn anchor_count() -> usize {
        1
    }

    /// Set anchor at creation step.
    pub fn set_anchor(&mut self, step: usize, bar_index: f64, price: f64) {
        if step == 0 {
            self.anchor.point.bar_index = bar_index;
            self.anchor.point.price = price;
        }
    }

    /// Move the entire drawing by delta.
    pub fn translate(&mut self, delta_bar: f64, _delta_price: f64) {
        // Only move horizontally (bar_index changes, price doesn't matter)
        self.anchor.point.bar_index += delta_bar;
    }

    /// Move a specific anchor.
    pub fn move_anchor(&mut self, _idx: usize, bar: f64, _price: f64) {
        // Only one anchor, just update bar_index
        self.anchor.point.bar_index = bar;
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

        let bar_idx = self.anchor.point.bar_index;

        // Convert bar index to X coordinate
        let visible_bars = (vp.end_bar - vp.start_bar).max(1.0);
        let bar_offset = bar_idx - vp.start_bar;
        let x_css = (bar_offset + 0.5) / visible_bars * pane_css_w;
        let x_phys = x_css * dpr;

        // Check if line is visible
        let pane_pw = pane_css_w * dpr;
        if x_phys < 0.0 || x_phys > pane_pw {
            return geom;
        }

        let pane_ph = pane_css_h * dpr;
        let line_w = (self.style.line_width * dpr).max(1.0);

        // Draw vertical line spanning full height
        let (dash, gap) = self
            .style
            .dash
            .map_or((0.0, 0.0), |d| (d[0] as f32, d[1] as f32));
        geom.lines.push(ColoredLine {
            x0: x_phys as f32,
            y0: 0.0,
            x1: x_phys as f32,
            y1: pane_ph as f32,
            width: line_w as f32,
            r: self.style.color[0],
            g: self.style.color[1],
            b: self.style.color[2],
            a: self.style.color[3],
            dash,
            gap,
        });

        // Draw anchor circle if selected
        if matches!(
            self.state,
            DrawingState::Selected | DrawingState::Dragging { .. }
        ) {
            let anchor_r = 5.0 * dpr;
            // Place anchor at center of visible area vertically
            let center_y = pane_ph / 2.0;

            geom.anchors.push(AnchorCircle {
                cx: x_phys,
                cy: center_y,
                radius: anchor_r,
                fill: [1.0, 1.0, 1.0, 0.9],
                border: self.style.color,
                border_width: 2.0 * dpr,
            });
        }

        geom
    }

    /// Hit-test the line.
    pub fn hit_test(
        &self,
        x_css: f64,
        y_css: f64,
        vp: &Viewport,
        pane_css_w: f64,
        pane_css_h: f64,
    ) -> HitResult {
        let bar_idx = self.anchor.point.bar_index;

        // Convert bar index to X coordinate
        let visible_bars = (vp.end_bar - vp.start_bar).max(1.0);
        let bar_offset = bar_idx - vp.start_bar;
        let line_x_css = (bar_offset + 0.5) / visible_bars * pane_css_w;

        // Check if within pane bounds
        if line_x_css < 0.0 || line_x_css > pane_css_w {
            return HitResult::miss();
        }

        // Distance from cursor to line
        let dist = (x_css - line_x_css).abs();

        // Anchor hit (center of pane vertically)
        let anchor_y = pane_css_h / 2.0;
        let anchor_dist = ((x_css - line_x_css).powi(2) + (y_css - anchor_y).powi(2)).sqrt();
        if anchor_dist <= self.anchor.hit_radius {
            return HitResult::hit(HitPart::Anchor(0), anchor_dist);
        }

        // Line body hit (7px threshold)
        if dist <= 7.0 {
            return HitResult::hit(HitPart::Body, dist);
        }

        HitResult::miss()
    }
}
