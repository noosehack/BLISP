# 🚨 PR2 CRITICAL FINDING: Semantic Divergence

**Date**: 2026-02-26
**Status**: ⛔ BLOCKING - Cannot proceed with naive unification
**Severity**: HIGH - Semantic difference, not implementation difference

---

## Summary

**The two `dlog_column` implementations are FUNDAMENTALLY DIFFERENT operations.**

This is NOT a case of duplicate implementations of the same function.
This is TWO DIFFERENT ALGORITHMS with different semantics.

---

## Implementation Comparison

### LOCAL (exec.rs:1092) - NA-SKIPPING LAG

```rust
fn dlog_column(col: &Column, _lag: usize) -> Column {
    let mut last_valid: Option<f64> = None;

    for &x in data.iter() {
        if x.is_nan() {
            result.push(f64::NAN);  // Skip NA
        } else if let Some(prev) = last_valid {
            if prev > 0.0 && x > 0.0 {
                result.push(x.ln() - prev.ln());
            } else {
                result.push(f64::NAN);
            }
            last_valid = Some(x);  // Update last valid
        } else {
            result.push(f64::NAN);
            last_valid = Some(x);
        }
    }
}
```

**Semantics**:
- ✅ **NA-skipping**: Looks back to LAST VALID observation
- ✅ **Negative guard**: Returns NA if prev ≤ 0 or x ≤ 0
- ✅ **First valid**: Returns NA (no previous to compare)
- ✅ **Lag parameter**: IGNORED (hardcoded NA-skipping behavior)

**Use Case**: "Business days dlog" with weekend masks

**Example**:
```
Input:  [100, NA, NA, 110, 120]
Output: [NA, NA, NA, ln(110/100), ln(120/110)]
                      ^-- skipped 2 NAs to find 100
```

### BLAWKTRUST (kernels_masked.rs:14) - FIXED POSITIONAL LAG

```rust
pub fn dlog_no_nulls(out: &mut [f64], x: &[f64], lag: usize) {
    out[..lag].fill(f64::NAN);  // Prefix NA

    unsafe {
        for i in lag..n {
            let curr = *xp.add(i);
            let prev = *xp.add(i - lag);
            *op.add(i) = curr.ln() - prev.ln();  // NO NA check!
        }
    }
}
```

**Semantics**:
- ⛔ **Fixed positional lag**: `x[i] - x[i-lag]` (calendar lag)
- ⛔ **No NA skipping**: Uses x[i-lag] even if NA
- ⛔ **No negative guard**: `ln(x)` where x ≤ 0 → NaN naturally
- ⛔ **Lag parameter**: RESPECTED (positional offset)

**Use Case**: "Calendar days dlog" without masks

**Example**:
```
Input:  [100, NA, NA, 110, 120]
Output: [NA, ln(NA/100)=NA, ln(NA/NA)=NA, ln(110/NA)=NA, ln(120/110)]
             ^-- lag=1, used NA as prev
```

---

## Semantic Matrix

| Property | LOCAL (exec.rs) | BLAWKTRUST (kernels_masked.rs) | Match? |
|----------|-----------------|--------------------------------|--------|
| **NA Handling** | Skip, find last valid | Use positionally (becomes NA) | ❌ NO |
| **Lag Semantics** | Observation-based | Position-based | ❌ NO |
| **Negative Guard** | YES (explicit check) | NO (implicit NaN) | ⚠️ Similar result |
| **First N rows** | First valid → NA | First `lag` → NA | ⚠️ Close |
| **Mask Aware** | YES (via NA skip) | NO | ❌ NO |
| **Lag Parameter** | IGNORED | USED | ❌ NO |

**Verdict**: ❌ **NOT EQUIVALENT**

---

## Why This Matters

### Current IR Executor Usage (exec.rs:157)

```rust
NumericFunc::SHF_PTW_NLN_DLOG => dlog_column(col, 1),
```

**Calls**: LOCAL (NA-skipping) version

**IR Taxonomy**:
- `SHF_PTW_NLN_DLOG` = Shift-equivariant, Pointwise, Nonlinear, DiffLog
- "Shift-equivariant" suggests **observation-based** (not position-based)

**Implication**: Local version is CORRECT for the IR semantics.

### BLISP V2 Blueprint Requirement

From blueprint section 6.1:
> If a kernel is in `blawktrust::builtins::ops`, BLISP must call *that* from IR executor

**Problem**: Blindly following this would BREAK semantics.

---

## Root Cause

The `blawktrust::dlog_column` function was designed for:
- **Clean data** (no NAs expected)
- **Calendar-based lag** (positional offset)
- **High performance** (unsafe, no branches)

The `exec.rs::dlog_column` was designed for:
- **Masked time series** (NAs from weekend masking)
- **Business-day lag** (skip non-trading days)
- **Financial correctness** (negative guards)

**These serve different use cases.**

---

## What This Reveals About Architecture

### IR Has Two Lag Concepts

1. **`SHF_PTW_LIN_SHF{k}`** (planner.rs:135)
   - Fixed positional shift
   - Should use: `x[i-k]` (position-based)
   - Blawktrust version appropriate

2. **`LAG_OBS{k}`** (planner.rs:153)
   - Observation-based lag (skip masked rows)
   - Should use: last k eligible observations
   - Local version appropriate

### But `dlog` Uses Fixed Name

**Current**: `NumericFunc::SHF_PTW_NLN_DLOG` (line 157)
- Uses NA-skipping (observation-based)
- Name suggests "Shift" but behaves like "LAG_OBS"

**Mismatch**: Taxonomy vs Implementation

---

## Correct Resolution Path

### Option 1: Keep LOCAL, Rename blawktrust Function ✅ SAFE

**Action**:
1. Keep `exec.rs:1092` dlog_column (NA-skipping)
2. Rename blawktrust version to `dlog_column_fixed_lag`
3. Document semantic difference
4. Update tripwire to allow this case

**Pros**: No semantic changes, preserves correctness
**Cons**: Tripwire still flags it (need whitelist)

### Option 2: Create Two IR Variants ✅ CORRECT

**Action**:
1. Add `NumericFunc::SHF_PTW_NLN_DLOG_OBS` (observation-based)
2. Keep `NumericFunc::SHF_PTW_NLN_DLOG` (position-based)
3. Map current usage to `_OBS` variant
4. Use blawktrust for position-based, local for obs-based

**Pros**: Explicit semantics, follows taxonomy
**Cons**: Requires planner changes

### Option 3: Mask-Aware Switch ⚠️ COMPLEX

**Action**:
1. Check if active_mask exists at runtime
2. Use NA-skipping if mask active, positional otherwise
3. Unified interface, divergent behavior

**Pros**: Single function name
**Cons**: Hidden complexity, hard to reason about

---

## Recommended Action for PR2

### PAUSE PR2 Kernel Unification ⛔

**Reason**: These are not duplicate kernels, they are different operations.

### Immediate Actions:

1. **Update Tripwire Whitelist**:
   ```bash
   # In ci/test_no_kernel_dupes.sh
   # Whitelist: dlog_column (different semantics, not duplication)
   ```

2. **Document Semantic Difference**:
   - Add comment in exec.rs:1092 explaining NA-skipping
   - Add comment in blawktrust explaining fixed-lag
   - Reference this report

3. **Verify Shift Semantics**:
   - Check `shift_column` (exec.rs:1350)
   - Ensure it's position-based (NOT observation-based)
   - This is the second high-risk operator

4. **Architectural Decision**:
   - Decide if we need both variants in IR
   - Update taxonomy if observation-based lag is legitimate
   - OR: refactor to use LAG_OBS{k} for masked paths

---

## Test That Would Have Caught This

```rust
#[test]
fn test_dlog_na_skipping_semantics() {
    let data = vec![100.0, f64::NAN, f64::NAN, 110.0, 120.0];

    // Local version (NA-skipping)
    let local_result = exec::dlog_column(&Column::F64(data.clone()), 1);

    // Blawktrust version (positional)
    let blawktrust_result = blawktrust::dlog_column(&Column::F64(data.clone()), 1);

    // They MUST differ on position 3 (after NAs)
    assert_ne!(local_result, blawktrust_result);

    // Local: ln(110/100) because it skipped NAs
    // Blawktrust: NA because prev was NA
}
```

**This test MUST be added before any unification.**

---

## Impact on V2 Blueprint

### Section 6.1 Needs Clarification

Original:
> If a kernel is in `blawktrust::builtins::ops`, BLISP must call *that*

**Amendment Needed**:
> If a kernel is in `blawktrust::builtins::ops` **and has matching semantics**,
> BLISP must call that. If semantics differ, maintain separate implementations
> and document the distinction.

### Kernel Authority Policy Update

**New Rule**:
1. Semantic equivalence must be PROVEN before unification
2. Differential tests MANDATORY (not optional)
3. Mask-aware vs mask-unaware are DIFFERENT operations
4. Tripwire should detect duplication, not semantic variants

---

## Conclusion

**PR2 Cannot Proceed as Originally Scoped.**

This is NOT a bug to fix, this is an ARCHITECTURAL DISCOVERY:
- Two different lag semantics exist in the codebase
- Both are needed for different use cases
- Taxonomy needs clarification
- Kernel authority needs semantic matching policy

**Next Steps**:
1. Document this finding in architecture docs
2. Add differential test proving divergence
3. Update tripwire to whitelist semantic variants
4. Decide on observation-based vs position-based taxonomy
5. Only THEN consider unification (if semantics align)

---

**END OF CRITICAL REPORT**

*"Prove equivalence before deletion. We just proved NON-equivalence."*
