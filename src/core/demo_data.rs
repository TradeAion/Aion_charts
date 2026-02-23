//! DemoMode — built-in sample OHLCV data generator.
//!
//! Generates realistic-looking candlestick data for demo/testing purposes.
//! Replaces the JS sample data generator that was previously in index.html.

use crate::core::data::Bar;

/// Generate `n` sample OHLCV bars starting from a given timestamp (epoch ms).
/// Uses a simple random walk with momentum for realistic-looking price action.
pub fn generate_sample_data(n: usize, start_ms: u64, interval_ms: u64) -> Vec<Bar> {
    let mut bars = Vec::with_capacity(n);
    let mut price: f64 = 42000.0;
    // Simple pseudo-random (deterministic for reproducibility, seeded by n)
    let mut seed: u64 = 12345 ^ (n as u64).wrapping_mul(67890);

    for i in 0..n {
        seed = lcg_next(seed);
        let r1 = lcg_f64(seed);
        seed = lcg_next(seed);
        let r2 = lcg_f64(seed);
        seed = lcg_next(seed);
        let r3 = lcg_f64(seed);
        seed = lcg_next(seed);
        let r4 = lcg_f64(seed);

        // Slight bullish bias (0.48 → slightly more ups than downs)
        let change = (r1 - 0.48) * price * 0.012;
        let o = price;
        let c = price + change;
        let h = o.max(c) + r2 * price * 0.005;
        let l = o.min(c) - r3 * price * 0.005;
        let vol = 200.0 + r4 * 4000.0;

        bars.push(Bar {
            timestamp: start_ms + (i as u64) * interval_ms,
            open: o as f32,
            high: h as f32,
            low: l as f32,
            close: c as f32,
            volume: vol as f32,
            _pad: 0.0,
        });

        price = c;
    }

    bars
}

/// Linear congruential generator — fast, deterministic pseudo-random.
#[inline]
fn lcg_next(seed: u64) -> u64 {
    seed.wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407)
}

/// Convert LCG state to f64 in [0, 1).
#[inline]
fn lcg_f64(seed: u64) -> f64 {
    (seed >> 11) as f64 / (1u64 << 53) as f64
}
