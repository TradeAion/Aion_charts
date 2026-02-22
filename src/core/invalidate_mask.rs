//! Invalidation system for efficient partial repaints.
//!
//! Based on LWC's `invalidate-mask.ts` - granular per-pane invalidation
//! to minimize unnecessary redraws.

use crate::core::pane::PaneId;
use std::collections::HashMap;

/// Invalidation level determines what needs to be repainted.
/// Higher levels include all work from lower levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(u8)]
pub enum InvalidationLevel {
    /// Nothing needs repainting.
    #[default]
    None = 0,
    /// Only cursor/crosshair moved — repaint top canvas only.
    Cursor = 1,
    /// Data changed — repaint base canvas.
    Light = 2,
    /// Layout/structure changed — full repaint.
    Full = 3,
}

/// Per-pane invalidation state.
#[derive(Debug, Clone, Copy, Default)]
pub struct PaneInvalidation {
    /// What level of invalidation is needed.
    pub level: InvalidationLevel,
    /// Whether to auto-scale the price range.
    pub auto_scale: bool,
}

impl PaneInvalidation {
    /// Create a cursor-level invalidation.
    pub fn cursor() -> Self {
        Self {
            level: InvalidationLevel::Cursor,
            auto_scale: false,
        }
    }

    /// Create a light invalidation (data changed).
    pub fn light() -> Self {
        Self {
            level: InvalidationLevel::Light,
            auto_scale: false,
        }
    }

    /// Create a full invalidation.
    pub fn full() -> Self {
        Self {
            level: InvalidationLevel::Full,
            auto_scale: false,
        }
    }

    /// Create a full invalidation with auto-scale.
    pub fn full_with_auto_scale() -> Self {
        Self {
            level: InvalidationLevel::Full,
            auto_scale: true,
        }
    }
}

/// Merge two pane invalidations, taking the maximum of each field.
fn merge_pane_invalidation(a: Option<PaneInvalidation>, b: PaneInvalidation) -> PaneInvalidation {
    match a {
        None => b,
        Some(existing) => PaneInvalidation {
            level: existing.level.max(b.level),
            auto_scale: existing.auto_scale || b.auto_scale,
        },
    }
}

/// Tracks what needs to be repainted across all panes.
///
/// The mask accumulates invalidations during event processing, then
/// the render loop consumes it to determine what to repaint.
#[derive(Debug, Clone, Default)]
pub struct InvalidateMask {
    /// Per-pane invalidation state, keyed by pane index.
    invalidated_panes: HashMap<u32, PaneInvalidation>,
    /// Global invalidation level (applies to all panes).
    global_level: InvalidationLevel,
    /// Force time axis repaint.
    time_scale: bool,
}

impl InvalidateMask {
    /// Create an empty mask (nothing invalidated).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a cursor-only invalidation for all panes.
    pub fn cursor() -> Self {
        Self {
            global_level: InvalidationLevel::Cursor,
            ..Default::default()
        }
    }

    /// Create a light invalidation for all panes.
    pub fn light() -> Self {
        Self {
            global_level: InvalidationLevel::Light,
            ..Default::default()
        }
    }

    /// Create a full invalidation for all panes.
    pub fn full() -> Self {
        Self {
            global_level: InvalidationLevel::Full,
            time_scale: true,
            ..Default::default()
        }
    }

    /// Invalidate a specific pane.
    pub fn invalidate_pane(&mut self, pane_index: u32, invalidation: PaneInvalidation) {
        let merged = merge_pane_invalidation(
            self.invalidated_panes.get(&pane_index).copied(),
            invalidation,
        );
        self.invalidated_panes.insert(pane_index, merged);
    }

    /// Invalidate a pane by ID.
    pub fn invalidate_pane_by_id(&mut self, pane_id: PaneId, invalidation: PaneInvalidation) {
        self.invalidate_pane(pane_id.0, invalidation);
    }

    /// Set global invalidation level (applies to all panes).
    pub fn set_global_level(&mut self, level: InvalidationLevel) {
        self.global_level = self.global_level.max(level);
    }

    /// Mark time scale as needing repaint.
    pub fn invalidate_time_scale(&mut self) {
        self.time_scale = true;
    }

    /// Get the effective invalidation level for a pane.
    pub fn invalidate_for_pane(&self, pane_index: u32) -> PaneInvalidation {
        let pane_invalidation = self.invalidated_panes.get(&pane_index).copied();
        let pane_level = pane_invalidation
            .map(|p| p.level)
            .unwrap_or(InvalidationLevel::None);

        PaneInvalidation {
            level: self.global_level.max(pane_level),
            auto_scale: pane_invalidation.map(|p| p.auto_scale).unwrap_or(false),
        }
    }

    /// Get the global invalidation level.
    pub fn global_level(&self) -> InvalidationLevel {
        self.global_level
    }

    /// Check if time scale needs repaint.
    pub fn time_scale_invalidated(&self) -> bool {
        self.time_scale || self.global_level >= InvalidationLevel::Light
    }

    /// Check if anything needs repainting.
    pub fn needs_repaint(&self) -> bool {
        self.global_level > InvalidationLevel::None
            || !self.invalidated_panes.is_empty()
            || self.time_scale
    }

    /// Merge another mask into this one.
    pub fn merge(&mut self, other: &InvalidateMask) {
        self.global_level = self.global_level.max(other.global_level);
        self.time_scale = self.time_scale || other.time_scale;
        for (&pane_idx, &invalidation) in &other.invalidated_panes {
            self.invalidate_pane(pane_idx, invalidation);
        }
    }

    /// Reset the mask after rendering.
    pub fn reset(&mut self) {
        self.invalidated_panes.clear();
        self.global_level = InvalidationLevel::None;
        self.time_scale = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalidation_levels_order() {
        assert!(InvalidationLevel::None < InvalidationLevel::Cursor);
        assert!(InvalidationLevel::Cursor < InvalidationLevel::Light);
        assert!(InvalidationLevel::Light < InvalidationLevel::Full);
    }

    #[test]
    fn test_merge_pane_invalidation() {
        let a = PaneInvalidation::cursor();
        let b = PaneInvalidation::light();
        let merged = merge_pane_invalidation(Some(a), b);
        assert_eq!(merged.level, InvalidationLevel::Light);
    }

    #[test]
    fn test_global_level_applies_to_all() {
        let mut mask = InvalidateMask::new();
        mask.set_global_level(InvalidationLevel::Light);

        let inv = mask.invalidate_for_pane(42);
        assert_eq!(inv.level, InvalidationLevel::Light);
    }
}
