use crate::core::indicators::runtime::value::RayValue;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Default maximum bars of history to keep per variable.
pub const DEFAULT_MAX_BARS_BACK: usize = 500;

/// Ring buffer for variable history, enabling `myVar[N]` lookups.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarSeries {
    buffer: VecDeque<RayValue>,
    capacity: usize,
}

impl VarSeries {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity.min(DEFAULT_MAX_BARS_BACK)),
            capacity,
        }
    }

    pub fn with_default_capacity() -> Self {
        Self::new(DEFAULT_MAX_BARS_BACK)
    }

    /// Push a new value for the current bar. Called at end of each bar.
    pub fn push(&mut self, value: RayValue) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(value);
    }

    /// Get value at offset from most recent (0 = current, 1 = previous, etc.)
    pub fn get(&self, offset: usize) -> Option<&RayValue> {
        if offset >= self.buffer.len() {
            return None;
        }
        let index = self.buffer.len().saturating_sub(1).saturating_sub(offset);
        self.buffer.get(index)
    }

    /// Get the most recent value (offset 0).
    pub fn current(&self) -> Option<&RayValue> {
        self.buffer.back()
    }

    /// Update the most recent value (for same-bar reassignments).
    pub fn update_current(&mut self, value: RayValue) {
        if let Some(last) = self.buffer.back_mut() {
            *last = value;
        } else {
            self.push(value);
        }
    }

    /// Number of bars stored.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl Default for VarSeries {
    fn default() -> Self {
        Self::with_default_capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::VarSeries;
    use crate::core::indicators::runtime::value::RayValue;

    #[test]
    fn push_and_get_at_offset() {
        let mut series = VarSeries::new(10);
        series.push(RayValue::Number(1.0));
        series.push(RayValue::Number(2.0));
        series.push(RayValue::Number(3.0));

        assert_eq!(series.get(0), Some(&RayValue::Number(3.0)));
        assert_eq!(series.get(1), Some(&RayValue::Number(2.0)));
        assert_eq!(series.get(2), Some(&RayValue::Number(1.0)));
        assert_eq!(series.get(3), None);
    }

    #[test]
    fn respects_capacity_limit() {
        let mut series = VarSeries::new(3);
        for i in 0..5 {
            series.push(RayValue::Number(i as f64));
        }

        assert_eq!(series.len(), 3);
        assert_eq!(series.get(0), Some(&RayValue::Number(4.0)));
        assert_eq!(series.get(1), Some(&RayValue::Number(3.0)));
        assert_eq!(series.get(2), Some(&RayValue::Number(2.0)));
    }

    #[test]
    fn update_current_modifies_last_value() {
        let mut series = VarSeries::new(10);
        series.push(RayValue::Number(1.0));
        series.push(RayValue::Number(2.0));
        series.update_current(RayValue::Number(99.0));

        assert_eq!(series.get(0), Some(&RayValue::Number(99.0)));
        assert_eq!(series.get(1), Some(&RayValue::Number(1.0)));
        assert_eq!(series.len(), 2);
    }

    #[test]
    fn current_returns_most_recent() {
        let mut series = VarSeries::new(10);
        assert_eq!(series.current(), None);

        series.push(RayValue::Number(42.0));
        assert_eq!(series.current(), Some(&RayValue::Number(42.0)));
    }
}
