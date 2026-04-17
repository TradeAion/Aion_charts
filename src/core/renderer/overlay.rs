//! OverlayRenderer — pane top canvas: crosshair, legend, drawings.
//!
//! Sits on the pane's top canvas (z-index:2).
//! No longer draws axes or crosshair labels — those are on their own widget canvases.
//!
//! Also renders dashed line series via Canvas2D strokePath (non-Solid LineStyle).

#![cfg(target_arch = "wasm32")]

use std::collections::HashMap;

use crate::core::chart_type::MainChartType;
use crate::core::data::BarArray;
use crate::core::drawings::types::DrawingGeometry;
use crate::core::execution_marks::{
    build_execution_text_lines, build_selected_trade_locator_plan,
    cluster_execution_mark_renderables, ExecutionLabelMode, ExecutionMarkHitArea,
    ExecutionRenderableMark, ExecutionSide,
};
use crate::core::footprint::{FootprintData, FootprintOptions};
use crate::core::formatters::{format_price, format_volume};
use crate::core::indicators::render::types::DrawInstruction;
use crate::core::markers::{MarkerManager, MarkerPosition, MarkerShape};
use crate::core::price_line::PriceLineManager;
use crate::core::renderer::canvas_dash::{clear_canvas_line_dash, set_canvas_line_dash};
use crate::core::renderer::line_generator;
use crate::core::renderer::rgba_str as rgba;
use crate::core::renderer::text_cache::TextWidthCache;
use crate::core::renderer::theme::contrast_text_color;
use crate::core::renderer::traits::{ChartStyle, CrosshairState};

use crate::core::renderer::series::CandleSizing;
use crate::core::renderer::transforms::bar_to_x;
use crate::core::renderer::value_projection::{
    price_to_pane_y_phys, project_main_last_value, TimeScaleIndex,
};
use crate::core::series::{LineStyle, SeriesCollection, SeriesType};
use crate::core::viewport::Viewport;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

pub struct OverlayRenderer {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    /// Reference to the base chart canvas for rendering base-layer drawings.
    base_canvas: Option<HtmlCanvasElement>,
    base_ctx: Option<CanvasRenderingContext2d>,
    pw: u32,
    ph: u32,
    dpr: f64,
    h_pixel_ratio: f64,
    v_pixel_ratio: f64,
    /// Shared text width cache for legend measurements.
    text_cache: TextWidthCache,
}

impl OverlayRenderer {
    /// Convert a CSS-space X coordinate that uses the LWC `-1px` bias
    /// (`x_css = frac * pane_css_w - 1`) into physical pixels.
    ///
    /// Plain `x_css * ratio` is only exact at ratio=1; on fractional DPR it
    /// introduces a constant offset. This keeps crosshair/marker X perfectly
    /// aligned with candle geometry generated in physical space.
    #[inline]
    fn biased_css_x_to_phys(x_css: f64, h_pixel_ratio: f64) -> f64 {
        (x_css + 1.0) * h_pixel_ratio - 1.0
    }

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
        Ok(Self {
            canvas,
            ctx,
            base_canvas: None,
            base_ctx: None,
            pw,
            ph,
            dpr,
            h_pixel_ratio: dpr,
            v_pixel_ratio: dpr,
            text_cache: TextWidthCache::new(50),
        })
    }

    /// Set the base chart canvas for rendering base-layer drawings.
    /// Call this after construction to enable drawing on the chart canvas.
    pub fn set_base_canvas(&mut self, canvas: HtmlCanvasElement) -> Result<(), String> {
        let ctx = canvas
            .get_context("2d")
            .map_err(|e| format!("base canvas get_context('2d') failed: {:?}", e))?
            .ok_or("base canvas get_context('2d') returned None")?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "base canvas context is not CanvasRenderingContext2d")?;
        ctx.set_image_smoothing_enabled(false);
        self.base_canvas = Some(canvas);
        self.base_ctx = Some(ctx);
        Ok(())
    }

    pub fn resize(&mut self, pw: u32, ph: u32, dpr: f64, h_pixel_ratio: f64, v_pixel_ratio: f64) {
        let pw = pw.max(1);
        let ph = ph.max(1);
        if self.pw == pw
            && self.ph == ph
            && (self.dpr - dpr).abs() < 1e-6
            && (self.h_pixel_ratio - h_pixel_ratio).abs() < 1e-6
            && (self.v_pixel_ratio - v_pixel_ratio).abs() < 1e-6
        {
            return;
        }

        self.pw = pw;
        self.ph = ph;
        self.dpr = dpr;
        self.h_pixel_ratio = h_pixel_ratio;
        self.v_pixel_ratio = v_pixel_ratio;
        if self.canvas.width() != pw {
            self.canvas.set_width(pw);
        }
        if self.canvas.height() != ph {
            self.canvas.set_height(ph);
        }
        self.ctx.set_image_smoothing_enabled(false);
    }

    /// Render crosshair lines + active drawings on the pane's top canvas.
    pub fn render(&mut self, crosshair: &CrosshairState, style: &ChartStyle) {
        self.render_with_drawings(crosshair, style, &[], None);
    }

    /// Render crosshair + top-layer drawing geometry on the overlay canvas.
    pub fn render_with_drawings(
        &mut self,
        crosshair: &CrosshairState,
        style: &ChartStyle,
        top_drawings: &[DrawingGeometry],
        legend_context: Option<(&BarArray, &Viewport, &TimeScaleIndex)>,
    ) {
        let pw = self.pw as f64;
        let ph = self.ph as f64;
        self.ctx.clear_rect(0.0, 0.0, pw, ph);

        // Legend: OHLCV values in top-left corner
        if let Some((bars, viewport, time_scale)) = legend_context {
            self.render_legend(crosshair, style, bars, viewport, time_scale);
        }

        // Draw active/hovered drawings BELOW crosshair
        for geom in top_drawings {
            self.draw_geometry(geom, style.font_family.as_str());
        }

        self.draw_crosshair(crosshair, style, pw, ph);
    }

    /// Draw a DrawingGeometry on the overlay canvas.
    fn draw_geometry(&self, geom: &DrawingGeometry, font_family: &str) {
        Self::draw_geometry_on(&self.ctx, geom, font_family);
    }

    /// Draw a DrawingGeometry (lines, rects, text, anchor circles) on any 2D context.
    fn draw_geometry_on(ctx: &CanvasRenderingContext2d, geom: &DrawingGeometry, font_family: &str) {
        // Filled rects
        for r in &geom.rects {
            if r.w <= 0.0 || r.h <= 0.0 {
                continue;
            }
            ctx.set_fill_style_str(&rgba(&[r.r, r.g, r.b, r.a]));
            ctx.fill_rect(r.x as f64, r.y as f64, r.w as f64, r.h as f64);
        }

        // Lines
        for l in &geom.lines {
            ctx.set_stroke_style_str(&rgba(&[l.r, l.g, l.b, l.a]));
            let line_w = (l.width as f64).round().max(1.0);
            ctx.set_line_width(line_w);
            ctx.set_line_join("round");

            if l.dash > 0.0 && l.gap > 0.0 {
                let _ = ctx.set_line_dash(&js_sys::Array::of2(
                    &JsValue::from(l.dash as f64),
                    &JsValue::from(l.gap as f64),
                ));
            } else {
                let _ = ctx.set_line_dash(&js_sys::Array::new());
            }

            // LWC strokeInPixel: add 0.5px offset for odd-width lines
            // to snap to pixel center and prevent blurry sub-pixel rendering
            let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };
            let mut x0 = l.x0 as f64;
            let mut y0 = l.y0 as f64;
            let mut x1 = l.x1 as f64;
            let mut y1 = l.y1 as f64;

            // Snap axis-aligned drawing edges directly onto device pixels.
            // This keeps Ctrl/angle-snapped straight lines and rectangle borders crisp.
            let is_vertical = (x1 - x0).abs() <= f64::EPSILON;
            let is_horizontal = (y1 - y0).abs() <= f64::EPSILON;
            if is_vertical {
                let x = x0.round() + correction;
                x0 = x;
                x1 = x;
                y0 = y0.round();
                y1 = y1.round();
                ctx.set_line_cap("butt");
            } else if is_horizontal {
                let y = y0.round() + correction;
                y0 = y;
                y1 = y;
                x0 = x0.round();
                x1 = x1.round();
                ctx.set_line_cap("butt");
            } else {
                // Keep diagonal lines visually smooth while still avoiding subpixel blur.
                x0 = x0.round() + correction;
                y0 = y0.round() + correction;
                x1 = x1.round() + correction;
                y1 = y1.round() + correction;
                ctx.set_line_cap("round");
            }

            ctx.begin_path();
            ctx.move_to(x0, y0);
            ctx.line_to(x1, y1);
            ctx.stroke();
        }
        clear_canvas_line_dash(ctx);

        // Text labels (in physical pixel coords)
        for t in &geom.texts {
            let font = format!("{}px {}", t.font_size, font_family);
            ctx.set_font(&font);
            ctx.set_fill_style_str(&rgba(&[t.r, t.g, t.b, t.a]));
            ctx.set_text_align(t.align.as_canvas_str());
            ctx.set_text_baseline("middle");
            let _ = ctx.fill_text(&t.text, t.x as f64, t.y as f64);
        }

        // Anchor circles
        for a in &geom.anchors {
            // Fill
            ctx.set_fill_style_str(&rgba(&a.fill));
            ctx.begin_path();
            let _ = ctx.arc(a.cx, a.cy, a.radius, 0.0, std::f64::consts::TAU);
            ctx.fill();
            // Border
            ctx.set_stroke_style_str(&rgba(&a.border));
            ctx.set_line_width(a.border_width);
            ctx.begin_path();
            let _ = ctx.arc(a.cx, a.cy, a.radius, 0.0, std::f64::consts::TAU);
            ctx.stroke();
        }
    }

    /// Render base-layer drawings on the chart (pane base) canvas.
    /// These sit above candles but below the crosshair/top canvas.
    /// Does NOT clear the canvas — call after engine.render() which already drew candles.
    pub fn render_base_drawings(&self, drawings: &[DrawingGeometry]) {
        let ctx = match &self.base_ctx {
            Some(c) => c,
            None => return, // base canvas not set up
        };
        for geom in drawings {
            Self::draw_geometry_on(ctx, geom, crate::core::renderer::theme::FONT_FAMILY);
        }
    }

    /// Render OHLCV legend in the top-left corner of the pane.
    /// Shows values for the bar at the crosshair position, or the last bar if no crosshair.
    fn render_legend(
        &mut self,
        crosshair: &CrosshairState,
        style: &ChartStyle,
        bars: &BarArray,
        viewport: &Viewport,
        time_scale: &TimeScaleIndex,
    ) {
        if bars.len() == 0 {
            return;
        }

        let dpr = self.dpr;
        let pane_css_w = if dpr > 0.0 { self.pw as f64 / dpr } else { 0.0 };

        // Pick the bar to display: hovered bar or last bar
        let bar_i = if crosshair.active {
            viewport
                .bar_index_for_crosshair(crosshair.x, pane_css_w)
                .and_then(|slot| time_scale.nearest_main_bar_index_for_logical(slot as f64))
                .or(crosshair.bar_index)
                .unwrap_or(bars.len() - 1)
                .min(bars.len() - 1)
        } else {
            bars.len() - 1
        };

        let o = bars.open(bar_i) as f64;
        let h = bars.high(bar_i) as f64;
        let l = bars.low(bar_i) as f64;
        let c = bars.close(bar_i) as f64;
        let v = bars.volume(bar_i) as f64;

        let is_bullish = c >= o;

        // Determine price step for formatting
        let step = if h > l && (h - l) > 0.0 {
            (h - l) / 4.0
        } else {
            0.01
        };

        // Format values
        let o_str = format_price(o, step);
        let h_str = format_price(h, step);
        let l_str = format_price(l, step);
        let c_str = format_price(c, step);
        let v_str = format_volume(v);

        // Draw in CSS coordinate space for sharp text
        self.ctx.save();
        let _ = self.ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);

        let fs = style.font_size as f64;
        let font = format!("{}px {}", fs, style.font_family);
        self.ctx.set_font(&font);
        self.ctx.set_text_baseline("top");
        self.ctx.set_text_align("left");

        let pad = 6.0; // CSS px padding from pane edge
        let gap = 4.0; // CSS px gap between label and value
        let mut x = pad;
        let y = pad;

        let label_color = &rgba(&style.axis_text_color);
        let value_color = if is_bullish {
            rgba(&style.bullish_color)
        } else {
            rgba(&style.bearish_color)
        };

        // Helper: draw "Label: Value" pair, advancing x
        let labels = [
            ("O", &o_str),
            ("H", &h_str),
            ("L", &l_str),
            ("C", &c_str),
            ("V", &v_str),
        ];
        for (label, value) in &labels {
            // Label
            self.ctx.set_fill_style_str(label_color);
            let _ = self.ctx.fill_text(label, x, y);
            let lw = self.text_cache.measure(&self.ctx, label, &font);
            x += lw + 2.0;

            // Value (colored)
            self.ctx.set_fill_style_str(&value_color);
            let _ = self.ctx.fill_text(value, x, y);
            let vw = self.text_cache.measure(&self.ctx, value, &font);
            x += vw + gap;
        }

        self.ctx.restore();
    }

    /// Render dashed (non-Solid) line series via Canvas2D strokePath.
    ///
    /// Draws on the base chart canvas (same z-level as solid lines).
    ///
    /// Uses `setLineDash()` with the LWC dash table, then `beginPath/moveTo/lineTo/stroke`.
    pub fn render_dashed_series(
        &self,
        series: &SeriesCollection,
        viewport: &Viewport,
        time_scale: &TimeScaleIndex,
        pane_w: f64,
        pane_h: f64,
        v_ratio: f64,
        on_overlay: bool,
    ) {
        // Pick target canvas context
        let ctx = if on_overlay {
            &self.ctx
        } else {
            match &self.base_ctx {
                Some(c) => c,
                None => return,
            }
        };

        for s in series.iter() {
            if s.series_type() != SeriesType::Line || !s.line_options.visible {
                continue;
            }
            if !s.line_options.line_style.is_dashed() {
                continue;
            }

            let points = line_generator::generate_line_series_points(
                s,
                viewport,
                time_scale.timestamps(),
                pane_w,
                pane_h,
            );
            if points.len() < 2 {
                continue;
            }

            Self::stroke_line_series(
                ctx,
                &points,
                &s.line_options.color,
                s.line_options.line_width,
                v_ratio,
                &s.line_options.line_style,
            );
        }
    }

    /// Draw a line series as a Canvas2D stroked path with an optional dash pattern.
    ///
    /// `points`: pixel-space (x, y) pairs (already in physical px).
    /// `color`: [R, G, B, A] in 0.0–1.0.
    /// `css_width`: line width in CSS px.
    /// `v_ratio`: vertical pixel ratio (CSS -> physical).
    /// `style`: the LineStyle (determines dash pattern).
    fn stroke_line_series(
        ctx: &CanvasRenderingContext2d,
        points: &[(f64, f64)],
        color: &[f32; 4],
        css_width: f64,
        v_ratio: f64,
        style: &LineStyle,
    ) {
        let line_w = (css_width * v_ratio).round().max(1.0);
        set_canvas_line_dash(ctx, *style, line_w);

        ctx.set_stroke_style_str(&rgba(color));
        ctx.set_line_width(line_w);
        ctx.set_line_join("round");
        ctx.set_line_cap("butt");

        // LWC strokeInPixel: 0.5px offset for odd-width lines
        let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };

        // Build a single connected path through all points
        ctx.begin_path();
        let (x0, y0) = points[0];
        ctx.move_to(x0.round() + correction, y0.round() + correction);

        for &(x, y) in &points[1..] {
            ctx.line_to(x.round() + correction, y.round() + correction);
        }

        ctx.stroke();

        // Reset dash
        clear_canvas_line_dash(ctx);
    }

    /// Render horizontal live last-price lines for all visible series.
    ///
    /// Each line starts at the currently printing point and extends to the
    /// price axis edge, so it stays visually connected to the live print.
    /// Style is controlled by `style.last_price_line` (LWC-like options).
    ///
    /// LWC behaviour: the price line is clipped against the FULL pane height,
    /// not just the candle area.  This allows the line to remain visible even
    /// when the price is near the bottom of the candle area (approaching the
    /// volume region).
    pub fn render_last_price_lines(
        &self,
        series: &SeriesCollection,
        bars: &crate::core::data::BarArray,
        time_scale: &TimeScaleIndex,
        main_chart_type: MainChartType,
        footprint_data: &FootprintData,
        footprint_opts: &FootprintOptions,
        viewport: &Viewport,
        style: &ChartStyle,
        pane_css_w: f64,
        pane_css_h: f64,
        h_pixel_ratio: f64,
        v_pixel_ratio: f64,
        _time_ms: f64,
    ) {
        if !style.last_price_line.visible {
            return;
        }

        let dpr = self.dpr;
        let h_ratio = if h_pixel_ratio > 0.0 {
            h_pixel_ratio
        } else {
            dpr
        };
        let v_ratio = if v_pixel_ratio > 0.0 {
            v_pixel_ratio
        } else {
            dpr
        };
        let pane_pw = pane_css_w * h_ratio;
        let pane_ph = pane_css_h * v_ratio;

        let line_w = (style.last_price_line.width * dpr).floor().max(1.0);
        let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };

        self.ctx.set_line_width(line_w);
        self.ctx.set_line_cap("butt");
        set_canvas_line_dash(&self.ctx, style.last_price_line.style, line_w);

        // Main series (candles / line / area / bars / baseline / footprint)
        // LWC clips against pane bounds, not candle area — the line stays
        // visible as long as it's within the pane, even if the price scale
        // has scrolled the last value near or into the volume region.
        //
        // All chart types use the same Y path: price_to_pane_y_phys().
        // Footprint has volume_height_ratio = 0, so candle_area == full pane.
        //
        // X anchor: in normal mode slot_center IS the candle center.
        // In footprint mode the candle is the left 15% of the slot, so we
        // compute the candle center explicitly — same as footprint_generator
        // does — so the line passes through the candle body.
        if bars.len() > 0 {
            if let Some(projected) = project_main_last_value(
                bars,
                main_chart_type,
                footprint_data,
                footprint_opts,
                viewport,
                style,
                pane_ph,
                v_ratio,
                dpr,
            ) {
                let last_idx = bars.len() - 1;
                if let Some(last_slot) = time_scale.logical_index_for_main_bar(last_idx) {
                    let slot_center = bar_to_x(last_slot + 0.5, viewport, pane_pw);
                    let x_anchor = if main_chart_type == MainChartType::Footprint {
                        // Mirror footprint_generator layout: candle = 15% of slot on left
                        let sizing =
                            CandleSizing::compute_from_pane(pane_pw, viewport, h_ratio, v_ratio);
                        let half_bar = (sizing.bar_width * 0.5).floor();
                        let slot_left = (slot_center - half_bar).round();
                        let candle_w = (sizing.bar_width * 0.15).round().max(3.0);
                        // Anchor from candle body's right edge so the horizontal line
                        // visually "leaves" the footprint candle the same way as
                        // regular candlestick mode.
                        slot_left + candle_w
                    } else {
                        slot_center
                    };
                    let y_phys = projected.y_phys;
                    // Clip to full pane height (LWC: y < 0 || y > bitmapSize.height)
                    if x_anchor >= 0.0 && x_anchor < pane_pw && y_phys >= 0.0 && y_phys <= pane_ph {
                        let y = y_phys.round() + correction;
                        self.ctx.set_stroke_style_str(&rgba(&projected.color));
                        self.ctx.begin_path();
                        self.ctx.move_to(x_anchor.max(0.0), y);
                        self.ctx.line_to(pane_pw + line_w + 1.0, y);
                        self.ctx.stroke();
                    }
                }
            }
        }

        // Overlay series
        for s in series.iter() {
            if !s.is_visible() {
                continue;
            }

            let (last_price, last_ts, color) = match s.series_type() {
                SeriesType::Line | SeriesType::Area | SeriesType::Baseline => {
                    if s.line_data.is_empty() {
                        continue;
                    }
                    (
                        s.line_data.values[s.line_data.len() - 1] as f64,
                        s.line_data.last_timestamp(),
                        s.series_color(),
                    )
                }
                SeriesType::Histogram => {
                    if s.histogram_data.is_empty() {
                        continue;
                    }
                    (
                        s.histogram_data.values[s.histogram_data.len() - 1] as f64,
                        s.histogram_data.last_timestamp(),
                        s.series_color(),
                    )
                }
                SeriesType::Bar => {
                    if s.bar_data.is_empty() {
                        continue;
                    }
                    (
                        s.bar_data.close[s.bar_data.len() - 1] as f64,
                        s.bar_data.last_timestamp(),
                        s.series_color(),
                    )
                }
                SeriesType::Candlestick => continue,
            };

            let ts = match last_ts {
                Some(v) => v,
                None => continue,
            };
            let bar_idx = match time_scale.logical_index_for_timestamp(ts) {
                Some(v) => v,
                None => continue,
            };
            let x_anchor = bar_to_x(bar_idx + 0.5, viewport, pane_pw);
            let y_phys = price_to_pane_y_phys(last_price, viewport, pane_ph);
            // Clip to full pane height (LWC behaviour)
            if x_anchor < 0.0 || x_anchor >= pane_pw || y_phys < 0.0 || y_phys > pane_ph {
                continue;
            }

            let y = y_phys.round() + correction;
            self.ctx.set_stroke_style_str(&rgba(&color));
            self.ctx.begin_path();
            self.ctx.move_to(x_anchor.max(0.0), y);
            self.ctx.line_to(pane_pw + line_w + 1.0, y);
            self.ctx.stroke();
        }

        // Reset dash
        clear_canvas_line_dash(&self.ctx);
    }

    // ... rest of impl unchanged ...

    /// Render the asset-name chip (e.g. "BTCUSD") on the pane overlay, anchored
    /// to the right edge at the last price Y.
    ///
    /// TradingView-style: the chip sits to the LEFT of the price-axis label,
    /// with rounded corners on the left and a flat right edge that visually
    /// connects to the price chip in the axis.
    pub fn render_asset_name_chip(
        &mut self,
        symbol: &str,
        bars: &BarArray,
        main_chart_type: MainChartType,
        footprint_data: &FootprintData,
        footprint_opts: &FootprintOptions,
        viewport: &Viewport,
        style: &ChartStyle,
        pane_css_w: f64,
        pane_css_h: f64,
        v_pixel_ratio: f64,
    ) {
        if symbol.is_empty() || !style.last_price_line.label_visible {
            return;
        }

        let dpr = self.dpr;
        let pane_ph = pane_css_h * dpr;

        // Get the last price and its color (same source as the price-axis label).
        let projected = match project_main_last_value(
            bars,
            main_chart_type,
            footprint_data,
            footprint_opts,
            viewport,
            style,
            pane_ph,
            v_pixel_ratio,
            dpr,
        ) {
            Some(v) => v,
            None => return,
        };
        let color = projected.color;
        let y_phys = projected.y_phys;
        // Clip: if Y is outside pane bounds, skip.
        if y_phys < 0.0 || y_phys > pane_ph {
            return;
        }

        // ── Replicate price-axis label height calculation exactly ──
        // (same math as right_axis_label_height_bmp + y_top in price_axis.rs)
        let fs_phys = style.font_size as f64 * dpr;
        let vertical_inset_phys = style.price_axis_inset_tb() * dpr;
        let total_h_raw = fs_phys + vertical_inset_phys * 2.0;
        let tick_h_bmp = dpr.floor().max(1.0) as i32;
        let mut single_h_bmp = total_h_raw.round() as i32;
        if single_h_bmp % 2 != tick_h_bmp % 2 {
            single_h_bmp += 1;
        }
        let single_h_bmp = single_h_bmp.max(1) as f64;

        // Same y_top as price chip's first row (physical pixels).
        let y_mid_raw = y_phys.round() - (dpr * 0.5).floor();
        let tick_h = dpr.floor().max(1.0);
        let y_top_phys = (y_mid_raw + tick_h / 2.0 - single_h_bmp / 2.0).floor();

        // Convert to CSS for overlay drawing.
        let chip_h_css = single_h_bmp / dpr;
        let chip_y_css = y_top_phys / dpr;

        // ── Horizontal sizing ──
        let padding_lr = 5.0; // CSS px
        let radius_css = 1.5; // subtle corner radius

        let fs = style.font_size as f64;
        let css_font = format!("{}px {}", fs, style.font_family);
        self.ctx.save();
        let _ = self.ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);
        self.ctx.set_font(&css_font);
        let text_w = self.text_cache.measure(&self.ctx, symbol, &css_font);

        let chip_w_css = text_w + padding_lr * 2.0;
        let chip_x_css = pane_css_w - chip_w_css; // flush right

        let r = radius_css;

        // ── Draw background: rounded-left, square-right ──
        self.ctx.set_fill_style_str(&rgba(&color));
        self.ctx.begin_path();
        self.ctx.move_to(chip_x_css + r, chip_y_css);
        self.ctx.line_to(chip_x_css + chip_w_css, chip_y_css);
        self.ctx
            .line_to(chip_x_css + chip_w_css, chip_y_css + chip_h_css);
        self.ctx.line_to(chip_x_css + r, chip_y_css + chip_h_css);
        let _ = self.ctx.arc_to(
            chip_x_css,
            chip_y_css + chip_h_css,
            chip_x_css,
            chip_y_css + chip_h_css - r,
            r,
        );
        self.ctx.line_to(chip_x_css, chip_y_css + r);
        let _ = self
            .ctx
            .arc_to(chip_x_css, chip_y_css, chip_x_css + r, chip_y_css, r);
        self.ctx.close_path();
        self.ctx.fill();

        // ── Draw text ──
        let text_color = contrast_text_color(color);
        self.ctx.set_fill_style_str(&rgba(&text_color));
        self.ctx.set_text_align("center");
        self.ctx.set_text_baseline("middle");
        let _ = self.ctx.fill_text(
            symbol,
            chip_x_css + chip_w_css / 2.0,
            chip_y_css + chip_h_css / 2.0,
        );

        self.ctx.restore();
    }

    /// Render custom price lines.
    ///
    /// Each line is a horizontal line at a specified price, spanning the full pane width.
    /// Supports all LineStyle dash patterns.
    pub fn render_price_lines(
        &self,
        price_lines: &PriceLineManager,
        viewport: &Viewport,
        _style: &ChartStyle,
        _pane_css_w: f64,
        _pane_css_h: f64,
    ) {
        if price_lines.is_empty() {
            return;
        }

        let dpr = self.dpr;
        let pane_pw = self.pw as f64;
        let pane_ph = self.ph as f64;

        for line in price_lines.iter() {
            if !line.is_visible() {
                continue;
            }

            let opts = &line.options;
            let y_phys = price_to_pane_y_phys(opts.price, viewport, pane_ph);

            if y_phys < 0.0 || y_phys > pane_ph {
                continue;
            }

            let line_w = (opts.line_width * dpr).round().max(1.0);
            let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };

            // Set dash pattern based on line style
            set_canvas_line_dash(&self.ctx, opts.line_style, line_w);

            // Highlight if hovered
            let color = if line.hovered {
                // Brighten color slightly when hovered
                [
                    (opts.color[0] * 1.2).min(1.0),
                    (opts.color[1] * 1.2).min(1.0),
                    (opts.color[2] * 1.2).min(1.0),
                    opts.color[3],
                ]
            } else {
                opts.color
            };

            self.ctx.set_stroke_style_str(&rgba(&color));
            self.ctx.set_line_width(line_w);
            self.ctx.set_line_cap("butt");

            let y = y_phys.round() + correction;
            self.ctx.begin_path();
            self.ctx.move_to(0.0, y);
            self.ctx.line_to(pane_pw, y);
            self.ctx.stroke();
        }

        // Reset dash
        clear_canvas_line_dash(&self.ctx);
    }

    fn draw_crosshair(&self, ch: &CrosshairState, style: &ChartStyle, pane_w: f64, pane_h: f64) {
        if !ch.active {
            return;
        }

        let mx = Self::biased_css_x_to_phys(ch.x, self.h_pixel_ratio);
        let my = ch.y * self.v_pixel_ratio;

        let vert_in_bounds = mx >= 0.0 && mx <= pane_w;
        let horz_in_bounds = my >= 0.0 && my <= pane_h;
        if !vert_in_bounds && !horz_in_bounds {
            return;
        }

        self.ctx.set_line_cap("butt");

        if style.crosshair_horz_line.visible && horz_in_bounds {
            let line_w = (style.crosshair_horz_line.width * self.v_pixel_ratio)
                .floor()
                .max(1.0);
            let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };
            self.ctx
                .set_stroke_style_str(&rgba(&style.crosshair_horz_line.color));
            self.ctx.set_line_width(line_w);
            set_canvas_line_dash(&self.ctx, style.crosshair_horz_line.style, line_w);

            let hy = my.round() + correction;
            let span = line_w + 1.0;
            self.ctx.begin_path();
            self.ctx.move_to(-span, hy);
            self.ctx.line_to(pane_w + span, hy);
            self.ctx.stroke();
        }

        if style.crosshair_vert_line.visible && vert_in_bounds {
            let line_w = (style.crosshair_vert_line.width * self.h_pixel_ratio)
                .floor()
                .max(1.0);
            let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };
            self.ctx
                .set_stroke_style_str(&rgba(&style.crosshair_vert_line.color));
            self.ctx.set_line_width(line_w);
            set_canvas_line_dash(&self.ctx, style.crosshair_vert_line.style, line_w);

            let vx = mx.round() + correction;
            let span = line_w + 1.0;
            self.ctx.begin_path();
            self.ctx.move_to(vx, -span);
            self.ctx.line_to(vx, pane_h + span);
            self.ctx.stroke();
        }

        clear_canvas_line_dash(&self.ctx);
    }

    /// Render crosshair marker circles at the intersection with line/area/baseline series.
    ///
    /// LWC pattern: two-pass rendering — border ring first, then fill dot.
    /// The marker appears at the crosshair X position, Y determined by interpolating
    /// the series data at that bar index.
    pub fn render_crosshair_markers(
        &self,
        crosshair: &CrosshairState,
        series: &SeriesCollection,
        bars: &crate::core::data::BarArray,
        time_scale: &TimeScaleIndex,
        viewport: &Viewport,
        style: &ChartStyle,
        pane_css_w: f64,
        pane_css_h: f64,
    ) {
        if !crosshair.active {
            return;
        }

        let dpr = self.dpr;
        let _pane_pw = pane_css_w * dpr;
        let pane_ph = pane_css_h * dpr;

        let target_ts = match viewport
            .bar_index_for_crosshair(crosshair.x, pane_css_w)
            .and_then(|slot| time_scale.resolve_rounded_timestamp(slot as f64))
            .or_else(|| {
                crosshair
                    .bar_index
                    .and_then(|idx| bars.get(idx).map(|b| b.timestamp))
            }) {
            Some(ts) => ts,
            None => return,
        };

        // Marker styling (LWC defaults)
        let radius = 4.0 * dpr;
        let border_width = 1.0 * dpr;

        // For each visible line/area/baseline series, find the value at bar_idx
        for s in series.iter() {
            if !s.is_visible() {
                continue;
            }
            match s.series_type() {
                SeriesType::Line | SeriesType::Area | SeriesType::Baseline => {}
                _ => continue,
            }

            // Linear search for matching timestamp in series data
            let data = &s.line_data;
            if data.is_empty() {
                continue;
            }

            let value = {
                let mut found: Option<f64> = None;
                for i in 0..data.len() {
                    if data.timestamps[i] == target_ts {
                        found = Some(data.values[i] as f64);
                        break;
                    }
                }
                match found {
                    Some(v) => v,
                    None => continue,
                }
            };

            let color = s.series_color();
            let y_css = viewport.price_to_css_y(value, pane_css_h);
            let y_phys = y_css * dpr;

            if y_phys < 0.0 || y_phys > pane_ph {
                continue;
            }

            let x_phys = Self::biased_css_x_to_phys(crosshair.x, self.h_pixel_ratio);

            // Two-pass rendering: border ring then fill
            // Pass 1: border ring (white or contrasting color)
            self.ctx.begin_path();
            let _ = self.ctx.arc(
                x_phys,
                y_phys,
                radius + border_width,
                0.0,
                std::f64::consts::TAU,
            );
            self.ctx.set_fill_style_str(&rgba(&style.bg_color));
            self.ctx.fill();

            // Pass 2: fill dot (series color)
            self.ctx.begin_path();
            let _ = self
                .ctx
                .arc(x_phys, y_phys, radius, 0.0, std::f64::consts::TAU);
            self.ctx.set_fill_style_str(&rgba(&color));
            self.ctx.fill();
        }
    }

    /// Render series markers (arrows, circles, squares) positioned at bar indices.
    ///
    /// LWC's setMarkers() API:
    /// - Shapes: arrowUp, arrowDown, circle, square
    /// - Position: aboveBar, belowBar, atPrice
    /// - Two-pass rendering for circles (border ring then fill)
    pub fn render_markers(
        &self,
        markers: &MarkerManager,
        bars: &BarArray,
        time_scale: &TimeScaleIndex,
        viewport: &Viewport,
        style: &ChartStyle,
        pane_css_w: f64,
        pane_css_h: f64,
    ) {
        let dpr = self.dpr;
        let pane_ph = pane_css_h * dpr;

        if bars.len() == 0 || time_scale.is_empty() {
            return;
        }

        let Some((start_idx, end_exclusive)) =
            time_scale.visible_main_bar_range(viewport.start_bar, viewport.end_bar)
        else {
            return;
        };
        let end_idx = end_exclusive.saturating_sub(1);

        // Collect all visible markers for two-pass rendering
        struct MarkerDraw {
            x_phys: f64,
            y_phys: f64,
            shape: MarkerShape,
            color: [f32; 4],
            size: f64,
            text: String,
            text_color: [f32; 4],
        }

        let mut to_draw: Vec<MarkerDraw> = Vec::new();

        for (_series_id, series_markers) in markers.iter() {
            let visible = series_markers.in_range(start_idx, end_idx);

            for marker in visible {
                let Some(logical_slot) = time_scale.logical_index_for_main_bar(marker.bar_index)
                else {
                    continue;
                };
                // Calculate X directly in physical space for exact alignment
                // with candle centers at fractional DPR/zoom levels.
                let x_phys = bar_to_x(logical_slot + 0.5, viewport, pane_css_w * dpr);

                if x_phys < 0.0 || x_phys > pane_css_w * dpr {
                    continue;
                }

                // Calculate Y position based on marker position
                let y_price: f64 = match marker.position {
                    MarkerPosition::AboveBar => {
                        // Above the bar's high
                        bars.high(marker.bar_index) as f64
                    }
                    MarkerPosition::BelowBar => {
                        // Below the bar's low
                        bars.low(marker.bar_index) as f64
                    }
                    MarkerPosition::AtPrice => marker.price,
                };

                let y_css = viewport.price_to_css_y(y_price, pane_css_h);

                // Add offset for above/below positioning
                let y_offset = match marker.position {
                    MarkerPosition::AboveBar => -(marker.size + 4.0), // above the high
                    MarkerPosition::BelowBar => marker.size + 4.0,    // below the low
                    MarkerPosition::AtPrice => 0.0,
                };

                let y_phys = (y_css + y_offset) * dpr;

                if y_phys < 0.0 || y_phys > pane_ph {
                    continue;
                }

                to_draw.push(MarkerDraw {
                    x_phys,
                    y_phys,
                    shape: marker.shape,
                    color: marker.color,
                    size: marker.size * dpr,
                    text: marker.text.clone(),
                    text_color: marker.text_color,
                });
            }
        }

        // Two-pass rendering for circles: first all border rings, then all fills
        // This ensures fills are always on top of all borders

        // Pass 1: Draw border rings for circles
        let border_width = 2.0 * dpr;
        for m in &to_draw {
            if m.shape == MarkerShape::Circle {
                self.ctx.begin_path();
                let _ = self.ctx.arc(
                    m.x_phys,
                    m.y_phys,
                    m.size + border_width,
                    0.0,
                    std::f64::consts::TAU,
                );
                self.ctx.set_fill_style_str(&rgba(&style.bg_color));
                self.ctx.fill();
            }
        }

        // Pass 2: Draw all shapes
        for m in &to_draw {
            let color_str = rgba(&m.color);
            self.ctx.set_fill_style_str(&color_str);
            self.ctx.set_stroke_style_str(&color_str);
            self.ctx.set_line_width(2.0 * dpr);

            match m.shape {
                MarkerShape::Circle => {
                    self.ctx.begin_path();
                    let _ = self
                        .ctx
                        .arc(m.x_phys, m.y_phys, m.size, 0.0, std::f64::consts::TAU);
                    self.ctx.fill();
                }
                MarkerShape::Square => {
                    let half = m.size;
                    self.ctx
                        .fill_rect(m.x_phys - half, m.y_phys - half, half * 2.0, half * 2.0);
                }
                MarkerShape::ArrowUp => {
                    // Triangle pointing up
                    let h = m.size * 1.5;
                    let w = m.size;
                    self.ctx.begin_path();
                    self.ctx.move_to(m.x_phys, m.y_phys - h); // top
                    self.ctx.line_to(m.x_phys - w, m.y_phys + h * 0.5); // bottom left
                    self.ctx.line_to(m.x_phys + w, m.y_phys + h * 0.5); // bottom right
                    self.ctx.close_path();
                    self.ctx.fill();
                }
                MarkerShape::ArrowDown => {
                    // Triangle pointing down
                    let h = m.size * 1.5;
                    let w = m.size;
                    self.ctx.begin_path();
                    self.ctx.move_to(m.x_phys, m.y_phys + h); // bottom
                    self.ctx.line_to(m.x_phys - w, m.y_phys - h * 0.5); // top left
                    self.ctx.line_to(m.x_phys + w, m.y_phys - h * 0.5); // top right
                    self.ctx.close_path();
                    self.ctx.fill();
                }
            }

            // Draw text label if present
            if !m.text.is_empty() {
                let font_size = style.font_size as f64 * dpr;
                let font = format!("{}px {}", font_size, style.font_family);
                self.ctx.set_font(&font);
                self.ctx.set_text_align("center");
                self.ctx.set_text_baseline("middle");
                self.ctx.set_fill_style_str(&rgba(&m.text_color));

                // Text positioned below/above the shape
                let text_y = match m.shape {
                    MarkerShape::ArrowUp => m.y_phys + m.size * 2.0 + font_size,
                    MarkerShape::ArrowDown => m.y_phys - m.size * 2.0 - font_size * 0.5,
                    _ => m.y_phys + m.size + font_size,
                };

                let _ = self.ctx.fill_text(&m.text, m.x_phys, text_y);
            }
        }
    }

    /// Render execution marks (trade executions) on the price chart.
    ///
    /// Execution marks are rendered based on their resolved bar indices:
    /// - A primary vertical arrow near the candle communicates the execution side
    /// - Hovering a mark shows only the exact execution-price chevron
    /// - Clicking a mark shows exact execution-price chevrons for the selected trade
    /// - Different roles (entry, scale_in, scale_out, exit) have visual distinction
    /// - Optional label rendering for custom text
    ///
    /// Returns the positions of rendered marks for hit-testing.
    pub fn render_execution_marks(
        &self,
        execution_marks: &crate::core::execution_marks::ExecutionMarkManager,
        bars: &BarArray,
        _time_scale: &TimeScaleIndex,
        viewport: &Viewport,
        style: &ChartStyle,
        show_text: bool,
        label_mode: ExecutionLabelMode,
        pnl_visible: bool,
        cluster_threshold_px: f64,
        hovered_execution_mark_id: Option<&str>,
        selected_execution_mark_id: Option<&str>,
        pane_css_w: f64,
        pane_css_h: f64,
    ) -> Vec<ExecutionMarkHitArea> {
        let dpr = self.dpr;
        let pane_pw = pane_css_w * dpr;
        let pane_ph = pane_css_h * dpr;

        if bars.is_empty() || execution_marks.is_empty() {
            return Vec::new();
        }

        // Get visible execution marks
        let visible_marks =
            execution_marks.in_logical_range(viewport.start_bar.floor(), viewport.end_bar.ceil());
        if visible_marks.is_empty() {
            return Vec::new();
        }
        let bar_count = bars.len();

        // Primary execution arrows follow the approved trading markup palette.
        let buy_color: [f32; 4] = [41.0 / 255.0, 98.0 / 255.0, 1.0, 1.0]; // #2962FF
        let sell_color: [f32; 4] = [1.0, 74.0 / 255.0, 104.0 / 255.0, 1.0]; // #FF4A68

        // Execution markers were visually oversized; tune closer to TV density.
        let arrow_size = 7.0 * dpr;
        let arrow_gap_css = 8.0;
        let base_hit_radius_css = arrow_size / dpr + 4.0;

        let mut renderables: Vec<ExecutionRenderableMark> = Vec::new();

        for mark in visible_marks {
            let Some(time_index) = mark.resolved_time_index else {
                continue;
            };
            let Some(bar_idx) = mark.resolved_bar_index else {
                continue;
            };

            if bar_idx >= bar_count {
                continue;
            }

            // Calculate X position (center of bar)
            let x_phys = bar_to_x(time_index + 0.5, viewport, pane_pw);
            let x_css = x_phys / dpr;

            if x_phys < 0.0 || x_phys > pane_pw {
                continue;
            }

            // Get candle high/low for this bar
            let candle_high = bars.high(bar_idx) as f64;
            let candle_low = bars.low(bar_idx) as f64;

            // Position arrow at candle high/low (not at execution price)
            let arrow_y_css = match mark.side {
                ExecutionSide::Buy => {
                    // Buy arrow below the candle low
                    let low_y = viewport.price_to_css_y(candle_low, pane_css_h);
                    low_y + arrow_gap_css
                }
                ExecutionSide::Sell => {
                    // Sell arrow above the candle high
                    let high_y = viewport.price_to_css_y(candle_high, pane_css_h);
                    high_y - arrow_gap_css
                }
            };

            let arrow_y_phys = arrow_y_css * dpr;
            let price_y_phys = viewport.price_to_css_y(mark.price, pane_css_h) * dpr;

            if arrow_y_phys < 0.0 || arrow_y_phys > pane_ph {
                continue;
            }

            // Execution visuals should track actual trade side, not role metadata.
            let color = mark.color.unwrap_or_else(|| match mark.side {
                ExecutionSide::Buy => buy_color,
                ExecutionSide::Sell => sell_color,
            });
            renderables.push(ExecutionRenderableMark {
                id: mark.id.clone(),
                timestamp_ms: mark.timestamp_ms,
                price: mark.price,
                quantity: mark.quantity,
                side: mark.side,
                role: mark.role,
                label: mark.label.clone(),
                realized_pnl: mark.realized_pnl,
                color,
                group_id: mark.group_id.clone(),
                x_css,
                arrow_y_css,
                price_y_css: price_y_phys / dpr,
            });
        }
        if renderables.is_empty() {
            return Vec::new();
        }

        let renderables_by_id: HashMap<&str, &ExecutionRenderableMark> = renderables
            .iter()
            .map(|renderable| (renderable.id.as_str(), renderable))
            .collect();
        let clusters =
            cluster_execution_mark_renderables(&renderables, cluster_threshold_px, base_hit_radius_css);
        let hit_areas: Vec<ExecutionMarkHitArea> =
            clusters.iter().map(|cluster| cluster.hit_area.clone()).collect();

        // Calculate price step for formatting
        let price_range = (viewport.price_max - viewport.price_min).abs();
        let price_step = if price_range > 0.0 {
            let base_step = price_range / 100.0;
            let log_step = base_step.log10().floor();
            10.0_f64.powf(log_step)
        } else {
            0.01
        };

        // Font setup
        let font_size = (style.font_size as f64 * 0.8) * dpr;
        let font = format!("{}px {}", font_size, style.font_family);
        let background_luminance = 0.2126 * style.bg_color[0] as f64
            + 0.7152 * style.bg_color[1] as f64
            + 0.0722 * style.bg_color[2] as f64;
        let execution_text_color = if background_luminance < 0.5 {
            "#F7F7F7".to_string()
        } else {
            rgba(&style.axis_text_color)
        };

        // ═══════════════════════════════════════════════════════════════════
        // Draw each execution mark
        // ═══════════════════════════════════════════════════════════════════
        for cluster in &clusters {
            let Some(leader) = renderables_by_id.get(cluster.leader_id.as_str()) else {
                continue;
            };
            let is_cluster = cluster.is_cluster();
            let color_str = rgba(&leader.color);
            let x_phys = cluster.x_css * dpr;
            let arrow_y_css = if is_cluster {
                viewport.price_to_css_y(cluster.vwap_price, pane_css_h)
            } else {
                leader.arrow_y_css
            };
            let arrow_y_phys = arrow_y_css * dpr;

            // Primary execution marker: true arrow near the candle.
            self.draw_execution_arrow(
                x_phys,
                arrow_y_phys,
                arrow_size,
                matches!(leader.side, ExecutionSide::Buy),
                &color_str,
            );
            if is_cluster {
                self.draw_execution_cluster_badge(
                    x_phys,
                    arrow_y_phys,
                    arrow_size,
                    cluster.member_ids.len(),
                    leader.color,
                );
            }

            // Hover should reveal only the precise execution location. Once a
            // mark is selected, the selected-trade locator pass owns the
            // chevron rendering for that trade group.
            let hovered_cluster = hovered_execution_mark_id.is_some_and(|hovered_id| {
                cluster
                    .member_ids
                    .iter()
                    .any(|member_id| member_id == hovered_id)
            });
            if hovered_cluster && selected_execution_mark_id.is_none() {
                if is_cluster {
                    for member in cluster.member_ids.iter().filter_map(|member_id| {
                        renderables_by_id.get(member_id.as_str()).copied()
                    }) {
                        self.draw_execution_price_chevron(
                            member.x_css * dpr,
                            member.price_y_css * dpr,
                            10.0 * dpr,
                            matches!(member.side, ExecutionSide::Buy),
                            &rgba(&member.color),
                        );
                    }
                } else {
                    self.draw_execution_price_chevron(
                        leader.x_css * dpr,
                        leader.price_y_css * dpr,
                        10.0 * dpr,
                        matches!(leader.side, ExecutionSide::Buy),
                        &color_str,
                    );
                }
            }

            if show_text && !is_cluster {
                let mark = execution_marks.get(&leader.id);
                let Some(mark) = mark else {
                    continue;
                };
                let text_lines =
                    build_execution_text_lines(mark, label_mode, pnl_visible, price_step);
                let display_label = &text_lines[0];
                let qty_text = &text_lines[1];
                let pnl_line = text_lines.get(2).cloned();

                self.ctx.set_font(&font);
                self.ctx.set_text_align("center");
                let line_height = font_size * 1.2;

                match leader.side {
                    ExecutionSide::Buy => {
                        self.ctx.set_text_baseline("top");
                        let text_y = arrow_y_phys + (arrow_size * 2.35) + 3.0 * dpr;
                        self.ctx.set_fill_style_str(&execution_text_color);
                        let _ = self.ctx.fill_text(display_label, x_phys, text_y);
                        let _ = self.ctx.fill_text(qty_text, x_phys, text_y + line_height);
                        if let Some(pnl_line) = pnl_line {
                            self.ctx
                                .set_fill_style_str(&self.execution_pnl_text_color(mark, style));
                            let _ = self
                                .ctx
                                .fill_text(&pnl_line, x_phys, text_y + (line_height * 2.0));
                        }
                    }
                    ExecutionSide::Sell => {
                        self.ctx.set_text_baseline("bottom");
                        let text_y = arrow_y_phys - (arrow_size * 2.35) - 3.0 * dpr;
                        if let Some(pnl_line) = pnl_line {
                            self.ctx
                                .set_fill_style_str(&self.execution_pnl_text_color(mark, style));
                            let _ = self
                                .ctx
                                .fill_text(&pnl_line, x_phys, text_y - (line_height * 2.0));
                        }
                        self.ctx.set_fill_style_str(&execution_text_color);
                        let _ = self.ctx.fill_text(qty_text, x_phys, text_y);
                        let _ = self.ctx.fill_text(display_label, x_phys, text_y - line_height);
                    }
                }
            }
        }

        hit_areas
    }

    /// Render exact execution-price chevrons for the selected trade group.
    pub fn render_selected_execution_locators(
        &self,
        execution_marks: &crate::core::execution_marks::ExecutionMarkManager,
        selected_mark_id: Option<&str>,
        viewport: &Viewport,
        _style: &ChartStyle,
        pane_css_w: f64,
        pane_css_h: f64,
    ) {
        let plan = build_selected_trade_locator_plan(execution_marks, selected_mark_id);
        if plan.chevrons.is_empty() {
            return;
        }

        let dpr = self.dpr;
        let pane_pw = pane_css_w * dpr;
        let buy_color = [41.0 / 255.0, 98.0 / 255.0, 1.0, 1.0];
        let sell_color = [1.0, 74.0 / 255.0, 104.0 / 255.0, 1.0];
        let chevron_size = 9.0 * dpr;

        self.ctx.save();
        for chevron in plan.chevrons {
            let x = bar_to_x(chevron.time_index + 0.5, viewport, pane_pw);
            let y = viewport.price_to_css_y(chevron.price, pane_css_h) * dpr;
            let color = chevron.color.unwrap_or(match chevron.side {
                ExecutionSide::Buy => buy_color,
                ExecutionSide::Sell => sell_color,
            });
            self.draw_execution_price_chevron(
                x,
                y,
                chevron_size,
                matches!(chevron.side, ExecutionSide::Buy),
                &rgba(&color),
            );
        }
        self.ctx.restore();
    }

    /// Draw the primary execution marker near the candle.
    ///
    /// This is intentionally a true vertical arrow instead of a chevron so
    /// traders can distinguish the side/intent marker from the precise fill
    /// locator drawn at the execution price itself.
    fn draw_execution_arrow(&self, x: f64, center_y: f64, size: f64, points_up: bool, color: &str) {
        self.ctx.save();
        self.ctx.set_line_join("miter");

        let x = x.round();
        let center_y = center_y.round();
        let scale = size / 8.0;
        let outline_width = (0.24 * self.dpr).clamp(0.22, 0.4);
        let outline_color = "rgba(9, 12, 18, 0.42)";

        let trace_arrow = || {
            self.ctx.begin_path();
            if points_up {
                self.ctx.move_to(x, center_y);
                self.ctx.line_to(x + 6.0 * scale, center_y + 8.0 * scale);
                self.ctx.line_to(x + 2.0 * scale, center_y + 8.0 * scale);
                self.ctx.line_to(x + 2.0 * scale, center_y + 18.0 * scale);
                self.ctx.line_to(x - 2.0 * scale, center_y + 18.0 * scale);
                self.ctx.line_to(x - 2.0 * scale, center_y + 8.0 * scale);
                self.ctx.line_to(x - 6.0 * scale, center_y + 8.0 * scale);
            } else {
                self.ctx.move_to(x - 2.0 * scale, center_y - 18.0 * scale);
                self.ctx.line_to(x + 2.0 * scale, center_y - 18.0 * scale);
                self.ctx.line_to(x + 2.0 * scale, center_y - 8.0 * scale);
                self.ctx.line_to(x + 6.0 * scale, center_y - 8.0 * scale);
                self.ctx.line_to(x, center_y);
                self.ctx.line_to(x - 6.0 * scale, center_y - 8.0 * scale);
                self.ctx.line_to(x - 2.0 * scale, center_y - 8.0 * scale);
            }
            self.ctx.close_path();
        };

        trace_arrow();
        self.ctx.set_fill_style_str(color);
        self.ctx.fill();

        trace_arrow();
        self.ctx.set_line_width(outline_width);
        self.ctx.set_stroke_style_str(outline_color);
        self.ctx.stroke();

        self.ctx.restore();
    }

    /// Draw the exact execution-price locator.
    ///
    /// This uses the exact SVG point geometry supplied for buy/sell chevrons.
    /// The locator is only rendered on the selected connection-line overlay.
    fn draw_execution_price_chevron(
        &self,
        tip_x: f64,
        y: f64,
        size: f64,
        points_right: bool,
        color: &str,
    ) {
        self.ctx.save();
        self.ctx.set_line_join("miter");

        let tip_x = tip_x.round();
        let y = y.round();
        let scale = size / 14.0;
        let outline_width = (0.22 * self.dpr).clamp(0.2, 0.38);
        let outline_color = "rgba(9, 12, 18, 0.42)";
        let trace_chevron = || {
            self.ctx.begin_path();
            if points_right {
                self.ctx.move_to(tip_x - 8.0 * scale, y - 8.0 * scale);
                self.ctx.line_to(tip_x, y);
                self.ctx.line_to(tip_x - 8.0 * scale, y + 8.0 * scale);
                self.ctx.line_to(tip_x - 10.0 * scale, y + 6.0 * scale);
                self.ctx.line_to(tip_x - 4.0 * scale, y);
                self.ctx.line_to(tip_x - 10.0 * scale, y - 6.0 * scale);
            } else {
                self.ctx.move_to(tip_x + 8.0 * scale, y - 8.0 * scale);
                self.ctx.line_to(tip_x + 10.0 * scale, y - 6.0 * scale);
                self.ctx.line_to(tip_x + 4.0 * scale, y);
                self.ctx.line_to(tip_x + 10.0 * scale, y + 6.0 * scale);
                self.ctx.line_to(tip_x + 8.0 * scale, y + 8.0 * scale);
                self.ctx.line_to(tip_x, y);
            }
            self.ctx.close_path();
        };

        trace_chevron();
        self.ctx.set_fill_style_str(color);
        self.ctx.fill();

        trace_chevron();
        self.ctx.set_line_width(outline_width);
        self.ctx.set_stroke_style_str(outline_color);
        self.ctx.stroke();

        self.ctx.restore();
    }

    fn draw_execution_cluster_badge(
        &self,
        x: f64,
        y: f64,
        arrow_size: f64,
        count: usize,
        arrow_color: [f32; 4],
    ) {
        let text = format!("×{}", count);
        let font_size = (8.5 * self.dpr).max(7.0);
        let font = format!("{}px ui-monospace, SFMono-Regular, Menlo, Consolas, monospace", font_size);
        self.ctx.save();
        self.ctx.set_font(&font);
        self.ctx.set_text_align("center");
        self.ctx.set_text_baseline("middle");

        let text_width = self
            .ctx
            .measure_text(&text)
            .ok()
            .map(|metrics| metrics.width())
            .unwrap_or(font_size * 2.0);
        let padding_x = 5.0 * self.dpr;
        let padding_y = 3.0 * self.dpr;
        let badge_w = text_width + padding_x * 2.0;
        let badge_h = font_size + padding_y * 2.0;
        let radius = (badge_h * 0.5).min(8.0 * self.dpr);
        let badge_x = x + arrow_size * 0.95;
        let badge_y = y - arrow_size * 0.95;
        let left = badge_x - badge_w * 0.5;
        let top = badge_y - badge_h * 0.5;

        self.trace_rounded_rect(left, top, badge_w, badge_h, radius);
        self.ctx.set_fill_style_str(&rgba(&arrow_color));
        self.ctx.fill();

        self.trace_rounded_rect(left, top, badge_w, badge_h, radius);
        self.ctx
            .set_stroke_style_str("rgba(9, 12, 18, 0.42)");
        self.ctx
            .set_line_width((0.22 * self.dpr).clamp(0.2, 0.38));
        self.ctx.stroke();

        let text_color = contrast_text_color(arrow_color);
        self.ctx.set_fill_style_str(&rgba(&text_color));
        let _ = self.ctx.fill_text(&text, badge_x, badge_y);
        self.ctx.restore();
    }

    fn trace_rounded_rect(&self, x: f64, y: f64, width: f64, height: f64, radius: f64) {
        let right = x + width;
        let bottom = y + height;
        let radius = radius.min(width * 0.5).min(height * 0.5);
        self.ctx.begin_path();
        self.ctx.move_to(x + radius, y);
        self.ctx.line_to(right - radius, y);
        self.ctx.quadratic_curve_to(right, y, right, y + radius);
        self.ctx.line_to(right, bottom - radius);
        self.ctx
            .quadratic_curve_to(right, bottom, right - radius, bottom);
        self.ctx.line_to(x + radius, bottom);
        self.ctx.quadratic_curve_to(x, bottom, x, bottom - radius);
        self.ctx.line_to(x, y + radius);
        self.ctx.quadratic_curve_to(x, y, x + radius, y);
        self.ctx.close_path();
    }

    fn execution_pnl_text_color(
        &self,
        mark: &crate::core::execution_marks::ExecutionMark,
        style: &ChartStyle,
    ) -> String {
        match mark.realized_pnl.unwrap_or(0.0).partial_cmp(&0.0) {
            Some(std::cmp::Ordering::Greater) => "#26A69A".to_string(),
            Some(std::cmp::Ordering::Less) => "#EF5350".to_string(),
            _ => rgba(&style.axis_text_color),
        }
    }

    /// Render persistent indicator labels emitted as `DrawInstruction::DrawLabel`.
    ///
    /// Labels are drawn on the overlay canvas in physical pixels.
    pub fn render_indicator_labels(
        &mut self,
        instructions: &[DrawInstruction],
        time_scale: &TimeScaleIndex,
        viewport: &Viewport,
        style: &ChartStyle,
        pane_css_w: f64,
        pane_css_h: f64,
    ) {
        if instructions.is_empty() || time_scale.is_empty() {
            return;
        }

        let dpr = self.dpr;
        let pane_pw = pane_css_w * dpr;
        let pane_ph = pane_css_h * dpr;
        let font_size = style.font_size as f64 * dpr;
        let font = format!("{}px {}", font_size, style.font_family);

        self.ctx.set_font(&font);
        self.ctx.set_text_align("center");
        self.ctx.set_text_baseline("middle");

        for instruction in instructions {
            let DrawInstruction::DrawLabel {
                timestamp,
                value,
                text,
                color,
                ..
            } = instruction
            else {
                continue;
            };
            if text.is_empty() {
                continue;
            }
            let Some(bar_idx) = time_scale.logical_index_for_timestamp(*timestamp) else {
                continue;
            };
            let x = bar_to_x(bar_idx + 0.5, viewport, pane_pw);
            let y = viewport.price_to_css_y(*value, pane_css_h) * dpr;
            if !x.is_finite() || !y.is_finite() {
                continue;
            }
            if x < 0.0 || x > pane_pw || y < 0.0 || y > pane_ph {
                continue;
            }
            self.ctx.set_fill_style_str(&rgba(color));
            let _ = self.ctx.fill_text(text, x, y);
        }
    }

    /// Render footprint text labels on the overlay canvas.
    ///
    /// This is called for **both** Canvas2D and WebGPU backends so the text
    /// always appears on the top-most Canvas2D layer (z-index:2).
    /// Coordinates in `texts` are already in physical pixels.
    pub fn render_footprint_texts(
        &self,
        texts: &[crate::core::renderer::draw_list::DrawText],
        style: &ChartStyle,
    ) {
        if texts.is_empty() {
            return;
        }

        let font_family = style.font_family.as_str();
        self.ctx.set_text_baseline("middle");

        let mut prev_size: Option<f32> = None;
        let mut prev_color: Option<[f32; 4]> = None;
        let mut prev_align: Option<&str> = None;

        for t in texts {
            if prev_size != Some(t.font_size) {
                let font = format!("{}px {}", t.font_size, font_family);
                self.ctx.set_font(&font);
                prev_size = Some(t.font_size);
            }

            let color = [t.r, t.g, t.b, t.a];
            if prev_color != Some(color) {
                self.ctx.set_fill_style_str(&rgba(&color));
                prev_color = Some(color);
            }

            let align_str = t.align.as_canvas_str();
            if prev_align != Some(align_str) {
                self.ctx.set_text_align(align_str);
                prev_align = Some(align_str);
            }

            let _ = self.ctx.fill_text(&t.text, t.x as f64, t.y as f64);
        }
    }
}
