//! Technical Analysis (ta.*) builtin functions.
//!
//! TA functions require series history and bar context, unlike simple math builtins.
//! They are evaluated with access to BarArray and current bar_index.

use crate::core::data::BarArray;
use crate::core::indicators::runtime::value::RayValue;

/// Context for TA function evaluation - provides access to bar data.
pub struct TaContext<'a> {
    pub bars: &'a BarArray,
    pub bar_index: usize,
}

/// Dispatch ta.* function calls.
/// Returns None if the function is not recognized.
pub fn call(fn_name: &str, args: &[RayValue], ctx: Option<&TaContext>) -> Option<RayValue> {
    match fn_name {
        "sma" => Some(ta_sma(args, ctx)),
        "ema" => Some(ta_ema(args, ctx)),
        "rma" => Some(ta_rma(args, ctx)),
        "wma" => Some(ta_wma(args, ctx)),
        "vwma" => Some(ta_vwma(args, ctx)),
        "rsi" => Some(ta_rsi(args, ctx)),
        "macd" => Some(ta_macd(args, ctx)),
        "bb" => Some(ta_bb(args, ctx)),
        "tr" => Some(ta_tr(args, ctx)),
        "atr" => Some(ta_atr(args, ctx)),
        "highest" => Some(ta_highest(args, ctx)),
        "lowest" => Some(ta_lowest(args, ctx)),
        "highestbars" => Some(ta_highestbars(args, ctx)),
        "lowestbars" => Some(ta_lowestbars(args, ctx)),
        "stdev" => Some(ta_stdev(args, ctx)),
        "variance" => Some(ta_variance(args, ctx)),
        "change" => Some(ta_change(args, ctx)),
        "mom" => Some(ta_mom(args, ctx)),
        "roc" => Some(ta_roc(args, ctx)),
        "cross" => Some(ta_cross(args)),
        "crossover" => Some(ta_crossover(args)),
        "crossunder" => Some(ta_crossunder(args)),
        "rising" => Some(ta_rising(args, ctx)),
        "falling" => Some(ta_falling(args, ctx)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Helper: extract series values from BarArray for a given length
// ---------------------------------------------------------------------------

/// Get the last `length` values of a series ending at bar_index.
/// Returns None if there isn't enough history.
#[allow(dead_code)]
fn get_series_window(source_values: &[f64], bar_index: usize, length: usize) -> Option<Vec<f64>> {
    if length == 0 || bar_index + 1 < length {
        return None;
    }
    let start = bar_index + 1 - length;
    let end = bar_index + 1;
    Some(source_values[start..end].to_vec())
}

/// Extract source series from args[0] - either a direct number or we need bar context.
/// For now, we support close/high/low/open/volume from bar context when source is a variable.
#[allow(dead_code)]
fn extract_source_value(arg: &RayValue, _ctx: Option<&TaContext>) -> Option<f64> {
    match arg {
        RayValue::Number(n) => Some(*n),
        _ => {
            // If arg is Na, return Na value marker
            if arg.is_na() {
                return None;
            }
            // For variables, we'd need the var series - for now just use the number
            arg.as_number()
        }
    }
}

// ---------------------------------------------------------------------------
// Simple Moving Average: ta.sma(source, length)
// ---------------------------------------------------------------------------

fn ta_sma(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let length = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(0);

    if length == 0 {
        return RayValue::Na;
    }

    let Some(ctx) = ctx else {
        return RayValue::Na;
    };

    if ctx.bar_index + 1 < length {
        return RayValue::Na;
    }

    // For ta.sma(close, length), we need to get the close values
    // The first arg tells us what series to use
    let source_series = get_close_series(ctx.bars, ctx.bar_index, length);

    if source_series.len() < length {
        return RayValue::Na;
    }

    let sum: f64 = source_series.iter().sum();
    RayValue::Number(sum / length as f64)
}

/// Helper to get close prices for the last N bars
fn get_close_series(bars: &BarArray, bar_index: usize, length: usize) -> Vec<f64> {
    if bar_index + 1 < length || bars.len() < length {
        return vec![];
    }
    let start = bar_index + 1 - length;
    let end = bar_index + 1;
    (start..end).map(|i| bars.close(i) as f64).collect()
}

fn get_high_series(bars: &BarArray, bar_index: usize, length: usize) -> Vec<f64> {
    if bar_index + 1 < length || bars.len() < length {
        return vec![];
    }
    let start = bar_index + 1 - length;
    let end = bar_index + 1;
    (start..end).map(|i| bars.high(i) as f64).collect()
}

fn get_low_series(bars: &BarArray, bar_index: usize, length: usize) -> Vec<f64> {
    if bar_index + 1 < length || bars.len() < length {
        return vec![];
    }
    let start = bar_index + 1 - length;
    let end = bar_index + 1;
    (start..end).map(|i| bars.low(i) as f64).collect()
}

fn get_volume_series(bars: &BarArray, bar_index: usize, length: usize) -> Vec<f64> {
    if bar_index + 1 < length || bars.len() < length {
        return vec![];
    }
    let start = bar_index + 1 - length;
    let end = bar_index + 1;
    (start..end).map(|i| bars.volume(i) as f64).collect()
}

// ---------------------------------------------------------------------------
// Exponential Moving Average: ta.ema(source, length)
// Note: EMA is stateful - it needs its previous value. For now we compute
// from scratch each bar (less efficient but stateless).
// ---------------------------------------------------------------------------

fn ta_ema(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let length = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(0);

    if length == 0 {
        return RayValue::Na;
    }

    let Some(ctx) = ctx else {
        return RayValue::Na;
    };

    if ctx.bar_index + 1 < length {
        return RayValue::Na;
    }

    // Get all close values from start to current bar
    let closes: Vec<f64> = (0..=ctx.bar_index)
        .map(|i| ctx.bars.close(i) as f64)
        .collect();

    // Calculate EMA from the beginning
    let alpha = 2.0 / (length as f64 + 1.0);

    // Initialize with SMA of first `length` values
    if closes.len() < length {
        return RayValue::Na;
    }

    let initial_sma: f64 = closes[..length].iter().sum::<f64>() / length as f64;

    // Apply EMA formula from bar `length` onwards
    let mut ema = initial_sma;
    for i in length..closes.len() {
        ema = alpha * closes[i] + (1.0 - alpha) * ema;
    }

    RayValue::Number(ema)
}

// ---------------------------------------------------------------------------
// RMA (Wilder's Smoothing / Running Moving Average): ta.rma(source, length)
// ---------------------------------------------------------------------------

fn ta_rma(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let length = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(0);

    if length == 0 {
        return RayValue::Na;
    }

    let Some(ctx) = ctx else {
        return RayValue::Na;
    };

    if ctx.bar_index + 1 < length {
        return RayValue::Na;
    }

    let closes: Vec<f64> = (0..=ctx.bar_index)
        .map(|i| ctx.bars.close(i) as f64)
        .collect();

    if closes.len() < length {
        return RayValue::Na;
    }

    // RMA uses alpha = 1/length (Wilder's smoothing)
    let alpha = 1.0 / length as f64;

    // Initialize with SMA of first `length` values
    let initial_sma: f64 = closes[..length].iter().sum::<f64>() / length as f64;

    let mut rma = initial_sma;
    for i in length..closes.len() {
        rma = alpha * closes[i] + (1.0 - alpha) * rma;
    }

    RayValue::Number(rma)
}

// ---------------------------------------------------------------------------
// Weighted Moving Average: ta.wma(source, length)
// ---------------------------------------------------------------------------

fn ta_wma(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let length = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(0);

    if length == 0 {
        return RayValue::Na;
    }

    let Some(ctx) = ctx else {
        return RayValue::Na;
    };

    let series = get_close_series(ctx.bars, ctx.bar_index, length);
    if series.len() < length {
        return RayValue::Na;
    }

    // WMA: sum(weight * value) / sum(weights)
    // Weights: 1, 2, 3, ..., length (most recent has highest weight)
    let mut weighted_sum = 0.0;
    let mut weight_sum = 0.0;
    for (i, value) in series.iter().enumerate() {
        let weight = (i + 1) as f64;
        weighted_sum += weight * value;
        weight_sum += weight;
    }

    RayValue::Number(weighted_sum / weight_sum)
}

// ---------------------------------------------------------------------------
// Volume Weighted Moving Average: ta.vwma(source, length)
// ---------------------------------------------------------------------------

fn ta_vwma(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let length = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(0);

    if length == 0 {
        return RayValue::Na;
    }

    let Some(ctx) = ctx else {
        return RayValue::Na;
    };

    let closes = get_close_series(ctx.bars, ctx.bar_index, length);
    let volumes = get_volume_series(ctx.bars, ctx.bar_index, length);

    if closes.len() < length || volumes.len() < length {
        return RayValue::Na;
    }

    let mut pv_sum = 0.0;
    let mut v_sum = 0.0;
    for i in 0..length {
        pv_sum += closes[i] * volumes[i];
        v_sum += volumes[i];
    }

    if v_sum.abs() < f64::EPSILON {
        return RayValue::Na;
    }

    RayValue::Number(pv_sum / v_sum)
}

// ---------------------------------------------------------------------------
// RSI: ta.rsi(source, length)
// ---------------------------------------------------------------------------

fn ta_rsi(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let length = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(14);

    if length == 0 {
        return RayValue::Na;
    }

    let Some(ctx) = ctx else {
        return RayValue::Na;
    };

    // Need at least length + 1 bars to calculate changes
    if ctx.bar_index < length {
        return RayValue::Na;
    }

    let closes: Vec<f64> = (0..=ctx.bar_index)
        .map(|i| ctx.bars.close(i) as f64)
        .collect();

    // Calculate gains and losses
    let mut gains = Vec::new();
    let mut losses = Vec::new();
    for i in 1..closes.len() {
        let change = closes[i] - closes[i - 1];
        if change > 0.0 {
            gains.push(change);
            losses.push(0.0);
        } else {
            gains.push(0.0);
            losses.push(-change);
        }
    }

    if gains.len() < length {
        return RayValue::Na;
    }

    // Use RMA (Wilder's smoothing) for average gain/loss
    let alpha = 1.0 / length as f64;

    // Initialize with SMA of first `length` values
    let avg_gain_init: f64 = gains[..length].iter().sum::<f64>() / length as f64;
    let avg_loss_init: f64 = losses[..length].iter().sum::<f64>() / length as f64;

    let mut avg_gain = avg_gain_init;
    let mut avg_loss = avg_loss_init;

    for i in length..gains.len() {
        avg_gain = alpha * gains[i] + (1.0 - alpha) * avg_gain;
        avg_loss = alpha * losses[i] + (1.0 - alpha) * avg_loss;
    }

    if avg_loss.abs() < f64::EPSILON {
        return RayValue::Number(100.0);
    }

    let rs = avg_gain / avg_loss;
    let rsi = 100.0 - (100.0 / (1.0 + rs));

    RayValue::Number(rsi)
}

// ---------------------------------------------------------------------------
// MACD: ta.macd(source, fast_length, slow_length, signal_length)
// Returns Tuple([macd_line, signal_line, histogram])
// ---------------------------------------------------------------------------

fn ta_macd(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let fast_length = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(12);
    let slow_length = args
        .get(2)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(26);
    let signal_length = args
        .get(3)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(9);

    if fast_length == 0 || slow_length == 0 || signal_length == 0 {
        return RayValue::Tuple(vec![RayValue::Na, RayValue::Na, RayValue::Na]);
    }

    let Some(ctx) = ctx else {
        return RayValue::Tuple(vec![RayValue::Na, RayValue::Na, RayValue::Na]);
    };

    // Need enough bars for slow EMA + signal smoothing
    let min_bars = slow_length + signal_length;
    if ctx.bar_index + 1 < min_bars {
        return RayValue::Tuple(vec![RayValue::Na, RayValue::Na, RayValue::Na]);
    }

    let closes: Vec<f64> = (0..=ctx.bar_index)
        .map(|i| ctx.bars.close(i) as f64)
        .collect();

    // Calculate fast EMA
    let fast_alpha = 2.0 / (fast_length as f64 + 1.0);
    let fast_ema = calculate_ema_series(&closes, fast_length, fast_alpha);

    // Calculate slow EMA
    let slow_alpha = 2.0 / (slow_length as f64 + 1.0);
    let slow_ema = calculate_ema_series(&closes, slow_length, slow_alpha);

    if fast_ema.is_empty() || slow_ema.is_empty() {
        return RayValue::Tuple(vec![RayValue::Na, RayValue::Na, RayValue::Na]);
    }

    // MACD line = fast EMA - slow EMA (aligned from slow_length onwards)
    let macd_start = slow_length.saturating_sub(1);
    let mut macd_line: Vec<f64> = Vec::new();
    for i in macd_start..closes.len() {
        let fast_idx = i.saturating_sub(fast_length.saturating_sub(1));
        let slow_idx = i.saturating_sub(slow_length.saturating_sub(1));
        if fast_idx < fast_ema.len() && slow_idx < slow_ema.len() {
            macd_line.push(fast_ema[fast_idx] - slow_ema[slow_idx]);
        }
    }

    if macd_line.len() < signal_length {
        return RayValue::Tuple(vec![RayValue::Na, RayValue::Na, RayValue::Na]);
    }

    // Signal line = EMA of MACD line
    let signal_alpha = 2.0 / (signal_length as f64 + 1.0);
    let signal_line = calculate_ema_series(&macd_line, signal_length, signal_alpha);

    if signal_line.is_empty() {
        return RayValue::Tuple(vec![RayValue::Na, RayValue::Na, RayValue::Na]);
    }

    // Get the latest values
    let macd_value = *macd_line.last().unwrap_or(&0.0);
    let signal_value = *signal_line.last().unwrap_or(&0.0);
    let histogram_value = macd_value - signal_value;

    RayValue::Tuple(vec![
        RayValue::Number(macd_value),
        RayValue::Number(signal_value),
        RayValue::Number(histogram_value),
    ])
}

/// Helper to calculate full EMA series from beginning
fn calculate_ema_series(values: &[f64], length: usize, alpha: f64) -> Vec<f64> {
    if values.len() < length {
        return vec![];
    }

    let mut ema_values = Vec::with_capacity(values.len() - length + 1);

    // Initialize with SMA of first `length` values
    let initial_sma: f64 = values[..length].iter().sum::<f64>() / length as f64;
    ema_values.push(initial_sma);

    // Apply EMA formula
    let mut ema = initial_sma;
    for i in length..values.len() {
        ema = alpha * values[i] + (1.0 - alpha) * ema;
        ema_values.push(ema);
    }

    ema_values
}

// ---------------------------------------------------------------------------
// Bollinger Bands: ta.bb(source, length, mult)
// Returns Tuple([middle, upper, lower])
// ---------------------------------------------------------------------------

fn ta_bb(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let length = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(20);
    let mult = args.get(2).and_then(|v| v.as_number()).unwrap_or(2.0);

    if length == 0 {
        return RayValue::Tuple(vec![RayValue::Na, RayValue::Na, RayValue::Na]);
    }

    let Some(ctx) = ctx else {
        return RayValue::Tuple(vec![RayValue::Na, RayValue::Na, RayValue::Na]);
    };

    if ctx.bar_index + 1 < length {
        return RayValue::Tuple(vec![RayValue::Na, RayValue::Na, RayValue::Na]);
    }

    let series = get_close_series(ctx.bars, ctx.bar_index, length);
    if series.len() < length {
        return RayValue::Tuple(vec![RayValue::Na, RayValue::Na, RayValue::Na]);
    }

    // Middle band = SMA
    let middle: f64 = series.iter().sum::<f64>() / length as f64;

    // Standard deviation
    let variance: f64 = series.iter().map(|x| (x - middle).powi(2)).sum::<f64>() / length as f64;
    let stdev = variance.sqrt();

    // Upper and lower bands
    let upper = middle + mult * stdev;
    let lower = middle - mult * stdev;

    RayValue::Tuple(vec![
        RayValue::Number(middle),
        RayValue::Number(upper),
        RayValue::Number(lower),
    ])
}

// ---------------------------------------------------------------------------
// True Range: ta.tr(handle_na)
// ---------------------------------------------------------------------------

fn ta_tr(_args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let Some(ctx) = ctx else {
        return RayValue::Na;
    };

    if ctx.bar_index == 0 {
        // First bar: TR = high - low
        let high = ctx.bars.high(0) as f64;
        let low = ctx.bars.low(0) as f64;
        return RayValue::Number(high - low);
    }

    let high = ctx.bars.high(ctx.bar_index) as f64;
    let low = ctx.bars.low(ctx.bar_index) as f64;
    let prev_close = ctx.bars.close(ctx.bar_index - 1) as f64;

    // TR = max(high - low, abs(high - prev_close), abs(low - prev_close))
    let tr = (high - low)
        .max((high - prev_close).abs())
        .max((low - prev_close).abs());

    RayValue::Number(tr)
}

// ---------------------------------------------------------------------------
// Average True Range: ta.atr(length)
// ---------------------------------------------------------------------------

fn ta_atr(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let length = args
        .first()
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(14);

    if length == 0 {
        return RayValue::Na;
    }

    let Some(ctx) = ctx else {
        return RayValue::Na;
    };

    if ctx.bar_index + 1 < length {
        return RayValue::Na;
    }

    // Calculate TR for all bars
    let mut tr_values = Vec::with_capacity(ctx.bar_index + 1);
    for i in 0..=ctx.bar_index {
        let tr = if i == 0 {
            ctx.bars.high(0) as f64 - ctx.bars.low(0) as f64
        } else {
            let high = ctx.bars.high(i) as f64;
            let low = ctx.bars.low(i) as f64;
            let prev_close = ctx.bars.close(i - 1) as f64;
            (high - low)
                .max((high - prev_close).abs())
                .max((low - prev_close).abs())
        };
        tr_values.push(tr);
    }

    // Apply RMA (Wilder's smoothing)
    let alpha = 1.0 / length as f64;
    let initial_sma: f64 = tr_values[..length].iter().sum::<f64>() / length as f64;

    let mut atr = initial_sma;
    for i in length..tr_values.len() {
        atr = alpha * tr_values[i] + (1.0 - alpha) * atr;
    }

    RayValue::Number(atr)
}

// ---------------------------------------------------------------------------
// Highest/Lowest: ta.highest(source, length), ta.lowest(source, length)
// ---------------------------------------------------------------------------

fn ta_highest(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let length = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(0);

    if length == 0 {
        return RayValue::Na;
    }

    let Some(ctx) = ctx else {
        return RayValue::Na;
    };

    let series = get_high_series(ctx.bars, ctx.bar_index, length);
    if series.is_empty() {
        return RayValue::Na;
    }

    series
        .iter()
        .copied()
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(RayValue::Number)
        .unwrap_or(RayValue::Na)
}

fn ta_lowest(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let length = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(0);

    if length == 0 {
        return RayValue::Na;
    }

    let Some(ctx) = ctx else {
        return RayValue::Na;
    };

    let series = get_low_series(ctx.bars, ctx.bar_index, length);
    if series.is_empty() {
        return RayValue::Na;
    }

    series
        .iter()
        .copied()
        .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(RayValue::Number)
        .unwrap_or(RayValue::Na)
}

// ---------------------------------------------------------------------------
// Highestbars/Lowestbars: bars since highest/lowest
// ---------------------------------------------------------------------------

fn ta_highestbars(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let length = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(0);

    if length == 0 {
        return RayValue::Na;
    }

    let Some(ctx) = ctx else {
        return RayValue::Na;
    };

    let series = get_high_series(ctx.bars, ctx.bar_index, length);
    if series.is_empty() {
        return RayValue::Na;
    }

    let (max_idx, _) = series
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap();

    // Return negative offset from current bar
    RayValue::Number(-((length - 1 - max_idx) as f64))
}

fn ta_lowestbars(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let length = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(0);

    if length == 0 {
        return RayValue::Na;
    }

    let Some(ctx) = ctx else {
        return RayValue::Na;
    };

    let series = get_low_series(ctx.bars, ctx.bar_index, length);
    if series.is_empty() {
        return RayValue::Na;
    }

    let (min_idx, _) = series
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap();

    RayValue::Number(-((length - 1 - min_idx) as f64))
}

// ---------------------------------------------------------------------------
// Standard Deviation: ta.stdev(source, length)
// ---------------------------------------------------------------------------

fn ta_stdev(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let length = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(0);

    if length < 2 {
        return RayValue::Na;
    }

    let Some(ctx) = ctx else {
        return RayValue::Na;
    };

    let series = get_close_series(ctx.bars, ctx.bar_index, length);
    if series.len() < length {
        return RayValue::Na;
    }

    let mean: f64 = series.iter().sum::<f64>() / length as f64;
    let variance: f64 = series.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / length as f64;

    RayValue::Number(variance.sqrt())
}

fn ta_variance(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let length = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(0);

    if length < 2 {
        return RayValue::Na;
    }

    let Some(ctx) = ctx else {
        return RayValue::Na;
    };

    let series = get_close_series(ctx.bars, ctx.bar_index, length);
    if series.len() < length {
        return RayValue::Na;
    }

    let mean: f64 = series.iter().sum::<f64>() / length as f64;
    let variance: f64 = series.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / length as f64;

    RayValue::Number(variance)
}

// ---------------------------------------------------------------------------
// Change/Momentum/ROC
// ---------------------------------------------------------------------------

fn ta_change(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let length = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(1);

    let Some(ctx) = ctx else {
        return RayValue::Na;
    };

    if ctx.bar_index < length {
        return RayValue::Na;
    }

    let current = ctx.bars.close(ctx.bar_index) as f64;
    let previous = ctx.bars.close(ctx.bar_index - length) as f64;

    RayValue::Number(current - previous)
}

fn ta_mom(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    // Momentum is the same as change
    ta_change(args, ctx)
}

fn ta_roc(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let length = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(1);

    let Some(ctx) = ctx else {
        return RayValue::Na;
    };

    if ctx.bar_index < length {
        return RayValue::Na;
    }

    let current = ctx.bars.close(ctx.bar_index) as f64;
    let previous = ctx.bars.close(ctx.bar_index - length) as f64;

    if previous.abs() < f64::EPSILON {
        return RayValue::Na;
    }

    RayValue::Number(100.0 * (current - previous) / previous)
}

// ---------------------------------------------------------------------------
// Cross functions (don't need bar context - operate on current values)
// ---------------------------------------------------------------------------

fn ta_cross(args: &[RayValue]) -> RayValue {
    // cross(a, b) = crossover(a, b) or crossunder(a, b)
    // For single-bar evaluation without history, we can only check equality
    let a = args.first().and_then(|v| v.as_number());
    let b = args.get(1).and_then(|v| v.as_number());

    match (a, b) {
        (Some(a), Some(b)) => RayValue::Bool((a - b).abs() < f64::EPSILON),
        _ => RayValue::Na,
    }
}

fn ta_crossover(_args: &[RayValue]) -> RayValue {
    // Without history, we can't determine crossover in a single evaluation
    // This would need the VarSeries for previous values
    // For now, return Na (proper implementation needs stateful tracking)
    RayValue::Na
}

fn ta_crossunder(_args: &[RayValue]) -> RayValue {
    // Same limitation as crossover
    RayValue::Na
}

// ---------------------------------------------------------------------------
// Rising/Falling
// ---------------------------------------------------------------------------

fn ta_rising(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let length = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(1);

    let Some(ctx) = ctx else {
        return RayValue::Na;
    };

    if ctx.bar_index < length {
        return RayValue::Na;
    }

    // Check if close has been rising for `length` bars
    for i in 0..length {
        let idx = ctx.bar_index - i;
        if idx == 0 {
            return RayValue::Bool(false);
        }
        let current = ctx.bars.close(idx) as f64;
        let previous = ctx.bars.close(idx - 1) as f64;
        if current <= previous {
            return RayValue::Bool(false);
        }
    }

    RayValue::Bool(true)
}

fn ta_falling(args: &[RayValue], ctx: Option<&TaContext>) -> RayValue {
    let length = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.round() as usize)
        .unwrap_or(1);

    let Some(ctx) = ctx else {
        return RayValue::Na;
    };

    if ctx.bar_index < length {
        return RayValue::Na;
    }

    // Check if close has been falling for `length` bars
    for i in 0..length {
        let idx = ctx.bar_index - i;
        if idx == 0 {
            return RayValue::Bool(false);
        }
        let current = ctx.bars.close(idx) as f64;
        let previous = ctx.bars.close(idx - 1) as f64;
        if current >= previous {
            return RayValue::Bool(false);
        }
    }

    RayValue::Bool(true)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::data::Bar;

    fn sample_bars() -> BarArray {
        let mut bars = BarArray::new();
        // Create 10 bars with predictable values for TA testing
        // Close prices: 102, 103, 104, 105, 106, 107, 108, 109, 110, 111 (rising)
        bars.set(vec![
            Bar {
                timestamp: 1,
                open: 100.0,
                high: 105.0,
                low: 95.0,
                close: 102.0,
                volume: 1000.0,
            },
            Bar {
                timestamp: 2,
                open: 101.0,
                high: 106.0,
                low: 96.0,
                close: 103.0,
                volume: 1100.0,
            },
            Bar {
                timestamp: 3,
                open: 102.0,
                high: 107.0,
                low: 97.0,
                close: 104.0,
                volume: 1200.0,
            },
            Bar {
                timestamp: 4,
                open: 103.0,
                high: 108.0,
                low: 98.0,
                close: 105.0,
                volume: 1300.0,
            },
            Bar {
                timestamp: 5,
                open: 104.0,
                high: 109.0,
                low: 99.0,
                close: 106.0,
                volume: 1400.0,
            },
            Bar {
                timestamp: 6,
                open: 105.0,
                high: 110.0,
                low: 100.0,
                close: 107.0,
                volume: 1500.0,
            },
            Bar {
                timestamp: 7,
                open: 106.0,
                high: 111.0,
                low: 101.0,
                close: 108.0,
                volume: 1600.0,
            },
            Bar {
                timestamp: 8,
                open: 107.0,
                high: 112.0,
                low: 102.0,
                close: 109.0,
                volume: 1700.0,
            },
            Bar {
                timestamp: 9,
                open: 108.0,
                high: 113.0,
                low: 103.0,
                close: 110.0,
                volume: 1800.0,
            },
            Bar {
                timestamp: 10,
                open: 109.0,
                high: 114.0,
                low: 104.0,
                close: 111.0,
                volume: 1900.0,
            },
        ])
        .unwrap();
        bars
    }

    #[test]
    fn sma_calculates_simple_average() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 4, // bars 0-4 available
        };

        // SMA of close for 5 bars: (102+103+104+105+106) / 5 = 104
        let result = ta_sma(&[RayValue::Na, RayValue::Number(5.0)], Some(&ctx));
        assert_eq!(result.as_number(), Some(104.0));
    }

    #[test]
    fn sma_returns_na_for_insufficient_bars() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 2, // only 3 bars available
        };

        let result = ta_sma(&[RayValue::Na, RayValue::Number(5.0)], Some(&ctx));
        assert!(result.is_na());
    }

    #[test]
    fn ema_calculates_exponential_average() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 9, // all 10 bars
        };

        let result = ta_ema(&[RayValue::Na, RayValue::Number(5.0)], Some(&ctx));

        // EMA should return a valid number
        assert!(result.as_number().is_some());
        let ema = result.as_number().unwrap();
        // EMA should be close to recent prices
        assert!(ema > 105.0 && ema < 115.0);
    }

    #[test]
    fn rsi_calculates_relative_strength() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 9,
        };

        let result = ta_rsi(&[RayValue::Na, RayValue::Number(5.0)], Some(&ctx));

        // RSI should be between 0 and 100
        let rsi = result.as_number().unwrap();
        assert!(rsi >= 0.0 && rsi <= 100.0);
        // With consistently rising prices, RSI should be high
        assert!(rsi > 50.0);
    }

    #[test]
    fn highest_finds_maximum() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 4,
        };

        // Highest high in last 5 bars
        let result = ta_highest(&[RayValue::Na, RayValue::Number(5.0)], Some(&ctx));
        assert_eq!(result.as_number(), Some(109.0)); // high of bar 4
    }

    #[test]
    fn lowest_finds_minimum() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 4,
        };

        // Lowest low in last 5 bars
        let result = ta_lowest(&[RayValue::Na, RayValue::Number(5.0)], Some(&ctx));
        assert_eq!(result.as_number(), Some(95.0)); // low of bar 0
    }

    #[test]
    fn tr_calculates_true_range() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 1,
        };

        let result = ta_tr(&[], Some(&ctx));
        // TR = max(H-L, |H-prevC|, |L-prevC|)
        // bar 1: H=106, L=96, prevC=102
        // TR = max(10, 4, 6) = 10
        assert_eq!(result.as_number(), Some(10.0));
    }

    #[test]
    fn stdev_calculates_standard_deviation() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 4,
        };

        let result = ta_stdev(&[RayValue::Na, RayValue::Number(5.0)], Some(&ctx));

        // Closes: 102, 103, 104, 105, 106
        // Mean: 104
        // Variance: ((−2)² + (−1)² + 0² + 1² + 2²) / 5 = 10/5 = 2
        // StdDev: sqrt(2) ≈ 1.414
        let stdev = result.as_number().unwrap();
        assert!((stdev - 1.414).abs() < 0.01);
    }

    #[test]
    fn change_calculates_difference() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 5,
        };

        let result = ta_change(&[RayValue::Na, RayValue::Number(3.0)], Some(&ctx));
        // close[5] - close[2] = 107 - 104 = 3
        assert_eq!(result.as_number(), Some(3.0));
    }

    #[test]
    fn rising_detects_uptrend() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 5,
        };

        let result = ta_rising(&[RayValue::Na, RayValue::Number(3.0)], Some(&ctx));
        // All closes are rising, so should be true
        assert_eq!(result, RayValue::Bool(true));
    }

    #[test]
    fn macd_returns_tuple() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 9, // Need enough bars for MACD
        };

        // Use smaller periods for our 10-bar test data
        let result = ta_macd(
            &[
                RayValue::Na,
                RayValue::Number(3.0), // fast
                RayValue::Number(5.0), // slow
                RayValue::Number(2.0), // signal
            ],
            Some(&ctx),
        );

        // Should return a tuple of 3 values
        assert!(matches!(result, RayValue::Tuple(_)));
        let tuple_len = result.tuple_len().unwrap();
        assert_eq!(tuple_len, 3, "MACD should return 3 values");

        // All values should be numbers (not Na) with enough data
        let macd = result.get_tuple_element(0).unwrap();
        let signal = result.get_tuple_element(1).unwrap();
        let histogram = result.get_tuple_element(2).unwrap();

        assert!(macd.as_number().is_some(), "MACD line should be a number");
        assert!(
            signal.as_number().is_some(),
            "Signal line should be a number"
        );
        assert!(
            histogram.as_number().is_some(),
            "Histogram should be a number"
        );

        // Histogram should be MACD - Signal
        let macd_val = macd.as_number().unwrap();
        let signal_val = signal.as_number().unwrap();
        let hist_val = histogram.as_number().unwrap();
        assert!((hist_val - (macd_val - signal_val)).abs() < 0.0001);
    }

    #[test]
    fn bb_returns_tuple() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 4, // 5 bars available
        };

        let result = ta_bb(
            &[
                RayValue::Na,
                RayValue::Number(5.0), // length
                RayValue::Number(2.0), // mult
            ],
            Some(&ctx),
        );

        // Should return a tuple of 3 values
        assert!(matches!(result, RayValue::Tuple(_)));
        let tuple_len = result.tuple_len().unwrap();
        assert_eq!(tuple_len, 3, "BB should return 3 values");

        let middle = result.get_tuple_element(0).unwrap();
        let upper = result.get_tuple_element(1).unwrap();
        let lower = result.get_tuple_element(2).unwrap();

        // All values should be numbers
        let middle_val = middle.as_number().unwrap();
        let upper_val = upper.as_number().unwrap();
        let lower_val = lower.as_number().unwrap();

        // Middle should be SMA(5) = (102+103+104+105+106)/5 = 104
        assert!((middle_val - 104.0).abs() < 0.01, "Middle should be ~104");

        // Upper should be > middle, lower should be < middle
        assert!(upper_val > middle_val, "Upper band should be above middle");
        assert!(lower_val < middle_val, "Lower band should be below middle");

        // Bands should be symmetric around middle
        assert!(
            ((upper_val - middle_val) - (middle_val - lower_val)).abs() < 0.0001,
            "Bands should be symmetric"
        );
    }

    #[test]
    fn atr_calculates_average_true_range() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 9,
        };

        let result = ta_atr(&[RayValue::Number(5.0)], Some(&ctx));

        // ATR should be a positive number
        let atr = result.as_number().unwrap();
        assert!(atr > 0.0, "ATR should be positive");
        // With our sample data (10 point range per bar), ATR should be around 10
        assert!(
            atr > 5.0 && atr < 15.0,
            "ATR should be around 10, got {}",
            atr
        );
    }

    #[test]
    fn wma_weighted_average() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 4,
        };

        let result = ta_wma(&[RayValue::Na, RayValue::Number(3.0)], Some(&ctx));

        // WMA gives more weight to recent prices
        let wma = result.as_number().unwrap();
        // Closes for last 3 bars: 104, 105, 106
        // Weights: 1, 2, 3 (sum = 6)
        // WMA = (104*1 + 105*2 + 106*3) / 6 = (104 + 210 + 318) / 6 = 632/6 = 105.33
        assert!(
            (wma - 105.33).abs() < 0.1,
            "WMA should be ~105.33, got {}",
            wma
        );
    }

    #[test]
    fn rma_wilders_smoothing() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 9,
        };

        let result = ta_rma(&[RayValue::Na, RayValue::Number(5.0)], Some(&ctx));

        // RMA should return a value close to recent prices
        let rma = result.as_number().unwrap();
        assert!(
            rma > 100.0 && rma < 115.0,
            "RMA should be in range, got {}",
            rma
        );
    }

    #[test]
    fn variance_calculates_variance() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 4,
        };

        let result = ta_variance(&[RayValue::Na, RayValue::Number(5.0)], Some(&ctx));

        // Closes: 102, 103, 104, 105, 106
        // Mean: 104
        // Variance: ((−2)² + (−1)² + 0² + 1² + 2²) / 5 = 10/5 = 2
        let variance = result.as_number().unwrap();
        assert!(
            (variance - 2.0).abs() < 0.01,
            "Variance should be 2, got {}",
            variance
        );
    }

    #[test]
    fn mom_equals_change() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 5,
        };

        let change_result = ta_change(&[RayValue::Na, RayValue::Number(3.0)], Some(&ctx));
        let mom_result = ta_mom(&[RayValue::Na, RayValue::Number(3.0)], Some(&ctx));

        // Momentum should equal change
        assert_eq!(change_result, mom_result, "ta.mom should equal ta.change");
    }

    #[test]
    fn roc_calculates_rate_of_change() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 5,
        };

        let result = ta_roc(&[RayValue::Na, RayValue::Number(3.0)], Some(&ctx));

        // ROC = (close - close[n]) / close[n] * 100
        // close[5] = 107, close[2] = 104
        // ROC = (107 - 104) / 104 * 100 = 3/104 * 100 ≈ 2.88
        let roc = result.as_number().unwrap();
        assert!(
            (roc - 2.88).abs() < 0.1,
            "ROC should be ~2.88%, got {}",
            roc
        );
    }

    #[test]
    fn falling_detects_downtrend() {
        // Create bars with falling prices
        let mut bars = BarArray::new();
        bars.set(vec![
            Bar {
                timestamp: 1,
                open: 110.0,
                high: 115.0,
                low: 105.0,
                close: 110.0,
                volume: 100.0,
            },
            Bar {
                timestamp: 2,
                open: 109.0,
                high: 114.0,
                low: 104.0,
                close: 109.0,
                volume: 100.0,
            },
            Bar {
                timestamp: 3,
                open: 108.0,
                high: 113.0,
                low: 103.0,
                close: 108.0,
                volume: 100.0,
            },
            Bar {
                timestamp: 4,
                open: 107.0,
                high: 112.0,
                low: 102.0,
                close: 107.0,
                volume: 100.0,
            },
            Bar {
                timestamp: 5,
                open: 106.0,
                high: 111.0,
                low: 101.0,
                close: 106.0,
                volume: 100.0,
            },
        ])
        .unwrap();

        let ctx = TaContext {
            bars: &bars,
            bar_index: 4,
        };

        let result = ta_falling(&[RayValue::Na, RayValue::Number(3.0)], Some(&ctx));
        assert_eq!(result, RayValue::Bool(true), "Should detect falling trend");
    }

    #[test]
    fn highestbars_returns_bars_since_highest() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 4,
        };

        let result = ta_highestbars(&[RayValue::Na, RayValue::Number(5.0)], Some(&ctx));

        // Highest high is at bar 4 (current bar), so offset should be 0
        assert_eq!(
            result.as_number(),
            Some(0.0),
            "Highest high should be at current bar"
        );
    }

    #[test]
    fn lowestbars_returns_bars_since_lowest() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 4,
        };

        let result = ta_lowestbars(&[RayValue::Na, RayValue::Number(5.0)], Some(&ctx));

        // Lowest low is at bar 0 (4 bars ago), so offset should be -4
        assert_eq!(
            result.as_number(),
            Some(-4.0),
            "Lowest low should be 4 bars ago"
        );
    }

    #[test]
    fn ta_functions_return_na_without_context() {
        // All TA functions should return Na when no context is provided
        assert_eq!(
            ta_sma(&[RayValue::Na, RayValue::Number(5.0)], None),
            RayValue::Na
        );
        assert_eq!(
            ta_ema(&[RayValue::Na, RayValue::Number(5.0)], None),
            RayValue::Na
        );
        assert_eq!(
            ta_rsi(&[RayValue::Na, RayValue::Number(5.0)], None),
            RayValue::Na
        );
        assert_eq!(ta_tr(&[], None), RayValue::Na);
        assert_eq!(ta_atr(&[RayValue::Number(5.0)], None), RayValue::Na);
    }

    #[test]
    fn ta_functions_return_na_with_insufficient_data() {
        let bars = sample_bars();
        let ctx = TaContext {
            bars: &bars,
            bar_index: 2, // Only 3 bars available
        };

        // SMA(10) needs 10 bars
        let result = ta_sma(&[RayValue::Na, RayValue::Number(10.0)], Some(&ctx));
        assert_eq!(
            result,
            RayValue::Na,
            "SMA should return Na with insufficient data"
        );

        // RSI(14) needs 15 bars (14 + 1 for change calculation)
        let result = ta_rsi(&[RayValue::Na, RayValue::Number(14.0)], Some(&ctx));
        assert_eq!(
            result,
            RayValue::Na,
            "RSI should return Na with insufficient data"
        );
    }
}
