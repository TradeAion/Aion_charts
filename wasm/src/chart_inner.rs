//! Internal chart state and helper methods.
//!
//! This module contains `ChartInner`, the internal state shared between
//! event closures and the public RayCore API. Helper methods here handle
//! the borrow checker dance of destructuring to access multiple fields.
#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use raycore::{
    Bar, ChartEngine, HitZone, InteractionHandler, MainChartType, OverlayRenderer,
    PriceAxisRenderer, TimeAxisRenderer,
};

use crate::canvas_manager::WidgetLayout;
use crate::subpane::{PaneHeightCoordinator, SubPane, SubPaneSeparatorStyle};

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
}

/// Internal chart state shared between event closures and the public API.
pub struct ChartInner {
    /// Requested renderer mode from create-time options (`auto`/`webgpu`/`canvas2d`).
    pub requested_renderer_mode: String,
    /// Actual active backend name (`webgpu` or `canvas2d`).
    pub active_renderer_name: String,
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
    /// Visual/interaction style for indicator sub-pane separators.
    pub subpane_separator_style: SubPaneSeparatorStyle,
    /// Replay mode active flag.
    pub replay_active: bool,
    /// Replay trim editing mode (click-to-set cutoff).
    pub replay_trim_edit_mode: bool,
    /// Replay playback running flag.
    pub replay_playing: bool,
    /// Right-edge trim cutoff index (inclusive) in `replay_archive`.
    pub replay_cutoff_index: Option<usize>,
    /// Full timeline snapshot/buffer while replay mode is active.
    pub replay_archive: Vec<Bar>,
    /// Playback speed in bars/second.
    pub replay_speed_bps: f64,
    /// Replay edge handling policy.
    pub replay_edge_behavior: ReplayEdgeBehavior,
    /// Last playback tick timestamp in milliseconds.
    pub replay_last_tick_ms: f64,
    /// Fractional bar accumulator for frame-based playback.
    pub replay_tick_accum_bars: f64,
    /// Symbol name (e.g. "BTCUSD") — used by the asset-name chip overlay.
    pub symbol: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayEdgeBehavior {
    AutoPause,
    LiveContinue,
    AutoExit,
}

impl Default for ReplayEdgeBehavior {
    fn default() -> Self {
        Self::AutoPause
    }
}

impl ReplayEdgeBehavior {
    pub fn from_key(value: &str) -> Option<Self> {
        match value {
            "auto_pause" => Some(Self::AutoPause),
            "live_continue" => Some(Self::LiveContinue),
            "auto_exit" => Some(Self::AutoExit),
            _ => None,
        }
    }

    pub fn as_key(self) -> &'static str {
        match self {
            Self::AutoPause => "auto_pause",
            Self::LiveContinue => "live_continue",
            Self::AutoExit => "auto_exit",
        }
    }
}

/// Helper methods that destructure `self` to satisfy the borrow checker.
/// Each method borrows `interaction` and `engine` fields separately.
impl ChartInner {
    /// Resolve a Ctrl+OHLC snap target at the current pointer position.
    ///
    /// Uses the nearest valid bar index (clamped to data bounds) so snapping
    /// never resolves to empty/future space when data exists.
    fn resolve_ohlc_snap_target(
        &self,
        x_css: f64,
        y_css: f64,
        pane_css_w: f64,
        pane_css_h: f64,
    ) -> Option<OhlcSnapTarget> {
        let bars_len = self.engine.bars.len();
        if bars_len == 0 || pane_css_w <= 0.0 || pane_css_h <= 0.0 {
            return None;
        }

        let bar_idx = self
            .engine
            .viewport
            .bar_index_at_pixel(x_css, pane_css_w, bars_len)
            .unwrap_or_else(|| {
                let raw = self.engine.viewport.pixel_to_bar(x_css, pane_css_w).floor();
                raw.clamp(0.0, (bars_len - 1) as f64) as usize
            });

        let snap_price = snap_to_ohlc_price(
            &self.engine.bars,
            bar_idx,
            y_css.clamp(0.0, pane_css_h),
            &self.engine.viewport,
            pane_css_h,
        );
        let snap_x_css = self.engine.viewport.bar_center_css(bar_idx, pane_css_w);
        let snap_y_css = self
            .engine
            .viewport
            .price_to_css_y(snap_price, pane_css_h)
            .clamp(0.0, pane_css_h);

        Some(OhlcSnapTarget {
            bar_idx,
            bar: bar_idx as f64 + 0.5,
            price: snap_price,
            x_css: snap_x_css,
            y_css: snap_y_css,
        })
    }

    fn replay_last_timestamp(&self) -> Option<u64> {
        self.replay_archive.last().map(|bar| bar.timestamp)
    }

    fn replay_reset_tick_clock(&mut self) {
        self.replay_last_tick_ms = 0.0;
        self.replay_tick_accum_bars = 0.0;
    }

    fn replay_pause_playback(&mut self) {
        self.replay_playing = false;
        self.replay_reset_tick_clock();
    }

    pub fn replay_set_trim_edit_mode(&mut self, enabled: bool) {
        self.replay_trim_edit_mode = enabled;
        if enabled {
            self.engine.drawings.clear_hovered();
        }
        self.interaction
            .set_replay_chart_trim_mode(self.replay_active && enabled);
    }

    fn replay_latest_cutoff(&self) -> Option<usize> {
        self.replay_archive.len().checked_sub(1)
    }

    fn replay_snapshot_engine_bars(&self) -> Vec<Bar> {
        (0..self.engine.bars.len())
            .filter_map(|idx| self.engine.bars.get(idx))
            .collect()
    }

    fn replay_apply_bars_preserve_viewport(&mut self, bars: Vec<Bar>) {
        let vp_start = self.engine.viewport.start_bar;
        let vp_end = self.engine.viewport.end_bar;
        let vp_price_min = self.engine.viewport.price_min;
        let vp_price_max = self.engine.viewport.price_max;
        let vp_price_locked = self.engine.viewport.price_locked;
        let vp_auto_scroll = self.engine.viewport.auto_scroll;
        let crosshair_bar_index = self.engine.crosshair.bar_index;

        self.engine.bars.set(bars);
        self.engine.studies.update_studies(&self.engine.bars);
        self.engine.indicators.on_set_data(&self.engine.bars);

        self.engine.viewport.start_bar = vp_start;
        self.engine.viewport.end_bar = vp_end;
        self.engine.viewport.price_locked = vp_price_locked;
        self.engine.viewport.auto_scroll = vp_auto_scroll;
        self.engine.viewport.clamp_to_data(self.engine.bars.len());

        if self.engine.viewport.price_locked {
            self.engine.viewport.price_min = vp_price_min;
            self.engine.viewport.price_max = vp_price_max;
        } else {
            self.engine.viewport.auto_fit_price(&self.engine.bars);
        }

        self.engine.crosshair.bar_index =
            crosshair_bar_index.filter(|&idx| idx < self.engine.bars.len());
    }

    fn replay_apply_cutoff_to_engine(&mut self) -> Result<(), String> {
        if self.replay_archive.is_empty() || self.replay_cutoff_index.is_none() {
            self.replay_apply_bars_preserve_viewport(Vec::new());
            return Ok(());
        }

        let max_idx = self.replay_archive.len() - 1;
        let cutoff = self.replay_cutoff_index.unwrap_or(max_idx).min(max_idx);
        self.replay_cutoff_index = Some(cutoff);
        self.replay_apply_bars_preserve_viewport(self.replay_archive[..=cutoff].to_vec());
        Ok(())
    }

    fn replay_validate_append_timestamp(
        op: &str,
        last: Option<u64>,
        ts: u64,
    ) -> Result<(), String> {
        if let Some(last_ts) = last {
            if ts <= last_ts {
                return Err(format!(
                    "{op} requires timestamp > last timestamp ({ts} <= {last_ts})"
                ));
            }
        }
        Ok(())
    }

    fn replay_validate_update_timestamp(
        op: &str,
        last: Option<u64>,
        ts: u64,
    ) -> Result<(), String> {
        let last_ts = last.ok_or_else(|| format!("{op} cannot update an empty series"))?;
        if ts != last_ts {
            return Err(format!(
                "{op} requires timestamp == last timestamp ({ts} != {last_ts})"
            ));
        }
        Ok(())
    }

    fn replay_apply_edge_behavior(&mut self) -> Result<(), String> {
        match self.replay_edge_behavior {
            ReplayEdgeBehavior::AutoPause => {
                self.replay_pause_playback();
                Ok(())
            }
            ReplayEdgeBehavior::LiveContinue => {
                self.replay_tick_accum_bars = 0.0;
                Ok(())
            }
            ReplayEdgeBehavior::AutoExit => self.replay_exit(),
        }
    }

    pub fn replay_enter(&mut self) -> Result<(), String> {
        if self.replay_active {
            return Ok(());
        }

        self.replay_archive = self.replay_snapshot_engine_bars();
        self.replay_cutoff_index = self.replay_latest_cutoff();
        self.replay_active = true;
        self.replay_trim_edit_mode = true;
        self.replay_pause_playback();
        self.replay_set_trim_edit_mode(true);
        self.interaction.pressed = false;
        self.interaction.drag_active = false;
        self.interaction.drawing_drag_active = false;
        self.interaction.set_drawing_cursor(None);
        self.engine.drawings.clear_hovered();

        if self.engine.drawings.is_creating() {
            self.engine.drawings.cancel_creation();
        }
        if let Some(id) = self.engine.drawings.selected_id {
            if matches!(
                self.engine.drawings.get(id).map(|d| d.state()),
                Some(raycore::core::drawings::types::DrawingState::Dragging { .. })
            ) {
                self.engine.drawings.end_drag(id);
            }
        }
        self.engine.drawings.active_tool = raycore::DrawingTool::None;
        Ok(())
    }

    pub fn replay_exit(&mut self) -> Result<(), String> {
        if !self.replay_active {
            return Ok(());
        }

        self.engine.set_data(self.replay_archive.clone())?;
        self.replay_active = false;
        self.replay_trim_edit_mode = false;
        self.replay_playing = false;
        self.replay_cutoff_index = None;
        self.replay_archive.clear();
        self.replay_reset_tick_clock();
        self.replay_set_trim_edit_mode(false);
        self.engine.drawings.clear_hovered();
        self.interaction.set_drawing_cursor(None);
        Ok(())
    }

    pub fn replay_replace_archive_from_data(&mut self, bars: Vec<Bar>) -> Result<(), String> {
        self.engine.set_data(bars.clone())?;
        self.replay_archive = bars;
        self.replay_cutoff_index = self.replay_latest_cutoff();
        self.replay_pause_playback();
        Ok(())
    }

    pub fn replay_set_playing(&mut self, playing: bool) {
        if !self.replay_active {
            self.replay_pause_playback();
            return;
        }
        if playing {
            if self.replay_archive.is_empty() || self.replay_cutoff_index.is_none() {
                self.replay_pause_playback();
                return;
            }
            if self.replay_apply_cutoff_to_engine().is_err() {
                self.replay_pause_playback();
                return;
            }
            // Leaving trim-edit mode while playback runs.
            self.replay_set_trim_edit_mode(false);
            self.replay_playing = true;
            self.replay_last_tick_ms = 0.0;
        } else {
            self.replay_pause_playback();
        }
    }

    pub fn replay_cutoff_from_pane_x(&self, x_css: f64) -> Option<usize> {
        if !self.replay_active || self.replay_archive.is_empty() {
            return None;
        }
        let (pane_w, _) = self.layout.pane_css_size();
        if pane_w <= 0.0 {
            return None;
        }
        self.engine
            .viewport
            .bar_index_at_pixel(x_css, pane_w, self.replay_archive.len())
    }

    pub fn replay_set_cutoff_bar(&mut self, index: usize) -> Result<(), String> {
        if !self.replay_active {
            return Ok(());
        }
        let Some(max_idx) = self.replay_latest_cutoff() else {
            self.replay_cutoff_index = None;
            self.replay_pause_playback();
            return self.engine.set_data(Vec::new());
        };

        self.replay_cutoff_index = Some(index.min(max_idx));
        let result = self.replay_apply_cutoff_to_engine();
        if result.is_ok() {
            // Any explicit trim action exits trim-edit mode.
            self.replay_set_trim_edit_mode(false);
        }
        result
    }

    pub fn replay_step_back(&mut self) -> Result<(), String> {
        if !self.replay_active || self.replay_archive.is_empty() {
            return Ok(());
        }
        let max_idx = self.replay_archive.len() - 1;
        let current = self.replay_cutoff_index.unwrap_or(max_idx).min(max_idx);
        self.replay_pause_playback();
        self.replay_set_cutoff_bar(current.saturating_sub(1))
    }

    pub fn replay_step_forward(&mut self) -> Result<(), String> {
        if !self.replay_active || self.replay_archive.is_empty() {
            return Ok(());
        }
        self.replay_pause_playback();

        let max_idx = self.replay_archive.len() - 1;
        let current = self.replay_cutoff_index.unwrap_or(0).min(max_idx);
        if current < max_idx {
            self.replay_set_cutoff_bar(current + 1)
        } else {
            self.replay_apply_edge_behavior()
        }
    }

    pub fn replay_tick(&mut self, now_ms: f64) -> Result<(), String> {
        if !self.replay_active || !self.replay_playing {
            return Ok(());
        }
        if self.replay_archive.is_empty() {
            self.replay_pause_playback();
            return Ok(());
        }

        let max_idx = self.replay_archive.len() - 1;
        if self.replay_cutoff_index.is_none() {
            self.replay_cutoff_index = Some(0);
            self.replay_apply_cutoff_to_engine()?;
        }
        let current = self.replay_cutoff_index.unwrap_or(0).min(max_idx);
        if current >= max_idx {
            return self.replay_apply_edge_behavior();
        }

        if self.replay_last_tick_ms <= 0.0 {
            self.replay_last_tick_ms = now_ms;
            return Ok(());
        }
        let dt_sec = ((now_ms - self.replay_last_tick_ms) / 1000.0).max(0.0);
        self.replay_last_tick_ms = now_ms;
        if dt_sec <= 0.0 {
            return Ok(());
        }

        let speed = if self.replay_speed_bps.is_finite() && self.replay_speed_bps > 0.0 {
            self.replay_speed_bps
        } else {
            1.0
        };
        self.replay_tick_accum_bars += dt_sec * speed;
        let advance = self.replay_tick_accum_bars.floor() as usize;
        if advance == 0 {
            return Ok(());
        }
        self.replay_tick_accum_bars -= advance as f64;

        let target = current.saturating_add(advance).min(max_idx);
        self.replay_set_cutoff_bar(target)?;
        if target >= max_idx {
            return self.replay_apply_edge_behavior();
        }
        Ok(())
    }

    pub fn replay_buffer_append_bar(&mut self, bar: Bar) -> Result<(), String> {
        Self::replay_validate_append_timestamp(
            "append_bar",
            self.replay_last_timestamp(),
            bar.timestamp,
        )?;
        self.replay_archive.push(bar);
        if self.replay_cutoff_index.is_none() {
            self.replay_cutoff_index = Some(0);
        }
        Ok(())
    }

    pub fn replay_buffer_update_last_bar(&mut self, bar: Bar) -> Result<(), String> {
        Self::replay_validate_update_timestamp(
            "update_last_bar",
            self.replay_last_timestamp(),
            bar.timestamp,
        )?;
        if let Some(last) = self.replay_archive.last_mut() {
            *last = bar;
        }
        Ok(())
    }

    pub fn replay_buffer_upsert_bar(&mut self, bar: Bar) -> Result<(), String> {
        match self.replay_last_timestamp() {
            None => {
                self.replay_archive.push(bar);
                self.replay_cutoff_index = Some(0);
                Ok(())
            }
            Some(last_ts) if bar.timestamp == last_ts => self.replay_buffer_update_last_bar(bar),
            Some(last_ts) if bar.timestamp > last_ts => self.replay_buffer_append_bar(bar),
            Some(last_ts) => Err(format!(
                "upsert_bar requires timestamp >= last timestamp ({} < {})",
                bar.timestamp, last_ts
            )),
        }
    }

    pub fn replay_crosshair_over_empty_area(&self) -> bool {
        self.replay_active
            && self.engine.crosshair.active
            && self.engine.crosshair.bar_index.is_none()
    }

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
        if zone == HitZone::Chart {
            engine.drawings.clear_hovered();
            interaction.set_drawing_cursor(None);
        }
    }

    pub fn on_pane_pointer_move(
        &mut self,
        x: f64,
        y: f64,
        shift_pressed: bool,
        ctrl_pressed: bool,
    ) {
        let (pw, ph) = self.layout.pane_css_size();
        let dpr = self.engine.dpr;

        // Pre-compute logical coords from viewport (before any mutable drawing borrow)
        let mut bar = self.engine.viewport.pixel_to_bar(x, pw);
        // Use candle area height for drawing coordinates — matches price_to_css_y().
        // Candles occupy the top (1 - volume_ratio) of the pane; volume is below.
        let candle_css_h = ph * self.engine.viewport.candle_height_frac();
        let mut price = self.engine.viewport.pixel_to_price(y, candle_css_h);
        let ctrl_drawing_snap_active = ctrl_pressed
            && (self.engine.drawings.is_tool_active()
                || self.engine.drawings.is_creating()
                || self.engine.drawings.selected_id.is_some()
                || self.interaction.drawing_drag_active);
        let ctrl_snap_target = if ctrl_drawing_snap_active {
            self.resolve_ohlc_snap_target(x, y, pw, ph)
        } else {
            None
        };

        // Ctrl-key OHLC magnet snapping (works during drawing creation/drag)
        if let Some(snap) = ctrl_snap_target {
            price = snap.price;
            bar = snap.bar;
        }

        let mut is_drawing_drag = false;
        let mut hover_cursor: Option<&'static str> = None;

        // Drawing tool: update preview or drag
        {
            let drawings = &mut self.engine.drawings;
            if drawings.is_creating() {
                // Shift-key angle snapping (45° increments) for line-based tools.
                // Ctrl+OHLC snap takes precedence over angle snap.
                let (final_bar, final_price) = if shift_pressed && !ctrl_drawing_snap_active {
                    // Only apply angle snap for line-based tools (not Brush, HLine, VLine)
                    let tool = drawings.creation_tool();
                    let should_snap = matches!(
                        tool,
                        Some(raycore::DrawingTool::TrendLine)
                            | Some(raycore::DrawingTool::Ray)
                            | Some(raycore::DrawingTool::Fibonacci)
                            | Some(raycore::DrawingTool::Scale)
                            | Some(raycore::DrawingTool::Rectangle)
                    );
                    if should_snap {
                        if let Some((anchor_bar, anchor_price)) = drawings.creation_first_anchor() {
                            snap_to_angle_45(anchor_bar, anchor_price, bar, price, pw, candle_css_h)
                        } else {
                            (bar, price)
                        }
                    } else {
                        (bar, price)
                    }
                } else {
                    (bar, price)
                };
                drawings.update_creation_preview(final_bar, final_price);
                // Still fall through so crosshair updates
            } else if let Some(id) = drawings.selected_id {
                if matches!(
                    drawings.get(id).map(|d| d.state()),
                    Some(raycore::core::drawings::types::DrawingState::Dragging { .. })
                ) {
                    // Apply angle snapping during anchor drag too.
                    // Ctrl+OHLC snap takes precedence over angle snap.
                    let (final_bar, final_price) = if shift_pressed && !ctrl_drawing_snap_active {
                        let tool = drawings.tool_of(id);
                        let should_snap = matches!(
                            tool,
                            Some(raycore::DrawingTool::TrendLine)
                                | Some(raycore::DrawingTool::Ray)
                                | Some(raycore::DrawingTool::Fibonacci)
                                | Some(raycore::DrawingTool::Scale)
                                | Some(raycore::DrawingTool::Rectangle)
                        );
                        if should_snap {
                            if let Some((anchor_bar, anchor_price)) =
                                drawings.drag_opposite_anchor(id)
                            {
                                snap_to_angle_45(
                                    anchor_bar,
                                    anchor_price,
                                    bar,
                                    price,
                                    pw,
                                    candle_css_h,
                                )
                            } else {
                                (bar, price)
                            }
                        } else {
                            (bar, price)
                        }
                    } else {
                        (bar, price)
                    };
                    drawings.update_drag(id, final_bar, final_price);
                    is_drawing_drag = true;
                }
            }

            // Hover hit-test for cursor feedback (not during drag/creation, no tool active)
            if !is_drawing_drag
                && !drawings.is_creating()
                && drawings.active_tool == raycore::DrawingTool::None
                && !(self.replay_active && self.replay_trim_edit_mode)
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
                    drawings.set_hovered(Some(hit_id));
                } else {
                    drawings.clear_hovered();
                }
            } else {
                drawings.clear_hovered();
            }
        }

        // Update hover cursor only when not in a drawing drag
        if !self.interaction.drawing_drag_active {
            self.interaction.set_drawing_cursor(hover_cursor);
        }

        if is_drawing_drag {
            // Show snap target while Ctrl-ohlc snapping during drag so users can
            // see exactly where the anchor will land.
            if let Some(snap) = ctrl_snap_target {
                self.engine.crosshair.active = true;
                self.engine.crosshair.x = snap.x_css;
                self.engine.crosshair.y = snap.y_css;
                self.engine.crosshair.bar_index = Some(snap.bar_idx);
                self.engine.crosshair.price = snap.price;
            } else {
                // Existing behavior outside Ctrl-snap drag.
                self.engine.crosshair.active = false;
            }
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

        if let Some(snap) = ctrl_snap_target {
            // Keep visual crosshair locked to the same OHLC point used by drawing snap.
            engine.crosshair.active = true;
            engine.crosshair.x = snap.x_css;
            engine.crosshair.y = snap.y_css;
            engine.crosshair.bar_index = Some(snap.bar_idx);
            engine.crosshair.price = snap.price;
        }

        // Emit crosshair move event
        if engine.crosshair.active {
            let bar_idx = engine.crosshair.bar_index;
            let timestamp = bar_idx.and_then(|idx| engine.bars.get(idx).map(|b| b.timestamp));
            engine.event_bus.emit(raycore::ChartEvent::CrosshairMove {
                x,
                y,
                bar_index: bar_idx,
                price: engine.crosshair.price,
                timestamp,
            });
        }
    }

    pub fn on_pointer_down(
        &mut self,
        x: f64,
        y: f64,
        zone: HitZone,
        _shift_pressed: bool,
        ctrl_pressed: bool,
    ) {
        let (pw, ph) = self.layout.pane_css_size();

        if zone == HitZone::Chart {
            let mut bar = self.engine.viewport.pixel_to_bar(x, pw);
            // Use candle area height — consistent with point_to_css / price_to_css_y.
            let candle_css_h = ph * self.engine.viewport.candle_height_frac();
            let mut price = self.engine.viewport.pixel_to_price(y, candle_css_h);
            let ctrl_drawing_snap_active = ctrl_pressed
                && (self.engine.drawings.is_tool_active()
                    || self.engine.drawings.is_creating()
                    || self.engine.drawings.selected_id.is_some());
            if let Some(snap) = ctrl_drawing_snap_active
                .then(|| self.resolve_ohlc_snap_target(x, y, pw, ph))
                .flatten()
            {
                bar = snap.bar;
                price = snap.price;
            }

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

                        // Rectangle: body clicks select only and fall through to chart pan.
                        // Edges are hit-tested as side anchors (TM/RM/BM/LM), so
                        // edge drags resize and can flip while opposite side stays fixed.
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
                self.engine.stamp_drawing_timestamps();
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
                self.engine.stamp_drawing_timestamps();
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
            self.engine.stamp_drawing_timestamps();
            self.interaction.drawing_drag_active = false;
            self.interaction.set_drawing_cursor(None);
            // Restore crosshair for mouse (touch handled by tracking mode)
            if !self.interaction.is_touch {
                self.engine.crosshair.active = true;
            }
            return;
        }

        let _ = (pw, ph); // suppress unused warning
                          // Capture click state BEFORE pointer_up clears it
        let was_pressed = self.interaction.pressed;
        let was_drag = self.interaction.drag_active;
        let click_x = self.interaction.last_move_x;
        let click_y = self.interaction.last_move_y;

        let Self {
            interaction,
            engine,
            ..
        } = self;
        interaction.pointer_up(&mut engine.viewport, &engine.bars, now_ms);

        // Emit click event if it was a tap/click (pressed, not a drag)
        if was_pressed && !was_drag {
            let candle_css_h = ph * engine.viewport.candle_height_frac();
            let price = engine.viewport.pixel_to_price(click_y, candle_css_h);
            let bar_f = engine.viewport.pixel_to_bar(click_x, pw);
            let bar_index = if bar_f >= 0.0 && (bar_f.round() as usize) < engine.bars.len() {
                Some(bar_f.round() as usize)
            } else {
                None
            };
            engine.event_bus.emit(raycore::ChartEvent::Click {
                x: click_x,
                y: click_y,
                bar_index,
                price,
            });
        }
    }

    pub fn on_pane_wheel(&mut self, x: f64, y: f64, dx: f64, dy: f64, dm: u32) {
        let (pw, ph) = self.layout.pane_css_size();
        let Self {
            interaction,
            engine,
            ..
        } = self;
        let zoom_price_with_time = engine.main_chart_type == MainChartType::Footprint
            && engine.main_chart_options.footprint.zoom_price_with_time;
        interaction.pane_wheel(
            x,
            y,
            dx,
            dy,
            dm,
            pw,
            ph,
            zoom_price_with_time,
            &mut engine.viewport,
            &engine.bars,
        );

        // Emit visible range change after zoom/pan
        engine
            .event_bus
            .emit(raycore::ChartEvent::VisibleRangeChange {
                start_bar: engine.viewport.start_bar,
                end_bar: engine.viewport.end_bar,
            });
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
        let (pw, ph) = self.layout.pane_css_size();
        let Self {
            interaction,
            engine,
            ..
        } = self;
        interaction.time_axis_wheel(x, dy, dm, pw, ph, &mut engine.viewport, &engine.bars);
    }

    pub fn on_pinch_start(&mut self, cx: f64, cy: f64, distance: f64) {
        let (pw, ph) = self.layout.pane_css_size();
        let Self {
            interaction,
            engine,
            ..
        } = self;
        let zoom_price_with_time = engine.main_chart_type == MainChartType::Footprint
            && engine.main_chart_options.footprint.zoom_price_with_time;
        interaction.pinch_start(
            cx,
            cy,
            distance,
            pw,
            ph,
            zoom_price_with_time,
            &engine.viewport,
        );
    }

    pub fn on_pinch_update(&mut self, scale: f64) {
        let Self {
            interaction,
            engine,
            ..
        } = self;
        let zoom_price_with_time = engine.main_chart_type == MainChartType::Footprint
            && engine.main_chart_options.footprint.zoom_price_with_time;
        interaction.pinch_update(
            scale,
            zoom_price_with_time,
            &mut engine.viewport,
            &engine.bars,
        );
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

// ═══════════════════════════════════════════════════════════════════════════════
// Drawing Snap Helpers
// ═══════════════════════════════════════════════════════════════════════════════

use std::f64::consts::PI;

#[derive(Debug, Clone, Copy)]
struct OhlcSnapTarget {
    bar_idx: usize,
    bar: f64,
    price: f64,
    x_css: f64,
    y_css: f64,
}

/// Snap endpoint to nearest 45° angle from anchor point.
///
/// Works in screen-space (CSS pixels) to ensure visual angles are correct,
/// then converts back to logical bar/price coordinates.
///
/// Snaps to: 0°, 45°, 90°, 135°, 180°, 225°, 270°, 315° (8 directions)
fn snap_to_angle_45(
    anchor_bar: f64,
    anchor_price: f64,
    target_bar: f64,
    target_price: f64,
    pane_css_w: f64,
    candle_css_h: f64,
) -> (f64, f64) {
    // We need to work in a normalized coordinate space where 1 unit in X
    // equals 1 unit in Y visually. Otherwise angles would be skewed.
    // Use the pane aspect ratio to normalize.

    let bar_range = 100.0; // Approximate visible bars (scale factor, cancels out)
    let price_range = 1000.0; // Approximate visible price range (cancels out)

    // Compute aspect ratio: pixels per bar vs pixels per price unit
    let px_per_bar = pane_css_w / bar_range;
    let px_per_price = candle_css_h / price_range;

    // Convert to normalized screen space
    let dx_screen = (target_bar - anchor_bar) * px_per_bar;
    let dy_screen = (anchor_price - target_price) * px_per_price; // Y inverted in screen space

    // Calculate angle and snap to nearest 45°
    let angle = dy_screen.atan2(dx_screen);
    let snapped_angle = (angle / (PI / 4.0)).round() * (PI / 4.0);

    // Preserve distance in screen space
    let distance = (dx_screen * dx_screen + dy_screen * dy_screen).sqrt();

    // Compute snapped screen deltas
    let snapped_dx_screen = distance * snapped_angle.cos();
    let snapped_dy_screen = distance * snapped_angle.sin();

    // Convert back to bar/price coordinates
    let snapped_bar = anchor_bar + snapped_dx_screen / px_per_bar;
    let snapped_price = anchor_price - snapped_dy_screen / px_per_price; // Y inverted

    (snapped_bar, snapped_price)
}

/// Snap to nearest OHLC price for a given bar index.
///
/// Finds the O/H/L/C value whose CSS Y is closest to the cursor Y position.
fn snap_to_ohlc_price(
    bars: &raycore::BarArray,
    bar_idx: usize,
    cursor_css_y: f64,
    viewport: &raycore::Viewport,
    pane_css_h: f64,
) -> f64 {
    let open = bars.open(bar_idx) as f64;
    let high = bars.high(bar_idx) as f64;
    let low = bars.low(bar_idx) as f64;
    let close = bars.close(bar_idx) as f64;

    // Convert each OHLC price to CSS Y and find nearest to cursor
    let candidates = [open, high, low, close];
    let mut best_price = close;
    let mut best_dist = f64::MAX;
    for &price in &candidates {
        let py = viewport.price_to_css_y(price, pane_css_h);
        let dist = (py - cursor_css_y).abs();
        if dist < best_dist {
            best_dist = dist;
            best_price = price;
        }
    }
    best_price
}
