# Mask System Contracts

This document freezes the mask system semantics to prevent future drift.

## 1. Core Definitions

### 1.1 Mask vs NA Distinction

**Mask (excluded)**: A row is structurally present in the index but excluded from computation.
- Stored in `Tags::masks` (named masks) and `Tags::active_mask` (compiled active mask)
- Example: weekend rows in a daily calendar
- `active_mask[i] = true` means row `i` is masked (excluded)

**NA (missing value)**: A data value is missing within an included row.
- Stored as `f64::NAN` in column data
- Example: missing price on a weekday due to data gap

**Orthogonality**: Masks and NAs are independent:
- A row can be masked with valid data (weekend with stale price)
- A row can be unmasked with NA (weekday with missing price)
- A row can be masked with NA (weekend with missing price)

## 2. Fundamental Rules

### 2.1 Masked Rows Always Output NA

**Rule**: For ANY numeric operation, `active_mask[i] = true` ⇒ `output[i] = NA`

**Rationale**: Masked rows represent excluded observations. Even if input data is valid, the output must be NA to indicate the row is not part of the computation.

**Applies to**:
- All unary numeric operations: `dlog`, `diff`, `shift`, `cs1`, `ecs1`, `abs`, `log`, `exp`, etc.
- All binary numeric operations: `+`, `-`, `*`, `/`, etc.
- All rolling operations: `rolling-mean`, `rolling-std`, etc.
- All schema operations: `xminus`, etc.

**Tripwire**: Test T1 (`t1_masked_rows_are_na_for_all_unary_ops`)

### 2.2 Rolling Window Counts Eligible Observations

**Definition**: `eligible = !masked && valid (not NA)`

**Rule**: Rolling windows count ONLY eligible observations, not calendar positions.

**Example**:
- `rolling-mean-partial 250` with weekend mask
- Window of 250 = last 250 weekday observations (excludes ~71 weekend days)
- First valid output at calendar day ~357, not day 250

**Strict vs Partial**:
- **Strict**: Requires exactly `w` eligible observations
  - `rolling-mean w` ⇒ output NA until we have exactly `w` eligible obs
- **Partial**: Requires `>= min_periods` eligible observations (typically `min_periods = 2`)
  - `rolling-mean-partial w` ⇒ output valid if we have ≥2 eligible obs

**Rationale**: Matches CLISPI's observation-based rolling semantics. CLISPI `w5` removes weekend rows before `wavg(250)`, so window counts 250 weekday observations. BLISP achieves same semantics with masks while keeping calendar index intact.

**Tripwire**: Test T2 (`t2_rolling_strict_vs_partial_start_dates`)

### 2.3 Rolling with Source NAs

**Rule**: Rolling windows skip both masked rows AND source NAs when counting observations.

**Example**:
```
Row  Date       Masked  Value   Eligible
0    Mon        false   100.0   yes
1    Tue        false   NA      no (source NA)
2    Wed        false   102.0   yes
3    Thu        false   103.0   yes
4    Fri        false   NA      no (source NA)
5    Sat        true    105.0   no (masked)
6    Sun        true    106.0   no (masked)
```

For `rolling-mean 3` at row 6:
- Scan backward: row 6 (masked) → skip
- Row 5 (masked) → skip
- Row 4 (NA) → skip
- Row 3 (103.0) → eligible #1
- Row 2 (102.0) → eligible #2
- Row 1 (NA) → skip
- Row 0 (100.0) → eligible #3
- Have 3 eligible ⇒ output `(100 + 102 + 103) / 3 = 101.67`

**Tripwire**: Test T3 (`t3_rolling_with_source_nas`)

## 3. Mask Propagation Rules

### 3.1 Unary Operations

**Rule**: Unary operations preserve input tags exactly (Arc clone).

**Implementation**: `map_numeric_preserve_tags` uses `Arc::clone(&frame.tags)`

**Result**:
- `output.masks = input.masks` (same Arc)
- `output.active_mask = input.active_mask` (same Arc)

**Examples**:
- `dlog(frame)` ⇒ output has same masks as input
- `shift(frame, 1)` ⇒ output has same masks as input
- `cs1(frame)` ⇒ output has same masks as input

**Tripwire**: Verified by Arc pointer equality in `map_numeric_preserve_tags` tests

### 3.2 Binary Operations

**Rule**: Binary operations merge mask sets and OR active masks.

**Mask Set Merge**:
- Collect all named masks from both frames
- If same name exists in both frames:
  - Must be identical (Arc pointer equality or bitwise equality)
  - Error if different bitsets have same name (prevents collision)
- Output mask set = union of all named masks

**Active Mask OR**:
- `output.active_mask[i] = lhs.active_mask[i] OR rhs.active_mask[i]`
- Provenance expression: `OR(lhs.expr, rhs.expr)`

**Rationale**: A row is excluded if it's excluded in EITHER operand. Conservative union semantics prevent accidentally including excluded data.

**Examples**:
- `(+ frameA frameB)`:
  - If row 5 is masked in A but not B ⇒ output row 5 is masked
  - If row 7 is masked in both A and B ⇒ output row 7 is masked
  - If row 9 is unmasked in both A and B ⇒ output row 9 is unmasked

**Tripwire**: Test T4 (`t4_binary_ops_or_active_masks`)

### 3.3 Schema Operations

**Rule**: Schema-transforming operations (like `xminus`) inherit masks from input.

**Rationale**: Schema ops change column structure but preserve index. Since masks are index-level metadata, they should be inherited.

**Example**:
- `xminus(frame)` creates cross-sectional spreads
- Preserves index, preserves masks
- If weekend rows were masked in input, they remain masked in output

### 3.4 Join Operations

**Rule**: Join output inherits masks from the RIGHT operand (Y).

**Rationale**:
- `asofr(X, Y)` output has Y's index (RIGHT OUTER JOIN semantics)
- Mask metadata follows index ownership
- Output structure = Y's structure with X's values filled in

**Decision Point** (frozen choice):
- **Current**: `output.masks = Y.masks`, `output.active_mask = Y.active_mask`
- **Alternative** (not chosen): `output.active_mask = Y.active_mask OR reindexed(X.active_mask)`

**Justification for current choice**:
- Simpler semantics: masks follow index
- Consistent with frame ownership model
- If X has additional exclusions, they should be applied BEFORE the join (user's responsibility)

**Tripwire**: Test T5 (`t5_join_inherits_y_masks`)

## 4. Mask Name Collision Policy

### 4.1 Deterministic Error on Collision

**Rule**: When merging mask sets, if the same name exists in both frames with DIFFERENT bitsets, error deterministically.

**Check**:
1. Arc pointer equality (`Arc::ptr_eq`)
2. If not pointer-equal, bitwise equality (`*a == *b`)
3. If different bitsets ⇒ error

**Error Message**: `"Mask '{name}' collision: different bitsets with same name"`

**Rationale**: Prevents silent corruption. If two frames have "weekend" masks that differ, it's likely a bug (different date ranges, misaligned indices). Force user to resolve explicitly.

**Tripwire**: Test T6 (`t6_mask_name_collision_deterministic`)

## 5. Performance Contracts

### 5.1 Rolling Operations Complexity

**Target**: O(n) amortized for strict rolling windows ✅ **ACHIEVED (Phase F)**

**Current Implementation**: O(n) streaming with VecDeque
- Maintains queue of last w eligible observations
- Each observation enters/exits queue exactly once
- Updates sum/sumsq incrementally
- For 1000 rows with w=250: ~1000 operations (vs ~250k for naive)

**Algorithm**:
```rust
for each calendar row i:
    if masked ⇒ output NA
    else if value valid:
        push to queue
        update running_sum, running_sumsq
        if queue.len() > w: pop front, update sums
        if strict and len==w: emit mean/std
        if partial and len>=2: emit mean/std
```

**Numerical Stability**:
- Variance: `var = (sumsq/n) - (mean)²` with `max(0)` clamp
- Population variance: dividing by `w` (matches CLISPI)

**Legacy Comparison**:
- Old O(n·w) implementation kept in `#[cfg(test)]` for verification
- Tripwire test compares outputs (bit-for-bit identical)

**Benchmark**: Test `test_streaming_performance_benefit` in `phase_f_streaming_rolling.rs`

## 6. Implementation Checklist

### 6.1 Every Kernel Must Respect Mask Gate

**Pattern**:
```rust
for i in 0..nrows {
    if active_mask.is_masked(i) {
        output[i] = f64::NAN;
        continue;
    }
    // ... compute ...
}
```

**Applies to**:
- All rolling operation implementations
- All unary numeric kernels
- All binary numeric kernels

**Enforcement**:
- Code review checklist
- Tripwire tests catch regressions
- Property test: randomly compose ops, assert masked rows stay NA

### 6.2 Tags Construction Sites

**Rule**: Every `Tags { ... }` literal MUST initialize `masks` and `active_mask`.

**Source**: `Tags::new()` for fresh frames (empty masks)
**Unary ops**: Clone from input
**Binary ops**: Merge + OR
**Schema ops**: Clone from input
**Join ops**: Clone from RIGHT operand

**Audit Locations** (all verified ✓):
- `src/builtins.rs:942` - mask-weekend (adds new mask) ✓
- `src/builtins.rs:994` - with-mask (activates mask) ✓
- `src/exec.rs:382` - xminus (inherits input masks) ✓
- `src/exec.rs:1302` - binary_frame_frame (merges + ORs) ✓
- `src/frame.rs:271` - reindex_by (TODO: needs target frame for proper inheritance)
- `src/frame.rs:408` - asofr (inherits Y masks) ✓
- `src/frame.rs:497` - asofr_fallback (inherits Y masks) ✓

## 7. API Surface

### 7.1 User-Facing Builtins

**mask-weekend**: `(mask-weekend frame [name])`
- Creates named weekend mask (Saturday + Sunday = true)
- Stores in `frame.tags.masks[name]`
- Does NOT activate (orthogonal to active_mask)

**with-mask**: `(with-mask frame mask-expr)`
- Activates mask expression
- Compiles expression to BitVec
- Sets as `frame.tags.active_mask`
- Mask expressions:
  - Symbol: `'weekend` → Name("weekend")
  - `(not expr)` → NOT
  - `(and expr ...)` → AND
  - `(or expr ...)` → OR

### 7.2 Internal IR Operations

**Rolling operations** (mask-aware):
- `rolling-mean w` (strict)
- `rolling-std w` (strict)
- `rolling-mean-partial w` (relaxed, min_periods=2)
- `rolling-std-partial w` (relaxed, min_periods=2)

All rolling ops count ONLY eligible observations.

## 8. Testing Strategy

### 8.1 Tripwire Tests (Regression Prevention)

Six tripwire tests in `tests/mask_tripwires.rs`:

1. **T1**: Masked rows are NA for all unary ops
2. **T2**: Rolling strict vs partial start dates correct
3. **T3**: Rolling with source NAs behaves correctly
4. **T4**: Binary ops OR active masks, masked rows NA
5. **T5**: Join inherits Y masks (policy explicit)
6. **T6**: Mask name collision deterministic

### 8.2 Property Tests (Future)

Randomly compose operations, assert:
- Masked rows always NA
- Mask propagation rules hold
- No silent mask loss

## 9. Migration Notes

### 9.1 CLISPI Compatibility

**CLISPI**: `locf → w5 → dlog → cs1 → wavg(250)`
- `w5` removes weekend rows (data-shaping)
- `wavg(250)` counts 250 observations in filtered data

**BLISP**: `locf → mask-weekend → with-mask → dlog → cs1 → wavg(250)`
- `mask-weekend` marks weekends (metadata)
- `with-mask` activates mask
- `wavg(250)` counts 250 eligible weekday observations
- Calendar index stays intact (masked rows remain, just excluded)

**Result**: Identical semantics, but BLISP preserves index structure.

### 9.2 Backward Compatibility

**Breaking change**: Operations now respect active_mask.

**Migration path**:
- Old code without masks: `active_mask = all-false` → no change
- New code with masks: explicit `mask-weekend` + `with-mask`

---

**Version**: 1.0 (2025-01-XX)
**Status**: FROZEN - Do not modify without consensus
**Tripwire Tests**: All passing ✓
