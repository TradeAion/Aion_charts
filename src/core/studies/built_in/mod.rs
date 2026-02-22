//! Built-in studies (SMA, EMA, RSI, MACD, etc.)

pub mod sma;
pub mod ema;
pub mod rsi;
pub mod macd;

pub use sma::SmaCalculator;
pub use ema::EmaCalculator;
pub use rsi::RsiCalculator;
pub use macd::MacdCalculator;

/// Register all built-in study calculators with the study manager.
pub fn register_built_in_studies(manager: &mut crate::core::studies::manager::StudyManager) {
    manager.register_calculator(Box::new(SmaCalculator));
    manager.register_calculator(Box::new(EmaCalculator));
    manager.register_calculator(Box::new(RsiCalculator));
    manager.register_calculator(Box::new(MacdCalculator));
}
