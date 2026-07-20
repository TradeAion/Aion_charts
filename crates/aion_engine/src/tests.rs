//! Headless-engine unit tests (extracted from `lib.rs`; `super` is the crate root).

use super::*;
use aion_render::canvas2d::{execute, Canvas2d, Viewport};
use aion_render::color::Color;

#[derive(Default)]
struct CountingCanvas {
    calls: usize,
}

impl Canvas2d for CountingCanvas {
    fn set_fill_solid(&mut self, _color: Color) {
        self.calls += 1;
    }
    fn set_fill_vgradient(&mut self, _y_top: f32, _y_bottom: f32, _top: Color, _bottom: Color) {
        self.calls += 1;
    }
    fn set_stroke(&mut self, _color: Color) {
        self.calls += 1;
    }
    fn set_line_width(&mut self, _width: f32) {
        self.calls += 1;
    }
    fn set_line_dash(&mut self, _pattern: &[f32]) {
        self.calls += 1;
    }
    fn fill_rect(&mut self, _x: f32, _y: f32, _w: f32, _h: f32) {
        self.calls += 1;
    }
    fn begin_path(&mut self) {
        self.calls += 1;
    }
    fn move_to(&mut self, _x: f32, _y: f32) {
        self.calls += 1;
    }
    fn line_to(&mut self, _x: f32, _y: f32) {
        self.calls += 1;
    }
    fn close_path(&mut self) {
        self.calls += 1;
    }
    fn arc(&mut self, _cx: f32, _cy: f32, _r: f32, _start: f32, _end: f32) {
        self.calls += 1;
    }
    fn stroke(&mut self) {
        self.calls += 1;
    }
    fn fill(&mut self) {
        self.calls += 1;
    }
}

#[test]
fn constructs_without_a_browser_or_gpu() {
    let chart = ChartEngine::new(800.0, 500.0, 2.0);
    assert_eq!(chart.series.len(), 1);
    assert_eq!(chart.panes.len(), 1);
    assert_eq!(chart.css_width, 800.0);
    assert_eq!(chart.dpr, 2.0);
}

#[test]
fn pane_layout_is_host_independent() {
    let mut pane = Pane::new();
    pane.top = 100.0;
    pane.height = 200.0;
    pane.layout(500.0);
    pane.price_scale.apply_autoscale_range(
        Some(aion_core::model::price_range::PriceRange::new(0.0, 2.0)),
        0.01,
    );
    let y = pane.price_scale.price_to_coordinate(1.0, 1.0);
    assert!(y.is_finite() && (100.0..=300.0).contains(&y));
}

#[test]
fn ingests_data_without_a_host_runtime() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    let report = chart
        .set_series_data(
            0,
            &[3.0, 1.0, 2.0],
            &[12.0, 10.0, 11.0],
            &[13.0, 11.0, 12.0],
            &[9.0, 8.0, 10.0],
            &[11.0, 10.0, 11.5],
        )
        .unwrap();
    assert!(report.reordered);
    assert_eq!(chart.data.merged_times(), &[1, 2, 3]);
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    assert!(chart.time_scale.visible_logical_range().is_some());
    let frame = chart.build_frame();
    assert_eq!(frame.panes.len(), 1);
    assert!(!frame.panes[0].main.is_empty());
}

#[test]
fn hidden_series_do_not_expand_autoscale() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart
        .set_series_data(
            0,
            &[1.0, 2.0],
            &[5.0, 6.0],
            &[10.0, 9.0],
            &[0.0, 1.0],
            &[7.0, 8.0],
        )
        .unwrap();
    let hidden = chart.add_series(SeriesKind::Line);
    chart
        .set_series_data(
            hidden,
            &[1.0, 2.0],
            &[1000.0, 1001.0],
            &[1000.0, 1001.0],
            &[1000.0, 1001.0],
            &[1000.0, 1001.0],
        )
        .unwrap();
    chart.set_series_visible(hidden, false);
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    chart.autoscale_visible();
    assert_eq!(
        chart.panes[0]
            .price_scale
            .price_range()
            .unwrap()
            .max_value(),
        10.0
    );

    chart.set_series_visible(hidden, true);
    chart.autoscale_visible();
    assert_eq!(
        chart.panes[0]
            .price_scale
            .price_range()
            .unwrap()
            .max_value(),
        1001.0
    );
}

#[test]
fn marker_autoscale_margins_are_headless_and_can_be_disabled() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart
        .set_series_data(
            0,
            &[1.0, 2.0],
            &[100.0, 101.0],
            &[102.0, 103.0],
            &[99.0, 100.0],
            &[101.0, 102.0],
        )
        .unwrap();
    chart.set_series_markers(
        0,
        vec![Marker {
            time: 2,
            position: marker_pos::ABOVE,
            shape: marker_shape::CIRCLE,
            color: Color::rgb(0x21, 0x96, 0xf3),
            text: String::new(),
        }],
    );
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    chart.build_frame();
    // Two fitted bars clamp marker geometry to LWC's maximum spacing bucket.
    assert_eq!(chart.panes[0].marker_margin_above, 48.0);
    assert_eq!(chart.panes[0].marker_margin_below, 0.0);

    chart.set_series_markers_auto_scale(0, false);
    chart.build_frame();
    assert_eq!(chart.panes[0].marker_margin_above, 0.0);
    assert_eq!(chart.panes[0].marker_margin_below, 0.0);
}

#[test]
fn public_time_scale_options_are_validated_and_headless() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart.time_scale.set_width(800.0);
    chart.set_bar_spacing(50.0);
    chart.set_right_offset(3.5);
    assert_eq!(chart.bar_spacing(), 50.0);
    assert_eq!(chart.right_offset(), 3.5);
    chart.set_bar_spacing(f64::NAN);
    chart.set_right_offset(f64::INFINITY);
    assert_eq!(chart.bar_spacing(), 50.0);
    assert_eq!(chart.right_offset(), 3.5);
}

#[test]
fn richer_time_scale_queries_and_mutations_are_headless() {
    let mut chart = ChartEngine::new(300.0, 200.0, 1.0);
    chart
        .set_series_data(
            0,
            &[10.0, 20.0, 30.0],
            &[1.0, 2.0, 3.0],
            &[1.0, 2.0, 3.0],
            &[1.0, 2.0, 3.0],
            &[1.0, 2.0, 3.0],
        )
        .unwrap();
    chart.time_scale.set_width(300.0);
    chart.fit_content();

    assert_eq!(chart.time_to_index(20.0, false), Some(1));
    assert_eq!(chart.time_to_index(15.0, false), None);
    assert_eq!(chart.time_to_index(15.0, true), Some(1));
    assert_eq!(chart.time_to_index(35.0, true), Some(2));
    let x = chart.logical_to_coordinate(1.0).unwrap();
    assert_eq!(chart.coordinate_to_logical(x), Some(1.0));
    assert_eq!(chart.logical_to_coordinate(1.25), Some(0.0));
    assert_eq!(
        chart.time_to_coordinate(20.0),
        chart.logical_to_coordinate(1.0)
    );
    assert_eq!(
        chart.coordinate_to_time(chart.logical_to_coordinate(2.0).unwrap()),
        Some(30.0)
    );

    chart.scroll_to_position(4.0);
    // The core clamps excessive future whitespace for a three-point data set.
    assert_eq!(chart.scroll_position(), 1.0);
    chart.scroll_to_real_time();
    assert_eq!(chart.scroll_position(), 0.0);
    chart.set_bar_spacing(20.0);
    chart.set_right_offset(2.0);
    chart.reset_time_scale();
    assert_eq!(chart.bar_spacing(), 6.0);
    assert_eq!(chart.right_offset(), 0.0);

    chart.set_visible_time_range(10.0, 20.0);
    assert_eq!(chart.visible_time_range(), Some((10.0, 20.0)));
}

#[test]
fn public_price_scale_state_is_headless_and_manual_ranges_survive_rendering() {
    let mut chart = ChartEngine::new(300.0, 200.0, 1.0);
    chart
        .set_series_data(
            0,
            &[10.0, 20.0, 30.0],
            &[100.0, 101.0, 102.0],
            &[101.0, 102.0, 103.0],
            &[99.0, 100.0, 101.0],
            &[100.5, 101.5, 102.5],
        )
        .unwrap();
    chart.time_scale.set_width(300.0);
    chart.layout_panes(172.0);
    chart.fit_content();
    chart.build_frame();
    assert_eq!(chart.price_scale_margins(0, false), Some((0.2, 0.1)));

    chart.set_price_scale_visible_range(0, false, 90.0, 110.0);
    assert_eq!(chart.price_scale_auto_scale(0, false), Some(false));
    chart.build_frame();
    assert_eq!(
        chart.price_scale_visible_range(0, false),
        Some((90.0, 110.0))
    );

    chart.set_price_scale_inverted(0, false, true);
    chart.set_price_scale_margins(0, false, 0.25, 0.15);
    assert_eq!(chart.price_scale_inverted(0, false), Some(true));
    assert_eq!(chart.price_scale_margins(0, false), Some((0.25, 0.15)));

    chart.set_price_scale_auto_scale(0, false, true);
    chart.build_frame();
    assert_eq!(chart.price_scale_auto_scale(0, false), Some(true));
    assert_ne!(
        chart.price_scale_visible_range(0, false),
        Some((90.0, 110.0))
    );
    assert_eq!(
        chart.series_price_scale(0),
        Some((0, PriceScaleTarget::Right))
    );

    chart.set_price_scale_mode(0, false, PriceScaleMode::Percentage);
    chart.build_frame();
    assert_eq!(
        chart.price_scale_mode(0, false),
        Some(PriceScaleMode::Percentage)
    );
    assert_eq!(chart.price_scale_auto_scale(0, false), Some(true));
    let coordinate = chart.series_price_to_coordinate(0, 101.5).unwrap();
    assert!((chart.series_coordinate_to_price(0, coordinate).unwrap() - 101.5).abs() < 1e-9);
    let axis = chart.build_axis_frame(80.0, |text| text.len() as f64 * 7.0);
    assert!(axis.labels.iter().any(|label| label.text.ends_with('%')));

    chart.set_price_scale_mode(0, false, PriceScaleMode::Logarithmic);
    chart.build_frame();
    let coordinate = chart.series_price_to_coordinate(0, 102.5).unwrap();
    assert!((chart.series_coordinate_to_price(0, coordinate).unwrap() - 102.5).abs() < 1e-8);
}

#[test]
fn left_price_scale_owns_range_axis_labels_and_pane_origin() {
    let mut chart = ChartEngine::new(300.0, 200.0, 1.0);
    chart
        .set_series_data(
            0,
            &[10.0, 20.0, 30.0],
            &[100.0, 101.0, 102.0],
            &[101.0, 102.0, 103.0],
            &[99.0, 100.0, 101.0],
            &[100.5, 101.5, 102.5],
        )
        .unwrap();
    chart.set_series_price_scale(0, PriceScaleTarget::Left);
    chart
        .options
        .apply_str(r#"{"leftPriceScale":{"visible":true},"rightPriceScale":{"visible":false}}"#)
        .unwrap();
    chart.pane_left = 58.0;
    chart.left_axis_w = 58.0;
    chart.pane_w = 242.0;
    chart.time_scale.set_width(242.0);
    chart.layout_panes(172.0);
    chart.fit_content();

    let frame = chart.build_frame();
    assert_eq!(
        chart.series_price_scale(0),
        Some((0, PriceScaleTarget::Left))
    );
    assert!(chart
        .price_scale_visible_range_for(0, PriceScaleTarget::Left)
        .is_some());
    assert!(chart
        .price_scale_visible_range_for(0, PriceScaleTarget::Right)
        .is_none());
    assert_eq!(frame.width, 300.0);
    assert_eq!(frame.panes[0].scissor[0], 58);
    assert!(frame.panes[0].main.iter().any(|prim| matches!(
        prim,
        aion_render::draw_list::Prim::Rect { rect, .. } if rect.x >= 58
    )));

    let axis = chart.build_axis_frame(80.0, |text| text.len() as f64 * 7.0);
    assert!(axis
        .labels
        .iter()
        .any(|label| label.align == AxisTextAlign::Right));
    assert!(!axis
        .labels
        .iter()
        .any(|label| label.align == AxisTextAlign::Left));
    let coordinate = chart.series_price_to_coordinate(0, 101.5).unwrap();
    assert!((chart.series_coordinate_to_price(0, coordinate).unwrap() - 101.5).abs() < 1e-9);
}

#[test]
fn series_data_and_logical_range_queries_match_lwc_gap_semantics() {
    let mut chart = ChartEngine::new(300.0, 200.0, 1.0);
    let times = (0..=10).map(|time| time as f64 * 10.0).collect::<Vec<_>>();
    let values = (0..=10).map(|value| value as f64).collect::<Vec<_>>();
    chart
        .set_series_data(0, &times, &values, &values, &values, &values)
        .unwrap();
    let sparse = chart.add_series(SeriesKind::Line);
    chart
        .set_series_data(
            sparse,
            &[0.0, 100.0],
            &[5.0, 15.0],
            &[5.0, 15.0],
            &[5.0, 15.0],
            &[5.0, 15.0],
        )
        .unwrap();

    assert_eq!(chart.series_kind(sparse), Some(SeriesKind::Line));
    assert_eq!(chart.series_data(sparse).len(), 2);
    assert_eq!(
        chart.series_data_by_index(sparse, 5, MismatchDirection::NearestLeft),
        Some(SeriesDataPoint {
            time: 0,
            open: 5.0,
            high: 5.0,
            low: 5.0,
            close: 5.0,
        })
    );
    assert_eq!(
        chart
            .series_data_by_index(sparse, 5, MismatchDirection::NearestRight)
            .map(|point| point.time),
        Some(100)
    );
    assert_eq!(
        chart.series_bars_in_logical_range(sparse, 3.0, 7.0),
        Some(BarsInLogicalRange {
            bars_before: 3.0,
            bars_after: 3.0,
            from: None,
            to: None,
        })
    );
    assert_eq!(
        chart.series_bars_in_logical_range(sparse, -1.5, 5.25),
        Some(BarsInLogicalRange {
            bars_before: -1.5,
            bars_after: 10.0,
            from: Some(0),
            to: Some(0),
        })
    );
}

#[test]
fn crosshair_geometry_is_host_independent() {
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
    chart.crosshair = Some((200.0, 120.0));
    let frame = chart.build_frame();
    assert!(frame.panes[0]
        .main
        .iter()
        .any(|p| matches!(p, aion_render::draw_list::Prim::VLine { .. })));
    assert!(frame.panes[0]
        .main
        .iter()
        .any(|p| matches!(p, aion_render::draw_list::Prim::HLine { .. })));
    assert!(frame.panes[0]
        .main
        .iter()
        .any(|p| matches!(p, aion_render::draw_list::Prim::Circle { .. })));

    let mut canvas = CountingCanvas::default();
    for pane in &frame.panes {
        execute(
            &pane.under,
            &pane.points,
            &mut canvas,
            Viewport {
                width: 800.0,
                height: 500.0,
            },
        );
        execute(
            &pane.main,
            &pane.points,
            &mut canvas,
            Viewport {
                width: 800.0,
                height: 500.0,
            },
        );
    }
    assert!(
        canvas.calls > 0,
        "the shared frame must be executable by a Canvas2D backend"
    );
}

#[test]
fn indicators_are_engine_owned_series() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart
        .set_series_data(
            0,
            &[1.0, 2.0, 3.0, 4.0],
            &[1.0, 2.0, 3.0, 4.0],
            &[1.0, 2.0, 3.0, 4.0],
            &[1.0, 2.0, 3.0, 4.0],
            &[1.0, 2.0, 3.0, 4.0],
        )
        .unwrap();
    let sma = chart.add_sma(0, 2).expect("valid indicator");
    let rows = chart.data.series_data(sma).unwrap();
    assert_eq!(rows.0, &[2, 3, 4]);
    assert_eq!(rows.1[3], &[1.5, 2.5, 3.5]);

    chart.update_series_bar(0, 4.0, [4.0, 5.0, 3.0, 5.0]);
    let rows = chart.data.series_data(sma).unwrap();
    assert_eq!(rows.1[3], &[1.5, 2.5, 4.0]);

    let ema = chart.add_ema(0, 2).expect("valid indicator");
    let initial_ema = chart.data.series_data(ema).unwrap().1[3];
    assert_eq!(initial_ema.len(), 3);
    assert!((initial_ema[2] - 4.166666666666667).abs() < 1e-12);
    chart.update_series_bar(0, 5.0, [5.0, 6.0, 4.0, 6.0]);
    let ema_rows = chart.data.series_data(ema).unwrap();
    assert!((ema_rows.1[3].last().copied().unwrap() - 5.388888888888889).abs() < 1e-12);
}

#[test]
fn bollinger_creates_three_output_series() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart
        .set_series_data(
            0,
            &[1.0, 2.0, 3.0],
            &[1.0, 2.0, 3.0],
            &[1.0, 2.0, 3.0],
            &[1.0, 2.0, 3.0],
            &[1.0, 2.0, 3.0],
        )
        .unwrap();
    let ids = chart.add_bollinger(0, 3, 2.0);
    assert_eq!(ids.len(), 3);
    assert!(chart.data.series_data(ids[0]).unwrap().1[3][0] > 3.0);
    assert_eq!(chart.data.series_data(ids[1]).unwrap().1[3], &[2.0]);
}

#[test]
fn retained_frame_reuses_pane_buffers() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart
        .set_series_data(
            0,
            &[1.0, 2.0, 3.0],
            &[1.0, 2.0, 3.0],
            &[2.0, 3.0, 4.0],
            &[0.0, 1.0, 2.0],
            &[1.5, 2.5, 3.5],
        )
        .unwrap();
    chart.time_scale.set_width(800.0);
    chart.fit_content();
    let mut frame = ChartFrame::default();
    chart.build_frame_into(&mut frame);
    let first_capacity = frame.panes[0].main.capacity();
    chart.crosshair = Some((300.0, 100.0));
    chart.build_frame_into(&mut frame);
    assert!(frame.panes[0].main.capacity() >= first_capacity);
}

#[test]
fn axis_frame_owns_label_content_and_positions() {
    let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
    chart
        .set_series_data(
            0,
            &[1.0, 2.0],
            &[10.0, 11.0],
            &[11.0, 12.0],
            &[9.0, 10.0],
            &[10.0, 11.0],
        )
        .unwrap();
    chart.time_scale.set_width(760.0);
    chart.fit_content();
    let axes = chart.build_axis_frame(80.0, |text| text.len() as f64);
    assert!(!axes.labels.is_empty());
    assert!(axes.labels.iter().any(|label| label.text.contains("11")));
}
