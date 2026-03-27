//! Fibonacci Retracement drawing — 2-anchor with horizontal level lines.
//!
//! Lines are confined to the horizontal span between the two anchor points
//! (not extended to full pane width).

use super::drawing::{
    generate_anchor_circles, next_drawing_id, point_to_bitmap, point_to_css, Drawing,
};
use super::hit_test;
use super::types::*;
use crate::core::renderer::draw_list::{ColoredLine, DrawText, TextAlign};
use crate::core::viewport::Viewport;
use crate::impl_drawing_accessors;

/// Standard Fibonacci retracement levels (matches TradingView defaults).
const FIB_LEVELS: &[(f64, &str)] = &[
    (0.0, "0"),
    (0.236, "0.236"),
    (0.382, "0.382"),
    (0.5, "0.5"),
    (0.618, "0.618"),
    (0.786, "0.786"),
    (1.0, "1"),
];

#[derive(Debug)]
pub struct FibonacciDrawing {
    id: u64,
    state: DrawingState,
    style: DrawingStyle,
    anchors: Vec<AnchorPoint>,
    /// Horizontal alignment for level labels (left / center / right).
    label_align: TextAlign,
}

impl FibonacciDrawing {
    pub fn new(bar_index: f64, price: f64) -> Self {
        let id = next_drawing_id();
        Self {
            id,
            state: DrawingState::Creating { step: 1 },
            style: DrawingStyle::fibonacci_from_theme(
                &crate::core::renderer::theme::ThemeConfig::default(),
            ),
            anchors: vec![
                AnchorPoint::new(bar_index, price),
                AnchorPoint::new(bar_index, price),
            ],
            label_align: TextAlign::Right,
        }
    }

    /// Get the current label alignment.
    pub fn label_align(&self) -> TextAlign {
        self.label_align
    }

    /// Set the label alignment (left / center / right).
    pub fn set_label_align(&mut self, align: TextAlign) {
        self.label_align = align;
    }

    /// Compute the price at a given fib level between the two anchor prices.
    fn level_price(&self, level: f64) -> f64 {
        let p0 = self.anchors[0].point.price;
        let p1 = self.anchors[1].point.price;
        p1 + (p0 - p1) * level
    }
}

impl Drawing for FibonacciDrawing {
    impl_drawing_accessors!(DrawingTool::Fibonacci);
    fn required_anchors(&self) -> usize {
        2
    }

    fn hit_test(&self, cx: f64, cy: f64, vp: &Viewport, pw: f64, ph: f64) -> HitResult {
        if self.anchors.len() < 2 {
            return HitResult::miss();
        }

        // Anchor hit-test first (highest priority)
        for (i, a) in self.anchors.iter().enumerate() {
            let (ax, ay) = point_to_css(&a.point, vp, pw, ph);
            let d = hit_test::point_to_circle_distance(cx, cy, ax, ay);
            if d <= hit_test::ANCHOR_HIT_THRESHOLD_CSS {
                return HitResult::hit(HitPart::Anchor(i), d);
            }
        }

        // Horizontal span between the two anchors (CSS px)
        let (x0, _) = point_to_css(&self.anchors[0].point, vp, pw, ph);
        let (x1, _) = point_to_css(&self.anchors[1].point, vp, pw, ph);
        let left = x0.min(x1);
        let right = x0.max(x1);

        // Only test fib level lines within the anchor span
        if cx >= left && cx <= right {
            for &(level, _) in FIB_LEVELS {
                let price = self.level_price(level);
                let y = vp.price_to_css_y(price, ph);
                let d = (cy - y).abs();
                if d <= hit_test::HIT_THRESHOLD_CSS {
                    return HitResult::hit(HitPart::Body, d);
                }
            }

            // Interior: within vertical span of fib levels
            let (_, y0) = point_to_css(&self.anchors[0].point, vp, pw, ph);
            let (_, y1) = point_to_css(&self.anchors[1].point, vp, pw, ph);
            let min_y = y0.min(y1);
            let max_y = y0.max(y1);
            if cy >= min_y && cy <= max_y {
                return HitResult::hit(HitPart::Body, (cy - min_y).min(max_y - cy));
            }
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
        let fs = (self.style.font_size * avg_ratio) as f32;
        let snap_to_pixel = !matches!(
            self.state,
            DrawingState::Dragging { .. } | DrawingState::Creating { .. }
        );

        // Compute bitmap X positions of the two anchors — lines are confined
        // to this horizontal span (NOT extended to full pane width).
        let (bx0, _) = point_to_bitmap(
            &self.anchors[0].point,
            vp,
            pw,
            ph,
            h_pixel_ratio,
            v_pixel_ratio,
            snap_to_pixel,
        );
        let (bx1, _) = point_to_bitmap(
            &self.anchors[1].point,
            vp,
            pw,
            ph,
            h_pixel_ratio,
            v_pixel_ratio,
            snap_to_pixel,
        );
        let left_x = (bx0.min(bx1)) as f32;
        let right_x = (bx0.max(bx1)) as f32;
        let h_pad = (5.0 * h_pixel_ratio) as f32;

        for &(level, label_text) in FIB_LEVELS {
            let price = self.level_price(level);
            let y = {
                let value = vp.price_to_css_y(price, ph) * v_pixel_ratio;
                if snap_to_pixel {
                    value.round()
                } else {
                    value
                }
            } as f32;

            // Level line — confined between anchor X positions
            geom.lines.push(ColoredLine {
                x0: left_x,
                y0: y,
                x1: right_x,
                y1: y,
                width: lw,
                r: c[0],
                g: c[1],
                b: c[2],
                a: c[3],
                dash: 0.0,
                gap: 0.0,
            });

            // Label x position and alignment driven by self.label_align
            let label_x = match self.label_align {
                TextAlign::Left => left_x + h_pad,
                TextAlign::Right => right_x - h_pad,
                TextAlign::Center => (left_x + right_x) * 0.5,
            };

            // Place label above the line: offset by half the font height plus
            // a small gap so text never collides with the level line.
            let gap_px = (2.0 * avg_ratio) as f32;
            let label_y = y - fs * 0.5 - gap_px;

            let price_label = format!("{} ({:.2})", label_text, price);
            geom.texts.push(DrawText {
                text: price_label,
                x: label_x,
                y: label_y,
                font_size: fs,
                r: c[0],
                g: c[1],
                b: c[2],
                a: c[3],
                align: self.label_align,
            });
        }

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
