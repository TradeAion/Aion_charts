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

/// Pick a bounded set of rows when several source points occupy the same physical x pixel.
///
/// The normal-spacing path remains unchanged. Once the source spacing drops below one physical
/// pixel, each bucket keeps its first/last rows plus the close extrema, preserving the visible
/// envelope and the line's endpoints while avoiding an O(number-of-source-points) draw list.
fn visible_line_rows(
    plot: &PlotList,
    from: i64,
    to: i64,
    bar_spacing: f64,
    hpr: f64,
    x_at: impl Fn(i64) -> f64,
) -> Vec<usize> {
    let visible = plot.visible_rows(from, to);
    if bar_spacing * hpr >= 1.0 {
        return visible.collect();
    }

    let close = plot.column(PlotValueIndex::Close);
    let indices = plot.indices();
    let mut out = Vec::new();
    let mut bucket_rows = Vec::new();
    let mut bucket: Option<i64> = None;

    let flush = |bucket_rows: &mut Vec<usize>, out: &mut Vec<usize>| {
        let (Some(&first), Some(&last)) = (bucket_rows.first(), bucket_rows.last()) else {
            return;
        };
        let mut low = first;
        let mut high = first;
        for &row in bucket_rows.iter().skip(1) {
            if close[row].is_finite() && (!close[low].is_finite() || close[row] < close[low]) {
                low = row;
            }
            if close[row].is_finite() && (!close[high].is_finite() || close[row] > close[high]) {
                high = row;
            }
        }
        let mut selected = [first, low, high, last];
        selected.sort_unstable();
        for row in selected {
            if out.last().copied() != Some(row) {
                out.push(row);
            }
        }
        bucket_rows.clear();
    };

    for row in visible {
        let current_bucket = x_at(indices[row]).floor() as i64;
        if bucket.is_some_and(|previous| previous != current_bucket) {
            flush(&mut bucket_rows, &mut out);
        }
        bucket = Some(current_bucket);
        bucket_rows.push(row);
    }
    flush(&mut bucket_rows, &mut out);
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct VisibleOhlc {
    /// Physical-pixel x coordinate. Aggregated buckets are pinned to their integer pixel so
    /// adjacent buckets cannot round back onto the same column in the geometry builders.
    x_px: f64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
}

/// Aggregate source OHLC rows that share a physical x pixel.
///
/// Each compressed bucket is itself a valid OHLC bar: first open, maximum high, minimum low, and
/// last close. At normal spacing this is an identity transform, apart from copying the visible
/// values into the small frame-local item list required by the render geometry builders.
fn visible_ohlc(
    plot: &PlotList,
    from: i64,
    to: i64,
    bar_spacing: f64,
    hpr: f64,
    x_at: impl Fn(i64) -> f64,
) -> Vec<VisibleOhlc> {
    let indices = plot.indices();
    let open = plot.column(PlotValueIndex::Open);
    let high = plot.column(PlotValueIndex::High);
    let low = plot.column(PlotValueIndex::Low);
    let close = plot.column(PlotValueIndex::Close);
    let visible = plot.visible_rows(from, to);

    if bar_spacing * hpr >= 1.0 {
        return visible
            .map(|row| VisibleOhlc {
                x_px: x_at(indices[row]),
                open: open[row],
                high: high[row],
                low: low[row],
                close: close[row],
            })
            .collect();
    }

    let mut out = Vec::new();
    let mut current_bucket: Option<i64> = None;
    let mut current: Option<VisibleOhlc> = None;
    for row in visible {
        let bucket = x_at(indices[row]).floor() as i64;
        if current_bucket.is_some_and(|previous| previous != bucket) {
            // `current` is always populated alongside `current_bucket` below.
            if let Some(item) = current.take() {
                out.push(item);
            }
        }

        match current.as_mut() {
            Some(item) => {
                item.high = item.high.max(high[row]);
                item.low = item.low.min(low[row]);
                item.close = close[row];
            }
            None => {
                current = Some(VisibleOhlc {
                    x_px: bucket as f64,
                    open: open[row],
                    high: high[row],
                    low: low[row],
                    close: close[row],
                });
            }
        }
        current_bucket = Some(bucket);
    }
    if let Some(item) = current {
        out.push(item);
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct VisibleHistogramRow {
    x_px: f64,
    source_row: usize,
    /// Geometry-local adjacency key. It is the actual time-point index at normal spacing and the
    /// physical pixel bucket when compressed.
    geometry_time: i64,
}

/// Select one conservative histogram sample per physical pixel, retaining the value with the
/// greatest magnitude so a volume/value spike cannot disappear merely because the scale is
/// compressed. The selected source row also carries its original up/down color classification.
fn visible_histogram_rows(
    plot: &PlotList,
    from: i64,
    to: i64,
    bar_spacing: f64,
    hpr: f64,
    x_at: impl Fn(i64) -> f64,
) -> Vec<VisibleHistogramRow> {
    let indices = plot.indices();
    let close = plot.column(PlotValueIndex::Close);
    let visible = plot.visible_rows(from, to);
    if bar_spacing * hpr >= 1.0 {
        return visible
            .map(|source_row| VisibleHistogramRow {
                x_px: x_at(indices[source_row]),
                source_row,
                geometry_time: indices[source_row],
            })
            .collect();
    }

    let mut out: Vec<VisibleHistogramRow> = Vec::new();
    for source_row in visible {
        let bucket = x_at(indices[source_row]).floor() as i64;
        match out.last_mut() {
            Some(item) if item.geometry_time == bucket => {
                if close[source_row].abs() > close[item.source_row].abs() {
                    item.source_row = source_row;
                }
            }
            _ => out.push(VisibleHistogramRow {
                x_px: bucket as f64,
                source_row,
                geometry_time: bucket,
            }),
        }
    }
    out
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
    fn format_scale_value(&self, scale: &PriceScaleCore, value: f64) -> String {
        if scale.mode() == PriceScaleMode::Percentage {
            PercentageFormatter::default().format(value)
        } else {
            self.price_formatter.format(value)
        }
    }

    /// Build backend-neutral axis label decisions. The host supplies only font measurement; all
    /// visible ranges, scale choices, snapping, formatting, and label positions come from the
    /// engine so Canvas2D, WebGPU text, and native glyph backends share one layout result.
    pub fn build_axis_frame<F>(&mut self, max_label_width: f64, measure: F) -> AxisFrame
    where
        F: Fn(&str) -> f64,
    {
        self.layout_for_frame();
        self.autoscale_visible();
        let mut out = AxisFrame::default();
        let visible = self.visible_range_for_frame();
        let text_color = Color::parse_css(&self.options.get().layout.text_color)
            .unwrap_or(Color::rgb(0x19, 0x19, 0x19));
        let label_bg = Color::rgb(0x13, 0x17, 0x22);
        let white = Color::rgb(0xff, 0xff, 0xff);
        let options = self.options.get();
        let right_text_x = self.pane_left + self.pane_w + 5.0 + 5.0;
        let left_text_x = (self.pane_left - 5.0 - 5.0).max(0.0);
        for pane in &self.panes {
            if options.right_price_scale.visible {
                for mark in pane.price_scale.build_tick_marks(100, 0.0) {
                    let y = mark.coord;
                    if y >= pane.top - 0.5 && y <= pane.top + pane.height + 0.5 {
                        out.labels.push(AxisLabel {
                            text: self.format_scale_value(&pane.price_scale, mark.logical),
                            x: right_text_x,
                            y,
                            color: text_color,
                            align: AxisTextAlign::Left,
                            midpoint: AxisTextMidpoint::Label,
                            bold: false,
                            background: None,
                        });
                    }
                }
            }
            if options.left_price_scale.visible {
                for mark in pane.left_scale.build_tick_marks(100, 0.0) {
                    let y = mark.coord;
                    if y >= pane.top - 0.5 && y <= pane.top + pane.height + 0.5 {
                        out.labels.push(AxisLabel {
                            text: self.format_scale_value(&pane.left_scale, mark.logical),
                            x: left_text_x,
                            y,
                            color: text_color,
                            align: AxisTextAlign::Right,
                            midpoint: AxisTextMidpoint::Label,
                            bold: false,
                            background: None,
                        });
                    }
                }
            }
        }
        if let Some((from, to)) = visible {
            let time_marks = self.time_marks(max_label_width);
            let maximum_weight = time_marks
                .iter()
                .map(|(_, weight)| *weight)
                .max()
                .unwrap_or(0);
            let times = self.data.merged_times();
            for &(index, weight) in &time_marks {
                if index < from || index > to {
                    continue;
                }
                let ts = times[index as usize];
                let kind = weight_to_tick_mark_type(weight, self.time_visible, false);
                out.labels.push(AxisLabel {
                    text: format_tick_label(ts, kind),
                    x: self.pane_left + self.time_scale.index_to_coordinate(index),
                    y: self.pane_h + 1.0 + 5.0 + 3.0 + 12.0 / 2.0,
                    color: text_color,
                    align: AxisTextAlign::Center,
                    midpoint: AxisTextMidpoint::None,
                    bold: weight >= maximum_weight,
                    background: None,
                });
            }
            self.append_marker_labels(&mut out.labels, from, to);
        }
        self.append_price_line_labels(&mut out.labels, &measure);
        self.append_last_value_label(&mut out.labels, &measure);
        self.append_crosshair_labels(&mut out.labels, &measure, label_bg, white);
        out.separators = self
            .panes
            .iter()
            .skip(1)
            .map(|p| p.top - PANE_SEPARATOR)
            .collect();
        out
    }

    /// LWC-compatible right-axis width negotiated from engine-formatted labels and host glyph
    /// measurement. The host contributes font metrics only; label selection and formatting stay
    /// headless. The result is snapped to an even media-pixel width.
    pub fn optimal_price_axis_width<F>(&mut self, measure: F) -> f64
    where
        F: Fn(&str) -> f64,
    {
        self.optimal_price_axis_width_for(PriceScaleTarget::Right, measure)
    }

    /// Measure one visible side independently. Overlay scales deliberately share no axis strip.
    pub fn optimal_price_axis_width_for<F>(&mut self, target: PriceScaleTarget, measure: F) -> f64
    where
        F: Fn(&str) -> f64,
    {
        const AXIS_BORDER_SIZE: f64 = 1.0;
        const AXIS_TICK_LENGTH: f64 = 5.0;
        const PRICE_PADDING_INNER: f64 = 5.0;
        const PRICE_PADDING_OUTER: f64 = 5.0;
        const PRICE_LABEL_OFFSET: f64 = 5.0;
        const PRICE_DEFAULT_TEXT_WIDTH: f64 = 34.0;

        let frame = self.build_axis_frame(80.0, &measure);
        let wanted_align = match target {
            PriceScaleTarget::Left => AxisTextAlign::Right,
            PriceScaleTarget::Right | PriceScaleTarget::Overlay => AxisTextAlign::Left,
        };
        let max_text_width = frame
            .labels
            .iter()
            .filter(|label| label.align == wanted_align)
            .map(|label| measure(&label.text))
            .fold(0.0_f64, f64::max);
        let text_width = if max_text_width > 0.0 {
            max_text_width
        } else {
            PRICE_DEFAULT_TEXT_WIDTH
        };
        let width = (AXIS_BORDER_SIZE
            + AXIS_TICK_LENGTH
            + PRICE_PADDING_INNER
            + PRICE_PADDING_OUTER
            + PRICE_LABEL_OFFSET
            + text_width)
            .ceil();
        width + (width as i64 % 2) as f64
    }

    fn append_marker_labels(&self, labels: &mut Vec<AxisLabel>, from: i64, to: i64) {
        let times = self.data.merged_times();
        for (pi, pane) in self.panes.iter().enumerate() {
            for s in &self.series {
                if !s.visible || s.pane_index.min(self.panes.len() - 1) != pi {
                    continue;
                }
                let scale = pane_scale(pane, series_scale_target(s));
                let Some(base_value) = self.series_base_value(s.id, from) else {
                    continue;
                };
                if scale.is_empty() {
                    continue;
                }
                let plot = self.data.plot(s.id);
                for marker in &s.markers {
                    if marker.text.is_empty() {
                        continue;
                    }
                    let Ok(pos) = times.binary_search(&marker.time) else {
                        continue;
                    };
                    let index = pos as i64;
                    if index < from || index > to {
                        continue;
                    }
                    let Some(row) = plot.search(index, MismatchDirection::None) else {
                        continue;
                    };
                    let high = plot.value_at(row, PlotValueIndex::High);
                    let low = plot.value_at(row, PlotValueIndex::Low);
                    let close = plot.value_at(row, PlotValueIndex::Close);
                    let x = self.pane_left + self.time_scale.index_to_coordinate(index);
                    let envelope = marker_envelope_size(self.time_scale.bar_spacing());
                    let half_envelope = envelope / 2.0;
                    let margin = marker_margin(self.time_scale.bar_spacing());
                    let text_height = self.options.get().layout.font_size;
                    let y = match marker.position {
                        crate::marker_pos::BELOW => {
                            scale.price_to_coordinate(low, base_value)
                                + envelope
                                + margin * 2.0
                                + text_height * 0.6
                        }
                        crate::marker_pos::ABOVE => {
                            scale.price_to_coordinate(high, base_value)
                                - envelope
                                - margin
                                - text_height * 0.6
                        }
                        _ => {
                            scale.price_to_coordinate(close, base_value)
                                + half_envelope
                                + margin
                                + text_height * 0.6
                        }
                    };
                    if y >= pane.top && y <= pane.top + pane.height && x >= 0.0 && x <= self.pane_w
                    {
                        labels.push(AxisLabel {
                            text: marker.text.clone(),
                            x,
                            y,
                            color: marker.color,
                            align: AxisTextAlign::Center,
                            midpoint: AxisTextMidpoint::None,
                            bold: false,
                            background: None,
                        });
                    }
                }
            }
        }
    }

    fn append_price_line_labels<F>(&self, labels: &mut Vec<AxisLabel>, measure: &F)
    where
        F: Fn(&str) -> f64,
    {
        for (pi, pane) in self.panes.iter().enumerate() {
            for s in &self.series {
                if s.pane_index.min(self.panes.len() - 1) != pi {
                    continue;
                }
                let target = series_scale_target(s);
                let scale = pane_scale(pane, target);
                let Some(base_value) = self.visible_series_base_value(s.id) else {
                    continue;
                };
                for line in &s.price_lines {
                    if scale.is_empty() {
                        continue;
                    }
                    let y = scale.price_to_coordinate(line.price, base_value);
                    if y < pane.top || y > pane.top + pane.height {
                        continue;
                    }
                    let text = if line.title.is_empty() {
                        self.format_scale_value(
                            scale,
                            scale.price_to_logical_value(line.price, base_value),
                        )
                    } else {
                        line.title.clone()
                    };
                    let width = 1.0 + 5.0 + 5.0 + 5.0 + measure(&text);
                    let height = 12.0 + 2.5 * 2.0;
                    let (x, align, background_x) = if target == PriceScaleTarget::Left {
                        (
                            self.pane_left - 10.0,
                            AxisTextAlign::Right,
                            self.pane_left - width,
                        )
                    } else {
                        (
                            self.pane_left + self.pane_w + 10.0,
                            AxisTextAlign::Left,
                            self.pane_left + self.pane_w,
                        )
                    };
                    labels.push(AxisLabel {
                        text,
                        x,
                        y,
                        color: line.color.contrast_text(),
                        align,
                        midpoint: AxisTextMidpoint::Label,
                        bold: false,
                        background: Some((
                            background_x,
                            y - height / 2.0,
                            width,
                            height,
                            line.color,
                        )),
                    });
                }
            }
        }
    }

    fn append_last_value_label<F>(&self, labels: &mut Vec<AxisLabel>, measure: &F)
    where
        F: Fn(&str) -> f64,
    {
        let series = &self.series[0];
        let target = series_scale_target(series);
        let plot = self.data.plot(series.id);
        let scale = pane_scale(&self.panes[0], target);
        if plot.is_empty() || scale.is_empty() {
            return;
        }
        let row = plot.size() - 1;
        let close = plot.value_at(row, PlotValueIndex::Close);
        let Some(base_value) = self.visible_series_base_value(series.id) else {
            return;
        };
        let y = scale.price_to_coordinate(close, base_value);
        if y < 0.0 || y > self.pane_h {
            return;
        }
        let color = if close >= plot.value_at(row, PlotValueIndex::Open) {
            Color::rgb(0x26, 0xa6, 0x9a)
        } else {
            Color::rgb(0xef, 0x53, 0x50)
        };
        let text = self.format_scale_value(scale, scale.price_to_logical_value(close, base_value));
        let width = 1.0 + 5.0 + 5.0 + 5.0 + measure(&text);
        let height = 12.0 + 2.5 * 2.0;
        let (x, align, background_x) = if target == PriceScaleTarget::Left {
            (
                self.pane_left - 10.0,
                AxisTextAlign::Right,
                self.pane_left - width,
            )
        } else {
            (
                self.pane_left + self.pane_w + 10.0,
                AxisTextAlign::Left,
                self.pane_left + self.pane_w,
            )
        };
        labels.push(AxisLabel {
            text,
            x,
            y,
            color: color.contrast_text(),
            align,
            midpoint: AxisTextMidpoint::Label,
            bold: false,
            background: Some((background_x, y - height / 2.0, width, height, color)),
        });
    }

    fn append_crosshair_labels<F>(
        &self,
        labels: &mut Vec<AxisLabel>,
        measure: &F,
        bg: Color,
        white: Color,
    ) where
        F: Fn(&str) -> f64,
    {
        let Some((x_css, y_css)) = self.crosshair else {
            return;
        };
        let Some((from, to)) = self.visible_range_for_frame() else {
            return;
        };
        if self.crosshair_mode == CrosshairMode::Hidden
            || self.data.plot(self.series[0].id).is_empty()
        {
            return;
        }
        if let Some(pi) = self
            .panes
            .iter()
            .position(|p| y_css >= p.top && y_css <= p.top + p.height)
        {
            let series = self
                .series
                .iter()
                .find(|series| series.pane_index == pi && !series.overlay && series.visible);
            let target = series
                .map(series_scale_target)
                .unwrap_or(PriceScaleTarget::Right);
            let scale = pane_scale(&self.panes[pi], target);
            if !scale.is_empty() {
                let base_value = series
                    .and_then(|series| self.series_base_value(series.id, from))
                    .unwrap_or(0.0);
                let (price, snap_y) = if pi == 0 {
                    self.crosshair_snap(x_css, y_css, from, to)
                } else {
                    (scale.coordinate_to_price(y_css, base_value), y_css)
                };
                let text =
                    self.format_scale_value(scale, scale.price_to_logical_value(price, base_value));
                let width = 1.0 + 5.0 + 5.0 + 5.0 + measure(&text);
                let height = 12.0 + 2.5 * 2.0;
                let (label_x, align, background_x) = if target == PriceScaleTarget::Left {
                    (
                        self.pane_left - 10.0,
                        AxisTextAlign::Right,
                        self.pane_left - width,
                    )
                } else {
                    (
                        self.pane_left + self.pane_w + 10.0,
                        AxisTextAlign::Left,
                        self.pane_left + self.pane_w,
                    )
                };
                labels.push(AxisLabel {
                    text,
                    x: label_x,
                    y: snap_y,
                    color: white,
                    align,
                    midpoint: AxisTextMidpoint::Label,
                    bold: false,
                    background: Some((background_x, snap_y - height / 2.0, width, height, bg)),
                });
            }
        }
        if x_css <= self.pane_w {
            let index = self.snapped_crosshair_index(x_css, from, to);
            let text = format_crosshair_time(
                self.data.merged_times()[index as usize],
                self.time_visible,
                false,
            );
            let width = measure(&text) + 9.0 * 2.0;
            let height = 12.0 + 3.0 + 3.0;
            let x = self.pane_left + self.time_scale.index_to_coordinate(index);
            let box_x = (x - width / 2.0).clamp(
                self.pane_left,
                (self.pane_left + self.pane_w - width).max(self.pane_left),
            );
            labels.push(AxisLabel {
                text,
                x: box_x + width / 2.0,
                y: self.pane_h + 1.0 + height / 2.0,
                color: white,
                align: AxisTextAlign::Center,
                midpoint: AxisTextMidpoint::StableTime,
                bold: false,
                background: Some((box_x, self.pane_h + 1.0, width, height, bg)),
            });
        }
    }

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

    fn build_crosshair_frame(
        &self,
        pane_index: usize,
        pane_w_px: i32,
        hpr: f64,
        vpr: f64,
        out: &mut Vec<Prim>,
    ) {
        let Some((x_css, y_css)) = self.crosshair else {
            return;
        };
        let Some((from, to)) = self.visible_range_for_frame() else {
            return;
        };
        if self.crosshair_mode == aion_core::model::magnet::CrosshairMode::Hidden
            || x_css > self.pane_w
            || y_css > self.pane_h
            || self.data.plot(self.series[0].id).is_empty()
        {
            return;
        }
        let index = self.snapped_crosshair_index(x_css, from, to);
        let snapped_x = self.time_scale.index_to_coordinate(index);
        let line_width = 1f64.max(hpr.floor()) as i32;
        let ch = self.options.get().crosshair;
        let vert_color = css_color(&ch.vert_line.color, CROSSHAIR_COLOR);
        let horz_color = css_color(&ch.horz_line.color, CROSSHAIR_COLOR);
        let pane = &self.panes[pane_index];
        if ch.vert_line.visible {
            out.push(Prim::VLine {
                x: (snapped_x * hpr).round() as i32,
                y0: (pane.top * vpr).round() as i32,
                y1: ((pane.top + pane.height) * vpr).round() as i32,
                width: line_width,
                style: LineStyle::LargeDashed,
                color: vert_color,
            });
        }
        if self.pane_at_y(y_css) != Some(pane_index) {
            return;
        }
        let snap_y = if pane_index == 0 {
            self.crosshair_snap(x_css, y_css, from, to).1
        } else {
            y_css
        };
        if ch.horz_line.visible {
            out.push(Prim::HLine {
                y: (snap_y * vpr).round() as i32,
                x0: 0,
                x1: pane_w_px,
                width: line_width,
                style: LineStyle::LargeDashed,
                color: horz_color,
            });
        }
        if pane_index == 0 && matches!(self.series[0].kind, SeriesKind::Line | SeriesKind::Area) {
            let plot = self.data.plot(self.series[0].id);
            let Some(row) = plot.search(index, MismatchDirection::None) else {
                return;
            };
            let close = plot.value_at(row, PlotValueIndex::Close);
            let base_value = self
                .series_base_value(self.series[0].id, from)
                .unwrap_or(0.0);
            let scale = pane_scale(&self.panes[0], series_scale_target(&self.series[0]));
            let cx = (snapped_x * hpr) as f32;
            let cy = (scale.price_to_coordinate(close, base_value) * vpr) as f32;
            let fill = if self.series[0].kind == SeriesKind::Area {
                AREA_LINE
            } else {
                LINE
            };
            let outer = ((CROSSHAIR_MARKER_RADIUS + CROSSHAIR_MARKER_BORDER_WIDTH) * vpr) as f32;
            let inner = (CROSSHAIR_MARKER_RADIUS * vpr) as f32;
            out.push(Prim::Circle {
                cx,
                cy,
                radius: outer,
                fill: MARKER_BORDER_COLOR,
                stroke_width: 0.0,
                stroke: MARKER_BORDER_COLOR,
            });
            out.push(Prim::Circle {
                cx,
                cy,
                radius: inner,
                fill,
                stroke_width: 0.0,
                stroke: fill,
            });
        }
    }

    fn pane_at_y(&self, y: f64) -> Option<usize> {
        self.panes
            .iter()
            .position(|p| y >= p.top && y <= p.top + p.height)
    }

    fn snapped_crosshair_index(&self, x_css: f64, from: i64, to: i64) -> i64 {
        self.time_scale.coordinate_to_index(x_css).clamp(from, to)
    }

    fn crosshair_snap(&self, x_css: f64, y_css: f64, from: i64, to: i64) -> (f64, f64) {
        let index = self.snapped_crosshair_index(x_css, from, to);
        let plot = self.data.plot(self.series[0].id);
        let base_value = self
            .series_base_value(self.series[0].id, from)
            .unwrap_or(0.0);
        let scale = pane_scale(&self.panes[0], series_scale_target(&self.series[0]));
        let row = plot.search(index, MismatchDirection::NearestLeft);
        let Some(row) = row else {
            return (scale.coordinate_to_price(y_css, base_value), y_css);
        };
        let close = plot.value_at(row, PlotValueIndex::Close);
        let price = match self.crosshair_mode {
            aion_core::model::magnet::CrosshairMode::Normal
            | aion_core::model::magnet::CrosshairMode::Hidden => {
                return (scale.coordinate_to_price(y_css, base_value), y_css)
            }
            aion_core::model::magnet::CrosshairMode::Magnet => close,
            aion_core::model::magnet::CrosshairMode::MagnetOhlc => {
                let open = plot.value_at(row, PlotValueIndex::Open);
                let high = plot.value_at(row, PlotValueIndex::High);
                let low = plot.value_at(row, PlotValueIndex::Low);
                let candidates = [
                    (open, scale.price_to_coordinate(open, base_value)),
                    (high, scale.price_to_coordinate(high, base_value)),
                    (low, scale.price_to_coordinate(low, base_value)),
                    (close, scale.price_to_coordinate(close, base_value)),
                ];
                magnet_snap(y_css, &candidates).unwrap_or(close)
            }
        };
        (price, scale.price_to_coordinate(price, base_value))
    }

    #[allow(clippy::too_many_arguments)]
    fn build_grid_frame(
        &self,
        out: &mut Vec<Prim>,
        marks: &[(i64, u8)],
        from: i64,
        to: i64,
        width: i32,
        top: i32,
        height: i32,
        hpr: f64,
        vpr: f64,
        scale: &aion_core::scale::price_scale_core::PriceScaleCore,
    ) {
        let grid = self.options.get().grid;
        let vert = css_color(&grid.vert_lines.color, GRID);
        let horz = css_color(&grid.horz_lines.color, GRID);
        let lw = 1f64.max(hpr.floor()) as i32;
        if grid.vert_lines.visible {
            for &(idx, _) in marks {
                if idx >= from && idx <= to {
                    out.push(Prim::VLine {
                        x: (self.time_scale.index_to_coordinate(idx) * hpr).round() as i32,
                        y0: top - lw,
                        y1: top + height + lw,
                        width: lw,
                        style: LineStyle::Solid,
                        color: vert,
                    });
                }
            }
        }
        if grid.horz_lines.visible {
            for mark in scale.build_tick_marks(100, 0.0) {
                out.push(Prim::HLine {
                    y: (mark.coord * vpr).round() as i32,
                    x0: -lw,
                    x1: width + lw,
                    width: lw,
                    style: LineStyle::Solid,
                    color: horz,
                });
            }
        }
    }

    fn build_candles_frame(
        &self,
        rs: ResolvedSeries,
        from: i64,
        to: i64,
        hpr: f64,
        vpr: f64,
        out: &mut Vec<Prim>,
        scale: &aion_core::scale::price_scale_core::PriceScaleCore,
    ) {
        let plot = self.data.plot(rs.id);
        let visible = visible_ohlc(
            plot,
            from,
            to,
            self.time_scale.bar_spacing(),
            hpr,
            |index| self.time_scale.index_to_coordinate(index) * hpr,
        );
        let items = visible
            .into_iter()
            .map(|bar| {
                let color = if bar.close >= bar.open {
                    rs.up
                } else {
                    rs.down
                };
                CandleItem {
                    x: bar.x_px / hpr,
                    open_y: scale.price_to_coordinate(bar.open, rs.base_value),
                    high_y: scale.price_to_coordinate(bar.high, rs.base_value),
                    low_y: scale.price_to_coordinate(bar.low, rs.base_value),
                    close_y: scale.price_to_coordinate(bar.close, rs.base_value),
                    body_color: color,
                    border_color: color,
                    wick_color: color,
                }
            })
            .collect::<Vec<_>>();
        build_candles(
            &items,
            &CandlesParams {
                bar_spacing: self.time_scale.bar_spacing(),
                horizontal_pixel_ratio: hpr,
                vertical_pixel_ratio: vpr,
                wick_visible: true,
                border_visible: true,
            },
            out,
        );
    }

    fn build_bars_frame(
        &self,
        rs: ResolvedSeries,
        from: i64,
        to: i64,
        hpr: f64,
        vpr: f64,
        out: &mut Vec<Prim>,
        scale: &aion_core::scale::price_scale_core::PriceScaleCore,
    ) {
        let plot = self.data.plot(rs.id);
        let visible = visible_ohlc(
            plot,
            from,
            to,
            self.time_scale.bar_spacing(),
            hpr,
            |index| self.time_scale.index_to_coordinate(index) * hpr,
        );
        let items = visible
            .into_iter()
            .map(|bar| BarItem {
                x: bar.x_px / hpr,
                open_y: scale.price_to_coordinate(bar.open, rs.base_value),
                high_y: scale.price_to_coordinate(bar.high, rs.base_value),
                low_y: scale.price_to_coordinate(bar.low, rs.base_value),
                close_y: scale.price_to_coordinate(bar.close, rs.base_value),
                color: if bar.close >= bar.open {
                    rs.up
                } else {
                    rs.down
                },
            })
            .collect::<Vec<_>>();
        build_bars(
            &items,
            &BarsParams {
                bar_spacing: self.time_scale.bar_spacing(),
                horizontal_pixel_ratio: hpr,
                vertical_pixel_ratio: vpr,
                open_visible: true,
                thin_bars: true,
            },
            out,
        );
    }

    fn build_histogram_frame(
        &self,
        rs: ResolvedSeries,
        from: i64,
        to: i64,
        hpr: f64,
        vpr: f64,
        out: &mut Vec<Prim>,
        scale: &aion_core::scale::price_scale_core::PriceScaleCore,
    ) {
        let plot = self.data.plot(rs.id);
        let idxs = plot.indices();
        let c = plot.column(PlotValueIndex::Close);
        let base = scale.price_to_coordinate(0.0, rs.base_value);
        let solid = if rs.color != LINE {
            rs.color
        } else {
            HISTOGRAM
        };
        let main = self.data.plot(self.series[0].id);
        let visible = visible_histogram_rows(
            plot,
            from,
            to,
            self.time_scale.bar_spacing(),
            hpr,
            |index| self.time_scale.index_to_coordinate(index) * hpr,
        );
        let items = visible
            .into_iter()
            .map(|item| {
                let r = item.source_row;
                let color = if self.series[rs.id].histogram_updown {
                    match main.search(idxs[r], MismatchDirection::None) {
                        Some(row)
                            if main.value_at(row, PlotValueIndex::Close)
                                >= main.value_at(row, PlotValueIndex::Open) =>
                        {
                            VOLUME_UP
                        }
                        Some(_) => VOLUME_DOWN,
                        None => solid,
                    }
                } else {
                    solid
                };
                HistogramItem {
                    x: item.x_px / hpr,
                    y: scale.price_to_coordinate(c[r], rs.base_value),
                    time: item.geometry_time,
                    color,
                }
            })
            .collect::<Vec<_>>();
        build_histogram(
            &items,
            &HistogramParams {
                bar_spacing: self.time_scale.bar_spacing(),
                horizontal_pixel_ratio: hpr,
                vertical_pixel_ratio: vpr,
                histogram_base: base,
            },
            out,
        );
    }

    fn build_line_frame(
        &self,
        rs: ResolvedSeries,
        from: i64,
        to: i64,
        hpr: f64,
        vpr: f64,
        band_bottom: f64,
        out: &mut Vec<Prim>,
        points: &mut Vec<[f32; 2]>,
        scale: &aion_core::scale::price_scale_core::PriceScaleCore,
    ) {
        let plot = self.data.plot(rs.id);
        let idxs = plot.indices();
        let c = plot.column(PlotValueIndex::Close);
        let first = points.len() as u32;
        let rows = visible_line_rows(
            plot,
            from,
            to,
            self.time_scale.bar_spacing(),
            hpr,
            |index| self.time_scale.index_to_coordinate(index) * hpr,
        );
        for r in rows {
            points.push([
                (self.time_scale.index_to_coordinate(idxs[r]) * hpr) as f32,
                (scale.price_to_coordinate(c[r], rs.base_value) * vpr) as f32,
            ]);
        }
        let count = points.len() as u32 - first;
        if count == 0 {
            return;
        }
        let color = if rs.color != LINE {
            rs.color
        } else if rs.kind == SeriesKind::Area {
            AREA_LINE
        } else {
            LINE
        };
        if rs.kind == SeriesKind::Area {
            out.push(Prim::AreaFill {
                first_point: first,
                point_count: count,
                base_y: (band_bottom * vpr) as f32,
                line_type: self.series[rs.id].line_type,
                gradient: Gradient {
                    top: rs.area_top,
                    bottom: rs.area_bottom,
                },
            });
        }
        out.push(Prim::Polyline {
            first_point: first,
            point_count: count,
            width: (rs.line_width * vpr) as f32,
            style: LineStyle::Solid,
            line_type: rs.line_type,
            color,
        });
        if rs.point_markers {
            let radius = (rs.line_width + 1.0).max(3.0);
            if self.time_scale.bar_spacing() >= 2.0 * radius + 2.0 {
                for i in first..first + count {
                    let [cx, cy] = points[i as usize];
                    out.push(Prim::Circle {
                        cx,
                        cy,
                        radius: (radius * vpr) as f32,
                        fill: color,
                        stroke_width: 0.0,
                        stroke: color,
                    });
                }
            }
        }
    }

    fn build_baseline_frame(
        &self,
        rs: ResolvedSeries,
        from: i64,
        to: i64,
        hpr: f64,
        vpr: f64,
        out: &mut Vec<Prim>,
        points: &mut Vec<[f32; 2]>,
        scale: &aion_core::scale::price_scale_core::PriceScaleCore,
    ) {
        let plot = self.data.plot(rs.id);
        let idxs = plot.indices();
        let close = plot.column(PlotValueIndex::Close);
        let rows = visible_line_rows(
            plot,
            from,
            to,
            self.time_scale.bar_spacing(),
            hpr,
            |index| self.time_scale.index_to_coordinate(index) * hpr,
        );
        if rows.len() < 2 {
            return;
        }
        let baseline_price = rs.baseline.unwrap_or_else(|| {
            let mut min = f64::INFINITY;
            let mut max = f64::NEG_INFINITY;
            for &row in &rows {
                min = min.min(close[row]);
                max = max.max(close[row]);
            }
            (min + max) / 2.0
        });
        let baseline_y = scale.price_to_coordinate(baseline_price, rs.base_value);
        for pair in rows.windows(2) {
            let a_row = pair[0];
            let b_row = pair[1];
            let a = (
                self.time_scale.index_to_coordinate(idxs[a_row]),
                scale.price_to_coordinate(close[a_row], rs.base_value),
            );
            let b = (
                self.time_scale.index_to_coordinate(idxs[b_row]),
                scale.price_to_coordinate(close[b_row], rs.base_value),
            );
            let mut segments = vec![(a, b)];
            if (a.1 < baseline_y) != (b.1 < baseline_y) && (b.1 - a.1).abs() > 1e-9 {
                let t = (baseline_y - a.1) / (b.1 - a.1);
                let crossing = (a.0 + (b.0 - a.0) * t, baseline_y);
                segments = vec![(a, crossing), (crossing, b)];
            }
            for (s0, s1) in segments {
                let above = (s0.1 + s1.1) * 0.5 < baseline_y;
                let first = points.len() as u32;
                points.push([(s0.0 * hpr) as f32, (s0.1 * vpr) as f32]);
                points.push([(s1.0 * hpr) as f32, (s1.1 * vpr) as f32]);
                let line = if above {
                    BASELINE_TOP_LINE
                } else {
                    BASELINE_BOTTOM_LINE
                };
                let fill = if above {
                    BASELINE_TOP_FILL
                } else {
                    BASELINE_BOTTOM_FILL
                };
                out.push(Prim::AreaFill {
                    first_point: first,
                    point_count: 2,
                    base_y: (baseline_y * vpr) as f32,
                    line_type: LineType::Simple,
                    gradient: Gradient {
                        top: fill,
                        bottom: fill,
                    },
                });
                out.push(Prim::Polyline {
                    first_point: first,
                    point_count: 2,
                    width: (LINE_WIDTH * vpr) as f32,
                    style: LineStyle::Solid,
                    line_type: LineType::Simple,
                    color: line,
                });
            }
        }
    }

    fn build_price_lines_frame(
        &self,
        pane_index: usize,
        out: &mut Vec<Prim>,
        width: i32,
        vpr: f64,
    ) {
        let pane = &self.panes[pane_index];
        let min_width = 1f64.max(vpr.floor()) as i32;
        for series in &self.series {
            if series.pane_index.min(self.panes.len() - 1) != pane_index {
                continue;
            }
            let scale = pane_scale(pane, series_scale_target(series));
            if scale.is_empty() {
                continue;
            }
            let Some(base_value) = self.visible_series_base_value(series.id) else {
                continue;
            };
            for line in &series.price_lines {
                out.push(Prim::HLine {
                    y: (scale.price_to_coordinate(line.price, base_value) * vpr).round() as i32,
                    x0: 0,
                    x1: width,
                    width: line.width.max(min_width),
                    style: line.style,
                    color: line.color,
                });
            }
        }
    }

    fn build_markers_frame(
        &self,
        pane_index: usize,
        from: i64,
        to: i64,
        hpr: f64,
        vpr: f64,
        out: &mut Vec<Prim>,
    ) {
        let pane = &self.panes[pane_index];
        let times = self.data.merged_times();
        for series in &self.series {
            if !series.visible || series.pane_index.min(self.panes.len() - 1) != pane_index {
                continue;
            }
            let scale = pane_scale(pane, series_scale_target(series));
            if scale.is_empty() {
                continue;
            }
            let Some(base_value) = self.series_base_value(series.id, from) else {
                continue;
            };
            let plot = self.data.plot(series.id);
            for marker in &series.markers {
                let Ok(pos) = times.binary_search(&marker.time) else {
                    continue;
                };
                let index = pos as i64;
                if index < from || index > to {
                    continue;
                }
                let Some(row) = plot.search(index, MismatchDirection::None) else {
                    continue;
                };
                let high = plot.value_at(row, PlotValueIndex::High);
                let low = plot.value_at(row, PlotValueIndex::Low);
                let close = plot.value_at(row, PlotValueIndex::Close);
                let x = (self.time_scale.index_to_coordinate(index) * hpr) as f32;
                let envelope = marker_envelope_size(self.time_scale.bar_spacing());
                let half_envelope = (envelope * 0.5 * vpr) as f32;
                let margin = (marker_margin(self.time_scale.bar_spacing()) * vpr) as f32;
                let y = match marker.position {
                    crate::marker_pos::ABOVE => {
                        (scale.price_to_coordinate(high, base_value) * vpr) as f32
                            - half_envelope
                            - margin
                    }
                    crate::marker_pos::BELOW => {
                        (scale.price_to_coordinate(low, base_value) * vpr) as f32
                            + half_envelope
                            + margin
                    }
                    _ => (scale.price_to_coordinate(close, base_value) * vpr) as f32,
                };
                match marker.shape {
                    crate::marker_shape::SQUARE => {
                        let size = (marker_shape_size(envelope, 0.7) * vpr) as f32;
                        out.push(Prim::RoundRect {
                            x: x - size * 0.5,
                            y: y - size * 0.5,
                            w: size,
                            h: size,
                            radii: [0.0; 4],
                            fill: marker.color,
                            border_width: 0.0,
                            border_color: marker.color,
                        });
                    }
                    crate::marker_shape::ARROW_UP | crate::marker_shape::ARROW_DOWN => {
                        let arrow_size = marker_shape_size(envelope, 1.0);
                        let half_arrow = (((arrow_size - 1.0) * 0.5) * vpr) as f32;
                        let base_size = ceiled_odd(envelope / 2.0);
                        let half_base = (((base_size - 1.0) * 0.5) * vpr) as f32;
                        let up = marker.shape == crate::marker_shape::ARROW_UP;
                        out.push(Prim::Triangle {
                            a: [x, y + if up { -half_arrow } else { half_arrow }],
                            b: [x - half_arrow, y],
                            c: [x + half_arrow, y],
                            color: marker.color,
                        });
                        out.push(Prim::RoundRect {
                            x: x - half_base,
                            y: if up { y } else { y - half_arrow },
                            w: half_base * 2.0,
                            h: half_arrow,
                            radii: [0.0; 4],
                            fill: marker.color,
                            border_width: 0.0,
                            border_color: marker.color,
                        });
                    }
                    _ => {
                        let radius =
                            (((marker_shape_size(envelope, 0.8) - 1.0) * 0.5) * vpr) as f32;
                        out.push(Prim::Circle {
                            cx: x,
                            cy: y,
                            radius,
                            fill: marker.color,
                            stroke_width: 0.0,
                            stroke: marker.color,
                        });
                    }
                }
            }
        }
    }

    fn build_last_value_line_frame(&self, out: &mut Vec<Prim>, width: i32, vpr: f64) {
        let series = &self.series[0];
        let scale = pane_scale(&self.panes[0], series_scale_target(series));
        let plot = self.data.plot(series.id);
        if plot.is_empty() || scale.is_empty() {
            return;
        }
        let last = plot.size() - 1;
        let close = plot.value_at(last, PlotValueIndex::Close);
        let Some(base_value) = self.visible_series_base_value(self.series[0].id) else {
            return;
        };
        let color = match self.series[0].kind {
            SeriesKind::Line => LINE,
            SeriesKind::Area => AREA_LINE,
            SeriesKind::Histogram => HISTOGRAM,
            _ => {
                let open = plot.value_at(last, PlotValueIndex::Open);
                if close >= open {
                    UP
                } else {
                    DOWN
                }
            }
        };
        out.push(Prim::HLine {
            y: (scale.price_to_coordinate(close, base_value) * vpr).round() as i32,
            x0: 0,
            x1: width,
            width: 1f64.max(vpr.floor()) as i32,
            style: LineStyle::Dashed,
            color,
        });
    }

    fn build_last_pulse_frame(&self, out: &mut Vec<Prim>, hpr: f64, vpr: f64) {
        if !self.series[0].last_price_animation {
            return;
        }
        let series = &self.series[0];
        let scale = pane_scale(&self.panes[0], series_scale_target(series));
        let plot = self.data.plot(series.id);
        if plot.is_empty() || scale.is_empty() {
            return;
        }
        let last = plot.size() - 1;
        let Some(&index) = plot.indices().last() else {
            return;
        };
        let close = plot.value_at(last, PlotValueIndex::Close);
        let Some(base_value) = self.visible_series_base_value(self.series[0].id) else {
            return;
        };
        let cx = (self.time_scale.index_to_coordinate(index) * hpr) as f32;
        let cy = (scale.price_to_coordinate(close, base_value) * vpr) as f32;
        let base = match self.series[0].kind {
            SeriesKind::Line => LINE,
            SeriesKind::Area => AREA_LINE,
            SeriesKind::Histogram => HISTOGRAM,
            _ => {
                let open = plot.value_at(last, PlotValueIndex::Open);
                if close >= open {
                    UP
                } else {
                    DOWN
                }
            }
        };
        const PERIOD_MS: f64 = 2600.0;
        let phase = (self.animation_time.rem_euclid(PERIOD_MS) / PERIOD_MS) as f32;
        let ring = Color::rgba(
            base.r(),
            base.g(),
            base.b(),
            ((1.0 - phase) * 0.35 * 255.0) as u8,
        );
        out.push(Prim::Circle {
            cx,
            cy,
            radius: (4.0 + phase * 10.0) * vpr as f32,
            fill: ring,
            stroke_width: 0.0,
            stroke: ring,
        });
        out.push(Prim::Circle {
            cx,
            cy,
            radius: 4.0 * vpr as f32,
            fill: base,
            stroke_width: 0.0,
            stroke: base,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_geometry_tracks_lwc_spacing_buckets() {
        assert_eq!(marker_envelope_size(0.5), 10.0);
        assert_eq!(marker_envelope_size(6.0), 10.0);
        assert_eq!(marker_envelope_size(20.0), 18.0);
        assert_eq!(marker_envelope_size(50.0), 28.0);
        assert_eq!(marker_shape_size(10.0, 0.8), 9.0);
        assert_eq!(marker_shape_size(10.0, 0.7), 9.0);
        assert_eq!(marker_margin(6.0), 3.0);
    }

    #[test]
    fn marker_autoscale_margins_match_lwc_position_rules() {
        let marker = |position| crate::Marker {
            time: 0,
            position,
            shape: crate::marker_shape::CIRCLE,
            color: Color::rgb(0, 0, 0),
            text: String::new(),
        };
        assert_eq!(
            marker_auto_scale_margins(&[marker(crate::marker_pos::ABOVE)], 6.0),
            (21.0, 0.0)
        );
        assert_eq!(
            marker_auto_scale_margins(&[marker(crate::marker_pos::IN_BAR)], 6.0),
            (11.0, 11.0)
        );
        assert_eq!(
            marker_auto_scale_margins(
                &[
                    marker(crate::marker_pos::ABOVE),
                    marker(crate::marker_pos::IN_BAR),
                ],
                6.0,
            ),
            (21.0, 11.0)
        );
    }

    fn test_plot(count: usize) -> PlotList {
        let indices: Vec<i64> = (0..count as i64).collect();
        let close: Vec<f64> = indices
            .iter()
            .map(|i| {
                if i % 10 == 4 {
                    100.0 + (*i as f64) * 0.2 + 8.0
                } else if i % 10 == 7 {
                    100.0 + (*i as f64) * 0.2 - 8.0
                } else {
                    100.0 + (*i as f64) * 0.2
                }
            })
            .collect();
        let mut plot = PlotList::new();
        plot.set_data(indices, close.clone(), close.clone(), close.clone(), close);
        plot
    }

    #[test]
    fn conflation_preserves_endpoints_and_pixel_bucket_extrema() {
        let plot = test_plot(100);
        let rows = visible_line_rows(&plot, 0, 99, 0.1, 1.0, |index| index as f64 * 0.1);
        assert!(
            rows.len() < 60,
            "sub-pixel data should be reduced: {} rows",
            rows.len()
        );
        assert_eq!(rows.first().copied(), Some(0));
        assert_eq!(rows.last().copied(), Some(99));
        // Bucket 0..3.999 keeps the high at row 4 only after the bucket boundary; bucket 4..7.999
        // must retain its low at row 7 rather than smoothing away the visible envelope.
        assert!(rows.contains(&4));
        assert!(rows.contains(&7));
        assert!(rows.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn normal_spacing_keeps_every_visible_row() {
        let plot = test_plot(32);
        let rows = visible_line_rows(&plot, 4, 20, 2.0, 1.0, |index| index as f64 * 2.0);
        assert_eq!(rows, (4..=20).map(|i| i as usize).collect::<Vec<_>>());
    }

    #[test]
    fn ohlc_conflation_keeps_first_open_last_close_and_full_envelope() {
        let indices: Vec<i64> = (0..8).collect();
        let open = vec![10.0, 12.0, 11.0, 14.0, 20.0, 19.0, 18.0, 17.0];
        let high = vec![13.0, 15.0, 19.0, 16.0, 22.0, 25.0, 21.0, 20.0];
        let low = vec![9.0, 8.0, 10.0, 11.0, 18.0, 16.0, 15.0, 14.0];
        let close = vec![12.0, 11.0, 14.0, 13.0, 19.0, 18.0, 17.0, 16.0];
        let mut plot = PlotList::new();
        plot.set_data(indices, open, high, low, close);

        let bars = visible_ohlc(&plot, 0, 7, 0.25, 1.0, |index| index as f64 * 0.25);
        assert_eq!(
            bars,
            vec![
                VisibleOhlc {
                    x_px: 0.0,
                    open: 10.0,
                    high: 19.0,
                    low: 8.0,
                    close: 13.0
                },
                VisibleOhlc {
                    x_px: 1.0,
                    open: 20.0,
                    high: 25.0,
                    low: 14.0,
                    close: 16.0
                },
            ]
        );
    }

    #[test]
    fn ohlc_normal_spacing_is_an_identity_transform() {
        let plot = test_plot(8);
        let bars = visible_ohlc(&plot, 2, 5, 2.0, 1.5, |index| index as f64 * 3.0);
        assert_eq!(bars.len(), 4);
        assert_eq!(bars[0].x_px, 6.0);
        assert_eq!(bars[0].open, plot.value_at(2, PlotValueIndex::Open));
        assert_eq!(bars[3].close, plot.value_at(5, PlotValueIndex::Close));
    }

    #[test]
    fn histogram_conflation_preserves_largest_magnitude_and_source_row() {
        let indices: Vec<i64> = (0..8).collect();
        let values = vec![1.0, -8.0, 3.0, 4.0, 2.0, 5.0, -12.0, 7.0];
        let mut plot = PlotList::new();
        plot.set_data(
            indices,
            values.clone(),
            values.clone(),
            values.clone(),
            values,
        );

        let rows = visible_histogram_rows(&plot, 0, 7, 0.25, 1.0, |index| index as f64 * 0.25);
        assert_eq!(
            rows,
            vec![
                VisibleHistogramRow {
                    x_px: 0.0,
                    source_row: 1,
                    geometry_time: 0
                },
                VisibleHistogramRow {
                    x_px: 1.0,
                    source_row: 6,
                    geometry_time: 1
                },
            ]
        );
    }
}
