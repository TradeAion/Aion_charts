#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use crate::core::renderer::traits::ChartStyle;

#[derive(Debug, Clone, Copy)]
pub(crate) struct RightAxisLabelMetrics {
    pub(crate) fs: f64,
    pub(crate) inset_inner: f64,
    pub(crate) inset_outer: f64,
    pub(crate) inset_tb: f64,
    pub(crate) tick_size: f64,
    pub(crate) border_size: f64,
    pub(crate) edge_inset: f64,
    pub(crate) full_label_inside_gap: f64,
}

impl RightAxisLabelMetrics {
    pub(crate) fn from_style(style: &ChartStyle, dpr: f64) -> Self {
        let border_size = if style.axis_border_visible {
            (style.axis_border_size as f64 * dpr).max(1.0).floor()
        } else {
            0.0
        };
        Self {
            fs: style.font_size as f64 * dpr,
            inset_inner: style.price_axis_inset_inner() * dpr,
            inset_outer: style.price_axis_inset_outer() * dpr,
            inset_tb: style.price_axis_inset_tb() * dpr,
            // Axis tick marks are hidden in compact mode; don't reserve connector width.
            tick_size: 0.0,
            border_size,
            edge_inset: (style.price_axis_label_edge_inset() * dpr).round(),
            full_label_inside_gap: (style.price_axis_full_label_inside_gap() * dpr).round(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RightAxisLabelGeometry {
    pub(crate) y_mid: f64,
    pub(crate) y_top: f64,
    pub(crate) y_bottom: f64,
    pub(crate) x_inside: f64,
    pub(crate) x_outside: f64,
    pub(crate) text_x_css: f64,
    pub(crate) text_y_css: f64,
    pub(crate) text_align_right: bool,
    pub(crate) radius: f64,
    pub(crate) tick_size: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RightAxisLabelWidthMode {
    TextFit,
    AxisFull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RightAxisLabelVerticalMode {
    ClampToPane,
    FollowValue,
}

pub(crate) fn right_axis_label_height_bmp(
    metrics: &RightAxisLabelMetrics,
    dpr: f64,
    extra_tb_inset: f64,
) -> f64 {
    let total_h = metrics.fs + (metrics.inset_tb + extra_tb_inset) * 2.0;
    let tick_h_bmp = dpr.floor().max(1.0) as i32;
    let mut total_h_bmp = total_h.round() as i32;
    if total_h_bmp % 2 != tick_h_bmp % 2 {
        total_h_bmp += 1;
    }
    total_h_bmp.max(1) as f64
}

pub(crate) fn compute_right_axis_label_geometry(
    axis_w: f64,
    pane_h: f64,
    y_coord_phys: f64,
    text_w_phys: f64,
    dpr: f64,
    metrics: &RightAxisLabelMetrics,
    extra_tb_inset: f64,
    width_mode: RightAxisLabelWidthMode,
) -> Option<RightAxisLabelGeometry> {
    compute_right_axis_label_geometry_with_vertical_mode(
        axis_w,
        pane_h,
        y_coord_phys,
        text_w_phys,
        dpr,
        metrics,
        extra_tb_inset,
        width_mode,
        RightAxisLabelVerticalMode::ClampToPane,
    )
}

pub(crate) fn compute_right_axis_label_geometry_with_vertical_mode(
    axis_w: f64,
    pane_h: f64,
    y_coord_phys: f64,
    text_w_phys: f64,
    dpr: f64,
    metrics: &RightAxisLabelMetrics,
    extra_tb_inset: f64,
    width_mode: RightAxisLabelWidthMode,
    vertical_mode: RightAxisLabelVerticalMode,
) -> Option<RightAxisLabelGeometry> {
    if axis_w <= 0.0 || pane_h <= 0.0 || dpr <= 0.0 {
        return None;
    }

    let total_h_bmp = right_axis_label_height_bmp(metrics, dpr, extra_tb_inset);
    let total_w_raw = metrics.border_size
        + metrics.inset_inner
        + metrics.inset_outer
        + text_w_phys
        + metrics.tick_size;
    // Right price scale in reference implementation uses align='left':
    // separator/border is at x=0, label extends from inside edge to the right.
    let inside_gap = if matches!(width_mode, RightAxisLabelWidthMode::AxisFull) {
        metrics.full_label_inside_gap.max(0.0)
    } else {
        0.0
    };
    let x_inside = (metrics.border_size + inside_gap).min(axis_w).max(0.0);
    let available_w = (axis_w - x_inside).max(1.0);
    let total_w_bmp = match width_mode {
        RightAxisLabelWidthMode::AxisFull => available_w.round().max(1.0),
        RightAxisLabelWidthMode::TextFit => total_w_raw.min(available_w).round().max(1.0),
    };

    let y_mid_raw = y_coord_phys.round() - (dpr * 0.5).floor();
    let half = total_h_bmp / 2.0;
    let edge_inset = metrics.edge_inset.max(0.0);
    let min_mid = half + edge_inset;
    let max_mid = pane_h - half - edge_inset;
    let y_mid = match vertical_mode {
        RightAxisLabelVerticalMode::ClampToPane => {
            if max_mid >= min_mid {
                y_mid_raw.clamp(min_mid, max_mid)
            } else {
                (pane_h * 0.5).round()
            }
        }
        RightAxisLabelVerticalMode::FollowValue => y_mid_raw,
    };
    let tick_h_bmp = dpr.floor().max(1.0);
    let y_top = (y_mid + tick_h_bmp / 2.0 - total_h_bmp / 2.0).floor();
    let y_bottom = y_top + total_h_bmp;

    let x_outside = match width_mode {
        RightAxisLabelWidthMode::AxisFull => axis_w,
        RightAxisLabelWidthMode::TextFit => (x_inside + total_w_bmp).min(axis_w),
    };
    let (text_x_css, text_align_right) = match width_mode {
        RightAxisLabelWidthMode::AxisFull => {
            let center_x_phys = (x_inside + axis_w) / 2.0;
            let min_left = x_inside + metrics.tick_size + metrics.inset_inner;
            let max_left = (axis_w - metrics.inset_outer - text_w_phys).max(min_left);
            let text_left_phys = (center_x_phys - text_w_phys / 2.0).clamp(min_left, max_left);
            (text_left_phys / dpr, false)
        }
        RightAxisLabelWidthMode::TextFit => (
            (x_inside + metrics.tick_size + metrics.inset_inner) / dpr,
            false,
        ),
    };
    let radius = (2.0 * dpr).round().min(total_h_bmp / 4.0).max(0.0);

    Some(RightAxisLabelGeometry {
        y_mid,
        y_top,
        y_bottom,
        x_inside,
        x_outside,
        text_x_css,
        text_y_css: (y_top + y_bottom) / 2.0 / dpr,
        text_align_right,
        radius,
        tick_size: metrics.tick_size,
    })
}

#[inline]
pub(crate) fn centered_full_width_label_text_x_css(
    geom: &RightAxisLabelGeometry,
    text_w_phys: f64,
    dpr: f64,
    metrics: &RightAxisLabelMetrics,
) -> f64 {
    let center_x_phys = (geom.x_inside + geom.x_outside) / 2.0;
    let min_left = geom.x_inside + geom.tick_size + metrics.inset_inner;
    let max_left = (geom.x_outside - metrics.inset_outer - text_w_phys).max(min_left);
    let text_left_phys = (center_x_phys - text_w_phys / 2.0).clamp(min_left, max_left);
    text_left_phys / dpr
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::renderer::theme::default_style;

    #[test]
    fn hidden_axis_border_does_not_reserve_label_separator_gap() {
        let mut style = default_style();
        style.axis_border_visible = false;
        let metrics = RightAxisLabelMetrics::from_style(&style, 2.0);
        let geom = compute_right_axis_label_geometry(
            80.0,
            120.0,
            60.0,
            30.0,
            2.0,
            &metrics,
            0.0,
            RightAxisLabelWidthMode::AxisFull,
        )
        .expect("geometry");

        assert_eq!(geom.x_inside, 0.0);
    }

    #[test]
    fn follow_value_geometry_does_not_stick_to_pane_edge() {
        let style = default_style();
        let dpr = 1.0;
        let metrics = RightAxisLabelMetrics::from_style(&style, dpr);
        let clamped = compute_right_axis_label_geometry(
            60.0,
            100.0,
            130.0,
            32.0,
            dpr,
            &metrics,
            0.0,
            RightAxisLabelWidthMode::AxisFull,
        )
        .expect("clamped geometry");
        let following = compute_right_axis_label_geometry_with_vertical_mode(
            60.0,
            100.0,
            130.0,
            32.0,
            dpr,
            &metrics,
            0.0,
            RightAxisLabelWidthMode::AxisFull,
            RightAxisLabelVerticalMode::FollowValue,
        )
        .expect("following geometry");

        assert!(clamped.y_mid < 100.0);
        assert_eq!(following.y_mid, 130.0);
        assert!(following.y_top > clamped.y_top);
    }
}
