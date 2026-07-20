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

    pub(super) fn pane_at_y(&self, y: f64) -> Option<usize> {
        self.panes
            .iter()
            .position(|p| y >= p.top && y <= p.top + p.height)
    }

    pub(super) fn snapped_crosshair_index(&self, x_css: f64, from: i64, to: i64) -> i64 {
        self.time_scale.coordinate_to_index(x_css).clamp(from, to)
    }

    pub(super) fn crosshair_snap(&self, x_css: f64, y_css: f64, from: i64, to: i64) -> (f64, f64) {
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
}
