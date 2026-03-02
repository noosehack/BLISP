# Orientation Target Architecture - Design Specification

**Date**: 2026-02-28
**Status**: Design (No Code Yet)
**Goal**: Make blawktrust's orientation system the single source of truth

---

## Design Principle

**Single Source of Truth**: blawktrust's `TableView.ori` is the ONLY orientation field.

**BLISP Role**: Thin language wrapper that exposes blawktrust's orientation system.

**No Parallel Metadata**: BLISP must not maintain separate axis/layout fields.

---

## Part 1: Target Data Structures

### blawktrust (No Changes)

**`TableView`** remains unchanged:
```rust
// /home/ubuntu/blawktrust/src/table/view.rs:7
pub struct TableView {
    pub table: Arc<Table>,
    pub ori: Ori,  // ← SINGLE SOURCE OF TRUTH
}
```

**Status**: ✅ Already correct, no changes needed.

---

### BLISP (Simplified)

**Current** (WRONG):
```rust
// /home/ubuntu/blisp/src/value.rs:352
pub struct TableViewWithMetadata {
    pub view: Arc<blawktrust::TableView>,
    pub axis: Axis,  // ❌ DELETE THIS
}
```

**Target** (CORRECT):
```rust
// /home/ubuntu/blisp/src/value.rs:352
pub struct TableViewWithMetadata {
    pub view: Arc<blawktrust::TableView>,
    // No orientation metadata here!
    // Use view.ori as single source of truth
}
```

**Rationale**:
- Orientation lives in `view.ori` (blawktrust)
- BLISP just carries the Arc
- No parallel metadata to keep in sync

**Optional extensions** (non-orientation metadata):
```rust
pub struct TableViewWithMetadata {
    pub view: Arc<blawktrust::TableView>,
    // Future: BLISP-specific metadata (not orientation)
    // pub tags: Option<Tags>,      // For tagged columns
    // pub mask: Option<Bitmap>,    // For filtered views
}
```

**Rule**: If metadata affects **computation semantics**, it belongs in blawktrust, not BLISP.

---

## Part 2: Orientation Operations

### (o ORI table) - Absolute Orientation

**Syntax**:
```lisp
(o 'H table)    ; Set to H orientation
(o 'Z table)    ; Set to Z orientation
(o '_N table)   ; Set to _N orientation
(o 'X table)    ; Set to X (elementwise) mode
(o 'R table)    ; Set to R (scalar) mode
```

**Supported symbols** (10 total):
- **D4 orientations** (8): `H`, `N`, `_N`, `_H`, `Z`, `S`, `_Z`, `_S`
- **Special modes** (2): `X`, `R`

**Implementation**:
```rust
// /home/ubuntu/blisp/src/builtins.rs:builtin_o
fn builtin_o(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    let table_val = &args[1];
    let ori_name = parse_symbol(&args[0], rt)?;

    // Map symbol to blawktrust Ori
    let ori = match ori_name {
        "H" => blawktrust::ORI_H,
        "N" => blawktrust::ORI_N,
        "_N" => blawktrust::ORI__N,
        "_H" => blawktrust::ORI__H,
        "Z" => blawktrust::ORI_Z,
        "S" => blawktrust::ORI_S,
        "_Z" => blawktrust::ORI__Z,
        "_S" => blawktrust::ORI__S,
        "X" => blawktrust::ORI_X,
        "R" => blawktrust::ORI_R,
        _ => return Err(format!("Unknown orientation: {}", ori_name)),
    };

    // Get TableView and set orientation
    match table_val {
        Value::TableView(tv) => {
            let new_view = tv.view.with_orientation(ori);  // ← blawktrust API
            Ok(Value::TableView(Arc::new(
                TableViewWithMetadata {
                    view: Arc::new(new_view),
                }
            )))
        }
        _ => Err("o expects TableView".to_string()),
    }
}
```

**Key change**: Calls `view.with_orientation(ori)` (blawktrust) instead of `with_axis()` (BLISP).

---

### (ro ORI table) - Relative Orientation (D4 Composition)

**Syntax**:
```lisp
(ro 'Z table)   ; Compose with Z (transpose)
(ro '_H table)  ; Compose with _H (flip both)

; Example: transpose twice = identity
(-> table
    (ro 'Z)     ; Apply Z
    (ro 'Z))    ; Apply Z again → back to H!
```

**Supported symbols** (8 D4 only):
- `H`, `N`, `_N`, `_H`, `Z`, `S`, `_Z`, `_S`
- **NOT** `X`, `R` (non-composable modes)

**Implementation**:
```rust
// /home/ubuntu/blisp/src/builtins.rs:builtin_ro (NEW)
fn builtin_ro(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    let table_val = &args[1];
    let ori_name = parse_symbol(&args[0], rt)?;

    // Map symbol to blawktrust Ori
    let ori = match ori_name {
        "H" => blawktrust::ORI_H,
        "N" => blawktrust::ORI_N,
        "_N" => blawktrust::ORI__N,
        "_H" => blawktrust::ORI__H,
        "Z" => blawktrust::ORI_Z,
        "S" => blawktrust::ORI_S,
        "_Z" => blawktrust::ORI__Z,
        "_S" => blawktrust::ORI__S,
        "X" | "R" => return Err("ro only supports D4 orientations (not X/R)".to_string()),
        _ => return Err(format!("Unknown orientation: {}", ori_name)),
    };

    // Get TableView and compose orientation
    match table_val {
        Value::TableView(tv) => {
            let new_view = tv.view.compose_orientation(ori)  // ← blawktrust D4 composition
                .ok_or("Cannot compose with non-D4 orientation")?;
            Ok(Value::TableView(Arc::new(
                TableViewWithMetadata {
                    view: Arc::new(new_view),
                }
            )))
        }
        _ => Err("ro expects TableView".to_string()),
    }
}
```

**Key**: Uses `view.compose_orientation(ori)` from blawktrust (D4 group composition).

---

## Part 3: Operation Dispatch

### Principle: Delegate to blawktrust

**All operations must delegate to blawktrust**, which dispatches based on `view.ori`.

### sum, mean, std (Aggregations)

**Current** (WRONG - BLISP reimplements):
```rust
// src/builtins.rs:2681
match tv.axis {  // ❌ Checks BLISP axis
    Axis::Col => { /* manual loop */ }
    Axis::Row => { /* manual loop */ }
}
```

**Target** (CORRECT - delegate to blawktrust):
```rust
// src/builtins.rs:builtin_sum
fn builtin_sum(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    match &args[0] {
        Value::TableView(tv) => {
            // Delegate to blawktrust (checks view.ori)
            let result_col = blawktrust::builtins::ori_ops::sum(&tv.view);
            Ok(Value::Col(Arc::new(result_col)))
        }
        Value::Col(c) => {
            // Scalar sum (unchanged)
            let sum = blawktrust::builtins::ops::sum(c);
            Ok(Value::Float(sum))
        }
        _ => Err("sum expects column or tableview".to_string()),
    }
}
```

**Key**: One-liner delegation, no manual loops.

**Applies to**:
- `builtin_sum` → `blawktrust::sum(&view)`
- `builtin_mean` → `blawktrust::mean(&view)` (if exists, else `sum/count`)
- `builtin_std` → `blawktrust::std(&view)` (if exists)

**Benefit**: Automatic dispatch by `ori_class()` (ColwiseLike, RowwiseLike, Real).

---

### cs1-cols, ecs1-cols (Cumulative)

**Current** (WRONG):
```rust
// src/builtins.rs:1543
match tv.axis {  // ❌ Checks BLISP axis
    Axis::Col => { /* cumsum down rows */ }
    Axis::Row => { /* cumsum across cols */ }
}
```

**Target** (CORRECT):
```rust
fn builtin_cs1_cols(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    match &args[0] {
        Value::TableView(tv) => {
            // Delegate to blawktrust (if API exists)
            let result = blawktrust::builtins::ori_ops::cs1(&tv.view);
            Ok(Value::tableview(result))
        }
        _ => Err("cs1-cols expects tableview".to_string()),
    }
}
```

**Note**: If blawktrust doesn't have `cs1()` yet, keep BLISP implementation but **check `view.ori_class()` instead of `axis`**:

```rust
// Fallback if blawktrust API missing
match tv.view.ori_class() {  // ✅ Check blawktrust ori
    OriClass::ColwiseLike => { /* cumsum down rows */ }
    OriClass::RowwiseLike => { /* cumsum across cols */ }
    _ => Err("cs1-cols not defined for this orientation"),
}
```

---

### dlog, shift, locf (Transforms)

**Current** (thin wrappers, but on Column not TableView):
```rust
// src/builtins.rs:2265
let result = blawktrust::builtins::ops::dlog_column(col, lag);
```

**Target** (use TableView-aware API if available):
```rust
fn builtin_dlog(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    match &args[0] {
        Value::TableView(tv) => {
            let lag = args[1].as_int()? as usize;
            // Use TableView-aware API (checks ori)
            let result = blawktrust::builtins::ori_ops::dlog(&tv.view, lag);
            Ok(Value::tableview(result))
        }
        Value::Col(col) => {
            let lag = args[1].as_int()? as usize;
            // Column version (orientation-agnostic)
            let result = blawktrust::builtins::ops::dlog_column(col, lag);
            Ok(Value::Col(Arc::new(result)))
        }
        _ => Err("dlog expects tableview or column".to_string()),
    }
}
```

**Note**: Check if `blawktrust::dlog(view, lag)` exists. If not, keep current Column-based implementation.

---

### Rolling Operations (wkd, wstd, etc.)

**Current**: Operate on columns, orientation-agnostic.

**Target**: Keep as-is (no TableView dispatch needed for column operations).

**Future**: If rowwise rolling is needed, add TableView-aware APIs to blawktrust.

---

## Part 4: Printing and Display

### Current Behavior (Already Correct)

**Print** shows `view.ori`:
```rust
// src/value.rs:596
Value::TableView(tv) => {
    let ori_name = match tv.ori {  // ← Uses blawktrust ori (via Deref)
        blawktrust::ORI_H => "H",
        blawktrust::ORI_Z => "Z",
        // ...
    };
    format!("TableView[ori={}, shape=...]", ori_name)
}
```

**Status**: ✅ Already uses `view.ori`, no changes needed.

**After migration**: Will show correct orientation immediately after `(o ...)`.

---

### CSV Output (save/PRT)

**Target**: CSV output order should reflect `view.ori`.

**Implementation**:
- If `ori` is ColwiseLike (H, N, _N, _H): Write columns in physical order (or reversed if flip_j)
- If `ori` is RowwiseLike (Z, S, _Z, _S): Write rows in transposed order
- Apply `view.map_ij()` to iterate in logical order

**Example**:
```rust
// Pseudo-code for CSV writing
for logical_i in 0..view.logical_nrows() {
    for logical_j in 0..view.logical_ncols() {
        let (phys_i, phys_j) = view.ori.map_ij(nr, nc, logical_i, logical_j);
        write_value(view.table.get(phys_i, phys_j));
    }
}
```

**Note**: blawktrust should provide a `logical_iter()` helper for this.

---

## Part 5: Symbol Support Summary

### Absolute Orientation (o)

| Symbol | blawktrust Constant | Meaning | Supported |
|--------|-------------------|---------|-----------|
| `H` | `ORI_H` | Normal (identity) | ✅ |
| `N` | `ORI_N` | Rows reversed (tac) | ✅ |
| `_N` | `ORI__N` | Columns reversed | ✅ |
| `_H` | `ORI__H` | Both reversed | ✅ |
| `Z` | `ORI_Z` | Transpose (row-major) | ✅ |
| `S` | `ORI_S` | Transpose (synonym) | ✅ |
| `_Z` | `ORI__Z` | Transpose + rows reversed | ✅ |
| `_S` | `ORI__S` | Transpose + cols reversed | ✅ |
| `X` | `ORI_X` | Elementwise mode | ✅ |
| `R` | `ORI_R` | Scalar reduction mode | ✅ |

**Total**: 10 symbols supported by `(o ...)`.

---

### Relative Orientation (ro)

| Symbol | blawktrust Constant | Composable | Supported |
|--------|-------------------|------------|-----------|
| `H` | `ORI_H` | ✅ Yes (identity) | ✅ |
| `N` | `ORI_N` | ✅ Yes | ✅ |
| `_N` | `ORI__N` | ✅ Yes | ✅ |
| `_H` | `ORI__H` | ✅ Yes | ✅ |
| `Z` | `ORI_Z` | ✅ Yes | ✅ |
| `S` | `ORI_S` | ✅ Yes | ✅ |
| `_Z` | `ORI__Z` | ✅ Yes | ✅ |
| `_S` | `ORI__S` | ✅ Yes | ✅ |
| `X` | `ORI_X` | ❌ No (mode) | ❌ Error |
| `R` | `ORI_R` | ❌ No (mode) | ❌ Error |

**Total**: 8 D4 symbols supported by `(ro ...)`.

**Rejection**: `(ro 'X ...)` and `(ro 'R ...)` return error (not D4).

---

## Part 6: Impact on Operations

### Aggregations (sum, mean, std)

**Before** (BLISP axis):
```lisp
(defparameter df (stdin))  ; axis=Col
(sum df)                   ; → 1×N (sum down columns)

(defparameter df-z (o 'Z df))  ; axis=Row
(sum df-z)                 ; → M×1 (sum across rows)
```

**After** (blawktrust ori):
```lisp
(defparameter df (stdin))  ; ori=ORI_H
(sum df)                   ; → 1×N (ColwiseLike)

(defparameter df-z (o 'Z df))  ; ori=ORI_Z
(sum df-z)                 ; → M×1 (RowwiseLike)
```

**Same behavior**, but now `ori` is consistent with print output.

---

### Time-Reversal (NEW - Enabled)

**Before** (not supported):
```lisp
(o 'N df)  ; axis=Col (same as H, no reversal)
```

**After** (enabled):
```lisp
(o 'N df)  ; ori=ORI_N (rows reversed)
(sum df-n) ; Sums down reversed rows (tac semantics)
```

**Use case**: Last-observation-carried-backward (LOCB).

---

### Column-Reversal (NEW - Enabled)

**Before** (not supported):
```lisp
(o '_N df)  ; Error: unknown symbol
```

**After** (enabled):
```lisp
(o '_N df)  ; ori=ORI__N (columns reversed)
; Logical view: columns appear in reverse order
```

**Use case**: Right-to-left ticker order.

---

### Scalar Reduction (NEW - Enabled)

**Before** (not supported):
```lisp
(o 'R df)  ; Error: unknown symbol
```

**After** (enabled):
```lisp
(o 'R df)  ; ori=ORI_R (scalar mode)
(sum df-r) ; → Single scalar (sum all elements)
```

**Use case**: Grand total, portfolio value.

---

### Elementwise Broadcast (NEW - Enabled)

**Before** (not supported):
```lisp
(o 'X df)  ; Error: unknown symbol
```

**After** (enabled):
```lisp
(o 'X df)  ; ori=ORI_X (elementwise mode)
(+ df-x 5) ; → Broadcast scalar to all elements
```

**Use case**: Scalar arithmetic on tables.

---

### D4 Composition (NEW - Enabled)

**Before** (not supported):
```lisp
; No composition operator
```

**After** (enabled):
```lisp
; Transpose twice = identity
(-> df
    (ro 'Z)  ; Transpose
    (ro 'Z)) ; Transpose again → back to H

; Flip horizontally from current state
(-> df
    (o 'Z)   ; Absolute: set to Z
    (ro '_H)) ; Relative: flip both axes
```

**Use case**: Composable transformations, macro-friendly.

---

## Part 7: Backward Compatibility

### Breaking Changes

**None** for typical usage:
- `(o 'H ...)` and `(o 'Z ...)` continue to work (same symbols)
- Aggregations produce same results (same shapes)

### Enhanced Features (Non-Breaking)

- ✅ `(o 'N ...)` now actually reverses (was no-op before)
- ✅ `(o '_N ...)`, `(o '_H ...)`, `(o '_Z ...)`, `(o '_S ...)` now recognized
- ✅ `(o 'X ...)`, `(o 'R ...)` now work
- ✅ `(ro ...)` is new (additive feature)

### Migration Path

**Existing scripts**: No changes required.

**New scripts**: Can opt into D4 features (`N`, `_N`, etc.).

---

## Part 8: Summary of Changes

### Data Structures

| Component | Before | After |
|-----------|--------|-------|
| **blawktrust TableView** | `{ table, ori }` | `{ table, ori }` ✅ No change |
| **BLISP TableViewWithMetadata** | `{ view, axis }` | `{ view }` ✅ Simplified |

### Operations

| Operation | Before | After |
|-----------|--------|-------|
| **`(o ...)`** | Sets BLISP `axis` | Calls `view.with_orientation(ori)` |
| **`(ro ...)`** | N/A | NEW - Calls `view.compose_orientation(ori)` |
| **`sum/mean/std`** | Check BLISP `axis` | Delegate to `blawktrust::sum(&view)` |
| **`cs1-cols/ecs1-cols`** | Check BLISP `axis` | Check `view.ori_class()` or delegate |
| **`dlog/shift/locf`** | Column-based | Use TableView-aware API if available |

### Symbols

| Symbol | Before | After |
|--------|--------|-------|
| `H`, `Z` | Mapped to `Axis::Col/Row` | Mapped to `ORI_H/ORI_Z` |
| `N`, `S` | Mapped to `Axis::Col/Row` (same as H/Z) | Mapped to `ORI_N/ORI_S` (distinct) |
| `_N`, `_H`, `_Z`, `_S` | Not recognized | Mapped to `ORI__N/ORI__H/ORI__Z/ORI__S` |
| `X`, `R` | Not recognized | Mapped to `ORI_X/ORI_R` |

---

## Part 9: Non-Goals (Out of Scope)

### IR System Integration

**Not changing** (for now):
- IR operations (Frame-based) remain orientation-agnostic
- IR planner/executor unchanged
- No TableView → Frame conversion with orientation preservation

**Rationale**: IR is a separate optimization layer. Focus on TableView operations first.

**Future**: If needed, propagate orientation through IR context.

---

### Frame Orientation

**Not adding**:
- Frame type remains BLISP-specific (no blawktrust integration)
- Frame operations don't have orientation metadata

**Rationale**: Frame is IR's columnar structure, separate from TableView.

---

### Keyword Syntax

**Not fixing** (for now):
- `:col`, `:row` keyword parsing (parser limitation)
- Use strings `":col"`, `":row"` as workaround

**Rationale**: Parser changes are orthogonal to orientation refactor.

---

**End of Target Architecture**
