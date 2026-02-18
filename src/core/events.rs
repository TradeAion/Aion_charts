//! Event system — input events from JS/native, internal events between subsystems.
//!
//! Design: events are simple enums dispatched synchronously.
//! No heap allocation for hot-path events.

/// Events that can come from the JS/native host.
#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
    /// Mouse wheel zoom. delta > 0 = zoom out, < 0 = zoom in.
    Zoom { focal_x: f64, delta: f64 },
    /// Pan by pixel delta (converted to bar delta by caller).
    Pan { delta_x: f64 },
    /// Canvas resize.
    Resize { width: u32, height: u32 },
    /// Mouse move (for crosshair, hover, hit-testing).
    MouseMove { x: f64, y: f64 },
    /// Click (for drawing selection, etc.)
    Click { x: f64, y: f64 },
}
