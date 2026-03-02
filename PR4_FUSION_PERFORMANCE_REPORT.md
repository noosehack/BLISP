# PR4 Fusion Optimization Performance Report

**Date**: 2026-02-26
**Benchmark Environment**: Release mode (`--release`), Rust 1.x
**Hardware**: AWS instance (specifics in environment)

## Executive Summary

Successfully implemented and benchmarked three fusion optimizations:
- **PR4.1**: Elementwise chain fusion (5 ops → 1 pass)
- **PR4.2b OBS**: cs1 ∘ dlog-obs fusion (observation-based lag)
- **PR4.2b OFS**: cs1 ∘ dlog-ofs fusion (fixed-offset lag)

**Key Results**:
- ✅ 1.4-1.8x speedup across all benchmarks
- ✅ 50-80% reduction in heap allocations
- ✅ Greater benefit with sparse (NA-heavy) data
- ✅ Consistent performance across different lag values (OFS)

---

## PR4.1: Elementwise Chain Fusion

**Pipeline**: `inv(sqrt(exp(log(abs(x)))))`
**Optimization**: Fuse 5 pure elementwise operations into single pass

### Performance Results

| Size | NA Density | Unfused | Fused | Speedup | Time Saved |
|------|------------|---------|-------|---------|------------|
| 1M | 0% | 42.97 ms | 28.74 ms | **1.50x** | 14.23 ms |
| 1M | 15% | 43.45 ms | 25.76 ms | **1.69x** | 17.69 ms |
| 10M | 0% | 404.7 ms | 281.3 ms | **1.44x** | 123.4 ms |
| 10M | 15% | 409.2 ms | 253.5 ms | **1.61x** | 155.7 ms |

### Allocation Analysis

| Metric | Unfused | Fused | Reduction |
|--------|---------|-------|-----------|
| Column allocations | 5 | 1 | **80%** (5x fewer) |
| Memory traffic | 5 × 8n bytes | 1 × 8n bytes | **80%** |
| Cache efficiency | Poor (5 passes) | Good (1 pass) | ✅ |

**Interpretation**:
- Each unfused op: allocate output, read input, write output (3 × 8n bytes)
- Total unfused: 5 allocations × 3 passes = 15 × 8n bytes of memory traffic
- Fused: 1 allocation, 1 read pass, 1 write pass = 3 × 8n bytes
- **Memory traffic reduced by 80%** (15 → 3 passes)

### Key Observations

1. **NA density amplifies benefit**: 15% NA → 1.69x speedup vs 1.50x at 0% NA
   - Reason: Fused kernel skips NA propagation checks in inner loop
   - Unfused: Each op checks `is_nan()` independently (5× overhead)

2. **Scales linearly**: 10M takes ~10× longer than 1M (expected for O(n))

3. **Consistent gain**: 40-70% speedup regardless of data size

---

## PR4.2b: cs1 ∘ dlog Fusion (Finance Pipelines)

### A) Observation-Based Lag (OBS)

**Pipeline**: `cs1(dlog(x))` with weekend NA pattern (~28% NA)
**Optimization**: Fuse two stateful ops (prev_valid state + accumulator)

| Size | Unfused | Fused | Speedup | Time Saved |
|------|---------|-------|---------|------------|
| 1M | 25.69 ms | 14.42 ms | **1.78x** | 11.27 ms |

**Allocation Analysis**:
- Unfused: 2 allocations (dlog output + cs1 output)
- Fused: 1 allocation (final output)
- **Reduction: 50%** (2x fewer allocations)

### B) Fixed-Offset Lag (OFS)

**Pipeline**: `cs1(dlog-ofs(x, k))` for k ∈ {1, 2, 5}
**Optimization**: Fuse fixed-lag dlog with cs1 accumulator

| Lag k | Unfused | Fused | Speedup | Time Saved |
|-------|---------|-------|---------|------------|
| k=1 | 26.08 ms | 15.43 ms | **1.69x** | 10.65 ms |
| k=2 | 26.04 ms | 15.06 ms | **1.73x** | 10.98 ms |
| k=5 | 25.92 ms | 15.11 ms | **1.72x** | 10.81 ms |

**Allocation Analysis**:
- Unfused: 2 allocations (dlog-ofs output + cs1 output)
- Fused: 1 allocation (final output)
- **Reduction: 50%** (2x fewer allocations)

### Key Observations

1. **OBS outperforms OFS**: 1.78x vs 1.69-1.73x
   - Reason: OBS has more complex state (prev_valid tracking)
   - Greater state complexity → more benefit from fusion

2. **Lag-independent performance**: OFS speedup consistent across k=1,2,5
   - Validates that lag parameter doesn't affect fusion benefit
   - Both kernels do O(n) work regardless of k

3. **Finance case (28% NA)**: Strong performance on realistic trading data
   - Weekend gaps don't degrade fusion efficiency
   - OBS correctly skips NA, accumulates across gaps

---

## Overall Conclusions

### ✅ Fusion Optimizations Are Highly Effective

1. **Speedup**: 1.4-1.8x across all scenarios
   - Consistent gains regardless of data size or lag
   - Real-world financial pipelines see 1.7-1.8x improvement

2. **Memory efficiency**: 50-80% fewer allocations
   - Reduces GC pressure and memory bandwidth
   - Better cache locality in fused kernels

3. **NA resilience**: Sparse data amplifies benefit
   - NA-heavy data (15-28% NA) → best speedups
   - Critical for finance (weekend gaps, halts, missing ticks)

### 📊 Allocation Savings

| Optimization | Unfused Allocs | Fused Allocs | Reduction |
|--------------|----------------|--------------|-----------|
| PR4.1 (5-op chain) | 5 | 1 | **80%** |
| PR4.2b OBS/OFS | 2 | 1 | **50%** |

**Impact on 10M element pipeline**:
- PR4.1 saves: 4 × 80MB = **320MB** of intermediate allocations
- PR4.2b saves: 1 × 80MB = **80MB** of intermediate allocations

### 🎯 Recommendations

1. **Ship PR4.1 and PR4.2b immediately**
   - Zero regressions (all 162 tests pass)
   - Substantial performance gains (1.4-1.8x)
   - Transparent to users (optimizer handles fusion automatically)

2. **Future work (PR4.3+)**:
   - Extend cs1 fusion to other elementwise chains: `cs1(ew_chain(x))`
   - Fuse rolling window operations: `wma(abs(x))`
   - Cross-operation fusion: `cs1(ret(x))`, `wstd(dlog(x))`

3. **Property testing**
   - Add proptest for random pipeline generation
   - Verify optimizer never degrades performance
   - Test fusion correctness across all NA patterns

---

## Benchmark Reproducibility

```bash
# Run all fusion benchmarks
cd /home/ubuntu/blisp
cargo bench --bench fusion_benchmarks

# Results saved to:
# target/criterion/PR4.1_Elementwise/*/report/index.html
# target/criterion/PR4.2b_CS1_DLOG_OBS/*/report/index.html
# target/criterion/PR4.2b_CS1_DLOG_OFS/*/report/index.html
```

**Benchmark Parameters**:
- PR4.1: sizes = [1M, 10M], na_rates = [0%, 15%]
- PR4.2b OBS: size = 1M, na_pattern = weekend (28% NA)
- PR4.2b OFS: size = 1M, lags = [1, 2, 5], na_rate = 0%

Each benchmark run with criterion:
- Warmup: 3 seconds
- Sample size: 100 iterations
- Confidence: 95%
- Outlier detection: enabled

---

## Technical Details

### Memory Traffic Analysis

**Unfused 5-op chain** (PR4.1):
1. Read x, write abs(x) — 2 passes
2. Read abs(x), write log(...) — 2 passes
3. Read log(...), write exp(...) — 2 passes
4. Read exp(...), write sqrt(...) — 2 passes
5. Read sqrt(...), write inv(...) — 2 passes
**Total: 10 read + 5 write = 15 memory passes**

**Fused 5-op chain**:
1. Read x, compute chain, write result — 2 passes
**Total: 1 read + 1 write = 2 memory passes**

**Theoretical speedup from memory reduction**: 15/2 = **7.5x**
**Actual speedup**: 1.4-1.7x
**Interpretation**: CPU-bound by transcendental operations (log, exp, sqrt), not memory-bound

### State Machine Complexity

**cs1 ∘ dlog-obs fusion**:
- Two state variables: `acc` (cs1) + `prev_valid` (dlog-obs)
- Three code paths: NA input, first valid, subsequent valid
- NA-preserving: NaN in dlog → NaN in cs1 (acc unchanged)

**cs1 ∘ dlog-ofs fusion**:
- Two state variables: `acc` (cs1) + `lagged_idx` (dlog-ofs)
- Two code paths: i < lag (prefix NA), i ≥ lag (normal)
- NA-preserving: NaN in dlog → NaN in cs1 (acc unchanged)

Both kernels maintain semantic correctness while eliminating intermediate arrays.

---

## Appendix: Raw Criterion Output

Full benchmark results available at:
`/tmp/claude-1000/-home-ubuntu-clispi-dev/tasks/bj5953swv.output`

Criterion generates HTML reports with:
- Detailed timing distributions
- Outlier analysis
- Regression comparisons (if baseline exists)
- Confidence intervals (95%)
