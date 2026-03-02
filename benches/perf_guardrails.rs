//! Performance Guardrails (per docs/contracts.md)
//!
//! These benchmarks MUST NOT regress by >20% without explicit justification.
//! They detect accidental slowdowns in Phase 3+.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

// Mock types to match blisp's structure (since it's a binary)
// In real setup, would extract to lib.rs

fn bench_dlog_large(c: &mut Criterion) {
    // Benchmark: dlog on 5M cells (some NA)
    // Contract: No >20% regression

    let mut group = c.benchmark_group("dlog_large");

    for size in [100_000, 1_000_000, 5_000_000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            // Generate test data with some NA
            let mut data = vec![100.0; size];
            for i in (0..size).step_by(10) {
                data[i] = f64::NAN; // 10% NA
            }
            let col = blawktrust::Column::new_f64(data);

            b.iter(|| black_box(blawktrust::builtins::ops::dlog_column(&col, 1)));
        });
    }
    group.finish();
}

fn bench_reindex_sorted(c: &mut Criterion) {
    // Benchmark: reindex_by with sorted indices (best case)
    // Contract: No >20% regression
    // Future: Two-pointer merge optimization should improve this

    let mut group = c.benchmark_group("reindex_sorted");

    for size in [10_000, 100_000, 1_000_000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            // Source and target both sorted
            let source_dates: Vec<i32> = (0..size).map(|i| i as i32).collect();
            let target_dates: Vec<i32> = (0..size).map(|i| i as i32).collect();

            let source_data: Vec<f64> = (0..size).map(|i| i as f64).collect();

            b.iter(|| {
                // Simulate reindex_by: hashmap build + lookup
                use std::collections::HashMap;
                let mut map: HashMap<i32, usize> = HashMap::new();
                for (i, &date) in source_dates.iter().enumerate() {
                    map.insert(date, i);
                }

                let mut result = Vec::with_capacity(size);
                for &target_date in &target_dates {
                    match map.get(&target_date) {
                        Some(&idx) => result.push(source_data[idx]),
                        None => result.push(f64::NAN),
                    }
                }
                black_box(result)
            });
        });
    }
    group.finish();
}

fn bench_reindex_unsorted(c: &mut Criterion) {
    // Benchmark: reindex_by with unsorted indices (worst case)
    // Contract: No >20% regression

    let mut group = c.benchmark_group("reindex_unsorted");

    for size in [10_000, 100_000, 1_000_000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            // Source sorted, target reverse-sorted (worst case)
            let source_dates: Vec<i32> = (0..size).map(|i| i as i32).collect();
            let target_dates: Vec<i32> = (0..size).rev().map(|i| i as i32).collect();

            let source_data: Vec<f64> = (0..size).map(|i| i as f64).collect();

            b.iter(|| {
                use std::collections::HashMap;
                let mut map: HashMap<i32, usize> = HashMap::new();
                for (i, &date) in source_dates.iter().enumerate() {
                    map.insert(date, i);
                }

                let mut result = Vec::with_capacity(size);
                for &target_date in &target_dates {
                    match map.get(&target_date) {
                        Some(&idx) => result.push(source_data[idx]),
                        None => result.push(f64::NAN),
                    }
                }
                black_box(result)
            });
        });
    }
    group.finish();
}

fn bench_reindex_sparse(c: &mut Criterion) {
    // Benchmark: reindex_by with sparse matching (50% hit rate)
    // Contract: No >20% regression

    let mut group = c.benchmark_group("reindex_sparse");

    for size in [10_000, 100_000, 1_000_000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            // Source has every other date
            let source_dates: Vec<i32> = (0..size).step_by(2).map(|i| i as i32).collect();
            let target_dates: Vec<i32> = (0..size).map(|i| i as i32).collect();

            let source_data: Vec<f64> = (0..source_dates.len()).map(|i| i as f64).collect();

            b.iter(|| {
                use std::collections::HashMap;
                let mut map: HashMap<i32, usize> = HashMap::new();
                for (i, &date) in source_dates.iter().enumerate() {
                    map.insert(date, i);
                }

                let mut result = Vec::with_capacity(size);
                for &target_date in &target_dates {
                    match map.get(&target_date) {
                        Some(&idx) => result.push(source_data[idx]),
                        None => result.push(f64::NAN),
                    }
                }
                black_box(result)
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_dlog_large,
    bench_reindex_sorted,
    bench_reindex_unsorted,
    bench_reindex_sparse,
);
criterion_main!(benches);
