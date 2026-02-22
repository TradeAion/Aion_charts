//! HorizontalLine drawing — single-anchor line spanning full pane width.
//!
//! Like price lines but as a drawing tool that can be selected, dragged, deleted.

use crate::core::drawings::types::{
    AnchorCircle, AnchorPoint, DrawingGeometry, DrawingState, DrawingStyle, HitPart, HitResult,
};
use crate::core::renderer::draw_list::ColoredLine;
use crate::core::viewport::Viewport;

/// A horizontal line drawing at a fixed price level.
pub struct HorizontalLine {
    /// Single anchor at the price level.
    pub anchor: AnchorPoint,
    pub style: DrawingStyle,
    pub state: DrawingState,
}

impl HorizontalLine {
    pub fn new(price: f64, style: DrawingStyle) -> Self {
        Self {
            anchor: AnchorPoint::new(0.0, price), // bar_index doesn't matter
            style,
            state: DrawingState::Creating { step: 0 },
        }
    }

    /// Number of anchor points needed (1 for horizontal line).
    pub fn anchor_count() -> usize {
        1
    }

    /// Set anchor at creation step.
    pub fn set_anchor(&mut self, step: usize, bar_index: f64, price: f64) {
        if step == 0 {
            self.anchor.point.price = price;
            self.anchor.point.bar_index = bar_index;
        }
    }

    /// Move the entire drawing by delta.
    pub fn translate(&mut self, _delta_bar: f64, delta_price: f64) {
        // Only move vertically (price changes, bar_index doesn't matter)
        self.anchor.point.price += delta_price;
    }

    /// Move a specific anchor.
    pub fn move_anchor(&mut self, _idx: usize, _bar: f64, price: f64) {
        // Only one anchor, just update price
        self.anchor.point.price = price;
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

        let price = self.anchor.point.price;
        let y_css = vp.price_to_css_y(price, pane_css_h);
        let y_phys = y_css * dpr;

        // Check if line is visible
        let pane_ph = pane_css_h * dpr;
        if y_phys < 0.0 || y_phys > pane_ph {
            return geom;
        }

        let pane_pw = pane_css_w * dpr;
        let line_w = (self.style.line_width * dpr).max(1.0);

        // Draw horizontal line spanning full width
        let (dash, gap) = self
            .style
            .dash
            .map_or((0.0, 0.0), |d| (d[0] as f32, d[1] as f32));
        geom.lines.push(ColoredLine {
            x0: 0.0,
            y0: y_phys as f32,
            x1: pane_pw as f32,
            y1: y_phys as f32,
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
            // Place anchor at center of visible area
            let center_x = pane_pw / 2.0;

            geom.anchors.push(AnchorCircle {
                cx: center_x,
                cy: y_phys,
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
        let price = self.anchor.point.price;
        let line_y_css = vp.price_to_css_y(price, pane_css_h);

        // Check if within pane bounds
        if line_y_css < 0.0 || line_y_css > pane_css_h {
            return HitResult::miss();
        }

        // Distance from cursor to line
        let dist = (y_css - line_y_css).abs();

        // Anchor hit (center of pane)
        let anchor_x = pane_css_w / 2.0;
        let anchor_dist = ((x_css - anchor_x).powi(2) + (y_css - line_y_css).powi(2)).sqrt();
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
