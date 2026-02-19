//! InteractionHandler — LWC-style pointer/wheel state machine.
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
//! ── Time Axis ──
//!   drag          → scale time (ratio from right edge, like LWC)
//!   wheel deltaY  → zoom time
//!   dbl-click     → reset time
//! ── Price Axis ──
//!   drag          → scale price (LWC inverted-Y formula)
//!   wheel deltaY  → zoom price
//!   dbl-click     → reset price

use crate::core::data::Bar;
use crate::core::viewport::Viewport;
use crate::core::renderer::traits::{CrosshairState, CrosshairMode};

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

/// Interaction state machine.
pub struct InteractionHandler {
    // ── Press / drag state ──
    pressed: bool,
    press_x: f64,
    press_y: f64,
    drag_active: bool,
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

    // ── Current hover zone (for cursor hints) ──
    current_zone: HitZone,
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
            current_zone: HitZone::None,
        }
    }

    /// Called when pointer enters a widget zone.
    pub fn pointer_enter(&mut self, zone: HitZone, crosshair: &mut CrosshairState) {
        self.current_zone = zone;
        match zone {
            HitZone::Chart => {
                crosshair.active = true;
            }
            _ => {
                crosshair.active = false;
            }
        }
    }

    /// Called when pointer leaves a widget zone.
    pub fn pointer_leave(&mut self, zone: HitZone, crosshair: &mut CrosshairState) {
        if self.current_zone == zone {
            self.current_zone = HitZone::None;
        }
        if zone == HitZone::Chart {
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
        bars: &[Bar],
        dpr: f64,
    ) {
        let pane_phys_w = pane_css_w * dpr;
        let pane_phys_h = pane_css_h * dpr;

        // Update crosshair
        if !self.pressed || self.press_zone == HitZone::Chart {
            crosshair.active = true;
            crosshair.x = x.clamp(0.0, pane_css_w);

            let bar_f = viewport.pixel_to_bar(crosshair.x * dpr, pane_phys_w);
            let snapped_idx = bar_f.round().max(0.0) as usize;
            let snapped_idx = snapped_idx.min(bars.len().saturating_sub(1));
            crosshair.bar_index = if snapped_idx < bars.len() { Some(snapped_idx) } else { None };

            match crosshair.mode {
                CrosshairMode::Magnet => {
                    if let Some(idx) = crosshair.bar_index {
                        let close_price = bars[idx].close as f64;
                        let price_frac = (close_price - viewport.price_min)
                            / (viewport.price_max - viewport.price_min);
                        crosshair.y = (1.0 - price_frac) * pane_css_h;
                        crosshair.price = close_price;
                    } else {
                        crosshair.y = y.clamp(0.0, pane_css_h);
                        crosshair.price = viewport.pixel_to_price(crosshair.y * dpr, pane_phys_h);
                    }
                }
                CrosshairMode::Normal => {
                    crosshair.y = y.clamp(0.0, pane_css_h);
                    crosshair.price = viewport.pixel_to_price(crosshair.y * dpr, pane_phys_h);
                }
            }
        }

        // Drag → scroll time + price (LWC: scrollTimeTo + scrollPriceTo)
        if self.pressed && self.press_zone == HitZone::Chart {
            let manhattan = (x - self.press_x).abs() + (y - self.press_y).abs();
            if !self.drag_active && manhattan >= CANCEL_CLICK_DISTANCE {
                self.drag_active = true;
            }
            if self.drag_active && pane_css_w > 0.0 {
                // Time scroll: same as before
                let bar_span = viewport.end_bar - viewport.start_bar;
                let dx_bars = (self.scroll_start_x - x) / pane_css_w * bar_span;
                let new_start = self.scroll_start_bar + dx_bars;
                let new_end = new_start + bar_span;
                viewport.set_range(new_start, new_end);
                viewport.clamp_to_data(bars.len());

                // Price scroll (LWC: PriceScale.scrollTo)
                // priceUnitsPerPixel = range.length / (internalHeight - 1)
                // pixelDelta = y - startY
                // priceDelta = pixelDelta * priceUnitsPerPixel
                // range.shift(priceDelta)
                if viewport.price_locked {
                    // When price is locked (user has manually scaled), scroll price too
                    let price_range = self.scroll_price_start_max - self.scroll_price_start_min;
                    if pane_css_h > 1.0 && price_range > 0.0 {
                        let price_per_px = price_range / (pane_css_h - 1.0);
                        let dy = y - self.scroll_price_start_y;
                        let price_delta = dy * price_per_px;
                        viewport.price_min = self.scroll_price_start_min + price_delta;
                        viewport.price_max = self.scroll_price_start_max + price_delta;
                    }
                } else {
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
        bars: &[Bar],
    ) {
        if self.pressed && self.press_zone == HitZone::TimeAxis {
            let manhattan = (x - self.press_x).abs();
            if !self.drag_active && manhattan >= CANCEL_CLICK_DISTANCE {
                self.drag_active = true;
            }
            if self.drag_active && pane_css_w > 1.0 {
                // LWC: newBarSpacing = savedBarSpacing * (width-x) / (width-startX)
                // visibleBars = width/barSpacing, so newVisibleBars = saved * (width-startX)/(width-x)
                let start_len = (pane_css_w - self.time_scale_start_x).clamp(1.0, pane_css_w);
                let current_len = (pane_css_w - x).clamp(1.0, pane_css_w);
                let ratio = start_len / current_len;
                let new_bar_count = (self.time_scale_start_visible_bars * ratio)
                    .clamp(2.0, bars.len() as f64 * 4.0);
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
    pub fn price_axis_pointer_move(
        &mut self,
        y: f64,
        pane_css_h: f64,
        viewport: &mut Viewport,
    ) {
        if self.pressed && self.press_zone == HitZone::PriceAxis {
            let manhattan = (y - self.press_y).abs();
            if !self.drag_active && manhattan >= CANCEL_CLICK_DISTANCE {
                self.drag_active = true;
            }
            if self.drag_active && pane_css_h > 1.0 {
                // LWC PriceScale.scaleTo:
                //   x = height - localY   (invert Y)
                //   if x < 0 { x = 0 }
                //   scaleCoeff = (startPoint + (height-1)*0.2) / (x + (height-1)*0.2)
                //   scaleCoeff = max(scaleCoeff, 0.1)
                //   newRange.scaleAroundCenter(scaleCoeff)
                let h = self.price_scale_height;
                let inv_y = (h - y).max(0.0);
                let offset = (h - 1.0) * 0.2;
                let scale_coeff = ((self.price_scale_start_y_inv + offset) / (inv_y + offset))
                    .max(0.1);

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

        match zone {
            HitZone::Chart => {
                self.scroll_start_x = x;
                self.scroll_start_bar = viewport.start_bar;
                // LWC: startScrollPrice — save start Y and price snapshot
                self.scroll_price_start_y = y;
                self.scroll_price_start_min = viewport.price_min;
                self.scroll_price_start_max = viewport.price_max;
            }
            HitZone::TimeAxis => {
                self.time_scale_start_x = x;
                self.time_scale_start_visible_bars = viewport.end_bar - viewport.start_bar;
            }
            HitZone::PriceAxis => {
                // LWC: PriceScale.startScale(x)
                // _scaleStartPoint = height - x  (inverted Y)
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
        bars: &[Bar],
        now_ms: f64,
    ) {
        let was_click = self.pressed && !self.drag_active;
        let zone = self.press_zone;

        self.pressed = false;
        self.drag_active = false;

        // Double-click detection (LWC: 500ms)
        if was_click && zone != HitZone::None {
            let dt = now_ms - self.last_click_time;
            if dt < 500.0 && self.last_click_zone == zone {
                match zone {
                    HitZone::TimeAxis => {
                        let len = bars.len() as f64;
                        let visible = len.min(200.0);
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
                        viewport.price_locked = false;
                        let len = bars.len() as f64;
                        let visible = len.min(200.0);
                        viewport.set_range(len - visible, len);
                        viewport.auto_fit_price(bars);
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
        bars: &[Bar],
    ) {
        if pane_css_w <= 0.0 { return; }

        let speed_adj = match delta_mode {
            2 => 120.0,  // DOM_DELTA_PAGE
            1 => 32.0,   // DOM_DELTA_LINE
            _ => 1.0,    // DOM_DELTA_PIXEL
        };

        let adj_dx = speed_adj * delta_x / 100.0;
        let adj_dy = -(speed_adj * delta_y / 100.0);

        // deltaY → zoom time
        if adj_dy.abs() > 0.001 {
            // LWC: zoomScale = sign(dy) * min(1, |dy|)
            let zoom_scale = adj_dy.signum() * adj_dy.abs().min(1.0);
            // LWC: newBarSpacing = barSpacing + scale * (barSpacing / 10)
            // factor on visible bars = 1 / (1 + scale/10)
            let factor = 1.0 / (1.0 + zoom_scale / 10.0);

            let scroll_position = x.clamp(0.0, pane_css_w);
            let focal_frac = scroll_position / pane_css_w;
            let focal_bar = viewport.start_bar + focal_frac * (viewport.end_bar - viewport.start_bar);

            viewport.zoom(focal_bar, factor);
            viewport.clamp_to_data(bars.len());
            if !viewport.price_locked {
                viewport.auto_fit_price(bars);
            }
        }

        // deltaX → scroll time
        // LWC: scrollChart(deltaX_css * -80) where shift = pixels / barSpacing
        if adj_dx.abs() > 0.001 {
            let visible_bars = viewport.end_bar - viewport.start_bar;
            let bar_spacing = pane_css_w / visible_bars;
            let scroll_bars = adj_dx * -80.0 / bar_spacing;
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
        bars: &[Bar],
    ) {
        self.pane_wheel(x, 0.0, delta_y, delta_mode, pane_css_w, viewport, bars);
    }

    /// Wheel event on the price axis — zoom price range.
    pub fn price_axis_wheel(
        &mut self,
        delta_y: f64,
        delta_mode: u32,
        viewport: &mut Viewport,
    ) {
        let speed_adj = match delta_mode {
            2 => 120.0,
            1 => 32.0,
            _ => 1.0,
        };
        let adj_dy = -(speed_adj * delta_y / 100.0);
        if adj_dy.abs() > 0.001 {
            let zoom_scale = adj_dy.signum() * adj_dy.abs().min(1.0);
            let factor = 1.0 / (1.0 + zoom_scale / 10.0);
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

    /// Get the current cursor style hint.
    pub fn cursor_hint(&self) -> &'static str {
        if self.drag_active {
            match self.press_zone {
                HitZone::Chart => "grabbing",
                HitZone::TimeAxis => "ew-resize",
                HitZone::PriceAxis => "ns-resize",
                _ => "default",
            }
        } else {
            match self.current_zone {
                HitZone::Chart => "crosshair",
                HitZone::TimeAxis => "ew-resize",
                HitZone::PriceAxis => "ns-resize",
                HitZone::None => "default",
            }
        }
    }
}
