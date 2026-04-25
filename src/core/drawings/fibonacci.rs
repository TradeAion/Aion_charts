//! Fibonacci Retracement drawing — 2-anchor with horizontal level lines.
//!
//! Lines are confined to the horizontal span between the two anchor points
//! (not extended to full pane width).

use super::drawing::{
    generate_anchor_circles, line_label_placement, line_middle_gap_range, next_drawing_id,
    point_to_bitmap, point_to_css, prepare_text_block, push_line_with_gap_range,
    push_rotated_text_block, Drawing, TEXT_LABEL_CLEARANCE_CSS,
};
use super::hit_test;
use super::types::*;
use crate::core::renderer::draw_list::{TextAlign, TextVerticalAlign};
use crate::core::viewport::Viewport;
use crate::impl_drawing_accessors;

/// Standard Fibonacci retracement levels (matches TradingView defaults).
fn default_fibonacci_levels() -> Vec<FibonacciLevel> {
    vec![
        FibonacciLevel::new(0.0, "0"),
        FibonacciLevel::new(0.236, "0.236"),
        FibonacciLevel::new(0.382, "0.382"),
        FibonacciLevel::new(0.5, "0.5"),
        FibonacciLevel::new(0.618, "0.618"),
        FibonacciLevel::new(0.786, "0.786"),
        FibonacciLevel::new(1.0, "1"),
    ]
}

#[derive(Debug)]
pub struct FibonacciDrawing {
    id: u64,
    state: DrawingState,
    locked: bool,
    style: DrawingStyle,
    anchors: Vec<AnchorPoint>,
    levels: Vec<FibonacciLevel>,
    /// Horizontal alignment for level labels (left / center / right).
    label_align: TextAlign,
    /// Vertical alignment for level labels (top / middle / bottom).
    label_vertical_align: TextVerticalAlign,
    /// Shared font/color styling for all level labels.
    label_style: DrawingTextStyle,
}

impl FibonacciDrawing {
    pub fn new(bar_index: f64, price: f64) -> Self {
        let id = next_drawing_id();
        Self {
            id,
            state: DrawingState::Creating { step: 1 },
            locked: false,
            style: DrawingStyle::fibonacci_from_theme(
                &crate::core::renderer::theme::ThemeConfig::default(),
            ),
            anchors: vec![
                AnchorPoint::new(bar_index, price),
                AnchorPoint::new(bar_index, price),
            ],
            levels: default_fibonacci_levels(),
            label_align: TextAlign::Right,
            label_vertical_align: TextVerticalAlign::Top,
            label_style: DrawingTextStyle::default(),
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

    pub fn label_vertical_align(&self) -> TextVerticalAlign {
        self.label_vertical_align
    }

    pub fn set_label_vertical_align(&mut self, align: TextVerticalAlign) {
        self.label_vertical_align = align;
    }

    pub fn label_style(&self) -> &DrawingTextStyle {
        &self.label_style
    }

    pub fn label_style_mut(&mut self) -> &mut DrawingTextStyle {
        &mut self.label_style
    }

    pub fn levels(&self) -> &[FibonacciLevel] {
        &self.levels
    }

    pub fn set_levels(&mut self, levels: Vec<FibonacciLevel>) {
        self.levels = if levels.is_empty() {
            default_fibonacci_levels()
        } else {
            levels
        };
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
            for level in &self.levels {
                let price = self.level_price(level.ratio);
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
        let label_color = self.label_style.resolved_color(*c);
        let fs = (self.label_style.resolved_font_size(self.style.font_size) * avg_ratio) as f32;
        // Keep live preview crisp while creating/dragging too.
        let snap_to_pixel = true;

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
        let h_inset = (5.0 * h_pixel_ratio) as f32;

        for level in &self.levels {
            let price = self.level_price(level.ratio);
            let y = {
                let value = vp.price_to_css_y(price, ph) * v_pixel_ratio;
                if snap_to_pixel {
                    value.round()
                } else {
                    value
                }
            } as f32;

            let price_label = format!("{} ({:.2})", level.label, price);
            let mut line_gap_range = None;
            if let Some(block) = prepare_text_block(&price_label, fs) {
                let gap_px = (1.0 * avg_ratio) as f32;
                let placement = line_label_placement(
                    left_x as f64,
                    y as f64,
                    right_x as f64,
                    y as f64,
                    self.label_align,
                    self.label_vertical_align,
                    &block,
                    fs,
                    h_inset as f64,
                    gap_px as f64,
                );
                if self.label_vertical_align == TextVerticalAlign::Middle {
                    line_gap_range = line_middle_gap_range(
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
                    self.label_style.italic,
                    label_color,
                    placement.align,
                    placement.rotation_rad,
                );
            }
            push_line_with_gap_range(
                &mut geom.lines,
                left_x as f64,
                y as f64,
                right_x as f64,
                y as f64,
                lw,
                *c,
                0.0,
                0.0,
                line_gap_range,
            );
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
