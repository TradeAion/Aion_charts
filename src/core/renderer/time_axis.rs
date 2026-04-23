//! TimeAxisRenderer — dedicated renderer for the time (X) axis widget.
//!
//! Mirrors LWC's TimeAxisWidget:
//! - Base canvas: background, border, tick marks, tick labels (normal + bold)
//! - Top canvas: crosshair time label (rounded rect + text)
//!
//! Each canvas is sized to the time axis container only.

#![cfg(target_arch = "wasm32")]

use crate::core::formatters::format_crosshair_time;
use crate::core::renderer::rgba_str as rgba;
use crate::core::renderer::text_cache::TextWidthCache;
use crate::core::renderer::tick_marks::{
    collect_visible_time_points, nearest_visible_time_point, timestamp_for_logical_index,
};
use crate::core::renderer::traits::{ChartStyle, CrosshairState, TickMark};
use crate::core::renderer::value_projection::TimeScaleIndex;
use crate::core::series::SeriesCollection;
use crate::core::viewport::Viewport;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

pub struct TimeAxisRenderer {
    base_canvas: HtmlCanvasElement,
    base_ctx: CanvasRenderingContext2d,
    top_canvas: HtmlCanvasElement,
    top_ctx: CanvasRenderingContext2d,
    pw: u32,
    ph: u32,
    dpr: f64,
    /// Shared text width cache for crosshair time label.
    text_cache: TextWidthCache,
}

impl TimeAxisRenderer {
    pub fn new(
        base_canvas: HtmlCanvasElement,
        top_canvas: HtmlCanvasElement,
        dpr: f64,
    ) -> Result<Self, String> {
        let base_ctx = get_2d_ctx(&base_canvas, "time-axis base")?;
        let top_ctx = get_2d_ctx(&top_canvas, "time-axis top")?;
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

    /// Render the base layer: background, border, tick marks, tick labels.
    /// `pane_w` is the pane width in physical pixels (only draw ticks within this range).
    pub fn render_base(
        &mut self,
        style: &ChartStyle,
        ticks: &[TickMark],
        pane_w: f64,
        axis_css_w: f64,
        axis_css_h: f64,
    ) {
        let w = self.pw as f64;
        let h = self.ph as f64;
        let dpr = self.dpr;
        let h_ratio = if axis_css_w > 0.0 { w / axis_css_w } else { dpr };
        let v_ratio = if axis_css_h > 0.0 { h / axis_css_h } else { dpr };

        // Clear + background
        self.base_ctx.clear_rect(0.0, 0.0, w, h);
        self.base_ctx.set_fill_style_str(&rgba(&style.bg_color));
        self.base_ctx.fill_rect(0.0, 0.0, w, h);

        // Border line at top edge (LWC: time axis border is at its top)
        let border_size = (style.axis_border_size as f64 * v_ratio).max(1.0).floor();
        if style.axis_border_visible {
            self.base_ctx
                .set_fill_style_str(&rgba(&style.axis_border_color));
            self.base_ctx.fill_rect(0.0, 0.0, w, border_size);
        }

        if style.axis_border_visible && style.axis_ticks_visible {
            self.base_ctx
                .set_fill_style_str(&rgba(&style.axis_border_color));
            let tick_width = h_ratio.floor().max(1.0);
            let tick_height = (style.axis_tick_length as f64 * v_ratio).round().max(1.0);
            for t in ticks {
                if t.pixel < 0.0 || t.pixel > pane_w {
                    continue;
                }
                let tick_x = (t.pixel - tick_width / 2.0).round();
                self.base_ctx
                    .fill_rect(tick_x, border_size, tick_width, tick_height);
            }
        }

        // Tick labels — draw in media (CSS) coordinate space for sharp text.
        self.base_ctx.save();
        let _ = self
            .base_ctx
            .set_transform(h_ratio, 0.0, 0.0, v_ratio, 0.0, 0.0);

        let css_font_normal = format!("{}px {}", style.font_size, style.font_family);
        let css_font_major = format!("600 {}px {}", style.font_size, style.font_family);
        self.base_ctx
            .set_fill_style_str(&rgba(&style.axis_text_color));
        self.base_ctx.set_text_align("center");
        self.base_ctx.set_text_baseline("middle");

        let inset_top_css = style.time_axis_inset_top();
        let tick_length_css = style.axis_tick_length as f64;
        let fs_css = style.font_size as f64;
        // LWC: yText = borderSize + tickLength + paddingTop + fontSize/2
        let text_y_css = border_size / v_ratio + tick_length_css + inset_top_css + fs_css / 2.0;
        let pane_css_w_axis = pane_w / h_ratio;

        for t in ticks {
            if t.pixel < 0.0 || t.pixel > pane_w || t.label.is_empty() {
                continue;
            }
            let css_font = if t.major {
                &css_font_major
            } else {
                &css_font_normal
            };
            self.base_ctx.set_font(css_font);
            let x_css = align_tick_label_x_css(
                &mut self.text_cache,
                &self.base_ctx,
                css_font,
                &t.label,
                t.pixel / h_ratio,
                pane_css_w_axis,
            );
            let _ = self.base_ctx.fill_text(&t.label, x_css, text_y_css);
        }
        self.base_ctx.restore();
    }

    /// Render the top layer: crosshair time label.
    /// `crosshair_x` is in CSS px relative to the pane.
    pub fn render_top(
        &mut self,
        crosshair: &CrosshairState,
        _bars: &crate::core::data::BarArray,
        _series: &SeriesCollection,
        time_scale: &TimeScaleIndex,
        vp: &Viewport,
        style: &ChartStyle,
        pane_css_w: f64,
        axis_css_w: f64,
        axis_css_h: f64,
    ) {
        let w = self.pw as f64;
        let h = self.ph as f64;
        let dpr = self.dpr;
        let h_ratio = if axis_css_w > 0.0 { w / axis_css_w } else { dpr };
        let v_ratio = if axis_css_h > 0.0 { h / axis_css_h } else { dpr };
        let axis_css_w = if axis_css_w > 0.0 {
            axis_css_w
        } else if dpr > 0.0 {
            w / dpr
        } else {
            pane_css_w
        };
        let axis_css_h = if axis_css_h > 0.0 {
            axis_css_h
        } else if dpr > 0.0 {
            h / dpr
        } else {
            0.0
        };

        self.top_ctx.clear_rect(0.0, 0.0, w, h);

        if !crosshair.active || !style.crosshair_vert_line.label_visible {
            return;
        }

        let mx_css = crosshair.x;
        if mx_css < 0.0 || mx_css > pane_css_w {
            return;
        }

        let logical_index = vp.pixel_to_bar(mx_css, pane_css_w);
        let visible_time_points = collect_visible_time_points(vp, time_scale);
        let snapped_timestamp = nearest_visible_time_point(&visible_time_points, logical_index)
            .filter(|point| (point.logical_index - logical_index).abs() <= 0.75)
            .map(|point| point.timestamp);
        let fallback_timestamp = vp
            .bar_index_for_crosshair(mx_css, pane_css_w)
            .and_then(|idx| timestamp_for_logical_index(time_scale, idx as i64));
        let bar_lbl = match snapped_timestamp
            .or(fallback_timestamp)
            .filter(|&ts| ts > 0)
        {
            Some(ts) => format_crosshair_time(ts),
            None => return,
        };

        // LWC parity: compute label geometry in media/CSS coordinates, then convert to bitmap.
        let css_font = format!("{}px {}", style.font_size, style.font_family);
        self.top_ctx.set_font(&css_font);

        let text_w = self
            .text_cache
            .measure(&self.top_ctx, &bar_lbl, &css_font)
            .round();
        let h_margin = style.time_axis_inset_horizontal();
        let label_w = text_w + 2.0 * h_margin;
        let label_half = label_w / 2.0;

        // Center on crosshair X, clamp to bounds (LWC uses +0.5 half-pixel offset).
        let mut coord = mx_css;
        let mut lx1 = (coord - label_half).floor() + 0.5;
        if lx1 < 0.0 {
            coord += -lx1;
            lx1 = (coord - label_half).floor() + 0.5;
        } else if lx1 + label_w > axis_css_w {
            coord -= (lx1 + label_w) - axis_css_w;
            lx1 = (coord - label_half).floor() + 0.5;
        }
        let lx2 = lx1 + label_w;

        // Label height excludes labelBottomOffset (LWC y2 calculation in time-axis-view-renderer.ts).
        // LWC: y2 = ceil(y1 + borderSize + tickLength + paddingTop + fontSize + paddingBottom)
        let border_size = style.axis_border_size as f64;
        let tick_length = style.axis_tick_length as f64;
        let inset_top = style.time_axis_inset_top();
        let inset_bottom = style.time_axis_inset_bottom();
        let fs = style.font_size as f64;

        // LWC: label starts at y=0 (covers border + tick area with its background).
        let by1_css = style.time_axis_crosshair_label_top_inset();
        let by2_css = (by1_css + border_size + tick_length + inset_top + fs + inset_bottom)
            .ceil()
            .min(axis_css_h.max(0.0));

        let lx1_bmp = (lx1 * h_ratio).round();
        let lx2_bmp = (lx2 * h_ratio).round();
        let by1_bmp = (by1_css * v_ratio).round();
        let by2_bmp = (by2_css * v_ratio).round();
        let radius = (2.0 * v_ratio.min(h_ratio.max(1.0))).round();

        // Rounded rect: top corners square, bottom corners rounded
        self.top_ctx
            .set_fill_style_str(&rgba(&style.crosshair_vert_line.label_bg_color));
        self.top_ctx.begin_path();
        self.top_ctx.move_to(lx1_bmp, by1_bmp);
        self.top_ctx.line_to(lx1_bmp, by2_bmp - radius);
        let _ = self
            .top_ctx
            .arc_to(lx1_bmp, by2_bmp, lx1_bmp + radius, by2_bmp, radius);
        self.top_ctx.line_to(lx2_bmp - radius, by2_bmp);
        let _ = self
            .top_ctx
            .arc_to(lx2_bmp, by2_bmp, lx2_bmp, by2_bmp - radius, radius);
        self.top_ctx.line_to(lx2_bmp, by1_bmp);
        self.top_ctx.close_path();
        self.top_ctx.fill();

        // Time text — draw in media (CSS) coordinate space for sharp text.
        self.top_ctx.save();
        let _ = self
            .top_ctx
            .set_transform(h_ratio, 0.0, 0.0, v_ratio, 0.0, 0.0);
        self.top_ctx.set_font(&css_font);
        self.top_ctx
            .set_fill_style_str(&rgba(&style.crosshair_label_text));
        self.top_ctx.set_text_align("left");
        self.top_ctx.set_text_baseline("middle");
        let text_x_css = lx1 + h_margin;
        // LWC: yText = y1 + borderSize + tickLength + paddingTop + fontSize/2
        let text_y_css = by1_css + border_size + tick_length + inset_top + fs / 2.0;
        let m = self
            .text_cache
            .measure_full(&self.top_ctx, "Apr0", &css_font);
        let _ = self
            .top_ctx
            .fill_text(&bar_lbl, text_x_css, text_y_css + m.y_mid_correction);
        self.top_ctx.restore();
    }

    /// Render a scrollbar indicator showing the visible range within the total data.
    ///
    /// The scrollbar appears as a thin track at the very top of the time axis,
    /// with a highlighted thumb showing the currently visible portion.
    ///
    /// - `viewport`: Current viewport with start_bar, end_bar
    /// - `total_bars`: Total number of bars in the dataset
    /// - `pane_w`: Width of the pane area in physical pixels
    pub fn render_scrollbar(
        &self,
        style: &ChartStyle,
        viewport: &Viewport,
        total_bars: usize,
        pane_w: f64,
    ) {
        if total_bars == 0 {
            return;
        }

        let dpr = self.dpr;
        let w = pane_w.min(self.pw as f64);

        // Scrollbar dimensions
        let track_height = (3.0 * dpr).round().max(2.0);
        let track_y = 0.0; // At the very top, overlapping the border slightly

        // Calculate thumb position and size
        let total = total_bars as f64;
        // Add some margin for scrolling past the end
        let scrollable_range = total + 50.0; // Same margin as keyboard scroll

        let visible_start = viewport.start_bar.max(0.0);
        let visible_end = viewport.end_bar;
        let visible_bars = visible_end - visible_start;

        // Thumb position as fraction of total scrollable range
        let thumb_start_frac = visible_start / scrollable_range;
        let thumb_width_frac = (visible_bars / scrollable_range).min(1.0);

        let thumb_x = (thumb_start_frac * w).round();
        let thumb_w = (thumb_width_frac * w).round().max(20.0 * dpr); // Minimum thumb width

        // Draw track background (semi-transparent)
        let track_color = [
            style.grid_color[0],
            style.grid_color[1],
            style.grid_color[2],
            0.3,
        ];
        self.base_ctx.set_fill_style_str(&rgba(&track_color));
        self.base_ctx.fill_rect(0.0, track_y, w, track_height);

        // Draw thumb (highlighted, more opaque)
        let thumb_color = [
            style.axis_text_color[0],
            style.axis_text_color[1],
            style.axis_text_color[2],
            0.5,
        ];
        self.base_ctx.set_fill_style_str(&rgba(&thumb_color));

        // Round the thumb corners
        let radius = (track_height / 2.0).min(3.0 * dpr);
        self.base_ctx.begin_path();
        let _ = self
            .base_ctx
            .round_rect_with_f64(thumb_x, track_y, thumb_w, track_height, radius);
        self.base_ctx.fill();
    }
}

#[inline]
fn align_tick_label_x_css(
    text_cache: &mut TextWidthCache,
    ctx: &CanvasRenderingContext2d,
    font_key: &str,
    label: &str,
    x_css: f64,
    axis_css_w: f64,
) -> f64 {
    let label_w = text_cache.measure(ctx, label, font_key);
    let half = label_w / 2.0;
    let left = (x_css - half).floor() + 0.5;
    if left < 0.0 {
        x_css + (0.0 - left)
    } else if left + label_w > axis_css_w {
        x_css - ((left + label_w) - axis_css_w)
    } else {
        x_css
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
