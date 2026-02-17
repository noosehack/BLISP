# NaN Policy Decision - v0.1 to v0.2

**Date:** 2026-02-17
**Decision:** Adopt KDB-ISH skipna-by-default for aggregations

---

## Executive Summary

**Changed:** Window aggregations now **skip NaN by default** (v0.2), not propagate (v0.1).

**Rationale:** Quantitative finance engines need pragmatic NaN handling. One missing tick shouldn't destroy 20-day rolling statistics.

---

## Current State (Before Decision)

### Policy Documents
- `NAN_PROPAGATION_POLICY.md` (v0.1) specified **PROPAGATE** for aggregations

### Actual Code
- ✅ `shift`, `diff`, `dlog`: Boundary NaN works correctly
- ✅ Arithmetic: Propagates NaN (Rust f64 default)
- ❌ Comparisons: Not implemented (but policy frozen)
- ❌ Windows: Not implemented

### Verification

**File:** `/home/ubuntu/blisp/src/builtins.rs`

**shift_column (line 579-594):**
```rust
let mut result = vec![f64::NAN; n];  // Line 583: NaN boundary
for i in lag..n {
    result[i] = data[i - lag];
}
```
✅ Correct

**dlog:** `/home/ubuntu/blawktrust/src/builtins/ops.rs:356, 414`
```rust
assert!(data[0].is_nan());  // Tests confirm boundary NaN
```
✅ Correct

---

## Problem with v0.1 Policy

**v0.1 said:** Aggregations/windows **propagate NaN**

```
❌ wstd([10, 20, NaN, 40, 50], window=3)
   => [NaN, NaN, NaN, NaN, NaN]

Problem: One missing price destroys ALL statistics!
```

**Real-world impact:**
- Can't handle data with occasional missing ticks
- Entire analysis fails from single bad point
- Not how kdb+/pandas/quant systems work

---

## Decision: v0.2 Policy

### SKIP NaN by default in aggregations/windows

```
✅ wstd([10, 20, NaN, 40, 50], window=3)
   => [NaN, NaN, σ(10,20), σ(20,40), σ(40,50)]

Each window computed over its valid observations
```

### Key Rules

**1. Arithmetic (unchanged):**
- Propagate NaN (IEEE 754)
- `NaN + 5 => NaN`

**2. Comparisons (unchanged):**
- Return false (IEEE 754)
- `NaN > 5 => false`

**3. Aggregations/Windows (CHANGED):**
- Skip NaN by default (kdb-ish)
- Output NaN only if **insufficient valid observations**
- `wstd` with ddof=1 needs ≥2 valid points
- `sum` needs ≥1 valid point

**4. Boundary NaN (unchanged):**
- Lag operations produce NaN at boundaries
- `shift(col, 1)[0] => NaN`

---

## Comparison

| Operation | v0.1 | v0.2 (NEW) | Reason |
|-----------|------|------------|--------|
| `10 + NaN` | NaN | NaN | IEEE (unchanged) |
| `NaN > 5` | false | false | IEEE (unchanged) |
| `sum([10,NaN,30])` | NaN | **40** | Skip NaN ← NEW |
| `wstd([10,NaN,30],3)` | NaN | **σ(10,30)** | Skip NaN ← NEW |
| `shift(col,1)[0]` | NaN | NaN | Boundary (unchanged) |

---

## Implementation

### Files Changed

**1. Created: `NAN_POLICY_V02.md`**
- Comprehensive v0.2 specification
- Skipna-by-default for aggregations
- Test requirements
- Migration guide from v0.1

**2. Created: `tests/nan_behavior_tests.rs`**
- 15+ test cases specifying expected behavior
- Tests for implemented operations (boundary NaN)
- Tests for future operations (windows) marked `#[ignore]`
- Serves as executable specification

**3. This file: `NAN_POLICY_DECISION.md`**
- Decision rationale
- Before/after comparison

### Code Changes

**None required** - Window operations not yet implemented.
Tests specify expected behavior for when they are implemented.

### Existing Code Status

✅ **Already correct:**
- `shift`, `diff`, `dlog` boundary behavior
- Arithmetic NaN propagation
- All existing tests passing

❌ **Not implemented yet:**
- Comparison operators (`>`, `<`, etc.)
- Window operations (`wstd`, `wzs`, `ur`)
- Simple aggregations (`sum`, `mean`)

When implementing these, **MUST follow v0.2 spec** (skipna by default).

---

## Rationale for Change

### Why v0.1 was wrong

**Too conservative:**
- Mathematically "pure" but practically unusable
- Real financial data has missing ticks
- Can't analyze real-world datasets

**Not industry standard:**
- pandas: `skipna=True` by default
- kdb+/q: Aggregate functions skip nulls
- SQL: `AVG`, `SUM` skip NULL
- R: `na.rm=TRUE` common

### Why v0.2 is correct

**✅ Quant-friendly:**
- Handle real data with missing points
- Compute meaningful statistics from valid subset
- Match industry tools (kdb+, pandas)

**✅ Pragmatic:**
- 19 valid points in 20-day window → compute std
- Better than returning NaN

**✅ Conservative when needed:**
- Still returns NaN if insufficient valid data
- `std` with ddof=1 needs ≥2 points
- Transparent about data quality

**✅ Flexible:**
- Can add `-strict` variants for propagate behavior
- Default is what quant users expect

---

## Migration Impact

### Breaking Change?

**No** - Window operations don't exist yet!

**Future code:**
- All window functions will implement v0.2 (skipna)
- No migration needed

### If v0.1 behavior needed

Future: Add `-strict` variants:
```lisp
(wstd-strict col 20 0)  ; Propagate NaN
(sum-strict col)        ; Propagate NaN
```

But 99% of users want default skipna behavior.

---

## Test Coverage

**`tests/nan_behavior_tests.rs`** specifies:

✅ **Implemented operations:**
1. `test_shift_boundary_nan` - shift produces boundary NaN
2. `test_diff_boundary_nan` - diff produces boundary NaN
3. `test_dlog_boundary_nan` - dlog produces boundary NaN
4. `test_arithmetic_propagates_nan` - arithmetic propagates
5. Edge cases (lag > length, empty, single element)

⏳ **Future operations (marked `#[ignore]`):**
6. `test_comparison_nan_is_false` - comparisons return false
7. `test_wstd_skips_nan` - wstd skips NaN in window
8. `test_wstd_insufficient_valid` - wstd returns NaN if <2 valid
9. `test_wzs_skips_nan` - wzs skips NaN
10. `test_sum_skips_nan` - sum skips NaN
11. `test_sum_all_nan` - sum returns NaN if all NaN

Tests serve as **executable specification** for future implementation.

---

## Next Steps

When implementing window operations:

1. ✅ Read `NAN_POLICY_V02.md`
2. ✅ Implement skipna behavior
3. ✅ Enable tests in `tests/nan_behavior_tests.rs`
4. ✅ Verify all tests pass
5. ✅ Add operation-specific tests

**Critical:** Do NOT propagate NaN by default in windows!

---

## Approval

**Decision:** APPROVED
**Status:** Policy v0.2 FROZEN ❄️
**Effective:** Immediately for all new code

**Supersedes:** `NAN_PROPAGATION_POLICY.md` (v0.1)
**Authoritative:** `NAN_POLICY_V02.md`

---

## Summary

**Old:** Propagate NaN everywhere (v0.1) - too conservative
**New:** Skip NaN in aggregations (v0.2) - kdb-ish, quant-friendly

**Impact:** Makes blisp usable for real financial data

**Code changes:** None needed now; tests specify future behavior

---

**Version:** 1.0
**Date:** 2026-02-17
**Frozen:** Yes ❄️
