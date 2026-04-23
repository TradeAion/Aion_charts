//! PriceAxisRenderer — dedicated renderer for the price (Y) axis widget.
//!
//! Mirrors LWC's PriceAxisWidget:
//! - Base canvas: background, border, tick marks, tick labels
//! - Top canvas: crosshair price label (rounded rect + text)
//!
//! Each canvas is sized to the price axis container only (not full chart).

#![cfg(target_arch = "wasm32")]

use crate::core::chart_type::MainChartType;
use crate::core::drawings::types::HorizontalLineAxisLabel;
use crate::core::footprint::{FootprintData, FootprintOptions};
use crate::core::formatters::format_countdown;
use crate::core::order_line::OrderLineManager;
use crate::core::price_line::PriceLineManager;
use crate::core::renderer::axis_label_geometry::{
    centered_full_width_label_text_x_css, compute_right_axis_label_geometry,
    compute_right_axis_label_geometry_with_vertical_mode, right_axis_label_height_bmp,
    RightAxisLabelGeometry, RightAxisLabelMetrics, RightAxisLabelVerticalMode,
    RightAxisLabelWidthMode,
};
use crate::core::renderer::rgba_str as rgba;
use crate::core::renderer::text_cache::TextWidthCache;
use crate::core::renderer::theme::contrast_text_color;
use crate::core::renderer::tick_marks::infer_bar_interval_ms;
use crate::core::renderer::traits::{ChartStyle, CrosshairState, TickMark};
use crate::core::renderer::value_projection::{
    candle_area_height_ph, collect_last_values, format_scale_value, price_to_pane_y_phys,
    y_tick_step_internal, ProjectedLastValue,
};
use crate::core::series::SeriesCollection;
use crate::core::viewport::Viewport;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlElement};

pub struct PriceAxisRenderer {
    base_canvas: HtmlCanvasElement,
    base_ctx: CanvasRenderingContext2d,
    top_canvas: HtmlCanvasElement,
    top_ctx: CanvasRenderingContext2d,
    pw: u32,
    ph: u32,
    dpr: f64,
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
        let pw = pw.max(1);
        let ph = ph.max(1);
        if self.pw == pw && self.ph == ph && (self.dpr - dpr).abs() < 1e-6 {
            return;
        }

        self.pw = pw;
        self.ph = ph;
        self.dpr = dpr;
        if self.base_canvas.width() != pw {
            self.base_canvas.set_width(pw);
        }
        if self.base_canvas.height() != ph {
            self.base_canvas.set_height(ph);
        }
        if self.top_canvas.width() != pw {
            self.top_canvas.set_width(pw);
        }
        if self.top_canvas.height() != ph {
            self.top_canvas.set_height(ph);
        }
        self.base_ctx.set_image_smoothing_enabled(false);
        self.top_ctx.set_image_smoothing_enabled(false);
    }

    /// Resize with explicit CSS display dimensions.
    ///
    /// Unlike `resize()`, this also sets the CSS `width` and `height` on both
    /// canvases so the browser displays them at the correct size. Required for
    /// subpane price axes where the layout system doesn't manage canvas CSS.
    pub fn resize_with_css(&mut self, pw: u32, ph: u32, dpr: f64, css_w: f64, css_h: f64) {
        self.resize(pw, ph, dpr);
        let w_str = format!("{}px", css_w);
        let h_str = format!("{}px", css_h);
        if let Some(elem) = self.base_canvas.dyn_ref::<HtmlElement>() {
            let _ = elem.style().set_property("width", &w_str);
            let _ = elem.style().set_property("height", &h_str);
        }
        if let Some(elem) = self.top_canvas.dyn_ref::<HtmlElement>() {
            let _ = elem.style().set_property("width", &w_str);
            let _ = elem.style().set_property("height", &h_str);
        }
    }

    /// Measure the maximum width of tick labels only (used by subpanes
    /// to contribute to the shared price axis column width).
    pub fn measure_tick_label_width(&mut self, style: &ChartStyle, ticks: &[TickMark]) -> f64 {
        if ticks.is_empty() {
            return 0.0;
        }
        let font = style.axis_font(self.dpr);
        self.base_ctx.set_font(&font);
        let mut max_w: f64 = 0.0;
        for t in ticks {
            let w = self.text_cache.measure(&self.base_ctx, &t.label, &font);
            if w > max_w {
                max_w = w;
            }
        }
        max_w
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
        main_chart_type: MainChartType,
        footprint_data: &FootprintData,
        footprint_opts: &FootprintOptions,
        price_lines: &PriceLineManager,
        vp: &Viewport,
        pane_ph: f64,
        v_pixel_ratio: f64,
    ) -> f64 {
        let font = style.axis_font(self.dpr);
        self.base_ctx.set_font(&font);
        let mut max_w: f64 = 0.0;

        for tick in ticks {
            let w = self.text_cache.measure(&self.base_ctx, &tick.label, &font);
            if w > max_w {
                max_w = w;
            }
        }

        // Last-value labels (same source as pane last-price lines).
        if style.last_price_line.label_visible {
            let mut items: Vec<_> = collect_last_values(
                series,
                bars,
                main_chart_type,
                footprint_data,
                footprint_opts,
                vp,
                style,
                pane_ph,
                v_pixel_ratio,
                self.dpr,
            );
            append_countdown_to_labels(&mut items, bars);
            for item in &items {
                let w = self.text_cache.measure(&self.base_ctx, &item.label, &font);
                if w > max_w {
                    max_w = w;
                }
                if let Some(countdown) = item.countdown.as_ref() {
                    let countdown_w = self.text_cache.measure(&self.base_ctx, countdown, &font);
                    if countdown_w > max_w {
                        max_w = countdown_w;
                    }
                }
            }
        }

        // Custom price-line labels.
        let step = y_tick_step_internal(vp, pane_ph, self.dpr, style);
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
    ///
    /// LWC behaviour: tick marks are recomputed and redrawn instantly every
    /// frame with no fade animation.  Labels are placed at exact multiples of
    /// the tick span so they slide smoothly with the viewport during drag.
    pub fn render_base(&mut self, style: &ChartStyle, ticks: &[TickMark]) {
        let w = self.pw as f64;
        let h = self.ph as f64;
        let dpr = self.dpr;

        // Clear + background
        self.base_ctx.clear_rect(0.0, 0.0, w, h);
        self.base_ctx.set_fill_style_str(&rgba(&style.bg_color));
        self.base_ctx.fill_rect(0.0, 0.0, w, h);

        // Border line at left edge (LWC: right price scale border is at its left)
        let border_size = (style.axis_border_size as f64 * dpr).max(1.0).floor();
        if style.axis_border_visible {
            self.base_ctx
                .set_fill_style_str(&rgba(&style.axis_border_color));
            self.base_ctx.fill_rect(0.0, 0.0, border_size, h);
        }

        if style.axis_border_visible && style.axis_ticks_visible {
            self.base_ctx
                .set_fill_style_str(&rgba(&style.axis_border_color));
            let tick_height = dpr.floor().max(1.0);
            let tick_length = (style.axis_tick_length as f64 * dpr).round().max(1.0);
            for t in ticks {
                let y = t.pixel.round();
                if y < 0.0 || y > h {
                    continue;
                }
                self.base_ctx.fill_rect(
                    border_size,
                    (y - tick_height / 2.0).round(),
                    tick_length,
                    tick_height,
                );
            }
        }

        // Tick labels — draw in media (CSS) coordinate space for sharp text.
        // LWC pattern: save → scale(dpr,dpr) → draw text with CSS-px font → restore.
        self.base_ctx.save();
        let _ = self.base_ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);

        let css_font = format!("{}px {}", style.font_size, style.font_family);
        self.base_ctx.set_font(&css_font);
        self.base_ctx.set_text_align("left");
        self.base_ctx.set_text_baseline("middle");

        let inner_inset_css = style.price_axis_inset_inner();
        let tick_length_css = style.axis_tick_length as f64;
        // LWC: textLeftX = tickMarkLeftX + tickLength + paddingInner
        // tickMarkLeftX = 0 (at border), so text_x = border + tickLength + paddingInner.
        let text_x_css = border_size / dpr + tick_length_css + inner_inset_css;

        let h_css = h / dpr;
        self.base_ctx
            .set_fill_style_str(&rgba(&style.axis_text_color));
        let label_half_css = style.font_size as f64 * 0.5;
        let min_label_y_css = label_half_css;
        let max_label_y_css = (h_css - label_half_css).max(min_label_y_css);
        for t in ticks {
            let y_css = (t.pixel / dpr).clamp(min_label_y_css, max_label_y_css);
            if y_css < 0.0 || y_css > h_css {
                continue;
            }
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
        v_pixel_ratio: f64,
    ) {
        let w = self.pw as f64;
        let h = self.ph as f64;
        let dpr = self.dpr;
        let v_ratio = if v_pixel_ratio > 0.0 {
            v_pixel_ratio
        } else {
            dpr
        };

        self.top_ctx.clear_rect(0.0, 0.0, w, h);

        if !crosshair.active || !style.crosshair_horz_line.label_visible {
            return;
        }

        let pane_limit_h = pane_ph.min(h);
        // Must match overlay crosshair line projection (uses vertical pixel ratio).
        let my = crosshair.y * v_ratio; // physical Y in pane space
        if my < 0.0 || my > pane_limit_h {
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
        let step = y_tick_step_internal(vp, pane_ph, dpr, style);
        let price_lbl = format_scale_value(vp, price, step);

        let font = style.axis_font(dpr);
        self.top_ctx.set_font(&font);
        let text_w = self
            .text_cache
            .measure(&self.top_ctx, &price_lbl, &font)
            .ceil();

        let metrics = RightAxisLabelMetrics::from_style(style, dpr);
        let extra_inset = style.crosshair_label_extra_inset() * dpr;
        let geom = match compute_right_axis_label_geometry_with_vertical_mode(
            w,
            pane_limit_h,
            my,
            text_w,
            dpr,
            &metrics,
            extra_inset,
            RightAxisLabelWidthMode::AxisFull,
            RightAxisLabelVerticalMode::FollowValue,
        ) {
            Some(v) => v,
            None => return,
        };

        draw_right_axis_label_background(
            &self.top_ctx,
            &geom,
            &style.crosshair_horz_line.label_bg_color,
        );

        let css_font = format!("{}px {}", style.font_size, style.font_family);
        draw_right_axis_label_text(
            &self.top_ctx,
            &mut self.text_cache,
            &price_lbl,
            &css_font,
            &style.crosshair_label_text,
            &geom,
            dpr,
        );
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
        main_chart_type: MainChartType,
        footprint_data: &FootprintData,
        footprint_opts: &FootprintOptions,
        vp: &Viewport,
        style: &ChartStyle,
        pane_ph: f64,
        v_pixel_ratio: f64,
    ) {
        if !style.last_price_line.label_visible {
            return;
        }

        let w = self.pw as f64;

        let mut labels = collect_last_values(
            series,
            bars,
            main_chart_type,
            footprint_data,
            footprint_opts,
            vp,
            style,
            pane_ph,
            v_pixel_ratio,
            self.dpr,
        );
        if labels.is_empty() {
            return;
        }
        append_countdown_to_labels(&mut labels, bars);

        let dpr = self.dpr;

        let _step = y_tick_step_internal(vp, pane_ph, dpr, style);
        let font = style.axis_font(dpr);
        self.base_ctx.set_font(&font);
        let metrics = RightAxisLabelMetrics::from_style(style, dpr);

        let css_font = format!("{}px {}", style.font_size, style.font_family);
        for (_i, item) in labels.iter().enumerate() {
            if item.y_phys < 0.0 || item.y_phys > self.ph as f64 {
                continue;
            }
            let text_color = contrast_text_color(item.color);
            let price_text_w = self
                .text_cache
                .measure(&self.base_ctx, &item.label, &font)
                .ceil();
            let countdown_text_w = item
                .countdown
                .as_ref()
                .map(|countdown| {
                    self.text_cache
                        .measure(&self.base_ctx, countdown, &font)
                        .ceil()
                })
                .unwrap_or(0.0);
            // Keep both rows centered within the same chip by anchoring geometry
            // with the widest rendered row width.
            let anchor_text_w = price_text_w.max(countdown_text_w);
            let mut geom = match compute_right_axis_label_geometry_with_vertical_mode(
                w,
                self.ph as f64,
                item.y_phys,
                anchor_text_w,
                dpr,
                &metrics,
                0.0,
                RightAxisLabelWidthMode::AxisFull,
                RightAxisLabelVerticalMode::FollowValue,
            ) {
                Some(v) => v,
                None => continue,
            };

            let has_countdown = item.countdown.is_some();

            // Single-line height for the price row.
            let single_h_bmp = right_axis_label_height_bmp(&metrics, dpr, 0.0);
            // Total label height: 2 rows when countdown exists, 1 row otherwise.
            let total_h_bmp = if has_countdown {
                single_h_bmp * 2.0
            } else {
                single_h_bmp
            };
            // Live labels are attachment-first: the first row follows the live
            // price line exactly and naturally clips at pane edges.
            let y_top = geom.y_top;
            let y_bottom = y_top + total_h_bmp;

            geom.y_top = y_top;
            geom.y_bottom = y_bottom;
            // Price text is vertically centered in the first row.
            geom.text_y_css = (y_top + y_top + single_h_bmp) / 2.0 / dpr;
            geom.text_x_css =
                centered_full_width_label_text_x_css(&geom, price_text_w, dpr, &metrics);
            // LWC: corner radius = 2 * horizontalPixelRatio. Rounded on the outside edge.

            draw_right_axis_label_background(&self.base_ctx, &geom, &item.color);
            draw_right_axis_label_tick(&self.base_ctx, &geom, &text_color, dpr);
            draw_right_axis_label_text(
                &self.base_ctx,
                &mut self.text_cache,
                &item.label,
                &css_font,
                &text_color,
                &geom,
                dpr,
            );

            // Render countdown on the second row (below the price).
            if let Some(ref countdown) = item.countdown {
                let countdown_y_top = y_top + single_h_bmp;
                let countdown_y_css =
                    (countdown_y_top + countdown_y_top + single_h_bmp) / 2.0 / dpr;
                let mut countdown_geom = geom;
                countdown_geom.text_y_css = countdown_y_css;
                countdown_geom.text_x_css = centered_full_width_label_text_x_css(
                    &countdown_geom,
                    countdown_text_w,
                    dpr,
                    &metrics,
                );
                draw_right_axis_label_text(
                    &self.base_ctx,
                    &mut self.text_cache,
                    countdown,
                    &css_font,
                    &text_color,
                    &countdown_geom,
                    dpr,
                );
            }
        }
    }

    /// Render last-value indicator labels on the price axis (for subpanes).
    ///
    /// Shows the current value of each indicator line (e.g. RSI = 65.2) as a
    /// colored pill label, matching the main chart's last-price label style.
    ///
    /// `pane_ph` is the pane height in physical pixels.
    pub fn render_indicator_last_values(
        &mut self,
        values: &[(f64, [f32; 4])], // (last_value, line_color) pairs
        vp: &Viewport,
        style: &ChartStyle,
        pane_ph: f64,
    ) {
        if values.is_empty() {
            return;
        }

        let w = self.pw as f64;
        let dpr = self.dpr;
        let candle_h = candle_area_height_ph(vp, pane_ph);
        let label_h = candle_h.min(self.ph as f64);
        if candle_h <= 0.0 || label_h <= 0.0 {
            return;
        }

        let step = y_tick_step_internal(vp, pane_ph, dpr, style);
        let font = style.axis_font(dpr);
        self.base_ctx.set_font(&font);
        let metrics = RightAxisLabelMetrics::from_style(style, dpr);
        let half_h = right_axis_label_height_bmp(&metrics, dpr, 0.0) / 2.0;

        struct IndicatorLabel {
            label: String,
            y_phys: f64,
            color: [f32; 4],
        }

        let mut entries: Vec<IndicatorLabel> = Vec::new();
        for &(value, color) in values {
            if !value.is_finite() {
                continue;
            }
            let y_phys = price_to_pane_y_phys(value, vp, pane_ph);
            if y_phys < 0.0 || y_phys > candle_h {
                continue;
            }
            let label = format_scale_value(vp, value, step);
            entries.push(IndicatorLabel {
                label,
                y_phys,
                color,
            });
        }

        if entries.is_empty() {
            return;
        }

        let mut layout: Vec<LabelRect> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| LabelRect {
                y_center: e.y_phys.round(),
                half_height: half_h,
                priority: 50,
                index: i,
            })
            .collect();
        resolve_label_overlaps(&mut layout, label_h);

        let css_font = format!("{}px {}", style.font_size, style.font_family);
        for (i, entry) in entries.iter().enumerate() {
            let text_color = contrast_text_color(entry.color);
            let text_w = self
                .text_cache
                .measure(&self.base_ctx, &entry.label, &font)
                .ceil();
            let y_mid = layout
                .get(i)
                .map(|l| l.y_center)
                .unwrap_or_else(|| entry.y_phys.round());
            let geom = match compute_right_axis_label_geometry(
                w,
                label_h,
                y_mid,
                text_w,
                dpr,
                &metrics,
                0.0,
                RightAxisLabelWidthMode::AxisFull,
            ) {
                Some(v) => v,
                None => continue,
            };

            draw_right_axis_label_background(&self.base_ctx, &geom, &entry.color);
            draw_right_axis_label_text(
                &self.base_ctx,
                &mut self.text_cache,
                &entry.label,
                &css_font,
                &text_color,
                &geom,
                dpr,
            );
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
        let label_h = candle_h.min(self.ph as f64);

        let step = y_tick_step_internal(vp, pane_ph, dpr, style);
        let font = style.axis_font(dpr);
        self.base_ctx.set_font(&font);
        let metrics = RightAxisLabelMetrics::from_style(style, dpr);
        let half_h = right_axis_label_height_bmp(&metrics, dpr, 0.0) / 2.0;

        struct PriceLineLabel {
            text: String,
            y_phys: f64,
            bg_color: [f32; 4],
            text_color: [f32; 4],
        }

        let mut entries: Vec<PriceLineLabel> = Vec::new();
        for line in price_lines.iter() {
            if !line.is_visible() || !line.options.show_label {
                continue;
            }
            let opts = &line.options;
            let y_phys = price_to_pane_y_phys(opts.price, vp, pane_ph);
            if y_phys < 0.0 || y_phys > candle_h {
                continue;
            }
            let text = if opts.label_text.is_empty() {
                format_scale_value(vp, opts.price, step)
            } else {
                opts.label_text.clone()
            };
            entries.push(PriceLineLabel {
                text,
                y_phys,
                bg_color: opts.label_bg_color.unwrap_or(opts.color),
                text_color: opts.label_text_color,
            });
        }

        if entries.is_empty() {
            return;
        }

        let mut layout: Vec<LabelRect> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| LabelRect {
                y_center: e.y_phys.round(),
                half_height: half_h,
                priority: 30,
                index: i,
            })
            .collect();
        resolve_label_overlaps(&mut layout, label_h);

        let css_font = format!("{}px {}", style.font_size, style.font_family);
        for (i, entry) in entries.iter().enumerate() {
            let text_w = self
                .text_cache
                .measure(&self.base_ctx, &entry.text, &font)
                .ceil();
            let y_mid = layout
                .get(i)
                .map(|l| l.y_center)
                .unwrap_or_else(|| entry.y_phys.round());
            let geom = match compute_right_axis_label_geometry(
                w,
                label_h,
                y_mid,
                text_w,
                dpr,
                &metrics,
                0.0,
                RightAxisLabelWidthMode::TextFit,
            ) {
                Some(v) => v,
                None => continue,
            };

            draw_right_axis_label_background(&self.base_ctx, &geom, &entry.bg_color);
            draw_right_axis_label_tick(&self.base_ctx, &geom, &entry.text_color, dpr);
            draw_right_axis_label_text(
                &self.base_ctx,
                &mut self.text_cache,
                &entry.text,
                &css_font,
                &entry.text_color,
                &geom,
                dpr,
            );
        }
    }

    /// Render Y-axis labels for horizontal line drawings.
    pub fn render_horizontal_line_labels(
        &mut self,
        labels: &[HorizontalLineAxisLabel],
        vp: &Viewport,
        style: &ChartStyle,
        pane_ph: f64,
    ) {
        if labels.is_empty() {
            return;
        }

        let w = self.pw as f64;
        let dpr = self.dpr;
        let candle_h = candle_area_height_ph(vp, pane_ph);
        let label_h = candle_h.min(self.ph as f64);
        let step = y_tick_step_internal(vp, pane_ph, dpr, style);
        let font = style.axis_font(dpr);
        self.base_ctx.set_font(&font);
        let metrics = RightAxisLabelMetrics::from_style(style, dpr);
        let half_h = right_axis_label_height_bmp(&metrics, dpr, 0.0) / 2.0;

        struct DrawingLabel {
            text: String,
            y_phys: f64,
            color: [f32; 4],
        }

        let mut entries: Vec<DrawingLabel> = Vec::new();
        for label in labels {
            if !label.price.is_finite() {
                continue;
            }
            let y_phys = price_to_pane_y_phys(label.price, vp, pane_ph);
            if y_phys < 0.0 || y_phys > candle_h {
                continue;
            }
            entries.push(DrawingLabel {
                text: format_scale_value(vp, label.price, step),
                y_phys,
                color: label.color,
            });
        }

        if entries.is_empty() {
            return;
        }

        let mut layout: Vec<LabelRect> = entries
            .iter()
            .enumerate()
            .map(|(i, entry)| LabelRect {
                y_center: entry.y_phys.round(),
                half_height: half_h,
                priority: 25,
                index: i,
            })
            .collect();
        resolve_label_overlaps(&mut layout, label_h);

        let css_font = format!("{}px {}", style.font_size, style.font_family);
        for (i, entry) in entries.iter().enumerate() {
            let text_w = self
                .text_cache
                .measure(&self.base_ctx, &entry.text, &font)
                .ceil();
            let y_mid = layout
                .get(i)
                .map(|label| label.y_center)
                .unwrap_or_else(|| entry.y_phys.round());
            let geom = match compute_right_axis_label_geometry(
                w,
                label_h,
                y_mid,
                text_w,
                dpr,
                &metrics,
                0.0,
                RightAxisLabelWidthMode::TextFit,
            ) {
                Some(value) => value,
                None => continue,
            };

            let text_color = contrast_text_color(entry.color);
            draw_right_axis_label_background(&self.base_ctx, &geom, &entry.color);
            draw_right_axis_label_tick(&self.base_ctx, &geom, &text_color, dpr);
            draw_right_axis_label_text(
                &self.base_ctx,
                &mut self.text_cache,
                &entry.text,
                &css_font,
                &text_color,
                &geom,
                dpr,
            );
        }
    }

    /// Render labels for order management lines on the price axis.
    ///
    /// Order line labels show only the price with appropriate colors.
    /// `pane_ph` is the pane height in physical pixels.
    pub fn render_order_line_labels(
        &mut self,
        order_lines: &OrderLineManager,
        vp: &Viewport,
        style: &ChartStyle,
        pane_ph: f64,
    ) {
        if order_lines.is_empty() {
            return;
        }

        let w = self.pw as f64;
        let dpr = self.dpr;

        // Keep order-label clamping in the same physical Y space as order-line
        // rendering in overlay.rs (full pane height). Using candle-area-only
        // bounds here causes the axis chip to clamp while the line continues,
        // which visually disconnects the label from its line.
        let label_h = pane_ph.min(self.ph as f64);

        let font = style.axis_font(dpr);
        self.base_ctx.set_font(&font);
        let metrics = RightAxisLabelMetrics::from_style(style, dpr);
        let price_precision = order_lines.price_precision() as usize;
        // Keep axis-label visibility in sync with overlay order-line rendering.
        // Use the same pill-height formula as overlay.rs so labels disappear at
        // the same time as the corresponding order-line controls.
        let font_size_phys = style.font_size as f64 * dpr;
        let inset_tb_phys = style.price_axis_inset_tb() * dpr;
        let tick_h_bmp = dpr.floor().max(1.0) as i32;
        let mut order_pill_h_bmp = (font_size_phys + inset_tb_phys * 2.0).round() as i32;
        if order_pill_h_bmp % 2 != tick_h_bmp % 2 {
            order_pill_h_bmp += 1;
        }
        let edge_visibility_margin = order_pill_h_bmp.max(1) as f64;

        struct OrderLineLabel {
            text: String,
            y_phys: f64,
            bg_color: [f32; 4],
            text_color: [f32; 4],
        }

        let mut entries: Vec<OrderLineLabel> = Vec::new();
        for line in order_lines.iter() {
            if !line.is_visible() {
                continue;
            }
            let opts = &line.options;
            let y_phys = price_to_pane_y_phys(opts.price, vp, pane_ph);

            // Keep labels clamped at the edge while lines are near the pane,
            // but stop rendering once the line is far outside the visible range.
            if !y_phys.is_finite() {
                continue;
            }
            if y_phys < -edge_visibility_margin || y_phys > pane_ph + edge_visibility_margin {
                continue;
            }

            let text = format!("{:.prec$}", opts.price, prec = price_precision);

            entries.push(OrderLineLabel {
                text,
                y_phys,
                bg_color: opts.effective_color(),
                text_color: opts.label_text_color,
            });
        }

        if entries.is_empty() {
            return;
        }

        let css_font = format!("{}px {}", style.font_size, style.font_family);
        for entry in entries.iter() {
            let text_w = self
                .text_cache
                .measure(&self.base_ctx, &entry.text, &font)
                .ceil();
            let geom = match compute_right_axis_label_geometry(
                w,
                label_h,
                entry.y_phys,
                text_w,
                dpr,
                &metrics,
                0.0,
                RightAxisLabelWidthMode::AxisFull,
            ) {
                Some(v) => v,
                None => continue,
            };

            draw_right_axis_label_background(&self.base_ctx, &geom, &entry.bg_color);
            draw_right_axis_label_tick(&self.base_ctx, &geom, &entry.text_color, dpr);
            draw_right_axis_label_text(
                &self.base_ctx,
                &mut self.text_cache,
                &entry.text,
                &css_font,
                &entry.text_color,
                &geom,
                dpr,
            );
        }
    }
}

/// Append a bar-close countdown string to the first (main series) label.
///
/// Uses inferred bar interval from timestamp deltas and `js_sys::Date::now()`
/// to compute the time remaining until the current bar closes.
/// Daily-or-higher intervals are rendered in coarse `Xd Hh` / `Hh` format.
pub(crate) fn append_countdown_to_labels(
    labels: &mut [ProjectedLastValue],
    bars: &crate::core::data::BarArray,
) {
    if labels.is_empty() || bars.len() < 2 {
        return;
    }
    let interval_ms = match infer_bar_interval_ms(bars) {
        Some(v) if v > 0 => v as f64,
        _ => return,
    };
    let last_ts = bars.timestamp(bars.len() - 1) as f64;
    let now_ms = js_sys::Date::now();
    let bar_close_ms = last_ts + interval_ms;
    let remaining_ms = bar_close_ms - now_ms;

    if let Some(countdown_str) = format_countdown(remaining_ms, Some(interval_ms)) {
        labels[0].countdown = Some(countdown_str);
    }
}

fn draw_right_axis_label_background(
    ctx: &CanvasRenderingContext2d,
    geom: &RightAxisLabelGeometry,
    bg_color: &[f32; 4],
) {
    ctx.set_fill_style_str(&rgba(bg_color));
    ctx.begin_path();
    // Right-axis labels are square on the inside edge and rounded on the outside edge.
    ctx.move_to(geom.x_inside, geom.y_top);
    ctx.line_to(geom.x_outside - geom.radius, geom.y_top);
    let _ = ctx.arc_to(
        geom.x_outside,
        geom.y_top,
        geom.x_outside,
        geom.y_top + geom.radius,
        geom.radius,
    );
    ctx.line_to(geom.x_outside, geom.y_bottom - geom.radius);
    let _ = ctx.arc_to(
        geom.x_outside,
        geom.y_bottom,
        geom.x_outside - geom.radius,
        geom.y_bottom,
        geom.radius,
    );
    ctx.line_to(geom.x_inside, geom.y_bottom);
    ctx.close_path();
    ctx.fill();
}

fn draw_right_axis_label_tick(
    ctx: &CanvasRenderingContext2d,
    geom: &RightAxisLabelGeometry,
    tick_color: &[f32; 4],
    dpr: f64,
) {
    let tick_h = dpr.floor().max(1.0);
    ctx.set_fill_style_str(&rgba(tick_color));
    ctx.fill_rect(geom.x_inside, geom.y_mid.round(), geom.tick_size, tick_h);
}

fn draw_right_axis_label_text(
    ctx: &CanvasRenderingContext2d,
    text_cache: &mut TextWidthCache,
    text: &str,
    font_css: &str,
    text_color: &[f32; 4],
    geom: &RightAxisLabelGeometry,
    dpr: f64,
) {
    ctx.save();
    let _ = ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);
    ctx.set_font(font_css);
    ctx.set_fill_style_str(&rgba(text_color));
    ctx.set_text_align(if geom.text_align_right {
        "right"
    } else {
        "left"
    });
    ctx.set_text_baseline("middle");
    let m = text_cache.measure_full(ctx, text, font_css);
    let _ = ctx.fill_text(text, geom.text_x_css, geom.text_y_css + m.y_mid_correction);
    ctx.restore();
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
