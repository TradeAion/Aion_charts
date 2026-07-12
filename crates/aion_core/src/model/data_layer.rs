//! Multi-series data layer. Port of the merged-time-point concept from `src/model/data-layer.ts`,
//! restructured for our SoA storage.
//!
//! The horizontal axis is indexed by position in the **union** of every series' timestamps
//! (the "merged time points"). Each series maps its own data onto those indices; a series with
//! no point at a given index is simply absent there (whitespace). This is what lets a price
//! series and a volume series — or a candlestick and a moving-average overlay — share one time
//! scale even when their sample sets differ.

use crate::helpers::algorithms::lower_bound;
use crate::model::plot_list::PlotList;
use crate::TimePointIndex;

pub type SeriesId = usize;

struct RawSeries {
    times: Vec<i64>,
    values: [Vec<f64>; 4],
    /// Rebuilt against merged indices; keys are positions in `merged_times`.
    plot: PlotList,
}

impl RawSeries {
    fn empty() -> Self {
        Self { times: Vec::new(), values: [vec![], vec![], vec![], vec![]], plot: PlotList::new() }
    }
}

#[derive(Default)]
pub struct DataLayer {
    series: Vec<RawSeries>,
    merged_times: Vec<i64>,
}

impl DataLayer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_series(&mut self) -> SeriesId {
        self.series.push(RawSeries::empty());
        self.series.len() - 1
    }

    pub fn series_count(&self) -> usize {
        self.series.len()
    }

    /// Union of all series' timestamps, sorted ascending (the time-scale points).
    pub fn merged_times(&self) -> &[i64] {
        &self.merged_times
    }

    pub fn plot(&self, id: SeriesId) -> &PlotList {
        &self.series[id].plot
    }

    pub fn plot_mut(&mut self, id: SeriesId) -> &mut PlotList {
        &mut self.series[id].plot
    }

    /// Merged index of the last point that has data (the time-scale base index), or None.
    pub fn base_index(&self) -> Option<TimePointIndex> {
        if self.merged_times.is_empty() {
            None
        } else {
            Some(self.merged_times.len() as i64 - 1)
        }
    }

    /// Full (re)assignment of a series' data. `times` must be ascending. Rebuilds the merged
    /// time points and re-maps every series onto the new index space.
    pub fn set_data(
        &mut self,
        id: SeriesId,
        times: Vec<i64>,
        open: Vec<f64>,
        high: Vec<f64>,
        low: Vec<f64>,
        close: Vec<f64>,
    ) {
        debug_assert!(times.windows(2).all(|w| w[0] < w[1]), "series times must be ascending unique");
        let s = &mut self.series[id];
        s.times = times;
        s.values = [open, high, low, close];
        self.rebuild_merged();
        self.reindex_all();
    }

    /// Streaming update of a series' last point: replaces the last bar or appends a new one.
    /// The fast path (append at a new global max time, or replace an existing point) avoids a
    /// full rebuild.
    pub fn update(&mut self, id: SeriesId, time: i64, values: [f64; 4]) {
        let last_merged = self.merged_times.last().copied();

        // Case 1: brand-new global max time — appended at the end, no indices shift.
        if last_merged.is_none_or(|last| time > last) {
            let new_index = self.merged_times.len() as TimePointIndex;
            self.merged_times.push(time);
            let s = &mut self.series[id];
            push_raw(s, time, values);
            s.plot.upsert_last(new_index, values);
            return;
        }

        // Case 2: an existing merged time, at or after this series' own last point — a
        // replace-last or append-at-series-end that maps to a non-decreasing plot index.
        let series_last = self.series[id].times.last().copied();
        let existing = self.merged_times.binary_search(&time).ok();
        if let (Some(pos), true) = (existing, series_last.is_none_or(|lt| time >= lt)) {
            let s = &mut self.series[id];
            if series_last == Some(time) {
                let row = s.times.len() - 1;
                for (col, v) in s.values.iter_mut().zip(values) {
                    col[row] = v;
                }
            } else {
                push_raw(s, time, values);
            }
            s.plot.upsert_last(pos as TimePointIndex, values);
            return;
        }

        // Case 3: insert into the middle of this series (and possibly the merged set) — rebuild.
        let s = &mut self.series[id];
        let insert = lower_bound(&s.times, |&t| t < time);
        if s.times.get(insert) == Some(&time) {
            for (col, v) in s.values.iter_mut().zip(values) {
                col[insert] = v;
            }
        } else {
            s.times.insert(insert, time);
            for (col, v) in s.values.iter_mut().zip(values) {
                col.insert(insert, v);
            }
        }
        self.rebuild_merged();
        self.reindex_all();
    }

    fn rebuild_merged(&mut self) {
        let total: usize = self.series.iter().map(|s| s.times.len()).sum();
        let mut all = Vec::with_capacity(total);
        for s in &self.series {
            all.extend_from_slice(&s.times);
        }
        all.sort_unstable();
        all.dedup();
        self.merged_times = all;
    }

    fn reindex_all(&mut self) {
        // borrow merged_times immutably while mutating each series' plot
        let merged = &self.merged_times;
        for s in &mut self.series {
            if s.times.is_empty() {
                s.plot.set_data(vec![], vec![], vec![], vec![], vec![]);
                continue;
            }
            let indices: Vec<TimePointIndex> = s
                .times
                .iter()
                .map(|t| merged.binary_search(t).expect("series time in merged") as TimePointIndex)
                .collect();
            s.plot.set_data(
                indices,
                s.values[0].clone(),
                s.values[1].clone(),
                s.values[2].clone(),
                s.values[3].clone(),
            );
        }
    }
}

fn push_raw(s: &mut RawSeries, time: i64, values: [f64; 4]) {
    s.times.push(time);
    for (col, v) in s.values.iter_mut().zip(values) {
        col.push(v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::plot_list::{MismatchDirection, PlotValueIndex};

    /// Value of a series at a merged *index* (maps index -> sparse row).
    fn value_at_index(dl: &DataLayer, id: SeriesId, index: TimePointIndex, plot: PlotValueIndex) -> f64 {
        let row = dl.plot(id).search(index, MismatchDirection::None).expect("index present");
        dl.plot(id).value_at(row, plot)
    }

    /// Sets a single-value series (all OHLC = the value) at the given times.
    fn set(dl: &mut DataLayer, id: SeriesId, times: &[i64], vals: &[f64]) {
        let col = |f: fn(f64) -> f64| vals.iter().map(|&v| f(v)).collect::<Vec<f64>>();
        dl.set_data(id, times.to_vec(), col(|v| v), col(|v| v), col(|v| v), col(|v| v));
    }

    #[test]
    fn merged_union_and_per_series_indices() {
        let mut dl = DataLayer::new();
        let a = dl.add_series();
        let b = dl.add_series();
        // A at times 1,2,3 ; B at 2,4 -> merged 1,2,3,4
        set(&mut dl, a, &[1, 2, 3], &[10.0, 20.0, 30.0]);
        set(&mut dl, b, &[2, 4], &[5.0, 7.0]);

        assert_eq!(dl.merged_times(), &[1, 2, 3, 4]);
        assert_eq!(dl.base_index(), Some(3));

        // A occupies merged indices 0,1,2 ; B occupies 1,3 (whitespace at 0,2)
        assert_eq!(dl.plot(a).indices(), &[0, 1, 2]);
        assert_eq!(dl.plot(b).indices(), &[1, 3]);
        assert!(dl.plot(b).contains(1));
        assert!(!dl.plot(b).contains(0));
        assert!(!dl.plot(b).contains(2));
    }

    #[test]
    fn adding_a_series_reindexes_existing() {
        let mut dl = DataLayer::new();
        let a = dl.add_series();
        set(&mut dl, a, &[10, 20, 30], &[1.0, 2.0, 3.0]);
        assert_eq!(dl.plot(a).indices(), &[0, 1, 2]);

        // new series introduces earlier + interleaved times -> A's indices shift
        let b = dl.add_series();
        set(&mut dl, b, &[5, 15, 25], &[9.0, 9.0, 9.0]);
        assert_eq!(dl.merged_times(), &[5, 10, 15, 20, 25, 30]);
        assert_eq!(dl.plot(a).indices(), &[1, 3, 5]);
        assert_eq!(dl.plot(b).indices(), &[0, 2, 4]);
    }

    #[test]
    fn update_appends_new_max_without_rebuild() {
        let mut dl = DataLayer::new();
        let a = dl.add_series();
        set(&mut dl, a, &[1, 2, 3], &[10.0, 20.0, 30.0]);
        dl.update(a, 4, [40.0, 41.0, 39.0, 40.0]);
        assert_eq!(dl.merged_times(), &[1, 2, 3, 4]);
        assert_eq!(dl.plot(a).indices(), &[0, 1, 2, 3]);
        assert_eq!(value_at_index(&dl, a, 3, PlotValueIndex::Close), 40.0);
    }

    #[test]
    fn update_replaces_last_point() {
        let mut dl = DataLayer::new();
        let a = dl.add_series();
        set(&mut dl, a, &[1, 2, 3], &[10.0, 20.0, 30.0]);
        dl.update(a, 3, [33.0, 35.0, 31.0, 34.0]);
        assert_eq!(dl.merged_times(), &[1, 2, 3]); // no new point
        assert_eq!(value_at_index(&dl, a, 2, PlotValueIndex::High), 35.0);
    }

    #[test]
    fn update_into_other_series_gap_uses_existing_index() {
        let mut dl = DataLayer::new();
        let a = dl.add_series();
        let b = dl.add_series();
        set(&mut dl, a, &[1, 2, 3, 4], &[1.0, 2.0, 3.0, 4.0]);
        set(&mut dl, b, &[1, 4], &[9.0, 9.0]); // whitespace at 2,3
        // B gets a point at time 3 (an existing merged time, index 2)
        dl.update(b, 3, [7.0, 7.0, 7.0, 7.0]);
        assert_eq!(dl.merged_times(), &[1, 2, 3, 4]);
        assert!(dl.plot(b).contains(2)); // time 3 -> merged index 2
        assert_eq!(value_at_index(&dl, b, 2, PlotValueIndex::Close), 7.0);
    }

    #[test]
    fn empty_layer() {
        let mut dl = DataLayer::new();
        let a = dl.add_series();
        assert_eq!(dl.merged_times(), &[] as &[i64]);
        assert_eq!(dl.base_index(), None);
        assert!(dl.plot(a).is_empty());
    }
}
