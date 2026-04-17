//! Shared validation helpers for series data arrays.

use crate::core::series::{HistogramPoint, LinePoint, OhlcPoint};

#[inline]
pub fn ensure_equal_len(
    name_a: &str,
    len_a: usize,
    name_b: &str,
    len_b: usize,
) -> Result<(), String> {
    if len_a != len_b {
        Err(format!(
            "{} and {} length mismatch: {} != {}",
            name_a, name_b, len_a, len_b
        ))
    } else {
        Ok(())
    }
}

#[inline]
pub fn ensure_strictly_increasing_timestamps(name: &str, timestamps: &[u64]) -> Result<(), String> {
    for i in 1..timestamps.len() {
        if timestamps[i] <= timestamps[i - 1] {
            return Err(format!(
                "{} timestamps must be strictly increasing at index {}: {} <= {}",
                name,
                i,
                timestamps[i],
                timestamps[i - 1]
            ));
        }
    }
    Ok(())
}

#[inline]
pub fn ensure_finite_value(
    context: &str,
    field: &str,
    value: f64,
    index: usize,
) -> Result<(), String> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(format!(
            "{context}: {field} at index {index} must be finite, got {value}"
        ))
    }
}

#[inline]
pub fn ensure_finite_color(context: &str, color: [f32; 4], index: usize) -> Result<(), String> {
    for (channel, value) in [
        ("r", color[0]),
        ("g", color[1]),
        ("b", color[2]),
        ("a", color[3]),
    ] {
        ensure_finite_value(context, &format!("color.{channel}"), value as f64, index)?;
    }
    Ok(())
}

#[inline]
pub fn ensure_finite_line_point(
    context: &str,
    point: &LinePoint,
    index: usize,
) -> Result<(), String> {
    ensure_finite_value(context, "value", point.value, index)
}

#[inline]
pub fn ensure_finite_histogram_point(
    context: &str,
    point: &HistogramPoint,
    index: usize,
) -> Result<(), String> {
    ensure_finite_value(context, "value", point.value, index)?;
    ensure_finite_color(context, point.color, index)
}

#[inline]
pub fn ensure_finite_ohlc_point(
    context: &str,
    point: &OhlcPoint,
    index: usize,
) -> Result<(), String> {
    for (field, value) in [
        ("open", point.open),
        ("high", point.high),
        ("low", point.low),
        ("close", point.close),
    ] {
        ensure_finite_value(context, field, value, index)?;
    }
    Ok(())
}
