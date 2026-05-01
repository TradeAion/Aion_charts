//! InteractionHandler — compatibility-style pointer/wheel state machine with full touch support.
//!
//! Pure Rust, no DOM dependencies. The WASM layer forwards raw pointer events
//! WITH the zone already determined (since each widget is a separate DOM element).
//!
//! Architecture matches reference implementation:
//! - PaneWidget events → zone=Chart
//! - PriceAxisWidget events → zone=PriceAxis
//! - TimeAxisWidget events → zone=TimeAxis
//! - Each widget fires its own mouseEnter/Leave naturally
//!
//! Interaction model (matching reference implementation):
//! ── Pane ──
//!   wheel deltaY  → zoom time (proportional, focal-point aware)
//!   wheel deltaX  → scroll time
//!   drag          → scroll time + price
//!   pinch         → zoom X and Y
//!   long press    → activate crosshair tracking mode
//!   double tap    → zoom in / reset
//! ── Time Axis ──
//!   drag          → scale time (ratio from right edge, like reference implementation)
//!   wheel deltaY  → zoom time
//!   dbl-click     → reset time
//! ── Price Axis ──
//!   drag          → scale price (reference implementation inverted-Y formula)
//!   wheel deltaY  → zoom price
//!   dbl-click     → reset price

use crate::core::constants::{
    DEFAULT_INITIAL_VISIBLE_BARS, DOUBLE_CLICK_WINDOW_MS, KINETIC_FRICTION_COEFFICIENT,
    KINETIC_TRIGGER_WINDOW_MS, MAX_PRICE_SCALE_COEFF, MIN_GLIDE_VELOCITY, MIN_KINETIC_VELOCITY,
    MIN_PINCH_SCALE, MIN_PRICE_SCALE_COEFF, PHYSICS_FRAME_MS, PINCH_SCALE_MULTIPLIER,
    PRICE_SCALE_DRAG_SENSITIVITY, SCROLL_MULTIPLIER, TIME_AXIS_MAX_BAR_MULTIPLIER,
    TIME_AXIS_MIN_BARS, VELOCITY_SAMPLE_WINDOW_MS, VELOCITY_SMOOTHING_FACTOR,
    WHEEL_DELTA_LINE_MULTIPLIER, WHEEL_DELTA_PAGE_MULTIPLIER, WHEEL_SPEED_DIVISOR,
    ZOOM_FACTOR_DIVISOR,
};
use crate::core::data::BarArray;
use crate::core::renderer::traits::{CrosshairMode, CrosshairState};
use crate::core::renderer::value_projection::TimeScaleIndex;
use crate::core::viewport::Viewport;

/// Get current time in milliseconds (platform-agnostic).
#[cfg(target_arch = "wasm32")]
fn now_ms() -> f64 {
    js_sys::Date::now()
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

/// Manhattan distance threshold before drag starts (reference implementation: CancelClickManhattanDistance = 5).
const CANCEL_CLICK_DISTANCE: f64 = 5.0;
/// Manhattan distance threshold for mouse double-click detection.
const DOUBLE_CLICK_DISTANCE: f64 = 5.0;
/// Replay trim cursor: inline SVG scissors icon with crosshair fallback.
const REPLAY_SCISSORS_CURSOR: &str = "url(\"data:image/svg+xml,%3Csvg%20xmlns%3D%27http%3A//www.w3.org/2000/svg%27%20width%3D%2724%27%20height%3D%2724%27%20viewBox%3D%270%200%2024%2024%27%20fill%3D%27none%27%20stroke%3D%27%23d7d9dd%27%20stroke-width%3D%272%27%20stroke-linecap%3D%27round%27%20stroke-linejoin%3D%27round%27%3E%3Ccircle%20cx%3D%276%27%20cy%3D%276%27%20r%3D%272%27/%3E%3Ccircle%20cx%3D%276%27%20cy%3D%2718%27%20r%3D%272%27/%3E%3Cpath%20d%3D%27M20%204%20L8.12%2015.88%27/%3E%3Cpath%20d%3D%27M14.47%2014.48%20L20%2020%27/%3E%3Cpath%20d%3D%27M8.12%208.12%20L12%2012%27/%3E%3C/svg%3E\") 6 6, crosshair";

fn reset_time_range(viewport: &mut Viewport, data_len: usize) {
    let len = data_len as f64;
    let visible = len.min(DEFAULT_INITIAL_VISIBLE_BARS);
    viewport.set_range(len - visible, len);
}

fn snap_crosshair_to_time_scale(
    viewport: &Viewport,
    time_scale: &TimeScaleIndex,
    crosshair: &mut CrosshairState,
    x_css: f64,
    pane_css_w: f64,
) {
    let grid_idx = viewport.bar_index_for_crosshair(x_css, pane_css_w);
    crosshair.bar_index = grid_idx.and_then(|idx| time_scale.main_bar_index_at_slot(idx));
    crosshair.x = grid_idx
        .map(|idx| viewport.bar_center_css(idx, pane_css_w))
        .unwrap_or(x_css);
}

/// Which zone the pointer is in — determined by the WASM layer based on DOM element.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HitZone {
    Chart,
    TimeAxis,
    PriceAxis,
    None,
}

/// Touch tracking mode — compatibility-style crosshair on touch.
/// On touch devices crosshair is hidden until user long-presses,
/// then it tracks the finger. Double-tap hides it again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TouchCrosshairMode {
    /// Crosshair hidden (default on touch).
    Hidden,
    /// Long-press activated — crosshair tracks finger.
    Tracking,
}

/// Interaction state machine.
pub struct InteractionHandler {
    // ── Press / drag state ──
    pub pressed: bool,
    press_x: f64,
    press_y: f64,
    pub drag_active: bool,
    press_zone: HitZone,

    // ── Double-click detection ──
    last_click_time: f64,
    last_click_zone: HitZone,
    last_click_x: f64,
    last_click_y: f64,

    // ── Time axis scale state (reference implementation: TimeScale.startScale / scaleTo / endScale) ──
    time_scale_start_x: f64,
    time_scale_start_visible_bars: f64,

    // ── Price axis scale state (reference implementation: PriceScale.startScale / scaleTo) ──
    // reference implementation inverts Y: _scaleStartPoint = height - localY
    price_scale_start_y_inv: f64,
    price_scale_start_range: f64,
    price_scale_start_mid: f64,
    price_scale_height: f64,

    // ── Chart pan state (reference implementation: startScrollTime / scrollTimeTo) ──
    scroll_start_x: f64,
    scroll_start_bar: f64,

    // ── Chart pan price state (reference implementation: PriceScale.startScroll / scrollTo) ──
    scroll_price_start_y: f64,
    scroll_price_start_min: f64,
    scroll_price_start_max: f64,

    // ── Kinetic scrolling (momentum) ──
    pub velocity_x: f64,
    pub velocity_y: f64,
    pub last_move_time: f64,
    pub last_move_x: f64,
    pub last_move_y: f64,
    pub is_gliding: bool,

    // ── Current hover zone (for cursor hints) ──
    current_zone: HitZone,
    /// Replay trim mode: force scissors cursor on chart pane.
    replay_chart_trim_mode: bool,

    // ── Drawing-aware cursor override ──
    /// When set, overrides the normal zone-based cursor (e.g. resize on anchor hover).
    pub drawing_cursor: Option<&'static str>,
    /// True while a drawing drag is in progress — suppresses crosshair + pan.
    pub drawing_drag_active: bool,

    // ── Touch-specific state ──
    /// Whether the current interaction is from a touch device.
    pub is_touch: bool,

    /// Touch crosshair mode — hidden until long-press on touch devices.
    pub touch_crosshair_mode: TouchCrosshairMode,

    // ── Pinch zoom state ──
    pub pinch_active: bool,
    pub pinch_start_distance: f64,
    pinch_start_bar_span: f64,
    pinch_start_center_bar: f64,
    pinch_start_price_anchor_internal: f64,
    pinch_start_price_range: f64,
    pinch_start_price_mid: f64,
    pinch_prev_scale: f64,

    // ── Long-press state ──
    /// Set to true by WASM layer when long-press timer fires.
    pub long_press_fired: bool,

    // ── Touch tracking mode (reference implementation _startTrackPoint) ──
    /// When in tracking mode, crosshair follows finger relative to initial position.
    track_start_x: f64,
    track_start_y: f64,
    track_crosshair_init_x: f64,
    track_crosshair_init_y: f64,
}

impl InteractionHandler {
    pub fn new() -> Self {
        Self {
            pressed: false,
            press_x: 0.0,
            press_y: 0.0,
            drag_active: false,
            press_zone: HitZone::None,
            last_click_time: 0.0,
            last_click_zone: HitZone::None,
            last_click_x: 0.0,
            last_click_y: 0.0,
            time_scale_start_x: 0.0,
            time_scale_start_visible_bars: 0.0,
            price_scale_start_y_inv: 0.0,
            price_scale_start_range: 0.0,
            price_scale_start_mid: 0.0,
            price_scale_height: 0.0,
            scroll_start_x: 0.0,
            scroll_start_bar: 0.0,
            scroll_price_start_y: 0.0,
            scroll_price_start_min: 0.0,
            scroll_price_start_max: 0.0,
            velocity_x: 0.0,
            velocity_y: 0.0,
            last_move_time: 0.0,
            last_move_x: 0.0,
            last_move_y: 0.0,
            is_gliding: false,
            current_zone: HitZone::None,
            replay_chart_trim_mode: false,
            drawing_cursor: None,
            drawing_drag_active: false,
            is_touch: false,
            touch_crosshair_mode: TouchCrosshairMode::Hidden,
            pinch_active: false,
            pinch_start_distance: 0.0,
            pinch_start_bar_span: 0.0,
            pinch_start_center_bar: 0.0,
            pinch_start_price_anchor_internal: 0.0,
            pinch_start_price_range: 0.0,
            pinch_start_price_mid: 0.0,
            pinch_prev_scale: 1.0,
            long_press_fired: false,
            track_start_x: 0.0,
            track_start_y: 0.0,
            track_crosshair_init_x: 0.0,
            track_crosshair_init_y: 0.0,
        }
    }

    /// Set whether the current device is touch (call from pointermove/pointerdown).
    pub fn set_touch(&mut self, is_touch: bool) {
        self.is_touch = is_touch;
    }

    /// Returns true when the current press matches the previous click closely
    /// enough to count as a repeated click/tap.
    pub fn is_double_click_candidate(
        &self,
        zone: HitZone,
        now_ms: f64,
        distance_threshold: f64,
    ) -> bool {
        if zone == HitZone::None || self.last_click_zone != zone || self.last_click_time <= 0.0 {
            return false;
        }

        let dt = now_ms - self.last_click_time;
        if dt < 0.0 || dt >= DOUBLE_CLICK_WINDOW_MS {
            return false;
        }

        let manhattan =
            (self.press_x - self.last_click_x).abs() + (self.press_y - self.last_click_y).abs();
        manhattan < distance_threshold
    }

    /// Called when pointer enters a widget zone.
    pub fn pointer_enter(&mut self, zone: HitZone, crosshair: &mut CrosshairState) {
        self.current_zone = zone;
        match zone {
            HitZone::Chart => {
                // On touch: crosshair ONLY shows in tracking mode (after long-press).
                // On mouse: crosshair always shows.
                if self.is_touch {
                    crosshair.active = self.touch_crosshair_mode == TouchCrosshairMode::Tracking;
                } else {
                    crosshair.active = true;
                }
            }
            _ => {
                if !self.is_touch {
                    crosshair.active = false;
                }
            }
        }
    }

    /// Called when pointer leaves a widget zone.
    pub fn pointer_leave(&mut self, zone: HitZone, crosshair: &mut CrosshairState) {
        if self.current_zone == zone {
            self.current_zone = HitZone::None;
        }
        // On touch: keep crosshair state (tracking persists).
        // On mouse: hide crosshair when leaving chart.
        if zone == HitZone::Chart && !self.is_touch {
            crosshair.active = false;
        }
    }

    pub fn is_pressed_in_zone(&self, zone: HitZone) -> bool {
        self.pressed && self.press_zone == zone
    }

    pub fn press_position(&self) -> (f64, f64) {
        (self.press_x, self.press_y)
    }

    // ── Pinch zoom (two-finger) ──

    /// Start a pinch gesture. Called from WASM when 2 touches detected.
    /// `cx`, `cy` = center of the two touches in CSS coords.
    /// `distance` = distance between the two touches.
    pub fn pinch_start(
        &mut self,
        cx: f64,
        cy: f64,
        distance: f64,
        pane_css_w: f64,
        pane_css_h: f64,
        zoom_price_with_time: bool,
        viewport: &Viewport,
    ) {
        self.pinch_active = true;
        self.pinch_start_distance = distance.max(1.0);
        self.pinch_start_bar_span = viewport.end_bar - viewport.start_bar;
        self.pinch_prev_scale = 1.0;
        self.is_gliding = false;
        self.velocity_x = 0.0;
        self.velocity_y = 0.0;

        // Focal bar for zoom
        let focal_frac = cx.clamp(0.0, pane_css_w) / pane_css_w.max(1.0);
        self.pinch_start_center_bar = viewport.start_bar + focal_frac * self.pinch_start_bar_span;

        // Price range snapshot
        self.pinch_start_price_range = viewport.price_max - viewport.price_min;
        self.pinch_start_price_mid = (viewport.price_min + viewport.price_max) / 2.0;
        if zoom_price_with_time {
            let candle_css_h = (pane_css_h * viewport.candle_height_frac()).max(1.0);
            let focal_y_css = cy.clamp(0.0, candle_css_h);
            let focal_price = viewport.pixel_to_price(focal_y_css, candle_css_h);
            self.pinch_start_price_anchor_internal = viewport.price_to_internal(focal_price);
        } else {
            self.pinch_start_price_anchor_internal = self.pinch_start_price_mid;
        }
    }

    /// Update pinch gesture. Called from WASM on touchmove with 2 touches.
    /// `scale` = current_distance / start_distance.
    /// When `zoom_price_with_time` is true (footprint), applies two-axis zoom.
    pub fn pinch_update(
        &mut self,
        scale: f64,
        zoom_price_with_time: bool,
        viewport: &mut Viewport,
        bars: &BarArray,
        data_len: usize,
    ) {
        if !self.pinch_active {
            return;
        }

        // reference implementation: zoomScale = (scale - prevScale) * 5
        let zoom_scale = (scale - self.pinch_prev_scale) * PINCH_SCALE_MULTIPLIER;
        self.pinch_prev_scale = scale;

        if zoom_scale.abs() < 0.0001 {
            return;
        }

        // Time zoom — same as reference implementation zoomTime
        let factor = 1.0 / (1.0 + zoom_scale / ZOOM_FACTOR_DIVISOR);
        viewport.zoom(self.pinch_start_center_bar, factor);
        viewport.clamp_to_data(data_len);

        if zoom_price_with_time {
            // Footprint mode: zoom price around the initial pinch focal Y value.
            Self::zoom_price_around_internal_anchor(
                viewport,
                self.pinch_start_price_anchor_internal,
                factor,
            );
            viewport.price_locked = true;
        } else if viewport.price_locked {
            // Existing behavior: when user explicitly locked scale, keep midpoint zoom.
            let half = self.pinch_start_price_range / 2.0 / scale.max(MIN_PINCH_SCALE);
            viewport.price_min = self.pinch_start_price_mid - half;
            viewport.price_max = self.pinch_start_price_mid + half;
        } else {
            // Existing behavior for unlocked non-footprint charts.
            viewport.auto_fit_price(bars);
        }
    }

    /// End pinch gesture.
    pub fn pinch_end(&mut self) {
        self.pinch_active = false;
    }

    // ── Long-press → tracking mode ──

    /// Called by WASM when the long-press timer fires (240ms like reference implementation).
    /// Activates crosshair tracking mode.
    pub fn long_press(&mut self, x: f64, y: f64, crosshair: &mut CrosshairState) {
        self.long_press_fired = true;
        self.touch_crosshair_mode = TouchCrosshairMode::Tracking;
        crosshair.active = true;

        // Initialize tracking anchor
        self.track_start_x = x;
        self.track_start_y = y;
        self.track_crosshair_init_x = x;
        self.track_crosshair_init_y = y;

        // Set crosshair position immediately
        crosshair.x = x;
        crosshair.y = y;
    }

    /// Double-tap on chart in touch mode: toggle crosshair off if tracking,
    /// otherwise zoom-in / reset.
    pub fn touch_double_tap(
        &mut self,
        crosshair: &mut CrosshairState,
        viewport: &mut Viewport,
        bars: &BarArray,
        data_len: usize,
    ) {
        if self.touch_crosshair_mode == TouchCrosshairMode::Tracking {
            // Hide crosshair
            self.touch_crosshair_mode = TouchCrosshairMode::Hidden;
            crosshair.active = false;
        } else {
            // Reset view (same as mouse double-click on chart)
            viewport.price_locked = false;
            reset_time_range(viewport, data_len);
            viewport.auto_fit_price(bars);
        }
    }

    pub fn exit_touch_tracking(&mut self, crosshair: &mut CrosshairState) {
        if self.touch_crosshair_mode == TouchCrosshairMode::Tracking {
            self.touch_crosshair_mode = TouchCrosshairMode::Hidden;
            crosshair.active = false;
        }
    }

    /// Pointer move on the PANE (chart area).
    pub fn pane_pointer_move(
        &mut self,
        x: f64,
        y: f64,
        pane_css_w: f64,
        pane_css_h: f64,
        viewport: &mut Viewport,
        crosshair: &mut CrosshairState,
        bars: &BarArray,
        time_scale: &TimeScaleIndex,
        dpr: f64,
    ) {
        let candle_phys_h = pane_css_h * viewport.candle_height_frac() * dpr;
        let now = now_ms();

        // ── TOUCH: tracking mode (long-press activated) ──
        // Crosshair follows finger; chart does NOT move.
        if self.is_touch
            && self.touch_crosshair_mode == TouchCrosshairMode::Tracking
            && self.pressed
        {
            let new_x = self.track_crosshair_init_x + (x - self.track_start_x);
            let cx = new_x.clamp(0.0, pane_css_w);

            crosshair.active = true;
            snap_crosshair_to_time_scale(viewport, time_scale, crosshair, cx, pane_css_w);

            // Y line behavior depends on mode
            match crosshair.mode {
                CrosshairMode::Magnet | CrosshairMode::MagnetOHLC => {
                    // Only snap Y if we have actual bar data
                    if let Some(idx) = crosshair.bar_index {
                        let cursor_y = (self.track_crosshair_init_y + (y - self.track_start_y))
                            .clamp(0.0, pane_css_h);
                        let snap_price =
                            magnet_snap_ohlc_price(bars, idx, cursor_y, viewport, pane_css_h);
                        crosshair.y = viewport
                            .price_to_css_y(snap_price, pane_css_h)
                            .clamp(0.0, pane_css_h);
                        crosshair.price = snap_price;
                    } else {
                        let cy = (self.track_crosshair_init_y + (y - self.track_start_y))
                            .clamp(0.0, pane_css_h);
                        crosshair.y = cy;
                        crosshair.price = viewport.pixel_to_price(cy * dpr, candle_phys_h);
                    }
                }
                CrosshairMode::Normal => {
                    let cy = (self.track_crosshair_init_y + (y - self.track_start_y))
                        .clamp(0.0, pane_css_h);
                    crosshair.y = cy;
                    crosshair.price = viewport.pixel_to_price(cy * dpr, candle_phys_h);
                }
            }
            // NO panning in tracking mode — return early
            return;
        }

        // ── TOUCH: hidden mode → NO crosshair at all ──
        if self.is_touch && self.touch_crosshair_mode == TouchCrosshairMode::Hidden {
            crosshair.active = false;
            // Fall through to drag/pan below
        }
        // ── MOUSE: always update crosshair ──
        else if !self.is_touch {
            crosshair.active = true;
            let cx = x.clamp(0.0, pane_css_w);
            snap_crosshair_to_time_scale(viewport, time_scale, crosshair, cx, pane_css_w);

            // Y line behavior depends on mode
            match crosshair.mode {
                CrosshairMode::Magnet | CrosshairMode::MagnetOHLC => {
                    // Only snap Y if we have actual bar data
                    if let Some(idx) = crosshair.bar_index {
                        let cursor_y = y.clamp(0.0, pane_css_h);
                        let snap_price =
                            magnet_snap_ohlc_price(bars, idx, cursor_y, viewport, pane_css_h);
                        crosshair.y = viewport
                            .price_to_css_y(snap_price, pane_css_h)
                            .clamp(0.0, pane_css_h);
                        crosshair.price = snap_price;
                    } else {
                        crosshair.y = y.clamp(0.0, pane_css_h);
                        crosshair.price = viewport.pixel_to_price(crosshair.y * dpr, candle_phys_h);
                    }
                }
                CrosshairMode::Normal => {
                    crosshair.y = y.clamp(0.0, pane_css_h);
                    crosshair.price = viewport.pixel_to_price(crosshair.y * dpr, candle_phys_h);
                }
            }
        }

        // ── Drag → scroll/pan (both mouse and touch-hidden mode) ──
        if self.pressed && self.press_zone == HitZone::Chart && !self.pinch_active {
            let manhattan = (x - self.press_x).abs() + (y - self.press_y).abs();
            if !self.drag_active && manhattan >= CANCEL_CLICK_DISTANCE {
                self.drag_active = true;
                // On touch: cancel long-press once drag starts
                if self.is_touch {
                    self.long_press_fired = false;
                }
            }
            if self.drag_active && pane_css_w > 0.0 {
                // Track velocity for optional kinetic scrolling.
                let dt = now - self.last_move_time;
                if dt > 0.0 && dt < VELOCITY_SAMPLE_WINDOW_MS {
                    let vx = (x - self.last_move_x) / dt;
                    self.velocity_x = self.velocity_x * VELOCITY_SMOOTHING_FACTOR
                        + vx * (1.0 - VELOCITY_SMOOTHING_FACTOR);
                }
                self.last_move_time = now;
                self.last_move_x = x;
                self.last_move_y = y;

                // Time scroll (horizontal only)
                let bar_span = viewport.end_bar - viewport.start_bar;
                let dx_bars = (self.scroll_start_x - x) / pane_css_w * bar_span;
                let new_start = self.scroll_start_bar + dx_bars;
                let new_end = new_start + bar_span;
                viewport.set_range(new_start, new_end);
                viewport.clamp_to_data(time_scale.len());

                // Price scroll — both mouse and touch when price is locked
                if viewport.price_locked {
                    let price_range = self.scroll_price_start_max - self.scroll_price_start_min;
                    if pane_css_h > 1.0 && price_range > 0.0 {
                        let price_per_px = price_range / (pane_css_h - 1.0);
                        let dy = y - self.scroll_price_start_y;
                        let price_delta = dy * price_per_px;
                        viewport.price_min = self.scroll_price_start_min + price_delta;
                        viewport.price_max = self.scroll_price_start_max + price_delta;
                    }
                } else if !viewport.price_locked {
                    viewport.auto_fit_price(bars);
                }
            }
        }
    }

    /// Pointer move on the TIME AXIS.
    /// reference implementation: scaleTo — ratio of distances from right edge.
    pub fn time_axis_pointer_move(
        &mut self,
        x: f64,
        pane_css_w: f64,
        viewport: &mut Viewport,
        bars: &BarArray,
        data_len: usize,
    ) {
        if self.pressed && self.press_zone == HitZone::TimeAxis {
            let manhattan = (x - self.press_x).abs();
            if !self.drag_active && manhattan >= CANCEL_CLICK_DISTANCE {
                self.drag_active = true;
            }
            if self.drag_active && pane_css_w > 1.0 {
                let start_len = (pane_css_w - self.time_scale_start_x).clamp(1.0, pane_css_w);
                let current_len = (pane_css_w - x).clamp(1.0, pane_css_w);
                let ratio = start_len / current_len;
                let max_visible_bars = (data_len.max(1) as f64) * TIME_AXIS_MAX_BAR_MULTIPLIER;
                let new_bar_count = (self.time_scale_start_visible_bars * ratio)
                    .clamp(TIME_AXIS_MIN_BARS, max_visible_bars);
                let end = viewport.end_bar;
                let new_start = end - new_bar_count;
                viewport.set_range(new_start, end);
                viewport.clamp_to_data(data_len);
                if !viewport.price_locked {
                    viewport.auto_fit_price(bars);
                }
            }
        }
    }

    /// Pointer move on the PRICE AXIS.
    /// Exponential delta-based scaling: smooth, symmetric, position-independent.
    /// Drag UP (y decreases) → zoom IN, drag DOWN (y increases) → zoom OUT.
    /// Uses an exponential mapping so sensitivity is uniform across the entire
    /// axis height and round-trips are mathematically perfect.
    pub fn price_axis_pointer_move(&mut self, y: f64, pane_css_h: f64, viewport: &mut Viewport) {
        if self.pressed && self.press_zone == HitZone::PriceAxis {
            let manhattan = (y - self.press_y).abs();
            if !self.drag_active && manhattan >= CANCEL_CLICK_DISTANCE {
                self.drag_active = true;
            }
            if self.drag_active && pane_css_h > 1.0 {
                let delta = self.press_y - y; // positive = moved up = zoom in
                let sensitivity = self.price_scale_height * PRICE_SCALE_DRAG_SENSITIVITY;
                let scale = (-delta / sensitivity)
                    .exp()
                    .clamp(MIN_PRICE_SCALE_COEFF, MAX_PRICE_SCALE_COEFF);

                let half = self.price_scale_start_range * scale / 2.0;
                let mid = self.price_scale_start_mid;
                viewport.price_min = mid - half;
                viewport.price_max = mid + half;
                viewport.price_locked = true;
            }
        }
    }

    /// Pointer down on any widget.
    pub fn pointer_down(
        &mut self,
        x: f64,
        y: f64,
        zone: HitZone,
        viewport: &Viewport,
        pane_css_h: f64,
    ) {
        self.pressed = true;
        self.press_x = x;
        self.press_y = y;
        self.drag_active = false;
        self.press_zone = zone;
        self.long_press_fired = false;

        // Reset gliding state
        self.velocity_x = 0.0;
        self.velocity_y = 0.0;
        self.last_move_time = now_ms();
        self.last_move_x = x;
        self.last_move_y = y;
        self.is_gliding = false;

        match zone {
            HitZone::Chart => {
                self.scroll_start_x = x;
                self.scroll_start_bar = viewport.start_bar;
                self.scroll_price_start_y = y;
                self.scroll_price_start_min = viewport.price_min;
                self.scroll_price_start_max = viewport.price_max;
            }
            HitZone::TimeAxis => {
                self.time_scale_start_x = x;
                self.time_scale_start_visible_bars = viewport.end_bar - viewport.start_bar;
            }
            HitZone::PriceAxis => {
                self.price_scale_height = pane_css_h;
                self.price_scale_start_y_inv = pane_css_h - y;
                self.price_scale_start_range = viewport.price_max - viewport.price_min;
                self.price_scale_start_mid = (viewport.price_min + viewport.price_max) / 2.0;
            }
            _ => {}
        }
    }

    /// Pointer up on any widget.
    pub fn pointer_up(
        &mut self,
        viewport: &mut Viewport,
        bars: &BarArray,
        data_len: usize,
        now_ms: f64,
        allow_double_click_reset: bool,
        allow_touch_kinetic_scroll: bool,
        allow_mouse_kinetic_scroll: bool,
    ) {
        let was_click = self.pressed && !self.drag_active;
        let zone = self.press_zone;
        let click_x = self.press_x;
        let click_y = self.press_y;

        // Kinetic scrolling: optional per pointer type, horizontal only.
        let allow_kinetic_scroll = if self.is_touch {
            allow_touch_kinetic_scroll
        } else {
            allow_mouse_kinetic_scroll
        };
        if allow_kinetic_scroll && self.pressed && self.drag_active && zone == HitZone::Chart {
            let dt = now_ms - self.last_move_time;
            if dt < KINETIC_TRIGGER_WINDOW_MS && self.velocity_x.abs() > MIN_KINETIC_VELOCITY {
                self.is_gliding = true;
                self.velocity_y = 0.0; // horizontal only
            } else {
                self.velocity_x = 0.0;
                self.velocity_y = 0.0;
            }
        } else {
            self.velocity_x = 0.0;
            self.velocity_y = 0.0;
        }

        // If touch tracking mode and user lifts finger — keep crosshair visible
        // (reference behavior: crosshair stays until next double-tap or touchStart+exit)

        self.pressed = false;
        self.drag_active = false;

        // Double-click / double-tap detection
        if allow_double_click_reset && was_click && zone != HitZone::None && !self.long_press_fired
        {
            let distance_threshold = if self.is_touch {
                30.0
            } else {
                DOUBLE_CLICK_DISTANCE
            };
            if self.is_double_click_candidate(zone, now_ms, distance_threshold) {
                match zone {
                    HitZone::TimeAxis => {
                        reset_time_range(viewport, data_len);
                        if !viewport.price_locked {
                            viewport.auto_fit_price(bars);
                        }
                    }
                    HitZone::PriceAxis => {
                        viewport.price_locked = false;
                        viewport.auto_fit_price(bars);
                    }
                    HitZone::Chart => {
                        // For touch: handled separately via touch_double_tap
                        if !self.is_touch {
                            viewport.price_locked = false;
                            reset_time_range(viewport, data_len);
                            viewport.auto_fit_price(bars);
                        }
                    }
                    _ => {}
                }
                self.last_click_time = 0.0;
                self.last_click_zone = HitZone::None;
                self.last_click_x = 0.0;
                self.last_click_y = 0.0;
            } else {
                self.last_click_time = now_ms;
                self.last_click_zone = zone;
                self.last_click_x = click_x;
                self.last_click_y = click_y;
            }
        }
    }

    /// Wheel event on the chart pane.
    /// reference baseline: deltaY → time zoom, deltaX → time scroll.
    /// Footprint mode can optionally apply cursor-anchored Y zoom as well.
    pub fn pane_wheel(
        &mut self,
        x: f64,
        y: f64,
        delta_x: f64,
        delta_y: f64,
        delta_mode: u32,
        pane_css_w: f64,
        pane_css_h: f64,
        zoom_price_with_time: bool,
        viewport: &mut Viewport,
        bars: &BarArray,
        data_len: usize,
    ) {
        if pane_css_w <= 0.0 {
            return;
        }

        let speed_adj = match delta_mode {
            2 => WHEEL_DELTA_PAGE_MULTIPLIER, // DOM_DELTA_PAGE
            1 => WHEEL_DELTA_LINE_MULTIPLIER, // DOM_DELTA_LINE
            _ => 1.0,                         // DOM_DELTA_PIXEL
        };

        let adj_dx = speed_adj * delta_x / WHEEL_SPEED_DIVISOR;
        let adj_dy = -(speed_adj * delta_y / WHEEL_SPEED_DIVISOR);

        // deltaY → zoom time
        if adj_dy.abs() > 0.001 {
            let zoom_scale = adj_dy.signum() * adj_dy.abs().min(1.0);
            let factor = 1.0 / (1.0 + zoom_scale / ZOOM_FACTOR_DIVISOR);

            let scroll_position = x.clamp(0.0, pane_css_w);
            let focal_frac = scroll_position / pane_css_w;
            let focal_bar =
                viewport.start_bar + focal_frac * (viewport.end_bar - viewport.start_bar);

            viewport.zoom(focal_bar, factor);
            viewport.clamp_to_data(data_len);
            if zoom_price_with_time {
                let candle_css_h = (pane_css_h * viewport.candle_height_frac()).max(1.0);
                let focal_y_css = y.clamp(0.0, candle_css_h);
                let focal_price = viewport.pixel_to_price(focal_y_css, candle_css_h);
                let focal_internal = viewport.price_to_internal(focal_price);
                Self::zoom_price_around_internal_anchor(viewport, focal_internal, factor);
                viewport.price_locked = true;
            } else if !viewport.price_locked {
                viewport.auto_fit_price(bars);
            }
        }

        // deltaX → scroll time
        if adj_dx.abs() > 0.001 {
            let visible_bars = viewport.end_bar - viewport.start_bar;
            let bar_spacing = pane_css_w / visible_bars;
            let scroll_bars = adj_dx * SCROLL_MULTIPLIER / bar_spacing;
            viewport.pan_clamped(scroll_bars, data_len);
            if !viewport.price_locked {
                viewport.auto_fit_price(bars);
            }
        }
    }

    /// Wheel event on the time axis — same zoom behavior as pane.
    pub fn time_axis_wheel(
        &mut self,
        x: f64,
        delta_y: f64,
        delta_mode: u32,
        pane_css_w: f64,
        pane_css_h: f64,
        viewport: &mut Viewport,
        bars: &BarArray,
        data_len: usize,
    ) {
        self.pane_wheel(
            x, 0.0, 0.0, delta_y, delta_mode, pane_css_w, pane_css_h, false, viewport, bars,
            data_len,
        );
    }

    /// Wheel event on the price axis — zoom price range.
    pub fn price_axis_wheel(&mut self, delta_y: f64, delta_mode: u32, viewport: &mut Viewport) {
        let speed_adj = match delta_mode {
            2 => WHEEL_DELTA_PAGE_MULTIPLIER,
            1 => WHEEL_DELTA_LINE_MULTIPLIER,
            _ => 1.0,
        };
        let adj_dy = -(speed_adj * delta_y / WHEEL_SPEED_DIVISOR);
        if adj_dy.abs() > 0.001 {
            let zoom_scale = adj_dy.signum() * adj_dy.abs().min(1.0);
            let factor = 1.0 / (1.0 + zoom_scale / ZOOM_FACTOR_DIVISOR);
            let mid = (viewport.price_min + viewport.price_max) / 2.0;
            let half = (viewport.price_max - viewport.price_min) / 2.0;
            viewport.price_min = mid - half * factor;
            viewport.price_max = mid + half * factor;
            viewport.price_locked = true;
        }
    }

    /// Is the user currently dragging?
    pub fn is_dragging(&self) -> bool {
        self.drag_active
    }

    /// Set the drawing-aware cursor override (called from WASM hover hit-test).
    pub fn set_drawing_cursor(&mut self, cursor: Option<&'static str>) {
        self.drawing_cursor = cursor;
    }

    /// Cancel the current pointer gesture without turning it into a click/double-click.
    ///
    /// Used by drawing/price-line flows that consume pointer release internally and
    /// still need the base interaction state machine to fully let go.
    pub fn cancel_pointer_gesture(&mut self) {
        self.pressed = false;
        self.drag_active = false;
        self.press_zone = HitZone::None;
        self.long_press_fired = false;
        self.is_gliding = false;
        self.velocity_x = 0.0;
        self.velocity_y = 0.0;
    }

    /// Start a touch-style horizontal glide from a requested logical bar delta.
    pub fn start_horizontal_glide_by_bars(
        &mut self,
        delta_bars: f64,
        pane_css_w: f64,
        visible_bar_span: f64,
    ) {
        if !delta_bars.is_finite()
            || !pane_css_w.is_finite()
            || !visible_bar_span.is_finite()
            || pane_css_w <= 0.0
            || visible_bar_span <= 0.0
        {
            return;
        }

        let total_dx_px = -delta_bars * pane_css_w / visible_bar_span;
        let decay = -KINETIC_FRICTION_COEFFICIENT.ln() / PHYSICS_FRAME_MS;
        if !decay.is_finite() || decay <= 0.0 {
            return;
        }

        let mut velocity_x = total_dx_px * decay;
        let min_keyboard_velocity = MIN_KINETIC_VELOCITY * 1.5;
        if velocity_x.abs() < min_keyboard_velocity {
            velocity_x = min_keyboard_velocity.copysign(total_dx_px);
        }

        if self.is_gliding && self.velocity_x.signum() == velocity_x.signum() {
            self.velocity_x += velocity_x;
        } else {
            self.velocity_x = velocity_x;
        }
        self.velocity_y = 0.0;
        self.is_gliding = true;
        self.last_move_time = now_ms();
    }

    /// Enable/disable replay trim mode cursor policy.
    pub fn set_replay_chart_trim_mode(&mut self, enabled: bool) {
        self.replay_chart_trim_mode = enabled;
    }

    #[inline]
    fn zoom_price_around_internal_anchor(
        viewport: &mut Viewport,
        anchor_internal: f64,
        factor: f64,
    ) {
        if !anchor_internal.is_finite() || !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let old_min = viewport.price_min;
        let old_max = viewport.price_max;
        if !old_min.is_finite() || !old_max.is_finite() || old_max <= old_min {
            return;
        }
        viewport.price_min = anchor_internal - (anchor_internal - old_min) * factor;
        viewport.price_max = anchor_internal + (old_max - anchor_internal) * factor;
    }

    /// Get the current cursor style hint.
    /// Priority: drawing drag → drawing hover cursor → zone-based default.
    pub fn cursor_hint(&self) -> &'static str {
        if self.replay_chart_trim_mode {
            if self.drag_active {
                return match self.press_zone {
                    HitZone::Chart => REPLAY_SCISSORS_CURSOR,
                    HitZone::TimeAxis => "ew-resize",
                    HitZone::PriceAxis => "ns-resize",
                    HitZone::None => "default",
                };
            }
            return match self.current_zone {
                HitZone::Chart => REPLAY_SCISSORS_CURSOR,
                HitZone::TimeAxis => "ew-resize",
                HitZone::PriceAxis => "ns-resize",
                HitZone::None => "default",
            };
        }

        // Drawing drag in progress — use the drag cursor
        if self.drawing_drag_active {
            return match self.drawing_cursor {
                Some("move") => "grabbing",
                Some(cursor) => cursor,
                None => "grabbing",
            };
        }

        if self.drag_active {
            match self.press_zone {
                HitZone::Chart => "grabbing",
                HitZone::TimeAxis => "ew-resize",
                HitZone::PriceAxis => "ns-resize",
                _ => "default",
            }
        } else if let Some(dc) = self.drawing_cursor {
            // Hovering over a drawing — show context-sensitive cursor
            dc
        } else {
            match self.current_zone {
                HitZone::Chart => "crosshair",
                HitZone::TimeAxis => "ew-resize",
                HitZone::PriceAxis => "ns-resize",
                HitZone::None => "default",
            }
        }
    }

    /// Process kinetic scrolling deceleration on each frame.
    /// Touch-only, horizontal-only (like reference implementation). Returns true if gliding is still active.
    pub fn update_gliding(
        &mut self,
        pane_css_w: f64,
        _pane_css_h: f64,
        viewport: &mut Viewport,
        bars: &BarArray,
        data_len: usize,
    ) -> bool {
        if !self.is_gliding {
            return false;
        }

        let now = now_ms();
        let dt = now - self.last_move_time;
        if dt <= 0.0 {
            return true;
        }

        // Horizontal only — no vertical price drift
        let dx = self.velocity_x * dt;

        if pane_css_w > 0.0 {
            let bar_span = viewport.end_bar - viewport.start_bar;
            let dx_bars = -dx / pane_css_w * bar_span;
            viewport.pan_clamped(dx_bars, data_len);
            if !viewport.price_locked {
                viewport.auto_fit_price(bars);
            }
        }

        // Decelerate (friction)
        let friction = KINETIC_FRICTION_COEFFICIENT.powf(dt / PHYSICS_FRAME_MS);
        self.velocity_x *= friction;
        self.last_move_time = now;

        // Stop gliding if velocity is negligible
        if self.velocity_x.abs() < MIN_GLIDE_VELOCITY {
            self.is_gliding = false;
            self.velocity_x = 0.0;
        }

        self.is_gliding
    }
}

/// Compute the magnet-snap price for a given bar (MagnetOHLC mode).
///
/// Snaps to the O/H/L/C value whose CSS Y is nearest to `cursor_css_y`
/// (matching the reference implementation's `magnet.ts` algorithm).
fn magnet_snap_ohlc_price(
    bars: &BarArray,
    idx: usize,
    cursor_css_y: f64,
    viewport: &Viewport,
    pane_css_h: f64,
) -> f64 {
    let open = bars.open(idx) as f64;
    let high = bars.high(idx) as f64;
    let low = bars.low(idx) as f64;
    let close = bars.close(idx) as f64;

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

#[cfg(test)]
mod tests {
    use super::{HitZone, InteractionHandler, MIN_KINETIC_VELOCITY};
    use crate::core::constants::PHYSICS_FRAME_MS;
    use crate::core::data::{Bar, BarArray};
    use crate::core::viewport::Viewport;

    fn mk_bars(len: usize) -> BarArray {
        let mut out = Vec::with_capacity(len);
        for i in 0..len {
            let base = 100.0 + i as f64 * 0.25;
            out.push(Bar {
                timestamp: i as u64 + 1,
                open: base,
                high: base + 1.0,
                low: base - 1.0,
                close: base + 0.2,
                volume: 10.0 + i as f64,
            });
        }
        let mut bars = BarArray::new();
        bars.set(out).unwrap();
        bars
    }

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn footprint_wheel_zooms_time_and_price_and_locks() {
        let mut ih = InteractionHandler::new();
        let bars = mk_bars(240);
        let mut vp = Viewport::new(800, 600);
        vp.set_range(20.0, 120.0);
        vp.price_min = 100.0;
        vp.price_max = 200.0;
        vp.price_locked = false;

        let start_span = vp.end_bar - vp.start_bar;
        let start_price_span = vp.price_max - vp.price_min;

        ih.pane_wheel(
            400.0,
            180.0,
            0.0,
            -120.0,
            0,
            800.0,
            600.0,
            true,
            &mut vp,
            &bars,
            bars.len(),
        );

        let end_span = vp.end_bar - vp.start_bar;
        let end_price_span = vp.price_max - vp.price_min;
        assert!(end_span < start_span, "time span should shrink on zoom-in");
        assert!(
            end_price_span < start_price_span,
            "price span should shrink on footprint zoom-in"
        );
        assert!(
            vp.price_locked,
            "footprint pane zoom should lock price scale"
        );
    }

    #[test]
    fn non_footprint_wheel_preserves_locked_price_range() {
        let mut ih = InteractionHandler::new();
        let bars = mk_bars(240);
        let mut vp = Viewport::new(800, 600);
        vp.set_range(20.0, 120.0);
        vp.price_min = 123.0;
        vp.price_max = 234.0;
        vp.price_locked = true;
        let start_min = vp.price_min;
        let start_max = vp.price_max;

        ih.pane_wheel(
            400.0,
            180.0,
            0.0,
            -120.0,
            0,
            800.0,
            600.0,
            false,
            &mut vp,
            &bars,
            bars.len(),
        );

        assert!(
            approx_eq(vp.price_min, start_min, 1e-9),
            "non-footprint pane wheel should not change locked price min"
        );
        assert!(
            approx_eq(vp.price_max, start_max, 1e-9),
            "non-footprint pane wheel should not change locked price max"
        );
        assert!(vp.price_locked, "locked scale should remain locked");
    }

    #[test]
    fn footprint_wheel_keeps_cursor_price_anchor() {
        let mut ih = InteractionHandler::new();
        let bars = mk_bars(240);
        let mut vp = Viewport::new(800, 600);
        vp.set_range(20.0, 120.0);
        vp.price_min = 100.0;
        vp.price_max = 200.0;
        vp.price_locked = false;

        let y_css = 140.0;
        let candle_css_h = 600.0 * vp.candle_height_frac();
        let before_price = vp.pixel_to_price(y_css, candle_css_h);

        ih.pane_wheel(
            300.0,
            y_css,
            0.0,
            -90.0,
            0,
            800.0,
            600.0,
            true,
            &mut vp,
            &bars,
            bars.len(),
        );

        let after_price = vp.pixel_to_price(y_css, candle_css_h);
        assert!(
            approx_eq(before_price, after_price, 1e-6),
            "cursor-anchored Y zoom should preserve the focal price"
        );
    }

    #[test]
    fn footprint_pinch_zooms_time_and_price_and_locks() {
        let mut ih = InteractionHandler::new();
        let bars = mk_bars(240);
        let mut vp = Viewport::new(800, 600);
        vp.set_range(20.0, 120.0);
        vp.price_min = 100.0;
        vp.price_max = 200.0;
        vp.price_locked = false;

        let start_span = vp.end_bar - vp.start_bar;
        let start_price_span = vp.price_max - vp.price_min;

        ih.pinch_start(400.0, 170.0, 100.0, 800.0, 600.0, true, &vp);
        ih.pinch_update(1.2, true, &mut vp, &bars, bars.len());

        let end_span = vp.end_bar - vp.start_bar;
        let end_price_span = vp.price_max - vp.price_min;
        assert!(end_span < start_span, "pinch should zoom time span");
        assert!(
            end_price_span < start_price_span,
            "pinch should zoom price span"
        );
        assert!(vp.price_locked, "footprint pinch should lock price scale");
    }

    #[test]
    fn non_footprint_pinch_does_not_force_lock() {
        let mut ih = InteractionHandler::new();
        let bars = mk_bars(240);
        let mut vp = Viewport::new(800, 600);
        vp.set_range(20.0, 120.0);
        vp.price_min = 100.0;
        vp.price_max = 200.0;
        vp.price_locked = false;

        ih.pinch_start(400.0, 170.0, 100.0, 800.0, 600.0, false, &vp);
        ih.pinch_update(1.2, false, &mut vp, &bars, bars.len());

        assert!(
            !vp.price_locked,
            "non-footprint pinch should keep unlocked auto-fit behavior"
        );
    }

    #[test]
    fn keyboard_glide_impulse_uses_continuous_pan_path() {
        let mut ih = InteractionHandler::new();
        let bars = mk_bars(240);
        let mut vp = Viewport::new(800, 600);
        vp.set_range(20.0, 120.0);
        vp.auto_fit_price(&bars);

        let start = vp.start_bar;
        ih.start_horizontal_glide_by_bars(10.0, 800.0, vp.end_bar - vp.start_bar);
        assert!(ih.is_gliding);
        assert!(ih.velocity_x < 0.0);

        ih.last_move_time -= PHYSICS_FRAME_MS;
        let still_gliding = ih.update_gliding(800.0, 600.0, &mut vp, &bars, bars.len());
        assert!(still_gliding);
        assert!(vp.start_bar > start);
    }

    #[test]
    fn mouse_kinetic_option_controls_chart_drag_glide() {
        let mut ih = InteractionHandler::new();
        let bars = mk_bars(240);
        let mut vp = Viewport::new(800, 600);
        vp.set_range(20.0, 120.0);

        ih.pressed = true;
        ih.drag_active = true;
        ih.press_zone = HitZone::Chart;
        ih.is_touch = false;
        ih.velocity_x = MIN_KINETIC_VELOCITY * 2.0;
        ih.last_move_time = 100.0;

        ih.pointer_up(&mut vp, &bars, bars.len(), 110.0, true, true, false);
        assert!(
            !ih.is_gliding,
            "mouse kinetic should stay off when disabled"
        );

        ih.pressed = true;
        ih.drag_active = true;
        ih.press_zone = HitZone::Chart;
        ih.is_touch = false;
        ih.velocity_x = MIN_KINETIC_VELOCITY * 2.0;
        ih.last_move_time = 100.0;

        ih.pointer_up(&mut vp, &bars, bars.len(), 110.0, true, true, true);
        assert!(ih.is_gliding, "mouse kinetic should start when enabled");
    }
}
