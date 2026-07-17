//! Boundary data validation & sanitization (roadmap Phase A3).
//!
//! Real market feeds are messy: out-of-order rows, duplicate timestamps, NaN/Infinity, absurd
//! magnitudes, mismatched array lengths. The rest of the engine ([`super::data_layer::DataLayer`]
//! and [`super::plot_list::PlotList`]) assumes **ascending, unique, finite** input — its ordering
//! guard is a `debug_assert!` that is compiled out of the release wasm build, so bad data there
//! would silently corrupt indices or panic in `reindex_all`.
//!
//! This module is the single choke point that makes that assumption safe to hold. Unlike
//! lightweight-charts' `data-validators.ts` (which only `assert`s in dev builds and throws in
//! prod), we *repair* what we can and *report* what we changed, so a production embedder gets a
//! rendered chart plus a diagnostic instead of a thrown error or a dead canvas.
//!
//! Repair policy, in order:
//! 1. **Length mismatch** between the time and value columns is unrecoverable → [`Err`].
//! 2. **Non-finite / out-of-safe-range** rows (NaN, ±Inf, |v| beyond [`MAX_SAFE_VALUE`], or a
//!    non-finite time) are dropped and counted.
//! 3. **Unordered** rows are stably sorted by time (`reordered` flagged).
//! 4. **Duplicate** timestamps collapse **last-wins** (the last occurrence in the *original*
//!    input for that timestamp survives — matching a streaming `update()` overwriting a bar).

/// LWC's safe magnitude bound (`data-validators.ts`): `Number.MAX_SAFE_INTEGER / 100`.
pub const MAX_SAFE_VALUE: f64 = 9_007_199_254_740_991.0 / 100.0;
/// Symmetric lower bound.
pub const MIN_SAFE_VALUE: f64 = -MAX_SAFE_VALUE;

/// What the sanitizer had to change to make the data ingestible.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    /// Rows dropped for a non-finite / out-of-range time or value.
    pub dropped_invalid: usize,
    /// Rows discarded because a later row shared their timestamp (last-wins).
    pub dropped_duplicate: usize,
    /// The input was not already ascending and had to be sorted.
    pub reordered: bool,
    /// Rows that made it into the sanitized output.
    pub accepted: usize,
}

impl ValidationReport {
    /// True when the input was already clean (nothing dropped or reordered).
    pub fn is_clean(&self) -> bool {
        self.dropped_invalid == 0 && self.dropped_duplicate == 0 && !self.reordered
    }
}

/// Structural problems the sanitizer cannot repair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// The time column and the value columns have differing lengths.
    LengthMismatch { times: usize, open: usize, high: usize, low: usize, close: usize },
}

impl core::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ValidationError::LengthMismatch { times, open, high, low, close } => write!(
                f,
                "time/OHLC arrays must have equal length (times={times}, open={open}, high={high}, low={low}, close={close})"
            ),
        }
    }
}

/// Ascending, unique, finite OHLC rows ready for [`super::data_layer::DataLayer::set_data`].
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SanitizedOhlc {
    pub times: Vec<i64>,
    pub open: Vec<f64>,
    pub high: Vec<f64>,
    pub low: Vec<f64>,
    pub close: Vec<f64>,
    pub report: ValidationReport,
}

fn safe(v: f64) -> bool {
    v.is_finite() && (MIN_SAFE_VALUE..=MAX_SAFE_VALUE).contains(&v)
}

/// Sanitize parallel time/OHLC columns into ascending, unique, finite rows.
///
/// `times` are wall-clock seconds as `f64` at the JS boundary; each is truncated toward zero to an
/// `i64` time key (a non-finite time drops the row). Single-value series pass the same value in all
/// four columns, so this covers line/area/histogram too.
pub fn sanitize_ohlc(
    times: &[f64],
    open: &[f64],
    high: &[f64],
    low: &[f64],
    close: &[f64],
) -> Result<SanitizedOhlc, ValidationError> {
    let n = times.len();
    if open.len() != n || high.len() != n || low.len() != n || close.len() != n {
        return Err(ValidationError::LengthMismatch {
            times: n,
            open: open.len(),
            high: high.len(),
            low: low.len(),
            close: close.len(),
        });
    }

    let mut report = ValidationReport::default();

    // 1. Keep only finite, in-range rows; remember original order for stable sort + last-wins.
    let mut rows: Vec<(i64, [f64; 4], usize)> = Vec::with_capacity(n);
    for i in 0..n {
        let t = times[i];
        let (o, h, l, c) = (open[i], high[i], low[i], close[i]);
        if !t.is_finite() || !(safe(o) && safe(h) && safe(l) && safe(c)) {
            report.dropped_invalid += 1;
            continue;
        }
        rows.push((t as i64, [o, h, l, c], i));
    }

    // 2. Detect out-of-order before sorting (so `reordered` reflects the caller's input).
    report.reordered = rows.windows(2).any(|w| w[0].0 > w[1].0);
    if report.reordered {
        // Stable by time so that, within a duplicate group, original order is preserved and the
        // last original occurrence is the one we keep below.
        rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)));
    }

    // 3. Collapse duplicate timestamps, last-wins. `rows` is time-ascending; equal times are in
    //    ascending original-index order, so the last of each run is the latest-provided bar.
    let mut out = SanitizedOhlc::default();
    out.times.reserve(rows.len());
    for (t, v, _) in rows {
        if out.times.last() == Some(&t) {
            report.dropped_duplicate += 1;
            let last = out.times.len() - 1;
            out.open[last] = v[0];
            out.high[last] = v[1];
            out.low[last] = v[2];
            out.close[last] = v[3];
        } else {
            out.times.push(t);
            out.open.push(v[0]);
            out.high.push(v[1]);
            out.low.push(v[2]);
            out.close.push(v[3]);
        }
    }

    report.accepted = out.times.len();
    out.report = report;
    Ok(out)
}

/// Owned-input variant used by typed-array hosts. Clean integer-timestamp feeds take ownership of
/// their columns without the intermediate row matrix; malformed or fractional feeds fall back to
/// the fully repairing sanitizer. This keeps the common ingestion path to one JS→WASM copy.
pub fn sanitize_ohlc_owned(
    times: Vec<f64>,
    open: Vec<f64>,
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
) -> Result<SanitizedOhlc, ValidationError> {
    let n = times.len();
    if open.len() != n || high.len() != n || low.len() != n || close.len() != n {
        return Err(ValidationError::LengthMismatch {
            times: n,
            open: open.len(),
            high: high.len(),
            low: low.len(),
            close: close.len(),
        });
    }
    let clean = times.windows(2).all(|w| w[0].is_finite() && w[0].fract() == 0.0 && w[0] < w[1])
        && times.last().map(|t| t.is_finite() && t.fract() == 0.0).unwrap_or(true)
        && open.iter().chain(&high).chain(&low).chain(&close).copied().all(safe);
    if clean {
        let accepted = times.len();
        return Ok(SanitizedOhlc {
            times: times.into_iter().map(|t| t as i64).collect(),
            open,
            high,
            low,
            close,
            report: ValidationReport { accepted, ..ValidationReport::default() },
        });
    }
    sanitize_ohlc(&times, &open, &high, &low, &close)
}

/// Sanitize a single streaming point. Returns `None` (with no effect on the chart) when the point
/// is non-finite or out of range, so a bad tick is dropped instead of corrupting the series.
pub fn sanitize_point(time: f64, values: [f64; 4]) -> Option<(i64, [f64; 4])> {
    if !time.is_finite() || !values.iter().copied().all(safe) {
        return None;
    }
    Some((time as i64, values))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ohlc(times: &[f64], v: &[f64]) -> Result<SanitizedOhlc, ValidationError> {
        // single-value convenience: all four columns equal
        sanitize_ohlc(times, v, v, v, v)
    }

    #[test]
    fn clean_data_passes_through_untouched() {
        let s = ohlc(&[1.0, 2.0, 3.0], &[10.0, 20.0, 30.0]).unwrap();
        assert_eq!(s.times, [1, 2, 3]);
        assert_eq!(s.close, [10.0, 20.0, 30.0]);
        assert!(s.report.is_clean());
        assert_eq!(s.report.accepted, 3);
    }

    #[test]
    fn length_mismatch_is_an_error() {
        let err = sanitize_ohlc(&[1.0, 2.0], &[1.0], &[1.0], &[1.0], &[1.0]).unwrap_err();
        assert!(matches!(err, ValidationError::LengthMismatch { times: 2, open: 1, .. }));
    }

    #[test]
    fn drops_non_finite_and_out_of_range() {
        let times = [1.0, 2.0, 3.0, 4.0, 5.0];
        let vals = [10.0, f64::NAN, f64::INFINITY, MAX_SAFE_VALUE * 2.0, 50.0];
        let s = ohlc(&times, &vals).unwrap();
        assert_eq!(s.times, [1, 5]); // rows 2,3,4 dropped
        assert_eq!(s.close, [10.0, 50.0]);
        assert_eq!(s.report.dropped_invalid, 3);
    }

    #[test]
    fn drops_row_with_non_finite_time() {
        let s = ohlc(&[1.0, f64::NAN, 3.0], &[10.0, 20.0, 30.0]).unwrap();
        assert_eq!(s.times, [1, 3]);
        assert_eq!(s.report.dropped_invalid, 1);
    }

    #[test]
    fn sorts_unordered_input() {
        let s = ohlc(&[3.0, 1.0, 2.0], &[30.0, 10.0, 20.0]).unwrap();
        assert_eq!(s.times, [1, 2, 3]);
        assert_eq!(s.close, [10.0, 20.0, 30.0]);
        assert!(s.report.reordered);
        assert_eq!(s.report.dropped_duplicate, 0);
    }

    #[test]
    fn dedupes_last_wins_in_order() {
        // duplicate time 2 appears twice; the later value (25) must win
        let s = ohlc(&[1.0, 2.0, 2.0, 3.0], &[10.0, 20.0, 25.0, 30.0]).unwrap();
        assert_eq!(s.times, [1, 2, 3]);
        assert_eq!(s.close, [10.0, 25.0, 30.0]);
        assert_eq!(s.report.dropped_duplicate, 1);
        assert!(!s.report.reordered);
    }

    #[test]
    fn dedupes_last_wins_after_sort() {
        // out of order AND duplicated: original indices break the tie so the later input wins
        // times: 2(a=20) , 1(=10) , 2(b=99) -> sorted stable: 1, 2a, 2b -> keep 2b
        let s = ohlc(&[2.0, 1.0, 2.0], &[20.0, 10.0, 99.0]).unwrap();
        assert_eq!(s.times, [1, 2]);
        assert_eq!(s.close, [10.0, 99.0]);
        assert!(s.report.reordered);
        assert_eq!(s.report.dropped_duplicate, 1);
    }

    #[test]
    fn empty_input_is_clean_empty_output() {
        let s = ohlc(&[], &[]).unwrap();
        assert!(s.times.is_empty());
        assert!(s.report.is_clean());
        assert_eq!(s.report.accepted, 0);
    }

    #[test]
    fn full_ohlc_columns_are_kept_independent() {
        let s = sanitize_ohlc(&[1.0, 2.0], &[1.0, 2.0], &[5.0, 6.0], &[0.5, 1.5], &[3.0, 4.0]).unwrap();
        assert_eq!(s.open, [1.0, 2.0]);
        assert_eq!(s.high, [5.0, 6.0]);
        assert_eq!(s.low, [0.5, 1.5]);
        assert_eq!(s.close, [3.0, 4.0]);
    }

    #[test]
    fn truncates_fractional_seconds_to_i64() {
        let s = ohlc(&[1.9, 2.4], &[10.0, 20.0]).unwrap();
        assert_eq!(s.times, [1, 2]);
    }

    #[test]
    fn sanitize_point_rejects_bad_ticks() {
        assert!(sanitize_point(f64::NAN, [1.0, 1.0, 1.0, 1.0]).is_none());
        assert!(sanitize_point(1.0, [1.0, f64::INFINITY, 1.0, 1.0]).is_none());
        assert_eq!(sanitize_point(1.5, [1.0, 2.0, 0.5, 1.5]), Some((1, [1.0, 2.0, 0.5, 1.5])));
    }

    #[test]
    fn owned_clean_input_avoids_repair_path() {
        let s = sanitize_ohlc_owned(vec![1.0, 2.0], vec![1.0, 2.0], vec![2.0, 3.0], vec![0.0, 1.0], vec![1.5, 2.5]).unwrap();
        assert!(s.report.is_clean());
        assert_eq!(s.times, [1, 2]);
    }
}
