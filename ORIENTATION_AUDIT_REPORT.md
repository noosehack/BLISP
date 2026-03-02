# BLISP Orientation System Audit - Complete Analysis

**Date**: 2026-02-28
**Repository**: /home/ubuntu/blisp
**Branch**: reconstruct/tableview-only
**Status**: ❌ **CRITICAL BUG FOUND** - Orientation fields are disconnected

---

## Executive Summary

BLISP has **three separate orientation-related fields** that are **disconnected** from each other:

1. **`axis`** (BLISP metadata) - Controls semantic aggregation direction ✅ USED
2. **`layout`** (BLISP metadata) - Never consulted by any operation ❌ DEAD CODE
3. **`ori`** (underlying blawktrust::TableView field) - Only used for display ⚠️ READ-ONLY

**THE BUG**: `(o 'Z table)` sets `layout` field but does NOT update the underlying `ori` field, and `layout` is never consulted by any operation, making the operation a NO-OP.

---

## Part 1: Orientation Field Definitions

### 1.1 TableViewWithMetadata Structure

**Location**: `src/value.rs:405-409`

```rust
pub struct TableViewWithMetadata {
    pub view: Arc<blawktrust::TableView>,  // Underlying view (contains ori field)
    pub layout: Layout,                     // BLISP-only metadata ❌ NEVER USED
    pub axis: Axis,                         // BLISP-only metadata ✅ USED
}
```

### 1.2 Axis Enum (Semantic Orientation)

**Location**: `src/value.rs:383-390`

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

**Status**: ✅ **ACTIVELY USED** by 5 builtin functions

### 1.3 Layout Enum (Physical Orientation - DEAD CODE)

**Location**: `src/value.rs:328-379`

```rust
pub enum Layout {
    /// Column-major family (columns contiguous in memory)
    NSWE,  // Normal (default) - "H"
    SNWE,  // Rows reversed - "N"
    NSEW,  // Columns reversed
    SNEW,  // Both reversed

    /// Row-major family (rows contiguous in memory)
    WENS,  // Normal row-major - "Z"
    EWNS,  // Synonym for WENS - "S"
    EWSN,  // Rows reversed
    WESN,  // Columns reversed
}

impl Layout {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "NSWE" | "H" => Some(Layout::NSWE),
            "SNWE" | "N" => Some(Layout::SNWE),
            "NSEW" => Some(Layout::NSEW),
            "SNEW" => Some(Layout::SNEW),
            "WENS" | "Z" => Some(Layout::WENS),
            "EWNS" | "S" => Some(Layout::EWNS),
            "EWSN" => Some(Layout::EWSN),
            "WESN" => Some(Layout::WESN),
            _ => None,
        }
    }
}
```

**Status**: ❌ **DEAD CODE** - Defined but never consulted by any operation

### 1.4 Underlying ORI Field (Display Only)

**Location**: Inside `blawktrust::TableView` (external crate)

**Constants**: `blawktrust::ORI_H`, `ORI_Z`, `ORI_X`, `ORI_R`

**Status**: ⚠️ **READ-ONLY** - Only accessed for display formatting (value.rs:662-667)

```rust
// src/value.rs:659-669
Value::TableView(tv) => {
    let (nr, nc) = tv.logical_shape();
    let ori_name = match tv.ori {  // ← ONLY USE OF ori FIELD
        blawktrust::ORI_H => "H",
        blawktrust::ORI_Z => "Z",
        blawktrust::ORI_X => "X",
        blawktrust::ORI_R => "R",
        _ => "?",
    };
    format!("TableView[ori={}, shape={}×{}]", ori_name, nr, nc)
}
```

**Search proof**: `rg "\.ori\b" --type rust` returns only `src/value.rs:662`

---

## Part 2: The `(o ...)` Operator Implementation

### 2.1 Builtin Registration

**Location**: `src/builtins.rs:221`

```rust
rt.register_builtin("o", builtin_o);
```

### 2.2 Builtin Implementation

**Location**: `src/builtins.rs:3760-3828`

```rust
fn builtin_o(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    use crate::value::{Axis, Layout, TableViewWithMetadata};

    match args.len() {
        2 => {
            let mode_arg = &args[0];
            let table_arg = &args[1];

            let mode_name = match mode_arg {
                Value::Sym(s) => rt.interner.resolve(*s),
                Value::Str(s) => s.as_ref(),
                _ => return Err(...),
            };

            // BRANCH 1: Keyword form (axis semantics)
            if mode_name.starts_with(':') {
                let keyword = &mode_name[1..];
                let axis = match keyword {
                    "col" => Axis::Col,
                    "row" => Axis::Row,
                    "reset" => Axis::Col,
                    _ => return Err(...),
                };

                match table_arg {
                    Value::TableView(tv) => {
                        let new_tv = tv.with_axis(axis);  // ← Sets axis field
                        Ok(Value::TableView(Arc::new(new_tv)))
                    }
                    _ => { ... }
                }
            } else {
                // BRANCH 2: Symbol form (layout - DEAD CODE PATH)
                let layout = Layout::from_str(mode_name)
                    .ok_or_else(|| format!("o: unknown layout symbol '{}'", mode_name))?;

                match table_arg {
                    Value::TableView(tv) => {
                        let new_tv = tv.with_layout(layout);  // ← Sets layout field (NEVER USED!)
                        Ok(Value::TableView(Arc::new(new_tv)))
                    }
                    _ => { ... }
                }
            }
        }
        _ => Err(...),
    }
}
```

### 2.3 Helper Methods

**Location**: `src/value.rs:478-490`

```rust
/// Clone with different axis (shares underlying view)
pub fn with_axis(&self, axis: Axis) -> Self {
    let mut out = self.clone_shallow();
    out.axis = axis;  // ← ONLY updates BLISP metadata
    out
}

/// Clone with different layout (shares underlying view)
pub fn with_layout(&self, layout: Layout) -> Self {
    let mut out = self.clone_shallow();
    out.layout = layout;  // ← ONLY updates BLISP metadata (DEAD CODE)
    out
}
```

**CRITICAL**: Neither method modifies the underlying `view.ori` field!

### 2.4 Behavior Summary

| Expression | Field Modified | Effect on Behavior |
|------------|---------------|-------------------|
| `(o :col table)` | `axis = Axis::Col` | ✅ Changes aggregation direction |
| `(o :row table)` | `axis = Axis::Row` | ✅ Changes aggregation direction |
| `(o 'H table)` | `layout = NSWE` | ❌ **NO EFFECT** (dead code) |
| `(o 'Z table)` | `layout = WENS` | ❌ **NO EFFECT** (dead code) |
| `(o 'N table)` | `layout = SNWE` | ❌ **NO EFFECT** (dead code) |
| `(o 'S table)` | `layout = EWNS` | ❌ **NO EFFECT** (dead code) |

**Proof**: The underlying `view.ori` remains unchanged (always `ORI_H` from initial creation).

---

## Part 3: Which Functions Consult Which Fields?

### 3.1 Functions That Check `axis` Field

**Search command**: `rg "\.axis" --type rust -n`

| Function | File:Line | Axis::Col Behavior | Axis::Row Behavior |
|----------|-----------|-------------------|-------------------|
| `builtin_cs1_cols` | builtins.rs:1543 | Cumsum down rows per column | Cumsum across columns per row |
| `builtin_ecs1_cols` | builtins.rs:1667 | Exp-cumsum down rows per column | Exp-cumsum across columns per row |
| `builtin_sum` | builtins.rs:2681 | Sum down rows → 1×N output | Sum across columns → M×1 output |
| `builtin_mean` | builtins.rs:2782 | Mean down rows → 1×N output | Mean across columns → M×1 output |
| `builtin_std` | builtins.rs:2907 | Std down rows → 1×N output | Std across columns → M×1 output |

**Total**: 5 functions check `axis`

### 3.2 Functions That Check `layout` Field

**Search command**: `rg "\.layout" --type rust -n`

**Result**: ❌ **ZERO FUNCTIONS**

Only uses of `.layout`:
- `src/value.rs:471` - `clone_shallow()` (copying metadata)
- `src/value.rs:488` - `with_layout()` (setting metadata)

**Conclusion**: The `layout` field is **NEVER CONSULTED** by any computational operation.

### 3.3 Functions That Check `ori` Field

**Search command**: `rg "\.ori\b" --type rust -n`

| Use | File:Line | Purpose |
|-----|-----------|---------|
| Display formatting | value.rs:662 | Read `tv.ori` for debug string: `TableView[ori=H, ...]` |

**Total**: 1 use (read-only, display only)

### 3.4 Functions That Do NOT Check Any Orientation

**Single-column/vector operations** (operate along column, orientation irrelevant):

| Function | File:Line | Notes |
|----------|-----------|-------|
| `builtin_dlog` | builtins.rs:2252 | Column operation (lag logic) |
| `builtin_shift` | builtins.rs:2285 | Column operation (positional shift) |
| `builtin_locf` | builtins.rs:813 | Column operation (LOCF) |
| `builtin_cs1` | builtins.rs:1502 | Single-column cumsum |
| `builtin_ecs1` | builtins.rs:1626 | Single-column exp-cumsum |

These operate on `Value::Col` (single columns), so orientation is not applicable.

**Frame operations** (from IR planner, not builtins):

IR operations in `exec.rs` do not consult BLISP metadata at all - they work directly on Frame structures which have their own orientation system.

---

## Part 4: Concrete Proof of Bug

### 4.1 Minimal Reproducible Example

**Test file**: `/home/ubuntu/blisp/test_orientation_bug.lisp`

```lisp
(defparameter df (stdin))

(print "1. Original (ori=H):")
(print df)

(print "\n2. After (o 'Z df) - layout set to Z but ori UNCHANGED:")
(defparameter df-z (o 'Z df))
(print df-z)

(print "\n3. Both produce identical sum (proof layout is ignored):")
(print "  sum(df):")
(print (sum df))
(print "  sum(df-z):")
(print (sum df-z))
```

**Test data**: `test_ori.csv`
```
A;B;C
10;20;30
11;21;31
12;22;32
```

### 4.2 Execution Output

```bash
$ cat test_ori.csv | ./blisp test_orientation_bug.lisp
✅ Running in HYBRID mode (IR for Frame ops, legacy fallback)
"1. Original (ori=H):"
TableView[ori=H, shape=3×3]

"2. After (o 'Z df) - layout set to Z but ori UNCHANGED:"
TableView[ori=H, shape=3×3]  # ← BUG: ori still H, not Z!

"3. Both produce identical sum (proof layout is ignored):"
"  sum(df):"
TableView[ori=H, shape=1×3]
"  sum(df-z):"
TableView[ori=H, shape=1×3]  # ← Identical output proves layout has no effect
```

### 4.3 What Should Happen (Expected)

After `(o 'Z df)`:
- Display should show: `TableView[ori=Z, shape=3×3]`
- Underlying `view.ori` should be `ORI_Z` (row-major)
- Physical memory layout should change (if layout controls storage)

### 4.4 What Actually Happens (Bug)

After `(o 'Z df)`:
- BLISP sets `metadata.layout = Layout::WENS`
- Underlying `view.ori` remains `ORI_H` (unchanged)
- Display shows `ori=H` (from underlying field)
- No behavioral change (layout is never consulted)

---

## Part 5: Root Cause Analysis

### 5.1 Why Is `axis` Used But `layout` Ignored?

**Answer**: Design inconsistency during development.

**Evidence**:
1. `axis` was added to support `:col` vs `:row` aggregation semantics (kdb-style)
2. `layout` was added to mirror CLISPI's `(o 'H/Z/N/S)` syntax for compatibility
3. Nobody implemented the logic to **bridge** `layout` → underlying `ori` field
4. Nobody implemented operations that **consult** `layout` for behavior

### 5.2 Why Are There Two Separate Metadata Fields?

**Design intent** (inferred from code):
- **`axis`**: Semantic orientation (which dimension to aggregate across)
- **`layout`**: Physical orientation (memory layout: column-major vs row-major)

**Problem**: They were designed as **orthogonal** concepts but never properly integrated:
- `axis` controls **computation** (which dimension)
- `layout` was intended to control **storage** (memory layout)
- But `layout` is only metadata in BLISP, while actual storage is in `blawktrust::TableView.ori`

### 5.3 Architecture Mismatch

```
┌─────────────────────────────────────────────────────┐
│ BLISP Layer (src/value.rs)                         │
│                                                     │
│  TableViewWithMetadata {                           │
│    view: Arc<TableView>,  ← Points to blawktrust   │
│    layout: Layout,        ← BLISP-only ❌ DEAD     │
│    axis: Axis,            ← BLISP-only ✅ USED     │
│  }                                                  │
└────────────────┬────────────────────────────────────┘
                 │ Deref
                 ▼
┌─────────────────────────────────────────────────────┐
│ blawktrust Layer (external crate)                  │
│                                                     │
│  TableView {                                        │
│    table: Table,                                    │
│    ori: u8,  ← REAL orientation ⚠️ READ-ONLY      │
│    ...                                              │
│  }                                                  │
└─────────────────────────────────────────────────────┘
```

**The gap**: BLISP's `layout` field has no way to modify blawktrust's `ori` field.

---

## Part 6: Recommended Fix

### Option A: Remove `layout` and Derive Everything from `ori` ✅ **RECOMMENDED**

**Rationale**:
- `layout` is dead code serving no purpose
- `ori` is the source of truth for physical orientation
- Simplifies the codebase (less metadata to track)
- Maintains compatibility with underlying blawktrust layer

**Changes Required**:

#### A.1 Remove `layout` field

**File**: `src/value.rs:405-409`

```rust
// BEFORE
pub struct TableViewWithMetadata {
    pub view: Arc<blawktrust::TableView>,
    pub layout: Layout,  // ← DELETE THIS
    pub axis: Axis,
}

// AFTER
pub struct TableViewWithMetadata {
    pub view: Arc<blawktrust::TableView>,
    pub axis: Axis,
}
```

#### A.2 Remove Layout enum entirely

**File**: `src/value.rs:328-379`

Delete the entire `Layout` enum and `impl Layout` block (~50 lines).

#### A.3 Update `builtin_o` to modify underlying `ori`

**File**: `src/builtins.rs:3806-3824`

```rust
// BEFORE (sets layout metadata)
} else {
    let layout = Layout::from_str(mode_name)
        .ok_or_else(|| format!("o: unknown layout symbol '{}'", mode_name))?;

    match table_arg {
        Value::TableView(tv) => {
            let new_tv = tv.with_layout(layout);  // ← WRONG
            Ok(Value::TableView(Arc::new(new_tv)))
        }
        _ => { ... }
    }
}

// AFTER (modifies underlying ori)
} else {
    let new_ori = match mode_name {
        "NSWE" | "H" => blawktrust::ORI_H,
        "WENS" | "Z" => blawktrust::ORI_Z,
        "SNWE" | "N" => blawktrust::ORI_X,  // or appropriate mapping
        "EWNS" | "S" => blawktrust::ORI_R,  // or appropriate mapping
        _ => return Err(format!("o: unknown orientation '{}'", mode_name)),
    };

    match table_arg {
        Value::TableView(tv) => {
            // Create new TableView with different ori
            let new_view = tv.view.with_orientation(new_ori)?;  // ← Assumes this method exists
            let new_meta = TableViewWithMetadata::from_view(Arc::new(new_view));
            Ok(Value::TableView(Arc::new(new_meta.with_axis(tv.axis))))
        }
        _ => { ... }
    }
}
```

**Blocker**: Requires `blawktrust::TableView` to expose a method like `with_orientation(ori: u8) -> TableView`.

**If blawktrust doesn't support this**: Option A is not viable without modifying blawktrust.

---

### Option B: Keep `axis`, Force `(o ...)` to Update `axis` Consistently

**Rationale**:
- Accept that `layout`/`ori` are display-only
- Make `(o ...)` operator semantically map symbols to axis
- Simpler fix requiring no blawktrust changes

**Changes Required**:

#### B.1 Remove `layout` field (same as Option A)

#### B.2 Update `builtin_o` to map symbols to axis

**File**: `src/builtins.rs:3806-3824`

```rust
} else {
    // Symbol form: map layout symbols to axis semantics
    let axis = match mode_name {
        "NSWE" | "H" => Axis::Col,  // Column-major = column-wise ops
        "SNWE" | "N" => Axis::Col,  // Still column-major
        "WENS" | "Z" => Axis::Row,  // Row-major = row-wise ops
        "EWNS" | "S" => Axis::Row,  // Still row-major
        _ => return Err(format!("o: unknown orientation '{}'", mode_name)),
    };

    match table_arg {
        Value::TableView(tv) => {
            let new_tv = tv.with_axis(axis);
            Ok(Value::TableView(Arc::new(new_tv)))
        }
        _ => { ... }
    }
}
```

**Semantic mapping**:
- `H`, `N` (column-major layouts) → `Axis::Col` (aggregate down columns)
- `Z`, `S` (row-major layouts) → `Axis::Row` (aggregate across columns)

**Trade-off**: Loses the distinction between reversed layouts (N vs H, S vs Z), but those are rarely used.

---

### Comparison: Option A vs Option B

| Aspect | Option A (Use `ori`) | Option B (Map to `axis`) |
|--------|---------------------|-------------------------|
| **Correctness** | ✅ Physically changes orientation | ⚠️ Semantic interpretation only |
| **Simplicity** | ❌ Requires blawktrust changes | ✅ Pure BLISP changes |
| **Lines changed** | ~30 lines (if blawktrust supports it) | ~15 lines |
| **Breaking changes** | None (users see correct behavior) | None (symbols map to axis) |
| **Future-proof** | ✅ Aligns with true orientation | ⚠️ Hides the ori field |
| **Dependencies** | ❌ Blocked on blawktrust API | ✅ Self-contained |

---

## Part 7: Recommended Action

**IMMEDIATE FIX**: **Option B** (map symbols to axis)

**Minimal patch**:

1. Delete `layout` field from `TableViewWithMetadata` (src/value.rs:407)
2. Delete `Layout` enum and impl (src/value.rs:328-379)
3. Update `builtin_o` symbol branch to map H/Z/N/S → Axis::Col/Row (src/builtins.rs:3806-3824)
4. Remove `with_layout()` method (src/value.rs:486-490)
5. Update tests if any reference layout

**Estimated effort**: 30 minutes

**LONG-TERM**: Investigate blawktrust API for proper orientation mutation (Option A).

---

## Part 8: Summary

### Key Findings

1. ✅ **`axis` field works correctly** - Used by 5 aggregation functions
2. ❌ **`layout` field is dead code** - Never consulted by any operation
3. ⚠️ **`ori` field is read-only** - Only used for display, never modified by `(o ...)`
4. 🐛 **Critical bug**: `(o 'Z table)` has no effect (sets unused `layout` field)

### Impact

- **User confusion**: `(o 'Z df)` appears to work (no error) but does nothing
- **Silent failure**: No warning that layout change is ignored
- **Wasted memory**: `layout` field tracked but unused

### Verification Commands

```bash
# Prove layout is never used
cd /home/ubuntu/blisp
rg "\.layout" --type rust -n
# Result: Only in clone/setter, never in logic

# Prove ori is only used for display
rg "\.ori\b" --type rust -n
# Result: Only src/value.rs:662 (display)

# Prove axis is actively used
rg "\.axis" --type rust -n
# Result: 5 builtin functions check it
```

---

**End of Audit**
