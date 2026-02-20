//! Fibonacci Retracement drawing — 2-anchor with horizontal level lines.

use crate::core::viewport::Viewport;
use crate::core::renderer::draw_list::{ColoredLine, ColoredRect, DrawText};
use super::types::*;
use super::drawing::{Drawing, next_drawing_id, point_to_css, generate_anchor_circles};
use super::hit_test;

/// Standard Fibonacci retracement levels.
const FIB_LEVELS: &[(f64, &str)] = &[
    (0.0,   "0%"),
    (0.236, "23.6%"),
    (0.382, "38.2%"),
    (0.5,   "50%"),
    (0.618, "61.8%"),
    (0.786, "78.6%"),
    (1.0,   "100%"),
];

#[derive(Debug)]
pub struct FibonacciDrawing {
    id: u64,
    state: DrawingState,
    style: DrawingStyle,
    anchors: Vec<AnchorPoint>,
}

impl FibonacciDrawing {
    pub fn new(bar_index: f64, price: f64) -> Self {
        let id = next_drawing_id();
        Self {
            id,
            state: DrawingState::Creating { step: 1 },
            style: DrawingStyle {
                color: [0.95, 0.75, 0.25, 1.0], // gold
                line_width: 1.0,
                fill_color: Some([0.95, 0.75, 0.25, 0.05]),
                dash: None,
                font_size: 10.0,
            },
            anchors: vec![
                AnchorPoint::new(bar_index, price),
                AnchorPoint::new(bar_index, price),
            ],
        }
    }

    /// Compute the price at a given fib level between the two anchor prices.
    fn level_price(&self, level: f64) -> f64 {
        let p0 = self.anchors[0].point.price;
        let p1 = self.anchors[1].point.price;
        p1 + (p0 - p1) * level
    }
}

impl Drawing for FibonacciDrawing {
    fn id(&self) -> u64 { self.id }
    fn tool(&self) -> DrawingTool { DrawingTool::Fibonacci }
    fn state(&self) -> DrawingState { self.state }
    fn set_state(&mut self, state: DrawingState) { self.state = state; }
    fn style(&self) -> &DrawingStyle { &self.style }
    fn style_mut(&mut self) -> &mut DrawingStyle { &mut self.style }
    fn anchors(&self) -> &[AnchorPoint] { &self.anchors }
    fn anchors_mut(&mut self) -> &mut Vec<AnchorPoint> { &mut self.anchors }
    fn required_anchors(&self) -> usize { 2 }

    fn hit_test(
        &self,
        cx: f64, cy: f64,
        vp: &Viewport, pw: f64, ph: f64,
    ) -> HitResult {
        if self.anchors.len() < 2 { return HitResult::miss(); }

        // Check anchors first
        for (i, a) in self.anchors.iter().enumerate() {
            let (ax, ay) = point_to_css(&a.point, vp, pw, ph);
            let d = hit_test::point_to_circle_distance(cx, cy, ax, ay);
            if d <= hit_test::ANCHOR_HIT_THRESHOLD_CSS {
                return HitResult::hit(HitPart::Anchor(i), d);
            }
        }

        // Check each horizontal fib level line
        for &(level, _) in FIB_LEVELS {
            let price = self.level_price(level);
            let y = vp.price_to_css_y(price, ph);
            let d = (cy - y).abs();
            if d <= hit_test::HIT_THRESHOLD_CSS {
                return HitResult::hit(HitPart::Body, d);
            }
        }

        // Check if cursor is within the vertical span of fib levels
        let (_, y0) = point_to_css(&self.anchors[0].point, vp, pw, ph);
        let (_, y1) = point_to_css(&self.anchors[1].point, vp, pw, ph);
        let min_y = y0.min(y1);
        let max_y = y0.max(y1);
        if cy >= min_y && cy <= max_y {
            return HitResult::hit(HitPart::Body, (cy - min_y).min(max_y - cy));
        }

        HitResult::miss()
    }

    fn generate_geometry(
        &self,
        vp: &Viewport, pw: f64, ph: f64, dpr: f64,
        show_anchors: bool,
    ) -> DrawingGeometry {
        let mut geom = DrawingGeometry::new();
        if self.anchors.len() < 2 { return geom; }

        let c = &self.style.color;
        let lw = (self.style.line_width * dpr) as f32;
        let pane_phys_w = (pw * dpr) as f32;
        let fs = (self.style.font_size * dpr) as f32;

        // Draw each fib level line across full pane width
        for (i, &(level, label_text)) in FIB_LEVELS.iter().enumerate() {
            let price = self.level_price(level);
            let y = (vp.price_to_css_y(price, ph) * dpr) as f32;

            // Level line
            geom.lines.push(ColoredLine {
                x0: 0.0, y0: y,
                x1: pane_phys_w, y1: y,
                width: lw,
                r: c[0], g: c[1], b: c[2], a: c[3],
                dash: (6.0 * dpr) as f32,
                gap: (4.0 * dpr) as f32,
            });

            // Fill zone between this level and the next
            if let Some(&(next_level, _)) = FIB_LEVELS.get(i + 1) {
                if let Some(fc) = self.style.fill_color {
                    let next_price = self.level_price(next_level);
                    let next_y = (vp.price_to_css_y(next_price, ph) * dpr) as f32;
                    let ry = y.min(next_y);
                    let rh = (y - next_y).abs();
                    geom.rects.push(ColoredRect {
                        x: 0.0, y: ry, w: pane_phys_w, h: rh,
                        r: fc[0], g: fc[1], b: fc[2],
                        a: fc[3] * if i % 2 == 0 { 1.0 } else { 0.5 },
                    });
                }
            }

            // Label (right-aligned)
            let price_label = format!("{} ({:.2})", label_text, price);
            geom.texts.push(DrawText {
                text: price_label,
                x: pane_phys_w - (5.0 * dpr) as f32,
                y: y - fs * 0.3,
                font_size: fs,
                r: c[0], g: c[1], b: c[2], a: c[3],
            });
        }

        if show_anchors {
            geom.anchors = generate_anchor_circles(&self.anchors, vp, pw, ph, dpr, c);
        }

        geom
    }
}
