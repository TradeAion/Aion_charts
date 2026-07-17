//! Deterministic real-engine fixture used by native golden tests and examples.

use aion_engine::ChartEngine;

pub fn demo_engine() -> ChartEngine {
    let mut chart = ChartEngine::new(480.0, 300.0, 1.0);
    let mut times = Vec::with_capacity(64);
    let mut open = Vec::with_capacity(64);
    let mut high = Vec::with_capacity(64);
    let mut low = Vec::with_capacity(64);
    let mut close = Vec::with_capacity(64);
    let mut price = 100.0;
    for i in 0..64 {
        let o = price;
        let delta = ((i * 17 % 11) as f64 - 5.0) * 0.22;
        let c = (o + delta).max(1.0);
        times.push(i as f64);
        open.push(o);
        high.push(o.max(c) + 0.7 + (i % 3) as f64 * 0.1);
        low.push(o.min(c) - 0.7 - (i % 2) as f64 * 0.1);
        close.push(c);
        price = c;
    }
    chart.set_series_data(0, &times, &open, &high, &low, &close).expect("fixture data is valid");
    chart.time_scale.set_width(480.0);
    chart.fit_content();
    chart
}
