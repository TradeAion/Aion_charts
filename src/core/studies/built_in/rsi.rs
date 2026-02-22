//! Relative Strength Index (RSI) study.

use crate::core::data::BarArray;
use crate::core::studies::manager::{Study, StudyCalculator};

/// RSI calculator.
pub struct RsiCalculator;

impl StudyCalculator for RsiCalculator {
    fn name(&self) -> &str {
        "rsi"
    }

    fn calculate(&self, study: &mut Study, bars: &BarArray, start_index: usize, end_index: usize) {
        let period = study.parameters.get("period").copied().unwrap_or(14.0) as usize;
        if period == 0 {
            return;
        }

        let output = study.get_output_mut(0);
        if output.is_none() {
            return;
        }
        let output = output.unwrap();

        // Ensure output data array is large enough
        if output.data.timestamps.len() < bars.len() {
            output.data.timestamps.resize(bars.len(), 0);
            output.data.values.resize(bars.len(), 0.0);
        }

        // Calculate price changes
        let mut gains = Vec::with_capacity(bars.len());
        let mut losses = Vec::with_capacity(bars.len());

        for i in 0..bars.len() {
            if i == 0 {
                gains.push(0.0);
                losses.push(0.0);
            } else {
                let change = bars.close(i) - bars.close(i - 1);
                if change > 0.0 {
                    gains.push(change as f64);
                    losses.push(0.0);
                } else {
                    gains.push(0.0);
                    losses.push((-change) as f64);
                }
            }
        }

        // Calculate RSI for each bar from start_index to end_index
        for i in start_index..=end_index {
            if i + 1 < period {
                // Not enough data
                output.data.timestamps[i] = bars.timestamp(i);
                output.data.values[i] = f32::NAN;
                continue;
            }

            // Calculate average gain and loss over the period
            let mut avg_gain = 0.0;
            let mut avg_loss = 0.0;

            if i + 1 == period {
                // First RSI calculation - simple average
                for j in 1..=period {
                    avg_gain += gains[j];
                    avg_loss += losses[j];
                }
                avg_gain /= period as f64;
                avg_loss /= period as f64;
            } else {
                // Subsequent calculations - smoothed average
                let prev_avg_gain = if i > 0 && !output.data.values[i - 1].is_nan() {
                    // We need to reconstruct previous averages
                    // This is a simplified approach - in practice you'd store these
                    let mut pg = 0.0;
                    for j in (i - period + 1)..=i {
                        pg += gains[j];
                    }
                    pg / period as f64
                } else {
                    0.0
                };

                let prev_avg_loss = if i > 0 && !output.data.values[i - 1].is_nan() {
                    let mut pl = 0.0;
                    for j in (i - period + 1)..=i {
                        pl += losses[j];
                    }
                    pl / period as f64
                } else {
                    0.0
                };

                avg_gain = (prev_avg_gain * (period as f64 - 1.0) + gains[i]) / period as f64;
                avg_loss = (prev_avg_loss * (period as f64 - 1.0) + losses[i]) / period as f64;
            }

            // Calculate RS and RSI
            let rs = if avg_loss == 0.0 {
                100.0 // Max RSI when no losses
            } else {
                avg_gain / avg_loss
            };

            let rsi = 100.0 - (100.0 / (1.0 + rs));

            output.data.timestamps[i] = bars.timestamp(i);
            output.data.values[i] = rsi as f32;
        }
    }
}
