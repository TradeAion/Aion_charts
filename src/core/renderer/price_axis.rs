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
use crate::core::renderer::rgba_str as rgba;
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

        // Tick labels — draw in media (CSS) coordinate space for sharp text.
        // LWC pattern: save → scale(dpr,dpr) → draw text with CSS-px font → restore.
        // This lets the browser's native text hinting produce sharp glyphs at all DPR.
        self.base_ctx.save();
        let _ = self.base_ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);

        let css_font = format!("{}px {}", style.font_size, style.font_family);
        self.base_ctx.set_font(&css_font);
        self.base_ctx.set_fill_style_str(&rgba(&style.axis_text_color));
        self.base_ctx.set_text_align("left");
        self.base_ctx.set_text_baseline("middle");

        let padding_inner_css = style.price_axis_padding_inner();
        let text_x_css = tick_length / dpr + padding_inner_css;

        for t in ticks {
            if t.pixel < 0.0 || t.pixel > pane_h { continue; }
            let y_css = t.pixel / dpr;
            let _ = self.base_ctx.fill_text(&t.label, text_x_css, y_css);
        }
        self.base_ctx.restore();
    }

    /// Render the top layer: crosshair price label.
    /// Matches LWC's PriceAxisViewRenderer._calculateGeometry for alignRight=true.
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

        // Price at crosshair Y — use candle area height (excludes volume zone at bottom)
        let candle_h = pane_h * (1.0 - vp.volume_height_ratio as f64);
        let price = vp.price_min + (1.0 - my / candle_h).clamp(0.0, 1.0) * (vp.price_max - vp.price_min);
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
        let total_h = fs + padding_top + padding_bottom;
        let total_w = border_size + padding_inner + padding_outer + text_w + tick_length;

        // LWC: label height parity must match tick height parity
        let tick_h_bmp = dpr.floor().max(1.0);
        let tick_h_i = tick_h_bmp as i32;
        let mut total_h_bmp = total_h.round() as i32;
        if total_h_bmp % 2 != tick_h_i % 2 { total_h_bmp += 1; }
        let total_h_bmp = total_h_bmp as f64;
        let total_w_bmp = total_w.round();

        let horz_border_bmp = if border_size > 0.0 {
            (border_size).max(1.0).floor()
        } else { 0.0 };

        let tick_size_bmp = tick_length;

        // LWC: yMid = round(coordinate * vpr) - floor(vpr * 0.5)
        let y_mid = my.round() - (dpr * 0.5).floor();
        let y_top = (y_mid + tick_h_bmp / 2.0 - total_h_bmp / 2.0).floor();
        let y_bottom = y_top + total_h_bmp;

        // LWC alignRight: xInside = bitmapSize.width - horzBorderBitmap (right edge minus border)
        let x_inside = w - horz_border_bmp;
        // xOutside = xInside - totalWidthBitmap (label extends leftward)
        let x_outside = x_inside - total_w_bmp;
        // xTick = xInside - tickSizeBitmap
        let x_tick = x_inside - tick_size_bmp;

        let radius = (2.0 * dpr).round();

        // Draw rounded rect — LWC alignRight corners: [radius, 0, 0, radius]
        // = top-left rounded, top-right square, bottom-right square, bottom-left rounded
        self.top_ctx.set_fill_style_str(&rgba(&style.crosshair_label_bg));
        self.top_ctx.begin_path();
        // Start top-left (rounded)
        self.top_ctx.move_to(x_outside + radius, y_top);
        // Top edge -> top-right (square)
        self.top_ctx.line_to(x_inside, y_top);
        // Right edge -> bottom-right (square)
        self.top_ctx.line_to(x_inside, y_bottom);
        // Bottom edge -> bottom-left (rounded)
        self.top_ctx.line_to(x_outside + radius, y_bottom);
        let _ = self.top_ctx.arc_to(x_outside, y_bottom, x_outside, y_bottom - radius, radius);
        // Left edge -> top-left (rounded)
        self.top_ctx.line_to(x_outside, y_top + radius);
        let _ = self.top_ctx.arc_to(x_outside, y_top, x_outside + radius, y_top, radius);
        self.top_ctx.close_path();
        self.top_ctx.fill();

        // Tick mark — LWC: fillRect(xInside, yMid, xTick - xInside, tickHeight)
        // For alignRight, xTick < xInside, so we draw from xTick to xInside
        self.top_ctx.set_fill_style_str(&rgba(&style.crosshair_label_text));
        self.top_ctx.fill_rect(x_tick, y_mid, x_inside - x_tick, tick_h_bmp);

        // Separator (border line) — LWC: fillRect(right - horzBorder, yTop, horzBorder, yBottom - yTop)
        // using pane background color
        self.top_ctx.set_fill_style_str(&rgba(&style.bg_color));
        self.top_ctx.fill_rect(w - horz_border_bmp, y_top, horz_border_bmp, y_bottom - y_top);

        // Price text — draw in media (CSS) coordinate space for sharp text rendering.
        self.top_ctx.save();
        let _ = self.top_ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);
        let css_font = format!("{}px {}", style.font_size, style.font_family);
        self.top_ctx.set_font(&css_font);
        self.top_ctx.set_fill_style_str(&rgba(&style.crosshair_label_text));
        self.top_ctx.set_text_align("right");
        self.top_ctx.set_text_baseline("middle");
        let text_x_css = (x_inside - tick_size_bmp - padding_inner - horz_border_bmp) / dpr;
        let text_y_css = (y_top + y_bottom) / 2.0 / dpr;
        let _ = self.top_ctx.fill_text(&price_lbl, text_x_css, text_y_css);
        self.top_ctx.restore();
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
