//! # AxiusCharts — High-Performance GPU Charting Engine
//!
//! AxiusCharts is a Rust/WebAssembly financial charting library designed for
//! professional-grade performance and visual quality. It supports WebGPU
//! with Canvas2D fallback on web platforms.
//!
//! ## Architecture Overview
//!
//! AxiusCharts follows a widget-based architecture similar to TradingView's
//! Lightweight Charts (LWC):
//!
//! - **Pane System**: Multiple panes with independent price scales
//! - **Layered Rendering**: Separate base and overlay (Canvas2D) layers
//! - **Widget Components**: Price axis, time axis, and chart pane as separate DOM elements
//! - **Shared State**: Unified viewport and data management across all widgets
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use axiuscharts::{Bar, BarArray, Viewport, ChartEngine};
//!
//! // Create a bar array and populate with data
//! let mut bars = BarArray::new();
//! bars.set(vec![
//!     Bar { timestamp: 1700000000000, open: 100.0, high: 105.0, low: 98.0, close: 103.0, volume: 1000.0, _pad: 0.0 },
//!     Bar { timestamp: 1700000060000, open: 103.0, high: 108.0, low: 101.0, close: 106.0, volume: 1200.0, _pad: 0.0 },
//! ]).unwrap();
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
//! - [`core::renderer`] — WebGPU + Canvas2D rendering backends
//! - [`core::interaction`] — Mouse/touch event handling
//! - [`core::studies`] — Technical indicators (SMA, EMA, RSI, MACD)
//! - [`core::drawings`] — User annotations (trend lines, rays, etc.)
//! - [`core::series`] — Multiple chart series types
//! - [`core::indicators`] — User-authored indicator DSL + runtime scaffolding
//!
//! ## Performance Characteristics
//!
//! | Operation | Complexity | Notes |
//! |-----------|------------|-------|
//! | `BarArray::append` | O(1) | Uses pending buffer pattern |
//! | `BarArray::set` | O(n) | Bulk data load |
//! | `Viewport::zoom` | O(1) | Sub-microsecond |
//! | `Viewport::auto_fit_price` | O(k) | k = visible bars |
//! | Canvas2D render pass | O(n) | n = visible candles |
//!
//! ## Feature Flags
//!
//! - `wasm32`: Enables WebAssembly-specific features (Canvas2D, web-sys bindings)
//!
//! ## Platform Support
//!
//! - **Web**: Chrome, Firefox, Safari (Canvas2D; WebGPU where supported)

pub mod core;
pub mod group;

// ═══════════════════════════════════════════════════════════════════════════════
// Re-exports: Core Data Types
// ═══════════════════════════════════════════════════════════════════════════════

/// OHLCV bar structure and columnar storage.
pub use crate::core::data::{Bar, BarArray};

/// Viewport state and coordinate transformations.
pub use crate::core::viewport::{PriceScaleMode, Viewport};

/// Central chart engine and viewport reset presets.
pub use crate::core::engine::{ChartEngine, MainViewportPreset};

/// Main chart type (candlestick, OHLC bars, line, area, footprint, etc.).
pub use crate::core::chart_type::{MainChartOptions, MainChartType};

/// Footprint (order-flow) chart data types and options.
pub use crate::core::footprint::{
    DiagonalImbalanceType, FootprintBar, FootprintData, FootprintDisplayMode,
    FootprintGradientStyle, FootprintLevel, FootprintOptions, FootprintPalette, ImbalanceType,
    VolumeColorIntensity,
};

// ═══════════════════════════════════════════════════════════════════════════════
// Re-exports: Rendering
// ═══════════════════════════════════════════════════════════════════════════════

/// Rendering traits and abstractions.
pub use crate::core::renderer::traits::{
    ChartRenderer, ChartStyle, CrosshairMode, CrosshairState, RenderContext, Renderer,
    RendererBackend, TickMark,
};

/// Theme configuration system — presets, custom themes, CSS variable output.
pub use crate::core::renderer::theme::{
    ThemeColors, ThemeConfig, ThemeCrosshair, ThemeDrawingDefaults, ThemeIndicatorPalette,
    ThemeLastPriceLine, ThemeLayout, ThemePreset, ThemeSeriesDefaults, ThemeSubpaneSeparator,
    ThemeTypography, ThemeWorkspace,
};

/// Candle sizing strategies.
pub use crate::core::renderer::series::CandleSizing;

/// Draw list for batched rendering.
pub use crate::core::renderer::draw_list::{ColoredRect, DrawList};

/// Geometry generation for candles and wicks.
pub use crate::core::renderer::geometry_generator;

/// Price/time axis tick mark computation.
pub use crate::core::renderer::tick_marks;

// ═══════════════════════════════════════════════════════════════════════════════
// Re-exports: Interaction
// ═══════════════════════════════════════════════════════════════════════════════

/// Mouse/touch interaction handling.
pub use crate::core::interaction::{HitZone, InteractionHandler, TouchCrosshairMode};

/// Drawing tool management.
pub use crate::core::drawings::DrawingManager;

pub use crate::core::drawings::persistence::{
    DrawingSnapshot, SerializedAnchorPoint, SerializedDrawing, SerializedDrawingPoint,
    SerializedDrawingStyle, DRAWINGS_SNAPSHOT_VERSION,
};
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
// Re-exports: User Indicator Runtime (V1 scaffolding)
// ═══════════════════════════════════════════════════════════════════════════════

pub use crate::core::indicators::compiler::diagnostics::{
    CompileDiagnostic, DiagnosticSeverity, SourceSpan,
};
pub use crate::core::indicators::render::types::{
    DrawInstruction, LayerBand, ObjectMutation, RenderOrderKey,
};
pub use crate::core::indicators::runtime::events::RuntimeEvent;
pub use crate::core::indicators::runtime::limits::{ResourceCounters, ResourceLimits};
pub use crate::core::indicators::runtime::mtf::{
    MtfMode, MtfRequest, MtfRequestKey, MtfResolvedSample, MtfResolver, NoopMtfResolver,
    SnapshotMtfResolver,
};
pub use crate::core::indicators::{
    ConstantValue, IndicatorCompileResult, IndicatorFrameInput, IndicatorFrameOutput,
    IndicatorInstanceId, IndicatorInstanceStats, IndicatorInstanceSummary, IndicatorManager,
    IndicatorMtfRequestTemplate, IndicatorProgram, IndicatorProgramId, IndicatorRuntimeMessage,
    InputSchemaField, OpCode, OutputSchemaField, ResourceDecl, INDICATOR_IR_VERSION,
    INDICATOR_STDLIB_VERSION,
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

/// Execution marks (trade executions).
pub use crate::core::execution_marks::{
    bar_index_to_timestamp, timestamp_to_bar_index, ExecutionMark, ExecutionMarkManager,
    ExecutionRole, ExecutionSide,
};

/// Screen-space hit areas for execution mark interaction.
#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::overlay::ExecutionMarkHitArea;

// ═══════════════════════════════════════════════════════════════════════════════
// Re-exports: Event System
// ═══════════════════════════════════════════════════════════════════════════════

/// Typed chart events and core event bus.
pub use crate::core::events::{ChartEvent, EventBus};

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
// Re-exports: Chart Grouping & Sync
// ═══════════════════════════════════════════════════════════════════════════════

/// Native chart-group owner with link-based multi-pane synchronization.
pub use crate::group::chart_group::ChartGroup;

/// Group pane state and identifiers.
pub use crate::group::pane::{
    ChartPane as GroupPane, ChartPaneId, CrosshairMagnetMode, CrosshairSnapshot, DataRange,
    TimeRange,
};

/// Feature-level synchronization policy manager.
pub use crate::group::sync_manager::{LinkKey as GroupLinkKey, SyncFeature, SyncManager};

// ═══════════════════════════════════════════════════════════════════════════════
// Re-exports: Demo/Sample Data
// ═══════════════════════════════════════════════════════════════════════════════

/// Generate sample OHLCV data for testing.
pub use crate::core::demo_data::generate_sample_data;

/// Generate synthetic OHLCV + footprint dataset for demo footprint mode.
pub use crate::core::demo_data::generate_footprint_sample_data;

/// Generate synthetic footprint data from OHLCV bars for demo/testing.
pub use crate::core::demo_data::generate_footprint_from_bars;

/// Generate synthetic footprint data for a single bar (live updates).
pub use crate::core::demo_data::generate_footprint_for_single_bar;

// ═══════════════════════════════════════════════════════════════════════════════
// Re-exports: WASM-specific
// ═══════════════════════════════════════════════════════════════════════════════

/// Canvas2D renderer (WASM only).
#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::canvas2d::Canvas2DRenderer;

/// WebGPU context and renderer (WASM only).
#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::wgpu_backend::WgpuRenderer;
#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::wgpu_context::GpuContext;

/// Overlay renderer for crosshair/annotations (WASM only).
#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::overlay::OverlayRenderer;

/// Price axis renderer (WASM only).
#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::price_axis::PriceAxisRenderer;

/// Time axis renderer (WASM only).
#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::time_axis::TimeAxisRenderer;
