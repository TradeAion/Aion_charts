use serde::{Deserialize, Serialize};
use std::fmt;

pub const DEFAULT_MAX_INDICATOR_PANES: usize = 32;
pub const DEFAULT_MAX_BARS_PER_LOAD: usize = 250_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardrailViolation {
    WorkspacePaneLimit {
        max_panes: usize,
    },
    IndicatorPaneLimit {
        max_indicator_panes: usize,
    },
    BarLoadLimit {
        requested: usize,
        max_bars_per_load: usize,
    },
    IntervalChangeLocked {
        current_interval: String,
    },
    IntervalNotAllowed {
        interval: String,
        allowed_intervals: Vec<String>,
    },
}

impl fmt::Display for GuardrailViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkspacePaneLimit { max_panes } => {
                write!(f, "workspace pane limit reached (max {max_panes})")
            }
            Self::IndicatorPaneLimit {
                max_indicator_panes,
            } => write!(
                f,
                "indicator pane limit reached (max {max_indicator_panes})"
            ),
            Self::BarLoadLimit {
                requested,
                max_bars_per_load,
            } => write!(
                f,
                "bar load limit exceeded ({requested} requested, max {max_bars_per_load})"
            ),
            Self::IntervalChangeLocked { current_interval } => {
                write!(f, "interval changes are locked to {current_interval}")
            }
            Self::IntervalNotAllowed {
                interval,
                allowed_intervals,
            } => write!(
                f,
                "interval {interval} is not allowed (allowed: {})",
                allowed_intervals.join(", ")
            ),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceGuardrails {
    pub max_panes: Option<usize>,
}

impl WorkspaceGuardrails {
    pub fn set_max_panes(&mut self, max_panes: Option<usize>) {
        self.max_panes = max_panes.filter(|value| *value > 0);
    }

    pub fn can_split(&self, current_panes: usize) -> bool {
        self.enforce_split(current_panes).is_ok()
    }

    pub fn enforce_total_panes(&self, total_panes: usize) -> Result<(), GuardrailViolation> {
        if let Some(max_panes) = self.max_panes {
            if total_panes > max_panes {
                return Err(GuardrailViolation::WorkspacePaneLimit { max_panes });
            }
        }
        Ok(())
    }

    pub fn enforce_split(&self, current_panes: usize) -> Result<(), GuardrailViolation> {
        self.enforce_total_panes(current_panes.saturating_add(1))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChartGuardrails {
    #[serde(default = "default_max_indicator_panes")]
    pub max_indicator_panes: Option<usize>,
    #[serde(default = "default_max_bars_per_load")]
    pub max_bars_per_load: Option<usize>,
    #[serde(default)]
    pub allowed_intervals: Option<Vec<String>>,
    #[serde(default)]
    pub lock_interval_change: bool,
}

impl Default for ChartGuardrails {
    fn default() -> Self {
        Self {
            max_indicator_panes: Some(DEFAULT_MAX_INDICATOR_PANES),
            max_bars_per_load: Some(DEFAULT_MAX_BARS_PER_LOAD),
            allowed_intervals: None,
            lock_interval_change: false,
        }
    }
}

fn default_max_indicator_panes() -> Option<usize> {
    Some(DEFAULT_MAX_INDICATOR_PANES)
}

fn default_max_bars_per_load() -> Option<usize> {
    Some(DEFAULT_MAX_BARS_PER_LOAD)
}

impl ChartGuardrails {
    pub fn set_max_indicator_panes(&mut self, max_indicator_panes: Option<usize>) {
        self.max_indicator_panes = max_indicator_panes.filter(|value| *value > 0);
    }

    pub fn set_max_bars_per_load(&mut self, max_bars_per_load: Option<usize>) {
        self.max_bars_per_load = max_bars_per_load.filter(|value| *value > 0);
    }

    pub fn set_allowed_intervals<I, S>(&mut self, allowed_intervals: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let normalized: Vec<String> = allowed_intervals
            .into_iter()
            .filter_map(|value| normalize_interval(value.as_ref()))
            .collect();
        self.allowed_intervals = (!normalized.is_empty()).then_some(normalized);
    }

    pub fn clear_allowed_intervals(&mut self) {
        self.allowed_intervals = None;
    }

    pub fn set_interval_change_locked(&mut self, locked: bool) {
        self.lock_interval_change = locked;
    }

    pub fn can_add_indicator_pane(&self, current_count: usize) -> bool {
        self.enforce_indicator_pane_add(current_count).is_ok()
    }

    pub fn can_load_bars(&self, requested: usize) -> bool {
        self.enforce_bar_load(requested).is_ok()
    }

    pub fn is_interval_allowed(&self, interval: &str) -> bool {
        self.enforce_interval_allowed(interval).is_ok()
    }

    pub fn can_change_interval(&self, current_interval: &str, requested_interval: &str) -> bool {
        self.enforce_interval_change(current_interval, requested_interval)
            .is_ok()
    }

    pub fn enforce_indicator_pane_total(
        &self,
        total_count: usize,
    ) -> Result<(), GuardrailViolation> {
        if let Some(max_indicator_panes) = self.max_indicator_panes {
            if total_count > max_indicator_panes {
                return Err(GuardrailViolation::IndicatorPaneLimit {
                    max_indicator_panes,
                });
            }
        }
        Ok(())
    }

    pub fn enforce_indicator_pane_add(
        &self,
        current_count: usize,
    ) -> Result<(), GuardrailViolation> {
        self.enforce_indicator_pane_total(current_count.saturating_add(1))
    }

    pub fn enforce_bar_load(&self, requested: usize) -> Result<(), GuardrailViolation> {
        if let Some(max_bars_per_load) = self.max_bars_per_load {
            if requested > max_bars_per_load {
                return Err(GuardrailViolation::BarLoadLimit {
                    requested,
                    max_bars_per_load,
                });
            }
        }
        Ok(())
    }

    pub fn enforce_interval_allowed(&self, interval: &str) -> Result<(), GuardrailViolation> {
        let normalized =
            normalize_interval(interval).unwrap_or_else(|| interval.trim().to_string());
        if let Some(allowed_intervals) = &self.allowed_intervals {
            if !allowed_intervals
                .iter()
                .any(|candidate| candidate == &normalized)
            {
                return Err(GuardrailViolation::IntervalNotAllowed {
                    interval: normalized,
                    allowed_intervals: allowed_intervals.clone(),
                });
            }
        }
        Ok(())
    }

    pub fn enforce_interval_change(
        &self,
        current_interval: &str,
        requested_interval: &str,
    ) -> Result<(), GuardrailViolation> {
        let current_normalized = normalize_interval(current_interval)
            .unwrap_or_else(|| current_interval.trim().to_string());
        let requested_normalized = normalize_interval(requested_interval)
            .unwrap_or_else(|| requested_interval.trim().to_string());

        self.enforce_interval_allowed(&requested_normalized)?;
        if self.lock_interval_change && requested_normalized != current_normalized {
            return Err(GuardrailViolation::IntervalChangeLocked {
                current_interval: current_normalized,
            });
        }
        Ok(())
    }
}

fn normalize_interval(interval: &str) -> Option<String> {
    let trimmed = interval.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::{
        ChartGuardrails, GuardrailViolation, WorkspaceGuardrails, DEFAULT_MAX_BARS_PER_LOAD,
        DEFAULT_MAX_INDICATOR_PANES,
    };

    #[test]
    fn workspace_guardrails_allow_unlimited_splits_by_default() {
        let guardrails = WorkspaceGuardrails::default();
        assert!(guardrails.can_split(1));
        assert!(guardrails.enforce_total_panes(16).is_ok());
    }

    #[test]
    fn workspace_guardrails_block_split_once_max_is_reached() {
        let mut guardrails = WorkspaceGuardrails::default();
        guardrails.set_max_panes(Some(2));

        assert!(guardrails.enforce_split(1).is_ok());
        assert_eq!(
            guardrails.enforce_split(2),
            Err(GuardrailViolation::WorkspacePaneLimit { max_panes: 2 })
        );
    }

    #[test]
    fn chart_guardrails_apply_safe_limits_by_default() {
        let guardrails = ChartGuardrails::default();
        assert!(guardrails.can_add_indicator_pane(4));
        assert!(guardrails.enforce_indicator_pane_total(12).is_ok());
        assert_eq!(
            guardrails.enforce_indicator_pane_total(DEFAULT_MAX_INDICATOR_PANES + 1),
            Err(GuardrailViolation::IndicatorPaneLimit {
                max_indicator_panes: DEFAULT_MAX_INDICATOR_PANES,
            })
        );
        assert_eq!(
            guardrails.enforce_bar_load(DEFAULT_MAX_BARS_PER_LOAD + 1),
            Err(GuardrailViolation::BarLoadLimit {
                requested: DEFAULT_MAX_BARS_PER_LOAD + 1,
                max_bars_per_load: DEFAULT_MAX_BARS_PER_LOAD,
            })
        );
    }

    #[test]
    fn chart_guardrails_block_indicator_pane_additions_at_limit() {
        let mut guardrails = ChartGuardrails::default();
        guardrails.set_max_indicator_panes(Some(2));

        assert!(guardrails.enforce_indicator_pane_add(1).is_ok());
        assert_eq!(
            guardrails.enforce_indicator_pane_add(2),
            Err(GuardrailViolation::IndicatorPaneLimit {
                max_indicator_panes: 2,
            })
        );
    }

    #[test]
    fn chart_guardrails_block_bar_loads_above_limit() {
        let mut guardrails = ChartGuardrails::default();
        guardrails.set_max_bars_per_load(Some(500));

        assert!(guardrails.enforce_bar_load(500).is_ok());
        assert_eq!(
            guardrails.enforce_bar_load(501),
            Err(GuardrailViolation::BarLoadLimit {
                requested: 501,
                max_bars_per_load: 500,
            })
        );
    }

    #[test]
    fn chart_guardrails_allow_only_listed_intervals() {
        let mut guardrails = ChartGuardrails::default();
        guardrails.set_allowed_intervals(["1m", "5m", "1h"]);

        assert!(guardrails.is_interval_allowed("5M"));
        assert_eq!(
            guardrails.enforce_interval_allowed("4h"),
            Err(GuardrailViolation::IntervalNotAllowed {
                interval: "4h".to_string(),
                allowed_intervals: vec!["1m".to_string(), "5m".to_string(), "1h".to_string(),],
            })
        );
    }

    #[test]
    fn chart_guardrails_can_lock_interval_changes() {
        let mut guardrails = ChartGuardrails::default();
        guardrails.set_allowed_intervals(["1m", "5m"]);
        guardrails.set_interval_change_locked(true);

        assert!(guardrails.enforce_interval_change("1m", "1M").is_ok());
        assert_eq!(
            guardrails.enforce_interval_change("1m", "5m"),
            Err(GuardrailViolation::IntervalChangeLocked {
                current_interval: "1m".to_string(),
            })
        );
    }

    #[test]
    fn zero_like_values_clear_limits_through_setters() {
        let mut workspace_guardrails = WorkspaceGuardrails::default();
        workspace_guardrails.set_max_panes(Some(0));
        assert_eq!(workspace_guardrails.max_panes, None);

        let mut chart_guardrails = ChartGuardrails::default();
        chart_guardrails.set_max_indicator_panes(Some(0));
        chart_guardrails.set_max_bars_per_load(Some(0));
        chart_guardrails.set_allowed_intervals(["", "   "]);
        assert_eq!(chart_guardrails.max_indicator_panes, None);
        assert_eq!(chart_guardrails.max_bars_per_load, None);
        assert_eq!(chart_guardrails.allowed_intervals, None);
    }
}
