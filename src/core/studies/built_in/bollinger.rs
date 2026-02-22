//! Bollinger Bands study.
//!
//! Outputs:
//! - 0: Middle band (SMA)
//! - 1: Upper band (SMA + k * stddev)
//! - 2: Lower band (SMA - k * stddev)
//!
//! Parameters:
//! - period: SMA period (default 20)
//! - stddev: Standard deviation multiplier (default 2.0)

use crate::core::data::BarArray;
use crate::core::studies::manager::{Study, StudyCalculator};

/// Bollinger Bands calculator.
pub struct BollingerCalculator;

impl StudyCalculator for BollingerCalculator {
    fn name(&self) -> &str {
        "bollinger"
    }

    fn calculate(&self, study: &mut Study, bars: &BarArray, start_index: usize, end_index: usize) {
        let period = study.parameters.get("period").copied().unwrap_or(20.0) as usize;
        let k = study.parameters.get("stddev").copied().unwrap_or(2.0);

        if period == 0 {
            return;
        }

        // Get all three outputs
        let bar_len = bars.len();

        // Ensure all outputs have enough space
        for out_idx in 0..3 {
            if let Some(output) = study.get_output_mut(out_idx) {
                if output.data.timestamps.len() < bar_len {
                    output.data.timestamps.resize(bar_len, 0);
                    output.data.values.resize(bar_len, 0.0);
                }
            }
        }

        // Calculate Bollinger Bands for each bar
        for i in start_index..=end_index {
            let ts = bars.timestamp(i);

            if i + 1 < period {
                // Not enough data
                for out_idx in 0..3 {
                    if let Some(output) = study.get_output_mut(out_idx) {
                        output.data.timestamps[i] = ts;
                        output.data.values[i] = f32::NAN;
                    }
                }
                continue;
            }

            // Calculate SMA
            let mut sum = 0.0;
            for j in (i + 1 - period)..=i {
                sum += bars.close(j) as f64;
            }
            let sma = sum / period as f64;

            // Calculate standard deviation
            let mut variance_sum = 0.0;
            for j in (i + 1 - period)..=i {
                let diff = bars.close(j) as f64 - sma;
                variance_sum += diff * diff;
            }
            let stddev = (variance_sum / period as f64).sqrt();

            // Upper and lower bands
            let upper = sma + k * stddev;
            let lower = sma - k * stddev;

            // Set outputs
            if let Some(output) = study.get_output_mut(0) {
                output.data.timestamps[i] = ts;
                output.data.values[i] = sma as f32;
            }
            if let Some(output) = study.get_output_mut(1) {
                output.data.timestamps[i] = ts;
                output.data.values[i] = upper as f32;
            }
            if let Some(output) = study.get_output_mut(2) {
                output.data.timestamps[i] = ts;
                output.data.values[i] = lower as f32;
            }
        }
    }
}
