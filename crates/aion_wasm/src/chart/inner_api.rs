//! `ChartInner` model/state API: series, panes, scales, coordinates, ranges. These are the
//! working halves of the thin `#[wasm_bindgen] impl AionChart` delegations in `chart.rs`.

use super::*;

impl ChartInner {
    /// Adds a series and returns its id. `kind`: 0 candles, 1 bars, 2 line, 3 area, 4 histogram.
    pub fn add_series(&mut self, kind: u8) -> u32 {
        let id = self.engine.add_series(SeriesKind::from_u8(kind));
        id as u32
    }

    pub fn add_sma(&mut self, source_id: u32, period: u32) -> u32 {
        self.engine
            .add_sma(source_id as SeriesId, period as usize)
            .map(|id| id as u32)
            .unwrap_or(u32::MAX)
    }

    pub fn add_ema(&mut self, source_id: u32, period: u32) -> u32 {
        self.engine
            .add_ema(source_id as SeriesId, period as usize)
            .map(|id| id as u32)
            .unwrap_or(u32::MAX)
    }

    pub fn add_bollinger(&mut self, source_id: u32, period: u32, deviation: f64) -> Vec<u32> {
        self.engine
            .add_bollinger(source_id as SeriesId, period as usize, deviation)
            .into_iter()
            .map(|id| id as u32)
            .collect()
    }

    /// Sets the main series' data (series 0). `times` are ascending UTC seconds.
    pub fn set_data(
        &mut self,
        times: &[f64],
        open: &[f64],
        high: &[f64],
        low: &[f64],
        close: &[f64],
    ) {
        let id = self.series[0].id;
        self.set_series_data(id as u32, times, open, high, low, close);
    }

    /// Sets a series' data by id.
    pub fn set_series_data(
        &mut self,
        id: u32,
        times: &[f64],
        open: &[f64],
        high: &[f64],
        low: &[f64],
        close: &[f64],
    ) {
        // Repair messy feed data (out-of-order, duplicate times, NaN/Inf, length mismatch) at the
        // boundary so the DataLayer's ascending-unique-finite contract always holds — a malformed
        // feed yields a warning and a rendered chart, never a wasm panic (roadmap Phase A3).
        let s = match sanitize_ohlc(times, open, high, low, close) {
            Ok(s) => s,
            Err(e) => {
                web_sys::console::warn_1(&format!("aion: set_series_data rejected — {e}").into());
                return;
            }
        };
        if !s.report.is_clean() {
            web_sys::console::warn_1(
                &format!(
                    "aion: set_series_data sanitized data — accepted {}, dropped {} invalid, {} duplicate{}",
                    s.report.accepted,
                    s.report.dropped_invalid,
                    s.report.dropped_duplicate,
                    if s.report.reordered { ", reordered" } else { "" },
                )
                .into(),
            );
        }
        self.engine
            .install_series_data(id as SeriesId, s.times, s.open, s.high, s.low, s.close);
    }

    pub fn set_series_data_typed(
        &mut self,
        id: u32,
        times: &Float64Array,
        open: &Float64Array,
        high: &Float64Array,
        low: &Float64Array,
        close: &Float64Array,
    ) {
        let s = match aion_core::model::data_validation::sanitize_ohlc_owned(
            times.to_vec(),
            open.to_vec(),
            high.to_vec(),
            low.to_vec(),
            close.to_vec(),
        ) {
            Ok(s) => s,
            Err(e) => {
                web_sys::console::warn_1(&format!("aion: set_series_data rejected — {e}").into());
                return;
            }
        };
        if !s.report.is_clean() {
            web_sys::console::warn_1(&format!("aion: set_series_data sanitized data — accepted {}, dropped {} invalid, {} duplicate{}", s.report.accepted, s.report.dropped_invalid, s.report.dropped_duplicate, if s.report.reordered { ", reordered" } else { "" }).into());
        }
        self.engine
            .install_series_data(id as SeriesId, s.times, s.open, s.high, s.low, s.close);
    }

    /// Streaming update of the main series (append new time or replace last).
    pub fn update_bar(&mut self, time: f64, open: f64, high: f64, low: f64, close: f64) {
        let id = self.series[0].id as u32;
        self.update_series_bar(id, time, open, high, low, close);
    }

    /// Streaming update of the series with `series_id` (append a new time or replace the last).
    pub fn update_series_bar(
        &mut self,
        series_id: u32,
        time: f64,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
    ) {
        // Ignore updates to an unknown series rather than corrupting the data layer.
        if !self.series.iter().any(|s| s.id == series_id as SeriesId) {
            web_sys::console::warn_1(&"aion: update_bar for unknown series id".into());
            return;
        }
        // Drop a bad tick rather than corrupting the series (roadmap Phase A3).
        if !self
            .engine
            .update_series_bar(series_id as SeriesId, time, [open, high, low, close])
        {
            web_sys::console::warn_1(&"aion: update_bar dropped a non-finite point".into());
        }
    }

    /// Sets a series' line/area color (overrides the kind default).
    pub fn set_series_color(&mut self, id: u32, r: u8, g: u8, b: u8) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.line_color = Color::rgb(r, g, b);
        }
    }

    pub fn set_series_visible(&mut self, id: u32, visible: bool) {
        self.engine.set_series_visible(id as SeriesId, visible);
    }

    /// Set candlestick/bar up & down body colors (CSS strings; empty/unparseable = keep default).
    pub fn set_series_updown_colors(&mut self, id: u32, up: &str, down: &str) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            if let Some(c) = Color::parse_css(up) {
                s.up_color = Some(c);
            }
            if let Some(c) = Color::parse_css(down) {
                s.down_color = Some(c);
            }
        }
    }

    /// Set candlestick wick colors per direction (CSS strings; empty/unparseable = keep current).
    /// Until set, each wick follows its direction's body color (LWC parity).
    pub fn set_series_wick_colors(&mut self, id: u32, up: &str, down: &str) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            if let Some(c) = Color::parse_css(up) {
                s.wick_up_color = Some(c);
            }
            if let Some(c) = Color::parse_css(down) {
                s.wick_down_color = Some(c);
            }
        }
    }

    /// Set candlestick border colors per direction (CSS strings; empty/unparseable = keep current).
    /// Until set, each border follows its direction's body color (LWC parity).
    pub fn set_series_border_colors(&mut self, id: u32, up: &str, down: &str) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            if let Some(c) = Color::parse_css(up) {
                s.border_up_color = Some(c);
            }
            if let Some(c) = Color::parse_css(down) {
                s.border_down_color = Some(c);
            }
        }
    }

    /// Toggle candlestick wick visibility (default visible; bars ignore this).
    pub fn set_series_wick_visible(&mut self, id: u32, visible: bool) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.wick_visible = Some(visible);
        }
    }

    /// Toggle candlestick body-border visibility (default visible; bars ignore this).
    pub fn set_series_border_visible(&mut self, id: u32, visible: bool) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.border_visible = Some(visible);
        }
    }

    /// Set a line/area series' stroke width (css px; non-positive ignored).
    pub fn set_series_line_width(&mut self, id: u32, width: f64) {
        if width > 0.0 {
            if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
                s.line_width = Some(width);
            }
        }
    }

    /// Set an area series' fill gradient colors (top at the line, bottom at the base; CSS strings).
    pub fn set_series_area_colors(&mut self, id: u32, top: &str, bottom: &str) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            if let Some(c) = Color::parse_css(top) {
                s.area_top_color = Some(c);
            }
            if let Some(c) = Color::parse_css(bottom) {
                s.area_bottom_color = Some(c);
            }
        }
    }

    /// Color a histogram by the main price series' up/down direction per bar (TradingView volume).
    pub fn set_series_histogram_updown(&mut self, id: u32, enabled: bool) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.histogram_updown = enabled;
        }
    }

    /// Set a line/area series' join type: 0 = simple, 1 = stepped, 2 = curved (roadmap Phase B3).
    pub fn set_series_line_type(&mut self, id: u32, line_type: u8) {
        let lt = match line_type {
            1 => LineType::WithSteps,
            2 => LineType::Curved,
            _ => LineType::Simple,
        };
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.line_type = lt;
        }
    }

    /// Toggle per-point disc markers on a line/area series (roadmap Phase B3).
    pub fn set_series_point_markers(&mut self, id: u32, visible: bool) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.point_markers = visible;
        }
    }

    /// Set a Baseline series' baseline price. `NaN` resets to auto (visible-range midpoint).
    pub fn set_series_baseline(&mut self, id: u32, price: f64) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.baseline = if price.is_finite() { Some(price) } else { None };
        }
    }

    /// Add a horizontal price line to a series; returns its id (roadmap Phase B4).
    #[allow(clippy::too_many_arguments)]
    pub fn create_price_line(
        &mut self,
        series_id: u32,
        price: f64,
        r: u8,
        g: u8,
        b: u8,
        width: u32,
        style: u8,
        title: &str,
    ) -> u32 {
        let id = self.next_price_line_id;
        self.next_price_line_id += 1;
        if let Some(s) = self
            .series
            .iter_mut()
            .find(|s| s.id == series_id as SeriesId)
        {
            s.price_lines.push(PriceLine {
                id,
                price,
                color: Color::rgb(r, g, b),
                width: width.max(1) as i32,
                style: line_style_from_u8(style),
                title: title.to_string(),
            });
        }
        id
    }

    /// Remove a price line by id (from whichever series holds it).
    pub fn remove_price_line(&mut self, id: u32) {
        for s in &mut self.series {
            s.price_lines.retain(|pl| pl.id != id);
        }
    }

    /// Replace a series' markers from a JSON array `[{time, position, shape, color, text}]`
    /// (position: above|below|inBar; shape: circle|square|arrowUp|arrowDown). Roadmap Phase B4.
    pub fn set_series_markers(&mut self, series_id: u32, json: &str) {
        let inputs: Vec<MarkerInput> = serde_json::from_str(json).unwrap_or_default();
        let markers: Vec<Marker> = inputs
            .into_iter()
            .map(|m| Marker {
                time: m.time as i64,
                position: match m.position.as_str() {
                    "below" | "belowBar" => marker_pos::BELOW,
                    "inBar" | "in" => marker_pos::IN_BAR,
                    _ => marker_pos::ABOVE,
                },
                shape: match m.shape.as_str() {
                    "square" => marker_shape::SQUARE,
                    "arrowUp" | "arrow_up" => marker_shape::ARROW_UP,
                    "arrowDown" | "arrow_down" => marker_shape::ARROW_DOWN,
                    _ => marker_shape::CIRCLE,
                },
                color: Color::parse_css(&m.color).unwrap_or(Color::rgb(0x21, 0x96, 0xf3)),
                text: m.text,
            })
            .collect();
        self.engine
            .set_series_markers(series_id as SeriesId, markers);
    }

    pub fn set_series_markers_auto_scale(&mut self, series_id: u32, enabled: bool) {
        self.engine
            .set_series_markers_auto_scale(series_id as SeriesId, enabled);
    }

    /// Toggle the pulsing last-price ring on a series (roadmap Phase B3).
    pub fn set_series_last_price_animation(&mut self, id: u32, enabled: bool) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.last_price_animation = enabled;
        }
    }

    /// Whether any series wants the last-price pulse (so the host can start/stop its rAF loop).
    pub fn wants_animation(&self) -> bool {
        self.series.iter().any(|s| s.last_price_animation)
    }

    /// Set the host animation clock (ms). The shell's rAF loop calls this then `render()`.
    pub fn set_animation_time(&mut self, t_ms: f64) {
        self.animation_time = t_ms;
    }

    /// Move a series onto its pane's bottom-band overlay scale (volume-style) and set that band's
    /// margins as fractions of the pane slot: `top` leaves that fraction above the band, `bottom`
    /// below it (e.g. top=0.8, bottom=0.0 ⇒ bottom 20%). Excludes the series from the pane's main
    /// autoscale (roadmap Phase B2).
    pub fn set_series_overlay(&mut self, id: u32, top: f64, bottom: f64) {
        let mut pane_index = 0;
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.overlay = true;
            s.left_scale = false;
            pane_index = s.pane_index;
        }
        if let Some(p) = self.panes.get_mut(pane_index) {
            p.overlay_top = top.clamp(0.0, 1.0);
            p.overlay_bottom = bottom.clamp(0.0, 1.0);
            p.overlay_scale
                .set_scale_margins(p.overlay_top, p.overlay_bottom);
            p.refresh_internal_margins();
        }
    }

    /// Move a series into pane `pane_index`, creating panes (with the given stretch factor for a
    /// newly-created last pane) as needed. Pane 0 is the top/price pane (roadmap Phase B1).
    pub fn set_series_pane(&mut self, id: u32, pane_index: usize, stretch_factor: f64) {
        while self.panes.len() <= pane_index {
            let mut p = Pane::new();
            p.stretch_factor = stretch_factor.max(0.01);
            self.panes.push(p);
        }
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.pane_index = pane_index;
        }
    }

    /// Number of stacked panes.
    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    /// CSS Y of each pane boundary (top edge of panes 1..n), for separator hit-testing by the host.
    /// Reflects the last layout pass.
    pub fn pane_separator_ys(&self) -> Vec<f64> {
        self.panes.iter().skip(1).map(|p| p.top).collect()
    }

    /// Drag the separator below pane `i` by `delta_css` (positive grows pane `i`, shrinks `i+1`),
    /// keeping both at least a minimum height. Freezes current heights as stretch factors so the
    /// other panes hold their size, then re-lays out (roadmap Phase B1).
    pub fn drag_pane_separator(&mut self, i: usize, delta_css: f64) {
        if i + 1 >= self.panes.len() {
            return;
        }
        const MIN_PANE_H: f64 = 24.0;
        for p in &mut self.panes {
            p.stretch_factor = p.height.max(1.0);
        }
        let top = self.panes[i].height;
        let bot = self.panes[i + 1].height;
        let new_top = (top + delta_css).clamp(MIN_PANE_H, (top + bot - MIN_PANE_H).max(MIN_PANE_H));
        let actual = new_top - top;
        self.panes[i].stretch_factor = new_top;
        self.panes[i + 1].stretch_factor = bot - actual;
    }

    /// CSS height of pane `i` from the last layout pass.
    pub fn pane_height(&self, i: usize) -> f64 {
        self.panes.get(i).map(|p| p.height).unwrap_or(0.0)
    }

    /// Relative stretch factor of pane `i`.
    pub fn pane_stretch(&self, i: usize) -> f64 {
        self.panes.get(i).map(|p| p.stretch_factor).unwrap_or(1.0)
    }

    /// Set pane `i`'s stretch factor (its share of the content height relative to the others).
    pub fn set_pane_stretch(&mut self, i: usize, factor: f64) {
        if let Some(p) = self.panes.get_mut(i) {
            p.stretch_factor = factor.max(0.01);
            if self.css_width > 0.0 {
                self.recompute_layout(false);
            }
        }
    }

    /// Resize pane `i` to `height_css`, absorbing the delta from its neighbour below (or above for
    /// the last pane) — the same freeze-and-redistribute behavior as dragging its separator.
    pub fn set_pane_height(&mut self, i: usize, height_css: f64) {
        if i >= self.panes.len() {
            return;
        }
        let current = self.panes[i].height;
        let delta = height_css - current;
        if i + 1 < self.panes.len() {
            self.drag_pane_separator(i, delta);
        } else if i > 0 {
            // last pane: move the separator above it the other way to grow/shrink it
            self.drag_pane_separator(i - 1, -delta);
        }
        if self.css_width > 0.0 {
            self.recompute_layout(false);
        }
    }

    /// 0 = candlestick, 1 = OHLC bars, 2 = line, 3 = area, 4 = histogram (sets the main series).
    pub fn set_series_type(&mut self, kind: u8) {
        self.series[0].kind = SeriesKind::from_u8(kind);
    }

    pub fn set_time_visible(&mut self, visible: bool) {
        self.time_visible = visible;
    }

    /// 0 = normal, 1 = magnet (LWC default), 2 = hidden, 3 = magnet OHLC.
    pub fn set_crosshair_mode(&mut self, mode: u8) {
        self.crosshair_mode = crosshair_mode_from_u8(mode);
        // keep the options store consistent so `options()` reflects it
        self.options.apply(&aion_core::options::patch(
            "crosshair",
            serde_json::json!({ "mode": mode }),
        ));
    }

    /// Deep-merge a JSON options patch and apply the runtime-affecting fields (crosshair mode).
    /// Colors (grid/crosshair/background) are read from the store during `render`. Call `render()`
    /// after to repaint (roadmap Phase A2).
    pub fn apply_options(&mut self, patch_json: &str) {
        if let Err(e) = self.options.apply_str(patch_json) {
            web_sys::console::warn_1(
                &format!("aion: apply_options ignored malformed patch — {e}").into(),
            );
            return;
        }
        // Re-derive runtime state that isn't read straight from the store each frame.
        self.crosshair_mode = crosshair_mode_from_u8(self.options.get().crosshair.mode);
    }

    /// Current options as a JSON string (round-trips the deep-merged state back to JS).
    pub fn options_json(&self) -> String {
        self.options.value().to_string()
    }

    /// Typed snapshot of the current options for the render path.
    pub(super) fn opts(&self) -> ChartOptions {
        self.options.get()
    }

    pub fn resize(&mut self, css_width: f64, css_height: f64, dpr: f64) {
        self.css_width = css_width;
        self.css_height = css_height;
        self.dpr = dpr;
        let bitmap_w = (css_width * dpr).round().max(1.0) as u32;
        let bitmap_h = (css_height * dpr).round().max(1.0) as u32;
        self.bitmap_w = bitmap_w;
        self.bitmap_h = bitmap_h;
        if let Some(gfx) = self.gfx.as_mut() {
            gfx.config.width = bitmap_w;
            gfx.config.height = bitmap_h;
            gfx.surface.configure(&gfx.device, &gfx.config);
        }
        // Update geometry eagerly so fit_content/zoom/scroll called before the next render
        // (and the price_axis_width getter) see the new pane size, not a stale one.
        self.recompute_layout(true);
    }

    /// Negotiates the price-axis width against its labels and sets the time-scale width /
    /// price-scale height accordingly. Idempotent; called on resize, data change, and render.
    /// (The axis labels depend only on the price range, so one refinement pass converges.)
    pub(super) fn recompute_layout(&mut self, allow_axis_shrink: bool) {
        let content_h = (self.css_height - TIME_AXIS_HEIGHT).max(1.0);
        self.engine.layout_panes(content_h);
        let options = self.opts();
        let measured_axis_w = if options.right_price_scale.visible {
            self.compute_price_axis_width(PriceScaleTarget::Right)
        } else {
            0.0
        };
        let measured_left_axis_w = if options.left_price_scale.visible {
            self.compute_price_axis_width(PriceScaleTarget::Left)
        } else {
            0.0
        };
        let mut axis_w = if options.right_price_scale.visible {
            negotiated_axis_width(self.axis_w, measured_axis_w, allow_axis_shrink)
        } else {
            0.0
        };
        let mut left_axis_w = if options.left_price_scale.visible {
            negotiated_axis_width(self.left_axis_w, measured_left_axis_w, allow_axis_shrink)
        } else {
            0.0
        };
        for _ in 0..2 {
            let pane_w = (self.css_width - left_axis_w - axis_w).max(1.0);
            self.pane_left = left_axis_w;
            self.left_axis_w = left_axis_w;
            self.axis_w = axis_w;
            self.time_scale.set_width(pane_w);
            self.engine.autoscale_visible();
            let measured_new_w = if options.right_price_scale.visible {
                self.compute_price_axis_width(PriceScaleTarget::Right)
            } else {
                0.0
            };
            let measured_new_left_w = if options.left_price_scale.visible {
                self.compute_price_axis_width(PriceScaleTarget::Left)
            } else {
                0.0
            };
            let new_w = if options.right_price_scale.visible {
                negotiated_axis_width(axis_w, measured_new_w, allow_axis_shrink)
            } else {
                0.0
            };
            let new_left_w = if options.left_price_scale.visible {
                negotiated_axis_width(left_axis_w, measured_new_left_w, allow_axis_shrink)
            } else {
                0.0
            };
            if new_w == axis_w && new_left_w == left_axis_w {
                break;
            }
            axis_w = new_w;
            left_axis_w = new_left_w;
        }
        self.pane_left = left_axis_w;
        self.left_axis_w = left_axis_w;
        self.pane_w = (self.css_width - left_axis_w - axis_w).max(1.0);
        self.pane_h = content_h;
        self.axis_w = axis_w;
    }

    // --- gestures ---

    pub fn zoom(&mut self, x_css: f64, scale: f64) {
        let x = x_css.max(1.0).min(self.time_scale.width());
        self.time_scale.zoom(x, scale);
    }
    pub fn scroll_start(&mut self, x_css: f64) {
        self.time_scale.start_scroll(x_css);
    }
    pub fn scroll_move(&mut self, x_css: f64) {
        self.time_scale.scroll_to(x_css);
    }
    pub fn scroll_end(&mut self) {
        self.time_scale.end_scroll();
    }
    pub fn fit_content(&mut self) {
        self.engine.fit_content();
    }
    pub fn scroll_position(&self) -> f64 {
        self.engine.scroll_position()
    }
    pub fn scroll_to_position(&mut self, position: f64) {
        self.engine.scroll_to_position(position);
    }
    pub fn scroll_to_real_time(&mut self) {
        self.engine.scroll_to_real_time();
    }
    pub fn reset_time_scale(&mut self) {
        self.engine.reset_time_scale();
    }
    pub fn time_scale_width(&self) -> f64 {
        self.time_scale.width()
    }
    pub fn time_scale_height(&self) -> f64 {
        if self.time_visible {
            TIME_AXIS_HEIGHT
        } else {
            0.0
        }
    }
    pub fn price_scale_width(&self, pane: usize, target: u8) -> f64 {
        if pane >= self.panes.len() {
            return 0.0;
        }
        match price_scale_target_from_u8(target) {
            PriceScaleTarget::Right => self.axis_w,
            PriceScaleTarget::Left => self.left_axis_w,
            PriceScaleTarget::Overlay => 0.0,
        }
    }
    pub fn price_scale_visible_range(&self, pane: usize, target: u8) -> Vec<f64> {
        self.engine
            .price_scale_visible_range_for(pane, price_scale_target_from_u8(target))
            .map(|(from, to)| vec![from, to])
            .unwrap_or_default()
    }
    pub fn set_price_scale_visible_range(&mut self, pane: usize, target: u8, from: f64, to: f64) {
        self.engine.set_price_scale_visible_range_for(
            pane,
            price_scale_target_from_u8(target),
            from,
            to,
        );
    }
    pub fn price_scale_auto_scale(&self, pane: usize, target: u8) -> Option<bool> {
        self.engine
            .price_scale_auto_scale_for(pane, price_scale_target_from_u8(target))
    }
    pub fn set_price_scale_auto_scale(&mut self, pane: usize, target: u8, enabled: bool) {
        self.engine.set_price_scale_auto_scale_for(
            pane,
            price_scale_target_from_u8(target),
            enabled,
        );
    }
    pub fn price_scale_inverted(&self, pane: usize, target: u8) -> Option<bool> {
        self.engine
            .price_scale_inverted_for(pane, price_scale_target_from_u8(target))
    }
    pub fn set_price_scale_inverted(&mut self, pane: usize, target: u8, inverted: bool) {
        self.engine.set_price_scale_inverted_for(
            pane,
            price_scale_target_from_u8(target),
            inverted,
        );
    }
    pub fn price_scale_margins(&self, pane: usize, target: u8) -> Vec<f64> {
        self.engine
            .price_scale_margins_for(pane, price_scale_target_from_u8(target))
            .map(|(top, bottom)| vec![top, bottom])
            .unwrap_or_default()
    }
    pub fn set_price_scale_margins(&mut self, pane: usize, target: u8, top: f64, bottom: f64) {
        self.engine.set_price_scale_margins_for(
            pane,
            price_scale_target_from_u8(target),
            top,
            bottom,
        );
    }
    pub fn price_scale_mode(&self, pane: usize, target: u8) -> Option<u8> {
        self.engine
            .price_scale_mode_for(pane, price_scale_target_from_u8(target))
            .map(price_scale_mode_to_u8)
    }
    pub fn set_price_scale_mode(&mut self, pane: usize, target: u8, mode: u8) {
        self.engine.set_price_scale_mode_for(
            pane,
            price_scale_target_from_u8(target),
            price_scale_mode_from_u8(mode),
        );
        // A mode change is a full layout invalidation in LWC: label formatting can become wider
        // (percentage) or narrower (indexed/normal), so the grow-fast/shrink-on-full-layout axis
        // policy must be allowed to renegotiate in both directions immediately.
        self.recompute_layout(true);
    }
    pub fn series_pane_index(&self, id: u32) -> Option<usize> {
        self.engine
            .series_price_scale(id as usize)
            .map(|(pane, _)| pane)
    }
    pub fn series_is_overlay(&self, id: u32) -> Option<bool> {
        self.engine
            .series_price_scale(id as usize)
            .map(|(_, target)| target == PriceScaleTarget::Overlay)
    }
    pub fn series_price_scale_id(&self, id: u32) -> Option<u8> {
        self.engine
            .series_price_scale(id as usize)
            .map(|(_, target)| price_scale_target_to_u8(target))
    }
    pub fn set_series_price_scale(&mut self, id: u32, target: u8) {
        self.engine
            .set_series_price_scale(id as usize, price_scale_target_from_u8(target));
        self.recompute_layout(true);
    }
    pub fn series_price_to_coordinate(&self, id: u32, price: f64) -> Option<f64> {
        self.engine.series_price_to_coordinate(id as usize, price)
    }
    pub fn series_coordinate_to_price(&self, id: u32, coordinate: f64) -> Option<f64> {
        self.engine
            .series_coordinate_to_price(id as usize, coordinate)
    }
    pub fn series_kind(&self, id: u32) -> Option<u8> {
        self.engine.series_kind(id as usize).map(SeriesKind::to_u8)
    }
    pub fn series_data_by_index(&self, id: u32, index: f64, mismatch: i8) -> Vec<f64> {
        if !index.is_finite() || index.fract() != 0.0 {
            return Vec::new();
        }
        self.engine
            .series_data_by_index(
                id as usize,
                index as i64,
                mismatch_direction_from_i8(mismatch),
            )
            .map(|point| {
                vec![
                    point.time as f64,
                    point.open,
                    point.high,
                    point.low,
                    point.close,
                ]
            })
            .unwrap_or_default()
    }
    pub fn series_data(&self, id: u32) -> Vec<f64> {
        let points = self.engine.series_data(id as usize);
        let mut output = Vec::with_capacity(points.len() * 5);
        for point in points {
            output.extend_from_slice(&[
                point.time as f64,
                point.open,
                point.high,
                point.low,
                point.close,
            ]);
        }
        output
    }
    pub fn series_bars_in_logical_range(&self, id: u32, from: f64, to: f64) -> Vec<f64> {
        self.engine
            .series_bars_in_logical_range(id as usize, from, to)
            .map(|info| {
                let mut output = vec![info.bars_before, info.bars_after];
                if let (Some(from), Some(to)) = (info.from, info.to) {
                    output.extend_from_slice(&[from as f64, to as f64]);
                }
                output
            })
            .unwrap_or_default()
    }
    pub fn set_crosshair(&mut self, x_css: f64, y_css: f64) {
        self.crosshair = Some((x_css, y_css));
    }
    pub fn clear_crosshair(&mut self) {
        self.crosshair = None;
    }
    pub fn price_axis_width(&self) -> f64 {
        self.axis_w
    }
    pub fn pane_left(&self) -> f64 {
        self.pane_left
    }

    // --- coordinate & logical-range API (roadmap Phase A4) ---
    //
    // Reflects the state of the last render (scale height/width, price range). All coordinates
    // are media (CSS) pixels relative to the pane origin, matching the pointer coords JS passes
    // to `set_crosshair`. `None`/empty means the query falls off the chart or there is no data.

    /// Y (CSS px) for a price on the active price scale, or `None` if the scale has no range yet.
    /// In percentage/indexed modes the price is its own base value (as in the render path).
    pub fn price_to_coordinate(&self, price: f64) -> Option<f64> {
        self.engine
            .series_price_to_coordinate(self.series[0].id, price)
    }

    /// Price for a Y (CSS px), or `None` if the scale has no range yet.
    pub fn coordinate_to_price(&self, y_css: f64) -> Option<f64> {
        self.engine
            .series_coordinate_to_price(self.series[0].id, y_css)
    }

    /// X (CSS px) for a UTC-seconds timestamp that sits exactly on a data point, else `None`
    /// (mirrors LWC `timeToCoordinate`, which does not snap to the nearest bar).
    pub fn time_to_coordinate(&self, time: f64) -> Option<f64> {
        self.engine.time_to_coordinate(time)
    }

    /// UTC-seconds timestamp of the data point nearest to X (CSS px), or `None` if X maps outside
    /// the data range (mirrors LWC `coordinateToTime`).
    pub fn coordinate_to_time(&self, x_css: f64) -> Option<f64> {
        self.engine.coordinate_to_time(x_css)
    }

    /// Integer logical bar owning an X coordinate, or `None` when there is no data. May be negative
    /// or beyond the last bar, matching LWC's public `coordinateToLogical`.
    pub fn coordinate_to_logical(&self, x_css: f64) -> Option<f64> {
        self.engine.coordinate_to_logical(x_css)
    }

    pub fn logical_to_coordinate(&self, logical: f64) -> Option<f64> {
        self.engine.logical_to_coordinate(logical)
    }

    pub fn time_to_index(&self, time: f64, find_nearest: bool) -> Option<i64> {
        self.engine.time_to_index(time, find_nearest)
    }

    /// Per-series values at the bar under an X coordinate, flattened as groups of five:
    /// `[series_id, open, high, low, close, ...]`. Only series that actually have a point at that
    /// bar are included (single-value series report the value in all four slots). Empty when the
    /// cursor is off the data. Backs the façade's `seriesData` map for crosshair/click events.
    pub fn hover_data(&self, x_css: f64) -> Vec<f64> {
        use aion_core::model::plot_list::MismatchDirection;
        let n = self.data.merged_times().len() as i64;
        if n == 0 {
            return Vec::new();
        }
        let index = self.time_scale.coordinate_to_index(x_css);
        if index < 0 || index >= n {
            return Vec::new();
        }
        let mut out = Vec::new();
        for s in &self.series {
            let plot = self.data.plot(s.id);
            if let Some(row) = plot.search(index, MismatchDirection::None) {
                out.push(s.id as f64);
                out.push(plot.value_at(row, PlotValueIndex::Open));
                out.push(plot.value_at(row, PlotValueIndex::High));
                out.push(plot.value_at(row, PlotValueIndex::Low));
                out.push(plot.value_at(row, PlotValueIndex::Close));
            }
        }
        out
    }

    /// Visible window in logical (bar) units as `[from, to]`, or empty when there is no data.
    pub fn visible_logical_range(&self) -> Vec<f64> {
        match self.engine.visible_logical_range() {
            Some((from, to)) => vec![from, to],
            None => Vec::new(),
        }
    }

    /// Set the visible window in logical (bar) units. No-op if `from > to`. Call `render()` after.
    pub fn set_visible_logical_range(&mut self, from: f64, to: f64) {
        self.engine.set_visible_logical_range(from, to);
    }

    /// Visible window as `[from_time, to_time]` UTC seconds (data points nearest each edge), or
    /// empty when there is no data.
    pub fn visible_time_range(&self) -> Vec<f64> {
        self.engine
            .visible_time_range()
            .map(|(from, to)| vec![from, to])
            .unwrap_or_default()
    }

    /// Set the visible window to span the data points bracketing `[from_time, to_time]` (UTC
    /// seconds). No-op if the times are reversed or there is no data. Call `render()` after.
    pub fn set_visible_time_range(&mut self, from_time: f64, to_time: f64) {
        self.engine.set_visible_time_range(from_time, to_time);
    }
}
