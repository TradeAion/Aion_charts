//! # RayCore — High-Performance GPU-Accelerated Charting Engine
//!
//! RayCore is a Rust/WebAssembly financial charting library designed for
//! professional-grade performance and visual quality. It uses WebGPU for
//! hardware-accelerated rendering on both native and web platforms.
//!
//! ## Architecture Overview
//!
//! RayCore follows a widget-based architecture similar to TradingView's
//! Lightweight Charts (LWC):
//!
//! - **Pane System**: Multiple panes with independent price scales
//! - **Layered Rendering**: Separate base (GPU) and overlay (Canvas2D) layers
//! - **Widget Components**: Price axis, time axis, and chart pane as separate DOM elements
//! - **Shared State**: Unified viewport and data management across all widgets
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use raycore::{Bar, BarArray, Viewport, ChartEngine};
//!
//! // Create a bar array and populate with data
//! let mut bars = BarArray::new();
//! bars.set(vec![
//!     Bar { timestamp: 1700000000000, open: 100.0, high: 105.0, low: 98.0, close: 103.0, volume: 1000.0, _pad: 0.0 },
//!     Bar { timestamp: 1700000060000, open: 103.0, high: 108.0, low: 101.0, close: 106.0, volume: 1200.0, _pad: 0.0 },
//! ]);
//!
//! // Create a viewport
//! let mut viewport = Viewport::new(800, 600);
//! viewport.set_range(0.0, 100.0);
//! viewport.auto_fit_price(&bars);
//! ```
//!
//! ## Module Structure
//!
//! - [`core::data`] — OHLCV bar storage with O(1) append operations
//! - [`core::viewport`] — Coordinate transformations and price scaling
//! - [`core::engine`] — Central chart engine coordinating all components
//! - [`core::renderer`] — GPU and Canvas2D rendering backends
//! - [`core::interaction`] — Mouse/touch event handling
//! - [`core::studies`] — Technical indicators (SMA, EMA, RSI, MACD)
//! - [`core::drawings`] — User annotations (trend lines, rays, etc.)
//! - [`core::series`] — Multiple chart series types
//!
//! ## Performance Characteristics
//!
//! | Operation | Complexity | Notes |
//! |-----------|------------|-------|
//! | `BarArray::append` | O(1) | Uses pending buffer pattern |
//! | `BarArray::set` | O(n) | Bulk data load |
//! | `Viewport::zoom` | O(1) | Sub-microsecond |
//! | `Viewport::auto_fit_price` | O(k) | k = visible bars |
//! | GPU render pass | O(n) | n = visible candles |
//!
//! ## Feature Flags
//!
//! - `wasm32`: Enables WebAssembly-specific features (Canvas2D, web-sys bindings)
//!
//! ## Platform Support
//!
//! - **Native**: Windows, macOS, Linux via wgpu's native backends
//! - **Web**: Chrome, Firefox, Safari via WebGPU (with Canvas2D fallback)

pub mod core;

// ═══════════════════════════════════════════════════════════════════════════════
// Re-exports: Core Data Types
// ═══════════════════════════════════════════════════════════════════════════════

/// OHLCV bar structure and columnar storage.
pub use crate::core::data::{Bar, BarArray};

/// Viewport state and coordinate transformations.
pub use crate::core::viewport::{PriceScaleMode, Viewport};

/// Central chart engine.
pub use crate::core::engine::ChartEngine;

/// Main chart type (candlestick, OHLC bars, line, area, etc.).
pub use crate::core::chart_type::{MainChartOptions, MainChartType};

// ═══════════════════════════════════════════════════════════════════════════════
// Re-exports: Rendering
// ═══════════════════════════════════════════════════════════════════════════════

/// Rendering traits and abstractions.
pub use crate::core::renderer::traits::{
    ChartRenderer, ChartStyle, CrosshairMode, CrosshairState, RenderContext, Renderer,
    RendererBackend, TickMark,
};

/// Candle sizing strategies.
pub use crate::core::renderer::series::CandleSizing;

/// Draw list for batched rendering.
pub use crate::core::renderer::draw_list::{ColoredRect, DrawList};

/// Geometry generation for candles and wicks.
pub use crate::core::renderer::geometry_generator;

/// Price/time axis tick mark computation.
pub use crate::core::renderer::tick_marks;

/// GPU context management.
pub use crate::core::renderer::wgpu_context::GpuContext;

/// WebGPU renderer implementation.
pub use crate::core::renderer::wgpu_backend::{CandleInstance, CandleUniforms, WgpuRenderer};

/// GPU pipeline management.
pub use crate::core::renderer::pipeline_manager::PipelineManager;

// ═══════════════════════════════════════════════════════════════════════════════
// Re-exports: Interaction
// ═══════════════════════════════════════════════════════════════════════════════

/// Mouse/touch interaction handling.
pub use crate::core::interaction::{HitZone, InteractionHandler, TouchCrosshairMode};

/// Drawing tool management.
pub use crate::core::drawings::DrawingManager;

/// Available drawing tools.
pub use crate::core::drawings::types::DrawingTool;

// ═══════════════════════════════════════════════════════════════════════════════
// Re-exports: Series Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Chart series (candlestick, line, area, histogram, etc.).
pub use crate::core::series::{
    AreaSeriesOptions, BarSeriesOptions, BaselineSeriesOptions, HistogramDataArray, HistogramPoint,
    HistogramSeriesOptions, LineDataArray, LinePoint, LineSeriesOptions, LineStyle, OhlcDataArray,
    OhlcPoint, Series, SeriesCollection, SeriesId, SeriesType,
};

// ═══════════════════════════════════════════════════════════════════════════════
// Re-exports: Technical Studies
// ═══════════════════════════════════════════════════════════════════════════════

/// Study (indicator) management.
pub use crate::core::studies::manager::{
    Study, StudyCalculator, StudyId, StudyInput, StudyManager, StudyOutput,
};

/// Built-in study implementations.
pub use crate::core::studies::built_in::{
    register_built_in_studies, EmaCalculator, MacdCalculator, RsiCalculator, SmaCalculator,
};

// ═══════════════════════════════════════════════════════════════════════════════
// Re-exports: Price Lines & Markers
// ═══════════════════════════════════════════════════════════════════════════════

/// Horizontal price lines.
pub use crate::core::price_line::{
    PriceLine, PriceLineHit, PriceLineId, PriceLineManager, PriceLineOptions,
};

/// Series markers (annotations on bars).
pub use crate::core::markers::{
    MarkerManager, MarkerPosition, MarkerShape, SeriesMarker, SeriesMarkers,
};

// ═══════════════════════════════════════════════════════════════════════════════
// Re-exports: Pane & Layout
// ═══════════════════════════════════════════════════════════════════════════════

/// Multi-pane management.
pub use crate::core::pane::{Pane, PaneId, PaneManager, PaneOptions};

/// Dirty region tracking for efficient re-rendering.
pub use crate::core::invalidate_mask::{InvalidateMask, InvalidationLevel, PaneInvalidation};

/// Kinetic scrolling animation.
pub use crate::core::kinetic_animation::{KineticAnimation, ScrollState};

/// Global crosshair state.
pub use crate::core::crosshair::{
    Crosshair as GlobalCrosshair, CrosshairMode as GlobalCrosshairMode, CrosshairPaneView,
};

// ═══════════════════════════════════════════════════════════════════════════════
// Re-exports: Demo/Sample Data
// ═══════════════════════════════════════════════════════════════════════════════

/// Generate sample OHLCV data for testing.
pub use crate::core::demo_data::generate_sample_data;

// ═══════════════════════════════════════════════════════════════════════════════
// Re-exports: WASM-specific
// ═══════════════════════════════════════════════════════════════════════════════

/// Canvas2D renderer (WASM only).
#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::canvas2d::Canvas2DRenderer;

/// Overlay renderer for crosshair/annotations (WASM only).
#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::overlay::OverlayRenderer;

/// Price axis renderer (WASM only).
#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::price_axis::PriceAxisRenderer;

/// Time axis renderer (WASM only).
#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::time_axis::TimeAxisRenderer;
