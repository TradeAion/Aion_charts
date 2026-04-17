//! Stochastic Oscillator study.
//!
//! Outputs:
//! - 0: %K (fast stochastic)
//! - 1: %D (slow stochastic, SMA of %K)
//!
//! Parameters:
//! - k_period: %K period (default 14)
//! - d_period: %D smoothing period (default 3)
//! - slowing: %K slowing period (default 3)

use crate::core::data::BarArray;
use crate::core::studies::manager::{Study, StudyCalculator};

/// Stochastic Oscillator calculator.
pub struct StochasticCalculator;

impl StudyCalculator for StochasticCalculator {
    fn name(&self) -> &str {
        "stochastic"
    }

    fn calculate(&self, study: &mut Study, bars: &BarArray, start_index: usize, end_index: usize) {
        let k_period = study.parameters.get("k_period").copied().unwrap_or(14.0) as usize;
        let d_period = study.parameters.get("d_period").copied().unwrap_or(3.0) as usize;
        let slowing = study.parameters.get("slowing").copied().unwrap_or(3.0) as usize;

        if k_period == 0 || d_period == 0 || slowing == 0 {
            return;
        }

        let bar_len = bars.len();

        // Ensure outputs have enough space
        for out_idx in 0..2 {
            if let Some(output) = study.get_output_mut(out_idx) {
                if output.data.timestamps.len() < bar_len {
                    output.data.timestamps.resize(bar_len, 0);
                    output.data.values.resize(bar_len, 0.0);
                }
            }
        }

        // Temporary storage for raw %K values (before slowing)
        let mut raw_k: Vec<f64> = vec![f64::NAN; bar_len];

        // Calculate raw %K for each bar
        for i in 0..bar_len {
            if i + 1 < k_period {
                continue;
            }

            // Find highest high and lowest low in the period
            let mut highest = f64::MIN;
            let mut lowest = f64::MAX;
            for j in (i + 1 - k_period)..=i {
                let high = bars.high(j) as f64;
                let low = bars.low(j) as f64;
                if high > highest {
                    highest = high;
                }
                if low < lowest {
                    lowest = low;
                }
            }

            let close = bars.close(i) as f64;
            let range = highest - lowest;

            if range > 0.0 {
                raw_k[i] = 100.0 * (close - lowest) / range;
            } else {
                raw_k[i] = 50.0; // Default to middle if no range
            }
        }

        // Calculate %K with slowing (SMA of raw %K)
        let mut k_values: Vec<f64> = vec![f64::NAN; bar_len];
        for i in start_index..=end_index {
            let ts = bars.timestamp(i);

            if i + 1 < k_period + slowing - 1 {
                if let Some(output) = study.get_output_mut(0) {
                    output.data.timestamps[i] = ts;
                    output.data.values[i] = f64::NAN;
                }
                continue;
            }

            // SMA of raw %K over slowing period
            let mut sum = 0.0;
            let mut count = 0;
            for j in (i + 1 - slowing)..=i {
                if !raw_k[j].is_nan() {
                    sum += raw_k[j];
                    count += 1;
                }
            }

            if count > 0 {
                k_values[i] = sum / count as f64;
                if let Some(output) = study.get_output_mut(0) {
                    output.data.timestamps[i] = ts;
                    output.data.values[i] = k_values[i];
                }
            } else {
                if let Some(output) = study.get_output_mut(0) {
                    output.data.timestamps[i] = ts;
                    output.data.values[i] = f64::NAN;
                }
            }
        }

        // Calculate %D (SMA of %K)
        for i in start_index..=end_index {
            let ts = bars.timestamp(i);

            if i + 1 < k_period + slowing + d_period - 2 {
                if let Some(output) = study.get_output_mut(1) {
                    output.data.timestamps[i] = ts;
                    output.data.values[i] = f64::NAN;
                }
                continue;
            }

            // SMA of %K over d_period
            let mut sum = 0.0;
            let mut count = 0;
            for j in (i + 1 - d_period)..=i {
                if !k_values[j].is_nan() {
                    sum += k_values[j];
                    count += 1;
                }
            }

            if count > 0 {
                if let Some(output) = study.get_output_mut(1) {
                    output.data.timestamps[i] = ts;
                    output.data.values[i] = sum / count as f64;
                }
            } else {
                if let Some(output) = study.get_output_mut(1) {
                    output.data.timestamps[i] = ts;
                    output.data.values[i] = f64::NAN;
                }
            }
        }
    }
}
