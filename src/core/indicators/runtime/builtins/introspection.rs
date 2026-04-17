//! Introspection builtins: syminfo.*, barstate.*, timeframe.*
//!
//! These provide runtime access to symbol, bar state, and timeframe metadata.

use crate::core::indicators::runtime::value::RayValue;

/// Context for introspection functions - provides access to symbol/bar/timeframe metadata.
#[derive(Debug, Clone, Default)]
pub struct IntrospectionContext {
    // Symbol info
    pub symbol: String,
    pub ticker: String,
    pub description: String,
    pub currency: String,
    pub base_currency: String,
    pub market_type: String,
    pub exchange: String,
    pub timezone: String,
    pub min_tick: f64,
    pub point_value: f64,

    // Timeframe info
    pub timeframe: String,
    pub timeframe_multiplier: i64,
    pub timeframe_period: String,
    pub is_daily: bool,
    pub is_weekly: bool,
    pub is_monthly: bool,
    pub is_intraday: bool,

    // Bar state
    pub bar_index: usize,
    pub bar_count: usize,
    pub is_confirmed: bool,
    pub is_last: bool,
    pub is_history: bool,
    pub is_realtime: bool,
    pub is_new: bool,
}

impl IntrospectionContext {
    /// Create a new introspection context from inputs and bar info.
    pub fn from_inputs(
        inputs: &serde_json::Value,
        bar_index: usize,
        bar_count: usize,
        is_confirmed: bool,
    ) -> Self {
        let symbol = inputs
            .get("symbol")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN")
            .to_string();

        let ticker = inputs
            .get("ticker")
            .and_then(|v| v.as_str())
            .unwrap_or(&symbol)
            .to_string();

        let timeframe = inputs
            .get("chartTimeframe")
            .and_then(|v| v.as_str())
            .unwrap_or("1D")
            .to_string();

        let description = inputs
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let currency = inputs
            .get("currency")
            .and_then(|v| v.as_str())
            .unwrap_or("USD")
            .to_string();

        let base_currency = inputs
            .get("baseCurrency")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let market_type = inputs
            .get("marketType")
            .and_then(|v| v.as_str())
            .unwrap_or("crypto")
            .to_string();

        let exchange = inputs
            .get("exchange")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let timezone = inputs
            .get("timezone")
            .and_then(|v| v.as_str())
            .unwrap_or("UTC")
            .to_string();

        let min_tick = inputs
            .get("minTick")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.01);

        let point_value = inputs
            .get("pointValue")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);

        // Parse timeframe
        let (multiplier, period, is_intraday, is_daily, is_weekly, is_monthly) =
            parse_timeframe(&timeframe);

        let is_last = bar_index + 1 >= bar_count;
        let is_history = !is_last || is_confirmed;
        let is_realtime = is_last && !is_confirmed;
        let is_new = bar_index == 0 || is_confirmed;

        Self {
            symbol,
            ticker,
            description,
            currency,
            base_currency,
            market_type,
            exchange,
            timezone,
            min_tick,
            point_value,
            timeframe,
            timeframe_multiplier: multiplier,
            timeframe_period: period,
            is_daily,
            is_weekly,
            is_monthly,
            is_intraday,
            bar_index,
            bar_count,
            is_confirmed,
            is_last,
            is_history,
            is_realtime,
            is_new,
        }
    }
}

/// Parse a timeframe string into components.
/// Returns (multiplier, period, is_intraday, is_daily, is_weekly, is_monthly)
fn parse_timeframe(tf: &str) -> (i64, String, bool, bool, bool, bool) {
    let tf_lower = tf.to_ascii_lowercase();

    // Handle special cases
    if tf_lower == "d" || tf_lower == "1d" {
        return (1, "D".to_string(), false, true, false, false);
    }
    if tf_lower == "w" || tf_lower == "1w" {
        return (1, "W".to_string(), false, false, true, false);
    }
    if tf_lower == "m" || tf_lower == "1m" && !tf_lower.contains("min") {
        // Check if it's monthly (M) not minute (m/min)
        if tf == "M" || tf == "1M" {
            return (1, "M".to_string(), false, false, false, true);
        }
    }

    // Parse numeric prefix
    let mut num_end = 0;
    for (i, ch) in tf_lower.char_indices() {
        if ch.is_ascii_digit() {
            num_end = i + 1;
        } else {
            break;
        }
    }

    let multiplier = if num_end > 0 {
        tf_lower[..num_end].parse::<i64>().unwrap_or(1)
    } else {
        1
    };

    let period_str = &tf_lower[num_end..];
    let (period, is_intraday, is_daily, is_weekly, is_monthly) = match period_str {
        "" | "m" | "min" | "minute" | "minutes" => ("m".to_string(), true, false, false, false),
        "h" | "hour" | "hours" => ("H".to_string(), true, false, false, false),
        "d" | "day" | "days" => ("D".to_string(), false, true, false, false),
        "w" | "week" | "weeks" => ("W".to_string(), false, false, true, false),
        "mo" | "month" | "months" => ("M".to_string(), false, false, false, true),
        _ => ("m".to_string(), true, false, false, false), // Default to minutes
    };

    (
        multiplier,
        period,
        is_intraday,
        is_daily,
        is_weekly,
        is_monthly,
    )
}

/// Evaluate syminfo.* properties.
pub fn call_syminfo(property: &str, ctx: &IntrospectionContext) -> Option<RayValue> {
    match property.to_ascii_lowercase().as_str() {
        "tickerid" | "ticker_id" => Some(RayValue::String(ctx.symbol.clone())),
        "ticker" => Some(RayValue::String(ctx.ticker.clone())),
        "description" => Some(RayValue::String(ctx.description.clone())),
        "currency" => Some(RayValue::String(ctx.currency.clone())),
        "basecurrency" | "base_currency" => Some(RayValue::String(ctx.base_currency.clone())),
        "type" | "market_type" => Some(RayValue::String(ctx.market_type.clone())),
        "exchange" => Some(RayValue::String(ctx.exchange.clone())),
        "timezone" => Some(RayValue::String(ctx.timezone.clone())),
        "mintick" | "min_tick" => Some(RayValue::Number(ctx.min_tick)),
        "pointvalue" | "point_value" => Some(RayValue::Number(ctx.point_value)),
        _ => None,
    }
}

/// Evaluate barstate.* properties.
pub fn call_barstate(property: &str, ctx: &IntrospectionContext) -> Option<RayValue> {
    match property.to_ascii_lowercase().as_str() {
        "isconfirmed" | "is_confirmed" => Some(RayValue::Bool(ctx.is_confirmed)),
        "islast" | "is_last" => Some(RayValue::Bool(ctx.is_last)),
        "ishistory" | "is_history" => Some(RayValue::Bool(ctx.is_history)),
        "isrealtime" | "is_realtime" => Some(RayValue::Bool(ctx.is_realtime)),
        "isnew" | "is_new" => Some(RayValue::Bool(ctx.is_new)),
        "isfirst" | "is_first" => Some(RayValue::Bool(ctx.bar_index == 0)),
        _ => None,
    }
}

/// Evaluate timeframe.* properties.
pub fn call_timeframe(property: &str, ctx: &IntrospectionContext) -> Option<RayValue> {
    match property.to_ascii_lowercase().as_str() {
        "period" => Some(RayValue::String(ctx.timeframe.clone())),
        "multiplier" => Some(RayValue::Number(ctx.timeframe_multiplier as f64)),
        "isdaily" | "is_daily" => Some(RayValue::Bool(ctx.is_daily)),
        "isweekly" | "is_weekly" => Some(RayValue::Bool(ctx.is_weekly)),
        "ismonthly" | "is_monthly" => Some(RayValue::Bool(ctx.is_monthly)),
        "isintraday" | "is_intraday" => Some(RayValue::Bool(ctx.is_intraday)),
        "isdwm" | "is_dwm" => Some(RayValue::Bool(
            ctx.is_daily || ctx.is_weekly || ctx.is_monthly,
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn syminfo_tickerid_returns_symbol() {
        let inputs = json!({ "symbol": "BTCUSD" });
        let ctx = IntrospectionContext::from_inputs(&inputs, 0, 100, true);
        let result = call_syminfo("tickerid", &ctx);
        assert_eq!(result, Some(RayValue::String("BTCUSD".to_string())));
    }

    #[test]
    fn syminfo_currency_defaults_to_usd() {
        let inputs = json!({ "symbol": "BTCUSD" });
        let ctx = IntrospectionContext::from_inputs(&inputs, 0, 100, true);
        let result = call_syminfo("currency", &ctx);
        assert_eq!(result, Some(RayValue::String("USD".to_string())));
    }

    #[test]
    fn barstate_isconfirmed_reflects_context() {
        let inputs = json!({ "symbol": "BTCUSD" });

        let ctx_confirmed = IntrospectionContext::from_inputs(&inputs, 50, 100, true);
        assert_eq!(
            call_barstate("isconfirmed", &ctx_confirmed),
            Some(RayValue::Bool(true))
        );

        let ctx_unconfirmed = IntrospectionContext::from_inputs(&inputs, 50, 100, false);
        assert_eq!(
            call_barstate("isconfirmed", &ctx_unconfirmed),
            Some(RayValue::Bool(false))
        );
    }

    #[test]
    fn barstate_islast_detects_last_bar() {
        let inputs = json!({ "symbol": "BTCUSD" });

        let ctx_last = IntrospectionContext::from_inputs(&inputs, 99, 100, true);
        assert_eq!(
            call_barstate("islast", &ctx_last),
            Some(RayValue::Bool(true))
        );

        let ctx_not_last = IntrospectionContext::from_inputs(&inputs, 50, 100, true);
        assert_eq!(
            call_barstate("islast", &ctx_not_last),
            Some(RayValue::Bool(false))
        );
    }

    #[test]
    fn barstate_isfirst_detects_first_bar() {
        let inputs = json!({ "symbol": "BTCUSD" });

        let ctx_first = IntrospectionContext::from_inputs(&inputs, 0, 100, true);
        assert_eq!(
            call_barstate("isfirst", &ctx_first),
            Some(RayValue::Bool(true))
        );

        let ctx_not_first = IntrospectionContext::from_inputs(&inputs, 50, 100, true);
        assert_eq!(
            call_barstate("isfirst", &ctx_not_first),
            Some(RayValue::Bool(false))
        );
    }

    #[test]
    fn timeframe_period_returns_chart_timeframe() {
        let inputs = json!({ "symbol": "BTCUSD", "chartTimeframe": "1h" });
        let ctx = IntrospectionContext::from_inputs(&inputs, 0, 100, true);
        assert_eq!(
            call_timeframe("period", &ctx),
            Some(RayValue::String("1h".to_string()))
        );
    }

    #[test]
    fn timeframe_isintraday_for_hourly() {
        let inputs = json!({ "symbol": "BTCUSD", "chartTimeframe": "1h" });
        let ctx = IntrospectionContext::from_inputs(&inputs, 0, 100, true);
        assert_eq!(
            call_timeframe("isintraday", &ctx),
            Some(RayValue::Bool(true))
        );
        assert_eq!(call_timeframe("isdaily", &ctx), Some(RayValue::Bool(false)));
    }

    #[test]
    fn timeframe_isdaily_for_1d() {
        let inputs = json!({ "symbol": "BTCUSD", "chartTimeframe": "1D" });
        let ctx = IntrospectionContext::from_inputs(&inputs, 0, 100, true);
        assert_eq!(call_timeframe("isdaily", &ctx), Some(RayValue::Bool(true)));
        assert_eq!(
            call_timeframe("isintraday", &ctx),
            Some(RayValue::Bool(false))
        );
    }

    #[test]
    fn timeframe_multiplier_parses_correctly() {
        let inputs = json!({ "symbol": "BTCUSD", "chartTimeframe": "4h" });
        let ctx = IntrospectionContext::from_inputs(&inputs, 0, 100, true);
        assert_eq!(
            call_timeframe("multiplier", &ctx),
            Some(RayValue::Number(4.0))
        );
    }

    #[test]
    fn parse_timeframe_handles_variants() {
        // Minutes
        let (m, p, intra, d, w, mo) = parse_timeframe("15m");
        assert_eq!(m, 15);
        assert_eq!(p, "m");
        assert!(intra);
        assert!(!d && !w && !mo);

        // Hours
        let (m, p, intra, _d, _w, _mo) = parse_timeframe("4h");
        assert_eq!(m, 4);
        assert_eq!(p, "H");
        assert!(intra);

        // Daily
        let (m, p, intra, d, _w, _mo) = parse_timeframe("1D");
        assert_eq!(m, 1);
        assert_eq!(p, "D");
        assert!(!intra);
        assert!(d);

        // Weekly
        let (m, p, _intra, _d, w, _mo) = parse_timeframe("W");
        assert_eq!(m, 1);
        assert_eq!(p, "W");
        assert!(w);
    }
}
