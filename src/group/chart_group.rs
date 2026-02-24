//! Native multi-chart grouping and synchronization.

use std::collections::{HashMap, HashSet, VecDeque};

use super::pane::{ChartPane, ChartPaneId, CrosshairSnapshot, DataRange, TimeRange};
use super::sync_manager::{SyncFeature, SyncManager};

/// Owns panes and synchronization rules for a chart group.
#[derive(Debug, Default, Clone)]
pub struct ChartGroup {
    panes: HashMap<ChartPaneId, ChartPane>,
    links: HashMap<ChartPaneId, HashSet<ChartPaneId>>,
    sync: SyncManager,
    next_pane_id: u64,
    auto_link_new_panes: bool,
}

impl ChartGroup {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    pub fn set_auto_link(&mut self, enabled: bool) {
        self.auto_link_new_panes = enabled;
    }

    pub fn add_pane(
        &mut self,
        symbol: impl Into<String>,
        interval: impl Into<String>,
    ) -> ChartPaneId {
        let id = ChartPaneId(self.next_pane_id);
        self.next_pane_id = self.next_pane_id.saturating_add(1);

        let pane = ChartPane::new(id, symbol, interval);
        self.panes.insert(id, pane);
        self.links.entry(id).or_default();

        if self.auto_link_new_panes {
            let existing: Vec<ChartPaneId> = self
                .panes
                .keys()
                .copied()
                .filter(|pane_id| *pane_id != id)
                .collect();
            for other in existing {
                self.link_panes(&id, &other);
            }
        }

        id
    }

    pub fn remove_pane(&mut self, pane: ChartPaneId) -> bool {
        if !self.panes.contains_key(&pane) {
            return false;
        }

        self.panes.remove(&pane);

        if let Some(neighbors) = self.links.remove(&pane) {
            for neighbor in neighbors {
                if let Some(neighbor_links) = self.links.get_mut(&neighbor) {
                    neighbor_links.remove(&pane);
                }
            }
        }

        true
    }

    pub fn pane(&self, pane: ChartPaneId) -> Option<&ChartPane> {
        self.panes.get(&pane)
    }

    pub fn pane_mut(&mut self, pane: ChartPaneId) -> Option<&mut ChartPane> {
        self.panes.get_mut(&pane)
    }

    pub fn panes(&self) -> impl Iterator<Item = &ChartPane> {
        self.panes.values()
    }

    pub fn link_panes(&mut self, a: &ChartPaneId, b: &ChartPaneId) -> bool {
        if a == b || !self.panes.contains_key(a) || !self.panes.contains_key(b) {
            return false;
        }

        self.links.entry(*a).or_default().insert(*b);
        self.links.entry(*b).or_default().insert(*a);
        true
    }

    pub fn unlink_panes(&mut self, a: &ChartPaneId, b: &ChartPaneId) -> bool {
        let mut removed = false;
        if let Some(links) = self.links.get_mut(a) {
            removed |= links.remove(b);
        }
        if let Some(links) = self.links.get_mut(b) {
            removed |= links.remove(a);
        }
        removed
    }

    pub fn is_linked(&self, a: ChartPaneId, b: ChartPaneId) -> bool {
        self.links
            .get(&a)
            .map(|set| set.contains(&b))
            .unwrap_or(false)
    }

    /// Set a global sync feature toggle. Unknown keys are ignored.
    pub fn set_sync(&mut self, feature: &str, enabled: bool) {
        let _ = self.sync.set_global_by_key(feature, enabled);
    }

    pub fn try_set_sync(&mut self, feature: &str, enabled: bool) -> Result<(), String> {
        self.sync.set_global_by_key(feature, enabled)
    }

    pub fn set_sync_for_pane(
        &mut self,
        pane: ChartPaneId,
        feature: &str,
        enabled: bool,
    ) -> Result<(), String> {
        if !self.panes.contains_key(&pane) {
            return Err(format!("pane {:?} does not exist", pane));
        }
        self.sync.set_for_pane_by_key(pane, feature, enabled)
    }

    pub fn set_sync_for_link(
        &mut self,
        a: ChartPaneId,
        b: ChartPaneId,
        feature: &str,
        enabled: bool,
    ) -> Result<(), String> {
        if !self.is_linked(a, b) {
            return Err(format!("pane {:?} is not linked with {:?}", a, b));
        }
        self.sync.set_for_link_by_key(a, b, feature, enabled)
    }

    pub fn update_symbol(
        &mut self,
        source: ChartPaneId,
        symbol: impl Into<String>,
    ) -> Vec<ChartPaneId> {
        let symbol = symbol.into();
        if let Some(source_pane) = self.panes.get_mut(&source) {
            source_pane.set_symbol(symbol.clone());
        } else {
            return Vec::new();
        }
        self.propagate(source, SyncFeature::Symbol, move |pane| {
            pane.set_symbol(symbol.clone())
        })
    }

    pub fn update_interval(
        &mut self,
        source: ChartPaneId,
        interval: impl Into<String>,
    ) -> Vec<ChartPaneId> {
        let interval = interval.into();
        if let Some(source_pane) = self.panes.get_mut(&source) {
            source_pane.set_interval(interval.clone());
        } else {
            return Vec::new();
        }
        self.propagate(source, SyncFeature::Interval, move |pane| {
            pane.set_interval(interval.clone())
        })
    }

    pub fn update_crosshair(
        &mut self,
        source: ChartPaneId,
        crosshair: CrosshairSnapshot,
    ) -> Vec<ChartPaneId> {
        if let Some(source_pane) = self.panes.get_mut(&source) {
            source_pane.set_crosshair(crosshair);
        } else {
            return Vec::new();
        }
        self.propagate(source, SyncFeature::Crosshair, move |pane| {
            pane.set_crosshair(crosshair)
        })
    }

    pub fn update_time_range(
        &mut self,
        source: ChartPaneId,
        time_range: TimeRange,
    ) -> Vec<ChartPaneId> {
        if let Some(source_pane) = self.panes.get_mut(&source) {
            source_pane.set_time_range(time_range);
        } else {
            return Vec::new();
        }
        self.propagate(source, SyncFeature::Time, move |pane| {
            pane.set_time_range(time_range)
        })
    }

    pub fn update_data_range(
        &mut self,
        source: ChartPaneId,
        data_range: DataRange,
    ) -> Vec<ChartPaneId> {
        if let Some(source_pane) = self.panes.get_mut(&source) {
            source_pane.set_data_range(data_range);
        } else {
            return Vec::new();
        }
        self.propagate(source, SyncFeature::DataRange, move |pane| {
            pane.set_data_range(data_range)
        })
    }

    pub fn sync_manager(&self) -> &SyncManager {
        &self.sync
    }

    fn propagate<F>(
        &mut self,
        source: ChartPaneId,
        feature: SyncFeature,
        mut apply: F,
    ) -> Vec<ChartPaneId>
    where
        F: FnMut(&mut ChartPane) -> bool,
    {
        if !self.panes.contains_key(&source) {
            return Vec::new();
        }

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut changed = Vec::new();

        queue.push_back(source);
        visited.insert(source);

        while let Some(current) = queue.pop_front() {
            let neighbors: Vec<ChartPaneId> = self
                .links
                .get(&current)
                .map(|set| set.iter().copied().collect())
                .unwrap_or_default();

            for target in neighbors {
                if visited.contains(&target) {
                    continue;
                }
                if !self.sync.is_enabled_between(feature, current, target) {
                    continue;
                }
                visited.insert(target);
                queue.push_back(target);
                if let Some(target_pane) = self.panes.get_mut(&target) {
                    if apply(target_pane) {
                        changed.push(target);
                    }
                }
            }
        }

        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::pane::{CrosshairMagnetMode, CrosshairSnapshot};

    #[test]
    fn symbol_sync_propagates_over_links() {
        let mut group = ChartGroup::new();
        let a = group.add_pane("AAPL", "1m");
        let b = group.add_pane("MSFT", "5m");
        let c = group.add_pane("BTC", "1h");

        group.link_panes(&a, &b);
        group.link_panes(&b, &c);

        let changed = group.update_symbol(a, "TSLA");
        assert_eq!(changed.len(), 2);
        assert_eq!(group.pane(a).unwrap().symbol, "TSLA");
        assert_eq!(group.pane(b).unwrap().symbol, "TSLA");
        assert_eq!(group.pane(c).unwrap().symbol, "TSLA");
    }

    #[test]
    fn feature_toggle_blocks_sync() {
        let mut group = ChartGroup::new();
        let a = group.add_pane("AAPL", "1m");
        let b = group.add_pane("MSFT", "5m");
        group.link_panes(&a, &b);

        group.set_sync("interval", false);
        let changed = group.update_interval(a, "15m");
        assert!(changed.is_empty());
        assert_eq!(group.pane(b).unwrap().interval, "5m");
    }

    #[test]
    fn pane_override_blocks_crosshair_for_that_pane() {
        let mut group = ChartGroup::new();
        let a = group.add_pane("AAPL", "1m");
        let b = group.add_pane("MSFT", "1m");
        group.link_panes(&a, &b);
        group.set_sync_for_pane(b, "crosshair", false).unwrap();

        let changed = group.update_crosshair(
            a,
            CrosshairSnapshot {
                active: true,
                x: 10.0,
                y: 20.0,
                bar_index: Some(5.0),
                price: Some(100.0),
                magnet: CrosshairMagnetMode::Ohlc,
            },
        );

        assert!(changed.is_empty());
        assert!(!group.pane(b).unwrap().crosshair.active);
    }

    #[test]
    fn link_override_blocks_data_range_only_on_one_link() {
        let mut group = ChartGroup::new();
        let a = group.add_pane("AAPL", "1m");
        let b = group.add_pane("MSFT", "1m");
        let c = group.add_pane("NVDA", "1m");
        group.link_panes(&a, &b);
        group.link_panes(&a, &c);
        group.set_sync_for_link(a, b, "data_range", false).unwrap();

        group.update_data_range(
            a,
            DataRange {
                from_timestamp: Some(100),
                to_timestamp: Some(500),
            },
        );

        assert_eq!(group.pane(b).unwrap().data_range.from_timestamp, None);
        assert_eq!(group.pane(c).unwrap().data_range.from_timestamp, Some(100));
    }
}
