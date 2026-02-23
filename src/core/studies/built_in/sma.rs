//! Simple Moving Average (SMA) study.

use crate::core::data::BarArray;
use crate::core::studies::manager::{Study, StudyCalculator};

/// SMA calculator.
pub struct SmaCalculator;

impl StudyCalculator for SmaCalculator {
    fn name(&self) -> &str {
        "sma"
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

        // Calculate SMA for each bar from start_index to end_index
        for i in start_index..=end_index {
            if i + 1 < period {
                // Not enough data for this period
                output.data.timestamps[i] = bars.timestamp(i);
                output.data.values[i] = f32::NAN;
                continue;
            }

            // Calculate average of last 'period' closes
            let mut sum = 0.0;
            for j in (i + 1 - period)..=i {
                sum += bars.close(j) as f64;
            }
            let avg = sum / period as f64;

            output.data.timestamps[i] = bars.timestamp(i);
            output.data.values[i] = avg as f32;
        }
    }
}
