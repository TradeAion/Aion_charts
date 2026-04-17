//! Exponential Moving Average (EMA) study.

use crate::core::data::BarArray;
use crate::core::studies::manager::{Study, StudyCalculator};

/// EMA calculator.
pub struct EmaCalculator;

impl StudyCalculator for EmaCalculator {
    fn name(&self) -> &str {
        "ema"
    }

    fn calculate(&self, study: &mut Study, bars: &BarArray, start_index: usize, end_index: usize) {
        let period = study.parameters.get("period").copied().unwrap_or(20.0) as usize;
        if period == 0 {
            return;
        }

        let Some(output) = study.get_output_mut(0) else {
            return;
        };

        // Ensure output data array is large enough
        if output.data.timestamps.len() < bars.len() {
            output.data.timestamps.resize(bars.len(), 0);
            output.data.values.resize(bars.len(), 0.0);
        }

        // Multiplier for EMA calculation
        let multiplier = 2.0 / (period as f64 + 1.0);

        // Calculate EMA for each bar from start_index to end_index
        for i in start_index..=end_index {
            if i == 0 {
                // First value is just the close price
                output.data.timestamps[i] = bars.timestamp(i);
                output.data.values[i] = bars.close(i);
                continue;
            }

            let prev_ema = if i > 0 && !output.data.values[i - 1].is_nan() {
                output.data.values[i - 1] as f64
            } else {
                // If previous EMA is NaN, use SMA for initialization
                let mut sum = 0.0;
                let count = (period.min(i + 1)) as f64;
                for j in 0..(count as usize) {
                    sum += bars.close(i - j) as f64;
                }
                sum / count
            };

            let current_close = bars.close(i) as f64;
            let ema = (current_close - prev_ema) * multiplier + prev_ema;

            output.data.timestamps[i] = bars.timestamp(i);
            output.data.values[i] = ema;
        }
    }
}
