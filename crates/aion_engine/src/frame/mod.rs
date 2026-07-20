//! Backend-neutral frame production for the headless chart model.
//!
//! This is intentionally independent of WebGPU, Canvas2D, and DOM types. Hosts may convert the
//! returned primitives into any raster backend, or inspect them in tests.

use aion_core::format::percentage_formatter::PercentageFormatter;
use aion_core::format::time_formatter::{
    format_crosshair_time, format_tick_label, weight_to_tick_mark_type,
};
use aion_core::model::magnet::{magnet_snap, CrosshairMode};
use aion_core::model::plot_list::{MismatchDirection, PlotList, PlotValueIndex};
use aion_core::model::price_range::PriceRange;
use aion_core::scale::price_scale_core::{PriceScaleCore, PriceScaleMode};
use aion_render::bars::{build_bars, BarItem, BarsParams};
use aion_render::candles::{build_candles, CandleItem, CandlesParams};
use aion_render::color::Color;
use aion_render::draw_list::{Gradient, LineStyle, LineType, Prim};
use aion_render::histogram::{build_histogram, HistogramItem, HistogramParams};

use crate::{ChartEngine, PriceScaleTarget, SeriesKind, PANE_SEPARATOR};

mod axis;
mod conflation;
mod crosshair;
mod series_geometry;
#[cfg(test)]
mod tests;

use conflation::{visible_histogram_rows, visible_line_rows, visible_ohlc};

const UP: Color = Color::rgb(0x26, 0xa6, 0x9a);
const DOWN: Color = Color::rgb(0xef, 0x53, 0x50);
const GRID: Color = Color::rgb(0xd6, 0xdc, 0xde);
const LINE: Color = Color::rgb(0x21, 0x96, 0xf3);
const AREA_LINE: Color = Color::rgb(0x33, 0xd7, 0x78);
const AREA_TOP: Color = Color::rgba(0x2e, 0xdc, 0x87, 102);
const AREA_BOTTOM: Color = Color::rgba(0x28, 0xdd, 0x64, 0);
const HISTOGRAM: Color = Color::rgba(0x26, 0xa6, 0x9a, 0x80);
const VOLUME_UP: Color = Color::rgba(0x26, 0xa6, 0x9a, 0x80);
const VOLUME_DOWN: Color = Color::rgba(0xef, 0x53, 0x50, 0x80);
const BASELINE_TOP_LINE: Color = Color::rgb(0x26, 0xa6, 0x9a);
const BASELINE_BOTTOM_LINE: Color = Color::rgb(0xef, 0x53, 0x50);
const BASELINE_TOP_FILL: Color = Color::rgba(0x26, 0xa6, 0x9a, 0x48);
const BASELINE_BOTTOM_FILL: Color = Color::rgba(0xef, 0x53, 0x50, 0x48);
const LINE_WIDTH: f64 = 3.0;
const CROSSHAIR_COLOR: Color = Color::rgb(0x95, 0x98, 0xa1);
const CROSSHAIR_MARKER_RADIUS: f64 = 4.0;
const CROSSHAIR_MARKER_BORDER_WIDTH: f64 = 2.0;
const MARKER_BORDER_COLOR: Color = Color::rgb(0xff, 0xff, 0xff);

fn ceiled_odd(value: f64) -> f64 {
    let ceiled = value.ceil() as i64;
    if ceiled % 2 == 0 {
        (ceiled - 1) as f64
    } else {
        ceiled as f64
    }
}

fn ceiled_even(value: f64) -> f64 {
    let ceiled = value.ceil() as i64;
    if ceiled % 2 != 0 {
        (ceiled - 1) as f64
    } else {
        ceiled as f64
    }
}

fn marker_envelope_size(bar_spacing: f64) -> f64 {
    ceiled_even(ceiled_odd(bar_spacing.clamp(12.0, 30.0)))
}

fn marker_shape_size(envelope: f64, coefficient: f64) -> f64 {
    ceiled_odd(envelope.clamp(12.0, 30.0) * coefficient)
}

fn marker_margin(bar_spacing: f64) -> f64 {
    ceiled_odd(bar_spacing.clamp(12.0, 30.0) * 0.1).max(3.0)
}

fn marker_auto_scale_margins(markers: &[crate::Marker], bar_spacing: f64) -> (f64, f64) {
    if markers.is_empty() {
        return (0.0, 0.0);
    }
    let margin_value = marker_envelope_size(bar_spacing) * 1.5 + marker_margin(bar_spacing) * 2.0;
    let has_above = markers
        .iter()
        .any(|marker| marker.position == crate::marker_pos::ABOVE);
    let has_below = markers
        .iter()
        .any(|marker| marker.position == crate::marker_pos::BELOW);
    let has_in_bar = markers
        .iter()
        .any(|marker| marker.position == crate::marker_pos::IN_BAR);
    let adjusted = || (margin_value / 2.0).ceil();
    (
        if has_above {
            margin_value
        } else if has_in_bar {
            adjusted()
        } else {
            0.0
        },
        if has_below {
            margin_value
        } else if has_in_bar {
            adjusted()
        } else {
            0.0
        },
    )
}

#[derive(Clone, Debug, Default)]
pub struct FramePane {
    pub top: f64,
    pub height: f64,
    pub scissor: [u32; 4],
    pub under: Vec<Prim>,
    pub main: Vec<Prim>,
    pub points: Vec<[f32; 2]>,
}

#[derive(Clone, Debug, Default)]
pub struct ChartFrame {
    pub width: f64,
    pub height: f64,
    pub pixel_ratio: f64,
    pub panes: Vec<FramePane>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisTextAlign {
    Left,
    Right,
    Center,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisTextMidpoint {
    /// Canvas `middle` baseline without an actual-glyph correction (time ticks and markers).
    None,
    /// Correct using this label's glyph bounds (price-axis labels).
    Label,
    /// Correct using LWC's stable representative time-label sample (crosshair time label).
    StableTime,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AxisLabel {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub color: Color,
    pub align: AxisTextAlign,
    pub midpoint: AxisTextMidpoint,
    pub bold: bool,
    pub background: Option<(f64, f64, f64, f64, Color)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AxisFrame {
    pub labels: Vec<AxisLabel>,
    pub separators: Vec<f64>,
}

#[derive(Clone, Copy)]
struct ResolvedSeries {
    id: usize,
    kind: SeriesKind,
    color: Color,
    up: Color,
    down: Color,
    line_width: f64,
    area_top: Color,
    area_bottom: Color,
    point_markers: bool,
    visible: bool,
    baseline: Option<f64>,
    line_type: LineType,
    scale_target: PriceScaleTarget,
    pane: usize,
    base_value: f64,
}

fn series_scale_target(series: &crate::SeriesEntry) -> PriceScaleTarget {
    if series.overlay {
        PriceScaleTarget::Overlay
    } else if series.left_scale {
        PriceScaleTarget::Left
    } else {
        PriceScaleTarget::Right
    }
}

fn pane_scale(pane: &crate::Pane, target: PriceScaleTarget) -> &PriceScaleCore {
    match target {
        PriceScaleTarget::Right => &pane.price_scale,
        PriceScaleTarget::Left => &pane.left_scale,
        PriceScaleTarget::Overlay => &pane.overlay_scale,
    }
}

fn css_color(value: &str, fallback: Color) -> Color {
    Color::parse_css(value).unwrap_or(fallback)
}

fn translate_prims_x(prims: &mut [Prim], dx: i32) {
    let dxf = dx as f32;
    for prim in prims {
        match prim {
            Prim::Rect { rect, .. } | Prim::RectFrame { rect, .. } => rect.x += dx,
            Prim::HLine { x0, x1, .. } => {
                *x0 += dx;
                *x1 += dx;
            }
            Prim::VLine { x, .. } => *x += dx,
            Prim::RoundRect { x, .. } => *x += dxf,
            Prim::Circle { cx, .. } => *cx += dxf,
            Prim::Triangle { a, b, c, .. } => {
                a[0] += dxf;
                b[0] += dxf;
                c[0] += dxf;
            }
            Prim::Text { x, .. } => *x += dxf,
            Prim::Polyline { .. } | Prim::AreaFill { .. } | Prim::Background { .. } => {}
        }
    }
}

impl ChartEngine {
    /// Recompute pane price ranges for the current visible time window.
    /// Hosts that need scale-dependent layout measurements may call this before building a frame;
    /// `build_frame` calls it as well so standalone backends remain correct.
    pub fn autoscale_visible(&mut self) {
        if let Some((from, to)) = self.visible_range_for_frame() {
            self.autoscale_for_frame(from, to);
        }
    }

    /// Build the visible chart geometry as backend-neutral primitives.
    ///
    /// The frame owns no GPU buffers and performs no browser calls. It is suitable for WebGPU,
    /// Canvas2D, tiny-skia, screenshots, and golden tests alike.
    pub fn build_frame(&mut self) -> ChartFrame {
        let mut frame = ChartFrame::default();
        self.build_frame_into(&mut frame);
        frame
    }

    /// Rebuild a frame while retaining its pane, primitive, and point allocations. Hosts that
    /// repaint repeatedly should keep one `ChartFrame` and call this method instead of allocating
    /// a fresh tree for every cursor/animation frame.
    pub fn build_frame_into(&mut self, output: &mut ChartFrame) {
        self.layout_for_frame();
        let visible = self.visible_range_for_frame();
        self.autoscale_visible();

        // LWC/fancy-canvas renders each pane with its actual bitmap/media ratio, which can differ
        // slightly from devicePixelRatio when a fractional-DPR pane dimension rounds. Using DPR
        // directly shifts bars and grid lines relative to the independently rounded pane bitmap.
        let nominal_dpr = self.dpr.max(0.01);
        let hpr = (self.pane_w * nominal_dpr).round().max(1.0) / self.pane_w.max(1.0);
        let vpr = (self.pane_h * nominal_dpr).round().max(1.0) / self.pane_h.max(1.0);
        let pane_count = self.panes.len().max(1);
        let pane_w_px = (self.pane_w * hpr).round().max(1.0) as u32;
        let pane_left_px = (self.pane_left * nominal_dpr).round().max(0.0) as u32;
        let mut resolved = Vec::with_capacity(self.series.len());
        for s in &self.series {
            let base_value = visible
                .and_then(|(from, _)| self.series_base_value(s.id, from))
                .unwrap_or(0.0);
            resolved.push(ResolvedSeries {
                id: s.id,
                kind: s.kind,
                color: s.line_color,
                up: s.up_color.unwrap_or(UP),
                down: s.down_color.unwrap_or(DOWN),
                line_width: s.line_width.unwrap_or(LINE_WIDTH),
                area_top: s.area_top_color.unwrap_or(AREA_TOP),
                area_bottom: s.area_bottom_color.unwrap_or(AREA_BOTTOM),
                point_markers: s.point_markers,
                visible: s.visible,
                baseline: s.baseline,
                line_type: s.line_type,
                scale_target: series_scale_target(s),
                pane: s.pane_index.min(pane_count - 1),
                base_value,
            });
        }

        output.width = self.pane_left + self.pane_w;
        output.height = self.pane_h;
        output.pixel_ratio = self.dpr;
        output.panes.resize_with(pane_count, FramePane::default);
        output.panes.truncate(pane_count);
        let time_marks = self.time_marks_for_frame();
        for (pi, pane) in self.panes.iter().enumerate() {
            let top_px = (pane.top * vpr).round().max(0.0) as u32;
            let height_px = (pane.height * vpr).round().max(0.0) as u32;
            let out = &mut output.panes[pi];
            out.top = pane.top;
            out.height = pane.height;
            out.scissor = [pane_left_px, top_px, pane_w_px, height_px];
            out.under.clear();
            out.main.clear();
            out.points.clear();
            if let Some((from, to)) = visible {
                self.build_grid_frame(
                    &mut out.under,
                    &time_marks,
                    from,
                    to,
                    pane_w_px as i32,
                    top_px as i32,
                    height_px as i32,
                    hpr,
                    vpr,
                    if pane.price_scale.is_empty() {
                        &pane.left_scale
                    } else {
                        &pane.price_scale
                    },
                );
                for rs in &resolved {
                    if rs.pane != pi || !rs.visible {
                        continue;
                    }
                    let scale = pane_scale(pane, rs.scale_target);
                    match rs.kind {
                        SeriesKind::Candlestick => {
                            self.build_candles_frame(*rs, from, to, hpr, vpr, &mut out.main, scale)
                        }
                        SeriesKind::Bar => {
                            self.build_bars_frame(*rs, from, to, hpr, vpr, &mut out.main, scale)
                        }
                        SeriesKind::Histogram => self.build_histogram_frame(
                            *rs,
                            from,
                            to,
                            hpr,
                            vpr,
                            &mut out.main,
                            scale,
                        ),
                        SeriesKind::Line | SeriesKind::Area => self.build_line_frame(
                            *rs,
                            from,
                            to,
                            hpr,
                            vpr,
                            pane.top + pane.height,
                            &mut out.main,
                            &mut out.points,
                            scale,
                        ),
                        SeriesKind::Baseline => self.build_baseline_frame(
                            *rs,
                            from,
                            to,
                            hpr,
                            vpr,
                            &mut out.main,
                            &mut out.points,
                            scale,
                        ),
                    }
                }
                self.build_markers_frame(pi, from, to, hpr, vpr, &mut out.main);
                self.build_price_lines_frame(pi, &mut out.main, pane_w_px as i32, vpr);
                if pi == 0 {
                    self.build_last_value_line_frame(&mut out.main, pane_w_px as i32, vpr);
                    self.build_last_pulse_frame(&mut out.main, hpr, vpr);
                }
            }
            self.build_crosshair_frame(pi, pane_w_px as i32, hpr, vpr, &mut out.main);
            if pane_left_px != 0 {
                translate_prims_x(&mut out.under, pane_left_px as i32);
                translate_prims_x(&mut out.main, pane_left_px as i32);
                for point in &mut out.points {
                    point[0] += pane_left_px as f32;
                }
            }
        }
    }

    fn layout_for_frame(&mut self) {
        // Hosts may negotiate an inner content width (for example after measuring the price axis).
        // Preserve that negotiated viewport; standalone/native callers start with pane_w/pane_h
        // equal to the CSS size.
        self.pane_w = if self.pane_w > 0.0 {
            self.pane_w
        } else {
            self.css_width.max(1.0)
        };
        self.pane_h = if self.pane_h > 0.0 {
            self.pane_h
        } else {
            self.css_height.max(1.0)
        };
        self.layout_panes(self.pane_h);
    }

    fn visible_range_for_frame(&self) -> Option<(i64, i64)> {
        let n = self.data.merged_times().len() as i64;
        let r = self.time_scale.visible_strict_range()?;
        if n == 0 {
            return None;
        }
        let from = r.left().max(0);
        let to = r.right().min(n - 1);
        (from <= to).then_some((from, to))
    }

    /// Visible merged-time indices for host-side axis labels and hit-testing.
    pub fn visible_range(&self) -> Option<(i64, i64)> {
        self.visible_range_for_frame()
    }

    fn autoscale_for_frame(&mut self, from: i64, to: i64) {
        let n = self.panes.len().max(1);
        let mut main: Vec<Option<PriceRange>> = vec![None; n];
        let mut left: Vec<Option<PriceRange>> = vec![None; n];
        let mut overlay: Vec<Option<PriceRange>> = vec![None; n];
        let mut main_marker_margins = vec![(0.0_f64, 0.0_f64); n];
        let mut left_marker_margins = vec![(0.0_f64, 0.0_f64); n];
        let mut overlay_marker_margins = vec![(0.0_f64, 0.0_f64); n];
        for s in &self.series {
            // Hidden series remain engine-owned so they can be toggled back on, but—matching
            // LWC—they must not contribute to the active price-scale autoscale range.
            if !s.visible {
                continue;
            }
            let mm = self.data.plot_mut(s.id).min_max_on_range_cached(
                from,
                to,
                &[PlotValueIndex::Low, PlotValueIndex::High],
            );
            let Some(mm) = mm else {
                continue;
            };
            let pane_index = s.pane_index.min(n - 1);
            let Some(base_value) = self.series_base_value(s.id, from) else {
                continue;
            };
            let scale_target = series_scale_target(s);
            let scale = pane_scale(&self.panes[pane_index], scale_target);
            let Some(range) =
                scale.price_range_to_logical(&PriceRange::new(mm.min, mm.max), base_value)
            else {
                continue;
            };
            let slot = match scale_target {
                PriceScaleTarget::Right => &mut main[pane_index],
                PriceScaleTarget::Left => &mut left[pane_index],
                PriceScaleTarget::Overlay => &mut overlay[pane_index],
            };
            *slot = Some(match slot.take() {
                Some(old) => old.merge(Some(&range)),
                None => range,
            });
            if s.markers_auto_scale {
                let margins = marker_auto_scale_margins(&s.markers, self.time_scale.bar_spacing());
                let target = match scale_target {
                    PriceScaleTarget::Right => &mut main_marker_margins[pane_index],
                    PriceScaleTarget::Left => &mut left_marker_margins[pane_index],
                    PriceScaleTarget::Overlay => &mut overlay_marker_margins[pane_index],
                };
                target.0 = target.0.max(margins.0);
                target.1 = target.1.max(margins.1);
            }
        }
        for (i, pane) in self.panes.iter_mut().enumerate() {
            let main_auto = pane.price_scale.is_auto_scale();
            let left_auto = pane.left_scale.is_auto_scale();
            let overlay_auto = pane.overlay_scale.is_auto_scale();
            pane.marker_margin_above = if main_auto {
                main_marker_margins[i].0
            } else {
                0.0
            };
            pane.marker_margin_below = if main_auto {
                main_marker_margins[i].1
            } else {
                0.0
            };
            pane.left_marker_margin_above = if left_auto {
                left_marker_margins[i].0
            } else {
                0.0
            };
            pane.left_marker_margin_below = if left_auto {
                left_marker_margins[i].1
            } else {
                0.0
            };
            pane.overlay_marker_margin_above = if overlay_auto {
                overlay_marker_margins[i].0
            } else {
                0.0
            };
            pane.overlay_marker_margin_below = if overlay_auto {
                overlay_marker_margins[i].1
            } else {
                0.0
            };
            pane.refresh_internal_margins();
            if main_auto {
                if let Some(range) = main[i].take() {
                    pane.price_scale.apply_autoscale_range(Some(range), 0.01);
                }
            }
            if left_auto {
                if let Some(range) = left[i].take() {
                    pane.left_scale.apply_autoscale_range(Some(range), 0.01);
                }
            }
            if overlay_auto {
                if let Some(range) = overlay[i].take() {
                    pane.overlay_scale.apply_autoscale_range(
                        Some(range.merge(Some(&PriceRange::new(0.0, 0.0)))),
                        0.01,
                    );
                }
            }
        }
    }

    fn time_marks_for_frame(&mut self) -> Vec<(i64, u8)> {
        let max_width = (12.0 + 4.0) * 5.0 / 8.0 * 8.0;
        self.time_marks(max_width)
    }

    /// Build the time marks used by both the frame grid and host axis labels.
    pub fn time_marks(&mut self, max_label_width: f64) -> Vec<(i64, u8)> {
        self.tick_marks
            .build(self.time_scale.bar_spacing(), max_label_width)
            .iter()
            .map(|m| (m.index, m.weight))
            .collect()
    }
}
