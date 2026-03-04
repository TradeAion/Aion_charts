//! DemoMode — built-in sample OHLCV data generator.
//!
//! Generates realistic-looking candlestick data for demo/testing purposes.
//! Replaces the JS sample data generator that was previously in index.html.

use crate::core::data::Bar;
use crate::core::footprint::{FootprintBar, FootprintData, FootprintLevel};

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

/// Generate synthetic footprint data from existing OHLCV bars.
///
/// Distributes the bar's total volume across price levels between low and high,
/// with realistic bid/ask splits that favor the bar's direction.
/// This is useful for demo/testing purposes when real tick-level data is not available.
///
/// `tick_size`: the price increment per footprint row. If 0.0, auto-calculated
/// from the average bar range.
pub fn generate_footprint_from_bars(bars: &[Bar], tick_size: f32) -> FootprintData {
    let mut fp_data = FootprintData::new();
    if bars.is_empty() {
        return fp_data;
    }

    // Auto-calculate tick size from average bar range if not provided
    let tick = if tick_size > 0.0 {
        tick_size
    } else {
        let avg_range: f32 = bars.iter().map(|b| b.high - b.low).sum::<f32>() / bars.len() as f32;
        // Target ~8-15 levels per bar
        let raw = avg_range / 10.0;
        // Round to a nice number
        round_tick_size(raw)
    };

    let mut seed: u64 = 54321;

    for (i, bar) in bars.iter().enumerate() {
        let low = bar.low;
        let high = bar.high;
        let range = high - low;
        if range <= 0.0 || tick <= 0.0 {
            continue;
        }

        let bull = bar.close >= bar.open;
        let total_vol = bar.volume.max(1.0);

        // Generate price levels from low to high
        let level_low = (low / tick).floor() * tick;
        let level_high = (high / tick).ceil() * tick;
        let num_levels = ((level_high - level_low) / tick).round() as usize;
        let num_levels = num_levels.max(1).min(50); // Clamp to avoid pathological cases

        let mut levels = Vec::with_capacity(num_levels);

        // Distribute volume across levels using a bell-curve centered on POC
        // POC tends to be near VWAP which is roughly at the volume-weighted mid
        let poc_price = if bull {
            bar.open + (bar.close - bar.open) * 0.4
        } else {
            bar.close + (bar.open - bar.close) * 0.4
        };

        let mut vol_weights: Vec<f64> = Vec::with_capacity(num_levels);
        let mut weight_sum = 0.0;

        for j in 0..num_levels {
            let price = level_low + j as f32 * tick;
            let dist = ((price + tick * 0.5 - poc_price) / range).abs() as f64;
            // Bell curve: higher weight near POC
            let w = (-dist * dist * 4.0).exp() + 0.05;
            vol_weights.push(w);
            weight_sum += w;
        }

        for j in 0..num_levels {
            let price = level_low + j as f32 * tick;
            let vol_frac = (vol_weights[j] / weight_sum) as f32;
            let level_vol = total_vol * vol_frac;

            // Bid/ask split depends on bar direction and position within the bar
            seed = lcg_next(seed);
            let noise = lcg_f64(seed) as f32 * 0.3; // ±15% noise

            let position_frac = if range > 0.0 {
                ((price - low) / range).clamp(0.0, 1.0)
            } else {
                0.5
            };

            // In a bullish bar, lower levels tend to have more bids (selling absorbed),
            // upper levels have more asks (buying pressure).
            // In a bearish bar, the opposite.
            let ask_ratio = if bull {
                0.3 + position_frac * 0.4 + noise
            } else {
                0.7 - position_frac * 0.4 + noise
            };
            let ask_ratio = ask_ratio.clamp(0.05, 0.95);

            let ask_vol = level_vol * ask_ratio;
            let bid_vol = level_vol * (1.0 - ask_ratio);

            levels.push(FootprintLevel {
                price,
                bid_volume: bid_vol.max(0.0),
                ask_volume: ask_vol.max(0.0),
            });
        }

        // Sort by price ascending (should already be, but ensure)
        levels.sort_by(|a, b| {
            a.price
                .partial_cmp(&b.price)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        fp_data.set_bar(i, FootprintBar { levels });
    }

    fp_data
}

/// Round a raw tick size to a "nice" number (1, 2, 5, 10, 25, 50, 100, etc.)
fn round_tick_size(raw: f32) -> f32 {
    if raw <= 0.0 {
        return 1.0;
    }

    let magnitude = 10.0_f32.powf(raw.log10().floor());
    let normalized = raw / magnitude;

    let nice = if normalized < 1.5 {
        1.0
    } else if normalized < 3.5 {
        2.5
    } else if normalized < 7.5 {
        5.0
    } else {
        10.0
    };

    (nice * magnitude).max(f32::EPSILON)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_tick_size() {
        assert_eq!(round_tick_size(0.3), 0.25);
        assert_eq!(round_tick_size(0.8), 1.0);
        assert_eq!(round_tick_size(3.0), 2.5);
        assert_eq!(round_tick_size(7.0), 5.0);
        assert_eq!(round_tick_size(15.0), 25.0);
        assert_eq!(round_tick_size(80.0), 100.0);
    }

    #[test]
    fn test_generate_footprint_from_bars() {
        let bars = generate_sample_data(10, 1000, 60_000);
        let fp = generate_footprint_from_bars(&bars, 0.0);

        assert_eq!(fp.len(), 10);
        for i in 0..10 {
            let bar = fp.get_bar(i).unwrap();
            assert!(!bar.levels.is_empty());
            assert!(bar.total_volume() > 0.0);
            // Levels should be sorted by price
            for j in 1..bar.levels.len() {
                assert!(bar.levels[j].price >= bar.levels[j - 1].price);
            }
        }
    }

    #[test]
    fn test_generate_footprint_with_explicit_tick() {
        let bars = generate_sample_data(5, 1000, 60_000);
        let fp = generate_footprint_from_bars(&bars, 50.0);

        assert_eq!(fp.len(), 5);
        for i in 0..5 {
            let bar = fp.get_bar(i).unwrap();
            // With explicit tick size, levels should be spaced by that tick
            if bar.levels.len() >= 2 {
                let diff = bar.levels[1].price - bar.levels[0].price;
                assert!((diff - 50.0).abs() < 0.01);
            }
        }
    }
}
