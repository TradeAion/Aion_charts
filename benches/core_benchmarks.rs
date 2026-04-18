//! Core benchmarks for AxiusCharts performance-critical paths.
//!
//! Run with: `cargo bench`
//!
//! These benchmarks cover:
//! - BarArray operations (append, set, access)
//! - Viewport coordinate transformations
//! - Price auto-fit calculations

use axiuscharts::core::data::{Bar, BarArray};
use axiuscharts::core::viewport::Viewport;
use axiuscharts::{
    cluster_execution_mark_renderables, hit_test_execution_mark_hit_areas, ExecutionMarkHitArea,
    ExecutionRenderableMark, ExecutionRole, ExecutionSide,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// ═══════════════════════════════════════════════════════════════════════════════
// Test Data Generators
// ═══════════════════════════════════════════════════════════════════════════════

fn generate_bars(count: usize) -> Vec<Bar> {
    (0..count)
        .map(|i| {
            let base_price = 100.0 + (i as f64 * 0.1);
            Bar::new(
                1700000000000u64 + (i as u64 * 60000),
                base_price,
                base_price + 2.0,
                base_price - 1.0,
                base_price + 0.5,
                1000.0 + (i as f64 * 10.0),
            )
        })
        .collect()
}

fn generate_single_bar(idx: usize) -> Bar {
    let base_price = 100.0 + (idx as f64 * 0.1);
    Bar::new(
        1700000000000u64 + (idx as u64 * 60000),
        base_price,
        base_price + 2.0,
        base_price - 1.0,
        base_price + 0.5,
        1000.0 + (idx as f64 * 10.0),
    )
}

fn generate_execution_renderables(count: usize, same_side: bool) -> Vec<ExecutionRenderableMark> {
    (0..count)
        .map(|i| {
            let x_css = 100.0 + ((i % 20) as f64 * 2.0);
            let price = 100.0 + (i as f64 * 0.01);
            let side = if same_side || i % 2 == 0 {
                ExecutionSide::Buy
            } else {
                ExecutionSide::Sell
            };
            ExecutionRenderableMark {
                id: format!("exec-{i}"),
                timestamp_ms: 1_700_000_000_000 + i as u64,
                price,
                quantity: 1.0 + (i % 4) as f64,
                side,
                role: if i % 3 == 0 {
                    ExecutionRole::Entry
                } else {
                    ExecutionRole::Exit
                },
                label: None,
                realized_pnl: if i % 3 == 0 { None } else { Some(10.0) },
                color: [0.2, 0.4, 0.8, 1.0],
                group_id: Some("trade-1".to_string()),
                x_css,
                arrow_y_css: 40.0,
                price_y_css: 120.0 + (i as f64 * 0.25),
            }
        })
        .collect()
}

fn generate_execution_hit_areas(count: usize) -> Vec<ExecutionMarkHitArea> {
    (0..count)
        .map(|i| {
            let id = format!("exec-{i}");
            ExecutionMarkHitArea::new(
                id.clone(),
                vec![id],
                100.0 + ((i % 32) as f64 * 8.0),
                80.0 + ((i / 32) as f64 * 6.0),
                10.0,
            )
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════════════
// BarArray Benchmarks
// ═══════════════════════════════════════════════════════════════════════════════

fn bench_bar_array_set(c: &mut Criterion) {
    let mut group = c.benchmark_group("BarArray::set");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let bars = generate_bars(*size);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let mut arr = BarArray::new();
                let _ = arr.set(black_box(bars.clone()));
                arr
            });
        });
    }

    group.finish();
}

fn bench_bar_array_append(c: &mut Criterion) {
    let mut group = c.benchmark_group("BarArray::append");

    // Benchmark appending to arrays of different initial sizes
    for initial_size in [0, 100, 1_000, 10_000].iter() {
        let initial_bars = generate_bars(*initial_size);
        let new_bar = generate_single_bar(*initial_size);

        group.bench_with_input(
            BenchmarkId::new("initial_size", initial_size),
            initial_size,
            |b, _| {
                b.iter_batched(
                    || {
                        let mut arr = BarArray::new();
                        if !initial_bars.is_empty() {
                            let _ = arr.set(initial_bars.clone());
                        }
                        arr
                    },
                    |mut arr| {
                        let _ = arr.append(black_box(new_bar));
                        arr
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_bar_array_append_streaming(c: &mut Criterion) {
    let mut group = c.benchmark_group("BarArray::append_streaming");

    // Simulate streaming 1000 bars one at a time
    let bars: Vec<Bar> = generate_bars(1000);

    group.throughput(Throughput::Elements(1000));
    group.bench_function("1000_bars_streamed", |b| {
        b.iter(|| {
            let mut arr = BarArray::new();
            for bar in bars.iter() {
                let _ = arr.append(black_box(*bar));
            }
            arr.flush(); // Ensure all pending are committed
            arr
        });
    });

    group.finish();
}

fn bench_bar_array_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("BarArray::access");

    let bars = generate_bars(10_000);
    let mut arr = BarArray::new();
    let _ = arr.set(bars);

    // Benchmark different access patterns
    group.bench_function("get_checked", |b| {
        b.iter(|| {
            let mut sum = 0.0f64;
            for i in 0..arr.len() {
                if let Some(bar) = arr.get(black_box(i)) {
                    sum += bar.close;
                }
            }
            sum
        });
    });

    group.bench_function("get_unchecked", |b| {
        b.iter(|| {
            let mut sum = 0.0f64;
            for i in 0..arr.len() {
                let bar = arr.get_unchecked(black_box(i));
                sum += bar.close;
            }
            sum
        });
    });

    group.bench_function("direct_accessor_close", |b| {
        b.iter(|| {
            let mut sum = 0.0f64;
            for i in 0..arr.len() {
                sum += arr.close(black_box(i));
            }
            sum
        });
    });

    group.finish();
}

fn bench_bar_array_update_last(c: &mut Criterion) {
    let mut group = c.benchmark_group("BarArray::update_last");

    for size in [100, 1_000, 10_000].iter() {
        let bars = generate_bars(*size);
        let updated_bar = Bar::new(
            bars.last().unwrap().timestamp,
            150.0,
            155.0,
            148.0,
            152.0,
            5000.0,
        );

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter_batched(
                || {
                    let mut arr = BarArray::new();
                    let _ = arr.set(bars.clone());
                    arr
                },
                |mut arr| {
                    let _ = arr.update_last(black_box(updated_bar));
                    arr
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

// ═══════════════════════════════════════════════════════════════════════════════
// Viewport Benchmarks
// ═══════════════════════════════════════════════════════════════════════════════

fn bench_viewport_coordinate_transforms(c: &mut Criterion) {
    let mut group = c.benchmark_group("Viewport::transforms");

    let mut vp = Viewport::new(1920, 1080);
    vp.set_range(0.0, 500.0);
    vp.price_min = 100.0;
    vp.price_max = 200.0;

    group.bench_function("bar_to_frac", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for i in 0..1000 {
                sum += vp.bar_to_frac(black_box(i as f64));
            }
            sum
        });
    });

    group.bench_function("price_to_frac", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for i in 0..1000 {
                let price = 100.0 + (i as f64 * 0.1);
                sum += vp.price_to_frac(black_box(price));
            }
            sum
        });
    });

    group.bench_function("price_to_css_y", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for i in 0..1000 {
                let price = 100.0 + (i as f64 * 0.1);
                sum += vp.price_to_css_y(black_box(price), 1080.0);
            }
            sum
        });
    });

    group.bench_function("pixel_to_bar", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for i in 0..1000 {
                sum += vp.pixel_to_bar(black_box(i as f64), 1920.0);
            }
            sum
        });
    });

    group.bench_function("bar_index_at_pixel", |b| {
        b.iter(|| {
            let mut count = 0usize;
            for i in 0..1000 {
                if vp
                    .bar_index_at_pixel(black_box(i as f64), 1920.0, 500)
                    .is_some()
                {
                    count += 1;
                }
            }
            count
        });
    });

    group.finish();
}

fn bench_viewport_auto_fit_price(c: &mut Criterion) {
    let mut group = c.benchmark_group("Viewport::auto_fit_price");

    for size in [100, 1_000, 10_000].iter() {
        let bars = generate_bars(*size);
        let mut arr = BarArray::new();
        let _ = arr.set(bars);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched(
                || {
                    let mut vp = Viewport::new(1920, 1080);
                    vp.set_range(0.0, size as f64);
                    vp
                },
                |mut vp| {
                    vp.auto_fit_price(black_box(&arr));
                    vp
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_viewport_zoom(c: &mut Criterion) {
    let mut group = c.benchmark_group("Viewport::zoom");

    group.bench_function("zoom_in", |b| {
        b.iter_batched(
            || {
                let mut vp = Viewport::new(1920, 1080);
                vp.set_range(0.0, 500.0);
                vp
            },
            |mut vp| {
                vp.zoom(black_box(250.0), black_box(0.9));
                vp
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("zoom_out", |b| {
        b.iter_batched(
            || {
                let mut vp = Viewport::new(1920, 1080);
                vp.set_range(0.0, 500.0);
                vp
            },
            |mut vp| {
                vp.zoom(black_box(250.0), black_box(1.1));
                vp
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("rapid_zoom_sequence", |b| {
        b.iter_batched(
            || {
                let mut vp = Viewport::new(1920, 1080);
                vp.set_range(0.0, 500.0);
                vp
            },
            |mut vp| {
                // Simulate rapid mouse wheel zooming
                for _ in 0..20 {
                    vp.zoom(250.0, 0.95);
                }
                vp
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_viewport_pan(c: &mut Criterion) {
    let mut group = c.benchmark_group("Viewport::pan");

    group.bench_function("pan_simple", |b| {
        b.iter_batched(
            || {
                let mut vp = Viewport::new(1920, 1080);
                vp.set_range(0.0, 500.0);
                vp
            },
            |mut vp| {
                vp.pan(black_box(10.0));
                vp
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("pan_clamped", |b| {
        b.iter_batched(
            || {
                let mut vp = Viewport::new(1920, 1080);
                vp.set_range(0.0, 500.0);
                vp
            },
            |mut vp| {
                vp.pan_clamped(black_box(10.0), black_box(1000));
                vp
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("rapid_pan_sequence", |b| {
        b.iter_batched(
            || {
                let mut vp = Viewport::new(1920, 1080);
                vp.set_range(0.0, 500.0);
                vp
            },
            |mut vp| {
                // Simulate drag panning
                for _ in 0..100 {
                    vp.pan_clamped(5.0, 1000);
                }
                vp
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_execution_mark_clustering(c: &mut Criterion) {
    let mut group = c.benchmark_group("ExecutionMarks::cluster");

    for size in [20usize, 100, 1_000].iter() {
        let renderables = generate_execution_renderables(*size, true);
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                cluster_execution_mark_renderables(
                    black_box(&renderables),
                    black_box(14.0),
                    black_box(14.0),
                )
            });
        });
    }

    group.finish();
}

fn bench_execution_mark_hit_test(c: &mut Criterion) {
    let mut group = c.benchmark_group("ExecutionMarks::hit_test");

    for size in [1_000usize, 10_000, 50_000, 100_000].iter() {
        let hit_areas = generate_execution_hit_areas(*size);
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                hit_test_execution_mark_hit_areas(
                    black_box(&hit_areas),
                    black_box(132.0),
                    black_box(140.0),
                )
                .map(|hit_area| hit_area.id.as_str())
            });
        });
    }

    group.finish();
}

// ═══════════════════════════════════════════════════════════════════════════════
// Criterion Groups
// ═══════════════════════════════════════════════════════════════════════════════

criterion_group!(
    bar_array_benches,
    bench_bar_array_set,
    bench_bar_array_append,
    bench_bar_array_append_streaming,
    bench_bar_array_access,
    bench_bar_array_update_last,
);

criterion_group!(
    viewport_benches,
    bench_viewport_coordinate_transforms,
    bench_viewport_auto_fit_price,
    bench_viewport_zoom,
    bench_viewport_pan,
);

criterion_group!(
    execution_mark_benches,
    bench_execution_mark_clustering,
    bench_execution_mark_hit_test,
);

criterion_main!(bar_array_benches, viewport_benches, execution_mark_benches);
