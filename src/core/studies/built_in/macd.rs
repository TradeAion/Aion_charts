//! MACD (Moving Average Convergence Divergence) study.

use crate::core::data::BarArray;
use crate::core::studies::manager::{Study, StudyCalculator};

/// MACD calculator.
pub struct MacdCalculator;

impl StudyCalculator for MacdCalculator {
    fn name(&self) -> &str {
        "macd"
    }

    fn calculate(&self, study: &mut Study, bars: &BarArray, start_index: usize, end_index: usize) {
        let fast_period = study.parameters.get("fast_period").copied().unwrap_or(12.0) as usize;
        let slow_period = study.parameters.get("slow_period").copied().unwrap_or(26.0) as usize;
        let signal_period = study
            .parameters
            .get("signal_period")
            .copied()
            .unwrap_or(9.0) as usize;

        if fast_period == 0 || slow_period == 0 || signal_period == 0 {
            return;
        }

        // Verify all 3 outputs exist
        if study.outputs.len() < 3 {
            return;
        }

        // Ensure output data arrays are large enough
        let len = bars.len();
        for idx in 0..3 {
            if study.outputs[idx].data.timestamps.len() < len {
                study.outputs[idx].data.timestamps.resize(len, 0);
                study.outputs[idx].data.values.resize(len, 0.0);
            }
        }

        // Calculate EMAs for fast and slow periods
        let mut fast_ema = vec![0.0; len];
        let mut slow_ema = vec![0.0; len];

        // Multipliers for EMA calculations
        let fast_multiplier = 2.0 / (fast_period as f64 + 1.0);
        let slow_multiplier = 2.0 / (slow_period as f64 + 1.0);
        let signal_multiplier = 2.0 / (signal_period as f64 + 1.0);

        // Calculate fast EMA
        for i in 0..len {
            if i == 0 {
                fast_ema[i] = bars.close(i) as f64;
            } else {
                let prev_fast = fast_ema[i - 1];
                let current_close = bars.close(i) as f64;
                fast_ema[i] = (current_close - prev_fast) * fast_multiplier + prev_fast;
            }
        }

        // Calculate slow EMA
        for i in 0..len {
            if i == 0 {
                slow_ema[i] = bars.close(i) as f64;
            } else {
                let prev_slow = slow_ema[i - 1];
                let current_close = bars.close(i) as f64;
                slow_ema[i] = (current_close - prev_slow) * slow_multiplier + prev_slow;
            }
        }

        // Calculate MACD line (fast EMA - slow EMA)
        for i in start_index..=end_index {
            let macd_value = fast_ema[i] - slow_ema[i];
            study.outputs[0].data.timestamps[i] = bars.timestamp(i);
            study.outputs[0].data.values[i] = macd_value as f32;
        }

        // Calculate signal line (EMA of MACD)
        for i in start_index..=end_index {
            if i == 0 {
                study.outputs[1].data.timestamps[i] = bars.timestamp(i);
                study.outputs[1].data.values[i] = study.outputs[0].data.values[i];
                continue;
            }

            let prev_signal = study.outputs[1].data.values[i - 1] as f64;
            let current_macd = study.outputs[0].data.values[i] as f64;
            let signal_value = (current_macd - prev_signal) * signal_multiplier + prev_signal;

            study.outputs[1].data.timestamps[i] = bars.timestamp(i);
            study.outputs[1].data.values[i] = signal_value as f32;
        }

        // Calculate histogram (MACD - Signal)
        for i in start_index..=end_index {
            let histogram_value = study.outputs[0].data.values[i] - study.outputs[1].data.values[i];
            study.outputs[2].data.timestamps[i] = bars.timestamp(i);
            study.outputs[2].data.values[i] = histogram_value;
        }
    }
}
