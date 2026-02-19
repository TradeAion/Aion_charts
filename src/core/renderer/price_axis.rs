//! PriceAxisRenderer — dedicated renderer for the price (Y) axis widget.
//!
//! Mirrors LWC's PriceAxisWidget:
//! - Base canvas: background, border, tick marks, tick labels
//! - Top canvas: crosshair price label (rounded rect + text)
//!
//! Each canvas is sized to the price axis container only (not full chart).

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, CanvasRenderingContext2d};
use crate::core::renderer::traits::{ChartStyle, CrosshairState, TickMark};
use crate::core::viewport::Viewport;
use crate::core::formatters::format_price;

pub struct PriceAxisRenderer {
    base_canvas: HtmlCanvasElement,
    base_ctx: CanvasRenderingContext2d,
    top_canvas: HtmlCanvasElement,
    top_ctx: CanvasRenderingContext2d,
    pw: u32,
    ph: u32,
    dpr: f64,
}

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

impl PriceAxisRenderer {
    pub fn new(base_canvas: HtmlCanvasElement, top_canvas: HtmlCanvasElement, dpr: f64) -> Result<Self, String> {
        let base_ctx = get_2d_ctx(&base_canvas, "price-axis base")?;
        let top_ctx = get_2d_ctx(&top_canvas, "price-axis top")?;
        let pw = base_canvas.width();
        let ph = base_canvas.height();
        Ok(Self { base_canvas, base_ctx, top_canvas, top_ctx, pw, ph, dpr })
    }

    pub fn resize(&mut self, pw: u32, ph: u32, dpr: f64) {
        self.pw = pw;
        self.ph = ph;
        self.dpr = dpr;
        self.base_canvas.set_width(pw.max(1));
        self.base_canvas.set_height(ph.max(1));
        self.top_canvas.set_width(pw.max(1));
        self.top_canvas.set_height(ph.max(1));
        self.base_ctx.set_image_smoothing_enabled(false);
        self.top_ctx.set_image_smoothing_enabled(false);
    }

    /// Measure the maximum tick label width (physical px) for the given ticks.
    pub fn measure_max_tick_width(&self, style: &ChartStyle, ticks: &[TickMark]) -> f64 {
        self.base_ctx.set_font(&style.axis_font(self.dpr));
        let mut max_w: f64 = 0.0;
        for t in ticks {
            if let Ok(m) = self.base_ctx.measure_text(&t.label) {
                let w = m.width();
                if w > max_w { max_w = w; }
            }
        }
        max_w
    }

    /// Render the base layer: background, border, tick marks, tick labels.
    /// `pane_h` is the pane height in physical pixels (used to know data area height).
    pub fn render_base(&self, style: &ChartStyle, ticks: &[TickMark], pane_h: f64) {
        let w = self.pw as f64;
        let h = self.ph as f64;
        let dpr = self.dpr;

        // Clear + background
        self.base_ctx.clear_rect(0.0, 0.0, w, h);
        self.base_ctx.set_fill_style_str(&rgba(&style.bg_color));
        self.base_ctx.fill_rect(0.0, 0.0, w, h);

        // Border line at left edge (LWC: right price scale border is at its left)
        let border_size = (style.axis_border_size as f64 * dpr).max(1.0).floor();
        self.base_ctx.set_fill_style_str(&rgba(&style.axis_border_color));
        self.base_ctx.fill_rect(0.0, 0.0, border_size, pane_h.min(h));

        // Tick marks (small horizontal bars at the border edge)
        let tick_length = (style.axis_tick_length as f64 * dpr).round();
        let tick_height = (1.0 * dpr).floor().max(1.0);
        let tick_offset = (dpr * 0.5).floor();

        self.base_ctx.set_fill_style_str(&rgba(&style.axis_border_color));
        for t in ticks {
            if t.pixel < 0.0 || t.pixel > pane_h { continue; }
            let y = t.pixel.round();
            self.base_ctx.fill_rect(0.0, y - tick_offset, tick_length, tick_height);
        }

        // Tick labels
        let font = style.axis_font(dpr);
        self.base_ctx.set_font(&font);
        self.base_ctx.set_fill_style_str(&rgba(&style.axis_text_color));
        self.base_ctx.set_text_align("left");
        self.base_ctx.set_text_baseline("middle");

        let padding_inner = style.price_axis_padding_inner() * dpr;
        let text_x = tick_length + padding_inner;

        for t in ticks {
            if t.pixel < 0.0 || t.pixel > pane_h { continue; }
            let _ = self.base_ctx.fill_text(&t.label, text_x, t.pixel);
        }
    }

    /// Render the top layer: crosshair price label.
    /// `crosshair_y` is in CSS px relative to the pane.
    pub fn render_top(
        &self,
        crosshair: &CrosshairState,
        vp: &Viewport,
        style: &ChartStyle,
        pane_css_h: f64,
    ) {
        let w = self.pw as f64;
        let h = self.ph as f64;
        let dpr = self.dpr;

        self.top_ctx.clear_rect(0.0, 0.0, w, h);

        if !crosshair.active { return; }

        let my = crosshair.y * dpr; // physical Y in pane space
        let pane_h = pane_css_h * dpr;
        if my < 0.0 || my > pane_h { return; }

        // Price at crosshair Y
        let price = vp.price_min + (1.0 - my / pane_h) * (vp.price_max - vp.price_min);
        let step = (vp.price_max - vp.price_min) / 10.0;
        let price_lbl = format_price(price, step.max(0.0001));

        let font = style.axis_font(dpr);
        self.top_ctx.set_font(&font);

        let fs = style.font_size as f64 * dpr;
        let padding_inner = style.price_axis_padding_inner() * dpr;
        let padding_outer = style.price_axis_padding_outer() * dpr;
        let tick_length = (style.axis_tick_length as f64 * dpr).round();
        let border_size = (style.axis_border_size as f64 * dpr).max(1.0).floor();
        let extra_pad = style.crosshair_label_extra_padding() * dpr;
        let padding_top = style.price_axis_padding_tb() * dpr + extra_pad;
        let padding_bottom = padding_top;

        let text_w = self.top_ctx.measure_text(&price_lbl).map(|m| m.width()).unwrap_or(40.0).ceil();
        let label_h = fs + padding_top + padding_bottom;
        let label_w = border_size + padding_inner + padding_outer + text_w + tick_length;

        // LWC: label height parity must match tick height parity
        let tick_h_i = (dpr.floor().max(1.0)) as i32;
        let mut label_h_i = label_h.round() as i32;
        if label_h_i % 2 != tick_h_i % 2 { label_h_i += 1; }
        let label_h = label_h_i as f64;

        let y_mid = my.round() - (dpr * 0.5).floor();
        let y_top = (y_mid + tick_h_i as f64 / 2.0 - label_h / 2.0).floor();
        let label_w_r = label_w.round().min(w);
        let radius = (2.0 * dpr).round();

        // Draw rounded rect (left corners rounded since border is at left)
        self.top_ctx.set_fill_style_str(&rgba(&style.crosshair_label_bg));
        self.top_ctx.begin_path();
        let x1 = 0.0;
        let x2 = label_w_r;
        let y1 = y_top;
        let y2 = y_top + label_h;

        self.top_ctx.move_to(x1 + radius, y1);
        self.top_ctx.line_to(x2, y1);
        self.top_ctx.line_to(x2, y2);
        self.top_ctx.line_to(x1 + radius, y2);
        let _ = self.top_ctx.arc_to(x1, y2, x1, y2 - radius, radius);
        self.top_ctx.line_to(x1, y1 + radius);
        let _ = self.top_ctx.arc_to(x1, y1, x1 + radius, y1, radius);
        self.top_ctx.close_path();
        self.top_ctx.fill();

        // Tick mark on label
        let tick_h_px = dpr.floor().max(1.0);
        let tick_off = (dpr * 0.5).floor();
        self.top_ctx.set_fill_style_str(&rgba(&style.crosshair_label_text));
        self.top_ctx.fill_rect(0.0, y_mid - tick_off, tick_length, tick_h_px);

        // Price text
        self.top_ctx.set_fill_style_str(&rgba(&style.crosshair_label_text));
        self.top_ctx.set_text_align("left");
        self.top_ctx.set_text_baseline("middle");
        let text_x = tick_length + padding_inner;
        let text_y = (y1 + y2) / 2.0;
        let _ = self.top_ctx.fill_text(&price_lbl, text_x, text_y);
    }
}

fn get_2d_ctx(canvas: &HtmlCanvasElement, label: &str) -> Result<CanvasRenderingContext2d, String> {
    let ctx = canvas
        .get_context("2d")
        .map_err(|e| format!("{} get_context('2d') failed: {:?}", label, e))?
        .ok_or(format!("{} get_context('2d') returned None", label))?
        .dyn_into::<CanvasRenderingContext2d>()
        .map_err(|_| format!("{} context is not CanvasRenderingContext2d", label))?;
    ctx.set_image_smoothing_enabled(false);
    Ok(ctx)
}
