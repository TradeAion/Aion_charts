//! Sync feature policy resolution for `ChartGroup`.

use std::collections::HashMap;

use super::pane::ChartPaneId;

/// Supported synchronization channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncFeature {
    Symbol,
    Interval,
    Crosshair,
    Time,
    DataRange,
}

impl SyncFeature {
    pub const ALL: [SyncFeature; 5] = [
        SyncFeature::Symbol,
        SyncFeature::Interval,
        SyncFeature::Crosshair,
        SyncFeature::Time,
        SyncFeature::DataRange,
    ];

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "symbol" => Some(Self::Symbol),
            "interval" | "timeframe" => Some(Self::Interval),
            "crosshair" => Some(Self::Crosshair),
            "time" | "scroll" | "zoom" | "visible_range" => Some(Self::Time),
            "data_range" | "data" | "history" => Some(Self::DataRange),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FeatureFlags {
    symbol: bool,
    interval: bool,
    crosshair: bool,
    time: bool,
    data_range: bool,
}

impl FeatureFlags {
    fn all(enabled: bool) -> Self {
        Self {
            symbol: enabled,
            interval: enabled,
            crosshair: enabled,
            time: enabled,
            data_range: enabled,
        }
    }

    fn get(&self, feature: SyncFeature) -> bool {
        match feature {
            SyncFeature::Symbol => self.symbol,
            SyncFeature::Interval => self.interval,
            SyncFeature::Crosshair => self.crosshair,
            SyncFeature::Time => self.time,
            SyncFeature::DataRange => self.data_range,
        }
    }

    fn set(&mut self, feature: SyncFeature, enabled: bool) {
        match feature {
            SyncFeature::Symbol => self.symbol = enabled,
            SyncFeature::Interval => self.interval = enabled,
            SyncFeature::Crosshair => self.crosshair = enabled,
            SyncFeature::Time => self.time = enabled,
            SyncFeature::DataRange => self.data_range = enabled,
        }
    }
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self::all(true)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinkKey {
    pub a: ChartPaneId,
    pub b: ChartPaneId,
}

impl LinkKey {
    pub fn new(a: ChartPaneId, b: ChartPaneId) -> Self {
        if a <= b {
            Self { a, b }
        } else {
            Self { a: b, b: a }
        }
    }
}

/// Resolves whether synchronization is active for a feature between two panes.
///
/// Resolution order:
/// 1. global feature toggle
/// 2. source pane feature toggle
/// 3. target pane feature toggle
/// 4. link (pair) feature toggle
#[derive(Debug, Default, Clone)]
pub struct SyncManager {
    global: FeatureFlags,
    pane_overrides: HashMap<ChartPaneId, FeatureFlags>,
    link_overrides: HashMap<LinkKey, FeatureFlags>,
}

impl SyncManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_global_enabled(&self, feature: SyncFeature) -> bool {
        self.global.get(feature)
    }

    pub fn set_global(&mut self, feature: SyncFeature, enabled: bool) {
        self.global.set(feature, enabled);
    }

    pub fn set_global_by_key(&mut self, feature: &str, enabled: bool) -> Result<(), String> {
        let feature = SyncFeature::from_key(feature)
            .ok_or_else(|| format!("unknown sync feature: {}", feature))?;
        self.set_global(feature, enabled);
        Ok(())
    }

    pub fn set_for_pane(&mut self, pane: ChartPaneId, feature: SyncFeature, enabled: bool) {
        let entry = self.pane_overrides.entry(pane).or_default();
        entry.set(feature, enabled);
    }

    pub fn set_for_pane_by_key(
        &mut self,
        pane: ChartPaneId,
        feature: &str,
        enabled: bool,
    ) -> Result<(), String> {
        let feature = SyncFeature::from_key(feature)
            .ok_or_else(|| format!("unknown sync feature: {}", feature))?;
        self.set_for_pane(pane, feature, enabled);
        Ok(())
    }

    pub fn set_for_link(
        &mut self,
        a: ChartPaneId,
        b: ChartPaneId,
        feature: SyncFeature,
        enabled: bool,
    ) {
        let entry = self.link_overrides.entry(LinkKey::new(a, b)).or_default();
        entry.set(feature, enabled);
    }

    pub fn set_for_link_by_key(
        &mut self,
        a: ChartPaneId,
        b: ChartPaneId,
        feature: &str,
        enabled: bool,
    ) -> Result<(), String> {
        let feature = SyncFeature::from_key(feature)
            .ok_or_else(|| format!("unknown sync feature: {}", feature))?;
        self.set_for_link(a, b, feature, enabled);
        Ok(())
    }

    pub fn is_enabled_between(
        &self,
        feature: SyncFeature,
        source: ChartPaneId,
        target: ChartPaneId,
    ) -> bool {
        if !self.global.get(feature) {
            return false;
        }

        if !self
            .pane_overrides
            .get(&source)
            .map(|f| f.get(feature))
            .unwrap_or(true)
        {
            return false;
        }

        if !self
            .pane_overrides
            .get(&target)
            .map(|f| f.get(feature))
            .unwrap_or(true)
        {
            return false;
        }

        self.link_overrides
            .get(&LinkKey::new(source, target))
            .map(|f| f.get(feature))
            .unwrap_or(true)
    }
}
