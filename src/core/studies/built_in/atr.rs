//! Average True Range (ATR) study.
//!
//! Outputs:
//! - 0: ATR value
//!
//! Parameters:
//! - period: ATR period (default 14)
//!
//! True Range = max(high - low, |high - prev_close|, |low - prev_close|)
//! ATR = SMA or EMA of True Range

use crate::core::data::BarArray;
use crate::core::studies::manager::{Study, StudyCalculator};

/// ATR calculator.
pub struct AtrCalculator;

impl StudyCalculator for AtrCalculator {
    fn name(&self) -> &str {
        "atr"
    }

    fn calculate(&self, study: &mut Study, bars: &BarArray, start_index: usize, end_index: usize) {
        let period = study.parameters.get("period").copied().unwrap_or(14.0) as usize;

        if period == 0 {
            return;
        }

        let bar_len = bars.len();

        // Ensure output has enough space
        if let Some(output) = study.get_output_mut(0) {
            if output.data.timestamps.len() < bar_len {
                output.data.timestamps.resize(bar_len, 0);
                output.data.values.resize(bar_len, 0.0);
            }
        }

        // Calculate True Range for each bar
        let mut true_ranges: Vec<f64> = vec![0.0; bar_len];

        for i in 0..bar_len {
            let high = bars.high(i) as f64;
            let low = bars.low(i) as f64;

            if i == 0 {
                // First bar: TR = high - low
                true_ranges[i] = high - low;
            } else {
                let prev_close = bars.close(i - 1) as f64;
                let tr1 = high - low;
                let tr2 = (high - prev_close).abs();
                let tr3 = (low - prev_close).abs();
                true_ranges[i] = tr1.max(tr2).max(tr3);
            }
        }

        // Calculate ATR using Wilder's smoothing (similar to EMA with alpha = 1/period)
        let mut atr = 0.0;
        let alpha = 1.0 / period as f64;

        for i in start_index..=end_index {
            let ts = bars.timestamp(i);

            if i + 1 < period {
                // Not enough data - use simple average of available TR
                let mut sum = 0.0;
                for j in 0..=i {
                    sum += true_ranges[j];
                }
                atr = sum / (i + 1) as f64;

                if let Some(output) = study.get_output_mut(0) {
                    output.data.timestamps[i] = ts;
                    output.data.values[i] = f32::NAN; // Don't show until we have full period
                }
                continue;
            }

            if i + 1 == period {
                // First ATR: simple average of first 'period' true ranges
                let mut sum = 0.0;
                for j in 0..period {
                    sum += true_ranges[j];
                }
                atr = sum / period as f64;
            } else {
                // Wilder's smoothing: ATR = prev_ATR * (1 - alpha) + TR * alpha
                // Equivalent to: ATR = (prev_ATR * (period - 1) + TR) / period
                atr = atr * (1.0 - alpha) + true_ranges[i] * alpha;
            }

            if let Some(output) = study.get_output_mut(0) {
                output.data.timestamps[i] = ts;
                output.data.values[i] = atr as f32;
            }
        }
    }
}
