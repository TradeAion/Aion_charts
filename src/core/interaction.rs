//! InteractionHandler — LWC-style pointer/wheel state machine with full touch support.
//!
//! Pure Rust, no DOM dependencies. The WASM layer forwards raw pointer events
//! WITH the zone already determined (since each widget is a separate DOM element).
//!
//! Architecture matches LWC:
//! - PaneWidget events → zone=Chart
//! - PriceAxisWidget events → zone=PriceAxis
//! - TimeAxisWidget events → zone=TimeAxis
//! - Each widget fires its own mouseEnter/Leave naturally
//!
//! Interaction model (matching LWC):
//! ── Pane ──
//!   wheel deltaY  → zoom time (proportional, focal-point aware)
//!   wheel deltaX  → scroll time
//!   drag          → scroll time + price
//!   pinch         → zoom X and Y
//!   long press    → activate crosshair tracking mode
//!   double tap    → zoom in / reset
//! ── Time Axis ──
//!   drag          → scale time (ratio from right edge, like LWC)
//!   wheel deltaY  → zoom time
//!   dbl-click     → reset time
//! ── Price Axis ──
//!   drag          → scale price (LWC inverted-Y formula)
//!   wheel deltaY  → zoom price
//!   dbl-click     → reset price

use crate::core::constants::{
    DEFAULT_INITIAL_VISIBLE_BARS, DOUBLE_CLICK_WINDOW_MS, KINETIC_FRICTION_COEFFICIENT,
    KINETIC_TRIGGER_WINDOW_MS, MIN_GLIDE_VELOCITY, MIN_KINETIC_VELOCITY, MIN_PINCH_SCALE,
    MIN_PRICE_SCALE_COEFF, PHYSICS_FRAME_MS, PINCH_SCALE_MULTIPLIER, PRICE_SCALE_OFFSET_COEFF,
    SCROLL_MULTIPLIER, TIME_AXIS_MAX_BAR_MULTIPLIER, TIME_AXIS_MIN_BARS, VELOCITY_SAMPLE_WINDOW_MS,
    VELOCITY_SMOOTHING_FACTOR, WHEEL_DELTA_LINE_MULTIPLIER, WHEEL_DELTA_PAGE_MULTIPLIER,
    WHEEL_SPEED_DIVISOR, ZOOM_FACTOR_DIVISOR,
};
use crate::core::data::BarArray;
use crate::core::renderer::traits::{CrosshairMode, CrosshairState};
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

/// Manhattan distance threshold before drag starts (LWC: CancelClickManhattanDistance = 5).
const CANCEL_CLICK_DISTANCE: f64 = 5.0;

/// Which zone the pointer is in — determined by the WASM layer based on DOM element.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HitZone {
    Chart,
    TimeAxis,
    PriceAxis,
    None,
}

/// Touch tracking mode — LWC-style crosshair on touch.
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

    // ── Time axis scale state (LWC: TimeScale.startScale / scaleTo / endScale) ──
    time_scale_start_x: f64,
    time_scale_start_visible_bars: f64,

    // ── Price axis scale state (LWC: PriceScale.startScale / scaleTo) ──
    // LWC inverts Y: _scaleStartPoint = height - localY
    price_scale_start_y_inv: f64,
    price_scale_start_range: f64,
    price_scale_start_mid: f64,
    price_scale_height: f64,

    // ── Chart pan state (LWC: startScrollTime / scrollTimeTo) ──
    scroll_start_x: f64,
    scroll_start_bar: f64,

    // ── Chart pan price state (LWC: PriceScale.startScroll / scrollTo) ──
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
    pinch_start_price_range: f64,
    pinch_start_price_mid: f64,
    pinch_prev_scale: f64,

    // ── Long-press state ──
    /// Set to true by WASM layer when long-press timer fires.
    pub long_press_fired: bool,

    // ── Touch tracking mode (LWC _startTrackPoint) ──
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
            drawing_cursor: None,
            drawing_drag_active: false,
            is_touch: false,
            touch_crosshair_mode: TouchCrosshairMode::Hidden,
            pinch_active: false,
            pinch_start_distance: 0.0,
            pinch_start_bar_span: 0.0,
            pinch_start_center_bar: 0.0,
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

    // ── Pinch zoom (two-finger) ──

    /// Start a pinch gesture. Called from WASM when 2 touches detected.
    /// `cx`, `cy` = center of the two touches in CSS coords.
    /// `distance` = distance between the two touches.
    pub fn pinch_start(
        &mut self,
        cx: f64,
        _cy: f64,
        distance: f64,
        pane_css_w: f64,
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
    }

    /// Update pinch gesture. Called from WASM on touchmove with 2 touches.
    /// `scale` = current_distance / start_distance.
    pub fn pinch_update(&mut self, scale: f64, viewport: &mut Viewport, bars: &BarArray) {
        if !self.pinch_active {
            return;
        }

        // LWC: zoomScale = (scale - prevScale) * 5
        let zoom_scale = (scale - self.pinch_prev_scale) * PINCH_SCALE_MULTIPLIER;
        self.pinch_prev_scale = scale;

        if zoom_scale.abs() < 0.0001 {
            return;
        }

        // Time zoom — same as LWC zoomTime
        let factor = 1.0 / (1.0 + zoom_scale / ZOOM_FACTOR_DIVISOR);
        viewport.zoom(self.pinch_start_center_bar, factor);
        viewport.clamp_to_data(bars.len());

        // Price zoom — scale around midpoint
        if viewport.price_locked {
            let half = self.pinch_start_price_range / 2.0 / scale.max(MIN_PINCH_SCALE);
            viewport.price_min = self.pinch_start_price_mid - half;
            viewport.price_max = self.pinch_start_price_mid + half;
        } else {
            viewport.auto_fit_price(bars);
        }
    }

    /// End pinch gesture.
    pub fn pinch_end(&mut self) {
        self.pinch_active = false;
    }

    // ── Long-press → tracking mode ──

    /// Called by WASM when the long-press timer fires (240ms like LWC).
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
    ) {
        if self.touch_crosshair_mode == TouchCrosshairMode::Tracking {
            // Hide crosshair
            self.touch_crosshair_mode = TouchCrosshairMode::Hidden;
            crosshair.active = false;
        } else {
            // Reset view (same as mouse double-click on chart)
            viewport.price_locked = false;
            let len = bars.len() as f64;
            let visible = len.min(DEFAULT_INITIAL_VISIBLE_BARS);
            viewport.set_range(len - visible, len);
            viewport.auto_fit_price(bars);
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
        dpr: f64,
    ) {
        let pane_phys_w = pane_css_w * dpr;
        let _pane_phys_h = pane_css_h * dpr;
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

            // Get grid-snapped index (can be beyond data_len in empty space)
            let grid_idx = viewport.bar_index_for_crosshair(cx * dpr, pane_phys_w);

            // bar_index is only set if we have actual data at this position
            crosshair.bar_index = grid_idx.filter(|&idx| idx < bars.len());

            // X line always snaps to bar grid (like LWC) - even in empty space
            if let Some(idx) = grid_idx {
                crosshair.x = viewport.bar_center_css(idx, pane_css_w);
            } else {
                crosshair.x = cx;
            }

            // Y line behavior depends on mode
            match crosshair.mode {
                CrosshairMode::MagnetOHLC => {
                    // Only snap Y if we have actual bar data
                    if let Some(idx) = crosshair.bar_index {
                        let snap_price =
                            magnet_snap_price(bars, idx, crosshair.y, viewport, pane_css_h);
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

            // Get grid-snapped index (can be beyond data_len in empty space)
            let grid_idx = viewport.bar_index_for_crosshair(cx * dpr, pane_phys_w);

            // bar_index is only set if we have actual data at this position
            crosshair.bar_index = grid_idx.filter(|&idx| idx < bars.len());

            // X line always snaps to bar grid (like LWC) - even in empty space
            if let Some(idx) = grid_idx {
                crosshair.x = viewport.bar_center_css(idx, pane_css_w);
            } else {
                crosshair.x = cx;
            }

            // Y line behavior depends on mode
            match crosshair.mode {
                CrosshairMode::MagnetOHLC => {
                    // Only snap Y if we have actual bar data
                    if let Some(idx) = crosshair.bar_index {
                        let cursor_y = y.clamp(0.0, pane_css_h);
                        let snap_price =
                            magnet_snap_price(bars, idx, cursor_y, viewport, pane_css_h);
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
                // Track velocity (touch-only, used for inertia)
                if self.is_touch {
                    let dt = now - self.last_move_time;
                    if dt > 0.0 && dt < VELOCITY_SAMPLE_WINDOW_MS {
                        let vx = (x - self.last_move_x) / dt;
                        self.velocity_x = self.velocity_x * VELOCITY_SMOOTHING_FACTOR
                            + vx * (1.0 - VELOCITY_SMOOTHING_FACTOR);
                    }
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
                viewport.clamp_to_data(bars.len());

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
    /// LWC: scaleTo — ratio of distances from right edge.
    pub fn time_axis_pointer_move(
        &mut self,
        x: f64,
        pane_css_w: f64,
        viewport: &mut Viewport,
        bars: &BarArray,
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
                let new_bar_count = (self.time_scale_start_visible_bars * ratio).clamp(
                    TIME_AXIS_MIN_BARS,
                    bars.len() as f64 * TIME_AXIS_MAX_BAR_MULTIPLIER,
                );
                let end = viewport.end_bar;
                let new_start = end - new_bar_count;
                viewport.set_range(new_start, end);
                viewport.clamp_to_data(bars.len());
                if !viewport.price_locked {
                    viewport.auto_fit_price(bars);
                }
            }
        }
    }

    /// Pointer move on the PRICE AXIS.
    /// LWC: PriceScale.scaleTo — inverted Y, coefficient formula.
    pub fn price_axis_pointer_move(&mut self, y: f64, pane_css_h: f64, viewport: &mut Viewport) {
        if self.pressed && self.press_zone == HitZone::PriceAxis {
            let manhattan = (y - self.press_y).abs();
            if !self.drag_active && manhattan >= CANCEL_CLICK_DISTANCE {
                self.drag_active = true;
            }
            if self.drag_active && pane_css_h > 1.0 {
                let h = self.price_scale_height;
                let inv_y = (h - y).max(0.0);
                let offset = (h - 1.0) * PRICE_SCALE_OFFSET_COEFF;
                let scale_coeff = ((self.price_scale_start_y_inv + offset) / (inv_y + offset))
                    .max(MIN_PRICE_SCALE_COEFF);

                let half = self.price_scale_start_range * scale_coeff / 2.0;
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
    pub fn pointer_up(&mut self, viewport: &mut Viewport, bars: &BarArray, now_ms: f64) {
        let was_click = self.pressed && !self.drag_active;
        let zone = self.press_zone;

        // Kinetic scrolling: TOUCH ONLY, horizontal only
        if self.is_touch && self.pressed && self.drag_active && zone == HitZone::Chart {
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
        // (LWC behavior: crosshair stays until next double-tap or touchStart+exit)

        self.pressed = false;
        self.drag_active = false;

        // Double-click / double-tap detection
        if was_click && zone != HitZone::None && !self.long_press_fired {
            let dt = now_ms - self.last_click_time;
            if dt < DOUBLE_CLICK_WINDOW_MS && self.last_click_zone == zone {
                match zone {
                    HitZone::TimeAxis => {
                        let len = bars.len() as f64;
                        let visible = len.min(DEFAULT_INITIAL_VISIBLE_BARS);
                        viewport.set_range(len - visible, len);
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
                            let len = bars.len() as f64;
                            let visible = len.min(DEFAULT_INITIAL_VISIBLE_BARS);
                            viewport.set_range(len - visible, len);
                            viewport.auto_fit_price(bars);
                        }
                    }
                    _ => {}
                }
                self.last_click_time = 0.0;
                self.last_click_zone = HitZone::None;
            } else {
                self.last_click_time = now_ms;
                self.last_click_zone = zone;
            }
        }
    }

    /// Wheel event on the chart pane.
    /// LWC: deltaY → zoomTime(scrollPosition, zoomScale)
    ///       deltaX → scrollChart(deltaX * -80)
    pub fn pane_wheel(
        &mut self,
        x: f64,
        delta_x: f64,
        delta_y: f64,
        delta_mode: u32,
        pane_css_w: f64,
        viewport: &mut Viewport,
        bars: &BarArray,
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
            viewport.clamp_to_data(bars.len());
            if !viewport.price_locked {
                viewport.auto_fit_price(bars);
            }
        }

        // deltaX → scroll time
        if adj_dx.abs() > 0.001 {
            let visible_bars = viewport.end_bar - viewport.start_bar;
            let bar_spacing = pane_css_w / visible_bars;
            let scroll_bars = adj_dx * SCROLL_MULTIPLIER / bar_spacing;
            viewport.pan_clamped(scroll_bars, bars.len());
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
        viewport: &mut Viewport,
        bars: &BarArray,
    ) {
        self.pane_wheel(x, 0.0, delta_y, delta_mode, pane_css_w, viewport, bars);
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

    /// Get the current cursor style hint.
    /// Priority: drawing drag → drawing hover cursor → zone-based default.
    pub fn cursor_hint(&self) -> &'static str {
        // Drawing drag in progress — use the drag cursor
        if self.drawing_drag_active {
            return self.drawing_cursor.unwrap_or("grabbing");
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
    /// Touch-only, horizontal-only (like LWC). Returns true if gliding is still active.
    pub fn update_gliding(
        &mut self,
        pane_css_w: f64,
        _pane_css_h: f64,
        viewport: &mut Viewport,
        bars: &BarArray,
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
            viewport.pan_clamped(dx_bars, bars.len());
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
/// (matching LWC's `magnet.ts` algorithm).
fn magnet_snap_price(
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
