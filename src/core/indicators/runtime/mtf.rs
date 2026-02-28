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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtfRequest {
    pub request_id: String,
    pub symbol: String,
    pub chart_timeframe: String,
    pub timeframe: String,
    pub field: String,
    pub mode: MtfMode,
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
}

impl MtfRequestKey {
    pub fn from_request(request: &MtfRequest) -> Self {
        Self {
            symbol: request.symbol.to_ascii_uppercase(),
            chart_timeframe: request.chart_timeframe.to_ascii_lowercase(),
            timeframe: request.timeframe.to_ascii_lowercase(),
            field: request.field.to_ascii_lowercase(),
            mode: request.mode,
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
        let (_, sample) = by_ts.range(..=chart_timestamp).next_back()?;
        Some(sample.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::{MtfMode, MtfRequest, MtfResolvedSample, MtfResolver, SnapshotMtfResolver};

    fn sample_request() -> MtfRequest {
        MtfRequest {
            request_id: "r1".to_string(),
            symbol: "BTCUSD".to_string(),
            chart_timeframe: "1m".to_string(),
            timeframe: "1h".to_string(),
            field: "close".to_string(),
            mode: MtfMode::Confirmed,
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
}
