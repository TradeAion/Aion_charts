//! LWC-matching formatters — price, time, volume.
//!
//! Architecture mirrors LWC's `src/formatters/`:
//! - PriceFormatter: automatic decimal precision based on price scale
//! - Time formatting: adaptive labels (year/month/day/time) matching LWC's defaultTickMarkFormatter
//! - VolumeFormatter: K/M/B suffixes matching LWC's volume-formatter.ts
//! - nice_step: LWC-like 1-2-2.5-5 series for clean tick values

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// ── Price formatting (LWC PriceFormatter) ────────────────────────────────────

/// Format a price value with dynamic decimal precision and grouped thousands.
/// Precision follows the requested tick step instead of forcing 2 decimals.
pub fn format_price(v: f64, step: f64) -> String {
    let d = decimal_precision(step);
    // LWC uses Unicode minus \u{2212} for negative (same width as +)
    if v < 0.0 {
        format!("\u{2212}{}", format_decimal_grouped(v.abs(), d))
    } else {
        format_decimal_grouped(v, d)
    }
}

/// Format a percentage value with sign prefix (+/−).
/// Used for Percentage price scale mode.
pub fn format_percent(v: f64, step: f64) -> String {
    let d = decimal_precision(step);
    if v < 0.0 {
        format!("\u{2212}{}%", format_decimal_grouped(v.abs(), d))
    } else if v > 0.0 {
        format!("+{}%", format_decimal_grouped(v, d))
    } else {
        format!("{}%", format_decimal_grouped(v, d))
    }
}

/// Format an indexed value (for IndexedTo100 mode).
/// Shows value without % sign, typically around 100.
pub fn format_indexed(v: f64, step: f64) -> String {
    let d = decimal_precision(step);
    if v < 0.0 {
        format!("\u{2212}{}", format_decimal_grouped(v.abs(), d))
    } else {
        format_decimal_grouped(v, d)
    }
}

/// Compute decimal precision from step size (matches LWC _calculateDecimal).
fn decimal_precision(step: f64) -> usize {
    if step <= 0.0 {
        return 2;
    }
    let mut prec = 0usize;
    let mut s = step;
    while s < 0.9999 && prec < 8 {
        s *= 10.0;
        prec += 1;
    }
    prec
}

fn format_decimal_grouped(v: f64, precision: usize) -> String {
    let raw = format!("{:.prec$}", v, prec = precision);
    let (int_part, frac_part) = match raw.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (raw.as_str(), None),
    };
    let grouped = add_thousands_separators(int_part);
    match frac_part {
        Some(frac) => format!("{}.{}", grouped, frac),
        None => grouped,
    }
}

fn add_thousands_separators(int_part: &str) -> String {
    if !int_part.bytes().all(|b| b.is_ascii_digit()) {
        return int_part.to_string();
    }
    let len = int_part.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, ch) in int_part.chars().enumerate() {
        out.push(ch);
        let rem = len - i - 1;
        if rem > 0 && rem % 3 == 0 {
            out.push(',');
        }
    }
    out
}

// ── Volume formatting (LWC VolumeFormatter) ──────────────────────────────────

/// Format a volume value with K/M/B suffixes.
/// Matches LWC's VolumeFormatter.format() exactly.
pub fn format_volume(vol: f64) -> String {
    let (sign, v) = if vol < 0.0 { ("-", -vol) } else { ("", vol) };

    if v < 995.0 {
        format!("{}{}", sign, format_vol_number(v, 0))
    } else if v < 999_995.0 {
        format!("{}{}K", sign, format_vol_number(v / 1000.0, 1))
    } else if v < 999_999_995.0 {
        let v2 = 1000.0 * (v / 1000.0).round();
        format!("{}{}M", sign, format_vol_number(v2 / 1_000_000.0, 1))
    } else {
        let v2 = 1_000_000.0 * (v / 1_000_000.0).round();
        format!("{}{}B", sign, format_vol_number(v2 / 1_000_000_000.0, 1))
    }
}

fn format_vol_number(value: f64, precision: usize) -> String {
    let scale = 10.0_f64.powi(precision as i32);
    let v = (value * scale).round() / scale;
    if v >= 1e-15 && v < 1.0 {
        let s = format!("{:.prec$}", v, prec = precision);
        trim_trailing_zeros(&s)
    } else {
        let s = format!("{}", v);
        trim_trailing_zeros(&s)
    }
}

fn trim_trailing_zeros(s: &str) -> String {
    if s.contains('.') {
        let trimmed = s.trim_end_matches('0');
        if trimmed.ends_with('.') {
            trimmed[..trimmed.len() - 1].to_string()
        } else {
            trimmed.to_string()
        }
    } else {
        s.to_string()
    }
}

/// Format a quantity value (e.g., trade size) with smart precision.
/// For small values (<1), shows up to 4 decimals.
/// For medium values, shows up to 2 decimals.
/// For large values, uses K/M suffixes.
pub fn format_qty(qty: f64) -> String {
    let (sign, v) = if qty < 0.0 { ("-", -qty) } else { ("", qty) };

    if v < 0.0001 {
        format!("{}{:.6}", sign, v)
    } else if v < 1.0 {
        // Show up to 4 decimals, trim trailing zeros
        let s = format!("{}{:.4}", sign, v);
        trim_trailing_zeros(&s)
    } else if v < 1000.0 {
        // Show up to 2 decimals, trim trailing zeros
        let s = format!("{}{:.2}", sign, v);
        trim_trailing_zeros(&s)
    } else if v < 1_000_000.0 {
        format!("{}{}K", sign, format_vol_number(v / 1000.0, 1))
    } else if v < 1_000_000_000.0 {
        format!("{}{}M", sign, format_vol_number(v / 1_000_000.0, 1))
    } else {
        format!("{}{}B", sign, format_vol_number(v / 1_000_000_000.0, 1))
    }
}

// ── Time formatting (LWC defaultTickMarkFormatter) ───────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeAxisLabel {
    pub text: String,
    pub major: bool,
}

/// Format a Unix epoch millisecond timestamp into an axis-appropriate label.
/// Adapts format based on context, matching LWC's defaultTickMarkFormatter.
#[cfg(target_arch = "wasm32")]
pub fn format_time_axis_label(ms: u64) -> TimeAxisLabel {
    let date = js_sys::Date::new(&JsValue::from_f64(ms as f64));
    let h = date.get_utc_hours();
    let m = date.get_utc_minutes();
    let s = date.get_utc_seconds();
    let day = date.get_utc_date();
    let month = date.get_utc_month() + 1; // 0-based in JS
    let year = date.get_utc_full_year();

    // Determine tick mark type based on time components (LWC logic)
    if h == 0 && m == 0 && s == 0 {
        if day == 1 {
            if month == 1 {
                TimeAxisLabel {
                    text: format!("{}", year),
                    major: true,
                }
            } else {
                TimeAxisLabel {
                    text: format_month_short(month),
                    major: true,
                }
            }
        } else {
            TimeAxisLabel {
                text: format!("{}", day),
                major: false,
            }
        }
    } else if s != 0 {
        TimeAxisLabel {
            text: format!("{:02}:{:02}:{:02}", h, m, s),
            major: false,
        }
    } else {
        TimeAxisLabel {
            text: format!("{:02}:{:02}", h, m),
            major: false,
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn format_month_short(month: u32) -> String {
    match month {
        1 => "Jan".into(),
        2 => "Feb".into(),
        3 => "Mar".into(),
        4 => "Apr".into(),
        5 => "May".into(),
        6 => "Jun".into(),
        7 => "Jul".into(),
        8 => "Aug".into(),
        9 => "Sep".into(),
        10 => "Oct".into(),
        11 => "Nov".into(),
        12 => "Dec".into(),
        _ => format!("{}", month),
    }
}

/// Format a full crosshair timestamp (date + time).
/// Used for crosshair time label — always shows full date/time.
#[cfg(target_arch = "wasm32")]
pub fn format_crosshair_time(ms: u64) -> String {
    let date = js_sys::Date::new(&JsValue::from_f64(ms as f64));
    let h = date.get_utc_hours();
    let m = date.get_utc_minutes();
    let day = date.get_utc_date();
    let month = date.get_utc_month() + 1;
    let year = date.get_utc_full_year();

    if h == 0 && m == 0 {
        format!("{}-{:02}-{:02}", year, month, day)
    } else {
        format!("{}-{:02}-{:02} {:02}:{:02}", year, month, day, h, m)
    }
}

#[cfg(target_arch = "wasm32")]
pub fn format_timestamp(ms: u64) -> String {
    format_time_axis_label(ms).text
}

#[cfg(not(target_arch = "wasm32"))]
pub fn format_time_axis_label(ms: u64) -> TimeAxisLabel {
    TimeAxisLabel {
        text: format!("{}", ms),
        major: true,
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn format_timestamp(ms: u64) -> String {
    format!("{}", ms)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn format_crosshair_time(ms: u64) -> String {
    format!("{}", ms)
}

// ── Countdown formatting ─────────────────────────────────────────────────────

/// Format a remaining duration in milliseconds as a compact countdown string.
///
/// - Less than 1 hour: `MM:SS`
/// - 1 hour to < 24 hours: `H:MM:SS`
/// - 24 hours or more: `Xd HH:MM:SS`
///
/// Returns `None` if `remaining_ms` is zero or negative.
pub fn format_countdown(remaining_ms: f64) -> Option<String> {
    if remaining_ms <= 0.0 {
        return None;
    }
    let total_secs = (remaining_ms / 1000.0).ceil() as u64;
    let secs = total_secs % 60;
    let total_mins = total_secs / 60;
    let mins = total_mins % 60;
    let total_hours = total_mins / 60;
    let hours = total_hours % 24;
    let days = total_hours / 24;

    if days > 0 {
        Some(format!("{}d {:02}:{:02}:{:02}", days, hours, mins, secs))
    } else if total_hours > 0 {
        Some(format!("{}:{:02}:{:02}", hours, mins, secs))
    } else {
        Some(format!("{:02}:{:02}", mins, secs))
    }
}

// ── Tick step computation (LWC-like 1-2-2.5-5 series) ───────────────────────

/// Compute a "nice" step value for axis ticks.
/// Uses LWC-like ladder: 1, 2, 2.5, 5, 10.
pub fn nice_step(raw: f64) -> f64 {
    if raw <= 0.0 {
        return 1.0;
    }
    let mag = 10.0_f64.powf(raw.log10().floor());
    let r = raw / mag;
    let n = if r <= 1.5 {
        1.0
    } else if r <= 2.25 {
        2.0
    } else if r <= 3.75 {
        2.5
    } else if r <= 7.5 {
        5.0
    } else {
        10.0
    };
    n * mag
}

/// Compute a "nice" step using the same ladder, but round upward only.
/// This is used where dense labels are undesirable (e.g. price-axis text rows).
pub fn nice_step_ceiling(raw: f64) -> f64 {
    if raw <= 0.0 {
        return 1.0;
    }
    let mag = 10.0_f64.powf(raw.log10().floor());
    let r = raw / mag;
    let n = if r <= 1.0 {
        1.0
    } else if r <= 2.0 {
        2.0
    } else if r <= 2.5 {
        2.5
    } else if r <= 5.0 {
        5.0
    } else {
        10.0
    };
    n * mag
}

#[cfg(test)]
mod tests {
    use super::format_price;

    #[test]
    fn format_price_follows_step_precision() {
        assert_eq!(format_price(240.0, 1.0), "240");
        assert_eq!(format_price(58_780.0, 1.0), "58,780");
    }

    #[test]
    fn format_price_preserves_higher_precision_when_needed() {
        assert_eq!(format_price(1.234, 0.001), "1.234");
        assert_eq!(format_price(-1_234.5, 0.1), "\u{2212}1,234.5");
    }
}
