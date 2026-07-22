//! Frame-production unit tests (extracted from `frame.rs`).

use super::conflation::{VisibleHistogramRow, VisibleOhlc};
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
                close: 13.0,
                source_row: 3
            },
            VisibleOhlc {
                x_px: 1.0,
                open: 20.0,
                high: 25.0,
                low: 14.0,
                close: 16.0,
                source_row: 7
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

fn crosshair_chart() -> ChartEngine {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart.series[0].kind = SeriesKind::Line;
    chart
        .set_series_data(
            0,
            &[1.0, 2.0, 3.0],
            &[10.0, 11.0, 12.0],
            &[11.0, 12.0, 13.0],
            &[9.0, 10.0, 11.0],
            &[10.5, 11.5, 12.5],
        )
        .unwrap();
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    chart
}

#[test]
fn crosshair_clamps_into_pane_instead_of_vanishing() {
    let mut chart = crosshair_chart();
    // LWC pane-widget.ts:714-719: out-of-range positions clamp instead of hiding the crosshair.
    chart.crosshair = Some((10_000.0, 10_000.0));
    assert_eq!(chart.clamped_crosshair(), Some((799.0, 499.0)));
    let frame = chart.build_frame();
    assert!(frame.panes[0]
        .main
        .iter()
        .any(|p| matches!(p, Prim::VLine { .. })));
    assert!(frame.panes[0].main.iter().any(|p| matches!(
        p,
        Prim::HLine {
            style: LineStyle::LargeDashed,
            ..
        }
    )));

    chart.crosshair = Some((-50.0, -50.0));
    assert_eq!(chart.clamped_crosshair(), Some((0.0, 0.0)));
    let frame = chart.build_frame();
    assert!(frame.panes[0]
        .main
        .iter()
        .any(|p| matches!(p, Prim::VLine { .. })));

    // Hidden mode still suppresses the crosshair entirely.
    chart.crosshair_mode = CrosshairMode::Hidden;
    let frame = chart.build_frame();
    assert!(!frame.panes[0]
        .main
        .iter()
        .any(|p| matches!(p, Prim::VLine { .. })));
}

#[test]
fn crosshair_draws_without_a_primary_series() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    // The primary series (id 0) stays empty; a secondary line series carries the data.
    let secondary = chart.add_series(SeriesKind::Line);
    chart
        .set_series_data(
            secondary,
            &[1.0, 2.0, 3.0],
            &[10.0, 11.0, 12.0],
            &[10.0, 11.0, 12.0],
            &[10.0, 11.0, 12.0],
            &[10.0, 11.0, 12.0],
        )
        .unwrap();
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    chart.crosshair = Some((200.0, 120.0));

    let frame = chart.build_frame();
    assert!(frame.panes[0]
        .main
        .iter()
        .any(|p| matches!(p, Prim::VLine { .. })));
    assert!(frame.panes[0].main.iter().any(|p| matches!(
        p,
        Prim::HLine {
            style: LineStyle::LargeDashed,
            ..
        }
    )));

    // The time label needs only the time scale; the price label comes off the containing pane's
    // default scale (the secondary series' right scale here).
    let axis = chart.build_axis_frame(80.0, |t| t.len() as f64 * 7.0);
    assert!(axis
        .labels
        .iter()
        .any(|l| l.midpoint == AxisTextMidpoint::StableTime));
    assert!(axis.labels.iter().any(|l| l.background.is_some()));
}

#[test]
fn magnet_snaps_across_all_visible_series_on_the_pane() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart.crosshair_mode = CrosshairMode::Magnet;
    chart
        .set_series_data(
            0,
            &[1.0, 2.0, 3.0],
            &[99.0, 100.0, 101.0],
            &[99.0, 100.0, 101.0],
            &[99.0, 100.0, 101.0],
            &[99.0, 100.0, 101.0],
        )
        .unwrap();
    let other = chart.add_series(SeriesKind::Line);
    chart
        .set_series_data(
            other,
            &[1.0, 2.0, 3.0],
            &[109.0, 110.0, 111.0],
            &[109.0, 110.0, 111.0],
            &[109.0, 110.0, 111.0],
            &[109.0, 110.0, 111.0],
        )
        .unwrap();
    let left = chart.add_series(SeriesKind::Line);
    chart
        .set_series_data(
            left,
            &[1.0, 2.0, 3.0],
            &[490.0, 500.0, 510.0],
            &[490.0, 500.0, 510.0],
            &[490.0, 500.0, 510.0],
            &[490.0, 500.0, 510.0],
        )
        .unwrap();
    chart.set_series_price_scale(left, PriceScaleTarget::Left);
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    chart.build_frame();
    let (from, to) = chart.visible_range_for_frame().unwrap();
    let right = pane_scale(&chart.panes[0], PriceScaleTarget::Right);
    let base = chart.series_base_value(0, from).unwrap();
    let x = chart.time_scale.index_to_coordinate(1);

    // Same-scale pick: the old primary-only magnet could only snap to the primary's 100.
    let y110 = right.price_to_coordinate(110.0, base);
    let (price, snapped_y) = chart.crosshair_snap(0, x, y110, from, to);
    assert_eq!(snapped_y, y110);
    assert!((price - 110.0).abs() < 1e-9);

    // Cross-scale pick: the left-scale series' bar converts on its own scale; the winning
    // coordinate converts back to a price on the pane's default (right) scale.
    let left_scale = pane_scale(&chart.panes[0], PriceScaleTarget::Left);
    let left_base = chart.series_base_value(left, from).unwrap();
    let y500 = left_scale.price_to_coordinate(500.0, left_base);
    let expected_on_default = right.coordinate_to_price(y500, base);
    let (price, snapped_y) = chart.crosshair_snap(0, x, y500, from, to);
    assert_eq!(snapped_y, y500);
    assert!((price - expected_on_default).abs() < 1e-9);
    assert!((price - 500.0).abs() > 1.0); // not the left-scale price
}

#[test]
fn magnet_ohlc_picks_nearest_of_open_high_low_close() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart.crosshair_mode = CrosshairMode::MagnetOhlc;
    chart
        .set_series_data(
            0,
            &[1.0, 2.0, 3.0],
            &[10.0, 20.0, 30.0],
            &[15.0, 28.0, 35.0],
            &[8.0, 12.0, 25.0],
            &[12.0, 24.0, 33.0],
        )
        .unwrap();
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    chart.build_frame();
    let (from, to) = chart.visible_range_for_frame().unwrap();
    let scale = pane_scale(&chart.panes[0], PriceScaleTarget::Right);
    let base = chart.series_base_value(0, from).unwrap();
    let x = chart.time_scale.index_to_coordinate(1);

    // Bar 1: open 20, high 28, low 12, close 24 — cursor nearest the high picks 28.
    let y28 = scale.price_to_coordinate(28.0, base);
    let (price, _) = chart.crosshair_snap(0, x, y28, from, to);
    assert!((price - 28.0).abs() < 1e-9);
    // Cursor nearest the low picks 12.
    let y12 = scale.price_to_coordinate(12.0, base);
    let (price, _) = chart.crosshair_snap(0, x, y12, from, to);
    assert!((price - 12.0).abs() < 1e-9);
}

#[test]
fn normal_mode_keeps_the_raw_cursor_price() {
    let mut chart = crosshair_chart();
    chart.crosshair_mode = CrosshairMode::Normal;
    chart.build_frame();
    let (from, to) = chart.visible_range_for_frame().unwrap();
    let scale = pane_scale(&chart.panes[0], PriceScaleTarget::Right);
    let base = chart.series_base_value(0, from).unwrap();
    let x = chart.time_scale.index_to_coordinate(1);
    let (price, snapped_y) = chart.crosshair_snap(0, x, 120.0, from, to);
    assert_eq!(snapped_y, 120.0);
    assert!((price - scale.coordinate_to_price(120.0, base)).abs() < 1e-9);
}

#[test]
fn do_not_snap_to_hidden_series_indices_moves_to_a_visible_bar() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    // Primary (visible) bars sit at merged indices 0, 1, 3; a hidden series owns index 2.
    chart
        .set_series_data(
            0,
            &[1.0, 2.0, 4.0],
            &[10.0, 11.0, 12.0],
            &[10.0, 11.0, 12.0],
            &[10.0, 11.0, 12.0],
            &[10.0, 11.0, 12.0],
        )
        .unwrap();
    let hidden = chart.add_series(SeriesKind::Line);
    chart
        .set_series_data(hidden, &[3.0], &[20.0], &[20.0], &[20.0], &[20.0])
        .unwrap();
    chart.set_series_visible(hidden, false);
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    let (from, to) = chart.visible_range_for_frame().unwrap();
    assert_eq!((from, to), (0, 3));
    let x2 = chart.time_scale.index_to_coordinate(2);

    // Default (off): the snapped index stays on the hidden-only bar.
    assert_eq!(chart.snapped_crosshair_index(x2, from, to), 2);

    // On: it moves to the nearest visible-series bar; the tie resolves left (LWC `indexOf(min)`).
    chart
        .options
        .apply_str(r#"{"crosshair":{"doNotSnapToHiddenSeriesIndices":true}}"#)
        .unwrap();
    assert_eq!(chart.snapped_crosshair_index(x2, from, to), 1);

    // The drawn vertical line follows the moved index.
    chart.crosshair = Some((x2, 120.0));
    let frame = chart.build_frame();
    let expected_x = chart.time_scale.index_to_coordinate(1).round() as i32;
    assert!(frame.panes[0]
        .main
        .iter()
        .any(|p| matches!(p, Prim::VLine { x, .. } if *x == expected_x)));
}

/// Two identical line series on the right scale (same last close => colliding label
/// candidates, the case LWC's overlap resolution exists for).
fn two_identical_line_series() -> ChartEngine {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart.series[0].kind = SeriesKind::Line;
    let times = [1.0, 2.0, 3.0, 4.0, 5.0];
    let values = [10.0, 11.0, 12.0, 11.5, 12.5];
    chart
        .set_series_data(0, &times, &values, &values, &values, &values)
        .unwrap();
    let second = chart.add_series(SeriesKind::Line);
    chart
        .set_series_data(second, &times, &values, &values, &values, &values)
        .unwrap();
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    chart
}

#[test]
fn last_value_labels_cover_every_visible_series_and_resolve_overlap() {
    let mut chart = two_identical_line_series();
    let boxed = |chart: &mut ChartEngine| {
        chart
            .build_axis_frame(80.0, |t| t.len() as f64 * 7.0)
            .labels
            .into_iter()
            .filter(|l| l.background.is_some())
            .collect::<Vec<_>>()
    };

    // LWC SeriesPriceAxisView: every visible series with lastValueVisible gets a label on its
    // scale, in the series' bar color (the line color for a line series — not up/down).
    let labels = boxed(&mut chart);
    assert_eq!(labels.len(), 2);
    assert!(labels
        .iter()
        .all(|l| matches!(l.background, Some((.., c)) if c == LINE)));
    // LWC `_fixLabelOverlap`: colliding labels are pushed apart by their box height.
    let height = 12.0 + 2.5 * 2.0;
    let gap = (labels[0].y - labels[1].y).abs();
    assert!(
        (gap - height).abs() < 1e-9,
        "overlapping labels must be pushed a full box height apart, got {gap}"
    );

    // `lastValueVisible: false` on one series drops only its label.
    chart.series[1].last_value_visible = false;
    assert_eq!(boxed(&mut chart).len(), 1);

    // A hidden series loses its label entirely (LWC series-price-axis-view.ts:24).
    chart.series[1].last_value_visible = true;
    chart.set_series_visible(1, false);
    assert_eq!(boxed(&mut chart).len(), 1);
}

#[test]
fn last_value_label_tracks_the_last_visible_bar() {
    let mut chart = two_identical_line_series();
    chart.series[1].last_value_visible = false;
    // Scroll one bar past the right edge: the label follows the last *visible* bar (LWC
    // series.ts lastValueData(false)), not the series' final bar.
    chart.set_right_offset(-1.0);
    let labels = chart
        .build_axis_frame(80.0, |t| t.len() as f64 * 7.0)
        .labels;
    let label = labels
        .iter()
        .find(|l| l.background.is_some())
        .expect("last-value label");
    let (_, to) = chart.visible_range_for_frame().unwrap();
    let scale = pane_scale(&chart.panes[0], PriceScaleTarget::Right);
    let base = chart.series_base_value(0, 0).unwrap();
    let expected_y = scale.price_to_coordinate(11.5, base); // close of bar index `to` = 3
    assert_eq!(to, 3);
    assert!((label.y - expected_y).abs() < 1e-9);
}

#[test]
fn price_line_family_renders_per_series_with_lwc_defaults() {
    let mut chart = two_identical_line_series();
    let dashed_ylines = |chart: &mut ChartEngine| {
        chart.build_frame().panes[0]
            .main
            .iter()
            .filter_map(|p| match p {
                Prim::HLine {
                    y,
                    style: LineStyle::Dashed,
                    width,
                    color,
                    ..
                } => Some((*y, *width, *color)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };

    // LWC priceLineVisible default true: every visible series gets a built-in last-price
    // line (dashed, 1px, following the bar color — the line color for a line series).
    let lines = dashed_ylines(&mut chart);
    assert_eq!(lines.len(), 2);
    assert!(lines
        .iter()
        .all(|&(_, width, color)| width == 1 && color == LINE));

    // priceLineVisible: false hides only that series' line.
    chart.series[1].price_line_visible = false;
    assert_eq!(dashed_ylines(&mut chart).len(), 1);

    // priceLineSource LastVisible anchors at the last visible bar when the final bar is
    // scrolled off the right edge; LastBar keeps the final bar.
    chart.series[0].price_line_source = 1;
    chart.series[1].price_line_source = 0;
    chart.series[1].price_line_visible = true;
    chart.set_right_offset(-1.0);
    let lines = dashed_ylines(&mut chart); // builds the frame, autoscaling the new window first
    let scale = pane_scale(&chart.panes[0], PriceScaleTarget::Right);
    let base = chart.series_base_value(0, 0).unwrap();
    let y_last_visible = (scale.price_to_coordinate(11.5, base) * 1.0).round() as i32;
    let y_last_bar = (scale.price_to_coordinate(12.5, base) * 1.0).round() as i32;
    assert_eq!(lines.len(), 2);
    assert!(lines.iter().any(|&(y, ..)| y == y_last_visible));
    assert!(lines.iter().any(|&(y, ..)| y == y_last_bar));

    // priceLineWidth / priceLineColor / priceLineStyle all reach the frame.
    chart.series[0].price_line_width = 3.0;
    chart.series[0].price_line_color = Some("#112233".to_string());
    chart.series[0].price_line_style = 0;
    chart.set_right_offset(0.0);
    let frame = chart.build_frame();
    assert!(frame.panes[0].main.iter().any(|p| matches!(
        p,
        Prim::HLine {
            width: 3,
            style: LineStyle::Solid,
            color,
            ..
        } if *color == Color::rgb(0x11, 0x22, 0x33)
    )));
}

#[test]
fn price_line_color_css_string_parses_at_render_time() {
    let mut chart = two_identical_line_series();
    // Uppercase hex is stored verbatim (options() returns it as-is) and parsed only when the
    // frame resolves the line color.
    assert!(chart.series_apply_options_json(0, r##"{"price_line_color": "#FF0000"}"##));
    let options: serde_json::Value =
        serde_json::from_str(&chart.series_options_json(0).unwrap()).unwrap();
    assert_eq!(options["price_line_color"], "#FF0000");
    let frame = chart.build_frame();
    assert!(frame.panes[0].main.iter().any(|p| matches!(
        p,
        Prim::HLine { color, .. } if *color == Color::rgb(0xFF, 0x00, 0x00)
    )));

    // An unparseable string (named color) falls back to the follow-the-bar-color default.
    assert!(chart.series_apply_options_json(0, r#"{"price_line_color": "red"}"#));
    let frame = chart.build_frame();
    assert!(frame.panes[0].main.iter().any(|p| matches!(
        p,
        Prim::HLine { color, .. } if *color == LINE
    )));
}

#[test]
fn dashed_line_style_splits_the_polyline_into_solid_runs() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart.series[0].kind = SeriesKind::Line;
    let times: Vec<f64> = (0..40).map(|i| i as f64).collect();
    let values: Vec<f64> = (0..40).map(|i| 100.0 + (i % 7) as f64).collect();
    chart
        .set_series_data(0, &times, &values, &values, &values, &values)
        .unwrap();
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    chart.series[0].line_style = 2; // LWC LineStyle.Dashed

    let frame = chart.build_frame();
    let runs: Vec<_> = frame.panes[0]
        .main
        .iter()
        .filter_map(|p| match p {
            Prim::Polyline {
                first_point,
                point_count,
                style,
                line_type,
                ..
            } => Some((*first_point, *point_count, *style, *line_type)),
            _ => None,
        })
        .collect();
    // A dashed stroke arrives as several solid sub-segments (gap geometry is frame-built, so
    // WebGPU and Canvas2D rasterize identical dashes).
    assert!(runs.len() > 1, "expected dash sub-segments, got {runs:?}");
    assert!(runs
        .iter()
        .all(|&(.., style, line_type)| style == LineStyle::Solid && line_type == LineType::Simple));
    // The runs leave real gaps: their on-length totals less than the full path length.
    let pool = &frame.panes[0].points;
    let on_length: f32 = runs
        .iter()
        .map(|&(first, count, ..)| {
            let w = &pool[first as usize..(first + count) as usize];
            w.windows(2)
                .map(|p| ((p[1][0] - p[0][0]).powi(2) + (p[1][1] - p[0][1]).powi(2)).sqrt())
                .sum::<f32>()
        })
        .sum();
    let full_length: f32 = {
        // solid reference frame: same line with the default solid style
        let mut solid = ChartEngine::new(800.0, 500.0, 1.0);
        solid.series[0].kind = SeriesKind::Line;
        solid
            .set_series_data(0, &times, &values, &values, &values, &values)
            .unwrap();
        solid.time_scale.set_width(800.0);
        solid.fit_content();
        let frame = solid.build_frame();
        let run = frame.panes[0]
            .main
            .iter()
            .find_map(|p| match p {
                Prim::Polyline {
                    first_point,
                    point_count,
                    ..
                } => Some((*first_point, *point_count)),
                _ => None,
            })
            .expect("solid line polyline");
        let w = &frame.panes[0].points[run.0 as usize..(run.0 + run.1) as usize];
        w.windows(2)
            .map(|p| ((p[1][0] - p[0][0]).powi(2) + (p[1][1] - p[0][1]).powi(2)).sqrt())
            .sum::<f32>()
    };
    assert!(
        on_length < full_length * 0.95,
        "dashes must leave gaps: on {on_length} vs full {full_length}"
    );
}

#[test]
fn line_visible_false_keeps_area_fill_but_drops_the_stroke() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart.series[0].kind = SeriesKind::Area;
    let times = [1.0, 2.0, 3.0];
    let values = [10.0, 11.0, 12.0];
    chart
        .set_series_data(0, &times, &values, &values, &values, &values)
        .unwrap();
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    chart.series[0].line_visible = false;

    // LWC lineVisible: the fill stays, the stroke goes.
    let frame = chart.build_frame();
    assert!(frame.panes[0]
        .main
        .iter()
        .any(|p| matches!(p, Prim::AreaFill { .. })));
    assert!(!frame.panes[0]
        .main
        .iter()
        .any(|p| matches!(p, Prim::Polyline { .. })));

    // A line series keeps nothing but its point markers.
    chart.series[0].kind = SeriesKind::Line;
    chart.series[0].point_markers = true;
    let frame = chart.build_frame();
    assert!(!frame.panes[0]
        .main
        .iter()
        .any(|p| matches!(p, Prim::Polyline { .. } | Prim::AreaFill { .. })));
    assert!(frame.panes[0]
        .main
        .iter()
        .any(|p| matches!(p, Prim::Circle { .. })));
}

#[test]
fn point_markers_radius_option_overrides_the_lwc_auto_default() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart.series[0].kind = SeriesKind::Line;
    let times = [1.0, 2.0, 3.0];
    let values = [10.0, 11.0, 12.0];
    chart
        .set_series_data(0, &times, &values, &values, &values, &values)
        .unwrap();
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    chart.series[0].point_markers = true;
    let radius = |chart: &mut ChartEngine| {
        chart.build_frame().panes[0]
            .main
            .iter()
            .find_map(|p| match p {
                Prim::Circle { radius, .. } => Some(*radius),
                _ => None,
            })
            .expect("point marker circle")
    };
    // LWC auto radius (line-pane-view.ts): lineWidth / 2 + 2 = 3.5 at the default width 3.
    assert_eq!(radius(&mut chart), 3.5);
    chart.series[0].point_markers_radius = Some(6.0);
    assert_eq!(radius(&mut chart), 6.0);
}

#[test]
fn crosshair_marks_cover_all_line_series_with_per_series_options() {
    let mut chart = two_identical_line_series();
    let x = chart.time_scale.index_to_coordinate(2);
    chart.crosshair = Some((x, 120.0));
    let circles = |chart: &mut ChartEngine| {
        chart.build_frame().panes[0]
            .main
            .iter()
            .filter_map(|p| match p {
                Prim::Circle { radius, fill, .. } => Some((*radius, *fill)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };

    // LWC crosshair-marks-pane-view: one mark per visible line-like series at the crosshair
    // index. Defaults: radius 4 + border 2, border = chart background, fill = bar color.
    let marks = circles(&mut chart);
    assert_eq!(marks.len(), 4);
    assert!(marks
        .iter()
        .any(|&(r, c)| r == 6.0 && c == Color::rgb(0xff, 0xff, 0xff)));
    assert!(marks.iter().any(|&(r, c)| r == 4.0 && c == LINE));

    // crosshairMarkerVisible: false drops the series' mark.
    chart.series[1].crosshair_marker_visible = false;
    assert_eq!(circles(&mut chart).len(), 2);

    // Per-series radius / border width / pinned colors reach the frame.
    chart.series[0].crosshair_marker_radius = 7.0;
    chart.series[0].crosshair_marker_border_width = 3.0;
    chart.series[0].crosshair_marker_border_color = Some("#010203".to_string());
    chart.series[0].crosshair_marker_background_color = Some("#040506".to_string());
    let marks = circles(&mut chart);
    assert!(marks
        .iter()
        .any(|&(r, c)| r == 10.0 && c == Color::rgb(0x01, 0x02, 0x03)));
    assert!(marks
        .iter()
        .any(|&(r, c)| r == 7.0 && c == Color::rgb(0x04, 0x05, 0x06)));
}

#[test]
fn baseline_quadrant_options_flow_into_fills_strokes_and_marker() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart.series[0].kind = SeriesKind::Baseline;
    let times = [1.0, 2.0];
    let values = [10.0, 20.0]; // auto baseline = 15, so the segment crosses it
    chart
        .set_series_data(0, &times, &values, &values, &values, &values)
        .unwrap();
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    let frame = chart.build_frame();

    // LWC baselineStyleDefaults: two-stop gradients per quadrant (alphas 0.28 -> 71, 0.05 -> 13).
    assert!(frame.panes[0].main.iter().any(|p| matches!(
        p,
        Prim::AreaFill { gradient, .. }
            if gradient.top == Color::rgba(0x26, 0xa6, 0x9a, 71)
                && gradient.bottom == Color::rgba(0x26, 0xa6, 0x9a, 13)
    )));
    assert!(frame.panes[0].main.iter().any(|p| matches!(
        p,
        Prim::AreaFill { gradient, .. }
            if gradient.top == Color::rgba(0xef, 0x53, 0x50, 13)
                && gradient.bottom == Color::rgba(0xef, 0x53, 0x50, 71)
    )));
    // One continuous solid stroke per quadrant in the LWC line colors.
    assert!(frame.panes[0].main.iter().any(|p| matches!(
        p,
        Prim::Polyline { color, .. } if *color == Color::rgb(0x26, 0xa6, 0x9a)
    )));
    assert!(frame.panes[0].main.iter().any(|p| matches!(
        p,
        Prim::Polyline { color, .. } if *color == Color::rgb(0xef, 0x53, 0x50)
    )));

    // The crosshair marker background follows the baseline bar colorer: the last bar sits
    // above the baseline, so the marker is the top line color.
    let x = chart.time_scale.index_to_coordinate(1);
    chart.crosshair = Some((x, 120.0));
    let frame = chart.build_frame();
    assert!(frame.panes[0].main.iter().any(|p| matches!(
        p,
        Prim::Circle { radius, fill, .. }
            if *radius == 4.0 && *fill == Color::rgb(0x26, 0xa6, 0x9a)
    )));
    chart.crosshair = None;

    // Per-quadrant options: custom colors/widths, and a dashed quadrant style splits runs.
    assert!(chart.series_apply_options_json(
        0,
        r##"{"top_line_color": "#010203", "top_fill_color1": "#0a0b0c",
             "top_fill_color2": "#0d0e0f", "top_line_width": 5, "bottom_line_style": 2}"##
    ));
    let frame = chart.build_frame();
    assert!(frame.panes[0].main.iter().any(|p| matches!(
        p,
        Prim::Polyline { color, width, .. } if *color == Color::rgb(0x01, 0x02, 0x03) && *width == 5.0
    )));
    assert!(frame.panes[0].main.iter().any(|p| matches!(
        p,
        Prim::AreaFill { gradient, .. }
            if gradient.top == Color::rgb(0x0a, 0x0b, 0x0c)
                && gradient.bottom == Color::rgb(0x0d, 0x0e, 0x0f)
    )));
    let bottom_runs = frame.panes[0]
        .main
        .iter()
        .filter(|p| {
            matches!(
                p,
                Prim::Polyline { color, .. } if *color == Color::rgb(0xef, 0x53, 0x50)
            )
        })
        .count();
    assert!(bottom_runs > 1, "dashed quadrant must split into runs");

    // lineVisible: false drops both quadrant strokes but keeps the fills.
    chart.series[0].line_visible = false;
    let frame = chart.build_frame();
    assert!(!frame.panes[0]
        .main
        .iter()
        .any(|p| matches!(p, Prim::Polyline { .. })));
    assert!(frame.panes[0]
        .main
        .iter()
        .any(|p| matches!(p, Prim::AreaFill { .. })));
}

#[test]
fn histogram_base_offsets_the_column_level() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    let histogram = chart.add_series(SeriesKind::Histogram);
    let times = [1.0, 2.0, 3.0];
    let values = [1.0, 2.0, 3.0];
    chart
        .set_series_data(histogram, &times, &values, &values, &values, &values)
        .unwrap();
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    chart.series[histogram].base = 4.0; // above every value: columns hang down from the base

    let frame = chart.build_frame();
    chart.autoscale_visible();
    let scale = pane_scale(&chart.panes[0], PriceScaleTarget::Right);
    let base_value = chart.series_base_value(histogram, 0).unwrap();
    let expected_top = (scale.price_to_coordinate(4.0, base_value) * 1.0).round() as i32;
    let rects: Vec<_> = frame.panes[0]
        .main
        .iter()
        .filter_map(|p| match p {
            Prim::Rect { rect, .. } => Some(*rect),
            _ => None,
        })
        .collect();
    assert_eq!(rects.len(), 3);
    assert!(
        rects.iter().all(|r| r.y == expected_top),
        "columns below the base start at the base level: {rects:?} vs {expected_top}"
    );
}

#[test]
fn invert_filled_area_flips_the_area_base_to_the_pane_top() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart.series[0].kind = SeriesKind::Area;
    let times = [1.0, 2.0, 3.0];
    let values = [10.0, 11.0, 12.0];
    chart
        .set_series_data(0, &times, &values, &values, &values, &values)
        .unwrap();
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    let base_y = |chart: &mut ChartEngine| {
        chart.build_frame().panes[0]
            .main
            .iter()
            .find_map(|p| match p {
                Prim::AreaFill { base_y, .. } => Some(*base_y),
                _ => None,
            })
            .expect("area fill")
    };
    // Default: fill from the line down to the pane bottom (500 at dpr 1).
    assert_eq!(base_y(&mut chart), 500.0);
    // LWC invertFilledArea: fill from the pane top down to the line.
    chart.series[0].invert_filled_area = true;
    assert_eq!(base_y(&mut chart), 0.0);
}

#[test]
fn bar_open_visible_and_thin_bars_reach_the_builder() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart.series[0].kind = SeriesKind::Bar;
    let times: Vec<f64> = (0..5).map(|i| i as f64).collect();
    let open: Vec<f64> = (0..5).map(|i| 100.0 + i as f64).collect();
    let high: Vec<f64> = open.iter().map(|v| v + 2.0).collect();
    let low: Vec<f64> = open.iter().map(|v| v - 2.0).collect();
    let close: Vec<f64> = open.iter().map(|v| v + 0.5).collect();
    chart
        .set_series_data(0, &times, &open, &high, &low, &close)
        .unwrap();
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    chart.set_bar_spacing(10.0);
    let rects = |chart: &mut ChartEngine| {
        chart.build_frame().panes[0]
            .main
            .iter()
            .filter_map(|p| match p {
                Prim::Rect { rect, .. } => Some(*rect),
                _ => None,
            })
            .collect::<Vec<_>>()
    };

    // LWC barStyleDefaults (openVisible true, thinBars true): body + open/close ticks per bar,
    // body capped to the 1px crisp width.
    let rs = rects(&mut chart);
    assert_eq!(rs.len(), 3 * 5);
    assert!(!rs.iter().any(|r| r.w == 3), "thin bodies stay 1px wide");

    // thinBars: false lets the body take the full optimal width (floor(10 * 0.3) = 3).
    chart.series[0].thin_bars = false;
    let rs = rects(&mut chart);
    assert!(rs.iter().any(|r| r.w == 3), "thick bodies reach the frame");

    // openVisible: false drops the open tick (body + close only).
    chart.series[0].open_visible = false;
    let rs = rects(&mut chart);
    assert_eq!(rs.len(), 2 * 5);
}

// --- per-data-point colors (LWC data-item colors, series-bar-colorer.ts) ---

const POINT_RED: u32 = 0xFF0000FF;
const POINT_GREEN: u32 = 0x00FF00FF;
const POINT_BLUE: u32 = 0x0000FFFF;

fn ohlc_chart(kind: SeriesKind, bars: usize) -> ChartEngine {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart.series[0].kind = kind;
    let times: Vec<f64> = (0..bars).map(|i| i as f64).collect();
    let open: Vec<f64> = (0..bars).map(|i| 100.0 + i as f64).collect();
    let high: Vec<f64> = open.iter().map(|v| v + 2.0).collect();
    let low: Vec<f64> = open.iter().map(|v| v - 2.0).collect();
    let close: Vec<f64> = open.iter().map(|v| v + 0.5).collect();
    chart
        .set_series_data(0, &times, &open, &high, &low, &close)
        .unwrap();
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    chart
}

#[test]
fn candlestick_per_point_colors_override_each_channel() {
    let mut chart = ohlc_chart(SeriesKind::Candlestick, 4);
    // Bar 1 (rising): custom body/wick/border. Bar 2 keeps the series resolution.
    assert!(chart.set_series_point_colors(
        0,
        Some(vec![0, POINT_RED, 0, 0]),
        Some(vec![0, POINT_GREEN, 0, 0]),
        Some(vec![0, POINT_BLUE, 0, 0]),
    ));
    let frame = chart.build_frame();
    let has = |color: u32| {
        frame.panes[0].main.iter().any(|p| match p {
            Prim::Rect { color: c, .. } => *c == Color(color),
            Prim::RectFrame { color: c, .. } => *c == Color(color),
            _ => false,
        })
    };
    assert!(has(POINT_RED), "custom body color drawn");
    assert!(has(POINT_GREEN), "custom wick color drawn");
    assert!(has(POINT_BLUE), "custom border color drawn");
    // The uncolored bars keep the LWC up-color resolution.
    assert!(has(0x26A69AFF), "series up color still drawn");
}

#[test]
fn bar_per_point_color_overrides_updown() {
    let mut chart = ohlc_chart(SeriesKind::Bar, 4);
    assert!(chart.set_series_point_colors(0, Some(vec![0, POINT_RED, 0, 0]), None, None));
    let frame = chart.build_frame();
    assert!(frame.panes[0].main.iter().any(|p| matches!(
        p,
        Prim::Rect { color, .. } if *color == Color(POINT_RED)
    )));
}

#[test]
fn histogram_per_bar_color_overrides_the_updown_tint() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart.series[0].kind = SeriesKind::Line;
    let times: Vec<f64> = (0..4).map(|i| i as f64).collect();
    let values: Vec<f64> = (0..4).map(|i| 100.0 + i as f64).collect();
    chart
        .set_series_data(0, &times, &values, &values, &values, &values)
        .unwrap();
    let volume = chart.add_series(SeriesKind::Histogram);
    chart
        .set_series_data(volume, &times, &values, &values, &values, &values)
        .unwrap();
    chart.series[volume].histogram_updown = true;
    chart.time_scale.set_width(800.0);
    chart.fit_content();

    // Default: the up/down tint colors every column.
    let frame = chart.build_frame();
    assert!(frame.panes[0].main.iter().any(|p| matches!(
        p,
        Prim::Rect { color, .. } if *color == VOLUME_UP
    )));

    // A per-bar color wins over the tint for that bar only.
    assert!(chart.set_series_point_colors(volume, Some(vec![0, POINT_RED, 0, 0]), None, None));
    let frame = chart.build_frame();
    assert!(frame.panes[0].main.iter().any(|p| matches!(
        p,
        Prim::Rect { color, .. } if *color == Color(POINT_RED)
    )));
    assert!(frame.panes[0].main.iter().any(|p| matches!(
        p,
        Prim::Rect { color, .. } if *color == VOLUME_UP
    )));
}

#[test]
fn line_per_point_colors_split_the_stroke_and_color_markers() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart.series[0].kind = SeriesKind::Line;
    chart.series[0].point_markers = true;
    let times: Vec<f64> = (0..4).map(|i| i as f64).collect();
    let values: Vec<f64> = (0..4).map(|i| 100.0 + i as f64).collect();
    chart
        .set_series_data(0, &times, &values, &values, &values, &values)
        .unwrap();
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    // Points 0,1 default (blue), points 2,3 red. LWC walkLine: the segment leaving a point
    // takes the point's color, so the runs are [0..2] blue and [2..3] red, sharing point 2.
    assert!(chart.set_series_point_colors(0, Some(vec![0, 0, POINT_RED, POINT_RED]), None, None));
    let frame = chart.build_frame();
    let runs: Vec<(u32, u32, Color)> = frame.panes[0]
        .main
        .iter()
        .filter_map(|p| match p {
            Prim::Polyline {
                first_point,
                point_count,
                color,
                ..
            } => Some((*first_point, *point_count, *color)),
            _ => None,
        })
        .collect();
    assert_eq!(runs.len(), 2, "one run per color span: {runs:?}");
    assert_eq!(runs[0].2, LINE);
    assert_eq!(runs[0].1, 3, "blue run covers points 0..=2");
    assert_eq!(runs[1].2, Color(POINT_RED));
    assert_eq!(runs[1].1, 2, "red run covers points 2..=3");
    // Adjacent runs share the boundary point, keeping the path continuous.
    let pool = &frame.panes[0].points;
    let end_of_blue = pool[(runs[0].0 + runs[0].1 - 1) as usize];
    let start_of_red = pool[runs[1].0 as usize];
    assert_eq!(end_of_blue, start_of_red);

    // Point markers take their own point's color (LWC draw-series-point-markers.ts).
    let marker_fills: Vec<Color> = frame.panes[0]
        .main
        .iter()
        .filter_map(|p| match p {
            Prim::Circle { fill, radius, .. } if *radius > 0.0 => Some(*fill),
            _ => None,
        })
        .collect();
    assert_eq!(marker_fills.len(), 4);
    assert_eq!(marker_fills[0], LINE);
    assert_eq!(marker_fills[1], LINE);
    assert_eq!(marker_fills[2], Color(POINT_RED));
    assert_eq!(marker_fills[3], Color(POINT_RED));
}

#[test]
fn area_per_point_colors_split_only_the_stroke() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart.series[0].kind = SeriesKind::Area;
    let times: Vec<f64> = (0..3).map(|i| i as f64).collect();
    let values: Vec<f64> = (0..3).map(|i| 100.0 + i as f64).collect();
    chart
        .set_series_data(0, &times, &values, &values, &values, &values)
        .unwrap();
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    assert!(chart.set_series_point_colors(0, Some(vec![0, POINT_RED, 0]), None, None));
    let frame = chart.build_frame();
    // The fill keeps the series-level gradient (documented deviation; LWC's `lineColor` data
    // field affects only the stroke).
    assert!(frame.panes[0].main.iter().any(|p| matches!(
        p,
        Prim::AreaFill { gradient, .. }
            if gradient.top == AREA_TOP && gradient.bottom == AREA_BOTTOM
    )));
    // The stroke splits: segment 0->1 keeps the series color, 1->2 takes point 1's color.
    let stroke_colors: Vec<Color> = frame.panes[0]
        .main
        .iter()
        .filter_map(|p| match p {
            Prim::Polyline { color, .. } => Some(*color),
            _ => None,
        })
        .collect();
    assert_eq!(stroke_colors, vec![AREA_LINE, Color(POINT_RED)]);
}

#[test]
fn last_value_label_background_honors_the_per_point_color() {
    let mut chart = ohlc_chart(SeriesKind::Candlestick, 3);
    // The final bar carries a custom body color: the last-value label (and the built-in
    // last-price line) follow it (LWC series-bar-colorer.ts).
    assert!(chart.set_series_point_colors(0, Some(vec![0, 0, POINT_RED]), None, None));
    let axis = chart.build_axis_frame(80.0, |t| t.len() as f64 * 7.0);
    assert!(axis
        .labels
        .iter()
        .any(|l| matches!(l.background, Some((.., c)) if c == Color(POINT_RED))));
    let frame = chart.build_frame();
    assert!(frame.panes[0].main.iter().any(|p| matches!(
        p,
        Prim::HLine { color, style: LineStyle::Dashed, .. } if *color == Color(POINT_RED)
    )));
}
