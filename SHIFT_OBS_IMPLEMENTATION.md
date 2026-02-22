# Mask-Aware Shift Implementation (shift-obs)

## Problem Statement

When a weekend mask is active, BLISP's `shift` operator was computing **calendar lag**, 
landing on masked (weekend) rows and returning NA. This caused mismatches with CLISPI, 
which after filtering weekends operates on the **observation stream** (business days only).

### Example: The Tuesday Problem
```
Calendar: Fri(100), Sat(masked), Sun(masked), Mon(103), Tue(104)

shift(2) on Tuesday:
  - Lands 2 calendar days back → Sunday (masked) → NA

Expected (CLISPI behavior):
  - Lands 2 business days back → Friday → 100
```

## Solution: Two Shift Operators

### 1. `shift` - Calendar Lag (Unchanged)
- Positional shift over all rows
- `output[i] = input[i-k]` for all rows
- If source row is masked → NA
- **Backward compatible** - existing scripts unchanged

### 2. `shift-obs` - Observation Lag (New)
- Skips masked rows when computing lag
- Looks back k **eligible (unmasked)** observations
- For each unmasked row i: `shift_obs(k)[i]` = value at k-th unmasked row before i
- Masked rows output NA

### Semantics Decision: Skip Masked Rows Only

We chose **Option A** from your blueprint:
- Eligible row: `!active_mask[row]`
- Does NOT depend on value NA (unlike rolling eligibility)
- Matches "business-day lag" semantics
- If target row unmasked but source value NA → result NA (as usual)

## Implementation

### 1. IR Layer (`src/ir.rs`)
```rust
pub enum NumericFunc {
    Shift { k: usize },      // Calendar lag (existing)
    ShiftObs { k: usize },   // Observation lag (new)
    // ...
}
```

### 2. Execution Layer (`src/exec.rs`)

#### Helper: Eligible Rows Precomputation
```rust
fn eligible_rows(mask: &ActiveMask, nrows: usize) -> (Vec<usize>, Vec<i32>) {
    // Returns:
    // - eligible: Vec of unmasked row indices
    // - pos_in_eligible: Map from row to position in eligible stream
}
```

#### Column Kernel: O(n) Shift Implementation
```rust
fn shift_obs_column(col: &Column, k: usize, mask: &ActiveMask, nrows: usize) -> Column {
    // 1. Precompute eligible rows and position map
    // 2. For each row:
    //    - If masked → NA
    //    - Else: find k-th eligible row before this one
    //    - Copy value (or NA if not enough eligible rows)
}
```

#### Frame Wrapper
```rust
fn apply_shift_obs_mask_aware(frame: &Frame, k: usize) -> Result<Frame, String> {
    // Apply shift_obs_column to each column
    // Preserve tags (I1-I3 contracts)
}
```

### 3. Planner Layer (`src/planner.rs`)
```rust
"shift-obs" | "shiftm" => {
    let k = parse_positive_int(&elements[1])?;
    plan_unary(NumericFunc::ShiftObs { k }, &elements[2..], plan, ctx, interner)
}
```

### 4. Fusion Layer (`src/ir_fusion.rs`)
```rust
fn is_fusible_unary(func: NumericFunc) -> bool {
    match func {
        NumericFunc::ShiftObs { .. } => false,  // NOT fusible (stateful, requires mask)
        // ...
    }
}
```

## Complexity

- **Precomputation**: O(n) to build eligible rows and position map
- **Per-column shift**: O(n) lookups using precomputed maps
- **Total**: O(n) per column (optimal)

## Usage Examples

### Basic Usage (No Mask)
```lisp
;; Without mask, both behave identically
(shift 2 data)      ;; → [NA, NA, 100, 101, 102, ...]
(shift-obs 2 data)  ;; → [NA, NA, 100, 101, 102, ...]
```

### With Weekend Mask
```lisp
;; Create and activate weekend mask
(def masked-data
  (-> (read-csv "prices.csv")
      (mask-weekend)
      (with-mask weekend)))

;; Calendar lag: may land on weekends → NA
(shift 2 masked-data)
;; Tue(01-09): lands on Sun(01-07, masked) → NA

;; Observation lag: skips weekends
(shift-obs 2 masked-data)
;; Tue(01-09): lands 2 business days back → Fri(01-05) → preserves value
```

## Testing

Created `demo_shift_obs.sh` demonstrating:
1. ✅ shift-obs without mask (behaves like shift)
2. ✅ shift-obs with mask (skips masked rows)
3. ✅ Comparison with calendar shift showing the difference

## Relation to CLISPI Parity

This fixes the **last calendar-vs-observation semantic mismatch**:

- ✅ Rolling operations already mask-aware (count eligible observations)
- ✅ shift-obs now mask-aware (skip masked rows)
- ✅ Any pipeline using lagged comparisons or diffs will stop getting NA
     just because the lag crosses a weekend

Your note: "z-score calc is perfect" remains true — rolling eligibility 
already correct. This completes the shift semantics to match.

## Contracts Satisfied

- **I1** (index preserved): Arc::ptr_eq maintained
- **I2** (colnames preserved): Arc::ptr_eq maintained  
- **I3** (nrows preserved): nrows unchanged
- **NA policy**: Masked rows → NA, unmasked rows → source value or NA
- **Shape preserved**: Same shape as input
- **Composable**: Works with all mask expressions (not, and, or)

## Future Extensions

If needed, could add:
- `diff_obs(k)`: observation-based difference
- `ret_obs(k)`: observation-based return
- General `lag_obs(k)` alias

But current `shift-obs` + existing operations should cover most use cases.

## Commit

```
Add mask-aware shift_obs for observation-based lag

Implements shift-obs (aka shiftm) to support business-day lag when masks
are active. This completes the shift semantics fix identified in CLISPI
parity testing.
```

## References

- Blueprint: User's detailed specification of semantics and implementation
- Existing: `eligible_rows()` helper already implemented
- Contracts: MASK_CONTRACTS.md for mask propagation rules
- Testing: demo_shift_obs.sh for verification

