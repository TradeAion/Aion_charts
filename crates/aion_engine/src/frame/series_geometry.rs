//! Per-kind series geometry builders (grid, candles, bars, histogram, line, baseline,
//! price lines, markers, last-value line and pulse) emitting backend-neutral prims.

use super::*;

impl ChartEngine {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn build_grid_frame(
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

    #[allow(clippy::too_many_arguments)] // mirrors the LWC renderer-data signature
    pub(super) fn build_candles_frame(
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
                let rising = bar.close >= bar.open;
                CandleItem {
                    x: bar.x_px / hpr,
                    open_y: scale.price_to_coordinate(bar.open, rs.base_value),
                    high_y: scale.price_to_coordinate(bar.high, rs.base_value),
                    low_y: scale.price_to_coordinate(bar.low, rs.base_value),
                    close_y: scale.price_to_coordinate(bar.close, rs.base_value),
                    body_color: if rising { rs.up } else { rs.down },
                    border_color: if rising { rs.border_up } else { rs.border_down },
                    wick_color: if rising { rs.wick_up } else { rs.wick_down },
                }
            })
            .collect::<Vec<_>>();
        build_candles(
            &items,
            &CandlesParams {
                bar_spacing: self.time_scale.bar_spacing(),
                horizontal_pixel_ratio: hpr,
                vertical_pixel_ratio: vpr,
                wick_visible: rs.wick_visible,
                border_visible: rs.border_visible,
            },
            out,
        );
    }

    #[allow(clippy::too_many_arguments)] // mirrors the LWC renderer-data signature
    pub(super) fn build_bars_frame(
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

    #[allow(clippy::too_many_arguments)] // mirrors the LWC renderer-data signature
    pub(super) fn build_histogram_frame(
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

    #[allow(clippy::too_many_arguments)] // mirrors the LWC renderer-data signature
    pub(super) fn build_line_frame(
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

    #[allow(clippy::too_many_arguments)] // mirrors the LWC renderer-data signature
    pub(super) fn build_baseline_frame(
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

    pub(super) fn build_price_lines_frame(
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

    pub(super) fn build_markers_frame(
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

    pub(super) fn build_last_value_line_frame(&self, out: &mut Vec<Prim>, width: i32, vpr: f64) {
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

    pub(super) fn build_last_pulse_frame(&self, out: &mut Vec<Prim>, hpr: f64, vpr: f64) {
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
