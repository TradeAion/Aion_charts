//! Backend-neutral frame production for the headless chart model.
//!
//! This is intentionally independent of WebGPU, Canvas2D, and DOM types. Hosts may convert the
//! returned primitives into any raster backend, or inspect them in tests.

use aion_core::format::time_formatter::{format_crosshair_time, format_tick_label, weight_to_tick_mark_type};
use aion_core::model::magnet::{magnet_snap, CrosshairMode};
use aion_core::model::plot_list::{MismatchDirection, PlotValueIndex};
use aion_core::model::price_range::PriceRange;
use aion_render::bars::{build_bars, BarItem, BarsParams};
use aion_render::candles::{build_candles, CandleItem, CandlesParams};
use aion_render::color::Color;
use aion_render::draw_list::{Gradient, LineStyle, LineType, Prim};
use aion_render::histogram::{build_histogram, HistogramItem, HistogramParams};

use crate::{ChartEngine, SeriesKind, PANE_SEPARATOR};

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
    Center,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AxisLabel {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub color: Color,
    pub align: AxisTextAlign,
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
    overlay: bool,
    pane: usize,
}

fn css_color(value: &str, fallback: Color) -> Color {
    Color::parse_css(value).unwrap_or(fallback)
}

impl ChartEngine {
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
        let text_color = Color::parse_css(&self.options.get().layout.text_color).unwrap_or(Color::rgb(0x19, 0x19, 0x19));
        let label_bg = Color::rgb(0x13, 0x17, 0x22);
        let white = Color::rgb(0xff, 0xff, 0xff);
        let text_x = self.pane_w + 5.0 + 5.0;
        for pane in &self.panes {
            for mark in pane.price_scale.build_tick_marks(100, 0.0) {
                let y = mark.coord;
                if y >= pane.top - 0.5 && y <= pane.top + pane.height + 0.5 {
                    out.labels.push(AxisLabel {
                        text: self.price_formatter.format(mark.logical),
                        x: text_x,
                        y,
                        color: text_color,
                        align: AxisTextAlign::Left,
                        background: None,
                    });
                }
            }
        }
        if let Some((from, to)) = visible {
            let time_marks = self.time_marks(max_label_width);
            let times = self.data.merged_times();
            for &(index, weight) in &time_marks {
                if index < from || index > to { continue; }
                let ts = times[index as usize];
                let kind = weight_to_tick_mark_type(weight, self.time_visible, false);
                out.labels.push(AxisLabel {
                    text: format_tick_label(ts, kind),
                    x: self.time_scale.index_to_coordinate(index),
                    y: self.pane_h + 1.0 + 5.0 + 3.0 + 12.0 / 2.0,
                    color: text_color,
                    align: AxisTextAlign::Center,
                    background: None,
                });
            }
            self.append_marker_labels(&mut out.labels, from, to);
        }
        self.append_price_line_labels(&mut out.labels, &measure);
        self.append_last_value_label(&mut out.labels, &measure);
        self.append_crosshair_labels(&mut out.labels, &measure, label_bg, white);
        out.separators = self.panes.iter().skip(1).map(|p| p.top - PANE_SEPARATOR).collect();
        out
    }

    fn append_marker_labels(&self, labels: &mut Vec<AxisLabel>, from: i64, to: i64) {
        let times = self.data.merged_times();
        for (pi, pane) in self.panes.iter().enumerate() {
            for s in &self.series {
                if s.pane_index.min(self.panes.len() - 1) != pi { continue; }
                let scale = if s.overlay { &pane.overlay_scale } else { &pane.price_scale };
                if scale.is_empty() { continue; }
                let plot = self.data.plot(s.id);
                for marker in &s.markers {
                    if marker.text.is_empty() { continue; }
                    let Ok(pos) = times.binary_search(&marker.time) else { continue };
                    let index = pos as i64;
                    if index < from || index > to { continue; }
                    let Some(row) = plot.search(index, MismatchDirection::None) else { continue };
                    let high = plot.value_at(row, PlotValueIndex::High);
                    let low = plot.value_at(row, PlotValueIndex::Low);
                    let x = self.time_scale.index_to_coordinate(index);
                    let y = match marker.position {
                        crate::marker_pos::BELOW => scale.price_to_coordinate(low, low) + 18.0,
                        crate::marker_pos::ABOVE => scale.price_to_coordinate(high, high) - 18.0,
                        _ => scale.price_to_coordinate((high + low) / 2.0, high) - 14.0,
                    };
                    if y >= pane.top && y <= pane.top + pane.height && x >= 0.0 && x <= self.pane_w {
                        labels.push(AxisLabel { text: marker.text.clone(), x, y, color: marker.color, align: AxisTextAlign::Center, background: None });
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
                if s.pane_index.min(self.panes.len() - 1) != pi { continue; }
                let scale = if s.overlay { &pane.overlay_scale } else { &pane.price_scale };
                for line in &s.price_lines {
                    if scale.is_empty() { continue; }
                    let y = scale.price_to_coordinate(line.price, line.price);
                    if y < pane.top || y > pane.top + pane.height { continue; }
                    let text = if line.title.is_empty() { self.price_formatter.format(line.price) } else { line.title.clone() };
                    let width = 1.0 + 5.0 + 5.0 + 5.0 + measure(&text);
                    let height = 12.0 + 2.5 * 2.0;
                    labels.push(AxisLabel { text, x: self.pane_w + 5.0 + 5.0, y, color: line.color.contrast_text(), align: AxisTextAlign::Left, background: Some((self.pane_w, y - height / 2.0, width, height, line.color)) });
                }
            }
        }
    }

    fn append_last_value_label<F>(&self, labels: &mut Vec<AxisLabel>, measure: &F)
    where
        F: Fn(&str) -> f64,
    {
        let plot = self.data.plot(self.series[0].id);
        if plot.is_empty() || self.panes[0].price_scale.is_empty() { return; }
        let row = plot.size() - 1;
        let close = plot.value_at(row, PlotValueIndex::Close);
        let y = self.panes[0].price_scale.price_to_coordinate(close, close);
        if y < 0.0 || y > self.pane_h { return; }
        let color = if close >= plot.value_at(row, PlotValueIndex::Open) { Color::rgb(0x26, 0xa6, 0x9a) } else { Color::rgb(0xef, 0x53, 0x50) };
        let text = self.price_formatter.format(close);
        let width = 1.0 + 5.0 + 5.0 + 5.0 + measure(&text);
        let height = 12.0 + 2.5 * 2.0;
        labels.push(AxisLabel { text, x: self.pane_w + 5.0 + 5.0, y, color: color.contrast_text(), align: AxisTextAlign::Left, background: Some((self.pane_w, y - height / 2.0, width, height, color)) });
    }

    fn append_crosshair_labels<F>(&self, labels: &mut Vec<AxisLabel>, measure: &F, bg: Color, white: Color)
    where
        F: Fn(&str) -> f64,
    {
        let Some((x_css, y_css)) = self.crosshair else { return };
        let Some((from, to)) = self.visible_range_for_frame() else { return };
        if self.crosshair_mode == CrosshairMode::Hidden || self.data.plot(self.series[0].id).is_empty() { return; }
        if let Some(pi) = self.panes.iter().position(|p| y_css >= p.top && y_css <= p.top + p.height) {
            let scale = &self.panes[pi].price_scale;
            if !scale.is_empty() {
                let (price, snap_y) = if pi == 0 { self.crosshair_snap(x_css, y_css, from, to) } else { (scale.coordinate_to_price(y_css, 0.0), y_css) };
                let text = self.price_formatter.format(price);
                let width = 1.0 + 5.0 + 5.0 + 5.0 + measure(&text);
                let height = 12.0 + 2.5 * 2.0;
                labels.push(AxisLabel { text, x: self.pane_w + 5.0 + 5.0, y: snap_y, color: white, align: AxisTextAlign::Left, background: Some((self.pane_w, snap_y - height / 2.0, width, height, bg)) });
            }
        }
        if x_css <= self.pane_w {
            let index = self.snapped_crosshair_index(x_css, from, to);
            let text = format_crosshair_time(self.data.merged_times()[index as usize], self.time_visible, false);
            let width = measure(&text) + 9.0 * 2.0;
            let height = 12.0 + 3.0 + 3.0;
            let x = self.time_scale.index_to_coordinate(index);
            let box_x = (x - width / 2.0).clamp(0.0, (self.css_width - width).max(0.0));
            labels.push(AxisLabel { text, x: box_x + width / 2.0, y: self.pane_h + 1.0 + height / 2.0, color: white, align: AxisTextAlign::Center, background: Some((box_x, self.pane_h + 1.0, width, height, bg)) });
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

        let hpr = self.dpr.max(0.01);
        let vpr = hpr;
        let pane_count = self.panes.len().max(1);
        let pane_w_px = (self.pane_w * hpr).round().max(1.0) as u32;
        let mut resolved = Vec::with_capacity(self.series.len());
        for s in &self.series {
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
                overlay: s.overlay,
                pane: s.pane_index.min(pane_count - 1),
            });
        }

        output.width = self.pane_w;
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
            out.scissor = [0, top_px, pane_w_px, height_px];
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
                    &pane.price_scale,
                );
                for rs in &resolved {
                    if rs.pane != pi || !rs.visible {
                        continue;
                    }
                    let scale = if rs.overlay { &pane.overlay_scale } else { &pane.price_scale };
                    match rs.kind {
                        SeriesKind::Candlestick => self.build_candles_frame(*rs, from, to, hpr, vpr, &mut out.main, scale),
                        SeriesKind::Bar => self.build_bars_frame(*rs, from, to, hpr, vpr, &mut out.main, scale),
                        SeriesKind::Histogram => self.build_histogram_frame(*rs, from, to, hpr, vpr, &mut out.main, scale),
                        SeriesKind::Line | SeriesKind::Area => self.build_line_frame(*rs, from, to, hpr, vpr, pane.top + pane.height, &mut out.main, &mut out.points, scale),
                        SeriesKind::Baseline => self.build_baseline_frame(*rs, from, to, hpr, vpr, &mut out.main, &mut out.points, scale),
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
        }
    }

    fn layout_for_frame(&mut self) {
        // Hosts may negotiate an inner content width (for example after measuring the price axis).
        // Preserve that negotiated viewport; standalone/native callers start with pane_w/pane_h
        // equal to the CSS size.
        self.pane_w = if self.pane_w > 0.0 { self.pane_w } else { self.css_width.max(1.0) };
        self.pane_h = if self.pane_h > 0.0 { self.pane_h } else { self.css_height.max(1.0) };
        self.layout_panes(self.pane_h);
    }

    fn visible_range_for_frame(&self) -> Option<(i64, i64)> {
        let n = self.data.merged_times().len() as i64;
        let r = self.time_scale.visible_strict_range()?;
        if n == 0 { return None; }
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
        let mut overlay: Vec<Option<PriceRange>> = vec![None; n];
        for s in &self.series {
            let Some(mm) = self.data.plot_mut(s.id).min_max_on_range_cached(from, to, &[PlotValueIndex::Low, PlotValueIndex::High]) else { continue; };
            let slot = if s.overlay { &mut overlay[s.pane_index.min(n - 1)] } else { &mut main[s.pane_index.min(n - 1)] };
            let range = PriceRange::new(mm.min, mm.max);
            *slot = Some(match slot.take() { Some(old) => old.merge(Some(&range)), None => range });
        }
        for (i, pane) in self.panes.iter_mut().enumerate() {
            if let Some(range) = main[i].take() { pane.price_scale.apply_autoscale_range(Some(range), 0.01); }
            if let Some(range) = overlay[i].take() { pane.overlay_scale.apply_autoscale_range(Some(range.merge(Some(&PriceRange::new(0.0, 0.0)))), 0.01); }
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

    fn build_crosshair_frame(&self, pane_index: usize, pane_w_px: i32, hpr: f64, vpr: f64, out: &mut Vec<Prim>) {
        let Some((x_css, y_css)) = self.crosshair else { return };
        let Some((from, to)) = self.visible_range_for_frame() else { return };
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
            let Some(row) = plot.search(index, MismatchDirection::None) else { return };
            let close = plot.value_at(row, PlotValueIndex::Close);
            let cx = (snapped_x * hpr) as f32;
            let cy = (self.panes[0].price_scale.price_to_coordinate(close, close) * vpr) as f32;
            let fill = if self.series[0].kind == SeriesKind::Area { AREA_LINE } else { LINE };
            let outer = ((CROSSHAIR_MARKER_RADIUS + CROSSHAIR_MARKER_BORDER_WIDTH) * vpr) as f32;
            let inner = (CROSSHAIR_MARKER_RADIUS * vpr) as f32;
            out.push(Prim::Circle { cx, cy, radius: outer, fill: MARKER_BORDER_COLOR, stroke_width: 0.0, stroke: MARKER_BORDER_COLOR });
            out.push(Prim::Circle { cx, cy, radius: inner, fill, stroke_width: 0.0, stroke: fill });
        }
    }

    fn pane_at_y(&self, y: f64) -> Option<usize> {
        self.panes.iter().position(|p| y >= p.top && y <= p.top + p.height)
    }

    fn snapped_crosshair_index(&self, x_css: f64, from: i64, to: i64) -> i64 {
        self.time_scale.coordinate_to_index(x_css).clamp(from, to)
    }

    fn crosshair_snap(&self, x_css: f64, y_css: f64, from: i64, to: i64) -> (f64, f64) {
        let index = self.snapped_crosshair_index(x_css, from, to);
        let plot = self.data.plot(self.series[0].id);
        let row = plot.search(index, MismatchDirection::NearestLeft);
        let Some(row) = row else { return (self.panes[0].price_scale.coordinate_to_price(y_css, 0.0), y_css) };
        let close = plot.value_at(row, PlotValueIndex::Close);
        let price = match self.crosshair_mode {
            aion_core::model::magnet::CrosshairMode::Normal | aion_core::model::magnet::CrosshairMode::Hidden => return (self.panes[0].price_scale.coordinate_to_price(y_css, close), y_css),
            aion_core::model::magnet::CrosshairMode::Magnet => close,
            aion_core::model::magnet::CrosshairMode::MagnetOhlc => {
                let open = plot.value_at(row, PlotValueIndex::Open);
                let high = plot.value_at(row, PlotValueIndex::High);
                let low = plot.value_at(row, PlotValueIndex::Low);
                let candidates = [
                    (open, self.panes[0].price_scale.price_to_coordinate(open, open)),
                    (high, self.panes[0].price_scale.price_to_coordinate(high, high)),
                    (low, self.panes[0].price_scale.price_to_coordinate(low, low)),
                    (close, self.panes[0].price_scale.price_to_coordinate(close, close)),
                ];
                magnet_snap(y_css, &candidates).unwrap_or(close)
            }
        };
        (price, self.panes[0].price_scale.price_to_coordinate(price, price))
    }

    #[allow(clippy::too_many_arguments)]
    fn build_grid_frame(&self, out: &mut Vec<Prim>, marks: &[(i64, u8)], from: i64, to: i64, width: i32, top: i32, height: i32, hpr: f64, vpr: f64, scale: &aion_core::scale::price_scale_core::PriceScaleCore) {
        let grid = self.options.get().grid;
        let vert = css_color(&grid.vert_lines.color, GRID);
        let horz = css_color(&grid.horz_lines.color, GRID);
        let lw = 1f64.max(hpr.floor()) as i32;
        if grid.vert_lines.visible { for &(idx, _) in marks { if idx >= from && idx <= to { out.push(Prim::VLine { x: (self.time_scale.index_to_coordinate(idx) * hpr).round() as i32, y0: top - lw, y1: top + height + lw, width: lw, style: LineStyle::Solid, color: vert }); } } }
        if grid.horz_lines.visible { for mark in scale.build_tick_marks(100, 0.0) { out.push(Prim::HLine { y: (mark.coord * vpr).round() as i32, x0: -lw, x1: width + lw, width: lw, style: LineStyle::Solid, color: horz }); } }
    }

    fn build_candles_frame(&self, rs: ResolvedSeries, from: i64, to: i64, hpr: f64, vpr: f64, out: &mut Vec<Prim>, scale: &aion_core::scale::price_scale_core::PriceScaleCore) {
        let plot = self.data.plot(rs.id); let idxs = plot.indices(); let o = plot.column(PlotValueIndex::Open); let h = plot.column(PlotValueIndex::High); let l = plot.column(PlotValueIndex::Low); let c = plot.column(PlotValueIndex::Close);
        let items = plot.visible_rows(from, to).map(|r| CandleItem { x: self.time_scale.index_to_coordinate(idxs[r]), open_y: scale.price_to_coordinate(o[r], c[r]), high_y: scale.price_to_coordinate(h[r], c[r]), low_y: scale.price_to_coordinate(l[r], c[r]), close_y: scale.price_to_coordinate(c[r], c[r]), body_color: if c[r] >= o[r] { rs.up } else { rs.down }, border_color: if c[r] >= o[r] { rs.up } else { rs.down }, wick_color: if c[r] >= o[r] { rs.up } else { rs.down } }).collect::<Vec<_>>();
        build_candles(&items, &CandlesParams { bar_spacing: self.time_scale.bar_spacing(), horizontal_pixel_ratio: hpr, vertical_pixel_ratio: vpr, wick_visible: true, border_visible: true }, out);
    }

    fn build_bars_frame(&self, rs: ResolvedSeries, from: i64, to: i64, hpr: f64, vpr: f64, out: &mut Vec<Prim>, scale: &aion_core::scale::price_scale_core::PriceScaleCore) {
        let plot = self.data.plot(rs.id); let idxs = plot.indices(); let o = plot.column(PlotValueIndex::Open); let h = plot.column(PlotValueIndex::High); let l = plot.column(PlotValueIndex::Low); let c = plot.column(PlotValueIndex::Close);
        let items = plot.visible_rows(from, to).map(|r| BarItem { x: self.time_scale.index_to_coordinate(idxs[r]), open_y: scale.price_to_coordinate(o[r], c[r]), high_y: scale.price_to_coordinate(h[r], c[r]), low_y: scale.price_to_coordinate(l[r], c[r]), close_y: scale.price_to_coordinate(c[r], c[r]), color: if c[r] >= o[r] { rs.up } else { rs.down } }).collect::<Vec<_>>();
        build_bars(&items, &BarsParams { bar_spacing: self.time_scale.bar_spacing(), horizontal_pixel_ratio: hpr, vertical_pixel_ratio: vpr, open_visible: true, thin_bars: true }, out);
    }

    fn build_histogram_frame(&self, rs: ResolvedSeries, from: i64, to: i64, hpr: f64, vpr: f64, out: &mut Vec<Prim>, scale: &aion_core::scale::price_scale_core::PriceScaleCore) {
        let plot = self.data.plot(rs.id); let idxs = plot.indices(); let c = plot.column(PlotValueIndex::Close); let base = scale.price_to_coordinate(0.0, 0.0); let solid = if rs.color != LINE { rs.color } else { HISTOGRAM }; let main = self.data.plot(self.series[0].id);
        let items = plot.visible_rows(from, to).map(|r| { let color = if self.series[rs.id].histogram_updown { match main.search(idxs[r], MismatchDirection::None) { Some(row) if main.value_at(row, PlotValueIndex::Close) >= main.value_at(row, PlotValueIndex::Open) => VOLUME_UP, Some(_) => VOLUME_DOWN, None => solid } } else { solid }; HistogramItem { x: self.time_scale.index_to_coordinate(idxs[r]), y: scale.price_to_coordinate(c[r], c[r]), time: idxs[r], color } }).collect::<Vec<_>>();
        build_histogram(&items, &HistogramParams { bar_spacing: self.time_scale.bar_spacing(), horizontal_pixel_ratio: hpr, vertical_pixel_ratio: vpr, histogram_base: base }, out);
    }

    fn build_line_frame(&self, rs: ResolvedSeries, from: i64, to: i64, hpr: f64, vpr: f64, band_bottom: f64, out: &mut Vec<Prim>, points: &mut Vec<[f32; 2]>, scale: &aion_core::scale::price_scale_core::PriceScaleCore) {
        let plot = self.data.plot(rs.id); let idxs = plot.indices(); let c = plot.column(PlotValueIndex::Close); let first = points.len() as u32;
        for r in plot.visible_rows(from, to) { points.push([(self.time_scale.index_to_coordinate(idxs[r]) * hpr) as f32, (scale.price_to_coordinate(c[r], c[r]) * vpr) as f32]); }
        let count = points.len() as u32 - first; if count == 0 { return; }
        let color = if rs.color != LINE { rs.color } else if rs.kind == SeriesKind::Area { AREA_LINE } else { LINE };
        if rs.kind == SeriesKind::Area { out.push(Prim::AreaFill { first_point: first, point_count: count, base_y: (band_bottom * vpr) as f32, line_type: self.series[rs.id].line_type, gradient: Gradient { top: rs.area_top, bottom: rs.area_bottom } }); }
        out.push(Prim::Polyline { first_point: first, point_count: count, width: (rs.line_width * vpr) as f32, style: LineStyle::Solid, line_type: rs.line_type, color });
        if rs.point_markers {
            let radius = (rs.line_width + 1.0).max(3.0);
            if self.time_scale.bar_spacing() >= 2.0 * radius + 2.0 {
                for i in first..first + count {
                    let [cx, cy] = points[i as usize];
                    out.push(Prim::Circle { cx, cy, radius: (radius * vpr) as f32, fill: color, stroke_width: 0.0, stroke: color });
                }
            }
        }
    }

    fn build_baseline_frame(&self, rs: ResolvedSeries, from: i64, to: i64, hpr: f64, vpr: f64, out: &mut Vec<Prim>, points: &mut Vec<[f32; 2]>, scale: &aion_core::scale::price_scale_core::PriceScaleCore) {
        let plot = self.data.plot(rs.id);
        let idxs = plot.indices();
        let close = plot.column(PlotValueIndex::Close);
        let rows: Vec<usize> = plot.visible_rows(from, to).collect();
        if rows.len() < 2 { return; }
        let baseline_price = rs.baseline.unwrap_or_else(|| {
            let mut min = f64::INFINITY;
            let mut max = f64::NEG_INFINITY;
            for &row in &rows { min = min.min(close[row]); max = max.max(close[row]); }
            (min + max) / 2.0
        });
        let baseline_y = scale.price_to_coordinate(baseline_price, baseline_price);
        for pair in rows.windows(2) {
            let a_row = pair[0];
            let b_row = pair[1];
            let a = (self.time_scale.index_to_coordinate(idxs[a_row]), scale.price_to_coordinate(close[a_row], close[a_row]));
            let b = (self.time_scale.index_to_coordinate(idxs[b_row]), scale.price_to_coordinate(close[b_row], close[b_row]));
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
                let line = if above { BASELINE_TOP_LINE } else { BASELINE_BOTTOM_LINE };
                let fill = if above { BASELINE_TOP_FILL } else { BASELINE_BOTTOM_FILL };
                out.push(Prim::AreaFill { first_point: first, point_count: 2, base_y: (baseline_y * vpr) as f32, line_type: LineType::Simple, gradient: Gradient { top: fill, bottom: fill } });
                out.push(Prim::Polyline { first_point: first, point_count: 2, width: (LINE_WIDTH * vpr) as f32, style: LineStyle::Solid, line_type: LineType::Simple, color: line });
            }
        }
    }

    fn build_price_lines_frame(&self, pane_index: usize, out: &mut Vec<Prim>, width: i32, vpr: f64) {
        let pane = &self.panes[pane_index];
        let min_width = 1f64.max(vpr.floor()) as i32;
        for series in &self.series {
            if series.pane_index.min(self.panes.len() - 1) != pane_index { continue; }
            let scale = if series.overlay { &pane.overlay_scale } else { &pane.price_scale };
            if scale.is_empty() { continue; }
            for line in &series.price_lines {
                out.push(Prim::HLine {
                    y: (scale.price_to_coordinate(line.price, line.price) * vpr).round() as i32,
                    x0: 0,
                    x1: width,
                    width: line.width.max(min_width),
                    style: line.style,
                    color: line.color,
                });
            }
        }
    }

    fn build_markers_frame(&self, pane_index: usize, from: i64, to: i64, hpr: f64, vpr: f64, out: &mut Vec<Prim>) {
        const SIZE: f64 = 6.0;
        const GAP: f64 = 4.0;
        let pane = &self.panes[pane_index];
        let times = self.data.merged_times();
        for series in &self.series {
            if series.pane_index.min(self.panes.len() - 1) != pane_index { continue; }
            let scale = if series.overlay { &pane.overlay_scale } else { &pane.price_scale };
            if scale.is_empty() { continue; }
            let plot = self.data.plot(series.id);
            for marker in &series.markers {
                let Ok(pos) = times.binary_search(&marker.time) else { continue; };
                let index = pos as i64;
                if index < from || index > to { continue; }
                let Some(row) = plot.search(index, MismatchDirection::None) else { continue; };
                let high = plot.value_at(row, PlotValueIndex::High);
                let low = plot.value_at(row, PlotValueIndex::Low);
                let x = (self.time_scale.index_to_coordinate(index) * hpr) as f32;
                let size = (SIZE * vpr) as f32;
                let gap = (GAP * vpr) as f32;
                let y = match marker.position {
                    crate::marker_pos::ABOVE => (scale.price_to_coordinate(high, high) * vpr) as f32 - size - gap,
                    crate::marker_pos::BELOW => (scale.price_to_coordinate(low, low) * vpr) as f32 + size + gap,
                    _ => (scale.price_to_coordinate((high + low) / 2.0, high) * vpr) as f32,
                };
                match marker.shape {
                    crate::marker_shape::SQUARE => out.push(Prim::RoundRect { x: x - size, y: y - size, w: size * 2.0, h: size * 2.0, radii: [2.0, 2.0, 2.0, 2.0], fill: marker.color, border_width: 0.0, border_color: marker.color }),
                    crate::marker_shape::ARROW_UP => out.push(Prim::Triangle { a: [x, y - size], b: [x - size, y + size], c: [x + size, y + size], color: marker.color }),
                    crate::marker_shape::ARROW_DOWN => out.push(Prim::Triangle { a: [x, y + size], b: [x - size, y - size], c: [x + size, y - size], color: marker.color }),
                    _ => out.push(Prim::Circle { cx: x, cy: y, radius: size, fill: marker.color, stroke_width: 0.0, stroke: marker.color }),
                }
            }
        }
    }

    fn build_last_value_line_frame(&self, out: &mut Vec<Prim>, width: i32, vpr: f64) {
        let plot = self.data.plot(self.series[0].id);
        if plot.is_empty() || self.panes[0].price_scale.is_empty() { return; }
        let last = plot.size() - 1;
        let close = plot.value_at(last, PlotValueIndex::Close);
        let color = match self.series[0].kind {
            SeriesKind::Line => LINE,
            SeriesKind::Area => AREA_LINE,
            SeriesKind::Histogram => HISTOGRAM,
            _ => {
                let open = plot.value_at(last, PlotValueIndex::Open);
                if close >= open { UP } else { DOWN }
            }
        };
        out.push(Prim::HLine {
            y: (self.panes[0].price_scale.price_to_coordinate(close, close) * vpr).round() as i32,
            x0: 0,
            x1: width,
            width: 1f64.max(vpr.floor()) as i32,
            style: LineStyle::Dashed,
            color,
        });
    }

    fn build_last_pulse_frame(&self, out: &mut Vec<Prim>, hpr: f64, vpr: f64) {
        if !self.series[0].last_price_animation { return; }
        let plot = self.data.plot(self.series[0].id);
        if plot.is_empty() || self.panes[0].price_scale.is_empty() { return; }
        let last = plot.size() - 1;
        let Some(&index) = plot.indices().last() else { return; };
        let close = plot.value_at(last, PlotValueIndex::Close);
        let cx = (self.time_scale.index_to_coordinate(index) * hpr) as f32;
        let cy = (self.panes[0].price_scale.price_to_coordinate(close, close) * vpr) as f32;
        let base = match self.series[0].kind {
            SeriesKind::Line => LINE,
            SeriesKind::Area => AREA_LINE,
            SeriesKind::Histogram => HISTOGRAM,
            _ => {
                let open = plot.value_at(last, PlotValueIndex::Open);
                if close >= open { UP } else { DOWN }
            }
        };
        const PERIOD_MS: f64 = 2600.0;
        let phase = (self.animation_time.rem_euclid(PERIOD_MS) / PERIOD_MS) as f32;
        let ring = Color::rgba(base.r(), base.g(), base.b(), ((1.0 - phase) * 0.35 * 255.0) as u8);
        out.push(Prim::Circle { cx, cy, radius: (4.0 + phase * 10.0) * vpr as f32, fill: ring, stroke_width: 0.0, stroke: ring });
        out.push(Prim::Circle { cx, cy, radius: 4.0 * vpr as f32, fill: base, stroke_width: 0.0, stroke: base });
    }
}
