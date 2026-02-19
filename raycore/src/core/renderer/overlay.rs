//! OverlayRenderer — Canvas2D overlay for axes, crosshair, and watermark.
//!
//! Sits on the TOP canvas (z-index:2), above candles and grid.
//! Draws: Y-axis (price), X-axis (time), crosshair + labels, watermark.
//! All dimensions/fonts/paddings match LWC exactly.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, CanvasRenderingContext2d};
use crate::core::data::Bar;
use crate::core::viewport::Viewport;
use crate::core::renderer::traits::{ChartStyle, CrosshairState, TickMark};
use crate::core::renderer::series::ChartLayout;
use crate::core::formatters::{format_price, format_timestamp};

pub struct OverlayRenderer {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    physical_width: u32,
    physical_height: u32,
    dpr: f64,
}

// ── Helpers ──────────────────────────────────────────────────────────────────

#[inline]
fn rgba(c: &[f32; 4]) -> String {
    format!(
        "rgba({},{},{},{})",
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        c[3]
    )
}

impl OverlayRenderer {
    pub fn new(canvas: HtmlCanvasElement, dpr: f64) -> Result<Self, String> {
        let ctx = canvas
            .get_context("2d")
            .map_err(|e| format!("overlay get_context('2d') failed: {:?}", e))?
            .ok_or("overlay get_context('2d') returned None")?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "overlay context is not CanvasRenderingContext2d")?;

        ctx.set_image_smoothing_enabled(false);
        let pw = canvas.width();
        let ph = canvas.height();
        Ok(Self { canvas, ctx, physical_width: pw, physical_height: ph, dpr })
    }

    pub fn resize(&mut self, pw: u32, ph: u32, dpr: f64) {
        self.physical_width = pw;
        self.physical_height = ph;
        self.dpr = dpr;
        self.canvas.set_width(pw.max(1));
        self.canvas.set_height(ph.max(1));
        self.ctx.set_image_smoothing_enabled(false);
    }

    /// Measure the maximum tick label width (physical px) for the given ticks.
    /// Used to compute dynamic Y-axis width.
    pub fn measure_max_tick_width(&self, style: &ChartStyle, ticks: &[TickMark]) -> f64 {
        self.ctx.set_font(&style.axis_font(self.dpr));
        let mut max_w: f64 = 0.0;
        for t in ticks {
            if let Ok(m) = self.ctx.measure_text(&t.label) {
                let w = m.width();
                if w > max_w { max_w = w; }
            }
        }
        max_w
    }

    /// Render axes, crosshair, watermark. Receives pre-computed ticks and layout.
    pub fn render(
        &self,
        bars: &[Bar],
        vp: &Viewport,
        style: &ChartStyle,
        crosshair: &CrosshairState,
        layout: &ChartLayout,
        y_ticks: &[TickMark],
        x_ticks: &[TickMark],
    ) {
        let pw = self.physical_width as f64;
        let ph = self.physical_height as f64;
        self.ctx.clear_rect(0.0, 0.0, pw, ph);

        self.draw_watermark(style, layout);
        self.draw_y_axis(style, layout, y_ticks);
        self.draw_x_axis(style, layout, x_ticks);
        self.draw_crosshair(crosshair, bars, vp, style, layout);
    }

    // ── Y-Axis (price) — LWC-matching ────────────────────────────────────

    fn draw_y_axis(
        &self,
        s: &ChartStyle,
        layout: &ChartLayout,
        ticks: &[TickMark],
    ) {
        let dpr = self.dpr;
        let ax = layout.chart_w; // left edge of price axis (physical)
        let y_axis_w = layout.total_w - layout.chart_w;
        let data_area_h = layout.candle_h + layout.vol_h;

        // Background (same as chart bg — solid fill)
        self.ctx.set_fill_style_str(&rgba(&s.bg_color));
        self.ctx.fill_rect(ax, 0.0, y_axis_w, self.physical_height as f64);

        // Border line (1px at left edge of axis, full height)
        let border_size = (s.axis_border_size as f64 * dpr).max(1.0).floor();
        self.ctx.set_fill_style_str(&rgba(&s.axis_border_color));
        // LWC: for right price scale, border is at the LEFT of the price axis area
        self.ctx.fill_rect(ax, 0.0, border_size, data_area_h);

        // Tick marks (small horizontal bars at the border edge)
        let tick_length = (s.axis_tick_length as f64 * dpr).round();
        let tick_height = (1.0 * dpr).floor().max(1.0);
        let tick_offset = (dpr * 0.5).floor();
        // LWC right axis: ticks start at (chart_w - tickLength) extending to chart_w
        // But since border is at ax, ticks go from ax to ax + tickLength
        let tick_left = ax;

        self.ctx.set_fill_style_str(&rgba(&s.axis_border_color));
        for t in ticks {
            if t.pixel < 0.0 || t.pixel > data_area_h { continue; }
            let y = (t.pixel * dpr / dpr).round() * 1.0; // already in physical px
            self.ctx.fill_rect(
                tick_left,
                y - tick_offset,
                tick_length,
                tick_height,
            );
        }

        // Tick labels (text in media/CSS space — we scale manually)
        let font = s.axis_font(dpr);
        self.ctx.set_font(&font);
        self.ctx.set_fill_style_str(&rgba(&s.axis_text_color));
        self.ctx.set_text_align("left");
        self.ctx.set_text_baseline("middle");

        // LWC right axis text position: tickMarkLeftX + tickLength + paddingInner
        // Since our ticks start at ax, text starts at ax + tickLength + paddingInner
        let padding_inner = s.price_axis_padding_inner() * dpr;
        let text_x = ax + tick_length + padding_inner;

        for t in ticks {
            if t.pixel < 0.0 || t.pixel > data_area_h { continue; }
            let _ = self.ctx.fill_text(&t.label, text_x, t.pixel);
        }
    }

    // ── X-Axis (time) — LWC-matching ─────────────────────────────────────

    fn draw_x_axis(
        &self,
        s: &ChartStyle,
        layout: &ChartLayout,
        ticks: &[TickMark],
    ) {
        let dpr = self.dpr;
        let ay = layout.candle_h + layout.vol_h; // top of time axis (physical)
        let x_axis_h = layout.x_axis_h;

        // Background
        self.ctx.set_fill_style_str(&rgba(&s.bg_color));
        self.ctx.fill_rect(0.0, ay, layout.total_w, x_axis_h);

        // Border line (1px at top of time axis, full width)
        let border_size = (s.axis_border_size as f64 * dpr).max(1.0).floor();
        self.ctx.set_fill_style_str(&rgba(&s.axis_border_color));
        self.ctx.fill_rect(0.0, ay, layout.chart_w, border_size);

        // Tick marks (small vertical bars below the border)
        let tick_length = (s.axis_tick_length as f64 * dpr).round();
        let tick_width = (1.0 * dpr).floor().max(1.0);
        let tick_offset = (dpr * 0.5).floor();

        self.ctx.set_fill_style_str(&rgba(&s.axis_border_color));
        for t in ticks {
            if t.pixel < 0.0 || t.pixel > layout.chart_w { continue; }
            let x = t.pixel.round();
            self.ctx.fill_rect(
                x - tick_offset,
                ay,
                tick_width,
                tick_length,
            );
        }

        // Tick labels
        let font = s.axis_font(dpr);
        self.ctx.set_font(&font);
        self.ctx.set_fill_style_str(&rgba(&s.axis_text_color));
        self.ctx.set_text_align("center");
        self.ctx.set_text_baseline("middle");

        // LWC: text y = border + tickLength + paddingTop + fontSize/2
        let padding_top = s.time_axis_padding_top() * dpr;
        let fs = s.font_size as f64 * dpr;
        let text_y = ay + border_size + tick_length + padding_top + fs / 2.0;

        for t in ticks {
            if t.pixel < 0.0 || t.pixel > layout.chart_w { continue; }
            let _ = self.ctx.fill_text(&t.label, t.pixel, text_y);
        }
    }

    // ── Crosshair — LWC-matching ─────────────────────────────────────────

    fn draw_crosshair(
        &self,
        ch: &CrosshairState,
        bars: &[Bar],
        vp: &Viewport,
        s: &ChartStyle,
        layout: &ChartLayout,
    ) {
        if !ch.active { return; }

        let dpr = self.dpr;
        let mx = ch.x * dpr;
        let my = ch.y * dpr;
        let data_area_h = layout.candle_h + layout.vol_h;

        if mx < 0.0 || mx > layout.chart_w || my < 0.0 || my > data_area_h { return; }

        // ── Dashed crosshair lines (LWC: LargeDashed = 6*lineWidth, 6*lineWidth) ──
        let line_w = (1.0 * dpr).floor().max(1.0);
        let dash_len = 6.0 * line_w;
        let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };

        self.ctx.set_stroke_style_str(&rgba(&s.crosshair_color));
        self.ctx.set_line_width(line_w);
        self.ctx.set_line_cap("butt");
        let _ = self.ctx.set_line_dash(&js_sys::Array::of2(
            &JsValue::from(dash_len),
            &JsValue::from(dash_len),
        ));

        // Horizontal line
        let hy = my.round() + correction;
        self.ctx.begin_path();
        self.ctx.move_to(0.0, hy);
        self.ctx.line_to(layout.chart_w, hy);
        self.ctx.stroke();

        // Vertical line
        let vx = mx.round() + correction;
        self.ctx.begin_path();
        self.ctx.move_to(vx, 0.0);
        self.ctx.line_to(vx, data_area_h);
        self.ctx.stroke();

        // Reset dash
        let _ = self.ctx.set_line_dash(&js_sys::Array::new());

        // ── Price label on Y-axis (rounded rect) ──
        let price = vp.price_min + (1.0 - my / layout.candle_h) * (vp.price_max - vp.price_min);
        let step = (vp.price_max - vp.price_min) / 10.0;
        let price_lbl = format_price(price, step.max(0.0001));

        let font = s.axis_font(dpr);
        self.ctx.set_font(&font);

        let fs = s.font_size as f64 * dpr;
        let padding_inner = s.price_axis_padding_inner() * dpr;
        let padding_outer = s.price_axis_padding_outer() * dpr;
        let tick_length = (s.axis_tick_length as f64 * dpr).round();
        let border_size = (s.axis_border_size as f64 * dpr).max(1.0).floor();
        let extra_pad = s.crosshair_label_extra_padding() * dpr;
        let padding_top = s.price_axis_padding_tb() * dpr + extra_pad;
        let padding_bottom = padding_top;

        let text_w = self.ctx.measure_text(&price_lbl).map(|m| m.width()).unwrap_or(40.0).ceil();
        let label_h = fs + padding_top + padding_bottom;
        let label_w = border_size + padding_inner + padding_outer + text_w + tick_length;

        // LWC: label height parity must match tick height parity
        let tick_h_i = (dpr.floor().max(1.0)) as i32;
        let mut label_h_i = label_h.round() as i32;
        if label_h_i % 2 != tick_h_i % 2 { label_h_i += 1; }
        let label_h = label_h_i as f64;

        let y_mid = my.round() - (dpr * 0.5).floor();
        let y_top = (y_mid + tick_h_i as f64 / 2.0 - label_h / 2.0).floor();
        let label_w_r = label_w.round();
        let ax = layout.chart_w;
        let radius = (2.0 * dpr).round();

        // Draw rounded rect (right side: left-top and left-bottom corners rounded)
        self.ctx.set_fill_style_str(&rgba(&s.crosshair_label_bg));
        self.ctx.begin_path();
        // Start top-right, go clockwise
        let x1 = ax;
        let x2 = ax + label_w_r;
        let y1 = y_top;
        let y2 = y_top + label_h;

        self.ctx.move_to(x1 + radius, y1);
        self.ctx.line_to(x2, y1);
        self.ctx.line_to(x2, y2);
        self.ctx.line_to(x1 + radius, y2);
        let _ = self.ctx.arc_to(x1, y2, x1, y2 - radius, radius);
        self.ctx.line_to(x1, y1 + radius);
        let _ = self.ctx.arc_to(x1, y1, x1 + radius, y1, radius);
        self.ctx.close_path();
        self.ctx.fill();

        // Draw tick mark on label (small colored rectangle at the border edge)
        let tick_h_px = dpr.floor().max(1.0);
        let tick_off = (dpr * 0.5).floor();
        self.ctx.set_fill_style_str(&rgba(&s.crosshair_label_text));
        self.ctx.fill_rect(ax, y_mid - tick_off, tick_length, tick_h_px);

        // Price text
        self.ctx.set_fill_style_str(&rgba(&s.crosshair_label_text));
        self.ctx.set_text_align("left");
        self.ctx.set_text_baseline("middle");
        let text_x = ax + tick_length + padding_inner;
        let text_y = (y1 + y2) / 2.0;
        let _ = self.ctx.fill_text(&price_lbl, text_x, text_y);

        // ── Time label on X-axis (rounded rect, bottom corners rounded) ──
        let bar_f = vp.start_bar + (mx / layout.chart_w) * (vp.end_bar - vp.start_bar);
        let bar_i = bar_f.round() as usize;
        let bar_lbl = if bar_i < bars.len() && bars[bar_i].timestamp > 0 {
            format_timestamp(bars[bar_i].timestamp)
        } else {
            format!("{}", bar_i)
        };

        let text_w_t = self.ctx.measure_text(&bar_lbl).map(|m| m.width()).unwrap_or(60.0).round();
        let h_margin = s.time_axis_padding_horizontal() * dpr;
        let label_w_t = text_w_t + 2.0 * h_margin;
        let label_half = label_w_t / 2.0;

        let time_scale_w = layout.chart_w / dpr; // CSS px
        let mut coord = mx / dpr; // CSS px
        let mut lx1 = (coord - label_half).floor() + 0.5;

        // Clamp to time scale bounds (LWC behavior)
        if lx1 < 0.0 {
            coord += -lx1;
            lx1 = (coord - label_half).floor() + 0.5;
        } else if lx1 + label_w_t / dpr > time_scale_w {
            coord -= (lx1 + label_w_t / dpr) - time_scale_w;
            lx1 = (coord - label_half).floor() + 0.5;
        }

        let lx2 = lx1 + label_w_t / dpr;

        // LWC time label height = borderSize + tickLength + paddingTop + fontSize + paddingBottom
        let t_border = s.axis_border_size as f64;
        let t_tick = s.axis_tick_length as f64;
        let t_pad_top = s.time_axis_padding_top();
        let t_pad_bottom = s.time_axis_padding_bottom();
        let t_fs = s.font_size as f64;
        let label_h_t = (t_border + t_tick + t_pad_top + t_fs + t_pad_bottom).ceil();

        let ty1 = data_area_h / dpr; // CSS Y of time axis top
        let ty2 = ty1 + label_h_t;

        // Scale to bitmap coords
        let bx1 = (lx1 * dpr).round();
        let by1 = (ty1 * dpr).round();
        let bx2 = (lx2 * dpr).round();
        let by2 = (ty2 * dpr).round();
        let r_scaled = (2.0 * dpr).round();

        // Rounded rect: top corners square, bottom corners rounded
        self.ctx.set_fill_style_str(&rgba(&s.crosshair_label_bg));
        self.ctx.begin_path();
        self.ctx.move_to(bx1, by1);
        self.ctx.line_to(bx2, by1);
        self.ctx.line_to(bx2, by2 - r_scaled);
        let _ = self.ctx.arc_to(bx2, by2, bx2 - r_scaled, by2, r_scaled);
        self.ctx.line_to(bx1 + r_scaled, by2);
        let _ = self.ctx.arc_to(bx1, by2, bx1, by2 - r_scaled, r_scaled);
        self.ctx.line_to(bx1, by1);
        self.ctx.close_path();
        self.ctx.fill();

        // Time tick mark
        let tick_w_px = dpr.floor().max(1.0);
        let tick_off_x = (dpr * 0.5).floor();
        let tick_x = (coord * dpr).round();
        let tick_top = by1;
        let tick_bottom = (tick_top + s.axis_tick_length as f64 * dpr).round();
        self.ctx.set_fill_style_str(&rgba(&s.crosshair_label_text));
        self.ctx.fill_rect(tick_x - tick_off_x, tick_top, tick_w_px, tick_bottom - tick_top);

        // Time text (media coords)
        let text_y_t = ty1 + t_border + t_tick + t_pad_top + t_fs / 2.0;
        // Scale text_y to physical
        let text_y_phys = text_y_t * dpr;
        self.ctx.set_fill_style_str(&rgba(&s.crosshair_label_text));
        self.ctx.set_text_align("left");
        self.ctx.set_text_baseline("middle");
        let text_x_t = bx1 + h_margin;
        let _ = self.ctx.fill_text(&bar_lbl, text_x_t, text_y_phys);
    }

    // ── Watermark ───────────────────────────────────────────────────────

    fn draw_watermark(&self, s: &ChartStyle, layout: &ChartLayout) {
        let fs = s.font_size_watermark as f64 * self.dpr;
        self.ctx.set_fill_style_str(&rgba(&s.watermark_color));
        self.ctx.set_font(&format!("bold {}px {}", fs, s.font_family));
        self.ctx.set_text_align("center");
        self.ctx.set_text_baseline("middle");
        let _ = self.ctx.fill_text("RayCharts", layout.chart_w / 2.0, layout.candle_h / 2.0);
    }
}
