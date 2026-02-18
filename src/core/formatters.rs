//! Price and time formatters — shared across all renderers and the overlay.
//!
//! Inspired by LWC's formatter architecture: formatters are decoupled from
//! rendering so axes, crosshair labels, HUD, etc. all use the same logic.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// ── Price formatting ─────────────────────────────────────────────────────────

/// Format a price value with automatic decimal precision based on tick step.
pub fn format_price(v: f64, step: f64) -> String {
    let d = if step >= 1.0 {
        0
    } else if step >= 0.1 {
        1
    } else if step >= 0.01 {
        2
    } else if step >= 0.001 {
        3
    } else {
        4
    };
    format!("{:.prec$}", v, prec = d)
}

// ── Time formatting ──────────────────────────────────────────────────────────

/// Format a Unix epoch millisecond timestamp into axis-appropriate label.
/// Adapts format: midnight → date only, otherwise → date+time.
#[cfg(target_arch = "wasm32")]
pub fn format_timestamp(ms: u64) -> String {
    let date = js_sys::Date::new(&JsValue::from_f64(ms as f64));
    let h = date.get_utc_hours();
    let m = date.get_utc_minutes();
    let mo = date.get_utc_month() + 1;
    let d = date.get_utc_date();
    let y = date.get_utc_full_year();

    if h == 0 && m == 0 {
        format!("{:02}/{:02}/{}", mo, d, y)
    } else {
        format!("{:02}/{:02} {:02}:{:02}", mo, d, h, m)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn format_timestamp(ms: u64) -> String {
    format!("{}", ms)
}

// ── Tick step computation ────────────────────────────────────────────────────

/// Compute a "nice" step value for axis ticks (1-2-5 series).
pub fn nice_step(raw: f64) -> f64 {
    if raw <= 0.0 {
        return 1.0;
    }
    let mag = 10.0_f64.powf(raw.log10().floor());
    let r = raw / mag;
    let n = if r <= 1.5 {
        1.0
    } else if r <= 3.5 {
        2.0
    } else if r <= 7.5 {
        5.0
    } else {
        10.0
    };
    n * mag
}
