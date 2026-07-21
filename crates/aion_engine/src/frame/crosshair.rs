//! Crosshair geometry: pane hit, index snap, magnet price snap, crosshair prims.

use super::*;

impl ChartEngine {
    pub(super) fn build_crosshair_frame(
        &self,
        pane_index: usize,
        pane_w_px: i32,
        hpr: f64,
        vpr: f64,
        out: &mut Vec<Prim>,
    ) {
        let Some((x_css, y_css)) = self.clamped_crosshair() else {
            return;
        };
        let Some((from, to)) = self.visible_range_for_frame() else {
            return;
        };
        if self.crosshair_mode == CrosshairMode::Hidden {
            return;
        }
        let index = self.snapped_crosshair_index(x_css, from, to);
        let snapped_x = self.time_scale.index_to_coordinate(index);
        let ch = self.options.get().crosshair;
        let vert_color = css_color(&ch.vert_line.color, CROSSHAIR_COLOR);
        let horz_color = css_color(&ch.horz_line.color, CROSSHAIR_COLOR);
        // LWC lineWidth is in CSS px; generalize the crisp "1 CSS px" rule (grid uses the same
        // `max(1, floor(ratio))`) so width 1 keeps today's output. Vertical lines take the
        // horizontal ratio for thickness, horizontal lines the vertical ratio. Style is the LWC
        // lineStyle u8 (default LargeDashed), expanded to a dash pattern by the backends.
        let vert_width = 1f64.max((ch.vert_line.width * hpr).floor()) as i32;
        let horz_width = 1f64.max((ch.horz_line.width * vpr).floor()) as i32;
        let vert_style = crate::line_style_from_u8(ch.vert_line.style);
        let horz_style = crate::line_style_from_u8(ch.horz_line.style);
        let pane = &self.panes[pane_index];
        if ch.vert_line.visible {
            out.push(Prim::VLine {
                x: (snapped_x * hpr).round() as i32,
                y0: (pane.top * vpr).round() as i32,
                y1: ((pane.top + pane.height) * vpr).round() as i32,
                width: vert_width,
                style: vert_style,
                color: vert_color,
            });
        }
        if self.pane_at_y(y_css) != Some(pane_index) {
            return;
        }
        let snap_y = self.crosshair_snap(pane_index, x_css, y_css, from, to).1;
        if ch.horz_line.visible {
            out.push(Prim::HLine {
                y: (snap_y * vpr).round() as i32,
                x0: 0,
                x1: pane_w_px,
                width: horz_width,
                style: horz_style,
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

    /// LWC `PaneWidget._setCrosshairPosition` (pane-widget.ts:714-719) clamps the cursor into
    /// the pane instead of dropping the crosshair: x into `[0, width - 1]`, y into
    /// `[0, height - 1]` (height = the full stacked-pane region).
    pub(super) fn clamped_crosshair(&self) -> Option<(f64, f64)> {
        let (x, y) = self.crosshair?;
        Some((
            x.clamp(0.0, (self.pane_w - 1.0).max(0.0)),
            y.clamp(0.0, (self.pane_h - 1.0).max(0.0)),
        ))
    }

    pub(super) fn pane_at_y(&self, y: f64) -> Option<usize> {
        self.panes
            .iter()
            .position(|p| y >= p.top && y <= p.top + p.height)
    }

    pub(super) fn snapped_crosshair_index(&self, x_css: f64, from: i64, to: i64) -> i64 {
        let index = self.time_scale.coordinate_to_index(x_css).clamp(from, to);
        self.snap_index_to_visible_series(index)
    }

    /// LWC `Crosshair.snapToVisibleSeriesIfNeeded` (model/crosshair.ts:273-316): with
    /// `doNotSnapToHiddenSeriesIndices` set, move the snapped index to the nearest bar index
    /// held by any visible series (min |Δx|, ties to the left like LWC's `indexOf(min)`).
    /// Default off — the index is unchanged.
    fn snap_index_to_visible_series(&self, index: i64) -> i64 {
        if !self
            .options
            .get()
            .crosshair
            .do_not_snap_to_hidden_series_indices
        {
            return index;
        }
        let mut closest_left: Option<i64> = None;
        let mut closest_right: Option<i64> = None;
        for s in &self.series {
            if !s.visible {
                continue;
            }
            let plot = self.data.plot(s.id);
            if let Some(row) = plot.search(index, MismatchDirection::NearestLeft) {
                let candidate = plot.indices()[row];
                if candidate == index {
                    return index; // already snapped
                }
                closest_left = Some(closest_left.map_or(candidate, |l: i64| l.max(candidate)));
            }
            if let Some(row) = plot.search(index, MismatchDirection::NearestRight) {
                let candidate = plot.indices()[row];
                if candidate == index {
                    return index; // already snapped
                }
                closest_right = Some(closest_right.map_or(candidate, |r: i64| r.min(candidate)));
            }
        }
        let x = self.time_scale.index_to_coordinate(index);
        let mut best = index;
        let mut best_dist = f64::INFINITY;
        for candidate in [closest_left, closest_right].into_iter().flatten() {
            let dist = (x - self.time_scale.index_to_coordinate(candidate)).abs();
            if dist < best_dist {
                best_dist = dist;
                best = candidate;
            }
        }
        best
    }

    /// The pane's default price scale (LWC `Pane.defaultPriceScale`): the scale of the first
    /// visible, non-overlay series on the pane, else the pane's right scale. Returns the scale
    /// and its base (first) value for coordinate conversion.
    pub(super) fn pane_default_scale(
        &self,
        pane_index: usize,
        from: i64,
    ) -> (&PriceScaleCore, f64) {
        let series = self.series.iter().find(|s| {
            s.visible && !s.overlay && s.pane_index.min(self.panes.len() - 1) == pane_index
        });
        let target = series
            .map(series_scale_target)
            .unwrap_or(PriceScaleTarget::Right);
        let base_value = series
            .and_then(|s| self.series_base_value(s.id, from))
            .unwrap_or(0.0);
        (pane_scale(&self.panes[pane_index], target), base_value)
    }

    /// Port of LWC `Magnet.align` (model/magnet.ts:30-86): in Magnet modes the horizontal line
    /// snaps to the OHLC candidate — gathered from every visible, non-overlay series on the pane
    /// with a bar exactly at the snapped index — nearest the cursor in *pixel* space (each
    /// candidate converted on its own series' scale, so log modes compare correctly), then
    /// converted back to a price on the pane's default scale. Normal/Hidden mode, or no
    /// candidates, keeps the raw cursor price.
    pub(super) fn crosshair_snap(
        &self,
        pane_index: usize,
        x_css: f64,
        y_css: f64,
        from: i64,
        to: i64,
    ) -> (f64, f64) {
        let index = self.snapped_crosshair_index(x_css, from, to);
        let (default_scale, default_base) = self.pane_default_scale(pane_index, from);
        let price = default_scale.coordinate_to_price(y_css, default_base);
        if matches!(
            self.crosshair_mode,
            CrosshairMode::Normal | CrosshairMode::Hidden
        ) {
            return (price, y_css);
        }
        let keys: &[PlotValueIndex] = match self.crosshair_mode {
            // LWC magnetOHLCPlotRowKeys vs magnetPlotRowKeys (magnet.ts:13-21)
            CrosshairMode::MagnetOhlc => &[
                PlotValueIndex::Open,
                PlotValueIndex::High,
                PlotValueIndex::Low,
                PlotValueIndex::Close,
            ],
            _ => &[PlotValueIndex::Close],
        };
        let mut candidates = Vec::new();
        for s in &self.series {
            if !s.visible || s.overlay || s.pane_index.min(self.panes.len() - 1) != pane_index {
                continue;
            }
            let scale = pane_scale(&self.panes[pane_index], series_scale_target(s));
            if scale.is_empty() {
                continue;
            }
            let plot = self.data.plot(s.id);
            let Some(row) = plot.search(index, MismatchDirection::None) else {
                continue;
            };
            let Some(base_value) = self.series_base_value(s.id, from) else {
                continue;
            };
            candidates.extend(
                keys.iter()
                    .map(|&key| scale.price_to_coordinate(plot.value_at(row, key), base_value)),
            );
        }
        match magnet_snap_coordinate(y_css, &candidates) {
            Some(nearest) => (
                default_scale.coordinate_to_price(nearest, default_base),
                nearest,
            ),
            None => (price, y_css),
        }
    }
}
