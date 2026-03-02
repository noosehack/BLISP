# BLISP Semantic Guarantees

This document defines the semantic contracts that BLISP upholds, with tripwire tests that prevent regressions.

---

## 1. Elementwise Operations: Bitwise Identical

**Contract:** Fused and unfused elementwise operations MUST produce bitwise-identical results, including IEEE-754 special values.

### IEEE-754 Edge Cases

| Input | Operation | Output | Notes |
|-------|-----------|--------|-------|
| `ln(0)` | dlog-obs | `-inf` | NOT `NaN` |
| `ln(negative)` | dlog-obs | `NaN` | |
| `0/0` | dlog-obs | `NaN` | `-inf - (-inf)` |
| `prev=0, x>0` | dlog-obs | `+inf` | `finite - (-inf)` |
| `NaN` input | any op | `NaN` | Propagates |
| `1/0` | inv | `+inf` | NOT error |
| `0/1` | inv | `0` | |
| `inf + finite` | addition | `inf` | |

**Tripwire Tests:**
- `tests/ieee_dlog_obs_tripwire.rs::tripwire_dlog_obs_ln_zero_gives_neg_inf`
- `tests/ieee_dlog_obs_tripwire.rs::tripwire_dlog_obs_zero_over_zero_gives_nan`
- `tests/ieee_dlog_obs_tripwire.rs::tripwire_fused_cs1_dlog_obs_matches_unfused_edge_cases`
- `src/selftest.rs::test_ieee_ln_zero_gives_neg_inf`
- `src/selftest.rs::test_ieee_zero_over_zero_gives_nan`
- `src/selftest.rs::test_ieee_fusion_preserves_edge_cases`

**Policy:**
- ✅ NEVER guard `ln`, `div`, `sqrt`, `inv` with zero checks
- ✅ Let IEEE-754 produce `-inf`, `+inf`, `NaN` naturally
- ✅ Fused operations MUST match unfused bitwise (use `ieee_equal()` for testing)

---

## 2. Non-Associative Reductions: Tolerance-Defined

**Contract:** Reductions over floating-point columns may differ due to summation order, but differences must be within numerical tolerance.

### Tolerance Defaults

| Operation | Default Tolerance | Notes |
|-----------|-------------------|-------|
| `sum`, `mean` | `1e-10` | Summation order affects |
| `std`, `var` | `1e-9` | Two-pass algorithm |
| `rolling-mean` | `1e-10` | Sliding window sums |
| User `verify` | `1e-6` | Configurable via `--tol` |

**Tripwire Tests:**
- `tests/differential_exec.rs` (proptest: IR vs legacy within tolerance)
- Verify subcommand: `blisp verify --tol 1e-6`

**Policy:**
- ✅ Associative changes (e.g., sum order) allowed if within tolerance
- ✅ Two-pass algorithms (std) allowed if more stable
- ❌ Changing algorithm class (e.g., online → batch) requires explicit approval

---

## 3. NA Propagation: Explicit Skip Policy

**Contract:** NA handling must be explicit and documented per operation.

### NA Semantics

| Operation | NA Policy | Example |
|-----------|-----------|---------|
| Elementwise (`+`, `*`, `log`) | **Propagate** | `NA + 1 = NA` |
| `dlog-obs`, `shift-obs` | **Skip** (LOCF) | `dlog([1, NA, 2]) = [NA, NA, ln(2/1)]` |
| `sum`, `mean` | **Skip** (implicit `sum0`) | `sum([1, NA, 2]) = 3` |
| `std`, `wstd` | **Skip** | `std([1, NA, 2])` = std of valid values |
| `locf` | **Fill** | `locf([1, NA, 2]) = [1, 1, 2]` |

**Tripwire Tests:**
- `tests/ieee_dlog_obs_tripwire.rs::tripwire_dlog_obs_na_propagation`
- `tests/mask_tripwires.rs::tripwire_masked_rows_produce_na_in_unary_ops`

**Policy:**
- ✅ Elementwise ops: `NA` in → `NA` out (no guarding)
- ✅ OBS (observation) ops: skip `NA`, use last valid observation
- ✅ Aggregations: skip `NA` by default (unless `strict` variant)
- ❌ NEVER silently convert `NA` to `0` or other sentinel

---

## 4. Orientation: H vs Z Must Differ

**Contract:** Orientation changes (`o 'H` vs `o 'Z`) MUST produce materially different results for aggregations.

### Orientation Semantics

| Orientation | Aggregation Direction | Example Output |
|-------------|----------------------|----------------|
| `H` (horizontal) | Down columns | `sum` → column sums |
| `Z` (vertical) | Across rows | `sum` → row sums |
| `N`, `S` | Column-major variants | Same as `H` |
| `_H`, `_Z`, etc. | Negative orientations | Reverse order |

**Example:**
```lisp
; Table: A=[1,3,5], B=[2,4,6]
(sum table)        ; H (default): [9, 12] (column sums)
(sum (o 'Z table)) ; Z: [3, 7, 11] (row sums)
```

**Tripwire Tests:**
- `tests/orientation_tripwires.rs::tripwire_orientation_z_affects_sum`
- `tests/orientation_tripwires.rs::tripwire_all_rowwise_vs_colwise_orientations`
- `src/selftest.rs::test_orientation_h_vs_z_different_shapes`

**Policy:**
- ✅ `H` and `Z` MUST produce different result shapes
- ✅ Orientation must affect ALL aggregation operations (`sum`, `mean`, `wstd`, etc.)
- ❌ NEVER ignore orientation parameter

---

## 5. Mask Operations: Explicit Row Suppression

**Contract:** Masked rows MUST produce `NA` for numeric operations, not be silently dropped.

### Mask Semantics

| Scenario | Behavior | Example |
|----------|----------|---------|
| Weekend mask + `dlog` | Masked rows → `NA` | `(with-mask 'wkd (dlog prices))` |
| Mask + rolling ops | Window skips masked | `(with-mask m (wstd x 5 1))` |
| Binary ops | OR active masks | `(+ masked1 masked2)` → union |

**Tripwire Tests:**
- `tests/mask_tripwires.rs::tripwire_masked_rows_produce_na_in_unary_ops`
- `tests/mask_tripwires.rs::tripwire_binary_ops_or_active_masks`
- `src/selftest.rs::test_mask_weekend_detection`

**Policy:**
- ✅ Masked row → `NA` output (visible to user)
- ✅ Weekend detection: `(4 + date) % 7 ∈ {0, 6}` (Saturday=6, Sunday=0)
- ❌ NEVER silently drop masked rows

---

## 6. Verification Semantics (blisp verify)

**Contract:** CSV verification uses IEEE-754 aware comparison with configurable tolerance.

### Verify Algorithm

```rust
fn ieee_equal(a: f64, b: f64, tolerance: f64) -> bool {
    match (a.is_nan(), b.is_nan()) {
        (true, true) => true,              // NaN == NaN (bitwise)
        (false, false) => {
            if a.is_infinite() && b.is_infinite() {
                a.signum() == b.signum()   // +inf == +inf, -inf == -inf
            } else if a.is_finite() && b.is_finite() {
                (a - b).abs() <= tolerance // Within tolerance
            } else {
                false                      // Mixed finite/infinite
            }
        }
        _ => false                         // Mixed NaN/finite
    }
}
```

### Comparison Rules

| Case | Result | Notes |
|------|--------|-------|
| `NaN` vs `NaN` | **Equal** | Bitwise comparison |
| `+inf` vs `+inf` | **Equal** | Bitwise comparison |
| `-inf` vs `-inf` | **Equal** | Bitwise comparison |
| `+inf` vs `-inf` | **Not equal** | Different infinities |
| `1.0` vs `1.0000001` | **Equal** (tol=1e-6) | Within default tolerance |
| `1.0` vs `1.01` | **Not equal** (tol=1e-6) | Exceeds tolerance |
| `NaN` vs `0.0` | **Not equal** | Mixed types |
| `inf` vs `1e308` | **Not equal** | Finite vs infinite |

**Tripwire Tests:**
- `src/verify.rs::tests::test_ieee_equal_nan`
- `src/verify.rs::tests::test_ieee_equal_inf`
- `src/verify.rs::tests::test_ieee_equal_finite`
- CI: `user-smoke` job with verify subcommand

**Policy:**
- ✅ `NaN == NaN` (bitwise, not IEEE-754 `!=`)
- ✅ `inf == inf` (same sign)
- ✅ Finite values: within `--tol` (default `1e-6`)
- ❌ NEVER fail on NaN vs NaN mismatch
- ❌ NEVER use string comparison for numbers

---

## 7. Fusion Correctness

**Contract:** Fused IR operations MUST produce identical results to unfused pipelines.

### Fusion Classes

| Fusion Type | Example | Correctness Test |
|-------------|---------|------------------|
| Elementwise chain | `abs(log(x))` | Bitwise identical |
| Reduction after elementwise | `cs1(dlog(x))` | Bitwise identical |
| Multi-stage pipeline | `wzs(cs1(dlog(x)))` | Within tolerance |

**Tripwire Tests:**
- `tests/ieee_dlog_obs_tripwire.rs::tripwire_fused_cs1_dlog_obs_matches_unfused_edge_cases`
- `tests/ieee_dlog_obs_tripwire.rs::tripwire_fused_dlog_obs_elementwise_matches_unfused`
- `tests/differential_exec.rs` (proptest: all fused vs unfused)

**Policy:**
- ✅ Fusion MUST preserve semantics (bitwise for elementwise, tolerance for reductions)
- ✅ Edge cases (ln(0), 0/0, NaN) MUST match unfused
- ❌ NEVER change semantics for performance

---

## 8. Platform Guarantees

**Contract:** BLISP requires standard IEEE-754 double-precision floats.

### Platform Requirements

| Requirement | Expected | Validation |
|-------------|----------|------------|
| `f64` size | 8 bytes | `selftest` checks |
| Endianness | Little-endian (assumed) | Not tested |
| IEEE-754 | Compliant | `ln(0) = -inf` test |

**Tripwire Tests:**
- `src/selftest.rs::test_platform_f64_size`

**Policy:**
- ✅ Assume IEEE-754 compliance
- ✅ Fail selftest if `sizeof(f64) != 8`
- ❌ Do NOT support non-IEEE-754 platforms

---

## Summary: Semantic Hierarchy

1. **Correctness > Performance**: Never trade semantics for speed
2. **Explicit > Implicit**: NA handling, orientation, masks must be visible
3. **IEEE-754 Native**: No guarding, let special values flow
4. **Bitwise for Elementwise**: Fused must match unfused exactly
5. **Tolerance for Reductions**: Summation order allowed to differ within bounds
6. **Tripwires Enforce**: Every semantic rule has automated tests

**Regression Prevention:**
- All semantic rules enforced by tripwire tests in `tests/` and `src/selftest.rs`
- CI fails if any tripwire breaks
- Proptest discovers edge cases (`tests/differential_exec.rs`)

**When in Doubt:**
- Check existing tripwire tests for precedent
- Add new tripwire test for new semantics
- Document in this file
