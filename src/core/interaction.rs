//! InteractionHandler — LWC-style pointer/wheel state machine.
//!
//! Pure Rust, no DOM dependencies. The WASM layer forwards raw pointer events
//! to these methods; the handler updates crosshair, pan, zoom state on the engine.
//!
//! Mirrors LWC's mouse-event-handler.ts architecture:
//! - Separate mouse-move (crosshair) vs pressed-mouse-move (drag/pan)
//! - Manhattan distance threshold before drag cancels click
//! - Wheel → zoom with focal point
//! - Magnet snap: crosshair snaps to nearest bar center X

use crate::core::data::Bar;
use crate::core::viewport::Viewport;
use crate::core::renderer::traits::{ChartStyle, CrosshairState};
use crate::core::renderer::series::ChartLayout;

/// Manhattan distance threshold before drag starts (LWC: CancelClickManhattanDistance = 5).
const CANCEL_CLICK_DISTANCE: f64 = 5.0;

/// Interaction state machine.
pub struct InteractionHandler {
    /// Is the pointer currently pressed?
    pressed: bool,
    /// Position where the pointer was pressed (CSS px).
    press_x: f64,
    press_y: f64,
    /// Last pointer position during drag (CSS px).
    last_x: f64,
    /// Has the drag exceeded manhattan distance threshold?
    drag_active: bool,
    /// Container dimensions in CSS px (set on each event).
    container_w: f64,
    container_h: f64,
}

impl InteractionHandler {
    pub fn new() -> Self {
        Self {
            pressed: false,
            press_x: 0.0,
            press_y: 0.0,
            last_x: 0.0,
            drag_active: false,
            container_w: 0.0,
            container_h: 0.0,
        }
    }

    /// Update container size — call on resize.
    pub fn set_container_size(&mut self, w: f64, h: f64) {
        self.container_w = w;
        self.container_h = h;
    }

    /// Pointer move event (CSS px relative to container).
    /// Updates crosshair + handles drag panning.
    pub fn pointer_move(
        &mut self,
        x: f64,
        y: f64,
        viewport: &mut Viewport,
        crosshair: &mut CrosshairState,
        bars: &[Bar],
        style: &ChartStyle,
        dpr: f64,
        y_axis_css_w: f64,
    ) {
        let layout = ChartLayout::from_physical(
            viewport.width, viewport.height, dpr, style, y_axis_css_w,
        );
        let chart_css_w = layout.chart_w / dpr;
        let chart_css_h = (layout.candle_h + layout.vol_h) / dpr;

        // Update crosshair (only when inside chart area)
        if x >= 0.0 && x <= chart_css_w && y >= 0.0 && y <= chart_css_h {
            crosshair.active = true;

            // Magnet snap: snap X to nearest bar center
            let bar_f = viewport.pixel_to_bar(x * dpr, layout.chart_w);
            let snapped_idx = bar_f.round().max(0.0) as usize;
            let snapped_idx = snapped_idx.min(bars.len().saturating_sub(1));

            // Convert snapped bar index back to pixel X
            let snapped_x = if !bars.is_empty() {
                let frac = (snapped_idx as f64 + 0.5 - viewport.start_bar)
                    / (viewport.end_bar - viewport.start_bar);
                frac * chart_css_w
            } else {
                x
            };

            crosshair.x = snapped_x;
            crosshair.y = y;
            crosshair.bar_index = if snapped_idx < bars.len() { Some(snapped_idx) } else { None };
            crosshair.price = viewport.pixel_to_price(y * dpr, layout.candle_h);
        } else {
            crosshair.active = false;
        }

        // Handle drag panning
        if self.pressed {
            let dx = x - self.last_x;
            let manhattan = (x - self.press_x).abs() + (y - self.press_y).abs();

            if !self.drag_active && manhattan >= CANCEL_CLICK_DISTANCE {
                self.drag_active = true;
            }

            if self.drag_active && chart_css_w > 0.0 {
                let bar_span = viewport.end_bar - viewport.start_bar;
                let delta_bars = -(dx / chart_css_w) * bar_span;
                viewport.pan_clamped(delta_bars, bars.len());

                if !viewport.price_locked {
                    viewport.auto_fit_price(bars);
                }
            }

            self.last_x = x;
        }
    }

    /// Pointer down event (CSS px).
    pub fn pointer_down(&mut self, x: f64, y: f64) {
        self.pressed = true;
        self.press_x = x;
        self.press_y = y;
        self.last_x = x;
        self.drag_active = false;
    }

    /// Pointer up event.
    pub fn pointer_up(&mut self) {
        self.pressed = false;
        self.drag_active = false;
    }

    /// Pointer leave event — hide crosshair.
    pub fn pointer_leave(&mut self, crosshair: &mut CrosshairState) {
        crosshair.active = false;
        self.pressed = false;
        self.drag_active = false;
    }

    /// Wheel event — zoom with focal point.
    /// `x` is CSS px from container left edge, `delta_y` is raw wheel deltaY.
    pub fn wheel(
        &mut self,
        x: f64,
        _y: f64,
        delta_y: f64,
        viewport: &mut Viewport,
        bars: &[Bar],
        style: &ChartStyle,
        dpr: f64,
        y_axis_css_w: f64,
    ) {
        let layout = ChartLayout::from_physical(
            viewport.width, viewport.height, dpr, style, y_axis_css_w,
        );
        let chart_css_w = layout.chart_w / dpr;
        if chart_css_w <= 0.0 { return; }

        let x_frac = (x / chart_css_w).clamp(0.0, 1.0);
        let focal_bar = viewport.start_bar + x_frac * (viewport.end_bar - viewport.start_bar);
        let factor = if delta_y > 0.0 { 1.1 } else { 1.0 / 1.1 };

        viewport.zoom(focal_bar, factor);

        if !viewport.price_locked {
            viewport.auto_fit_price(bars);
        }
    }

    /// Is the user currently dragging?
    pub fn is_dragging(&self) -> bool {
        self.drag_active
    }
}
