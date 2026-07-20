//! Series data/coordinate queries mirroring the LWC series API (`price_to_coordinate`,
//! `data_by_index`, `bars_in_logical_range`, ...). Extracted from `lib.rs`.

use super::*;

impl ChartEngine {
    pub fn series_price_to_coordinate(&self, id: SeriesId, price: f64) -> Option<f64> {
        if !price.is_finite() {
            return None;
        }
        let (pane, target) = self.series_price_scale(id)?;
        let scale = self.price_scale_for(pane, target)?;
        if scale.is_empty() {
            return None;
        }
        let base = self.visible_series_base_value(id)?;
        Some(scale.price_to_coordinate(price, base))
    }

    pub fn series_coordinate_to_price(&self, id: SeriesId, coordinate: f64) -> Option<f64> {
        if !coordinate.is_finite() {
            return None;
        }
        let (pane, target) = self.series_price_scale(id)?;
        let scale = self.price_scale_for(pane, target)?;
        if scale.is_empty() {
            return None;
        }
        let base = self.visible_series_base_value(id)?;
        Some(scale.coordinate_to_price(coordinate, base))
    }

    pub fn series_kind(&self, id: SeriesId) -> Option<SeriesKind> {
        self.series
            .iter()
            .find(|series| series.id == id)
            .map(|series| series.kind)
    }

    fn series_point_at_row(&self, id: SeriesId, row: usize) -> Option<SeriesDataPoint> {
        let plot = self.data.plot(id);
        let index = *plot.indices().get(row)?;
        let time = *self.data.merged_times().get(index as usize)?;
        Some(SeriesDataPoint {
            time,
            open: plot.value_at(row, PlotValueIndex::Open),
            high: plot.value_at(row, PlotValueIndex::High),
            low: plot.value_at(row, PlotValueIndex::Low),
            close: plot.value_at(row, PlotValueIndex::Close),
        })
    }

    pub fn series_data_by_index(
        &self,
        id: SeriesId,
        logical_index: i64,
        mismatch: MismatchDirection,
    ) -> Option<SeriesDataPoint> {
        let row = self.data.plot(id).search(logical_index, mismatch)?;
        self.series_point_at_row(id, row)
    }

    pub fn series_data(&self, id: SeriesId) -> Vec<SeriesDataPoint> {
        let size = self.data.plot(id).size();
        (0..size)
            .filter_map(|row| self.series_point_at_row(id, row))
            .collect()
    }

    /// LWC `barsInLogicalRange`, including its gap behavior and fractional bars-before/after
    /// results. Times are original UTC seconds of the first/last series bars inside the range.
    pub fn series_bars_in_logical_range(
        &self,
        id: SeriesId,
        from: f64,
        to: f64,
    ) -> Option<BarsInLogicalRange> {
        if !from.is_finite() || !to.is_finite() || from > to {
            return None;
        }
        let plot = self.data.plot(id);
        let data_first = plot.first_index()?;
        let data_last = plot.last_index()?;
        let strict = LogicalRange::new(from, to).to_strict();
        let first_row = plot.search(strict.left(), MismatchDirection::NearestRight);
        let last_row = plot.search(strict.right(), MismatchDirection::NearestLeft);
        let first_index = first_row.and_then(|row| plot.indices().get(row).copied());
        let last_index = last_row.and_then(|row| plot.indices().get(row).copied());

        if first_index
            .zip(last_index)
            .is_some_and(|(first, last)| first > last)
        {
            return Some(BarsInLogicalRange {
                bars_before: from - data_first as f64,
                bars_after: data_last as f64 - to,
                from: None,
                to: None,
            });
        }

        let bars_before = match first_index {
            None => from - data_first as f64,
            Some(index) if index == data_first => from - data_first as f64,
            Some(index) => (index - data_first) as f64,
        };
        let bars_after = match last_index {
            None => data_last as f64 - to,
            Some(index) if index == data_last => data_last as f64 - to,
            Some(index) => (data_last - index) as f64,
        };
        let times = first_index.zip(last_index).and_then(|(first, last)| {
            Some((
                *self.data.merged_times().get(first as usize)?,
                *self.data.merged_times().get(last as usize)?,
            ))
        });
        Some(BarsInLogicalRange {
            bars_before,
            bars_after,
            from: times.map(|times| times.0),
            to: times.map(|times| times.1),
        })
    }
}
