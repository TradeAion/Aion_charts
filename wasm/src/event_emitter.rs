//! JS event emitter — stores `js_sys::Function` callbacks per event name.
//!
//! Usage from WASM:
//! ```js
//! chart.on("crosshairMove", (event) => console.log(event));
//! chart.on("visibleRangeChange", (event) => updateUI(event));
//! chart.off("crosshairMove", myCallback);
//! ```
//!
//! The emitter stores callbacks in a `HashMap<String, Vec<CallbackEntry>>`.
//! When `emit()` is called, all registered callbacks for that event name
//! are invoked with the provided `JsValue` data.
#![allow(dead_code)]

use std::collections::HashMap;
use wasm_bindgen::prelude::*;

/// Entry in the callback list — tracks the function and whether it's one-shot.
struct CallbackEntry {
    func: js_sys::Function,
    once: bool,
}

/// Event emitter that stores JS function callbacks per event name.
///
/// This is the WASM-side bridge between the core `EventBus` (which collects
/// platform-agnostic `ChartEvent` values) and the JS consumer's callbacks.
pub struct EventEmitter {
    listeners: HashMap<String, Vec<CallbackEntry>>,
}

impl EventEmitter {
    /// Create a new empty event emitter.
    pub fn new() -> Self {
        Self {
            listeners: HashMap::new(),
        }
    }

    /// Register a callback for the given event name.
    ///
    /// The callback will be called with a single `JsValue` argument
    /// containing the event data object.
    pub fn on(&mut self, event: &str, callback: js_sys::Function) {
        self.listeners
            .entry(event.to_string())
            .or_default()
            .push(CallbackEntry {
                func: callback,
                once: false,
            });
    }

    /// Register a one-shot callback that auto-removes after first invocation.
    pub fn once(&mut self, event: &str, callback: js_sys::Function) {
        self.listeners
            .entry(event.to_string())
            .or_default()
            .push(CallbackEntry {
                func: callback,
                once: true,
            });
    }

    /// Remove a specific callback for the given event name.
    ///
    /// Removes the first matching callback (compared by JS object identity).
    /// Returns true if a callback was removed.
    pub fn off(&mut self, event: &str, callback: &js_sys::Function) -> bool {
        if let Some(list) = self.listeners.get_mut(event) {
            let before = list.len();
            list.retain(|entry| entry.func != *callback);
            list.len() < before
        } else {
            false
        }
    }

    /// Emit an event, calling all registered callbacks with the provided data.
    ///
    /// One-shot callbacks (`once()`) are automatically removed after invocation.
    /// Callback errors are logged to console but do not halt other callbacks.
    pub fn emit(&mut self, event: &str, data: &JsValue) {
        if let Some(list) = self.listeners.get_mut(event) {
            // Call all callbacks. Track which ones are `once` for removal.
            let mut remove_indices = Vec::new();
            for (i, entry) in list.iter().enumerate() {
                if let Err(e) = entry.func.call1(&JsValue::NULL, data) {
                    web_sys::console::error_2(
                        &JsValue::from_str(&format!("RayCore: Error in '{}' callback:", event)),
                        &e,
                    );
                }
                if entry.once {
                    remove_indices.push(i);
                }
            }
            // Remove once-callbacks in reverse order to maintain indices
            for i in remove_indices.into_iter().rev() {
                list.remove(i);
            }
        }
    }

    /// Remove all callbacks for a specific event name.
    pub fn remove_all_for(&mut self, event: &str) {
        self.listeners.remove(event);
    }

    /// Remove all callbacks for all events. Called during `dispose()`.
    pub fn remove_all_listeners(&mut self) {
        self.listeners.clear();
    }

    /// Check if any listeners are registered for the given event.
    pub fn has_listeners(&self, event: &str) -> bool {
        self.listeners
            .get(event)
            .map(|l| !l.is_empty())
            .unwrap_or(false)
    }

    /// Total number of registered callbacks across all events.
    pub fn listener_count(&self) -> usize {
        self.listeners.values().map(|l| l.len()).sum()
    }
}

impl Default for EventEmitter {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a `ChartEvent` to a JS object for the callback.
///
/// Each event type produces a plain JS object with the event's fields
/// as properties, plus a `type` field with the event name string.
pub fn chart_event_to_js(event: &raycore::ChartEvent) -> JsValue {
    let obj = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &obj,
        &JsValue::from_str("type"),
        &JsValue::from_str(event.name()),
    );

    match event {
        raycore::ChartEvent::CrosshairMove {
            x,
            y,
            bar_index,
            price,
            timestamp,
        } => {
            let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("x"), &JsValue::from_f64(*x));
            let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("y"), &JsValue::from_f64(*y));
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("barIndex"),
                &match bar_index {
                    Some(idx) => JsValue::from_f64(*idx as f64),
                    None => JsValue::NULL,
                },
            );
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("price"),
                &JsValue::from_f64(*price),
            );
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("timestamp"),
                &match timestamp {
                    Some(ts) => JsValue::from_f64(*ts as f64),
                    None => JsValue::NULL,
                },
            );
        }
        raycore::ChartEvent::VisibleRangeChange { start_bar, end_bar } => {
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("startBar"),
                &JsValue::from_f64(*start_bar),
            );
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("endBar"),
                &JsValue::from_f64(*end_bar),
            );
        }
        raycore::ChartEvent::Click {
            x,
            y,
            bar_index,
            price,
        } => {
            let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("x"), &JsValue::from_f64(*x));
            let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("y"), &JsValue::from_f64(*y));
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("barIndex"),
                &match bar_index {
                    Some(idx) => JsValue::from_f64(*idx as f64),
                    None => JsValue::NULL,
                },
            );
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("price"),
                &JsValue::from_f64(*price),
            );
        }
        raycore::ChartEvent::DrawingCreated { id, tool } => {
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("id"),
                &JsValue::from_f64(*id as f64),
            );
            let _ =
                js_sys::Reflect::set(&obj, &JsValue::from_str("tool"), &JsValue::from_str(tool));
        }
        raycore::ChartEvent::DrawingSelected { id } => {
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("id"),
                &match id {
                    Some(id) => JsValue::from_f64(*id as f64),
                    None => JsValue::NULL,
                },
            );
        }
        raycore::ChartEvent::SymbolChange { symbol } => {
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("symbol"),
                &JsValue::from_str(symbol),
            );
        }
        raycore::ChartEvent::IntervalChange { interval } => {
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("interval"),
                &JsValue::from_str(interval),
            );
        }
        raycore::ChartEvent::PriceScaleChange { mode } => {
            let _ =
                js_sys::Reflect::set(&obj, &JsValue::from_str("mode"), &JsValue::from_str(mode));
        }
        raycore::ChartEvent::ChartTypeChange { chart_type } => {
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("chartType"),
                &JsValue::from_str(chart_type),
            );
        }
        raycore::ChartEvent::Resize { width, height } => {
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("width"),
                &JsValue::from_f64(*width),
            );
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("height"),
                &JsValue::from_f64(*height),
            );
        }
        raycore::ChartEvent::RendererFallback {
            requested,
            active,
            reason,
        } => {
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("requested"),
                &JsValue::from_str(requested),
            );
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("active"),
                &JsValue::from_str(active),
            );
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("reason"),
                &JsValue::from_str(reason),
            );
        }
        raycore::ChartEvent::Error { message } => {
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("message"),
                &JsValue::from_str(message),
            );
        }
    }

    obj.into()
}
