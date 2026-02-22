//! Built-in studies (SMA, EMA, RSI, MACD, Bollinger, Stochastic, ATR, VWAP)

pub mod atr;
pub mod bollinger;
pub mod ema;
pub mod macd;
pub mod rsi;
pub mod sma;
pub mod stochastic;
pub mod vwap;

pub use atr::AtrCalculator;
pub use bollinger::BollingerCalculator;
pub use ema::EmaCalculator;
pub use macd::MacdCalculator;
pub use rsi::RsiCalculator;
pub use sma::SmaCalculator;
pub use stochastic::StochasticCalculator;
pub use vwap::VwapCalculator;

/// Register all built-in study calculators with the study manager.
pub fn register_built_in_studies(manager: &mut crate::core::studies::manager::StudyManager) {
    manager.register_calculator(Box::new(SmaCalculator));
    manager.register_calculator(Box::new(EmaCalculator));
    manager.register_calculator(Box::new(RsiCalculator));
    manager.register_calculator(Box::new(MacdCalculator));
    manager.register_calculator(Box::new(BollingerCalculator));
    manager.register_calculator(Box::new(StochasticCalculator));
    manager.register_calculator(Box::new(AtrCalculator));
    manager.register_calculator(Box::new(VwapCalculator));
}
