# Rolling Operations Performance Baseline

**Date**: 2026-02-21
**Branch**: `reconstruct/tableview-only`
**Hardware**: Linux 6.8.0-1046-aws (EC2 instance)
**Compiler**: Rust release mode (unoptimized + debuginfo)

---

## Executive Summary

**Current implementation is O(n·w) per column** - nested loops recompute entire window on every iteration.

**Performance impact**:
- `rolling_mean`: Throughput degrades from **33.7 Melem/s** (w=5) to **2.7 Melem/s** (w=250) = **12x slowdown**
- `rolling_std`: Throughput degrades from **26.4 Melem/s** (w=5) to **1.8 Melem/s** (w=250) = **14x slowdown**

**Optimization target**: O(n) with running sums
- Expected improvement: ~10-50x faster for typical window sizes (w=20-250)
- Throughput should be **constant regardless of window size**

---

## Benchmark Results

### Rolling Mean - Window Scaling (20k rows, 1 column)

| Window | Time | Throughput | O(n·w) Factor |
|--------|------|------------|---------------|
| 5      | 253 µs  | 33.7 Melem/s | 1.0x |
| 20     | 593 µs  | 33.7 Melem/s | 2.3x |
| 50     | 1.44 ms | 13.9 Melem/s | 5.7x |
| 100    | 3.11 ms | 6.4 Melem/s  | 12.3x |
| 250    | 7.25 ms | 2.7 Melem/s  | 28.7x |

**Diagnosis**: Throughput drops as window increases - classic O(n·w) signature.

---

### Rolling Mean - Data Scaling (w=20, 1 column)

| Rows   | Time    | Throughput | Scaling |
|--------|---------|------------|---------|
| 2k     | 81 µs   | 24.5 Melem/s | 1.0x |
| 20k    | 652 µs  | 30.7 Melem/s | 8.0x |
| 200k   | 6.07 ms | 32.8 Melem/s | 74.9x |

**Diagnosis**: Good O(n) scaling in data size. Throughput relatively constant ~25-33 Melem/s.

---

### Rolling Std - Window Scaling (20k rows, 1 column)

| Window | Time | Throughput | O(n·w) Factor |
|--------|------|------------|---------------|
| 5      | 758 µs  | 26.4 Melem/s | 1.0x |
| 20     | 1.23 ms | 16.3 Melem/s | 1.6x |
| 50     | 2.56 ms | 7.8 Melem/s  | 3.4x |
| 100    | 4.06 ms | 4.9 Melem/s  | 5.4x |
| 250    | 10.8 ms | 1.8 Melem/s  | 14.3x |

**Diagnosis**: Even worse O(n·w) behavior than rolling_mean (more expensive per window).

---

### Rolling Mean - NA Rate Sensitivity (20k rows, w=20)

| NA Rate | Time   | Throughput | Impact |
|---------|--------|------------|--------|
| 0%      | ~650 µs | ~31 Melem/s | baseline |
| 5%      | ~650 µs | ~31 Melem/s | no change |
| 20%     | ~650 µs | ~31 Melem/s | no change |

**Diagnosis**: NA skipping has negligible overhead (good contract enforcement).

---

## Root Cause Analysis

### Current Implementation (src/exec.rs:394-424)

```rust
fn rolling_mean_column(col: &Column, w: usize) -> Column {
    for i in (w - 1)..nrows {                    // Outer loop: O(n)
        let window_start = i + 1 - w;
        let window_end = i + 1;

        let mut sum = 0.0;
        let mut count = 0;

        for &x in &data[window_start..window_end] {  // Inner loop: O(w)
            if !x.is_nan() {
                sum += x;
                count += 1;
            }
        }

        if count >= w {
            result[i] = sum / (w as f64);
        }
    }
}
```

**Problem**: Recomputes sum over entire window on every iteration.

---

### Optimized Approach: Running Sums (O(n))

```rust
fn rolling_mean_column_optimized(col: &Column, w: usize) -> Column {
    let mut running_sum = 0.0;
    let mut valid_count = 0;

    for i in 0..nrows {
        // Add entering value (if valid)
        if i < data.len() && !data[i].is_nan() {
            running_sum += data[i];
            valid_count += 1;
        }

        // Remove leaving value (if valid)
        if i >= w {
            let leaving_idx = i - w;
            if !data[leaving_idx].is_nan() {
                running_sum -= data[leaving_idx];
                valid_count -= 1;
            }
        }

        // Emit result
        if i >= w - 1 && valid_count >= w {
            result[i] = running_sum / (w as f64);
        }
    }
}
```

**Complexity**: O(n) - single pass, constant work per element.

---

## Expected Improvements

### Theoretical Speedup (w=20, n=20k)

| Operation | Current | Optimized | Speedup |
|-----------|---------|-----------|---------|
| rolling_mean | O(n·w) = 400M ops | O(n) = 20k ops | **20,000x** |

**Reality check**: Actual speedup will be lower due to:
- Memory bandwidth (not compute-bound)
- Cache effects (running sums are cache-friendly)
- Branch misprediction (NA checks)

**Conservative estimate**: 10-50x faster for w=20-250.

---

### Empirical Target (after optimization)

| Window | Current Throughput | Target Throughput | Goal |
|--------|-------------------|-------------------|------|
| 5      | 33.7 Melem/s | 50+ Melem/s | Memory bandwidth limit |
| 20     | 33.7 Melem/s | 50+ Melem/s | **Constant throughput** |
| 50     | 13.9 Melem/s | 50+ Melem/s | **3.6x improvement** |
| 100    | 6.4 Melem/s  | 50+ Melem/s | **7.8x improvement** |
| 250    | 2.7 Melem/s  | 50+ Melem/s | **18.5x improvement** |

**Key invariant**: Throughput should be **constant across window sizes** (memory-bound, not compute-bound).

---

## Performance Gates

**Baseline numbers (no regression allowed)**:
- `rolling_mean(w=20, n=20k)`: **593 µs** / 33.7 Melem/s
- `rolling_std(w=20, n=20k)`: **1.23 ms** / 16.3 Melem/s

**Post-optimization targets**:
- `rolling_mean(w=250, n=20k)`: **< 1 ms** (currently 7.25 ms) = **7x+ improvement required**
- `rolling_std(w=250, n=20k)`: **< 2 ms** (currently 10.8 ms) = **5x+ improvement required**
- Window scaling: Throughput must remain **constant (±20%)** across w=5 to w=250

**Regression policy**: No >20% slowdown without explicit justification and approval.

---

## Next Steps

1. ✅ **Baseline established** - O(n·w) confirmed via benchmarks
2. ⏳ **Optimize rolling_mean** to O(n) with running sums (Task #3)
3. ⏳ **Optimize rolling_std** to O(n) with running (sum, sumsq, count) (Task #4)
4. ⏳ **Verify correctness** - All 116 IR tests must still pass
5. ⏳ **Measure improvement** - Re-run benchmarks, confirm constant throughput
6. ⏳ **Update this doc** - Record post-optimization numbers

---

## Benchmark Invocation

```bash
# Full benchmark suite (~2 minutes)
cargo bench --bench rolling_baseline

# Quick smoke test (~30 seconds)
cargo bench --bench rolling_baseline -- --quick

# Specific benchmark
cargo bench --bench rolling_baseline -- 'rolling_mean_window_scaling'

# Compare before/after optimization
cargo bench --bench rolling_baseline --save-baseline before
# ... make changes ...
cargo bench --bench rolling_baseline --baseline before
```

---

## Appendix: Full Benchmark Suite

The complete benchmark suite includes:
1. **rolling_mean_window_scaling**: w ∈ {5, 20, 50, 100, 250}
2. **rolling_mean_data_scaling**: n ∈ {2k, 20k, 200k}
3. **rolling_mean_na_rate**: NA% ∈ {0, 5, 20}
4. **rolling_std_window_scaling**: w ∈ {5, 20, 50, 100, 250}
5. **rolling_std_data_scaling**: n ∈ {2k, 20k, 200k}
6. **rolling_zscore_pipeline**: derived form (mean + std + div)
7. **rolling_mean_multi_column**: ncols ∈ {1, 5, 10, 50}

All tests use:
- Date index (contiguous)
- NA rate: 5% (realistic)
- Single-column tests: 1 column
- Multi-column tests: up to 50 columns

---

*Document maintained by: Claude Sonnet 4.5*
*Last benchmark run: 2026-02-21*
*Status: Baseline established, ready for optimization*
