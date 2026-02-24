//! ChartGroup module: native multi-chart synchronization primitives.
//!
//! This module provides a graph-based chart grouping system where each pane can
//! be linked to others and synchronization features can be toggled globally,
//! per pane, or per pane-link.

pub mod chart_group;
pub mod pane;
pub mod sync_manager;
