//! Axis frame production: price/time labels, widths, marker/price-line/crosshair labels.

use super::*;

impl ChartEngine {
    pub(super) fn format_scale_value(&self, scale: &PriceScaleCore, value: f64) -> String {
        if scale.mode() == PriceScaleMode::Percentage {
            // Percentage mode has its own formatter; the host price formatter does not apply here
            // (matching LWC, where percentage display is independent of `priceFormatter`).
            return PercentageFormatter::default().format(value);
        }
        if let Some(f) = &self.price_formatter_fn {
            if let Some(s) = f(value) {
                return s;
            }
        }
        self.price_formatter.format(value)
    }

    /// Time-axis tick label, honoring a host `tickMarkFormatter` when installed.
    pub(super) fn format_time_tick(&self, ts: i64, kind: TickMarkType) -> String {
        if let Some(f) = &self.tick_mark_formatter_fn {
            if let Some(s) = f(ts, kind as u8) {
                return s;
            }
        }
        format_tick_label(ts, kind)
    }

    /// Crosshair time label, honoring a host `timeFormatter` when installed.
    pub(super) fn format_crosshair_ts(&self, ts: i64) -> String {
        if let Some(f) = &self.time_formatter_fn {
            if let Some(s) = f(ts) {
                return s;
            }
        }
        format_crosshair_time(ts, self.time_visible, self.seconds_visible)
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
                let kind =
                    weight_to_tick_mark_type(weight, self.time_visible, self.seconds_visible);
                out.labels.push(AxisLabel {
                    text: self.format_time_tick(ts, kind),
                    x: self.pane_left + self.time_scale.index_to_coordinate(index),
                    y: self.pane_h + 1.0 + 5.0 + 3.0 + options.layout.font_size / 2.0,
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
        self.append_crosshair_labels(&mut out.labels, &measure);
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

    pub(super) fn append_marker_labels(&self, labels: &mut Vec<AxisLabel>, from: i64, to: i64) {
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

    pub(super) fn append_price_line_labels<F>(&self, labels: &mut Vec<AxisLabel>, measure: &F)
    where
        F: Fn(&str) -> f64,
    {
        let font_size = self.options.get().layout.font_size;
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
                    let height = font_size + 2.5 * 2.0;
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

    pub(super) fn append_last_value_label<F>(&self, labels: &mut Vec<AxisLabel>, measure: &F)
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
        let height = self.options.get().layout.font_size + 2.5 * 2.0;
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

    pub(super) fn append_crosshair_labels<F>(&self, labels: &mut Vec<AxisLabel>, measure: &F)
    where
        F: Fn(&str) -> f64,
    {
        let Some((x_css, y_css)) = self.clamped_crosshair() else {
            return;
        };
        let Some((from, to)) = self.visible_range_for_frame() else {
            return;
        };
        if self.crosshair_mode == CrosshairMode::Hidden {
            return;
        }
        // Price-axis label tracks the horizontal line (LWC `horzLine`); time-axis label tracks the
        // vertical line (LWC `vertLine`). Each carries its own `labelVisible`/`labelBackgroundColor`,
        // and the text color is the LWC contrast pick against that background.
        let options = self.options.get();
        let font_size = options.layout.font_size;
        let ch = options.crosshair;
        if ch.horz_line.label_visible {
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
                    let (price, snap_y) = self.crosshair_snap(pi, x_css, y_css, from, to);
                    let text = self
                        .format_scale_value(scale, scale.price_to_logical_value(price, base_value));
                    let width = 1.0 + 5.0 + 5.0 + 5.0 + measure(&text);
                    let height = font_size + 2.5 * 2.0;
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
                    let label_bg =
                        css_color(&ch.horz_line.label_background_color, CROSSHAIR_LABEL_BG);
                    labels.push(AxisLabel {
                        text,
                        x: label_x,
                        y: snap_y,
                        color: label_bg.contrast_text(),
                        align,
                        midpoint: AxisTextMidpoint::Label,
                        bold: false,
                        background: Some((
                            background_x,
                            snap_y - height / 2.0,
                            width,
                            height,
                            label_bg,
                        )),
                    });
                }
            }
        }
        if ch.vert_line.label_visible && x_css <= self.pane_w {
            let index = self.snapped_crosshair_index(x_css, from, to);
            let text = self.format_crosshair_ts(self.data.merged_times()[index as usize]);
            let width = measure(&text) + 9.0 * 2.0;
            let height = font_size + 3.0 + 3.0;
            let x = self.pane_left + self.time_scale.index_to_coordinate(index);
            let box_x = (x - width / 2.0).clamp(
                self.pane_left,
                (self.pane_left + self.pane_w - width).max(self.pane_left),
            );
            let label_bg = css_color(&ch.vert_line.label_background_color, CROSSHAIR_LABEL_BG);
            labels.push(AxisLabel {
                text,
                x: box_x + width / 2.0,
                y: self.pane_h + 1.0 + height / 2.0,
                color: label_bg.contrast_text(),
                align: AxisTextAlign::Center,
                midpoint: AxisTextMidpoint::StableTime,
                bold: false,
                background: Some((box_x, self.pane_h + 1.0, width, height, label_bg)),
            });
        }
    }
}
