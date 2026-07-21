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
