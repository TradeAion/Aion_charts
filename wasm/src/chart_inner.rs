//! Internal chart state and helper methods.
//!
//! This module contains `ChartInner`, the internal state shared between
//! event closures and the public RayCore API. Helper methods here handle
//! the borrow checker dance of destructuring to access multiple fields.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use raycore::{
    Canvas2DRenderer, ChartEngine, HitZone, InteractionHandler, OverlayRenderer, PriceAxisRenderer,
    TimeAxisRenderer,
};

use crate::canvas_manager::WidgetLayout;
use crate::subpane::{PaneHeightCoordinator, SubPane};

// ============================================================================
// Event Listener Handle - for proper cleanup in dispose()
// ============================================================================

/// Tracks an event listener so it can be removed later.
/// Stores the element, event name, and closure reference.
pub struct EventListenerHandle {
    element: web_sys::EventTarget,
    event_name: String,
    callback: js_sys::Function,
}

impl EventListenerHandle {
    /// Create a new handle and attach the listener to the element.
    pub fn new<F>(
        element: &web_sys::EventTarget,
        event_name: &str,
        closure: &Closure<F>,
    ) -> Result<Self, JsValue>
    where
        F: ?Sized,
    {
        let callback: js_sys::Function =
            closure.as_ref().unchecked_ref::<js_sys::Function>().clone();
        element.add_event_listener_with_callback(event_name, &callback)?;
        Ok(Self {
            element: element.clone(),
            event_name: event_name.to_string(),
            callback,
        })
    }

    /// Create a new handle with options and attach the listener.
    pub fn new_with_options<F>(
        element: &web_sys::EventTarget,
        event_name: &str,
        closure: &Closure<F>,
        options: &web_sys::AddEventListenerOptions,
    ) -> Result<Self, JsValue>
    where
        F: ?Sized,
    {
        let callback: js_sys::Function =
            closure.as_ref().unchecked_ref::<js_sys::Function>().clone();
        element.add_event_listener_with_callback_and_add_event_listener_options(
            event_name, &callback, options,
        )?;
        Ok(Self {
            element: element.clone(),
            event_name: event_name.to_string(),
            callback,
        })
    }

    /// Remove the event listener from the element.
    pub fn remove(&self) {
        let _ = self
            .element
            .remove_event_listener_with_callback(&self.event_name, &self.callback);
    }
}

/// Collection of event listener handles for batch removal.
pub struct EventListenerRegistry {
    handles: Vec<EventListenerHandle>,
}

impl EventListenerRegistry {
    pub fn new() -> Self {
        Self {
            handles: Vec::new(),
        }
    }

    /// Add an event listener and track it for later removal.
    pub fn add<F>(
        &mut self,
        element: &web_sys::EventTarget,
        event_name: &str,
        closure: &Closure<F>,
    ) -> Result<(), JsValue>
    where
        F: ?Sized,
    {
        let handle = EventListenerHandle::new(element, event_name, closure)?;
        self.handles.push(handle);
        Ok(())
    }

    /// Add an event listener with options and track it.
    pub fn add_with_options<F>(
        &mut self,
        element: &web_sys::EventTarget,
        event_name: &str,
        closure: &Closure<F>,
        options: &web_sys::AddEventListenerOptions,
    ) -> Result<(), JsValue>
    where
        F: ?Sized,
    {
        let handle = EventListenerHandle::new_with_options(element, event_name, closure, options)?;
        self.handles.push(handle);
        Ok(())
    }

    /// Remove all tracked event listeners.
    pub fn remove_all(&mut self) {
        for handle in self.handles.drain(..) {
            handle.remove();
        }
    }

    /// Get the number of tracked listeners.
    pub fn len(&self) -> usize {
        self.handles.len()
    }
}

// ============================================================================
// ExactPixelSizes and ChartInner
// ============================================================================

/// Exact device-pixel sizes for each widget container, reported by
/// `ResizeObserver` with `device-pixel-content-box`. When available these
/// replace the lossy `round(css * dpr)` fallback and eliminate ±1px blur.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExactPixelSizes {
    /// Set to true once the observer has fired at least once.
    pub available: bool,
    pub pane_pw: u32,
    pub pane_ph: u32,
    pub price_axis_pw: u32,
    pub price_axis_ph: u32,
    pub time_axis_pw: u32,
    pub time_axis_ph: u32,
    pub corner_stub_pw: u32,
    pub corner_stub_ph: u32,
}

/// Internal chart state shared between event closures and the public API.
pub struct ChartInner {
    pub engine: ChartEngine,
    pub overlay: OverlayRenderer,
    pub price_axis_renderer: PriceAxisRenderer,
    pub time_axis_renderer: TimeAxisRenderer,
    pub layout: WidgetLayout,
    pub interaction: InteractionHandler,
    /// Exact pixel sizes from device-pixel-content-box ResizeObserver.
    pub exact_sizes: ExactPixelSizes,
    /// Sub-panes for indicators (RSI, ATR, etc.)
    pub subpanes: Vec<SubPane>,
    /// Next sub-pane ID
    pub next_subpane_id: u32,
    /// Which sub-pane the cursor is currently over (None = main pane or outside).
    /// Used for proper crosshair coordination instead of y=-1000 hack.
    pub active_subpane_id: Option<u32>,
    /// Coordinates pane heights using stretch factors (PaneManager bridge).
    pub pane_coordinator: PaneHeightCoordinator,
}

/// Helper methods that destructure `self` to satisfy the borrow checker.
/// Each method borrows `interaction` and `engine` fields separately.
impl ChartInner {
    pub fn on_pointer_enter(&mut self, zone: HitZone) {
        let Self {
            interaction,
            engine,
            ..
        } = self;
        interaction.pointer_enter(zone, &mut engine.crosshair);
    }

    pub fn on_pointer_leave(&mut self, zone: HitZone) {
        let Self {
            interaction,
            engine,
            ..
        } = self;
        interaction.pointer_leave(zone, &mut engine.crosshair);
    }

    pub fn on_pane_pointer_move(&mut self, x: f64, y: f64) {
        let (pw, ph) = self.layout.pane_css_size();
        let dpr = self.engine.dpr;

        // Pre-compute logical coords from viewport (before any mutable drawing borrow)
        let bar = self.engine.viewport.pixel_to_bar(x, pw);
        // Use candle area height for drawing coordinates — matches price_to_css_y().
        // Candles occupy the top (1 - volume_ratio) of the pane; volume is below.
        let candle_css_h = ph * self.engine.viewport.candle_height_frac();
        let price = self.engine.viewport.pixel_to_price(y, candle_css_h);

        let mut is_drawing_drag = false;
        let mut hover_cursor: Option<&'static str> = None;

        // Drawing tool: update preview or drag
        {
            let drawings = &mut self.engine.drawings;
            if drawings.is_creating() {
                drawings.update_creation_preview(bar, price);
                // Still fall through so crosshair updates
            } else if let Some(id) = drawings.selected_id {
                if matches!(
                    drawings.get(id).map(|d| d.state()),
                    Some(raycore::core::drawings::types::DrawingState::Dragging { .. })
                ) {
                    drawings.update_drag(id, bar, price);
                    is_drawing_drag = true;
                }
            }

            // Hover hit-test for cursor feedback (not during drag/creation, no tool active)
            if !is_drawing_drag
                && !drawings.is_creating()
                && drawings.active_tool == raycore::DrawingTool::None
            {
                if let Some((hit_id, result)) =
                    drawings.hit_test(x, y, &self.engine.viewport, pw, ph)
                {
                    use raycore::core::drawings::types::cursor_for_drawing_hit;
                    let tool = drawings
                        .get(hit_id)
                        .map(|d| d.tool())
                        .unwrap_or(raycore::DrawingTool::None);
                    hover_cursor = Some(cursor_for_drawing_hit(tool, result.part, None));
                }
            }
        }

        // Update hover cursor only when not in a drawing drag
        if !self.interaction.drawing_drag_active {
            self.interaction.set_drawing_cursor(hover_cursor);
        }

        if is_drawing_drag {
            // Suppress crosshair while dragging a drawing
            self.engine.crosshair.active = false;
            return; // don't move chart while dragging drawing
        }

        let Self {
            interaction,
            engine,
            ..
        } = self;
        interaction.pane_pointer_move(
            x,
            y,
            pw,
            ph,
            &mut engine.viewport,
            &mut engine.crosshair,
            &engine.bars,
            dpr,
        );
    }

    pub fn on_pointer_down(&mut self, x: f64, y: f64, zone: HitZone) {
        let (pw, ph) = self.layout.pane_css_size();

        if zone == HitZone::Chart {
            let bar = self.engine.viewport.pixel_to_bar(x, pw);
            // Use candle area height — consistent with point_to_css / price_to_css_y.
            let candle_css_h = ph * self.engine.viewport.candle_height_frac();
            let price = self.engine.viewport.pixel_to_price(y, candle_css_h);

            let mut should_return = false;
            let mut drag_cursor: Option<&'static str> = None;

            {
                let drawings = &mut self.engine.drawings;

                if drawings.is_tool_active() {
                    // Start creating a new drawing
                    if !drawings.is_creating() {
                        drawings.start_creating(bar, price);
                    } else {
                        // Multi-step tools: place next anchor on click
                        drawings.finalize_creation_step(bar, price);
                    }
                    should_return = true;
                } else {
                    // No tool active: check if user clicked on an existing drawing
                    let hit = drawings.hit_test(x, y, &self.engine.viewport, pw, ph);
                    if let Some((id, result)) = hit {
                        use raycore::core::drawings::types::{cursor_for_drawing_hit, HitPart};
                        let tool = drawings
                            .get(id)
                            .map(|d| d.tool())
                            .unwrap_or(raycore::DrawingTool::None);
                        let anchor_idx = match result.part {
                            HitPart::Anchor(i) => Some(i),
                            _ => None,
                        };

                        // Rectangle: body clicks select only and fall through to
                        // chart pan. Edge clicks move the whole rectangle.
                        // Anchor clicks resize (move single anchor).
                        if tool == raycore::DrawingTool::Rectangle && result.part == HitPart::Body {
                            drawings.select(id);
                            // Don't start drag — fall through to chart pan
                        } else {
                            drawings.select(id);
                            drawings.start_drag(id, anchor_idx, bar, price);
                            drag_cursor =
                                Some(cursor_for_drawing_hit(tool, result.part, anchor_idx));
                            should_return = true;
                        }
                    } else {
                        // Click on empty space: deselect
                        drawings.deselect_all();
                    }
                }
            }

            if should_return {
                if let Some(cursor) = drag_cursor {
                    self.interaction.drawing_drag_active = true;
                    self.interaction.set_drawing_cursor(Some(cursor));
                }
                return; // don't pan while drawing tool / drawing drag
            }
        }

        let Self {
            interaction,
            engine,
            ..
        } = self;
        interaction.pointer_down(x, y, zone, &engine.viewport, ph);
    }

    pub fn on_pointer_up(&mut self) {
        let now_ms = js_sys::Date::now();
        let (pw, ph) = self.layout.pane_css_size();

        // If a drawing was being created (drag-to-create: release = place second anchor)
        {
            let drawings = &mut self.engine.drawings;
            if drawings.is_creating() {
                // Read the preview anchor position first (immutable borrow scope)
                let anchor_pos: Option<(f64, f64)> = {
                    drawings
                        .all()
                        .iter()
                        .find(|d| {
                            matches!(
                                d.state(),
                                raycore::core::drawings::types::DrawingState::Creating { .. }
                            )
                        })
                        .and_then(|d| {
                            let anchors = d.anchors();
                            if anchors.len() >= 2 {
                                Some((anchors[1].point.bar_index, anchors[1].point.price))
                            } else if !anchors.is_empty() {
                                Some((anchors[0].point.bar_index, anchors[0].point.price))
                            } else {
                                None
                            }
                        })
                };
                // Now finalize with the stored position (mutable borrow)
                if let Some((bar, price)) = anchor_pos {
                    drawings.finalize_creation_step(bar, price);
                }
                return;
            }
        }

        // End any drawing drag
        let mut ended_drag = false;
        {
            let drawings = &mut self.engine.drawings;
            if let Some(id) = drawings.selected_id {
                if matches!(
                    drawings.get(id).map(|d| d.state()),
                    Some(raycore::core::drawings::types::DrawingState::Dragging { .. })
                ) {
                    drawings.end_drag(id);
                    ended_drag = true;
                }
            }
        }
        if ended_drag {
            self.interaction.drawing_drag_active = false;
            self.interaction.set_drawing_cursor(None);
            // Restore crosshair for mouse (touch handled by tracking mode)
            if !self.interaction.is_touch {
                self.engine.crosshair.active = true;
            }
            return;
        }

        let _ = (pw, ph); // suppress unused warning
        let Self {
            interaction,
            engine,
            ..
        } = self;
        interaction.pointer_up(&mut engine.viewport, &engine.bars, now_ms);
    }

    pub fn on_pane_wheel(&mut self, x: f64, dx: f64, dy: f64, dm: u32) {
        let (pw, _) = self.layout.pane_css_size();
        let Self {
            interaction,
            engine,
            ..
        } = self;
        interaction.pane_wheel(x, dx, dy, dm, pw, &mut engine.viewport, &engine.bars);
    }

    pub fn on_price_axis_move(&mut self, y: f64) {
        let (_, ph) = self.layout.pane_css_size();
        let Self {
            interaction,
            engine,
            ..
        } = self;
        interaction.price_axis_pointer_move(y, ph, &mut engine.viewport);
    }

    pub fn on_price_axis_wheel(&mut self, dy: f64, dm: u32) {
        let Self {
            interaction,
            engine,
            ..
        } = self;
        interaction.price_axis_wheel(dy, dm, &mut engine.viewport);
    }

    pub fn on_time_axis_move(&mut self, x: f64) {
        let (pw, _) = self.layout.pane_css_size();
        let Self {
            interaction,
            engine,
            ..
        } = self;
        interaction.time_axis_pointer_move(x, pw, &mut engine.viewport, &engine.bars);
    }

    pub fn on_time_axis_wheel(&mut self, x: f64, dy: f64, dm: u32) {
        let (pw, _) = self.layout.pane_css_size();
        let Self {
            interaction,
            engine,
            ..
        } = self;
        interaction.time_axis_wheel(x, dy, dm, pw, &mut engine.viewport, &engine.bars);
    }

    pub fn on_pinch_start(&mut self, cx: f64, cy: f64, distance: f64) {
        let (pw, _) = self.layout.pane_css_size();
        let Self {
            interaction,
            engine,
            ..
        } = self;
        interaction.pinch_start(cx, cy, distance, pw, &engine.viewport);
    }

    pub fn on_pinch_update(&mut self, scale: f64) {
        let Self {
            interaction,
            engine,
            ..
        } = self;
        interaction.pinch_update(scale, &mut engine.viewport, &engine.bars);
    }

    pub fn on_pinch_end(&mut self) {
        self.interaction.pinch_end();
    }

    pub fn on_long_press(&mut self, x: f64, y: f64) {
        let Self {
            interaction,
            engine,
            ..
        } = self;
        interaction.long_press(x, y, &mut engine.crosshair);
    }

    pub fn on_touch_double_tap(&mut self) {
        let Self {
            interaction,
            engine,
            ..
        } = self;
        interaction.touch_double_tap(&mut engine.crosshair, &mut engine.viewport, &engine.bars);
    }

    pub fn cursor_css(&self) -> &'static str {
        self.interaction.cursor_hint()
    }
}

/// Shared inner state type alias.
pub type SharedInner = Rc<RefCell<ChartInner>>;

/// Helper: get CSS coords from PointerEvent relative to an element.
pub fn event_css_pos(e: &web_sys::PointerEvent, el: &web_sys::Element) -> (f64, f64) {
    let rect = el.get_bounding_client_rect();
    (
        e.client_x() as f64 - rect.left(),
        e.client_y() as f64 - rect.top(),
    )
}

/// Helper: get CSS coords from WheelEvent relative to an element.
pub fn wheel_css_pos(e: &web_sys::WheelEvent, el: &web_sys::Element) -> (f64, f64) {
    let rect = el.get_bounding_client_rect();
    (
        e.client_x() as f64 - rect.left(),
        e.client_y() as f64 - rect.top(),
    )
}
