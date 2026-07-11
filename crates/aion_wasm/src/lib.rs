//! aion_wasm — browser host shell.
//!
//! Phase 0: a real chart pipeline end to end — `aion_core` scales -> candle geometry ->
//! WebGPU quads. The JS side (packages/charts, examples/web_demo) owns DOM events and calls
//! the exported gesture methods; rendering happens on demand via `render()`.

#[cfg(target_arch = "wasm32")]
mod chart;
#[cfg(target_arch = "wasm32")]
mod text;

#[cfg(target_arch = "wasm32")]
pub use chart::{create_chart, AionChart};
