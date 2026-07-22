//! Axis frame production: price/time labels, widths, marker/price-line/crosshair labels.

use super::*;

/// A last-value label candidate before axis overlap resolution (LWC IPriceAxisView state:
/// the original `coordinate` plus the render coordinate the overlap pass adjusts). `align`
/// is the owning scale's `alignLabels` — a scale with it off leaves its labels at their raw
/// coordinates (price-axis-widget.ts:633 early-return).
struct LastValueLabel {
    text: String,
    y: f64,
    height: f64,
    color: Color,
    align: bool,
}

/// Minimal port of LWC price-axis-widget.ts `_fixLabelOverlap` + `recalculateOverlapping`
/// for the last-value labels of one axis side: split the labels around the center label
/// (the first series', LWC's `centerSource`), clamp edge labels into the viewport, then push
/// overlapping labels apart outward from the center, shifting whole groups back when they
/// would fall off the scale. Only labels from `alignLabels` scales participate (LWC gates
/// the whole pass on that option); the rest keep their raw coordinates. Fewer than two
/// aligned labels are left untouched.
fn resolve_last_value_label_overlap(labels: &mut [LastValueLabel], scale_height: f64) {
    let aligned: Vec<usize> = (0..labels.len()).filter(|&i| labels[i].align).collect();
    if aligned.len() < 2 {
        return;
    }
    let center = labels[aligned[0]].y;
    // Split around the center and sort each side toward it (LWC sorts by the original
    // coordinate, so capture the order before any adjustment).
    let mut top: Vec<usize> = aligned
        .iter()
        .copied()
        .filter(|&i| labels[i].y <= center)
        .collect();
    top.sort_by(|&a, &b| labels[b].y.total_cmp(&labels[a].y)); // center-to-top
    let mut bottom: Vec<usize> = aligned
        .iter()
        .copied()
        .filter(|&i| labels[i].y > center)
        .collect();
    if !top.is_empty() && !bottom.is_empty() {
        bottom.push(top[0]); // share the center label between both passes
    }
    bottom.sort_by(|&a, &b| labels[a].y.total_cmp(&labels[b].y));
    // Edge clamp (price-axis-widget.ts:659-669): a label half-off the scale snaps fully inside.
    for &i in &aligned {
        let label = &mut labels[i];
        let half = (label.height / 2.0).floor();
        if label.y > -half && label.y < half {
            label.y = half;
        }
        if label.y > scale_height - half && label.y < scale_height + half {
            label.y = scale_height - half;
        }
    }
    recalculate_overlapping(labels, &top, 1.0, scale_height);
    recalculate_overlapping(labels, &bottom, -1.0, scale_height);
}

/// LWC `recalculateOverlapping` (price-axis-widget.ts:77-121): walk the labels outward from
/// the center (`direction` 1 = toward the top, -1 = toward the bottom) and push each
/// overlapping label past its predecessor; when a pushed group would leave the viewport,
/// shift the whole group back by the space that was free before it.
fn recalculate_overlapping(
    labels: &mut [LastValueLabel],
    order: &[usize],
    direction: f64,
    scale_height: f64,
) {
    if order.is_empty() {
        return;
    }
    let first = order[0];
    let init_height = labels[first].height;
    let mut space_before_group = (if direction > 0.0 {
        scale_height / 2.0 - (labels[first].y - init_height / 2.0)
    } else {
        labels[first].y - init_height / 2.0 - scale_height / 2.0
    })
    .max(0.0);
    let mut group_start = 0usize;
    for i in 1..order.len() {
        let view = order[i];
        let prev = order[i - 1];
        let height = labels[prev].height;
        let overlap = if direction > 0.0 {
            labels[view].y > labels[prev].y - height
        } else {
            labels[view].y < labels[prev].y + height
        };
        if overlap {
            let render_y = labels[prev].y - height * direction;
            labels[view].y = render_y;
            let edge_point = render_y - direction * height / 2.0;
            let out_of_viewport = if direction > 0.0 {
                edge_point < 0.0
            } else {
                edge_point > scale_height
            };
            if out_of_viewport && space_before_group > 0.0 {
                let desired_shift = if direction > 0.0 {
                    -1.0 - edge_point
                } else {
                    edge_point - scale_height
                };
                let shift = desired_shift.min(space_before_group);
                for &k in &order[group_start..] {
                    labels[k].y += direction * shift;
                }
                space_before_group -= shift;
            }
        } else {
            group_start = i;
            space_before_group = if direction > 0.0 {
                labels[prev].y - height - labels[view].y
            } else {
                labels[view].y - (labels[prev].y + height)
            };
        }
    }
}

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

    /// Format one value through a series' `priceFormat` (LWC series.ts `_recreateFormatter`):
    /// volume = K/M/B suffixes, percent = `%` sign, price = precision/min_move decimals,
    /// custom = the installed host fn. Returns `None` when the format defers to the
    /// scale/chart-level resolution (a factory-default price format, or a custom format with
    /// no/declining fn).
    pub(crate) fn format_with_price_format(
        &self,
        format: &SeriesPriceFormat,
        value: f64,
    ) -> Option<String> {
        match format.kind {
            // LWC custom: `formatTickmarks` defaults to mapping the formatter over the values,
            // so one per-value fn serves ticks and labels alike; a `None` return (the callback
            // threw at the boundary) defers to the built-in fallback.
            PriceFormatKind::Custom => format.formatter.as_ref().and_then(|f| f(value)),
            PriceFormatKind::Volume => Some(VolumeFormatter::new(format.precision).format(value)),
            // LWC wires `PercentageFormatter(precision)` — passing the precision as the raw
            // price scale, which reads like an upstream quirk (precision 2 -> one decimal).
            // We treat precision as decimal digits (10^precision), matching the percentage
            // scale mode's output at the default precision 2.
            PriceFormatKind::Percent => Some(
                PercentageFormatter::with_price_scale(10i64.pow(format.precision)).format(value),
            ),
            PriceFormatKind::Price => {
                if format.is_lwc_default() {
                    None
                } else {
                    Some(
                        PriceFormatter::from_precision(format.precision, format.min_move)
                            .format(value),
                    )
                }
            }
        }
    }

    /// A series' OWN format drives its last-value label, its price-line labels, and the
    /// crosshair price label when the series is the label source. LWC's
    /// `localization.priceFormatter` keeps precedence when installed (price-scale.ts
    /// `_formatValue` consults it before the scale/series formatter).
    pub(super) fn format_series_value(
        &self,
        series: &crate::SeriesEntry,
        scale: &PriceScaleCore,
        value: f64,
    ) -> String {
        if scale.mode() == PriceScaleMode::Percentage {
            return PercentageFormatter::default().format(value);
        }
        if let Some(f) = &self.price_formatter_fn {
            if let Some(s) = f(value) {
                return s;
            }
        }
        self.format_with_price_format(&series.price_format, value)
            .unwrap_or_else(|| self.price_formatter.format(value))
    }

    /// Axis TICK label formatting: the format of the scale's primary source — the first
    /// visible, non-overlay series bound to that scale (LWC uses the scale's main source for
    /// ticks: price-scale.ts `updateFormatter` picks the lowest-zorder data source). A primary
    /// source with the factory-default price format defers to the chart-level/built-in
    /// formatter, exactly like before per-series formats existed.
    pub(super) fn format_tick_value(
        &self,
        pane_index: usize,
        target: PriceScaleTarget,
        scale: &PriceScaleCore,
        value: f64,
    ) -> String {
        if scale.mode() == PriceScaleMode::Percentage {
            return PercentageFormatter::default().format(value);
        }
        let primary = self.series.iter().find(|s| {
            s.visible
                && !s.overlay
                && s.pane_index == pane_index
                && series_scale_target(s) == target
        });
        if let Some(series) = primary {
            if let Some(s) = self.format_with_price_format(&series.price_format, value) {
                return s;
            }
        }
        self.format_scale_value(scale, value)
    }

    /// Time-axis tick label, honoring a host `tickMarkFormatter` when installed. Month
    /// labels use the locale month-name table (LWC `localization.locale`).
    pub(super) fn format_time_tick(&self, ts: i64, kind: TickMarkType) -> String {
        if let Some(f) = &self.tick_mark_formatter_fn {
            if let Some(s) = f(ts, kind as u8) {
                return s;
            }
        }
        format_tick_label_with(ts, kind, &self.month_names)
    }

    /// Crosshair time label, honoring a host `timeFormatter` when installed. Otherwise the
    /// engine's `localization.dateFormat` pattern with the locale month-name table (LWC
    /// chart-options-defaults.ts:34-37).
    pub(super) fn format_crosshair_ts(&self, ts: i64) -> String {
        if let Some(f) = &self.time_formatter_fn {
            if let Some(s) = f(ts) {
                return s;
            }
        }
        format_crosshair_time_with(
            ts,
            self.time_visible,
            self.seconds_visible,
            &self.date_format,
            &self.month_names,
        )
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
        let mut out = AxisFrame {
            separator_hover: self.separator_hover,
            ..AxisFrame::default()
        };
        let visible = self.visible_range_for_frame();
        let layout_text_color = Color::parse_css(&self.options.get().layout.text_color)
            .unwrap_or(Color::rgb(0x19, 0x19, 0x19));
        // Per-scale label color (LWC `textColor`): the scale's own color when set, else the
        // layout text color (price-axis-widget.ts:569).
        let scale_text_color = |scale: &PriceScaleCore| {
            scale
                .options()
                .text_color
                .as_deref()
                .and_then(Color::parse_css)
                .unwrap_or(layout_text_color)
        };
        let options = self.options.get();
        let right_text_x = self.pane_left + self.pane_w + 5.0 + 5.0;
        let left_text_x = (self.pane_left - 5.0 - 5.0).max(0.0);
        for (pi, pane) in self.panes.iter().enumerate() {
            if options.right_price_scale.visible {
                // LWC `entireTextOnly`: corner marks shift in by half the font height so no
                // label text is clipped (price-tick-mark-builder.ts:71).
                let entire_margin = if pane.price_scale.options().entire_text_only {
                    pane.price_scale.options().font_size / 2.0
                } else {
                    0.0
                };
                let text_color = scale_text_color(&pane.price_scale);
                let ticks_visible = pane.price_scale.options().ticks_visible;
                for mark in pane.price_scale.build_tick_marks(100, entire_margin) {
                    let y = mark.coord;
                    if y >= pane.top - 0.5 && y <= pane.top + pane.height + 0.5 {
                        if ticks_visible {
                            out.price_ticks.push(PriceAxisTick { y, left: false });
                        }
                        out.labels.push(AxisLabel {
                            text: self.format_tick_value(
                                pi,
                                PriceScaleTarget::Right,
                                &pane.price_scale,
                                mark.logical,
                            ),
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
                let entire_margin = if pane.left_scale.options().entire_text_only {
                    pane.left_scale.options().font_size / 2.0
                } else {
                    0.0
                };
                let text_color = scale_text_color(&pane.left_scale);
                let ticks_visible = pane.left_scale.options().ticks_visible;
                for mark in pane.left_scale.build_tick_marks(100, entire_margin) {
                    let y = mark.coord;
                    if y >= pane.top - 0.5 && y <= pane.top + pane.height + 0.5 {
                        if ticks_visible {
                            out.price_ticks.push(PriceAxisTick { y, left: true });
                        }
                        out.labels.push(AxisLabel {
                            text: self.format_tick_value(
                                pi,
                                PriceScaleTarget::Left,
                                &pane.left_scale,
                                mark.logical,
                            ),
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
                // A hidden time axis (LWC `timeScale.visible` false) drops its whole strip,
                // tick labels and tick stubs included.
                if !self.time_axis_visible {
                    continue;
                }
                let x = self.pane_left + self.time_scale.index_to_coordinate(index);
                if self.time_ticks_visible {
                    out.time_ticks.push(x);
                }
                let kind =
                    weight_to_tick_mark_type(weight, self.time_visible, self.seconds_visible);
                out.labels.push(AxisLabel {
                    text: self.format_time_tick(ts, kind),
                    x,
                    y: self.pane_h + 1.0 + 5.0 + 3.0 + options.layout.font_size / 2.0,
                    color: layout_text_color,
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
        // LWC `minimumWidth` floors the negotiated strip width (chart-widget.ts
        // `_adjustSizeImpl`: `Math.max(optimalWidth(), minimumWidth)` across the pane's
        // scales on this side).
        let minimum_width = match target {
            PriceScaleTarget::Overlay => 0.0,
            PriceScaleTarget::Right | PriceScaleTarget::Left => self
                .panes
                .iter()
                .map(|pane| pane_scale(pane, target).options().minimum_width)
                .fold(0.0_f64, f64::max),
        };
        let width = (AXIS_BORDER_SIZE
            + AXIS_TICK_LENGTH
            + PRICE_PADDING_INNER
            + PRICE_PADDING_OUTER
            + PRICE_LABEL_OFFSET
            + text_width)
            .ceil()
            .max(minimum_width);
        width + (width as i64 % 2) as f64
    }

    pub(super) fn append_marker_labels(&self, labels: &mut Vec<AxisLabel>, from: i64, to: i64) {
        let times = self.data.merged_times();
        for (pi, pane) in self.panes.iter().enumerate() {
            for s in &self.series {
                if !s.visible || s.pane_index != pi {
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
                    if plot.is_whitespace_row(row) {
                        continue;
                    }
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
                if s.pane_index != pi {
                    continue;
                }
                let target = series_scale_target(s);
                let scale = pane_scale(pane, target);
                let Some(base_value) = self.visible_series_base_value(s.id) else {
                    continue;
                };
                for line in &s.price_lines {
                    // LWC `axisLabelVisible`: a hidden label leaves the line itself drawn.
                    if !line.axis_label_visible {
                        continue;
                    }
                    if scale.is_empty() {
                        continue;
                    }
                    let y = scale.price_to_coordinate(line.price, base_value);
                    if y < pane.top || y > pane.top + pane.height {
                        continue;
                    }
                    let text = if line.title.is_empty() {
                        // LWC custom-price-line-price-axis-view.ts: the label is the line's
                        // price in the OWNING series' format.
                        self.format_series_value(
                            s,
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
                    // LWC defaults: the label background follows the line color and the text is
                    // its contrast pick (as the crosshair labels do); both are overridable.
                    let background = line
                        .axis_label_color
                        .as_deref()
                        .and_then(Color::parse_css)
                        .unwrap_or(line.color);
                    let text_color = line
                        .axis_label_text_color
                        .as_deref()
                        .and_then(Color::parse_css)
                        .unwrap_or_else(|| background.contrast_text());
                    labels.push(AxisLabel {
                        text,
                        x,
                        y,
                        color: text_color,
                        align,
                        midpoint: AxisTextMidpoint::Label,
                        bold: false,
                        background: Some((
                            background_x,
                            y - height / 2.0,
                            width,
                            height,
                            background,
                        )),
                    });
                }
            }
        }
    }

    /// LWC SeriesPriceAxisView: every visible series with `lastValueVisible` (default true)
    /// gets a last-value label on its price scale — the background is the series' bar color,
    /// the text its contrast, the value the last visible bar's close in the scale's format.
    /// Labels sharing an axis side are pushed apart with LWC's overlap resolution
    /// (price-axis-widget.ts `_fixLabelOverlap`).
    pub(super) fn append_last_value_label<F>(&self, labels: &mut Vec<AxisLabel>, measure: &F)
    where
        F: Fn(&str) -> f64,
    {
        let Some((from, to)) = self.visible_range_for_frame() else {
            return;
        };
        let height = self.options.get().layout.font_size + 2.5 * 2.0;
        let mut right: Vec<LastValueLabel> = Vec::new();
        let mut left: Vec<LastValueLabel> = Vec::new();
        for (pi, pane) in self.panes.iter().enumerate() {
            for series in &self.series {
                if !series.visible || !series.last_value_visible {
                    continue;
                }
                if series.pane_index != pi {
                    continue;
                }
                let target = series_scale_target(series);
                let scale = pane_scale(pane, target);
                let plot = self.data.plot(series.id);
                if plot.is_empty() || scale.is_empty() {
                    continue;
                }
                // LWC series.ts lastValueData(false): the label tracks the last *visible*
                // bar; whitespace rows are skipped (LWC's plot list omits them).
                let Some(row) = plot.last_non_whitespace_row(to) else {
                    continue;
                };
                let close = plot.value_at(row, PlotValueIndex::Close);
                if !close.is_finite() {
                    continue;
                }
                let Some(base_value) = self.series_base_value(series.id, from) else {
                    continue;
                };
                let y = scale.price_to_coordinate(close, base_value);
                if y < 0.0 || y > self.pane_h {
                    continue;
                }
                let baseline = if series.kind == SeriesKind::Baseline {
                    self.resolved_baseline_price(series.id, from, to)
                } else {
                    None
                };
                let color = self.series_bar_color(series, row, baseline);
                // The series' OWN priceFormat drives its last-value label (LWC
                // series-price-axis-view.ts text, via the scale's series formatter).
                let text = self.format_series_value(
                    series,
                    scale,
                    scale.price_to_logical_value(close, base_value),
                );
                // LWC appends overlay (no-scale) series' labels to the pane's default axis
                // (price-axis-widget.ts:601-607); the engine's default axis is the right one.
                // The label's `alignLabels` comes from the axis it lands on.
                let (group, align) = if target == PriceScaleTarget::Left {
                    (&mut left, pane.left_scale.options().align_labels)
                } else {
                    (&mut right, pane.price_scale.options().align_labels)
                };
                group.push(LastValueLabel {
                    text,
                    y,
                    height,
                    color,
                    align,
                });
            }
        }
        // LWC aligns labels per price-axis widget; the engine has one strip per side. Scales
        // with `alignLabels` off keep raw coordinates; the rest resolve overlap as before.
        resolve_last_value_label_overlap(&mut right, self.pane_h);
        resolve_last_value_label_overlap(&mut left, self.pane_h);
        for (group, target) in [
            (right, PriceScaleTarget::Right),
            (left, PriceScaleTarget::Left),
        ] {
            for label in group {
                let width = 1.0 + 5.0 + 5.0 + 5.0 + measure(&label.text);
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
                    text: label.text,
                    x,
                    y: label.y,
                    color: label.color.contrast_text(),
                    align,
                    midpoint: AxisTextMidpoint::Label,
                    bold: false,
                    background: Some((
                        background_x,
                        label.y - label.height / 2.0,
                        width,
                        label.height,
                        label.color,
                    )),
                });
            }
        }
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
                    // The label source (the pane's first visible, non-overlay series) formats
                    // the crosshair price label with its own priceFormat.
                    let text = match series {
                        Some(series) => self.format_series_value(
                            series,
                            scale,
                            scale.price_to_logical_value(price, base_value),
                        ),
                        None => self.format_scale_value(
                            scale,
                            scale.price_to_logical_value(price, base_value),
                        ),
                    };
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
        if ch.vert_line.label_visible && x_css <= self.pane_w && self.time_axis_visible {
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
