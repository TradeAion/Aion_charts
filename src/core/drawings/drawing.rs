//! Drawing trait — the interface all drawing tools implement.

use super::types::*;
use crate::core::viewport::Viewport;
use std::any::Any;

/// Unique ID counter for drawings.
static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

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
            self.state
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
/// Y uses the candle area height (matching `price_to_css_y`) which is consistent
/// with how prices are recorded when candle_height_frac is applied.
pub fn point_to_css(
    pt: &DrawingPoint,
    vp: &Viewport,
    pane_css_w: f64,
    pane_css_h: f64,
) -> (f64, f64) {
    let frac = (pt.bar_index - vp.start_bar) / (vp.end_bar - vp.start_bar);
    let x = frac * pane_css_w;
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
) -> (f64, f64) {
    let (cx, cy) = point_to_css(pt, vp, pane_css_w, pane_css_h);
    let bx = (cx * h_pixel_ratio).round();
    let by = (cy * v_pixel_ratio).round();
    (bx, by)
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
