//! Public price-scale state accessors (mode/autoscale/inversion/margins/visible range) for
//! pane, left/right and overlay scales, plus per-series scale binding. Extracted from `lib.rs`.

use super::*;

impl ChartEngine {
    pub fn price_scale_for(
        &self,
        pane: usize,
        target: PriceScaleTarget,
    ) -> Option<&PriceScaleCore> {
        let pane = self.panes.get(pane)?;
        Some(match target {
            PriceScaleTarget::Right => &pane.price_scale,
            PriceScaleTarget::Left => &pane.left_scale,
            PriceScaleTarget::Overlay => &pane.overlay_scale,
        })
    }

    pub fn price_scale_for_mut(
        &mut self,
        pane: usize,
        target: PriceScaleTarget,
    ) -> Option<&mut PriceScaleCore> {
        let pane = self.panes.get_mut(pane)?;
        Some(match target {
            PriceScaleTarget::Right => &mut pane.price_scale,
            PriceScaleTarget::Left => &mut pane.left_scale,
            PriceScaleTarget::Overlay => &mut pane.overlay_scale,
        })
    }

    /// Current visible raw-value range for a pane price scale.
    pub fn price_scale_visible_range(&self, pane: usize, overlay: bool) -> Option<(f64, f64)> {
        self.price_scale_visible_range_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
        )
    }

    pub fn price_scale_visible_range_for(
        &self,
        pane: usize,
        target: PriceScaleTarget,
    ) -> Option<(f64, f64)> {
        let range = self.price_scale_for(pane, target)?.price_range_for_api()?;
        Some((range.min_value(), range.max_value()))
    }

    /// Install a manual raw-value range and disable autoscale, matching LWC `setVisibleRange`.
    pub fn set_price_scale_visible_range(
        &mut self,
        pane: usize,
        overlay: bool,
        from: f64,
        to: f64,
    ) {
        self.set_price_scale_visible_range_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
            from,
            to,
        );
    }

    pub fn set_price_scale_visible_range_for(
        &mut self,
        pane: usize,
        target: PriceScaleTarget,
        from: f64,
        to: f64,
    ) {
        if !from.is_finite() || !to.is_finite() || from >= to {
            return;
        }
        if let Some(scale) = self.price_scale_for_mut(pane, target) {
            scale.set_auto_scale(false);
            let range = scale.price_range_from_api(&PriceRange::new(from, to));
            scale.set_price_range(Some(range));
        }
    }

    pub fn price_scale_auto_scale(&self, pane: usize, overlay: bool) -> Option<bool> {
        self.price_scale_auto_scale_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
        )
    }

    pub fn price_scale_auto_scale_for(
        &self,
        pane: usize,
        target: PriceScaleTarget,
    ) -> Option<bool> {
        Some(self.price_scale_for(pane, target)?.is_auto_scale())
    }

    pub fn set_price_scale_auto_scale(&mut self, pane: usize, overlay: bool, enabled: bool) {
        self.set_price_scale_auto_scale_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
            enabled,
        );
    }

    pub fn set_price_scale_auto_scale_for(
        &mut self,
        pane: usize,
        target: PriceScaleTarget,
        enabled: bool,
    ) {
        if let Some(scale) = self.price_scale_for_mut(pane, target) {
            scale.set_auto_scale(enabled);
        }
    }

    pub fn price_scale_inverted(&self, pane: usize, overlay: bool) -> Option<bool> {
        self.price_scale_inverted_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
        )
    }

    pub fn price_scale_inverted_for(&self, pane: usize, target: PriceScaleTarget) -> Option<bool> {
        Some(self.price_scale_for(pane, target)?.is_inverted())
    }

    pub fn set_price_scale_inverted(&mut self, pane: usize, overlay: bool, inverted: bool) {
        self.set_price_scale_inverted_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
            inverted,
        );
    }

    pub fn set_price_scale_inverted_for(
        &mut self,
        pane: usize,
        target: PriceScaleTarget,
        inverted: bool,
    ) {
        if let Some(scale) = self.price_scale_for_mut(pane, target) {
            scale.set_invert_scale(inverted);
        }
    }

    pub fn price_scale_margins(&self, pane: usize, overlay: bool) -> Option<(f64, f64)> {
        self.price_scale_margins_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
        )
    }

    pub fn price_scale_margins_for(
        &self,
        pane: usize,
        target: PriceScaleTarget,
    ) -> Option<(f64, f64)> {
        let margins = self.price_scale_for(pane, target)?.options().scale_margins;
        Some((margins.top, margins.bottom))
    }

    pub fn set_price_scale_margins(&mut self, pane: usize, overlay: bool, top: f64, bottom: f64) {
        self.set_price_scale_margins_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
            top,
            bottom,
        );
    }

    pub fn set_price_scale_margins_for(
        &mut self,
        pane: usize,
        target: PriceScaleTarget,
        top: f64,
        bottom: f64,
    ) {
        if !top.is_finite()
            || !bottom.is_finite()
            || top < 0.0
            || bottom < 0.0
            || top > 1.0
            || bottom > 1.0
            || top + bottom > 1.0
        {
            return;
        }
        if let Some(scale) = self.price_scale_for_mut(pane, target) {
            scale.set_scale_margins(top, bottom);
        }
    }

    pub fn price_scale_mode(&self, pane: usize, overlay: bool) -> Option<PriceScaleMode> {
        self.price_scale_mode_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
        )
    }

    pub fn price_scale_mode_for(
        &self,
        pane: usize,
        target: PriceScaleTarget,
    ) -> Option<PriceScaleMode> {
        Some(self.price_scale_for(pane, target)?.mode())
    }

    pub fn set_price_scale_mode(&mut self, pane: usize, overlay: bool, mode: PriceScaleMode) {
        self.set_price_scale_mode_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
            mode,
        );
    }

    pub fn set_price_scale_mode_for(
        &mut self,
        pane: usize,
        target: PriceScaleTarget,
        mode: PriceScaleMode,
    ) {
        if let Some(scale) = self.price_scale_for_mut(pane, target) {
            scale.set_mode(mode);
        }
    }

    pub fn set_series_price_scale(&mut self, id: SeriesId, target: PriceScaleTarget) {
        if let Some(series) = self.series.iter_mut().find(|series| series.id == id) {
            series.overlay = target == PriceScaleTarget::Overlay;
            series.left_scale = target == PriceScaleTarget::Left;
        }
    }

    pub fn series_price_scale(&self, id: SeriesId) -> Option<(usize, PriceScaleTarget)> {
        self.series
            .iter()
            .find(|series| series.id == id)
            .map(|series| {
                let target = if series.overlay {
                    PriceScaleTarget::Overlay
                } else if series.left_scale {
                    PriceScaleTarget::Left
                } else {
                    PriceScaleTarget::Right
                };
                (series.pane_index, target)
            })
    }

    /// First close at or to the right of the visible left edge, matching LWC series first-value
    /// selection for percentage/indexed coordinate modes.
    pub(crate) fn series_base_value(&self, id: SeriesId, visible_from: i64) -> Option<f64> {
        let plot = self.data.plot(id);
        let row = plot.search(visible_from, MismatchDirection::NearestRight)?;
        let value = plot.value_at(row, PlotValueIndex::Close);
        value.is_finite().then_some(value)
    }

    pub(crate) fn visible_series_base_value(&self, id: SeriesId) -> Option<f64> {
        let from = self.time_scale.visible_strict_range()?.left();
        self.series_base_value(id, from)
    }
}
