//! Event system — typed chart events with platform-agnostic subscription.
//!
//! ## Architecture
//!
//! - [`ChartEvent`] is the typed event enum carrying event-specific data.
//! - [`EventBus`] is the core-side event dispatcher that collects events
//!   and allows platform-specific backends to drain and forward them.
//!
//! The core engine emits events into the `EventBus`. The WASM layer
//! drains the bus each frame and forwards events to JS callbacks via
//! the `EventEmitter` (in `wasm/src/event_emitter.rs`).
//!
//! ## Event Flow
//!
//! ```text
//! InteractionHandler / Viewport / DrawingManager
//!         │ emit(ChartEvent)
//!         ▼
//!     EventBus (ring buffer of pending events)
//!         │ drain()
//!         ▼
//!   WASM EventEmitter → js_sys::Function callbacks
//! ```

use std::collections::VecDeque;

// ═════════════════════════════════════════════════════════════════════════════
// Event Types
// ═════════════════════════════════════════════════════════════════════════════

/// All chart events that can be observed by consumers.
///
/// Each variant carries the relevant data for that event type.
/// Events are emitted by the core engine and forwarded to registered
/// callbacks by the platform layer (WASM EventEmitter, native handler, etc.).
#[derive(Debug, Clone)]
pub enum ChartEvent {
    /// Crosshair position changed (mouse/touch move over chart pane).
    CrosshairMove {
        /// X position in CSS pixels relative to chart pane.
        x: f64,
        /// Y position in CSS pixels relative to chart pane.
        y: f64,
        /// Bar index under the crosshair, if any.
        bar_index: Option<usize>,
        /// Price value at the crosshair Y position.
        price: f64,
        /// Timestamp of the bar under the crosshair, if any.
        timestamp: Option<u64>,
    },

    /// Visible bar range changed (zoom, pan, or data load).
    VisibleRangeChange {
        /// Start bar index (fractional, can be negative for whitespace).
        start_bar: f64,
        /// End bar index (fractional).
        end_bar: f64,
    },

    /// User clicked on the chart pane (pointer up without drag).
    Click {
        /// X position in CSS pixels relative to chart pane.
        x: f64,
        /// Y position in CSS pixels relative to chart pane.
        y: f64,
        /// Bar index at click position, if any.
        bar_index: Option<usize>,
        /// Price value at click Y position.
        price: f64,
    },

    /// A drawing was completed (all anchor points placed).
    DrawingCreated {
        /// Unique ID of the created drawing.
        id: u32,
        /// Drawing tool type name (e.g. "trend_line", "rectangle").
        tool: String,
    },

    /// A drawing was selected by clicking on it.
    DrawingSelected {
        /// Unique ID of the selected drawing, or None if deselected.
        id: Option<u32>,
    },

    /// Symbol changed via `set_symbol()`.
    SymbolChange {
        /// New symbol string.
        symbol: String,
    },

    /// Interval changed via `set_interval()`.
    IntervalChange {
        /// New interval string.
        interval: String,
    },

    /// Price scale mode changed.
    PriceScaleChange {
        /// New mode name: "normal", "logarithmic", "percentage", "indexedTo100".
        mode: String,
    },

    /// Main chart type changed.
    ChartTypeChange {
        /// New chart type name: "candlestick", "ohlc", "line", "area", etc.
        chart_type: String,
    },

    /// Chart container was resized.
    Resize {
        /// New width in CSS pixels.
        width: f64,
        /// New height in CSS pixels.
        height: f64,
    },

    /// Renderer request could not be satisfied and fell back.
    RendererFallback {
        /// Requested renderer mode (`auto`, `webgpu`, `canvas2d`).
        requested: String,
        /// Active renderer after fallback (currently `canvas2d`).
        active: String,
        /// Human-readable fallback reason.
        reason: String,
    },

    /// An error occurred during chart operation.
    Error {
        /// Human-readable error message.
        message: String,
    },

    /// User clicked on an execution mark.
    ExecutionMarkClick {
        /// Unique ID of the execution mark.
        id: String,
        /// Unix timestamp (ms) of the execution.
        timestamp_ms: u64,
        /// Execution price.
        price: f64,
        /// Side: "buy" or "sell".
        side: String,
        /// Role: "entry", "scale_in", "scale_out", or "exit".
        role: String,
        /// Execution quantity.
        quantity: f64,
        /// Optional group ID.
        group_id: Option<String>,
    },

    /// User clicked on a clustered execution-mark arrow.
    ExecutionClusterClick {
        /// Leading mark ID for the cluster.
        leader_id: String,
        /// All member IDs collapsed into the cluster.
        member_ids: Vec<String>,
    },

    /// User hovered over an execution mark.
    ExecutionMarkHover {
        /// Unique ID of the execution mark, or None when leaving.
        id: Option<String>,
        /// Unix timestamp (ms) of the execution, if hovering.
        timestamp_ms: Option<u64>,
        /// Execution price, if hovering.
        price: Option<f64>,
        /// Side: "buy" or "sell", if hovering.
        side: Option<String>,
        /// Role: "entry", "scale_in", "scale_out", or "exit", if hovering.
        role: Option<String>,
        /// Execution quantity, if hovering.
        quantity: Option<f64>,
        /// Optional group ID, if hovering.
        group_id: Option<String>,
    },

    /// User dragged an order line to modify its price.
    OrderLineModified {
        /// Unique ID of the order line.
        id: String,
        /// Original price before modification.
        old_price: f64,
        /// New price after modification.
        new_price: f64,
        /// Order type: "Limit", "Stop", "TakeProfit", etc.
        order_type: String,
        /// Order side: "Buy" or "Sell".
        side: String,
        /// Order quantity.
        quantity: f64,
    },

    /// User cancelled an order via the cancel button.
    OrderLineCancelled {
        /// Unique ID of the cancelled order line.
        id: String,
        /// Price at which the order was placed.
        price: f64,
        /// Order type: "Limit", "Stop", "TakeProfit", etc.
        order_type: String,
        /// Order side: "Buy" or "Sell".
        side: String,
        /// Order quantity.
        quantity: f64,
    },
}

impl ChartEvent {
    /// Returns the string event name used for JS callback registration.
    ///
    /// These names match the `chart.on("eventName", callback)` API.
    pub fn name(&self) -> &'static str {
        match self {
            Self::CrosshairMove { .. } => "crosshairMove",
            Self::VisibleRangeChange { .. } => "visibleRangeChange",
            Self::Click { .. } => "click",
            Self::DrawingCreated { .. } => "drawingCreated",
            Self::DrawingSelected { .. } => "drawingSelected",
            Self::SymbolChange { .. } => "symbolChange",
            Self::IntervalChange { .. } => "intervalChange",
            Self::PriceScaleChange { .. } => "priceScaleChange",
            Self::ChartTypeChange { .. } => "chartTypeChange",
            Self::Resize { .. } => "resize",
            Self::RendererFallback { .. } => "rendererFallback",
            Self::Error { .. } => "error",
            Self::ExecutionMarkClick { .. } => "executionMarkClick",
            Self::ExecutionClusterClick { .. } => "executionClusterClick",
            Self::ExecutionMarkHover { .. } => "executionMarkHover",
            Self::OrderLineModified { .. } => "orderLineModified",
            Self::OrderLineCancelled { .. } => "orderLineCancelled",
        }
    }

    /// All valid event names for documentation and validation.
    pub const ALL_EVENT_NAMES: &'static [&'static str] = &[
        "crosshairMove",
        "visibleRangeChange",
        "click",
        "drawingCreated",
        "drawingSelected",
        "symbolChange",
        "intervalChange",
        "priceScaleChange",
        "chartTypeChange",
        "resize",
        "rendererFallback",
        "error",
        "executionMarkClick",
        "executionClusterClick",
        "executionMarkHover",
        "orderLineModified",
        "orderLineCancelled",
    ];
}

// ═════════════════════════════════════════════════════════════════════════════
// EventBus — core-side event collector
// ═════════════════════════════════════════════════════════════════════════════

/// Maximum number of events buffered before oldest are dropped.
const EVENT_BUS_CAPACITY: usize = 256;

/// Core-side event bus that collects events for the platform layer to drain.
///
/// Events are stored in a ring buffer. If the buffer is full (consumer not
/// draining fast enough), the oldest events are dropped. This prevents
/// unbounded memory growth if the consumer stops calling `drain()`.
///
/// The `EventBus` lives inside `ChartEngine` and is written to by
/// interaction handlers, viewport changes, and other state mutations.
#[derive(Debug)]
pub struct EventBus {
    queue: VecDeque<ChartEvent>,
    /// Whether the bus is enabled. Disabled = events are silently dropped.
    /// This allows disabling events during batch operations.
    enabled: bool,
}

impl EventBus {
    /// Create a new empty event bus.
    pub fn new() -> Self {
        Self {
            queue: VecDeque::with_capacity(64),
            enabled: true,
        }
    }

    /// Push an event onto the bus.
    ///
    /// If the bus is disabled or at capacity, the event is dropped.
    pub fn emit(&mut self, event: ChartEvent) {
        if !self.enabled {
            return;
        }
        if self.queue.len() >= EVENT_BUS_CAPACITY {
            // Drop oldest event to make room
            self.queue.pop_front();
        }
        self.queue.push_back(event);
    }

    /// Drain all pending events from the bus.
    ///
    /// Returns an iterator over all buffered events, clearing the queue.
    /// The WASM layer calls this each frame to forward events to JS callbacks.
    pub fn drain(&mut self) -> impl Iterator<Item = ChartEvent> + '_ {
        self.queue.drain(..)
    }

    /// Check if there are pending events without draining.
    pub fn has_pending(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Number of pending events.
    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }

    /// Enable or disable the event bus.
    ///
    /// When disabled, `emit()` silently drops all events.
    /// Useful during bulk data loads or batch operations.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Whether the bus is currently enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Clear all pending events without processing them.
    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Tests
// ═════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::constants::PHYSICS_FRAME_MS;
    use crate::core::data::Bar;
    use crate::core::engine::ChartEngine;
    use crate::core::interaction::InteractionHandler;
    use crate::core::renderer::traits::RendererBackend;

    fn sample_bars(count: usize) -> Vec<Bar> {
        (0..count)
            .map(|i| {
                let base = 100.0 + i as f64 * 0.5;
                Bar::new(
                    1_000 + i as u64,
                    base,
                    base + 1.0,
                    base - 1.0,
                    base + 0.25,
                    10.0,
                )
            })
            .collect()
    }

    #[test]
    fn emit_and_drain() {
        let mut bus = EventBus::new();
        bus.emit(ChartEvent::SymbolChange {
            symbol: "BTCUSD".into(),
        });
        bus.emit(ChartEvent::IntervalChange {
            interval: "1D".into(),
        });
        assert_eq!(bus.pending_count(), 2);

        let events: Vec<_> = bus.drain().collect();
        assert_eq!(events.len(), 2);
        assert_eq!(bus.pending_count(), 0);
    }

    #[test]
    fn disabled_bus_drops_events() {
        let mut bus = EventBus::new();
        bus.set_enabled(false);
        bus.emit(ChartEvent::Click {
            x: 100.0,
            y: 200.0,
            bar_index: None,
            price: 50000.0,
        });
        assert_eq!(bus.pending_count(), 0);
    }

    #[test]
    fn capacity_limit_drops_oldest() {
        let mut bus = EventBus::new();
        for i in 0..EVENT_BUS_CAPACITY + 10 {
            bus.emit(ChartEvent::SymbolChange {
                symbol: format!("SYM{}", i),
            });
        }
        assert_eq!(bus.pending_count(), EVENT_BUS_CAPACITY);

        // First event should be the 11th one (first 10 were dropped)
        let first = bus.drain().next().unwrap();
        if let ChartEvent::SymbolChange { symbol } = first {
            assert_eq!(symbol, "SYM10");
        } else {
            panic!("Expected SymbolChange");
        }
    }

    #[test]
    fn event_names() {
        let e = ChartEvent::CrosshairMove {
            x: 0.0,
            y: 0.0,
            bar_index: None,
            price: 0.0,
            timestamp: None,
        };
        assert_eq!(e.name(), "crosshairMove");

        let e = ChartEvent::Error {
            message: "test".into(),
        };
        assert_eq!(e.name(), "error");
    }

    #[test]
    fn all_event_names_are_valid() {
        assert_eq!(ChartEvent::ALL_EVENT_NAMES.len(), 17);
        assert!(ChartEvent::ALL_EVENT_NAMES.contains(&"crosshairMove"));
        assert!(ChartEvent::ALL_EVENT_NAMES.contains(&"error"));
        assert!(ChartEvent::ALL_EVENT_NAMES.contains(&"rendererFallback"));
        assert!(ChartEvent::ALL_EVENT_NAMES.contains(&"executionMarkClick"));
        assert!(ChartEvent::ALL_EVENT_NAMES.contains(&"executionClusterClick"));
        assert!(ChartEvent::ALL_EVENT_NAMES.contains(&"executionMarkHover"));
        assert!(ChartEvent::ALL_EVENT_NAMES.contains(&"orderLineModified"));
        assert!(ChartEvent::ALL_EVENT_NAMES.contains(&"orderLineCancelled"));
    }

    #[test]
    fn clear_drops_all() {
        let mut bus = EventBus::new();
        bus.emit(ChartEvent::Resize {
            width: 800.0,
            height: 600.0,
        });
        assert!(bus.has_pending());
        bus.clear();
        assert!(!bus.has_pending());
    }

    #[test]
    fn glide_tick_emits_visible_range_change_while_animation_is_active() {
        let mut engine = ChartEngine::new(RendererBackend::Noop, 800, 400, 1.0);
        engine.set_data(sample_bars(120)).unwrap();
        engine.viewport.set_range(20.0, 80.0);
        engine.event_bus.clear();

        let mut interaction = InteractionHandler::new();
        let visible_span = engine.viewport.end_bar - engine.viewport.start_bar;
        interaction.start_horizontal_glide_by_bars(10.0, 800.0, visible_span);
        interaction.last_move_time -= PHYSICS_FRAME_MS;

        let before_start = engine.viewport.start_bar;
        let before_end = engine.viewport.end_bar;
        let still_gliding = interaction.update_gliding(
            800.0,
            400.0,
            &mut engine.viewport,
            &engine.bars,
            engine.bars.len(),
        );

        assert!(
            still_gliding,
            "glide tick should still be active mid-animation"
        );
        assert!(
            engine.emit_visible_range_change_if_changed(before_start, before_end),
            "glide tick should emit when the visible bar range changes"
        );

        let events: Vec<_> = engine.event_bus.drain().collect();
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChartEvent::VisibleRangeChange { start_bar, end_bar } => {
                assert!(*end_bar > *start_bar);
                assert!(
                    (start_bar - before_start).abs() > 1e-9 || (end_bar - before_end).abs() > 1e-9
                );
            }
            other => panic!("expected VisibleRangeChange, got {other:?}"),
        }
    }
}
