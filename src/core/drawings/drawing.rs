//! Drawing trait — the interface all drawing tools implement.

use crate::core::viewport::Viewport;
use super::types::*;

/// Unique ID counter for drawings.
static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub fn next_drawing_id() -> u64 {
    NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// The trait every drawing tool implements.
pub trait Drawing: std::fmt::Debug {
    /// Unique ID for this drawing instance.
    fn id(&self) -> u64;

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
    fn generate_geometry(
        &self,
        vp: &Viewport,
        pane_css_w: f64,
        pane_css_h: f64,
        dpr: f64,
        show_anchors: bool,
    ) -> DrawingGeometry;

    /// Z-order for this drawing's current state.
    fn z_order(&self) -> ZOrder {
        match self.state() {
            DrawingState::Creating { .. } | DrawingState::Dragging { .. } => ZOrder::Top,
            DrawingState::Selected => ZOrder::Normal,
            DrawingState::Idle => ZOrder::Normal,
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
            self.set_state(DrawingState::Creating { step: next_step as u8 });
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

// ── Helper: convert DrawingPoint to CSS pixel coords ────────────────────────

/// Convert a logical DrawingPoint to CSS pixel coordinates.
///
/// bar_index is fractional (from `pixel_to_bar`), so NO +0.5 offset is needed.
/// Y uses the candle area height (matching `price_to_css_y`) which is consistent
/// with how prices are recorded when candle_height_frac is applied.
pub fn point_to_css(
    pt: &DrawingPoint,
    vp: &Viewport,
    pane_css_w: f64,
    pane_css_h: f64,
) -> (f64, f64) {
    let frac = (pt.bar_index - vp.start_bar) / (vp.end_bar - vp.start_bar);
    let x = (frac * pane_css_w).clamp(0.0, pane_css_w);
    let y = vp.price_to_css_y(pt.price, pane_css_h);
    (x, y)
}

/// Generate standard anchor circles for a drawing.
pub fn generate_anchor_circles(
    anchors: &[AnchorPoint],
    vp: &Viewport,
    pane_css_w: f64,
    pane_css_h: f64,
    dpr: f64,
    color: &[f32; 4],
) -> Vec<AnchorCircle> {
    anchors.iter().map(|a| {
        let (cx, cy) = point_to_css(&a.point, vp, pane_css_w, pane_css_h);
        AnchorCircle {
            cx: cx * dpr,
            cy: cy * dpr,
            radius: (a.hit_radius * dpr).round(),
            fill: [1.0, 1.0, 1.0, 1.0], // white fill
            border: *color,
            border_width: (1.0 * dpr).floor().max(1.0),
        }
    }).collect()
}
