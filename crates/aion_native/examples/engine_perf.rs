use std::time::Instant;

use aion_engine::ChartEngine;

fn main() {
    const BARS: usize = 50_000;
    let mut times = Vec::with_capacity(BARS);
    let mut open = Vec::with_capacity(BARS);
    let mut high = Vec::with_capacity(BARS);
    let mut low = Vec::with_capacity(BARS);
    let mut close = Vec::with_capacity(BARS);
    let mut price = 100.0;
    for i in 0..BARS {
        let wobble = ((i as f64) * 0.017).sin() * 0.8;
        let next = price + wobble;
        times.push(i as f64);
        open.push(price);
        high.push(price.max(next) + 0.4);
        low.push(price.min(next) - 0.4);
        close.push(next);
        price = next;
    }

    let mut chart = ChartEngine::new(1600.0, 800.0, 1.0);
    chart.set_series_data(0, &times, &open, &high, &low, &close).expect("valid fixture");
    chart.time_scale.set_width(1600.0);
    chart.fit_content();
    let _sma = chart.add_sma(0, 20).expect("valid indicator");
    let update_start = Instant::now();
    for i in 0..1_000 {
        let value = price + (i as f64 * 0.01).sin();
        chart.update_series_bar(0, (BARS + i) as f64, [value, value + 0.4, value - 0.4, value]);
    }
    let update_elapsed = update_start.elapsed();
    println!("1,000 streaming updates with SMA: {:?} ({:.3} µs/update)", update_elapsed, update_elapsed.as_secs_f64() * 1_000_000.0 / 1_000.0);
    let mut frame = aion_engine::ChartFrame::default();
    let start = Instant::now();
    for _ in 0..10 {
        chart.build_frame_into(&mut frame);
    }
    let elapsed = start.elapsed();
    println!("{BARS} bars: 10 retained frame builds in {:?} ({:.2} ms/frame)", elapsed, elapsed.as_secs_f64() * 100.0);
}
