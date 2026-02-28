# Reproducibility and Guardrails Checklist

**Date**: 2026-02-28
**Branch**: `reconstruct/tableview-only`
**HEAD**: ef90a49

## Status Overview

| Item | Status | Evidence |
|------|--------|----------|
| Toolchain pinned | ✅ DONE | `rust-toolchain.toml` (Rust 1.93.1) |
| Cargo.lock committed | ✅ DONE | Tracked in git since inception |
| CI uses --locked | ✅ DONE | All cargo commands in CI enforce lockfile |
| Zero clippy warnings | ✅ DONE | `-D warnings` in CI |
| Primary golden (GLD_NUM) | ✅ DONE | Validates financial pipeline |
| Secondary goldens | ⏳ TODO | Need 1-2 more curated pipelines |
| Metamorphic golden | ⏳ TODO | Need invariance-based test |
| Differential fuzz (AST≡IR) | ⏳ TODO | Property test on small random frames |
| Numeric policy (inf/NaN) | ⏳ TODO | Need explicit spec |
| Performance baselines | ⏳ TODO | Benchmark suite for regression detection |
| Clean-room bootstrap | ⏳ TODO | Docker/container test |

---

## ✅ DONE: Reproducible Builds

### Toolchain Lock (`rust-toolchain.toml`)
```toml
[toolchain]
channel = "1.93.1"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

**Rationale**: Pins exact Rust version. CI and local devs use same compiler.

### Dependency Lock (`Cargo.lock`)
- **Status**: Committed and tracked
- **Enforcement**: All CI commands use `--locked` flag
- **Verification**:
  ```bash
  cargo build --locked --lib
  cargo test --locked --workspace --all-features
  cargo clippy --locked --all-targets --all-features -- -D warnings
  ```

### CI Commands Match Local
```yaml
# Format check
cargo fmt --check

# Clippy (strict)
cargo clippy --locked --all-targets --all-features -- -D warnings

# Tests
cargo test --locked --lib
cargo test --locked --test blawktrust_api_integration

# Builds
cargo build --locked --lib --release
cargo build --locked --bin blisp --release
```

**Result**: Builds are **deterministic** across environments.

---

## ✅ DONE: Primary Golden Test (GLD_NUM.csv)

**Location**: `/home/ubuntu/blisp/GLD_NUM_BLISP.sh`

**Pipeline**:
```lisp
(let* ((s (-> (stdin) (w5) (dlog) (x- 1) (cs1) (wzs 25 1) (> -1) (shift 2))))
  (-> (file "../GC1C.csv") (mapr s) (dlog) (ur 250 5) (* s) (cs1)))
```

**Validates**:
- w5 (5-day rolling mean)
- dlog (observation-based semantics)
- cs1 (cumulative sum)
- wzs (winsorized z-score)
- mapr (as-of join)
- ur (250-day rolling with 5-day step)

**Result**: 6826 rows, numeric equivalence with CLISPI reference (tolerance: 1e-6)

**Coverage**:
- ✅ Rolling windows (OBS semantics)
- ✅ Joins (mapr)
- ✅ Windowing (ur)
- ✅ NA propagation
- ❌ Irregular timestamps (all trading days, no gaps)
- ❌ High NA density (financial data has low NA rate)
- ❌ Mask reindexing edge cases

---

## ⏳ TODO: Secondary Golden Tests

### Golden 2: Irregular Timestamps + High NA Density

**Purpose**: Stress OBS semantics with calendar gaps and missing data

**Proposed pipeline**:
```lisp
; Synthetic data with:
; - Weekends (NA clusters)
; - Holidays (irregular gaps)
; - 30% NA rate in values
(-> data
    (rolling-mean 5)    ; OBS should skip weekend NAs
    (locf)              ; Fill forward
    (dlog)              ; Returns after locf
    (rolling-std 10))   ; Volatility
```

**Expected behavior**: Document how OBS handles:
- Saturday/Sunday gaps in rolling windows
- Multi-day holiday periods
- High NA density (30% vs GLD_NUM's ~2%)

**Status**: ❌ Not implemented

---

### Golden 3: Mask + Reindex Stress Test

**Purpose**: Validate mask interaction with joins and orientation changes

**Proposed pipeline**:
```lisp
; Two frames with different index density
; x: daily, y: weekly
; Join creates mask, then operations on masked data
(let ((masked (mapr x y)))
  (-> masked
      (o 'Z)         ; Orientation change
      (rolling-mean 5)
      (o 'X)         ; Back to original
      (cs1)))
```

**Expected behavior**: Mask should:
- Propagate correctly through orientation changes
- Interact correctly with rolling ops
- Preserve NA positions

**Status**: ❌ Not implemented

---

## ⏳ TODO: Metamorphic Golden (Invariance Test)

**Purpose**: Catch semantic drift via algebraic properties

### Proposed: Scaling Invariance for zscore
```lisp
; Property: zscore(k*x) == zscore(x) for any scalar k ≠ 0
; Implementation:
(let* ((x (load "test_data.csv"))
       (k 42.7)
       (z1 (rolling-zscore 10 x))
       (z2 (rolling-zscore 10 (* x k))))
  (assert-equiv z1 z2 :tolerance 1e-10))
```

**Validates**: Rolling zscore implementation is scale-invariant (expected property)

### Proposed: Shift Invariance for Returns
```lisp
; Property: dlog(x) should be invariant to additive shifts in log-space
; i.e., dlog(x) == dlog(exp(c) * x) - c  (for appropriate c)
```

**Status**: ❌ Not implemented

**Recommendation**: Start with zscore scaling invariance (simpler to verify)

---

## ⏳ TODO: Differential Fuzz Testing (AST ≡ IR)

**Purpose**: Catch semantic drift on random small frames before it reaches production

### Proposed Test
```rust
proptest! {
    #[test]
    fn differential_fuzz_small_frames(
        seed in any::<u64>(),
        nrows in 5usize..30,      // Small enough to be fast
        ncols in 1usize..3,
        na_rate in 0.0..0.5,      // Up to 50% NA
        op_depth in 0usize..3     // Pipeline depth
    ) {
        let mut rt = Runtime::new();

        // Generate random frame
        let frame = gen_random_frame(seed, nrows, ncols, na_rate);

        // Generate random operation pipeline
        let pipeline = gen_random_pipeline(seed, op_depth, &SAFE_OPS);

        // Evaluate both ways
        let ast_result = direct_eval(&pipeline, &frame, &rt.interner)?;
        let ir_result = execute(&plan(&normalize(pipeline), &rt.interner)?, &mut rt)?;

        // Assert equivalence
        assert_frame_equiv(&ast_result, &ir_result);
    }
}
```

**Safe operation subset** (for fuzzing):
- Unary: dlog, ret, abs, log, exp
- Binary: +, -, *, / (with constants)
- Rolling: rolling-mean, rolling-std (small windows)
- **Exclude**: joins (mapr/asofr) initially (too many edge cases)

**Why small frames?**
- Fast: 100 test cases in ~1 second
- Cheap: Runs on every commit
- Catches: OBS semantic drift, NA handling bugs, mask issues

**Status**: ❌ Not implemented

---

## ⏳ TODO: Numeric Policy Specification

### Current State: **UNDEFINED** (causes 3 ignored tests)

The following operations have **unspecified behavior**:

| Operation | Input | AST returns | IR returns | Policy needed? |
|-----------|-------|-------------|------------|----------------|
| `log(0.0)` | Zero | `-inf` | `NaN` | ✅ YES |
| `log(-x)` | Negative | `NaN` | `NaN` | ✅ Specify |
| `1.0 / 0.0` | Div-by-zero | `inf` | `NaN` | ✅ YES |
| `sqrt(-x)` | Negative | `NaN` | `NaN` | ✅ Specify |
| `dlog` on zeros | Price = 0 | `-inf` | `NaN` | ✅ YES |

### Ignored Tests Blocked by Missing Policy

1. `diff_small_unary_dlog` - dlog produces inf vs NaN
2. `diff_prop_small_date_frames` - Random pipelines hit edge cases
3. `diff_prop_small_timestamp_frames` - Same

**Questions to answer**:

1. **Division by zero**: Return `inf`, `-inf`, or `NaN`?
   - **Recommendation**: `NaN` (consistent with NA semantics)
   - Rationale: `inf` is not a "missing value" but `NaN` is

2. **Logarithm of zero/negative**: Return `-inf` or `NaN`?
   - **Recommendation**: `NaN` for both
   - Rationale: In financial context, `log(0)` is nonsensical (price can't be 0)

3. **Float comparison tolerance**: Exact, ULP-based, or epsilon?
   - **Recommendation**: ULP-based for most, exact for inf/NaN
   - Rationale: Allows for rounding differences, but detects semantic bugs

### Proposed Policy (needs approval)

```rust
// NUMERIC_POLICY.md

1. Division by zero → NaN
2. log(x) where x ≤ 0 → NaN
3. sqrt(x) where x < 0 → NaN
4. Comparison tolerance:
   - Integers: exact equality
   - Floats: 4 ULPs or 1e-10 relative (whichever is larger)
   - inf: exact equality
   - NaN: both sides NaN or both sides valid
```

**Status**: ⏳ Needs decision + documentation

---

## ⏳ TODO: Performance Regression Guardrails

**Purpose**: Prevent accidental O(n·w) regressions before fusion work

### Proposed Benchmark Suite

Location: `benches/regression_guardrails.rs`

```rust
criterion_group! {
    name = regression_guardrails;
    config = Criterion::default()
        .sample_size(20)        // Fast
        .warm_up_time(Duration::from_millis(500));
    targets =
        bench_rolling_mean_throughput,
        bench_dlog_throughput,
        bench_mapr_join_small
}

fn bench_rolling_mean_throughput(c: &mut Criterion) {
    // 10k rows, window=250
    // Baseline: ~500 µs (pre-fusion)
    // Tripwire: Fail if > 2x regression
}
```

**Metrics to track**:
- **Throughput**: rows/second
- **Latency**: p50, p99 for small frames
- **Allocations**: Prevent unnecessary Vec allocations

**CI integration**:
```yaml
- name: Run regression benchmarks
  run: cargo bench --bench regression_guardrails -- --save-baseline main
  # Compare to baseline on PR
```

**Status**: ❌ Not implemented

**Recommendation**: Add BEFORE starting fusion work (baseline = pre-fusion perf)

---

## ⏳ TODO: Clean-Room Bootstrap Test

**Purpose**: Prove reproducibility on fresh machine

### Proposed Dockerfile

```dockerfile
FROM rust:1.93.1-slim

WORKDIR /build

# Clone repo (or COPY if testing local)
RUN apt-get update && apt-get install -y git
RUN git clone https://github.com/noosehack/BLISP.git
WORKDIR /build/BLISP

# Checkout specific SHA
ARG COMMIT_SHA
RUN git checkout $COMMIT_SHA

# Build and test (exactly as CI does)
RUN cargo build --locked --lib
RUN cargo test --locked --workspace --all-features
RUN cargo clippy --locked --all-targets --all-features -- -D warnings

# Verify output matches expected
RUN cargo build --locked --release --bin blisp
# Could run GLD_NUM here if data available
```

**Usage**:
```bash
docker build --build-arg COMMIT_SHA=ef90a49 -t blisp-verify .
```

**Status**: ❌ Not implemented

---

## Summary: What's Needed

### Immediate (blocks full reproducibility)
1. ✅ ~~Toolchain pin~~ (DONE: ef90a49)
2. ✅ ~~CI --locked~~ (DONE: ef90a49)

### High Priority (blocks semantic confidence)
3. ⏳ **Numeric policy** (inf/NaN) - 3 ignored tests waiting
4. ⏳ **Differential fuzz test** - AST≡IR on small random frames
5. ⏳ **Secondary golden** - Irregular timestamps + high NA

### Medium Priority (nice-to-have before fusion)
6. ⏳ **Metamorphic golden** - Scaling invariance
7. ⏳ **Performance baselines** - Regression tripwires

### Low Priority (can defer)
8. ⏳ **Clean-room bootstrap** - Dockerfile verification

---

## Next Actions

**For user to decide**:
1. Numeric policy: Approve `NaN` for all edge cases? (see Proposed Policy above)
2. Secondary golden: Which pipeline to use? (see Golden 2 proposal)
3. Differential fuzz: Approve safe-ops subset? (see Fuzz proposal)

**For implementation** (after decisions):
1. Create `NUMERIC_POLICY.md` with approved rules
2. Update AST and IR to match policy
3. Remove 3 `#[ignore]` from differential_exec tests
4. Add differential fuzz property test
5. Add secondary golden test
6. Add performance baselines (before fusion)

**Verification commands** (already working):
```bash
cargo build --locked --lib
cargo test --locked --workspace --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
./GLD_NUM_BLISP.sh && python3 verify_gld_num.py
```
