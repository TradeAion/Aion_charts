//! Drawing trait — the interface all drawing tools implement.

use super::types::*;
use crate::core::renderer::draw_list::{ColoredLine, DrawText, TextAlign, TextVerticalAlign};
use crate::core::viewport::Viewport;
use std::any::Any;

/// Unique ID counter for drawings.
static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Universal CSS-pixel gap between a drawing shape and its attached text label.
///
/// Used by every text-capable tool (lines, rays, rectangle, horizontal/vertical line)
/// to keep the visual spacing between the shape edge and its text consistent.
/// Multiply by `avg_ratio` (devicePixelRatio average) when emitting bitmap geometry.
pub const TEXT_DRAWING_GAP_CSS: f64 = 2.0;

pub fn next_drawing_id() -> u64 {
    NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Ensure subsequent calls to `next_drawing_id()` return at least `min_next`.
pub fn ensure_next_drawing_id_at_least(min_next: u64) {
    use std::sync::atomic::Ordering;
    let mut current = NEXT_ID.load(Ordering::Relaxed);
    while current < min_next {
        match NEXT_ID.compare_exchange_weak(current, min_next, Ordering::Relaxed, Ordering::Relaxed)
        {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}

/// Macro to implement the repetitive accessor methods on Drawing.
///
/// All drawing structs have the same `id`, `state`, `style`, `anchors` fields
/// and the same trivial accessor impls. This macro eliminates ~40 lines of
/// boilerplate per tool.
///
/// Usage:
/// ```ignore
/// impl Drawing for MyDrawing {
///     impl_drawing_accessors!(DrawingTool::MyTool);
///     fn required_anchors(&self) -> usize { 2 }
///     fn hit_test(...) { ... }
///     fn generate_geometry(...) { ... }
/// }
/// ```
#[macro_export]
macro_rules! impl_drawing_accessors {
    ($tool:expr) => {
        fn id(&self) -> u64 {
            self.id
        }
        fn set_id(&mut self, id: u64) {
            self.id = id;
        }
        fn tool(&self) -> DrawingTool {
            $tool
        }
        fn state(&self) -> DrawingState {
            self.state.clone()
        }
        fn set_state(&mut self, state: DrawingState) {
            self.state = state;
        }
        fn style(&self) -> &DrawingStyle {
            &self.style
        }
        fn style_mut(&mut self) -> &mut DrawingStyle {
            &mut self.style
        }
        fn anchors(&self) -> &[AnchorPoint] {
            &self.anchors
        }
        fn anchors_mut(&mut self) -> &mut Vec<AnchorPoint> {
            &mut self.anchors
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    };
}

/// The trait every drawing tool implements.
pub trait Drawing: std::fmt::Debug {
    /// Unique ID for this drawing instance.
    fn id(&self) -> u64;
    fn set_id(&mut self, id: u64);

    /// The tool type.
    fn tool(&self) -> DrawingTool;

    /// Current interaction state.
    fn state(&self) -> DrawingState;
    fn set_state(&mut self, state: DrawingState);

    /// Style (color, width, dash, etc.)
    fn style(&self) -> &DrawingStyle;
    fn style_mut(&mut self) -> &mut DrawingStyle;

    /// Anchor points (logical coordinates).
    fn anchors(&self) -> &[AnchorPoint];
    fn anchors_mut(&mut self) -> &mut Vec<AnchorPoint>;

    /// Downcast helpers for tool-specific persistence.
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// How many anchor points this tool needs to be fully created.
    fn required_anchors(&self) -> usize;

    /// Hit-test: does the cursor (in CSS px) intersect this drawing?
    /// `vp`, `pane_css_w`, `pane_css_h` are needed to convert logical→pixel.
    fn hit_test(
        &self,
        cursor_css_x: f64,
        cursor_css_y: f64,
        vp: &Viewport,
        pane_css_w: f64,
        pane_css_h: f64,
    ) -> HitResult;

    /// Generate pixel-space geometry for rendering.
    /// `show_anchors`: true when Selected or Dragging (render anchor circles).
    /// `h_pixel_ratio` / `v_pixel_ratio`: separate horizontal/vertical ratios
    /// for bitmap-accurate coordinate conversion (from device-pixel-content-box).
    fn generate_geometry(
        &self,
        vp: &Viewport,
        pane_css_w: f64,
        pane_css_h: f64,
        dpr: f64,
        h_pixel_ratio: f64,
        v_pixel_ratio: f64,
        show_anchors: bool,
    ) -> DrawingGeometry;

    /// Z-order for this drawing's current state.
    fn z_order(&self) -> ZOrder {
        match self.state() {
            DrawingState::Creating { .. }
            | DrawingState::Dragging { .. }
            | DrawingState::Selected => ZOrder::Top,
            DrawingState::Idle => ZOrder::Bottom,
        }
    }

    /// Called during creation: add the next anchor at the given logical position.
    /// Returns true if the drawing is now complete.
    fn add_creation_point(&mut self, bar_index: f64, price: f64) -> bool {
        let step = match self.state() {
            DrawingState::Creating { step } => step as usize,
            _ => return true,
        };
        let required = self.required_anchors();

        let anchors = self.anchors_mut();
        if step < anchors.len() {
            anchors[step].point = DrawingPoint::new(bar_index, price);
        } else {
            anchors.push(AnchorPoint::new(bar_index, price));
        }

        let next_step = step + 1;
        if next_step >= required {
            self.set_state(DrawingState::Idle);
            true
        } else {
            self.set_state(DrawingState::Creating {
                step: next_step as u8,
            });
            false
        }
    }

    /// Update the "live preview" anchor during creation (mouse move).
    fn update_creation_preview(&mut self, bar_index: f64, price: f64) {
        let step = match self.state() {
            DrawingState::Creating { step } => step as usize,
            _ => return,
        };
        let anchors = self.anchors_mut(); // borrow after state read
                                          // Ensure we have enough anchors for the preview
        while anchors.len() <= step {
            anchors.push(AnchorPoint::new(bar_index, price));
        }
        anchors[step].point = DrawingPoint::new(bar_index, price);
    }

    /// Move the entire drawing by a delta in logical coordinates.
    fn move_by(&mut self, delta_bar: f64, delta_price: f64) {
        for anchor in self.anchors_mut().iter_mut() {
            anchor.point.bar_index += delta_bar;
            anchor.point.price += delta_price;
        }
    }

    /// Move a single anchor to a new logical position.
    fn move_anchor(&mut self, index: usize, bar_index: f64, price: f64) {
        let anchors = self.anchors_mut();
        if index < anchors.len() {
            anchors[index].point = DrawingPoint::new(bar_index, price);
        }
    }
}

// ── Helper: convert DrawingPoint to bitmap pixel coords ─────────────────────

/// Convert a logical DrawingPoint to CSS pixel coordinates.
///
/// bar_index is fractional (from `pixel_to_bar`), so NO +0.5 offset is needed.
/// We still apply the viewport's `-1px` X alignment so a drawing anchor created
/// from a pointer position round-trips back to the same CSS coordinate used by
/// the crosshair and time-scale snapping helpers.
/// Y uses the pane height and relies on `Viewport::price_to_css_y()` to apply
/// the candle-area fraction internally, keeping drawings locked to the price
/// pane even when the main pane reserves space for volume below.
pub fn point_to_css(
    pt: &DrawingPoint,
    vp: &Viewport,
    pane_css_w: f64,
    pane_css_h: f64,
) -> (f64, f64) {
    let frac = (pt.bar_index - vp.start_bar) / (vp.end_bar - vp.start_bar);
    let x = frac * pane_css_w - 1.0;
    let y = vp.price_to_css_y(pt.price, pane_css_h);
    (x, y)
}

/// Convert a logical DrawingPoint to bitmap (physical pixel) coordinates.
///
/// Uses separate horizontal/vertical pixel ratios (from device-pixel-content-box)
/// and rounds to nearest pixel for crisp rendering, matching LWC's approach.
pub fn point_to_bitmap(
    pt: &DrawingPoint,
    vp: &Viewport,
    pane_css_w: f64,
    pane_css_h: f64,
    h_pixel_ratio: f64,
    v_pixel_ratio: f64,
    snap_to_pixel: bool,
) -> (f64, f64) {
    let (cx, cy) = point_to_css(pt, vp, pane_css_w, pane_css_h);
    // Match the renderer's LWC-style `-1px` X bias in physical space too.
    // Plain `cx * ratio` drifts on fractional DPR / exact bitmap sizing.
    let bx = (cx + 1.0) * h_pixel_ratio - 1.0;
    let by = cy * v_pixel_ratio;
    if snap_to_pixel {
        (bx.round(), by.round())
    } else {
        (bx, by)
    }
}

/// Generate standard anchor circles for a drawing.
/// Uses separate h/v pixel ratios for bitmap-accurate placement.
pub fn generate_anchor_circles(
    anchors: &[AnchorPoint],
    vp: &Viewport,
    pane_css_w: f64,
    pane_css_h: f64,
    h_pixel_ratio: f64,
    v_pixel_ratio: f64,
    color: &[f32; 4],
    snap_to_pixel: bool,
) -> Vec<AnchorCircle> {
    anchors
        .iter()
        .map(|a| {
            let (bx, by) = point_to_bitmap(
                &a.point,
                vp,
                pane_css_w,
                pane_css_h,
                h_pixel_ratio,
                v_pixel_ratio,
                snap_to_pixel,
            );
            // Use average ratio for radius so circles stay round
            let avg_ratio = (h_pixel_ratio + v_pixel_ratio) * 0.5;
            AnchorCircle {
                cx: bx,
                cy: by,
                radius: (a.hit_radius * avg_ratio).round(),
                fill: super::default_anchor_color(),
                border: *color,
                border_width: (1.0 * avg_ratio).floor().max(1.0),
            }
        })
        .collect()
}

/// Prepared multi-line text block metrics for drawing labels.
#[derive(Debug, Clone)]
pub struct PreparedTextBlock {
    pub lines: Vec<String>,
    pub line_height: f32,
    pub total_height: f32,
    pub max_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextBlockBounds {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineLabelPlacement {
    pub anchor_x: f32,
    pub anchor_y: f32,
    pub top_local_y: f32,
    pub align: TextAlign,
    pub rotation_rad: f32,
    pub anchor_t: f64,
    pub line_len: f64,
}

fn estimate_text_line_width(line: &str, font_size: f32) -> f32 {
    line.chars()
        .map(|ch| match ch {
            'i' | 'l' | '!' | '|' | '.' | ',' | ':' | ';' | '\'' => 0.32,
            ' ' => 0.33,
            'm' | 'w' | 'M' | 'W' | '@' | '#' | '%' | '&' => 0.9,
            '0'..='9' => 0.62,
            'A'..='Z' => 0.7,
            _ => 0.58,
        })
        .sum::<f32>()
        * font_size
}

pub fn prepare_text_block(text: &str, font_size: f32) -> Option<PreparedTextBlock> {
    let lines = text
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return None;
    }

    let line_height = (font_size * 1.2).max(font_size);
    let total_height = font_size + line_height * (lines.len().saturating_sub(1) as f32);
    let max_width = lines
        .iter()
        .map(|line| estimate_text_line_width(line, font_size))
        .fold(0.0, f32::max);

    Some(PreparedTextBlock {
        lines,
        line_height,
        total_height,
        max_width,
    })
}

pub fn text_block_bounds(
    block: &PreparedTextBlock,
    x: f32,
    top_y: f32,
    align: TextAlign,
) -> TextBlockBounds {
    let left = match align {
        TextAlign::Left => x,
        TextAlign::Center => x - block.max_width * 0.5,
        TextAlign::Right => x - block.max_width,
    };
    TextBlockBounds {
        left,
        top: top_y,
        width: block.max_width,
        height: block.total_height,
    }
}

pub fn push_text_block(
    texts: &mut Vec<DrawText>,
    block: &PreparedTextBlock,
    x: f32,
    top_y: f32,
    font_size: f32,
    font_weight: u16,
    _italic: bool,
    color: [f32; 4],
    align: TextAlign,
) {
    // Drawing labels are ALWAYS italic regardless of caller-supplied style.
    // This is a deliberate product decision: italic is the visual signature
    // of drawing-tool annotations (matches TradingView). The `_italic` arg
    // is kept for call-site compatibility but ignored.
    let italic = true;
    for (line_idx, line) in block.lines.iter().enumerate() {
        texts.push(DrawText {
            text: line.clone(),
            x,
            y: top_y + block.line_height * line_idx as f32,
            font_size,
            font_weight,
            italic,
            rotation_rad: 0.0,
            r: color[0],
            g: color[1],
            b: color[2],
            a: color[3],
            align,
            vertical_align: TextVerticalAlign::Top,
        });
    }
}

pub fn push_rotated_text_block(
    texts: &mut Vec<DrawText>,
    block: &PreparedTextBlock,
    anchor_x: f32,
    anchor_y: f32,
    top_local_y: f32,
    font_size: f32,
    font_weight: u16,
    _italic: bool,
    color: [f32; 4],
    align: TextAlign,
    rotation_rad: f32,
) {
    // See `push_text_block`: drawing labels are always italic.
    let italic = true;
    let (sin_theta, cos_theta) = rotation_rad.sin_cos();
    for (line_idx, line) in block.lines.iter().enumerate() {
        let local_y = top_local_y + block.line_height * line_idx as f32;
        let x = anchor_x - sin_theta * local_y;
        let y = anchor_y + cos_theta * local_y;
        texts.push(DrawText {
            text: line.clone(),
            x,
            y,
            font_size,
            font_weight,
            italic,
            rotation_rad,
            r: color[0],
            g: color[1],
            b: color[2],
            a: color[3],
            align,
            vertical_align: TextVerticalAlign::Top,
        });
    }
}

pub fn optical_middle_top(anchor_y: f32, block: &PreparedTextBlock, font_size: f32) -> f32 {
    anchor_y - block.total_height * 0.5 + font_size * 0.12
}

pub fn optical_middle_local_top(block: &PreparedTextBlock, font_size: f32) -> f32 {
    -block.total_height * 0.5 + font_size * 0.08
}

fn push_line_segment(
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

pub fn push_line_with_gap(
    lines: &mut Vec<ColoredLine>,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    width: f32,
    color: [f32; 4],
    dash: f32,
    gap: f32,
    gap_bounds: Option<TextBlockBounds>,
    gap_padding: f32,
) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= f64::EPSILON {
        return;
    }

    if let Some(bounds) = gap_bounds {
        let ux = dx / len;
        let uy = dy / len;
        let center_x = bounds.left as f64 + bounds.width as f64 * 0.5;
        let center_y = bounds.top as f64 + bounds.height as f64 * 0.5;
        let center_t = ((center_x - x0) * ux + (center_y - y0) * uy).clamp(0.0, len);
        let half_span = 0.5 * (bounds.width as f64 * ux.abs() + bounds.height as f64 * uy.abs())
            + gap_padding as f64;
        let gap_start_t = (center_t - half_span).clamp(0.0, len);
        let gap_end_t = (center_t + half_span).clamp(0.0, len);
        let min_segment = (width as f64 * 0.75).max(1.0);

        if gap_start_t > min_segment {
            push_line_segment(
                lines,
                x0,
                y0,
                x0 + ux * gap_start_t,
                y0 + uy * gap_start_t,
                width,
                color,
                dash,
                gap,
            );
        }
        if len - gap_end_t > min_segment {
            push_line_segment(
                lines,
                x0 + ux * gap_end_t,
                y0 + uy * gap_end_t,
                x1,
                y1,
                width,
                color,
                dash,
                gap,
            );
        }
        return;
    }

    push_line_segment(lines, x0, y0, x1, y1, width, color, dash, gap);
}

pub fn push_line_with_gap_range(
    lines: &mut Vec<ColoredLine>,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    width: f32,
    color: [f32; 4],
    dash: f32,
    gap: f32,
    gap_range: Option<(f64, f64)>,
) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= f64::EPSILON {
        return;
    }

    if let Some((mut gap_start_t, mut gap_end_t)) = gap_range {
        gap_start_t = gap_start_t.clamp(0.0, len);
        gap_end_t = gap_end_t.clamp(0.0, len);
        if gap_end_t <= gap_start_t {
            return push_line_segment(lines, x0, y0, x1, y1, width, color, dash, gap);
        }

        let ux = dx / len;
        let uy = dy / len;
        let min_segment = (width as f64 * 0.75).max(1.0);

        if gap_start_t > min_segment {
            push_line_segment(
                lines,
                x0,
                y0,
                x0 + ux * gap_start_t,
                y0 + uy * gap_start_t,
                width,
                color,
                dash,
                gap,
            );
        }
        if len - gap_end_t > min_segment {
            push_line_segment(
                lines,
                x0 + ux * gap_end_t,
                y0 + uy * gap_end_t,
                x1,
                y1,
                width,
                color,
                dash,
                gap,
            );
        }
        return;
    }

    push_line_segment(lines, x0, y0, x1, y1, width, color, dash, gap);
}

pub fn line_text_anchor(
    start_x: f64,
    start_y: f64,
    end_x: f64,
    end_y: f64,
    align: TextAlign,
    inset: f64,
) -> (f64, f64, TextAlign) {
    let ((x0, y0), (x1, y1)) = if start_x <= end_x {
        ((start_x, start_y), (end_x, end_y))
    } else {
        ((end_x, end_y), (start_x, start_y))
    };

    let dx = x1 - x0;
    let dy = y1 - y0;
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let inset_t = (inset / len).clamp(0.0, 0.25);
    let t = match align {
        TextAlign::Left => inset_t,
        TextAlign::Center => 0.5,
        TextAlign::Right => 1.0 - inset_t,
    };

    (x0 + dx * t, y0 + dy * t, align)
}

pub fn line_label_placement(
    start_x: f64,
    start_y: f64,
    end_x: f64,
    end_y: f64,
    horizontal_align: TextAlign,
    vertical_align: TextVerticalAlign,
    block: &PreparedTextBlock,
    font_size: f32,
    inset: f64,
    side_gap: f64,
) -> LineLabelPlacement {
    let ((x0, y0), (x1, y1)) = if start_x < end_x || (start_x == end_x && start_y <= end_y) {
        ((start_x, start_y), (end_x, end_y))
    } else {
        ((end_x, end_y), (start_x, start_y))
    };

    let dx = x1 - x0;
    let dy = y1 - y0;
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let angle = dy.atan2(dx) as f32;
    let anchor_t = match horizontal_align {
        TextAlign::Left => inset.clamp(0.0, len),
        TextAlign::Center => len * 0.5,
        TextAlign::Right => (len - inset).clamp(0.0, len),
    };
    let ux = dx / len;
    let uy = dy / len;
    let anchor_x = (x0 + ux * anchor_t) as f32;
    let anchor_y = (y0 + uy * anchor_t) as f32;
    let top_local_y = match vertical_align {
        // Visually-tight gap: subtract the font's internal leading (~20% of
        // font_size sits as empty whitespace at the top of every glyph box).
        // Without this compensation the label looks like it's floating well
        // above the line even with side_gap=2px. Matches TradingView density.
        TextVerticalAlign::Top => {
            let internal_leading = font_size * 0.2;
            -side_gap as f32 - block.total_height + internal_leading
        }
        TextVerticalAlign::Middle => optical_middle_local_top(block, font_size),
        TextVerticalAlign::Bottom => side_gap as f32 - (font_size * 0.15),
    };

    LineLabelPlacement {
        anchor_x,
        anchor_y,
        top_local_y,
        align: horizontal_align,
        rotation_rad: angle,
        anchor_t,
        line_len: len,
    }
}

pub fn line_middle_gap_range(
    placement: &LineLabelPlacement,
    block: &PreparedTextBlock,
    padding: f32,
) -> Option<(f64, f64)> {
    let width = block.max_width as f64;
    let padding = padding as f64;
    if placement.line_len <= f64::EPSILON {
        return None;
    }

    let range = match placement.align {
        TextAlign::Left => (0.0, placement.anchor_t + width + padding),
        TextAlign::Center => (
            placement.anchor_t - width * 0.5 - padding,
            placement.anchor_t + width * 0.5 + padding,
        ),
        TextAlign::Right => (placement.anchor_t - width - padding, placement.line_len),
    };
    Some((
        range.0.clamp(0.0, placement.line_len),
        range.1.clamp(0.0, placement.line_len),
    ))
}

pub fn rotated_text_box_top_left(placement: &LineLabelPlacement, width: f64) -> (f64, f64) {
    let local_left_x = match placement.align {
        TextAlign::Left => 0.0,
        TextAlign::Center => -(width as f32) * 0.5,
        TextAlign::Right => -(width as f32),
    };
    let (sin_theta, cos_theta) = placement.rotation_rad.sin_cos();
    let global_left =
        placement.anchor_x + local_left_x * cos_theta - placement.top_local_y * sin_theta;
    let global_top =
        placement.anchor_y + local_left_x * sin_theta + placement.top_local_y * cos_theta;
    (global_left as f64, global_top as f64)
}

pub fn rect_text_anchor(
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    horizontal_align: TextAlign,
    vertical_align: TextVerticalAlign,
    inset_x: f64,
    inset_y: f64,
) -> (f64, f64, TextAlign, TextVerticalAlign) {
    let x = match horizontal_align {
        TextAlign::Left => left + inset_x,
        TextAlign::Center => (left + right) * 0.5,
        TextAlign::Right => right - inset_x,
    };
    // Rectangle semantics:
    // - Top / Bottom labels are outside the rectangle bounds.
    // - Middle labels remain inside, centered vertically.
    let y = match vertical_align {
        TextVerticalAlign::Top => top - inset_y,
        TextVerticalAlign::Middle => (top + bottom) * 0.5,
        TextVerticalAlign::Bottom => bottom + inset_y,
    };

    (x, y, horizontal_align, vertical_align)
}

#[cfg(test)]
mod tests {
    use super::{
        point_to_bitmap, point_to_css, prepare_text_block, push_line_with_gap, text_block_bounds,
        DrawingPoint,
    };
    use crate::core::renderer::draw_list::TextAlign;
    use crate::core::renderer::transforms::bar_to_x;
    use crate::core::viewport::Viewport;

    fn test_viewport() -> Viewport {
        let mut vp = Viewport::new(1000, 600);
        vp.start_bar = 10.0;
        vp.end_bar = 20.0;
        vp.price_min = 90.0;
        vp.price_max = 110.0;
        vp
    }

    #[test]
    fn point_to_css_matches_pixel_to_bar_round_trip() {
        let vp = test_viewport();
        let pane_w = 1000.0;
        let pane_h = 600.0;
        let pointer_x = 349.0;
        let logical_bar = vp.pixel_to_bar(pointer_x, pane_w);
        let point = DrawingPoint::new(logical_bar, 100.0);

        let (x, _y) = point_to_css(&point, &vp, pane_w, pane_h);

        assert!((x - pointer_x).abs() < 1e-9);
    }

    #[test]
    fn point_to_css_matches_bar_center_css_for_snapped_bar_centers() {
        let vp = test_viewport();
        let pane_w = 1000.0;
        let pane_h = 600.0;
        let snapped_slot = 13usize;
        let point = DrawingPoint::new(snapped_slot as f64 + 0.5, 100.0);

        let (x, _y) = point_to_css(&point, &vp, pane_w, pane_h);

        assert!((x - vp.bar_center_css(snapped_slot, pane_w)).abs() < 1e-9);
    }

    #[test]
    fn point_to_bitmap_matches_physical_bar_projection_for_fractional_ratio() {
        let vp = test_viewport();
        let pane_css_w = 1000.0;
        let pane_css_h = 600.0;
        let h_ratio = 1.25;
        let point = DrawingPoint::new(13.5, 100.0);

        let (x, _y) = point_to_bitmap(&point, &vp, pane_css_w, pane_css_h, h_ratio, 1.0, false);

        let expected_x = bar_to_x(point.bar_index, &vp, pane_css_w * h_ratio);
        assert!(
            (x - expected_x).abs() < 1e-9,
            "expected physical projection {expected_x}, got {x}"
        );
    }

    #[test]
    fn point_to_css_round_trips_pointer_price_when_volume_area_is_visible() {
        let mut vp = test_viewport();
        vp.volume_height_ratio = 0.15;
        let pane_css_h = 600.0;
        let candle_css_h = pane_css_h * vp.candle_height_frac();
        let y_css = 240.0;
        let price = vp.pixel_to_price(y_css, candle_css_h);
        let point = DrawingPoint::new(13.5, price);

        let (_x, projected_y) = point_to_css(&point, &vp, 1000.0, pane_css_h);

        assert!(
            (projected_y - y_css).abs() < 1e-9,
            "expected projected y {y_css}, got {projected_y}"
        );
    }

    #[test]
    fn centered_text_gap_splits_horizontal_line_into_two_segments() {
        let block = prepare_text_block("Dev", 12.0).expect("text block");
        let bounds = text_block_bounds(
            &block,
            100.0,
            50.0 - block.total_height * 0.5,
            TextAlign::Center,
        );
        let mut lines = Vec::new();

        push_line_with_gap(
            &mut lines,
            0.0,
            50.0,
            200.0,
            50.0,
            2.0,
            [1.0, 0.5, 0.0, 1.0],
            0.0,
            0.0,
            Some(bounds),
            4.0,
        );

        assert_eq!(lines.len(), 2);
        assert!(lines[0].x1 < 100.0);
        assert!(lines[1].x0 > 100.0);
    }
}
