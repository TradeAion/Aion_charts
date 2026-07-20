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
