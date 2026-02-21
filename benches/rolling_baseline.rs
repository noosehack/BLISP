//! Rolling Operations Baseline Benchmark
//!
//! PURPOSE: Detect O(n·w) complexity in rolling operations
//!
//! Current implementation is naive O(n·w) per column:
//! - rolling_mean: nested loop (outer over rows, inner sums window)
//! - rolling_std: nested loop (outer over rows, inner collects + computes variance)
//!
//! Optimization target: O(n) with running sums
//!
//! Performance gates (no >20% regression after optimization):
//! - Throughput (cells/sec) should improve dramatically
//! - Time should scale linearly in n, not n·w
//! - Memory allocations should be O(n), not O(n·w)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use blisp::frame::{Frame, Tags, IndexColumn};
use blisp::runtime::Runtime;
use blisp::ast::Expr;
use blisp::normalize::normalize;
use blisp::planner::plan;
use blisp::exec::execute;
use blisp::value::Value;
use blawktrust::Column;
use std::sync::Arc;

/// Generate test frame with specified properties
fn make_test_frame(nrows: usize, ncols: usize, na_rate: f64, seed: u64) -> Arc<Frame> {

    // Simple LCG for reproducible randomness
    let mut rng_state = seed;
    let mut next_rand = || {
        rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
        (rng_state / 65536) % 32768
    };

    // Generate date index
    let dates: Vec<i32> = (0..nrows).map(|i| 20000 + i as i32).collect();
    let index = IndexColumn::Date(Arc::new(dates));

    // Generate column names
    let colnames: Vec<String> = (0..ncols).map(|i| format!("col{}", i)).collect();

    // Generate numeric columns with NA
    let cols: Vec<Arc<Column>> = (0..ncols)
        .map(|col_idx| {
            let data: Vec<f64> = (0..nrows)
                .map(|row_idx| {
                    let rand_val = next_rand() as f64 / 32768.0;
                    if rand_val < na_rate {
                        f64::NAN  // NA
                    } else {
                        // Generate value: sin wave + column offset
                        ((row_idx as f64 * 0.01).sin() + col_idx as f64) * 100.0
                    }
                })
                .collect();
            Arc::new(Column::new_f64(data))
        })
        .collect();

    let tags = Tags::new("DATE".to_string(), index, colnames);
    Arc::new(Frame::new(tags, cols))
}

/// Execute expression through IR layer (plan + execute)
fn eval_expr(expr: Expr, rt: &mut Runtime) -> Arc<Frame> {
    let normalized = normalize(expr, &mut rt.interner);
    let plan = plan(&normalized, &rt.interner).expect("plan failed");

    match execute(&plan, rt).expect("execute failed") {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    }
}

/// Benchmark rolling_mean with varying window sizes
fn bench_rolling_mean_window_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("rolling_mean_window_scaling");

    let nrows = 20_000;
    let ncols = 1;
    let na_rate = 0.05;

    for w in [5, 20, 50, 100, 250] {
        group.throughput(Throughput::Elements((nrows * ncols) as u64));

        group.bench_with_input(BenchmarkId::from_parameter(w), &w, |b, &w| {
            let frame = make_test_frame(nrows, ncols, na_rate, 42);

            b.iter(|| {
                let mut rt = Runtime::new();
                let x_sym = rt.interner.intern("x");
                rt.define(x_sym, Value::Frame(Arc::clone(&frame)));

                let rm_sym = rt.interner.intern("rolling-mean");
                let expr = Expr::List(vec![
                    Expr::Sym(rm_sym),
                    Expr::Int(w as i64),
                    Expr::Sym(x_sym),
                ]);

                black_box(eval_expr(expr, &mut rt))
            });
        });
    }

    group.finish();
}

/// Benchmark rolling_mean with varying data sizes
fn bench_rolling_mean_data_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("rolling_mean_data_scaling");

    let w = 20;
    let ncols = 1;
    let na_rate = 0.05;

    for nrows in [2_000, 20_000, 200_000] {
        group.throughput(Throughput::Elements((nrows * ncols) as u64));

        group.bench_with_input(BenchmarkId::from_parameter(nrows), &nrows, |b, &nrows| {
            let frame = make_test_frame(nrows, ncols, na_rate, 42);

            b.iter(|| {
                let mut rt = Runtime::new();
                let x_sym = rt.interner.intern("x");
                rt.define(x_sym, Value::Frame(Arc::clone(&frame)));

                let rm_sym = rt.interner.intern("rolling-mean");
                let expr = Expr::List(vec![
                    Expr::Sym(rm_sym),
                    Expr::Int(w as i64),
                    Expr::Sym(x_sym),
                ]);

                black_box(eval_expr(expr, &mut rt))
            });
        });
    }

    group.finish();
}

/// Benchmark rolling_mean with varying NA rates
fn bench_rolling_mean_na_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("rolling_mean_na_rate");

    let nrows = 20_000;
    let ncols = 1;
    let w = 20;

    for na_pct in [0, 5, 20] {
        let na_rate = na_pct as f64 / 100.0;
        group.throughput(Throughput::Elements((nrows * ncols) as u64));

        group.bench_with_input(BenchmarkId::from_parameter(na_pct), &na_pct, |b, _| {
            let frame = make_test_frame(nrows, ncols, na_rate, 42);

            b.iter(|| {
                let mut rt = Runtime::new();
                let x_sym = rt.interner.intern("x");
                rt.define(x_sym, Value::Frame(Arc::clone(&frame)));

                let rm_sym = rt.interner.intern("rolling-mean");
                let expr = Expr::List(vec![
                    Expr::Sym(rm_sym),
                    Expr::Int(w as i64),
                    Expr::Sym(x_sym),
                ]);

                black_box(eval_expr(expr, &mut rt))
            });
        });
    }

    group.finish();
}

/// Benchmark rolling_std with varying window sizes
fn bench_rolling_std_window_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("rolling_std_window_scaling");

    let nrows = 20_000;
    let ncols = 1;
    let na_rate = 0.05;

    for w in [5, 20, 50, 100, 250] {
        group.throughput(Throughput::Elements((nrows * ncols) as u64));

        group.bench_with_input(BenchmarkId::from_parameter(w), &w, |b, &w| {
            let frame = make_test_frame(nrows, ncols, na_rate, 42);

            b.iter(|| {
                let mut rt = Runtime::new();
                let x_sym = rt.interner.intern("x");
                rt.define(x_sym, Value::Frame(Arc::clone(&frame)));

                let rs_sym = rt.interner.intern("rolling-std");
                let expr = Expr::List(vec![
                    Expr::Sym(rs_sym),
                    Expr::Int(w as i64),
                    Expr::Sym(x_sym),
                ]);

                black_box(eval_expr(expr, &mut rt))
            });
        });
    }

    group.finish();
}

/// Benchmark rolling_std with varying data sizes
fn bench_rolling_std_data_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("rolling_std_data_scaling");

    let w = 20;
    let ncols = 1;
    let na_rate = 0.05;

    for nrows in [2_000, 20_000, 200_000] {
        group.throughput(Throughput::Elements((nrows * ncols) as u64));

        group.bench_with_input(BenchmarkId::from_parameter(nrows), &nrows, |b, &nrows| {
            let frame = make_test_frame(nrows, ncols, na_rate, 42);

            b.iter(|| {
                let mut rt = Runtime::new();
                let x_sym = rt.interner.intern("x");
                rt.define(x_sym, Value::Frame(Arc::clone(&frame)));

                let rs_sym = rt.interner.intern("rolling-std");
                let expr = Expr::List(vec![
                    Expr::Sym(rs_sym),
                    Expr::Int(w as i64),
                    Expr::Sym(x_sym),
                ]);

                black_box(eval_expr(expr, &mut rt))
            });
        });
    }

    group.finish();
}

/// Benchmark derived rolling-zscore pipeline
fn bench_rolling_zscore_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("rolling_zscore_pipeline");

    let w = 20;
    let ncols = 1;
    let na_rate = 0.05;

    for nrows in [2_000, 20_000, 200_000] {
        group.throughput(Throughput::Elements((nrows * ncols) as u64));

        group.bench_with_input(BenchmarkId::from_parameter(nrows), &nrows, |b, &nrows| {
            let frame = make_test_frame(nrows, ncols, na_rate, 42);

            b.iter(|| {
                let mut rt = Runtime::new();
                let x_sym = rt.interner.intern("x");
                rt.define(x_sym, Value::Frame(Arc::clone(&frame)));

                let rz_sym = rt.interner.intern("rolling-zscore");
                let expr = Expr::List(vec![
                    Expr::Sym(rz_sym),
                    Expr::Int(w as i64),
                    Expr::Sym(x_sym),
                ]);

                black_box(eval_expr(expr, &mut rt))
            });
        });
    }

    group.finish();
}

/// Benchmark multi-column rolling operations (real-world workload)
fn bench_rolling_mean_multi_column(c: &mut Criterion) {
    let mut group = c.benchmark_group("rolling_mean_multi_column");

    let nrows = 20_000;
    let w = 20;
    let na_rate = 0.05;

    for ncols in [1, 5, 10, 50] {
        group.throughput(Throughput::Elements((nrows * ncols) as u64));

        group.bench_with_input(BenchmarkId::from_parameter(ncols), &ncols, |b, &ncols| {
            let frame = make_test_frame(nrows, ncols, na_rate, 42);

            b.iter(|| {
                let mut rt = Runtime::new();
                let x_sym = rt.interner.intern("x");
                rt.define(x_sym, Value::Frame(Arc::clone(&frame)));

                let rm_sym = rt.interner.intern("rolling-mean");
                let expr = Expr::List(vec![
                    Expr::Sym(rm_sym),
                    Expr::Int(w as i64),
                    Expr::Sym(x_sym),
                ]);

                black_box(eval_expr(expr, &mut rt))
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_rolling_mean_window_scaling,
    bench_rolling_mean_data_scaling,
    bench_rolling_mean_na_rate,
    bench_rolling_std_window_scaling,
    bench_rolling_std_data_scaling,
    bench_rolling_zscore_pipeline,
    bench_rolling_mean_multi_column,
);
criterion_main!(benches);
