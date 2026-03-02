# D4 Orientation System - Gap Analysis

**Date**: 2026-02-28
**Status**: Documentation of semantic gap between blawktrust (full D4) and BLISP (2-state axis)

---

## Executive Summary

**blawktrust** (the underlying engine) implements a **full D4 dihedral group orientation system** with 10 orientations.

**BLISP** (the language layer) currently implements a **2-state axis selector** that maps to only 2 of those 10 orientations.

This document explains:
1. What the full D4 system is designed to do
2. What BLISP currently implements
3. The semantic gap
4. How to bridge it (if needed)

---

## Part 1: The Full D4 System (blawktrust Engine)

### 10 Orientations (8 D4 + 2 modes)

**Source**: `/home/ubuntu/blawktrust/src/table/orientation.rs`

#### D4 Symmetries (8 orientations)

| Name | Compass | Meaning | swap | flip_i | flip_j | Class |
|------|---------|---------|------|--------|--------|-------|
| **H** | NSWE | Normal (identity) | false | false | false | ColwiseLike |
| **N** | SNWE | Rows reversed (tac) | false | true | false | ColwiseLike |
| **_N** | NSEW | Columns reversed | false | false | true | ColwiseLike |
| **_H** | SNEW | Both reversed | false | true | true | ColwiseLike |
| **Z** | WENS | Transpose (row-major) | true | false | false | RowwiseLike |
| **S** | EWNS | Transpose (synonym) | true | false | false | RowwiseLike |
| **_Z** | EWSN | Transpose + flip rows | true | true | false | RowwiseLike |
| **_S** | WESN | Transpose + flip cols | true | false | true | RowwiseLike |

#### Non-D4 Modes (2 special orientations)

| Name | Meaning | Class |
|------|---------|-------|
| **X** | Elementwise (broadcast scalars) | Each |
| **R** | Reduce to scalar | Real |

### Physical Semantics

**All orientations share the same physical storage** (Vec<Vec<f64>> - columnar).

**Orientation = view transformation**:
- `map_ij(logical_i, logical_j) → (physical_i, physical_j)`
- O(1) - just flag manipulation, no data copying

### Examples (blawktrust Engine)

```rust
use blawktrust::{Table, TableView, ORI_H, ORI_N, ORI_Z};

let table = Table::new(
    vec!["A".to_string(), "B".to_string()],
    vec![
        Column::F64(vec![1.0, 2.0, 3.0]),
        Column::F64(vec![4.0, 5.0, 6.0]),
    ]
);

// Physical layout (always column-major):
// Column A: [1.0, 2.0, 3.0]
// Column B: [4.0, 5.0, 6.0]

// H orientation: Normal
let view_h = TableView::new(table.clone());
// Logical view:
//   A  B
// 0 1  4
// 1 2  5
// 2 3  6

// N orientation: Rows reversed (time-reversal, tac)
let view_n = view_h.with_orientation(ORI_N);
// Logical view:
//   A  B
// 0 3  6  ← row 2 appears first
// 1 2  5  ← row 1 in middle
// 2 1  4  ← row 0 appears last

// Z orientation: Transpose
let view_z = view_h.with_orientation(ORI_Z);
// Logical view:
//   0  1  2
// A 1  2  3
// B 4  5  6
```

### Operation Dispatch (blawktrust)

**Operations consult orientation** to determine aggregation mode:

```rust
pub fn sum(view: &TableView) -> Column {
    match view.ori_class() {
        OriClass::ColwiseLike => sum_colwise(&view.table),    // Sum down columns
        OriClass::RowwiseLike => sum_rowwise_tiled(&view.table), // Sum across rows
        OriClass::Real => sum_all(&view.table),               // Single scalar
        OriClass::Each => panic!("sum not defined for Each"),
    }
}
```

**Key insight**: Operations in blawktrust **do** check orientation and dispatch accordingly.

---

## Part 2: BLISP's Current Implementation (2-State Axis)

### What BLISP Implements

**Source**: `/home/ubuntu/blisp/src/value.rs`, `/home/ubuntu/blisp/src/builtins.rs`

#### Axis Enum (2 states)

```rust
pub enum Axis {
    Col,  // Operate down rows per column
    Row,  // Operate across columns per row
}
```

#### Symbol Mapping (My Recent Fix)

| Symbol(s) | Maps To | Semantic |
|-----------|---------|----------|
| `'H`, `'N`, `'NSWE`, `'SNWE` | `Axis::Col` | Column-wise aggregation |
| `'Z`, `'S`, `'WENS`, `'EWNS` | `Axis::Row` | Row-wise aggregation |

#### What This Loses

**Collapsed semantics**:
- `H` and `N` are treated identically (no time-reversal)
- `Z` and `S` are treated identically (no distinction)
- Underscore variants (`_H`, `_N`, `_Z`, `_S`) are not recognized
- `X` and `R` modes are not supported

### BLISP's Disconnect from blawktrust

**BLISP does NOT use blawktrust's orientation system!**

BLISP has:
- `TableViewWithMetadata { view: Arc<blawktrust::TableView>, axis: Axis }`
- The `axis` field is **BLISP-only metadata**
- The underlying `view.ori` field (blawktrust's orientation) is **ignored**

**Evidence**:

1. **Creation**: `stdin` creates TableView with `ORI_H` always
   ```rust
   // src/io.rs:436
   return Ok(Value::tableview(blawktrust::TableView::new(empty_bt)));
   // → blawktrust::TableView::new() sets ori = ORI_H
   ```

2. **`(o ...)` operator**: Sets BLISP `axis`, not blawktrust `ori`
   ```rust
   // src/builtins.rs:3819
   let new_tv = tv.with_axis(axis);  // Only changes BLISP metadata
   // Does NOT call view.with_orientation() on blawktrust side
   ```

3. **Operations**: Legacy builtins check BLISP `axis`, not blawktrust `ori`
   ```rust
   // src/builtins.rs:2681 (builtin_sum)
   match tv.axis {  // ← Checks BLISP metadata
       Axis::Col => sum_colwise(),
       Axis::Row => sum_rowwise(),
   }
   // Does NOT call blawktrust::sum(view) which checks view.ori
   ```

---

## Part 3: The Semantic Gap

### What Works in blawktrust But Not BLISP

| Feature | blawktrust | BLISP |
|---------|-----------|-------|
| **8 D4 orientations** | ✅ Full D4 group | ❌ Only 2 axis states |
| **Time-reversal (`H` → `N`)** | ✅ Flip rows via `flip_i` | ❌ N=H (same axis) |
| **Column-reversal** | ✅ Flip cols via `flip_j` | ❌ Not supported |
| **Underscore variants** | ✅ _H, _N, _Z, _S | ❌ Not recognized |
| **X mode (elementwise)** | ✅ Broadcast scalars | ❌ Not supported |
| **R mode (reduce all)** | ✅ Scalar reduction | ❌ Not supported |
| **Relative orientation (`ro`)** | ✅ D4 composition | ❌ Not supported |
| **O(1) view transforms** | ✅ Just flag changes | ❌ Not using blawktrust views |

### Example of the Gap

**In blawktrust** (if fully wired):
```rust
let view_h = TableView::new(table);       // H orientation
let view_n = view_h.with_orientation(ORI_N);  // Time-reversed view

// sum() would see different logical order:
// H: [1,2,3] + [4,5,6] → sum each column
// N: [3,2,1] + [6,5,4] → sum each column (reversed order)
```

**In BLISP** (current):
```lisp
(defparameter df (stdin))     ; Creates with axis=Col
(defparameter df-n (o 'N df)) ; Sets axis=Col (same as H!)

; Both behave identically:
(sum df)    ; Sums down columns
(sum df-n)  ; Also sums down columns (no reversal)
```

### Why the Gap Exists

**Design choice**: BLISP was built **independently** from blawktrust's orientation system.

**Layers**:
1. **blawktrust**: Full D4 orientation system (engine)
2. **BLISP**: Minimal axis selector (language)
3. **Gap**: BLISP doesn't call blawktrust's orientation APIs

**Historical**: The `layout` field in BLISP was probably an **abandoned attempt** to bridge this gap.

---

## Part 4: Bridging the Gap (If Needed)

### Option A: Keep Current State (Recommended)

**Accept that BLISP and blawktrust have different orientation semantics.**

**BLISP**: Aggregation axis (2-state: col/row)
**blawktrust**: Full D4 transformation system

**Rationale**:
- Current BLISP system is **simple and works**
- D4 semantics may not be needed for BLISP use cases
- Keeps language layer clean and minimal

**Action**: Document clearly (done in this file)

---

### Option B: Wire BLISP to blawktrust Orientation

**Make BLISP use blawktrust's orientation system.**

#### Step 1: Remove BLISP `axis`, Use blawktrust `ori`

**Current**:
```rust
pub struct TableViewWithMetadata {
    pub view: Arc<blawktrust::TableView>,  // Has .ori field
    pub axis: Axis,                         // BLISP-only (ignored by blawktrust)
}
```

**After**:
```rust
pub struct TableViewWithMetadata {
    pub view: Arc<blawktrust::TableView>,  // Has .ori field
    // Remove axis field entirely
}
```

#### Step 2: Update `(o ...)` to Call blawktrust

**Current**:
```rust
let new_tv = tv.with_axis(axis);  // BLISP metadata
```

**After**:
```rust
let ori = match mode_name {
    "H" => blawktrust::ORI_H,
    "N" => blawktrust::ORI_N,
    "Z" => blawktrust::ORI_Z,
    "_H" => blawktrust::ORI_H_REV,
    // ... all 10 orientations
    _ => return Err(...),
};
let new_view = tv.view.with_orientation(ori);  // blawktrust API
let new_tv = TableViewWithMetadata::from_view(Arc::new(new_view));
```

#### Step 3: Update Operations to Use blawktrust Dispatch

**Current** (BLISP legacy builtins):
```rust
fn builtin_sum(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    match tv.axis {  // Check BLISP metadata
        Axis::Col => sum_colwise(),   // BLISP implementation
        Axis::Row => sum_rowwise(),   // BLISP implementation
    }
}
```

**After** (delegate to blawktrust):
```rust
fn builtin_sum(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    let result_col = blawktrust::builtins::ori_ops::sum(&tv.view);
    // blawktrust sum() checks view.ori and dispatches
    Ok(Value::Col(Arc::new(result_col)))
}
```

#### Step 4: Add Missing Orientations

Support all 10 orientations in `(o ...)`:
- ✅ H, N, Z, S (currently mapped to axis)
- ➕ _H, _N, _Z, _S (add underscore variants)
- ➕ X (elementwise mode)
- ➕ R (scalar mode)

#### Step 5: Add Relative Orientation (`ro`)

Implement `builtin_ro()` using blawktrust's D4 composition:
```rust
fn builtin_ro(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    let view = args[0].as_tableview()?;
    let ori = parse_orientation(args[1])?;

    let new_view = view.view.compose_orientation(ori)
        .ok_or("Cannot compose with X/R")?;

    Ok(Value::TableView(Arc::new(
        TableViewWithMetadata::from_view(Arc::new(new_view))
    )))
}
```

---

### Option C: Hybrid (Most Realistic)

**Keep both systems, document when each applies.**

**For legacy builtins** (`sum`, `mean`, `std`):
- Use BLISP `axis` (2-state selector)
- Simple, works for aggregations

**For blawktrust operations** (if exposed):
- Use blawktrust `ori` (full D4)
- Advanced, for users who need it

**Example**:
```lisp
;; Legacy aggregations use axis
(sum (o 'Z df))    ; Sets axis=Row, sums across columns

;; blawktrust ops use ori (if exposed)
(bt-sum (o 'N df)) ; Would use blawktrust ORI_N with time-reversal
```

**Action**: Add a flag to `TableViewWithMetadata`:
```rust
pub struct TableViewWithMetadata {
    pub view: Arc<blawktrust::TableView>,
    pub axis: Axis,           // For legacy builtins
    pub use_bt_ori: bool,     // If true, ignore axis and use view.ori
}
```

---

## Part 5: Concrete Examples of Lost Semantics

### Time Reversal (H → N)

**What it should do** (blawktrust):
```
Physical data: [1,2,3,4,5]

H orientation: Access [1,2,3,4,5] (normal)
N orientation: Access [5,4,3,2,1] (reversed, tac)

Use case: Last-observation-carried-backward (LOCB)
```

**What BLISP does**:
```lisp
(o 'H df)  ; axis=Col
(o 'N df)  ; axis=Col (same!)
; No reversal happens
```

### Column Reversal (_N, _S)

**What it should do**:
```
Physical columns: [A, B, C]

Normal: Access [A, B, C]
_N:     Access [C, B, A] (columns reversed)

Use case: Right-to-left markets, reverse ticker order
```

**What BLISP does**:
```lisp
(o '_N df)  ; Error: unknown orientation symbol '_N'
; Not recognized at all
```

### Elementwise Mode (X)

**What it should do**:
```
Table: 3×2
Scalar: 5

X mode: Broadcast scalar to all elements
(+ table 5)  → add 5 to every cell

Use case: Scalar arithmetic on tables
```

**What BLISP does**:
```lisp
(o 'X df)  ; Error: unknown orientation symbol 'X'
; Not supported
```

### Scalar Reduce Mode (R)

**What it should do**:
```
Table: M×N

R mode: sum() produces single scalar (sum all elements)

Use case: Grand total, portfolio value
```

**What BLISP does**:
```lisp
(o 'R df)  ; Error: unknown orientation symbol 'R'
; Not supported
; Workaround: (sum (sum df)) if axis gymnastics work
```

---

## Part 6: Recommendations

### For BLISP Phase 0/1 (Current)

**Status**: ✅ **Keep as-is**

**Rationale**:
- 2-state axis works for current use cases
- Simple, predictable, documented
- No user complaints about missing D4

**Action**: Document the gap (this file)

---

### For BLISP Phase 2 (If D4 Needed)

**Trigger**: User request for time-reversal, column-reversal, or X/R modes

**Approach**: Option B (wire to blawktrust)

**Steps**:
1. Remove BLISP `axis` field
2. Update `(o ...)` to call `view.with_orientation(ori)`
3. Delegate operations to `blawktrust::builtins::ori_ops`
4. Add all 10 orientation symbols
5. Add `(ro ...)` for D4 composition
6. Test thoroughly (blawktrust has 85 passing tests)

**Effort**: ~2 days of focused work

**Benefit**: Full D4 symmetry, O(1) view transforms, D4 composition

---

### For Documentation (Now)

**Action**: Add clear notes to BLISP docs:

```markdown
## Orientation System

BLISP currently implements a **2-state aggregation axis** (col/row).

The underlying blawktrust engine supports a **full D4 orientation system**
with 10 orientations (8 D4 symmetries + 2 modes), but BLISP does not
expose this yet.

### Current Semantics

- `(o 'H df)` or `(o 'N df)` → Column-wise aggregation (axis=Col)
- `(o 'Z df)` or `(o 'S df)` → Row-wise aggregation (axis=Row)

**Not supported**:
- Time-reversal (`N` behaves same as `H`)
- Column-reversal (`_N`, `_S` not recognized)
- Elementwise mode (`X`)
- Scalar mode (`R`)
- Relative composition (`ro`)

### Future Enhancement

If D4 semantics are needed, BLISP can wire directly to blawktrust's
orientation system. See `D4_ORIENTATION_GAP_ANALYSIS.md`.
```

---

## Summary Table

| Feature | blawktrust Engine | BLISP Language | Gap |
|---------|------------------|----------------|-----|
| **Orientations** | 10 (8 D4 + X + R) | 2 (Col/Row) | 8 missing |
| **Time-reversal** | ✅ Via `flip_i` | ❌ Not implemented | H=N treated same |
| **Column-reversal** | ✅ Via `flip_j` | ❌ Not implemented | _N, _S not recognized |
| **Transpose** | ✅ Via `swap` | ✅ Via axis=Row | Works (Z sets Row) |
| **Elementwise (X)** | ✅ Broadcast mode | ❌ Not exposed | Missing |
| **Scalar (R)** | ✅ Reduce all mode | ❌ Not exposed | Missing |
| **Composition (`ro`)** | ✅ D4 group table | ❌ Not implemented | Missing |
| **O(1) transforms** | ✅ Just flag change | ⚠️ Metadata only | BLISP doesn't use blawktrust views |
| **Dispatch** | ✅ By `ori_class()` | ⚠️ By BLISP `axis` | Parallel systems |

---

## Conclusion

**The gap is intentional** (for now).

BLISP chose simplicity (2-state axis) over completeness (D4 symmetry).

This is a **valid engineering trade-off** given:
- Current use cases don't need D4
- Implementation is simpler
- Performance is adequate

**The gap can be closed** if needed by wiring BLISP to blawktrust's existing orientation APIs (which already work and have 85 passing tests).

**No urgency** unless users request:
- Time-reversal operations
- Column-reversal operations
- Elementwise/scalar modes
- D4 composition

**For now**: Document and move on.

---

**End of Gap Analysis**
