//! Benchmarks for Ft-measurable rolling moments

use blawktrust::builtins::{rolling_moments_past_only_f64, MomentsMask};
use blawktrust::Column;
use std::time::Instant;

fn generate_data(n: usize) -> Vec<f64> {
    (0..n).map(|i| (i as f64 * 0.01).sin()).collect()
}

fn benchmark_single_pass_combined() {
    let data = generate_data(100_000);
    let window = 25;

    let mask = MomentsMask::from_names(&["mean", "std"]);

    let start = Instant::now();
    let _ = rolling_moments_past_only_f64(&data, window, None, mask, None);
    let elapsed = start.elapsed();

    println!(
        "Combined (mean+std): {:?} for {} elements",
        elapsed,
        data.len()
    );
}

fn benchmark_separate_passes() {
    let data = generate_data(100_000);
    let window = 25;

    let start = Instant::now();

    // Separate pass for mean
    let mask_mean = MomentsMask::from_names(&["mean"]);
    let _ = rolling_moments_past_only_f64(&data, window, None, mask_mean, None);

    // Separate pass for std
    let mask_std = MomentsMask::from_names(&["std"]);
    let _ = rolling_moments_past_only_f64(&data, window, None, mask_std, None);

    let elapsed = start.elapsed();

    println!(
        "Separate (mean, then std): {:?} for {} elements",
        elapsed,
        data.len()
    );
}

fn benchmark_all_moments() {
    let data = generate_data(100_000);
    let window = 25;

    let mask = MomentsMask::all();

    let start = Instant::now();
    let _ = rolling_moments_past_only_f64(&data, window, None, mask, None);
    let elapsed = start.elapsed();

    println!(
        "All moments (mean+std+skew+kurt+count): {:?} for {} elements",
        elapsed,
        data.len()
    );
}

fn benchmark_just_mean_std() {
    let data = generate_data(100_000);
    let window = 25;

    let mask = MomentsMask::from_names(&["mean", "std"]);

    let start = Instant::now();
    let _ = rolling_moments_past_only_f64(&data, window, None, mask, None);
    let elapsed = start.elapsed();

    println!(
        "Just mean+std: {:?} for {} elements",
        elapsed,
        data.len()
    );
}

fn main() {
    println!("=== Ft-measurable Rolling Moments Benchmarks ===\n");

    println!("Test 1: Combined vs Separate");
    println!("------------------------------");
    benchmark_single_pass_combined();
    benchmark_separate_passes();

    println!("\nTest 2: All moments vs subset");
    println!("------------------------------");
    benchmark_just_mean_std();
    benchmark_all_moments();

    println!("\n=== Speedup Analysis ===");
    println!("Expected: Combined should be ~2x faster than separate");
    println!("Expected: All moments ~30% slower than just mean+std");
}
