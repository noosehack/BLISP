//! PR4.1 and PR4.2b Fusion Performance Benchmarks
//!
//! Measures wall time for:
//! - A) Elementwise chain fusion (PR4.1): inv(sqrt(exp(log(abs(x)))))
//! - B) cs1 ∘ dlog-obs fusion (PR4.2b): cs1(dlog(x)) with weekend NA pattern
//! - C) cs1 ∘ dlog-ofs fusion (PR4.2b): cs1(dlog-ofs(x,k)) for k ∈ {1,2,5}
//!
//! Each benchmark compares:
//! - Unfused: Multiple passes with intermediate allocations
//! - Fused: Single pass with one output allocation

use blawktrust::Column;
use blisp::exec::{
    abs_column, cumsum_column, dlog_obs_column, dlog_ofs_column, exp_column,
    fused_cs1_dlog_obs_column, fused_cs1_dlog_ofs_column, fused_elementwise_column, inv_column,
    log_column, sqrt_column,
};
use blisp::ir::NumericFunc;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

/// Generate test data with specified NA density
fn generate_test_data(size: usize, na_rate: f64) -> Vec<f64> {
    let mut data = Vec::with_capacity(size);
    let mut value = 100.0;

    for i in 0..size {
        if (i as f64 / size as f64) < na_rate {
            data.push(f64::NAN);
        } else {
            data.push(value);
            value *= 1.001; // Small growth for realistic data
        }
    }
    data
}

/// Generate data with weekend-style NA blocks (5 trading days, 2 weekend days)
fn generate_weekend_data(size: usize) -> Vec<f64> {
    let mut data = Vec::with_capacity(size);
    let mut value = 100.0;

    for i in 0..size {
        if i % 7 < 5 {
            // Trading day (Mon-Fri)
            data.push(value);
            value *= 1.001;
        } else {
            // Weekend (Sat-Sun)
            data.push(f64::NAN);
        }
    }
    data
}

/// Benchmark A: Elementwise chain fusion (PR4.1)
///
/// Tests: inv(sqrt(exp(log(abs(x)))))
/// - Unfused: 5 passes, 5 intermediate allocations
/// - Fused: 1 pass, 1 output allocation
fn bench_pr4_1_elementwise_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("PR4.1_Elementwise");

    for &size in &[1_000_000, 10_000_000] {
        for &na_rate in &[0.0, 0.15] {
            let param_name = format!("n={}_na={}%", size, (na_rate * 100.0) as usize);

            // Unfused: 5 separate operations
            group.bench_function(BenchmarkId::new("unfused", &param_name), |b| {
                let data = generate_test_data(size, na_rate);
                let col = Column::new_f64(data);

                b.iter(|| {
                    let r1 = abs_column(&col);
                    let r2 = log_column(&r1);
                    let r3 = exp_column(&r2);
                    let r4 = sqrt_column(&r3);
                    let r5 = inv_column(&r4);
                    black_box(r5)
                });
            });

            // Fused: Single pass through chain
            group.bench_function(BenchmarkId::new("fused", &param_name), |b| {
                let data = generate_test_data(size, na_rate);
                let col = Column::new_f64(data);

                let ops = vec![
                    NumericFunc::ABS,
                    NumericFunc::LOG,
                    NumericFunc::EXP,
                    NumericFunc::SQRT,
                    NumericFunc::INV,
                ];

                b.iter(|| black_box(fused_elementwise_column(&col, &ops)));
            });
        }
    }

    group.finish();
}

/// Benchmark B: cs1 ∘ dlog-obs fusion (PR4.2b)
///
/// Tests: cs1(dlog(x)) with weekend NA pattern (~28% NA)
/// - Unfused: 2 passes (dlog-obs → cs1)
/// - Fused: 1 pass with two state variables (prev_valid + acc)
fn bench_pr4_2b_cs1_dlog_obs(c: &mut Criterion) {
    let mut group = c.benchmark_group("PR4.2b_CS1_DLOG_OBS");

    let size = 1_000_000;

    // Unfused: dlog-obs then cs1
    group.bench_function("unfused", |b| {
        let data = generate_weekend_data(size);
        let col = Column::new_f64(data);

        b.iter(|| {
            let dlog_result = dlog_obs_column(&col, 1);
            let cs1_result = cumsum_column(&dlog_result);
            black_box(cs1_result)
        });
    });

    // Fused: Single pass
    group.bench_function("fused", |b| {
        let data = generate_weekend_data(size);
        let col = Column::new_f64(data);

        b.iter(|| black_box(fused_cs1_dlog_obs_column(&col)));
    });

    group.finish();
}

/// Benchmark C: cs1 ∘ dlog-ofs fusion for different k values (PR4.2b)
///
/// Tests: cs1(dlog-ofs(x, k)) for k ∈ {1,2,5}
/// - Unfused: 2 passes (dlog-ofs(k) → cs1)
/// - Fused: 1 pass with two state variables (lagged value + acc)
fn bench_pr4_2b_cs1_dlog_ofs(c: &mut Criterion) {
    let mut group = c.benchmark_group("PR4.2b_CS1_DLOG_OFS");

    let size = 1_000_000;

    for &k in &[1, 2, 5] {
        // Unfused: dlog-ofs(k) then cs1
        group.bench_function(BenchmarkId::new("unfused", format!("k={}", k)), |b| {
            let data = generate_test_data(size, 0.0);
            let col = Column::new_f64(data);

            b.iter(|| {
                let dlog_result = dlog_ofs_column(&col, k);
                let cs1_result = cumsum_column(&dlog_result);
                black_box(cs1_result)
            });
        });

        // Fused: Single pass
        group.bench_function(BenchmarkId::new("fused", format!("k={}", k)), |b| {
            let data = generate_test_data(size, 0.0);
            let col = Column::new_f64(data);

            b.iter(|| black_box(fused_cs1_dlog_ofs_column(&col, k)));
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_pr4_1_elementwise_chain,
    bench_pr4_2b_cs1_dlog_obs,
    bench_pr4_2b_cs1_dlog_ofs,
);
criterion_main!(benches);
