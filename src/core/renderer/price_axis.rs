//! PriceAxisRenderer — dedicated renderer for the price (Y) axis widget.
//!
//! Mirrors LWC's PriceAxisWidget:
//! - Base canvas: background, border, tick marks, tick labels
//! - Top canvas: crosshair price label (rounded rect + text)
//!
//! Each canvas is sized to the price axis container only (not full chart).

#![cfg(target_arch = "wasm32")]

use crate::core::price_line::PriceLineManager;
use crate::core::renderer::rgba_str as rgba;
use crate::core::renderer::text_cache::TextWidthCache;
use crate::core::renderer::traits::{ChartStyle, CrosshairState, TickMark};
use crate::core::renderer::value_projection::{
    candle_area_height_ph, collect_last_values, format_scale_value, price_to_pane_y_phys,
    y_tick_step_internal,
};
use crate::core::series::SeriesCollection;
use crate::core::viewport::Viewport;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

pub struct PriceAxisRenderer {
    base_canvas: HtmlCanvasElement,
    base_ctx: CanvasRenderingContext2d,
    top_canvas: HtmlCanvasElement,
    top_ctx: CanvasRenderingContext2d,
    pw: u32,
    ph: u32,
    dpr: f64,
    /// Shared text width cache for tick labels + crosshair label.
    text_cache: TextWidthCache,
}

impl PriceAxisRenderer {
    pub fn new(
        base_canvas: HtmlCanvasElement,
        top_canvas: HtmlCanvasElement,
        dpr: f64,
    ) -> Result<Self, String> {
        let base_ctx = get_2d_ctx(&base_canvas, "price-axis base")?;
        let top_ctx = get_2d_ctx(&top_canvas, "price-axis top")?;
        let pw = base_canvas.width();
        let ph = base_canvas.height();
        Ok(Self {
            base_canvas,
            base_ctx,
            top_canvas,
            top_ctx,
            pw,
            ph,
            dpr,
            text_cache: TextWidthCache::new(50),
        })
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

    /// Measure the optimal axis text width (physical px).
    ///
    /// Includes:
    /// - Tick labels
    /// - Last-price labels (main + overlays)
    /// - Custom price-line labels
    /// - Top/bottom edge price labels (crosshair-width safety margin)
    pub fn measure_optimal_width(
        &mut self,
        style: &ChartStyle,
        ticks: &[TickMark],
        series: &SeriesCollection,
        bars: &crate::core::data::BarArray,
        price_lines: &PriceLineManager,
        vp: &Viewport,
        pane_ph: f64,
    ) -> f64 {
        let font = style.axis_font(self.dpr);
        self.base_ctx.set_font(&font);
        let mut max_w: f64 = 0.0;

        for t in ticks {
            let w = self.text_cache.measure(&self.base_ctx, &t.label, &font);
            if w > max_w {
                max_w = w;
            }
        }

        // Last-value labels (same source as pane last-price lines).
        if style.last_price_line.label_visible {
            for item in collect_last_values(series, bars, vp, style, pane_ph, self.dpr) {
                let w = self.text_cache.measure(&self.base_ctx, &item.label, &font);
                if w > max_w {
                    max_w = w;
                }
            }
        }

        // Custom price-line labels.
        let step = y_tick_step_internal(vp, pane_ph, self.dpr);
        for line in price_lines.iter() {
            if !line.is_visible() || !line.options.show_label {
                continue;
            }
            let text = if line.options.label_text.is_empty() {
                format_scale_value(vp, line.options.price, step)
            } else {
                line.options.label_text.clone()
            };
            let w = self.text_cache.measure(&self.base_ctx, &text, &font);
            if w > max_w {
                max_w = w;
            }
        }

        // Edge values: reserve width for crosshair labels near top/bottom.
        let candle_h = candle_area_height_ph(vp, pane_ph);
        if candle_h > 2.0 {
            let top = vp.pixel_to_price(1.0, candle_h);
            let bottom = vp.pixel_to_price(candle_h - 2.0, candle_h);
            let lo = top.min(bottom) + 0.111_111_111_111_11;
            let hi = top.max(bottom) - 0.111_111_111_111_11;

            let lo_lbl = format_scale_value(vp, lo, step);
            let hi_lbl = format_scale_value(vp, hi, step);
            let lo_w = self.text_cache.measure(&self.base_ctx, &lo_lbl, &font);
            let hi_w = self.text_cache.measure(&self.base_ctx, &hi_lbl, &font);
            max_w = max_w.max(lo_w).max(hi_w);
        }

        max_w
    }

    /// Render the base layer: background, border, tick marks, tick labels.
    /// `pane_h` is the pane height in physical pixels (used to know data area height).
    pub fn render_base(&mut self, style: &ChartStyle, ticks: &[TickMark], pane_h: f64) {
        let w = self.pw as f64;
        let h = self.ph as f64;
        let dpr = self.dpr;

        // Clear + background
        self.base_ctx.clear_rect(0.0, 0.0, w, h);
        self.base_ctx.set_fill_style_str(&rgba(&style.bg_color));
        self.base_ctx.fill_rect(0.0, 0.0, w, h);

        // Border line at left edge (LWC: right price scale border is at its left)
        let border_size = (style.axis_border_size as f64 * dpr).max(1.0).floor();
        self.base_ctx
            .set_fill_style_str(&rgba(&style.axis_border_color));
        self.base_ctx
            .fill_rect(0.0, 0.0, border_size, pane_h.min(h));

        // Tick marks (small horizontal bars at the border edge)
        let tick_length = (style.axis_tick_length as f64 * dpr).round();
        let tick_height = (1.0 * dpr).floor().max(1.0);
        let tick_offset = (dpr * 0.5).floor();

        self.base_ctx
            .set_fill_style_str(&rgba(&style.axis_border_color));
        for t in ticks {
            if t.pixel < 0.0 || t.pixel > pane_h {
                continue;
            }
            let y = t.pixel.round();
            self.base_ctx
                .fill_rect(0.0, y - tick_offset, tick_length, tick_height);
        }

        // Tick labels — draw in media (CSS) coordinate space for sharp text.
        // LWC pattern: save → scale(dpr,dpr) → draw text with CSS-px font → restore.
        // This lets the browser's native text hinting produce sharp glyphs at all DPR.
        self.base_ctx.save();
        let _ = self.base_ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);

        let css_font = format!("{}px {}", style.font_size, style.font_family);
        self.base_ctx.set_font(&css_font);
        self.base_ctx
            .set_fill_style_str(&rgba(&style.axis_text_color));
        self.base_ctx.set_text_align("left");
        self.base_ctx.set_text_baseline("alphabetic");

        let padding_inner_css = style.price_axis_padding_inner();
        let text_x_css = tick_length / dpr + padding_inner_css;

        for t in ticks {
            if t.pixel < 0.0 || t.pixel > pane_h {
                continue;
            }
            let y_css = t.pixel / dpr;
            // yMidCorrection: precise centering using actualBoundingBoxAscent/Descent
            let m = self
                .text_cache
                .measure_full(&self.base_ctx, &t.label, &css_font);
            let _ = self
                .base_ctx
                .fill_text(&t.label, text_x_css, y_css + m.y_mid_correction);
        }
        self.base_ctx.restore();
    }

    /// Render the top layer: crosshair price label.
    /// Matches LWC's PriceAxisViewRenderer._calculateGeometry for alignRight=true.
    ///
    /// `pane_ph` is the pane height in physical pixels.
    pub fn render_top(
        &mut self,
        crosshair: &CrosshairState,
        vp: &Viewport,
        style: &ChartStyle,
        pane_ph: f64,
    ) {
        let w = self.pw as f64;
        let h = self.ph as f64;
        let dpr = self.dpr;

        self.top_ctx.clear_rect(0.0, 0.0, w, h);

        if !crosshair.active || !style.crosshair_horz_line.label_visible {
            return;
        }

        let my = crosshair.y * dpr; // physical Y in pane space
        if my < 0.0 || my > pane_ph {
            return;
        }

        // Candle area height in physical pixels (same as pane renderer math)
        let candle_h = candle_area_height_ph(vp, pane_ph);
        if candle_h <= 0.0 {
            return;
        }

        // Price at crosshair Y (convert from internal price-scale space to raw price).
        let internal =
            vp.price_min + (1.0 - my / candle_h).clamp(0.0, 1.0) * (vp.price_max - vp.price_min);
        let price = vp.internal_to_price(internal);
        let step = y_tick_step_internal(vp, pane_ph, dpr);
        let price_lbl = format_scale_value(vp, price, step);

        let font = style.axis_font(dpr);
        self.top_ctx.set_font(&font);
        let text_w = self
            .text_cache
            .measure(&self.top_ctx, &price_lbl, &font)
            .ceil();

        let fs = style.font_size as f64 * dpr;
        let padding_inner = style.price_axis_padding_inner() * dpr;
        let padding_outer = style.price_axis_padding_outer() * dpr;
        let tick_length = (style.axis_tick_length as f64 * dpr).round();
        let border_size = (style.axis_border_size as f64 * dpr).max(1.0).floor();
        let extra_pad = style.crosshair_label_extra_padding() * dpr;
        let padding_top = style.price_axis_padding_tb() * dpr + extra_pad;
        let padding_bottom = padding_top;

        let total_h = fs + padding_top + padding_bottom;
        let total_w_raw = border_size + padding_inner + padding_outer + text_w + tick_length;
        let total_w = total_w_raw.min(w);

        // LWC: label height parity must match tick height parity
        let tick_h_bmp = dpr.floor().max(1.0);
        let tick_h_i = tick_h_bmp as i32;
        let mut total_h_bmp = total_h.round() as i32;
        if total_h_bmp % 2 != tick_h_i % 2 {
            total_h_bmp += 1;
        }
        let total_h_bmp = total_h_bmp as f64;
        let total_w_bmp = total_w.round();

        let horz_border_bmp = if border_size > 0.0 {
            (border_size).max(1.0).floor()
        } else {
            0.0
        };

        let tick_size_bmp = tick_length;

        // LWC: yMid = round(coordinate * vpr) - floor(vpr * 0.5)
        let y_mid_raw = my.round() - (dpr * 0.5).floor();
        let half_label = total_h_bmp / 2.0;

        // Vertical clamping: push label inward when near top/bottom edge of pane
        // (LWC price-axis-widget.ts _fixLabelOverlap pattern)
        let y_mid = if y_mid_raw - half_label < 0.0 {
            half_label
        } else if y_mid_raw + half_label > pane_ph {
            pane_ph - half_label
        } else {
            y_mid_raw
        };

        let y_top = (y_mid + tick_h_bmp / 2.0 - total_h_bmp / 2.0).floor();
        let y_bottom = y_top + total_h_bmp;

        // LWC alignRight: xInside = bitmapSize.width - horzBorderBitmap (right edge minus border)
        let x_inside = w - horz_border_bmp;
        // xOutside = xInside - totalWidthBitmap (label extends leftward)
        let x_outside = x_inside - total_w_bmp;
        // Clamp radius to avoid overflow when label fills full width
        let radius = (2.0 * dpr).round().min(total_h_bmp / 4.0).max(0.0);

        // Draw rounded rect — LWC alignRight corners: [radius, 0, 0, radius]
        // = top-left rounded, top-right square, bottom-right square, bottom-left rounded
        self.top_ctx
            .set_fill_style_str(&rgba(&style.crosshair_horz_line.label_bg_color));
        self.top_ctx.begin_path();
        // Start top-left (rounded)
        self.top_ctx.move_to(x_outside + radius, y_top);
        // Top edge -> top-right (square)
        self.top_ctx.line_to(x_inside, y_top);
        // Right edge -> bottom-right (square)
        self.top_ctx.line_to(x_inside, y_bottom);
        // Bottom edge -> bottom-left (rounded)
        self.top_ctx.line_to(x_outside + radius, y_bottom);
        let _ = self
            .top_ctx
            .arc_to(x_outside, y_bottom, x_outside, y_bottom - radius, radius);
        // Left edge -> top-left (rounded)
        self.top_ctx.line_to(x_outside, y_top + radius);
        let _ = self
            .top_ctx
            .arc_to(x_outside, y_top, x_outside + radius, y_top, radius);
        self.top_ctx.close_path();
        self.top_ctx.fill();

        // Separator (border line) — LWC: fillRect(right - horzBorder, yTop, horzBorder, yBottom - yTop)
        // using pane background color
        self.top_ctx.set_fill_style_str(&rgba(&style.bg_color));
        self.top_ctx.fill_rect(
            w - horz_border_bmp,
            y_top,
            horz_border_bmp,
            y_bottom - y_top,
        );

        // Price text — draw in media (CSS) coordinate space for sharp text rendering.
        self.top_ctx.save();
        let _ = self.top_ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);
        let css_font = format!("{}px {}", style.font_size, style.font_family);
        self.top_ctx.set_font(&css_font);
        self.top_ctx
            .set_fill_style_str(&rgba(&style.crosshair_label_text));
        self.top_ctx.set_text_align("right");
        self.top_ctx.set_text_baseline("alphabetic");
        let text_x_css = (x_inside - tick_size_bmp - padding_inner - horz_border_bmp) / dpr;
        let text_y_css = (y_top + y_bottom) / 2.0 / dpr;
        let m = self
            .text_cache
            .measure_full(&self.top_ctx, &price_lbl, &css_font);
        let _ = self
            .top_ctx
            .fill_text(&price_lbl, text_x_css, text_y_css + m.y_mid_correction);
        self.top_ctx.restore();
    }

    /// Render last-price labels for all visible series on the price axis.
    ///
    /// Each label shows the series' last value with a series-colored background,
    /// similar to the crosshair label but smaller (no extra padding).
    /// Rendered on the base canvas so crosshair label can draw on top.
    ///
    /// `pane_ph` is the pane height in physical pixels (same as used for candle rendering).
    pub fn render_last_price_labels(
        &mut self,
        series: &SeriesCollection,
        bars: &crate::core::data::BarArray,
        vp: &Viewport,
        style: &ChartStyle,
        pane_ph: f64,
    ) {
        if !style.last_price_line.label_visible {
            return;
        }

        let w = self.pw as f64;

        let labels = collect_last_values(series, bars, vp, style, pane_ph, self.dpr);
        if labels.is_empty() {
            return;
        }

        let dpr = self.dpr;
        let candle_h = candle_area_height_ph(vp, pane_ph);
        let font = style.axis_font(dpr);
        self.base_ctx.set_font(&font);

        let fs = style.font_size as f64 * dpr;
        let padding_inner = style.price_axis_padding_inner() * dpr;
        let padding_outer = style.price_axis_padding_outer() * dpr;
        let tick_length = (style.axis_tick_length as f64 * dpr).round();
        let border_size = (style.axis_border_size as f64 * dpr).max(1.0).floor();
        let padding_tb = style.price_axis_padding_tb() * dpr;

        for item in &labels {
            let text_w = self
                .text_cache
                .measure(&self.base_ctx, &item.label, &font)
                .ceil();
            let total_h = fs + padding_tb * 2.0;
            let total_w_raw = border_size + padding_inner + padding_outer + text_w + tick_length;
            let total_w = total_w_raw.min(w);

            // Vertical positioning: center label on the Y position
            let y_mid = item.y_phys.round();
            let half_h = (total_h / 2.0).round();
            let y_top = (y_mid - half_h).max(0.0);
            let y_bottom = (y_top + total_h).min(candle_h);

            let x_inside = w - border_size;
            let x_outside = x_inside - total_w.round();
            let radius = (2.0 * dpr).round();

            // Rounded rect background (series color)
            self.base_ctx.set_fill_style_str(&rgba(&item.color));
            self.base_ctx.begin_path();
            self.base_ctx.move_to(x_outside + radius, y_top);
            self.base_ctx.line_to(x_inside, y_top);
            self.base_ctx.line_to(x_inside, y_bottom);
            self.base_ctx.line_to(x_outside + radius, y_bottom);
            let _ = self
                .base_ctx
                .arc_to(x_outside, y_bottom, x_outside, y_bottom - radius, radius);
            self.base_ctx.line_to(x_outside, y_top + radius);
            let _ = self
                .base_ctx
                .arc_to(x_outside, y_top, x_outside + radius, y_top, radius);
            self.base_ctx.close_path();
            self.base_ctx.fill();

            // Separator border
            self.base_ctx.set_fill_style_str(&rgba(&style.bg_color));
            self.base_ctx
                .fill_rect(w - border_size, y_top, border_size, y_bottom - y_top);

            // Text in CSS coordinate space
            self.base_ctx.save();
            let _ = self.base_ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);
            let css_font = format!("{}px {}", style.font_size, style.font_family);
            self.base_ctx.set_font(&css_font);
            self.base_ctx.set_fill_style_str("rgba(255,255,255,0.9)");
            self.base_ctx.set_text_align("right");
            self.base_ctx.set_text_baseline("alphabetic");
            let text_x_css = (x_inside - tick_length - padding_inner - border_size) / dpr;
            let text_y_css = (y_top + y_bottom) / 2.0 / dpr;
            let m = self
                .text_cache
                .measure_full(&self.base_ctx, &item.label, &css_font);
            let _ =
                self.base_ctx
                    .fill_text(&item.label, text_x_css, text_y_css + m.y_mid_correction);
            self.base_ctx.restore();
        }
    }

    /// Render labels for custom price lines on the price axis.
    ///
    /// `pane_ph` is the pane height in physical pixels.
    pub fn render_price_line_labels(
        &mut self,
        price_lines: &PriceLineManager,
        vp: &Viewport,
        style: &ChartStyle,
        pane_ph: f64,
    ) {
        if price_lines.is_empty() {
            return;
        }

        let w = self.pw as f64;
        let dpr = self.dpr;

        // Candle area height in physical pixels
        let candle_h = candle_area_height_ph(vp, pane_ph);

        let step = y_tick_step_internal(vp, pane_ph, dpr);
        let font = style.axis_font(dpr);
        self.base_ctx.set_font(&font);

        let fs = style.font_size as f64 * dpr;
        let padding_inner = style.price_axis_padding_inner() * dpr;
        let padding_outer = style.price_axis_padding_outer() * dpr;
        let tick_length = (style.axis_tick_length as f64 * dpr).round();
        let border_size = (style.axis_border_size as f64 * dpr).max(1.0).floor();
        let padding_tb = style.price_axis_padding_tb() * dpr;

        for line in price_lines.iter() {
            if !line.is_visible() || !line.options.show_label {
                continue;
            }

            let opts = &line.options;
            // Use same transform as candle/overlay rendering
            let y_phys = price_to_pane_y_phys(opts.price, vp, pane_ph);

            if y_phys < 0.0 || y_phys > candle_h {
                continue;
            }

            // Label text: custom or formatted price
            let lbl = if opts.label_text.is_empty() {
                format_scale_value(vp, opts.price, step)
            } else {
                opts.label_text.clone()
            };

            let text_w = self.text_cache.measure(&self.base_ctx, &lbl, &font).ceil();
            let total_h = fs + padding_tb * 2.0;
            let total_w_raw = border_size + padding_inner + padding_outer + text_w + tick_length;
            let total_w = total_w_raw.min(w);

            let y_mid = y_phys.round();
            let half_h = (total_h / 2.0).round();
            let y_top = (y_mid - half_h).max(0.0);
            let y_bottom = (y_top + total_h).min(candle_h);

            let x_inside = w - border_size;
            let x_outside = x_inside - total_w.round();
            let radius = (2.0 * dpr).round();

            // Background color: custom or line color
            let bg_color = opts.label_bg_color.unwrap_or(opts.color);

            // Rounded rect background
            self.base_ctx.set_fill_style_str(&rgba(&bg_color));
            self.base_ctx.begin_path();
            self.base_ctx.move_to(x_outside + radius, y_top);
            self.base_ctx.line_to(x_inside, y_top);
            self.base_ctx.line_to(x_inside, y_bottom);
            self.base_ctx.line_to(x_outside + radius, y_bottom);
            let _ = self
                .base_ctx
                .arc_to(x_outside, y_bottom, x_outside, y_bottom - radius, radius);
            self.base_ctx.line_to(x_outside, y_top + radius);
            let _ = self
                .base_ctx
                .arc_to(x_outside, y_top, x_outside + radius, y_top, radius);
            self.base_ctx.close_path();
            self.base_ctx.fill();

            // Tick mark
            let tick_h = (1.0 * dpr).floor().max(1.0);
            self.base_ctx
                .set_fill_style_str(&rgba(&opts.label_text_color));
            self.base_ctx
                .fill_rect(x_inside - tick_length, y_mid, tick_length, tick_h);

            // Separator border
            self.base_ctx.set_fill_style_str(&rgba(&style.bg_color));
            self.base_ctx
                .fill_rect(w - border_size, y_top, border_size, y_bottom - y_top);

            // Text
            self.base_ctx.save();
            let _ = self.base_ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);
            let css_font = format!("{}px {}", style.font_size, style.font_family);
            self.base_ctx.set_font(&css_font);
            self.base_ctx
                .set_fill_style_str(&rgba(&opts.label_text_color));
            self.base_ctx.set_text_align("right");
            self.base_ctx.set_text_baseline("alphabetic");
            let text_x_css = (x_inside - tick_length - padding_inner - border_size) / dpr;
            let text_y_css = (y_top + y_bottom) / 2.0 / dpr;
            let m = self
                .text_cache
                .measure_full(&self.base_ctx, &lbl, &css_font);
            let _ = self
                .base_ctx
                .fill_text(&lbl, text_x_css, text_y_css + m.y_mid_correction);
            self.base_ctx.restore();
        }
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

// ═══════════════════════════════════════════════════════════════════════════════
// Label Overlap Prevention
// ═══════════════════════════════════════════════════════════════════════════════

/// A label with position and size for overlap detection.
#[derive(Debug, Clone)]
pub struct LabelRect {
    /// Center Y position (physical pixels).
    pub y_center: f64,
    /// Half height (physical pixels).
    pub half_height: f64,
    /// Priority: higher priority labels push away lower priority ones.
    /// Crosshair label = 100, last price = 50, price lines = 30, tick labels = 10.
    pub priority: i32,
    /// Original index (for mapping back after sorting).
    pub index: usize,
}

impl LabelRect {
    pub fn top(&self) -> f64 {
        self.y_center - self.half_height
    }

    pub fn bottom(&self) -> f64 {
        self.y_center + self.half_height
    }

    /// Check if this label overlaps with another.
    pub fn overlaps(&self, other: &LabelRect) -> bool {
        let gap = 2.0; // minimum gap between labels
        self.top() - gap < other.bottom() && self.bottom() + gap > other.top()
    }
}

/// Resolve overlapping labels by pushing them apart.
/// Higher priority labels stay in place; lower priority labels move.
/// Returns adjusted Y centers.
pub fn resolve_label_overlaps(labels: &mut [LabelRect], pane_h: f64) {
    if labels.len() < 2 {
        return;
    }

    // Sort by Y position
    labels.sort_by(|a, b| {
        a.y_center
            .partial_cmp(&b.y_center)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Multiple passes to resolve overlaps
    for _pass in 0..5 {
        let mut any_overlap = false;

        for i in 0..labels.len() - 1 {
            let (left, right) = labels.split_at_mut(i + 1);
            let a = &mut left[i];
            let b = &mut right[0];

            if a.overlaps(b) {
                any_overlap = true;

                // Calculate overlap amount
                let overlap = a.bottom() + 2.0 - b.top();

                // Push apart based on priority
                if a.priority > b.priority {
                    // Push b down
                    b.y_center += overlap;
                } else if b.priority > a.priority {
                    // Push a up
                    a.y_center -= overlap;
                } else {
                    // Equal priority: push both equally
                    let half = overlap / 2.0;
                    a.y_center -= half;
                    b.y_center += half;
                }
            }
        }

        // Clamp to visible area
        for label in labels.iter_mut() {
            let min_y = label.half_height;
            let max_y = pane_h - label.half_height;
            label.y_center = label.y_center.clamp(min_y, max_y);
        }

        if !any_overlap {
            break;
        }
    }

    // Sort back by original index
    labels.sort_by_key(|l| l.index);
}
