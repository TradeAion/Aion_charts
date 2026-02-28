use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::RwLock;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum MtfMode {
    Confirmed,
    Live,
}

impl Default for MtfMode {
    fn default() -> Self {
        Self::Confirmed
    }
}

impl MtfMode {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.unwrap_or("confirmed").to_ascii_lowercase().as_str() {
            "live" => Self::Live,
            _ => Self::Confirmed,
        }
    }
}

/// barmerge.gaps_on / barmerge.gaps_off behavior for HTF data.
/// Controls whether na values are inserted when HTF bars don't align with chart bars.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum BarmergeGaps {
    /// gaps_on: Insert na values when no HTF bar aligns with chart bar
    GapsOn,
    /// gaps_off: Forward-fill last HTF value when no alignment (default, matches Pine Script)
    #[default]
    GapsOff,
}

impl BarmergeGaps {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.map(|s| s.to_ascii_lowercase()).as_deref() {
            Some("barmerge.gaps_on") | Some("gaps_on") | Some("true") => Self::GapsOn,
            _ => Self::GapsOff, // Default matches Pine Script
        }
    }
}

/// barmerge.lookahead_on / barmerge.lookahead_off behavior for HTF data.
/// Controls whether script can access future HTF data (dangerous for backtesting).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum BarmergeLookahead {
    /// lookahead_off: Only show HTF values that would have been known at chart bar time (default, safe)
    #[default]
    LookaheadOff,
    /// lookahead_on: Show HTF values even if they wouldn't have been known yet (dangerous)
    LookaheadOn,
}

impl BarmergeLookahead {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.map(|s| s.to_ascii_lowercase()).as_deref() {
            Some("barmerge.lookahead_on") | Some("lookahead_on") | Some("true") => {
                Self::LookaheadOn
            }
            _ => Self::LookaheadOff,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtfRequest {
    pub request_id: String,
    pub symbol: String,
    pub chart_timeframe: String,
    pub timeframe: String,
    pub field: String,
    pub mode: MtfMode,
    /// barmerge gaps behavior
    #[serde(default)]
    pub gaps: BarmergeGaps,
    /// barmerge lookahead behavior
    #[serde(default)]
    pub lookahead: BarmergeLookahead,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtfResolvedSample {
    pub request_id: String,
    pub timestamp: u64,
    pub value: Option<f64>,
    pub source_timeframe: String,
    pub source_bar_open: Option<u64>,
    pub source_bar_close: Option<u64>,
    pub is_confirmed: bool,
}

pub trait MtfResolver: Send + Sync {
    fn resolve(&self, request: &MtfRequest, chart_timestamp: u64) -> Option<MtfResolvedSample>;
}

#[derive(Default)]
pub struct NoopMtfResolver;

impl MtfResolver for NoopMtfResolver {
    fn resolve(&self, _request: &MtfRequest, _chart_timestamp: u64) -> Option<MtfResolvedSample> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MtfRequestKey {
    pub symbol: String,
    pub chart_timeframe: String,
    pub timeframe: String,
    pub field: String,
    pub mode: MtfMode,
    pub gaps: BarmergeGaps,
    pub lookahead: BarmergeLookahead,
}

impl MtfRequestKey {
    pub fn from_request(request: &MtfRequest) -> Self {
        Self {
            symbol: request.symbol.to_ascii_uppercase(),
            chart_timeframe: request.chart_timeframe.to_ascii_lowercase(),
            timeframe: request.timeframe.to_ascii_lowercase(),
            field: request.field.to_ascii_lowercase(),
            mode: request.mode,
            gaps: request.gaps,
            lookahead: request.lookahead,
        }
    }
}

#[derive(Default)]
pub struct SnapshotMtfResolver {
    series: RwLock<HashMap<MtfRequestKey, BTreeMap<u64, MtfResolvedSample>>>,
}

impl SnapshotMtfResolver {
    pub fn clear(&self) {
        if let Ok(mut write) = self.series.write() {
            write.clear();
        }
    }

    pub fn set_series(&self, request: &MtfRequest, samples: Vec<MtfResolvedSample>) {
        let key = MtfRequestKey::from_request(request);
        let mut by_ts = BTreeMap::new();
        for mut sample in samples {
            sample.request_id = request.request_id.clone();
            by_ts.insert(sample.timestamp, sample);
        }
        if let Ok(mut write) = self.series.write() {
            write.insert(key, by_ts);
        }
    }
}

impl MtfResolver for SnapshotMtfResolver {
    fn resolve(&self, request: &MtfRequest, chart_timestamp: u64) -> Option<MtfResolvedSample> {
        let key = MtfRequestKey::from_request(request);
        let guard = self.series.read().ok()?;
        let by_ts = guard.get(&key)?;

        // Find the appropriate HTF sample based on barmerge policies
        let sample = match (request.lookahead, request.gaps) {
            // lookahead_off (safe): Only use HTF values where the HTF bar has closed
            // The sample's source_bar_close must be <= chart_timestamp
            (BarmergeLookahead::LookaheadOff, gaps) => {
                // Find the most recent sample where source_bar_close <= chart_timestamp
                // (i.e., HTF bar was confirmed before this chart bar)
                let confirmed_sample = by_ts
                    .range(..=chart_timestamp)
                    .filter(|(_, s)| {
                        s.is_confirmed || s.source_bar_close.map_or(false, |c| c <= chart_timestamp)
                    })
                    .next_back()
                    .map(|(_, s)| s);

                match gaps {
                    // gaps_off: Forward-fill last known value
                    BarmergeGaps::GapsOff => confirmed_sample.cloned(),
                    // gaps_on: Only return if HTF bar actually closes on this chart bar
                    BarmergeGaps::GapsOn => {
                        // Check if there's an exact match at this timestamp
                        if let Some(exact) = by_ts.get(&chart_timestamp) {
                            if exact.is_confirmed
                                || exact
                                    .source_bar_close
                                    .map_or(false, |c| c <= chart_timestamp)
                            {
                                return Some(exact.clone());
                            }
                        }
                        // No exact match, check if this timestamp is the close of an HTF bar
                        confirmed_sample.and_then(|s| {
                            if s.source_bar_close == Some(chart_timestamp) {
                                Some(s.clone())
                            } else {
                                // Return na (None value but with metadata)
                                Some(MtfResolvedSample {
                                    request_id: request.request_id.clone(),
                                    timestamp: chart_timestamp,
                                    value: None, // na
                                    source_timeframe: request.timeframe.clone(),
                                    source_bar_open: s.source_bar_open,
                                    source_bar_close: s.source_bar_close,
                                    is_confirmed: s.is_confirmed,
                                })
                            }
                        })
                    }
                }
            }

            // lookahead_on (dangerous): Use HTF values even if bar hasn't closed
            // This gives "future knowledge" - the final value of the HTF bar
            (BarmergeLookahead::LookaheadOn, gaps) => {
                // Find any sample that covers this timestamp (source_bar_open <= chart_timestamp < source_bar_close)
                // or the most recent sample at or before chart_timestamp
                let any_sample = by_ts.range(..=chart_timestamp).next_back().map(|(_, s)| s);

                // Also look for future samples that would cover this timestamp
                let future_sample = by_ts.range(chart_timestamp..).next().and_then(|(_, s)| {
                    if s.source_bar_open.map_or(false, |o| o <= chart_timestamp) {
                        Some(s)
                    } else {
                        None
                    }
                });

                // Prefer the future sample if it covers this timestamp
                let sample = future_sample.or(any_sample);

                match gaps {
                    // gaps_off: Forward-fill
                    BarmergeGaps::GapsOff => sample.cloned(),
                    // gaps_on: Only if exact or bar close
                    BarmergeGaps::GapsOn => {
                        if let Some(exact) = by_ts.get(&chart_timestamp) {
                            return Some(exact.clone());
                        }
                        sample.and_then(|s| {
                            if s.source_bar_close == Some(chart_timestamp) {
                                Some(s.clone())
                            } else {
                                Some(MtfResolvedSample {
                                    request_id: request.request_id.clone(),
                                    timestamp: chart_timestamp,
                                    value: None, // na
                                    source_timeframe: request.timeframe.clone(),
                                    source_bar_open: s.source_bar_open,
                                    source_bar_close: s.source_bar_close,
                                    is_confirmed: s.is_confirmed,
                                })
                            }
                        })
                    }
                }
            }
        };

        sample
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BarmergeGaps, BarmergeLookahead, MtfMode, MtfRequest, MtfResolvedSample, MtfResolver,
        SnapshotMtfResolver,
    };

    fn sample_request() -> MtfRequest {
        MtfRequest {
            request_id: "r1".to_string(),
            symbol: "BTCUSD".to_string(),
            chart_timeframe: "1m".to_string(),
            timeframe: "1h".to_string(),
            field: "close".to_string(),
            mode: MtfMode::Confirmed,
            gaps: BarmergeGaps::default(),
            lookahead: BarmergeLookahead::default(),
        }
    }

    #[test]
    fn resolver_returns_previous_or_exact_sample() {
        let resolver = SnapshotMtfResolver::default();
        let request = sample_request();
        resolver.set_series(
            &request,
            vec![
                MtfResolvedSample {
                    request_id: "r1".to_string(),
                    timestamp: 100,
                    value: Some(10.0),
                    source_timeframe: "1h".to_string(),
                    source_bar_open: Some(0),
                    source_bar_close: Some(100),
                    is_confirmed: true,
                },
                MtfResolvedSample {
                    request_id: "r1".to_string(),
                    timestamp: 200,
                    value: Some(20.0),
                    source_timeframe: "1h".to_string(),
                    source_bar_open: Some(100),
                    source_bar_close: Some(200),
                    is_confirmed: true,
                },
            ],
        );

        let exact = resolver
            .resolve(&request, 200)
            .expect("exact sample should resolve");
        assert_eq!(exact.value, Some(20.0));

        let previous = resolver
            .resolve(&request, 150)
            .expect("previous sample should resolve");
        assert_eq!(previous.value, Some(10.0));
    }

    #[test]
    fn resolver_returns_none_before_first_sample() {
        let resolver = SnapshotMtfResolver::default();
        let request = sample_request();
        resolver.set_series(
            &request,
            vec![MtfResolvedSample {
                request_id: "r1".to_string(),
                timestamp: 100,
                value: Some(10.0),
                source_timeframe: "1h".to_string(),
                source_bar_open: Some(0),
                source_bar_close: Some(100),
                is_confirmed: true,
            }],
        );

        assert!(resolver.resolve(&request, 99).is_none());
    }

    #[test]
    fn barmerge_gaps_parse_variants() {
        assert_eq!(BarmergeGaps::parse(None), BarmergeGaps::GapsOff);
        assert_eq!(
            BarmergeGaps::parse(Some("barmerge.gaps_off")),
            BarmergeGaps::GapsOff
        );
        assert_eq!(
            BarmergeGaps::parse(Some("barmerge.gaps_on")),
            BarmergeGaps::GapsOn
        );
        assert_eq!(BarmergeGaps::parse(Some("gaps_on")), BarmergeGaps::GapsOn);
        assert_eq!(BarmergeGaps::parse(Some("true")), BarmergeGaps::GapsOn);
    }

    #[test]
    fn barmerge_lookahead_parse_variants() {
        assert_eq!(
            BarmergeLookahead::parse(None),
            BarmergeLookahead::LookaheadOff
        );
        assert_eq!(
            BarmergeLookahead::parse(Some("barmerge.lookahead_off")),
            BarmergeLookahead::LookaheadOff
        );
        assert_eq!(
            BarmergeLookahead::parse(Some("barmerge.lookahead_on")),
            BarmergeLookahead::LookaheadOn
        );
        assert_eq!(
            BarmergeLookahead::parse(Some("lookahead_on")),
            BarmergeLookahead::LookaheadOn
        );
        assert_eq!(
            BarmergeLookahead::parse(Some("true")),
            BarmergeLookahead::LookaheadOn
        );
    }

    #[test]
    fn gaps_off_forward_fills_htf_value() {
        let resolver = SnapshotMtfResolver::default();
        let mut request = sample_request();
        request.gaps = BarmergeGaps::GapsOff;
        request.lookahead = BarmergeLookahead::LookaheadOff;

        resolver.set_series(
            &request,
            vec![
                MtfResolvedSample {
                    request_id: "r1".to_string(),
                    timestamp: 100,
                    value: Some(10.0),
                    source_timeframe: "1h".to_string(),
                    source_bar_open: Some(0),
                    source_bar_close: Some(100),
                    is_confirmed: true,
                },
                MtfResolvedSample {
                    request_id: "r1".to_string(),
                    timestamp: 200,
                    value: Some(20.0),
                    source_timeframe: "1h".to_string(),
                    source_bar_open: Some(100),
                    source_bar_close: Some(200),
                    is_confirmed: true,
                },
            ],
        );

        // At timestamp 150 (between HTF bar closes), gaps_off should forward-fill with 10.0
        let result = resolver.resolve(&request, 150).expect("should resolve");
        assert_eq!(result.value, Some(10.0), "gaps_off should forward-fill");
    }

    #[test]
    fn gaps_on_returns_na_between_htf_bars() {
        let resolver = SnapshotMtfResolver::default();
        let mut request = sample_request();
        request.gaps = BarmergeGaps::GapsOn;
        request.lookahead = BarmergeLookahead::LookaheadOff;

        resolver.set_series(
            &request,
            vec![
                MtfResolvedSample {
                    request_id: "r1".to_string(),
                    timestamp: 100,
                    value: Some(10.0),
                    source_timeframe: "1h".to_string(),
                    source_bar_open: Some(0),
                    source_bar_close: Some(100),
                    is_confirmed: true,
                },
                MtfResolvedSample {
                    request_id: "r1".to_string(),
                    timestamp: 200,
                    value: Some(20.0),
                    source_timeframe: "1h".to_string(),
                    source_bar_open: Some(100),
                    source_bar_close: Some(200),
                    is_confirmed: true,
                },
            ],
        );

        // At timestamp 150 (between HTF bar closes), gaps_on should return na (None value)
        let result = resolver
            .resolve(&request, 150)
            .expect("should resolve with metadata");
        assert_eq!(
            result.value, None,
            "gaps_on should return na between HTF bars"
        );

        // At timestamp 200 (exact HTF bar close), should return the value
        let exact = resolver.resolve(&request, 200).expect("should resolve");
        assert_eq!(
            exact.value,
            Some(20.0),
            "should have value at HTF bar close"
        );
    }

    #[test]
    fn lookahead_on_uses_future_htf_value() {
        let resolver = SnapshotMtfResolver::default();
        let mut request = sample_request();
        request.gaps = BarmergeGaps::GapsOff;
        request.lookahead = BarmergeLookahead::LookaheadOn;

        resolver.set_series(
            &request,
            vec![
                MtfResolvedSample {
                    request_id: "r1".to_string(),
                    timestamp: 100,
                    value: Some(10.0),
                    source_timeframe: "1h".to_string(),
                    source_bar_open: Some(0),
                    source_bar_close: Some(100),
                    is_confirmed: true,
                },
                MtfResolvedSample {
                    request_id: "r1".to_string(),
                    timestamp: 200,
                    value: Some(20.0),
                    source_timeframe: "1h".to_string(),
                    source_bar_open: Some(100),
                    source_bar_close: Some(200),
                    is_confirmed: true,
                },
            ],
        );

        // At timestamp 150 (within the second HTF bar), lookahead_on should see 20.0
        // because we're looking at a bar that opens at 100 and closes at 200
        let result = resolver.resolve(&request, 150).expect("should resolve");
        assert_eq!(
            result.value,
            Some(20.0),
            "lookahead_on should use future HTF bar value"
        );
    }
}
