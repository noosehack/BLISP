# Orientation Refactor - Implementation Plan

**Date**: 2026-02-28
**Goal**: Migrate BLISP to use blawktrust orientation as single source of truth
**Approach**: Step-by-step minimal diffs with compile checks at each step

---

## Prerequisites

### Verify blawktrust APIs Exist

**Check** that blawktrust exports these symbols:

```bash
cd /home/ubuntu/blawktrust
rg "pub const ORI_" src/table/orientation.rs
rg "pub fn.*with_orientation" src/table/view.rs
rg "pub fn.*compose_orientation" src/table/view.rs
rg "pub fn sum\(" src/builtins/ori_ops.rs
```

**Expected**:
- ✅ `ORI_H`, `ORI_N`, `ORI__N`, `ORI__H`, `ORI_Z`, `ORI_S`, `ORI__Z`, `ORI__S`, `ORI_X`, `ORI_R`
- ✅ `TableView::with_orientation(ori)`
- ✅ `TableView::compose_orientation(ori)`
- ✅ `sum(view: &TableView) -> Column`

**If missing**: Add to blawktrust first (outside scope of this plan).

---

## Step A: Remove BLISP Axis Enum and Field

**Goal**: Delete parallel orientation system from BLISP.

### A.1: Delete Axis Enum

**File**: `/home/ubuntu/blisp/src/value.rs`

**Find** (lines 332-342):
```rust
pub enum Axis {
    Col,  // Operate down time per column (default, kdb-like)
    Row,  // Operate across columns per row (cross-sectional)
}

impl Default for Axis {
    fn default() -> Self {
        Axis::Col  // Column-wise default
    }
}
```

**Delete**: Entire enum and impl block (~11 lines).

---

### A.2: Remove `axis` Field from TableViewWithMetadata

**File**: `/home/ubuntu/blisp/src/value.rs`

**Find** (lines 352-356):
```rust
pub struct TableViewWithMetadata {
    pub view: Arc<blawktrust::TableView>,
    pub axis: Axis,  // ← DELETE THIS LINE
}
```

**Change to**:
```rust
pub struct TableViewWithMetadata {
    pub view: Arc<blawktrust::TableView>,
    // Orientation lives in view.ori (blawktrust)
}
```

---

### A.3: Update `from_view()` Constructor

**File**: `/home/ubuntu/blisp/src/value.rs`

**Find** (lines 358-362):
```rust
pub fn from_view(view: Arc<blawktrust::TableView>) -> Self {
    Self {
        view,
        axis: Axis::default(),  // ← DELETE THIS LINE
    }
}
```

**Change to**:
```rust
pub fn from_view(view: Arc<blawktrust::TableView>) -> Self {
    Self { view }
}
```

---

### A.4: Update `with_meta()` (if exists)

**File**: `/home/ubuntu/blisp/src/value.rs`

**Find** (lines 377-381):
```rust
pub fn with_meta(view: Arc<blawktrust::TableView>, axis: Axis) -> Self {
    Self { view, axis }
}
```

**Delete**: Entire method (no longer needed).

---

### A.5: Update `with_new_metadata()` (if exists)

**File**: `/home/ubuntu/blisp/src/value.rs`

**Find** (lines 383-390):
```rust
pub fn with_new_metadata(&self, axis: Axis) -> Self {
    Self {
        view: Arc::clone(&self.view),
        axis,
    }
}
```

**Delete**: Entire method (no longer needed).

---

### A.6: Update `clone_shallow()`

**File**: `/home/ubuntu/blisp/src/value.rs`

**Find** (lines 407-414):
```rust
pub fn clone_shallow(&self) -> Self {
    Self {
        view: Arc::clone(&self.view),
        axis: self.axis,  // ← DELETE THIS LINE
    }
}
```

**Change to**:
```rust
pub fn clone_shallow(&self) -> Self {
    Self {
        view: Arc::clone(&self.view),
    }
}
```

---

### A.7: Delete `with_axis()` Method

**File**: `/home/ubuntu/blisp/src/value.rs`

**Find** (lines 417-422):
```rust
pub fn with_axis(&self, axis: Axis) -> Self {
    let mut out = self.clone_shallow();
    out.axis = axis;
    out
}
```

**Delete**: Entire method (~6 lines).

---

### A.8: Update Table Struct (if it has axis)

**File**: `/home/ubuntu/blisp/src/value.rs`

**Find** (lines 346-350):
```rust
pub struct Table {
    pub columns: Vec<(SymbolId, blawktrust::Column)>,
    pub row_count: usize,
    pub axis: Axis,  // ← DELETE IF EXISTS
}
```

**Change to**:
```rust
pub struct Table {
    pub columns: Vec<(SymbolId, blawktrust::Column)>,
    pub row_count: usize,
}
```

**Also update**: `Table::new()` to remove `axis: Axis::default()` line.

---

### A.9: Verify Compilation

```bash
cd /home/ubuntu/blisp
cargo build 2>&1 | head -50
```

**Expected**: Compilation errors in builtins.rs (Step B will fix).

---

## Step B: Implement builtin_o Using blawktrust Orientation

**Goal**: Make `(o ...)` set blawktrust `ori` instead of BLISP `axis`.

### B.1: Update builtin_o Implementation

**File**: `/home/ubuntu/blisp/src/builtins.rs`

**Find** (lines 3760-3828):
```rust
fn builtin_o(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    use crate::value::{Axis, TableViewWithMetadata};  // ← WRONG imports

    // ... parsing code ...

    // Symbol form: map layout symbols to axis semantics
    let axis = match mode_name {
        "H" | "N" | "NSWE" | "SNWE" => Axis::Col,
        "Z" | "S" | "WENS" | "EWNS" => Axis::Row,
        _ => return Err(format!("o: unknown orientation symbol '{}'", mode_name)),
    };

    // Get table and set axis
    match table_arg {
        Value::TableView(tv) => {
            let new_tv = tv.with_axis(axis);  // ← WRONG
            Ok(Value::TableView(Arc::new(new_tv)))
        }
        _ => { ... }
    }
}
```

**Replace with**:
```rust
fn builtin_o(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    use crate::value::TableViewWithMetadata;

    if args.len() != 2 {
        return Err(format!("o expects 2 arguments, got {}", args.len()));
    }

    let mode_arg = &args[0];
    let table_arg = &args[1];

    // Parse orientation symbol
    let mode_name = match mode_arg {
        Value::Sym(s) => rt.interner.resolve(*s),
        Value::Str(s) => s.as_ref(),
        _ => return Err(format!("o expects symbol or string, got {}", mode_arg.type_name())),
    };

    // Map symbol to blawktrust Ori constant
    let ori = match mode_name {
        // Column-major (ColwiseLike)
        "H" | "NSWE" => blawktrust::ORI_H,
        "N" | "SNWE" => blawktrust::ORI_N,
        "_N" | "NSEW" => blawktrust::ORI__N,
        "_H" | "SNEW" => blawktrust::ORI__H,

        // Row-major (RowwiseLike)
        "Z" | "WENS" => blawktrust::ORI_Z,
        "S" | "EWNS" => blawktrust::ORI_S,
        "_Z" | "EWSN" => blawktrust::ORI__Z,
        "_S" | "WESN" => blawktrust::ORI__S,

        // Special modes
        "X" => blawktrust::ORI_X,
        "R" => blawktrust::ORI_R,

        _ => return Err(format!(
            "o: unknown orientation '{}'. Valid: H, N, _N, _H, Z, S, _Z, _S, X, R",
            mode_name
        )),
    };

    // Get table and set orientation
    match table_arg {
        Value::TableView(tv) => {
            // Call blawktrust API to create new view with orientation
            let new_view = tv.view.with_orientation(ori);
            Ok(Value::TableView(Arc::new(TableViewWithMetadata {
                view: Arc::new(new_view),
            })))
        }
        _ => {
            // Try to convert to TableView first
            let view = ensure_tableview(table_arg, rt)?;
            let new_view = view.with_orientation(ori);
            Ok(Value::TableView(Arc::new(TableViewWithMetadata {
                view: Arc::new(new_view),
            })))
        }
    }
}
```

**Key changes**:
- Remove `Axis` import
- Map all 10 orientation symbols to blawktrust constants
- Call `view.with_orientation(ori)` instead of `with_axis()`
- Support underscore variants (`_N`, `_H`, `_Z`, `_S`)
- Support X and R modes

---

### B.2: Verify Compilation

```bash
cd /home/ubuntu/blisp
cargo build 2>&1 | head -50
```

**Expected**: Compilation errors in aggregation builtins (Step D will fix).

---

## Step C: Add builtin_ro for D4 Composition

**Goal**: Implement `(ro ...)` for relative orientation.

### C.1: Implement builtin_ro

**File**: `/home/ubuntu/blisp/src/builtins.rs`

**Location**: After `builtin_o` (around line 3829)

**Add**:
```rust
/// (ro ORI table) - Relative orientation (D4 composition)
///
/// Composes current orientation with given orientation using D4 group rules.
/// Only works with D4 orientations (H, N, _N, _H, Z, S, _Z, _S).
/// Returns error for X and R modes (not composable).
///
/// Examples:
///   (ro 'Z table)   ; Transpose from current state
///   (ro 'Z (ro 'Z table))  ; Z∘Z = H (identity)
fn builtin_ro(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    use crate::value::TableViewWithMetadata;

    if args.len() != 2 {
        return Err(format!("ro expects 2 arguments, got {}", args.len()));
    }

    let mode_arg = &args[0];
    let table_arg = &args[1];

    // Parse orientation symbol
    let mode_name = match mode_arg {
        Value::Sym(s) => rt.interner.resolve(*s),
        Value::Str(s) => s.as_ref(),
        _ => return Err(format!("ro expects symbol or string, got {}", mode_arg.type_name())),
    };

    // Map symbol to blawktrust Ori constant (D4 only)
    let ori = match mode_name {
        // Column-major (D4)
        "H" | "NSWE" => blawktrust::ORI_H,
        "N" | "SNWE" => blawktrust::ORI_N,
        "_N" | "NSEW" => blawktrust::ORI__N,
        "_H" | "SNEW" => blawktrust::ORI__H,

        // Row-major (D4)
        "Z" | "WENS" => blawktrust::ORI_Z,
        "S" | "EWNS" => blawktrust::ORI_S,
        "_Z" | "EWSN" => blawktrust::ORI__Z,
        "_S" | "WESN" => blawktrust::ORI__S,

        // Reject non-D4 modes
        "X" | "R" => return Err(format!(
            "ro only works with D4 orientations (H,N,_N,_H,Z,S,_Z,_S), not '{}'",
            mode_name
        )),

        _ => return Err(format!(
            "ro: unknown orientation '{}'. Valid: H, N, _N, _H, Z, S, _Z, _S",
            mode_name
        )),
    };

    // Get table and compose orientation
    match table_arg {
        Value::TableView(tv) => {
            // Call blawktrust D4 composition API
            let new_view = tv.view.compose_orientation(ori)
                .ok_or_else(|| format!(
                    "Cannot compose orientation (current={:?}, transform={:?})",
                    tv.view.ori, ori
                ))?;

            Ok(Value::TableView(Arc::new(TableViewWithMetadata {
                view: Arc::new(new_view),
            })))
        }
        _ => {
            // Try to convert to TableView first
            let view = ensure_tableview(table_arg, rt)?;
            let new_view = view.compose_orientation(ori)
                .ok_or_else(|| "Cannot compose orientation".to_string())?;

            Ok(Value::TableView(Arc::new(TableViewWithMetadata {
                view: Arc::new(new_view),
            })))
        }
    }
}
```

---

### C.2: Register builtin_ro

**File**: `/home/ubuntu/blisp/src/builtins.rs`

**Find** (around line 221):
```rust
// Orientation
rt.register_builtin("o", builtin_o);
```

**Add after**:
```rust
// Orientation
rt.register_builtin("o", builtin_o);
rt.register_builtin("ro", builtin_ro);  // ← ADD THIS
```

---

### C.3: Verify Compilation

```bash
cd /home/ubuntu/blisp
cargo build 2>&1 | head -50
```

**Expected**: Still have errors in aggregation builtins (Step D will fix).

---

## Step D: Update Builtins to Use blawktrust Dispatch

**Goal**: Remove BLISP reimplementations, delegate to blawktrust.

### D.1: Update builtin_sum

**File**: `/home/ubuntu/blisp/src/builtins.rs`

**Find** (lines 2663-2733):
```rust
fn builtin_sum(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    // ... massive implementation checking tv.axis ...
}
```

**Replace with**:
```rust
fn builtin_sum(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("sum expects 1 argument, got {}", args.len()));
    }

    match &args[0] {
        Value::Col(c) => {
            // Column sum (scalar result)
            let mut sum = 0.0;
            if let blawktrust::Column::F64(data) = c.as_ref() {
                for &val in data {
                    if !val.is_nan() {
                        sum += val;
                    }
                }
            }
            Ok(Value::Float(sum))
        }
        Value::TableView(tv) => {
            // Delegate to blawktrust (checks view.ori)
            let result_col = blawktrust::builtins::ori_ops::sum(&tv.view);
            Ok(Value::Col(Arc::new(result_col)))
        }
        _ => Err(format!("sum expects column or tableview, got {}", args[0].type_name())),
    }
}
```

**Reduction**: ~60 lines → ~20 lines.

---

### D.2: Update builtin_mean

**File**: `/home/ubuntu/blisp/src/builtins.rs`

**Find** (lines 2747-2843):
```rust
fn builtin_mean(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    // ... massive implementation checking tv.axis ...
}
```

**Replace with**:
```rust
fn builtin_mean(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("mean expects 1 argument, got {}", args.len()));
    }

    match &args[0] {
        Value::Col(c) => {
            // Column mean (scalar result)
            let mut sum = 0.0;
            let mut count = 0;
            if let blawktrust::Column::F64(data) = c.as_ref() {
                for &val in data {
                    if !val.is_nan() {
                        sum += val;
                        count += 1;
                    }
                }
            }
            let result = if count > 0 {
                sum / count as f64
            } else {
                f64::NAN
            };
            Ok(Value::Float(result))
        }
        Value::TableView(tv) => {
            // Delegate to blawktrust (if API exists)
            // OPTION 1: If blawktrust has mean()
            // let result_col = blawktrust::builtins::ori_ops::mean(&tv.view);
            // Ok(Value::Col(Arc::new(result_col)))

            // OPTION 2: Implement using sum/count (fallback)
            let sum_col = blawktrust::builtins::ori_ops::sum(&tv.view);
            // Compute count per aggregate dimension
            // ... (implementation depends on blawktrust API)

            // For now: return error until blawktrust mean() is added
            Err("mean on TableView: delegate to blawktrust (TODO)".to_string())
        }
        _ => Err(format!("mean expects column or tableview, got {}", args[0].type_name())),
    }
}
```

**Note**: Check if `blawktrust::mean()` exists. If not, either:
- Add to blawktrust first
- Implement via `sum/count` using blawktrust primitives
- Keep BLISP implementation but check `view.ori_class()` instead of `axis`

---

### D.3: Update builtin_std

**File**: `/home/ubuntu/blisp/src/builtins.rs`

**Find** (lines 2867-2975):
```rust
fn builtin_std(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    // ... massive implementation checking tv.axis ...
}
```

**Replace with** (similar to mean):
```rust
fn builtin_std(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("std expects 1 argument, got {}", args.len()));
    }

    match &args[0] {
        Value::Col(c) => {
            // Column std (scalar result) - keep existing implementation
            // ... (unchanged)
        }
        Value::TableView(tv) => {
            // Check if blawktrust has std() API
            // If not, fallback to checking ori_class()
            match tv.view.ori_class() {
                blawktrust::OriClass::ColwiseLike => {
                    // Std down rows per column (existing logic)
                    // ...
                }
                blawktrust::OriClass::RowwiseLike => {
                    // Std across columns per row (existing logic)
                    // ...
                }
                _ => Err("std not defined for this orientation".to_string()),
            }
        }
        _ => Err(format!("std expects column or tableview, got {}", args[0].type_name())),
    }
}
```

**Key change**: Replace `tv.axis` check (Line 2907) with `tv.view.ori_class()` check.

---

### D.4: Update builtin_cs1_cols

**File**: `/home/ubuntu/blisp/src/builtins.rs`

**Find** (lines 1534-1613):
```rust
fn builtin_cs1_cols(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    // ... implementation checking tv.axis ...
}
```

**Replace** `tv.axis` check (Line 1543) with `tv.view.ori_class()`:

```rust
// OLD:
match tv.axis {

// NEW:
match tv.view.ori_class() {
```

**Also update** branch patterns:
```rust
// OLD:
Axis::Col => { /* ... */ }
Axis::Row => { /* ... */ }

// NEW:
blawktrust::OriClass::ColwiseLike => { /* ... */ }
blawktrust::OriClass::RowwiseLike => { /* ... */ }
_ => return Err("cs1-cols not defined for this orientation".to_string()),
```

---

### D.5: Update builtin_ecs1_cols

**File**: `/home/ubuntu/blisp/src/builtins.rs`

**Find** (lines 1658-1740):
```rust
fn builtin_ecs1_cols(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    // ... implementation checking tv.axis ...
}
```

**Replace** `tv.axis` check (Line 1667) with `tv.view.ori_class()` (same pattern as cs1_cols).

---

### D.6: Verify Compilation

```bash
cd /home/ubuntu/blisp
cargo build 2>&1 | tee build.log
```

**Expected**: Clean build (warnings OK, no errors).

---

## Step E: Adjust IR Executor (Optional)

**Goal**: Make IR operations orientation-aware (if desired).

### E.1: Current State

IR operations work on Frame (not TableView), so they don't have orientation metadata.

**Decision point**:
- **Option 1**: Leave IR orientation-agnostic (transforms only)
- **Option 2**: Propagate orientation through IR context

### E.2: Option 1 (Recommended - No Changes)

**Rationale**:
- IR is for Frame operations (dlog, shift, etc.)
- Orientation is for TableView aggregations
- Clean separation of concerns

**Action**: No changes to IR executor.

---

### E.3: Option 2 (Future Work - Not Implemented Now)

**If needed later**:

1. Add `ori` field to IR Value::Frame
2. Propagate `ori` through IR planning context
3. Update IR executor to check `ori` for operations that care

**Defer**: Not part of this refactor.

---

## Step F: Build and Test

### F.1: Clean Build

```bash
cd /home/ubuntu/blisp
cargo clean
cargo build --release
```

**Expected**: Success with zero errors.

---

### F.2: Run Existing Tests

```bash
cd /home/ubuntu/blisp
cargo test
```

**Expected**: Some tests may fail (they check old behavior). Update tests in Step G.

---

## Summary of Implementation Steps

| Step | Goal | Files Changed | Lines Changed |
|------|------|---------------|---------------|
| **A** | Remove BLISP axis | value.rs | -30 lines (delete) |
| **B** | Implement builtin_o | builtins.rs:3760-3828 | ~40 lines (rewrite) |
| **C** | Add builtin_ro | builtins.rs:~3829 | +80 lines (new) |
| **D.1** | Update builtin_sum | builtins.rs:2663-2733 | -60 → +20 lines |
| **D.2** | Update builtin_mean | builtins.rs:2747-2843 | ~30 lines (simplify) |
| **D.3** | Update builtin_std | builtins.rs:2867-2975 | ~10 lines (check ori_class) |
| **D.4** | Update builtin_cs1_cols | builtins.rs:1534-1613 | ~5 lines (check ori_class) |
| **D.5** | Update builtin_ecs1_cols | builtins.rs:1658-1740 | ~5 lines (check ori_class) |
| **E** | IR (optional) | exec.rs | 0 lines (no changes) |

**Total**: ~200 lines changed (net: -30 deletions + add ro).

---

## Rollback Plan

**If something breaks**:

```bash
cd /home/ubuntu/blisp
git checkout HEAD -- src/value.rs src/builtins.rs
cargo build
```

**Recommendation**: Commit after each step:
```bash
git add src/value.rs && git commit -m "Step A: Remove BLISP axis"
git add src/builtins.rs && git commit -m "Step B: Implement builtin_o"
# etc.
```

---

**End of Implementation Plan**
