//! Text drawing — single-anchor, auto-sized text annotation with optional
//! fill background and optional border.
//!
//! Behavior:
//! - One click places the anchor and immediately enters text-edit mode.
//! - The text box is auto-sized to fit the entered text (with padding).
//! - Alignment (H + V) determines how the box expands relative to the anchor:
//!   - H=Left/Center/Right: anchor is the left edge / horizontal center / right edge.
//!   - V=Top/Middle/Bottom: anchor is the top edge / vertical center / bottom edge.
//! - Fill (`style.fill_color`) and border (`style.color` + `line_width` + `dash`)
//!   each have an explicit on/off toggle stored on the drawing so the user can
//!   disable them while preserving the last picked color.

use super::drawing::{
    next_drawing_id, optical_middle_top, point_to_bitmap, point_to_css, prepare_text_block,
    push_text_block, Drawing, TEXT_DRAWING_GAP_CSS,
};
use super::hit_test;
use super::types::*;
use crate::core::renderer::draw_list::{ColoredLine, ColoredRect, TextAlign, TextVerticalAlign};
use crate::core::viewport::Viewport;
use crate::impl_drawing_accessors;

/// Internal padding (CSS px) between the text glyphs and the surrounding
/// border / fill rectangle. Applied on every side.
const TEXT_BOX_PADDING_CSS: f64 = TEXT_DRAWING_GAP_CSS;

/// Minimum CSS-pixel width of an empty text box (used for hit-testing and
/// rendering the empty placeholder / caret target).
const MIN_BOX_WIDTH_CSS: f64 = 12.0;

#[derive(Debug)]
pub struct TextDrawing {
    id: u64,
    state: DrawingState,
    locked: bool,
    style: DrawingStyle,
    anchors: Vec<AnchorPoint>,
    text: DrawingText,
    /// Whether the border (rectangle outline) is drawn. When `false`, the
    /// border color is preserved on `style.color` so toggling back on
    /// restores the previous color.
    border_enabled: bool,
    /// Whether the fill background is drawn. When `false`, the fill color is
    /// preserved on `style.fill_color` so toggling back on restores it.
    fill_enabled: bool,
}

impl TextDrawing {
    pub fn new(bar_index: f64, price: f64) -> Self {
        let id = next_drawing_id();
        let theme = crate::core::renderer::theme::ThemeConfig::default();
        let mut style = DrawingStyle::from_theme(&theme);
        style.line_width = 1.0;
        // Always keep a fill color stored even when fill is disabled so the
        // user can toggle without losing their last pick. Default fill is
        // the drawing color at low alpha (matches Rectangle convention).
        let mut fill = style.color;
        fill[3] = 0.15;
        style.fill_color = Some(fill);

        Self {
            id,
            // Single-anchor tool: start at step 0 so finalize_creation_step()
            // completes immediately on the first click.
            state: DrawingState::Creating { step: 0 },
            locked: false,
            style,
            anchors: vec![AnchorPoint::new(bar_index, price)],
            text: DrawingText {
                value: String::new(),
                horizontal_align: TextAlign::Left,
                vertical_align: TextVerticalAlign::Top,
                style: DrawingTextStyle::default(),
            },
            // Defaults: text-only by default — no border, no fill — matches
            // the typical "just drop a label on the chart" use case. The user
            // can opt-in via the inspector.
            border_enabled: false,
            fill_enabled: false,
        }
    }

    pub fn text(&self) -> &DrawingText {
        &self.text
    }

    pub fn text_mut(&mut self) -> &mut DrawingText {
        &mut self.text
    }

    #[inline]
    pub fn border_enabled(&self) -> bool {
        self.border_enabled
    }

    #[inline]
    pub fn set_border_enabled(&mut self, enabled: bool) {
        self.border_enabled = enabled;
    }

    #[inline]
    pub fn fill_enabled(&self) -> bool {
        self.fill_enabled
    }

    #[inline]
    pub fn set_fill_enabled(&mut self, enabled: bool) {
        self.fill_enabled = enabled;
    }

    /// Compute the CSS-pixel bounding box of the text + padding, anchored
    /// according to the current alignment. Returns `(left, top, right, bottom)`.
    pub fn box_css_bounds(&self, anchor_css_x: f64, anchor_css_y: f64) -> (f64, f64, f64, f64) {
        let fs = self.text.style.resolved_font_size(self.style.font_size) as f32;
        let display_text = if self.text.value.trim().is_empty() {
            // Reserve at least one line of space for empty boxes so the caret
            // and selection outline have something to grab.
            " "
        } else {
            self.text.value.as_str()
        };
        let block = prepare_text_block(display_text, fs);
        let (text_w, text_h) = if let Some(b) = block {
            (b.max_width as f64, b.total_height as f64)
        } else {
            (0.0, fs as f64)
        };
        let pad = TEXT_BOX_PADDING_CSS;
        let box_w = (text_w + pad * 2.0).max(MIN_BOX_WIDTH_CSS);
        let box_h = text_h + pad * 2.0;

        let left = match self.text.horizontal_align {
            TextAlign::Left => anchor_css_x,
            TextAlign::Center => anchor_css_x - box_w * 0.5,
            TextAlign::Right => anchor_css_x - box_w,
        };
        let top = match self.text.vertical_align {
            TextVerticalAlign::Top => anchor_css_y,
            TextVerticalAlign::Middle => anchor_css_y - box_h * 0.5,
            TextVerticalAlign::Bottom => anchor_css_y - box_h,
        };
        (left, top, left + box_w, top + box_h)
    }
}

impl Drawing for TextDrawing {
    impl_drawing_accessors!(DrawingTool::Text);

    fn required_anchors(&self) -> usize {
        1
    }

    fn hit_test(&self, cx: f64, cy: f64, vp: &Viewport, pw: f64, ph: f64) -> HitResult {
        if self.anchors.is_empty() {
            return HitResult::miss();
        }
        let (ax, ay) = point_to_css(&self.anchors[0].point, vp, pw, ph);
        // Anchor handle hit
        let ad = hit_test::point_to_circle_distance(cx, cy, ax, ay);
        if ad <= hit_test::ANCHOR_HIT_THRESHOLD_CSS {
            return HitResult::hit(HitPart::Anchor(0), ad);
        }
        // Body = inside the auto-sized text box (with padding).
        let (left, top, right, bottom) = self.box_css_bounds(ax, ay);
        if hit_test::point_in_rect(cx, cy, left, top, right, bottom) {
            let d = hit_test::point_to_rect_edge_distance(cx, cy, left, top, right, bottom);
            return HitResult::hit(HitPart::Body, d);
        }
        // Within edge threshold from outside → still treat as Body for
        // easier grabbing on small text boxes.
        let d = hit_test::point_to_rect_edge_distance(cx, cy, left, top, right, bottom);
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
        if self.anchors.is_empty() {
            return geom;
        }
        let snap_to_pixel = true;
        let (ax_bmp, ay_bmp) = point_to_bitmap(
            &self.anchors[0].point,
            vp,
            pw,
            ph,
            h_pixel_ratio,
            v_pixel_ratio,
            snap_to_pixel,
        );

        let avg_ratio = (h_pixel_ratio + v_pixel_ratio) * 0.5;
        let fs = (self.text.style.resolved_font_size(self.style.font_size) * avg_ratio) as f32;
        let pad = (TEXT_BOX_PADDING_CSS * avg_ratio) as f32;
        let min_w_bmp = (MIN_BOX_WIDTH_CSS * avg_ratio) as f32;

        let block = prepare_text_block(
            if self.text.value.trim().is_empty() {
                " "
            } else {
                self.text.value.as_str()
            },
            fs,
        );
        let (text_w, text_h) = if let Some(b) = &block {
            (b.max_width, b.total_height)
        } else {
            (0.0_f32, fs)
        };
        let box_w = (text_w + pad * 2.0).max(min_w_bmp);
        let box_h = text_h + pad * 2.0;

        let box_left = match self.text.horizontal_align {
            TextAlign::Left => ax_bmp as f32,
            TextAlign::Center => ax_bmp as f32 - box_w * 0.5,
            TextAlign::Right => ax_bmp as f32 - box_w,
        };
        let box_top = match self.text.vertical_align {
            TextVerticalAlign::Top => ay_bmp as f32,
            TextVerticalAlign::Middle => ay_bmp as f32 - box_h * 0.5,
            TextVerticalAlign::Bottom => ay_bmp as f32 - box_h,
        };
        let box_right = box_left + box_w;
        let box_bottom = box_top + box_h;

        // Fill background.
        if self.fill_enabled {
            if let Some(fc) = self.style.fill_color {
                geom.rects.push(ColoredRect {
                    x: box_left,
                    y: box_top,
                    w: box_w,
                    h: box_h,
                    r: fc[0],
                    g: fc[1],
                    b: fc[2],
                    a: fc[3],
                });
            }
        }

        // Border (4 edges).
        if self.border_enabled {
            let c = &self.style.color;
            let lw = (self.style.line_width * avg_ratio).floor().max(1.0) as f32;
            let d = self.style.dash.map_or(0.0, |dd| (dd[0] * avg_ratio) as f32);
            let g = self.style.dash.map_or(0.0, |dd| (dd[1] * avg_ratio) as f32);
            let edges = [
                (box_left, box_top, box_right, box_top),
                (box_left, box_bottom, box_right, box_bottom),
                (box_left, box_top, box_left, box_bottom),
                (box_right, box_top, box_right, box_bottom),
            ];
            for (x0, y0, x1, y1) in edges {
                geom.lines.push(ColoredLine {
                    x0,
                    y0,
                    x1,
                    y1,
                    width: lw,
                    r: c[0],
                    g: c[1],
                    b: c[2],
                    a: c[3],
                    dash: d,
                    gap: g,
                });
            }
        }

        // Text glyphs.
        if let Some(block) = block {
            if !self.text.value.trim().is_empty() {
                let text_color = self.text.style.resolved_color(self.style.color);
                // Always horizontally CENTER the text inside the auto-sized
                // box. The drawing's `horizontal_align` only governs how the
                // box is positioned relative to the click anchor — once the
                // box exists, padding should be visually equal on both sides.
                // The width estimate from `prepare_text_block` over- or
                // under-shoots real font metrics, so left-aligning the text
                // would leave a visibly uneven gap on the right.
                let text_x = (box_left + box_right) * 0.5;
                // Optically center the text vertically inside the box. The
                // canvas "top" baseline used by `push_text_block` includes
                // ascender padding above the cap height, so a naive
                // `box_top + pad` leaves visible empty space above the caps
                // and crowds the descenders against the bottom edge.
                // `optical_middle_top` gives the top-y that visually centers
                // glyphs around the supplied anchor y.
                let box_center_y = (box_top + box_bottom) * 0.5;
                let top_y = optical_middle_top(box_center_y, &block, fs);
                push_text_block(
                    &mut geom.texts,
                    &block,
                    text_x,
                    top_y,
                    fs,
                    600,
                    self.text.style.italic,
                    text_color,
                    TextAlign::Center,
                );
            }
        }

        if show_anchors {
            let radius = (self.anchors[0].hit_radius * avg_ratio).round();
            let border_width = (1.0 * avg_ratio).floor().max(1.0);
            geom.anchors.push(AnchorCircle {
                cx: ax_bmp,
                cy: ay_bmp,
                radius,
                fill: super::default_anchor_color(),
                border: self.style.color,
                border_width,
            });
        }

        geom
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::renderer::draw_list::{TextAlign, TextVerticalAlign};

    fn test_viewport() -> Viewport {
        let mut vp = Viewport::new(1000, 600);
        vp.start_bar = 0.0;
        vp.end_bar = 100.0;
        vp.price_min = 0.0;
        vp.price_max = 100.0;
        vp
    }

    #[test]
    fn defaults_are_text_only_no_border_no_fill() {
        let d = TextDrawing::new(50.0, 50.0);
        assert!(!d.border_enabled());
        assert!(!d.fill_enabled());
        assert_eq!(d.required_anchors(), 1);
    }

    #[test]
    fn empty_text_creates_no_glyphs_but_has_clickable_box() {
        let vp = test_viewport();
        let mut d = TextDrawing::new(50.0, 50.0);
        d.set_state(DrawingState::Idle);

        let geom = d.generate_geometry(&vp, 1000.0, 600.0, 1.0, 1.0, 1.0, false);
        assert!(geom.texts.is_empty());

        // Find the actual anchor CSS position so the hit-test target is
        // independent of viewport price-axis transforms.
        let (ax, ay) = super::point_to_css(&d.anchors[0].point, &vp, 1000.0, 600.0);
        let (left, top, right, bottom) = d.box_css_bounds(ax, ay);
        let cx = (left + right) * 0.5;
        let cy = (top + bottom) * 0.5;
        let result = d.hit_test(cx, cy, &vp, 1000.0, 600.0);
        assert!(
            result.is_hit(),
            "click inside the empty text box should hit"
        );
    }

    #[test]
    fn enabling_border_and_fill_produces_geometry() {
        let vp = test_viewport();
        let mut d = TextDrawing::new(50.0, 50.0);
        d.set_state(DrawingState::Idle);
        d.text_mut().value = "Hello".to_string();
        d.set_border_enabled(true);
        d.set_fill_enabled(true);

        let geom = d.generate_geometry(&vp, 1000.0, 600.0, 1.0, 1.0, 1.0, false);
        assert_eq!(geom.lines.len(), 4, "border = 4 edge lines");
        assert_eq!(geom.rects.len(), 1, "fill = 1 rect");
        assert!(!geom.texts.is_empty(), "text glyphs rendered");
    }

    #[test]
    fn alignment_shifts_box_relative_to_anchor() {
        let d_left = {
            let mut d = TextDrawing::new(50.0, 50.0);
            d.text_mut().value = "abc".to_string();
            d.text_mut().horizontal_align = TextAlign::Left;
            d
        };
        let d_right = {
            let mut d = TextDrawing::new(50.0, 50.0);
            d.text_mut().value = "abc".to_string();
            d.text_mut().horizontal_align = TextAlign::Right;
            d
        };
        let (left_l, _, _, _) = d_left.box_css_bounds(100.0, 100.0);
        let (left_r, _, right_r, _) = d_right.box_css_bounds(100.0, 100.0);
        // Left-aligned: anchor == left edge.
        assert!((left_l - 100.0).abs() < 1e-6);
        // Right-aligned: anchor == right edge.
        assert!((right_r - 100.0).abs() < 1e-6);
        assert!(left_r < 100.0);
    }

    #[test]
    fn vertical_middle_centers_box_on_anchor() {
        let mut d = TextDrawing::new(50.0, 50.0);
        d.text_mut().value = "x".to_string();
        d.text_mut().vertical_align = TextVerticalAlign::Middle;
        let (_, top, _, bottom) = d.box_css_bounds(100.0, 200.0);
        let center = (top + bottom) * 0.5;
        assert!((center - 200.0).abs() < 1e-6);
    }
}
