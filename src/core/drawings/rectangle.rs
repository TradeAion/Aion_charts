//! Rectangle drawing — 2-anchor filled rectangle with border.

use super::drawing::{next_drawing_id, point_to_bitmap, point_to_css, Drawing};
use super::hit_test;
use super::types::*;
use crate::core::renderer::draw_list::{ColoredLine, ColoredRect};
use crate::core::viewport::Viewport;
use crate::impl_drawing_accessors;

#[derive(Debug)]
pub struct RectangleDrawing {
    id: u64,
    state: DrawingState,
    style: DrawingStyle,
    anchors: Vec<AnchorPoint>,
}

impl RectangleDrawing {
    pub fn new(bar_index: f64, price: f64) -> Self {
        let id = next_drawing_id();
        Self {
            id,
            state: DrawingState::Creating { step: 1 },
            style: DrawingStyle::rectangle_from_theme(
                &crate::core::renderer::theme::ThemeConfig::default(),
            ),
            anchors: vec![
                AnchorPoint::new(bar_index, price),
                AnchorPoint::new(bar_index, price),
            ],
        }
    }

    #[inline]
    fn normalized_bounds(&self) -> (f64, f64, f64, f64) {
        let a = &self.anchors[0].point;
        let b = &self.anchors[1].point;
        let left = a.bar_index.min(b.bar_index);
        let right = a.bar_index.max(b.bar_index);
        let top = a.price.max(b.price);
        let bottom = a.price.min(b.price);
        (left, right, top, bottom)
    }

    #[inline]
    fn set_from_bounds(&mut self, left: f64, right: f64, top: f64, bottom: f64) {
        self.anchors[0].point = DrawingPoint::new(left, top);
        self.anchors[1].point = DrawingPoint::new(right, bottom);
    }

    /// Return the opposite corner (bar, price) for a rectangle corner index:
    /// 0=TL -> BR, 1=TR -> BL, 2=BR -> TL, 3=BL -> TR.
    pub(crate) fn opposite_corner(&self, index: usize) -> Option<(f64, f64)> {
        let (left, right, top, bottom) = self.normalized_bounds();
        match index {
            0 => Some((right, bottom)),
            1 => Some((left, bottom)),
            2 => Some((left, top)),
            3 => Some((right, top)),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn is_corner_anchor(index: usize) -> bool {
        index < 4
    }

    #[inline]
    pub(crate) fn is_edge_anchor(index: usize) -> bool {
        (4..=7).contains(&index)
    }

    /// Returns a stable opposite reference point to pin during drag:
    /// - corners: opposite corner
    /// - edge midpoints: opposite side coordinate (x or y)
    pub(crate) fn opposite_reference_for_anchor(&self, index: usize) -> Option<(f64, f64)> {
        let (left, right, top, bottom) = self.normalized_bounds();
        match index {
            0 => Some((right, bottom)),    // TL -> BR
            1 => Some((left, bottom)),     // TR -> BL
            2 => Some((left, top)),        // BR -> TL
            3 => Some((right, top)),       // BL -> TR
            4 => Some((f64::NAN, bottom)), // TM -> fix bottom
            5 => Some((left, f64::NAN)),   // RM -> fix left
            6 => Some((f64::NAN, top)),    // BM -> fix top
            7 => Some((right, f64::NAN)),  // LM -> fix right
            _ => None,
        }
    }

    /// Move a rectangle corner while keeping the opposite corner fixed.
    ///
    /// This enables natural TradingView-like flipping when the dragged corner
    /// crosses over the opposite side.
    pub(crate) fn move_corner_with_fixed_opposite(
        &mut self,
        index: usize,
        bar_index: f64,
        price: f64,
        opposite_bar: f64,
        opposite_price: f64,
    ) {
        if Self::is_corner_anchor(index) {
            // Keep exact dragged + fixed points as the two diagonal anchors.
            // Rendering/hit-test still normalize bounds, but this prevents the
            // opposite corner from drifting/shaking when crossing only one axis.
            self.anchors[0].point = DrawingPoint::new(bar_index, price);
            self.anchors[1].point = DrawingPoint::new(opposite_bar, opposite_price);
            return;
        }

        let (mut left, mut right, mut top, mut bottom) = self.normalized_bounds();
        match index {
            4 => {
                // Top edge midpoint, keep bottom fixed.
                top = price;
                bottom = opposite_price;
            }
            5 => {
                // Right edge midpoint, keep left fixed.
                right = bar_index;
                left = opposite_bar;
            }
            6 => {
                // Bottom edge midpoint, keep top fixed.
                bottom = price;
                top = opposite_price;
            }
            7 => {
                // Left edge midpoint, keep right fixed.
                left = bar_index;
                right = opposite_bar;
            }
            _ => return,
        }
        let norm_left = left.min(right);
        let norm_right = left.max(right);
        let norm_top = top.max(bottom);
        let norm_bottom = top.min(bottom);
        self.set_from_bounds(norm_left, norm_right, norm_top, norm_bottom);
    }

    #[inline]
    fn nearest_edge_anchor(
        cx: f64,
        cy: f64,
        left: f64,
        right: f64,
        top: f64,
        bottom: f64,
    ) -> usize {
        let d_top = (cy - top).abs();
        let d_right = (right - cx).abs();
        let d_bottom = (cy - bottom).abs();
        let d_left = (cx - left).abs();

        let mut best_d = d_top;
        let mut best_idx = 4; // top-mid
        if d_right < best_d {
            best_d = d_right;
            best_idx = 5; // right-mid
        }
        if d_bottom < best_d {
            best_d = d_bottom;
            best_idx = 6; // bottom-mid
        }
        if d_left < best_d {
            best_idx = 7; // left-mid
        }
        best_idx
    }
}

impl Drawing for RectangleDrawing {
    impl_drawing_accessors!(DrawingTool::Rectangle);
    fn required_anchors(&self) -> usize {
        2
    }

    fn hit_test(&self, cx: f64, cy: f64, vp: &Viewport, pw: f64, ph: f64) -> HitResult {
        if self.anchors.len() < 2 {
            return HitResult::miss();
        }

        let (x0, y0) = point_to_css(&self.anchors[0].point, vp, pw, ph);
        let (x1, y1) = point_to_css(&self.anchors[1].point, vp, pw, ph);
        let left = x0.min(x1);
        let right = x0.max(x1);
        let top = y0.min(y1);
        let bottom = y0.max(y1);

        // 8 handles: TL, TR, BR, BL, TM, RM, BM, LM.
        let mid_x = (left + right) * 0.5;
        let mid_y = (top + bottom) * 0.5;
        let handles = [
            (left, top),
            (right, top),
            (right, bottom),
            (left, bottom),
            (mid_x, top),
            (right, mid_y),
            (mid_x, bottom),
            (left, mid_y),
        ];
        for (i, (ax, ay)) in handles.into_iter().enumerate() {
            let d = hit_test::point_to_circle_distance(cx, cy, ax, ay);
            if d <= hit_test::ANCHOR_HIT_THRESHOLD_CSS {
                return HitResult::hit(HitPart::Anchor(i), d);
            }
        }

        // Check if inside the rectangle fill
        if hit_test::point_in_rect(cx, cy, x0, y0, x1, y1) {
            let d = hit_test::point_to_rect_edge_distance(cx, cy, x0, y0, x1, y1);
            // Near the edge → side-resize anchor, deep interior → Body (pan pass-through)
            if d <= hit_test::HIT_THRESHOLD_CSS {
                let edge_anchor = Self::nearest_edge_anchor(cx, cy, left, right, top, bottom);
                return HitResult::hit(HitPart::Anchor(edge_anchor), d);
            }
            return HitResult::hit(HitPart::Body, d);
        }

        // Check edges from outside (within threshold) and map to side-resize anchors.
        let d = hit_test::point_to_rect_edge_distance(cx, cy, x0, y0, x1, y1);
        if d <= hit_test::HIT_THRESHOLD_CSS {
            let edge_anchor = Self::nearest_edge_anchor(cx, cy, left, right, top, bottom);
            return HitResult::hit(HitPart::Anchor(edge_anchor), d);
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

        let px0 = bx0.min(bx1) as f32;
        let py0 = by0.min(by1) as f32;
        let px1 = bx0.max(bx1) as f32;
        let py1 = by0.max(by1) as f32;
        let w = px1 - px0;
        let h = py1 - py0;

        // Fill
        if let Some(fc) = self.style.fill_color {
            geom.rects.push(ColoredRect {
                x: px0,
                y: py0,
                w,
                h,
                r: fc[0],
                g: fc[1],
                b: fc[2],
                a: fc[3],
            });
        }

        // Border (4 edge lines)
        let c = &self.style.color;
        let avg_ratio = (h_pixel_ratio + v_pixel_ratio) * 0.5;
        let lw = (self.style.line_width * avg_ratio).floor().max(1.0) as f32;
        let d = self.style.dash.map_or(0.0, |d| (d[0] * avg_ratio) as f32);
        let g = self.style.dash.map_or(0.0, |d| (d[1] * avg_ratio) as f32);

        // Top edge
        geom.lines.push(ColoredLine {
            x0: px0,
            y0: py0,
            x1: px1,
            y1: py0,
            width: lw,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash: d,
            gap: g,
        });
        // Bottom edge
        geom.lines.push(ColoredLine {
            x0: px0,
            y0: py1,
            x1: px1,
            y1: py1,
            width: lw,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash: d,
            gap: g,
        });
        // Left edge
        geom.lines.push(ColoredLine {
            x0: px0,
            y0: py0,
            x1: px0,
            y1: py1,
            width: lw,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash: d,
            gap: g,
        });
        // Right edge
        geom.lines.push(ColoredLine {
            x0: px1,
            y0: py0,
            x1: px1,
            y1: py1,
            width: lw,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash: d,
            gap: g,
        });

        if show_anchors {
            let avg_ratio = (h_pixel_ratio + v_pixel_ratio) * 0.5;
            let radius = (self.anchors[0].hit_radius * avg_ratio).round();
            let border_width = (1.0 * avg_ratio).floor().max(1.0);
            let mx = ((px0 + px1) * 0.5) as f64;
            let my = ((py0 + py1) * 0.5) as f64;
            geom.anchors = vec![
                AnchorCircle {
                    cx: px0 as f64,
                    cy: py0 as f64,
                    radius,
                    fill: super::default_anchor_color(),
                    border: *c,
                    border_width,
                }, // TL
                AnchorCircle {
                    cx: px1 as f64,
                    cy: py0 as f64,
                    radius,
                    fill: super::default_anchor_color(),
                    border: *c,
                    border_width,
                }, // TR
                AnchorCircle {
                    cx: px1 as f64,
                    cy: py1 as f64,
                    radius,
                    fill: super::default_anchor_color(),
                    border: *c,
                    border_width,
                }, // BR
                AnchorCircle {
                    cx: px0 as f64,
                    cy: py1 as f64,
                    radius,
                    fill: super::default_anchor_color(),
                    border: *c,
                    border_width,
                }, // BL
                AnchorCircle {
                    cx: mx,
                    cy: py0 as f64,
                    radius,
                    fill: super::default_anchor_color(),
                    border: *c,
                    border_width,
                }, // TM
                AnchorCircle {
                    cx: px1 as f64,
                    cy: my,
                    radius,
                    fill: super::default_anchor_color(),
                    border: *c,
                    border_width,
                }, // RM
                AnchorCircle {
                    cx: mx,
                    cy: py1 as f64,
                    radius,
                    fill: super::default_anchor_color(),
                    border: *c,
                    border_width,
                }, // BM
                AnchorCircle {
                    cx: px0 as f64,
                    cy: my,
                    radius,
                    fill: super::default_anchor_color(),
                    border: *c,
                    border_width,
                }, // LM
            ];
        }

        geom
    }

    fn move_anchor(&mut self, index: usize, bar_index: f64, price: f64) {
        if Self::is_corner_anchor(index) {
            let Some((opp_bar, opp_price)) = self.opposite_corner(index) else {
                return;
            };
            self.move_corner_with_fixed_opposite(index, bar_index, price, opp_bar, opp_price);
            return;
        }

        let (mut left, mut right, mut top, mut bottom) = self.normalized_bounds();
        match index {
            4 => {
                // Top edge midpoint
                top = price;
            }
            5 => {
                // Right edge midpoint
                right = bar_index;
            }
            6 => {
                // Bottom edge midpoint
                bottom = price;
            }
            7 => {
                // Left edge midpoint
                left = bar_index;
            }
            _ => return,
        }
        let norm_left = left.min(right);
        let norm_right = left.max(right);
        let norm_top = top.max(bottom);
        let norm_bottom = top.min(bottom);
        self.set_from_bounds(norm_left, norm_right, norm_top, norm_bottom);
    }
}
