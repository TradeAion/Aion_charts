// TEMPORARY probe test — will be removed after diagnosis.
use super::*;
use crate::SeriesKind;

#[test]
fn probe_out_of_pane_coordinates() {
    let chart = settled_candle_chart();
    // far out-of-pane coordinates in every direction, incl. onto the axis strips
    for &(x, y) in &[(-500.0, 250.0), (10_000.0, 250.0), (250.0, -100.0), (250.0, 10_000.0), (1_000_000.0, 250.0)] {
        let _ = chart.hit_test_series(x, y);
        let _ = chart.hit_test_one_series(0, x, y);
    }
    let line = settled_line_chart(&[1.0, 2.0, 3.0, 4.0, 5.0]);
    for &(x, y) in &[(-500.0, 250.0), (10_000.0, 250.0), (250.0, -100.0), (250.0, 10_000.0)] {
        let _ = line.hit_test_series(x, y);
        let _ = line.hit_test_one_series(0, x, y);
    }
    // mid-drag state: scrolled past the right edge into empty space, then hit far right
    let mut chart = settled_candle_chart();
    chart.set_right_offset(100.0);
    chart.build_frame();
    let _ = chart.hit_test_series(10_000.0, 250.0);
    let _ = chart.hit_test_series(250.0, 250.0);
    // scrolled far past the left edge (negative visible range)
    chart.set_right_offset(-100.0);
    chart.build_frame();
    let _ = chart.hit_test_series(250.0, 250.0);
    let _ = chart.hit_test_series(10_000.0, 250.0);
}
