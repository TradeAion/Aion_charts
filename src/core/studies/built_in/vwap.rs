//! Volume Weighted Average Price (VWAP) study.
//!
//! Outputs:
//! - 0: VWAP value
//!
//! Parameters:
//! - anchor: Reset anchor ("session", "week", "month", "year", "none")
//!           Default is "none" (cumulative from first bar)
//!
//! VWAP = Cumulative(Typical Price * Volume) / Cumulative(Volume)
//! Typical Price = (High + Low + Close) / 3

use crate::core::data::BarArray;
use crate::core::studies::manager::{Study, StudyCalculator};

/// VWAP calculator.
pub struct VwapCalculator;

impl StudyCalculator for VwapCalculator {
    fn name(&self) -> &str {
        "vwap"
    }

    fn calculate(&self, study: &mut Study, bars: &BarArray, start_index: usize, end_index: usize) {
        let bar_len = bars.len();

        // Ensure output has enough space
        if let Some(output) = study.get_output_mut(0) {
            if output.data.timestamps.len() < bar_len {
                output.data.timestamps.resize(bar_len, 0);
                output.data.values.resize(bar_len, 0.0);
            }
        }

        // For simplicity, we'll use cumulative VWAP from bar 0
        // (A more complete implementation would support session/day anchoring)
        let mut cum_tp_vol = 0.0; // Cumulative (Typical Price * Volume)
        let mut cum_vol = 0.0; // Cumulative Volume

        // We need to calculate from the beginning to maintain cumulative state
        // But only write outputs for start_index..=end_index
        let calc_start = 0;

        for i in calc_start..=end_index {
            let high = bars.high(i) as f64;
            let low = bars.low(i) as f64;
            let close = bars.close(i) as f64;
            let volume = bars.volume(i) as f64;

            // Typical price
            let tp = (high + low + close) / 3.0;

            // Accumulate
            cum_tp_vol += tp * volume;
            cum_vol += volume;

            // Only write output for requested range
            if i >= start_index {
                let ts = bars.timestamp(i);
                let vwap = if cum_vol > 0.0 {
                    cum_tp_vol / cum_vol
                } else {
                    close // Fallback to close if no volume
                };

                if let Some(output) = study.get_output_mut(0) {
                    output.data.timestamps[i] = ts;
                    output.data.values[i] = vwap;
                }
            }
        }
    }
}
