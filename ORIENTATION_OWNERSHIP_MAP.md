# Orientation Ownership Map - Current State

**Date**: 2026-02-28
**Purpose**: Map where orientation data lives and flows today

---

## Part 1: Where Orientation Lives in blawktrust

### Source Files

**`/home/ubuntu/blawktrust/src/table/orientation.rs`** (455 lines)

#### Ori Enum (Line 15-23)
```rust
pub enum Ori {
    D4 { swap: bool, flip_i: bool, flip_j: bool },
    Each,  // X mode
    Real,  // R mode
}
```

#### OriClass Enum (Line 25-31)
```rust
pub enum OriClass {
    ColwiseLike,   // H, N, _N, _H
    RowwiseLike,   // Z, S, _Z, _S
    Each,
    Real,
}
```

#### Orientation Constants (Line 114-189)
```rust
pub const ORI_H: Ori = Ori::D4 { swap: false, flip_i: false, flip_j: false };
pub const ORI_N: Ori = Ori::D4 { swap: false, flip_i: true, flip_j: false };
pub const ORI__N: Ori = Ori::D4 { swap: false, flip_i: false, flip_j: true };
pub const ORI__H: Ori = Ori::D4 { swap: false, flip_i: true, flip_j: true };
pub const ORI_Z: Ori = Ori::D4 { swap: true, flip_i: false, flip_j: false };
pub const ORI_S: Ori = Ori::D4 { swap: true, flip_i: false, flip_j: false };  // Synonym
pub const ORI__Z: Ori = Ori::D4 { swap: true, flip_i: true, flip_j: false };
pub const ORI__S: Ori = Ori::D4 { swap: true, flip_i: false, flip_j: true };
pub const ORI_X: Ori = Ori::Each;
pub const ORI_R: Ori = Ori::Real;
```

#### ORI_SPECS Registry (Line 191-283)
```rust
pub const ORI_SPECS: [OriSpec; 10] = [
    OriSpec { name: "H", compass: "NSWE", ori: ORI_H, class: ColwiseLike },
    OriSpec { name: "N", compass: "SNWE", ori: ORI_N, class: ColwiseLike },
    // ... all 10 orientations
];
```

#### Helper Functions
- `lookup_ori(name: &str) -> Option<&OriSpec>` (Line 285)
- `impl Ori::class()` (Line 33)
- `impl Ori::logical_shape()` (Line 55)
- `impl Ori::map_ij()` (Line 85)

---

**`/home/ubuntu/blawktrust/src/table/view.rs`** (237 lines)

#### TableView Struct (Line 7-11)
```rust
pub struct TableView {
    pub table: Arc<Table>,
    pub ori: Ori,  // ← SINGLE SOURCE OF TRUTH in blawktrust
}
```

#### Orientation Methods (Line 31-40)
```rust
pub fn with_orientation(&self, ori: Ori) -> Self {
    Self {
        table: Arc::clone(&self.table),  // O(1) - shares data
        ori,
    }
}
```

---

**`/home/ubuntu/blawktrust/src/table/d4_compose.rs`** (150+ lines)

#### D4 Composition Table (Line 15-24)
```rust
pub const D4_COMP: [[u8; 8]; 8] = [ /* ... */ ];
```

#### Composition Function (Line 85-90)
```rust
pub fn compose(a: Ori, b: Ori) -> Option<Ori> {
    let id_a = d4_to_id(a)?;
    let id_b = d4_to_id(b)?;
    let id_c = D4_COMP[id_a as usize][id_b as usize];
    Some(id_to_d4(id_c))
}
```

#### TableView Method (Line 120-128)
```rust
impl TableView {
    pub fn compose_orientation(&self, other: Ori) -> Option<Self> {
        let new_ori = compose(self.ori, other)?;
        Some(Self {
            table: Arc::clone(&self.table),
            ori: new_ori,
        })
    }
}
```

---

## Part 2: Where BLISP Stores Orientation Metadata

### Source Files

**`/home/ubuntu/blisp/src/value.rs`**

#### Axis Enum (Line 332-342)
```rust
pub enum Axis {
    Col,  // Operate down time per column (default, kdb-like)
    Row,  // Operate across columns per row (cross-sectional)
}

impl Default for Axis {
    fn default() -> Self {
        Axis::Col
    }
}
```

#### TableViewWithMetadata Struct (Line 352-356)
```rust
pub struct TableViewWithMetadata {
    pub view: Arc<blawktrust::TableView>,  // Has .ori field from blawktrust
    pub axis: Axis,                         // ← BLISP-only parallel metadata
}
```

**Status**: BLISP has TWO orientation fields:
- `view.ori` (blawktrust's orientation) - **IGNORED**
- `axis` (BLISP's aggregation selector) - **USED**

#### Helper Methods (Line 407-422)
```rust
pub fn clone_shallow(&self) -> Self {
    Self {
        view: Arc::clone(&self.view),
        axis: self.axis,  // ← Copies BLISP metadata
    }
}

pub fn with_axis(&self, axis: Axis) -> Self {
    let mut out = self.clone_shallow();
    out.axis = axis;  // ← Sets BLISP metadata only
    out
}
```

**Problem**: `with_axis()` does NOT call `view.with_orientation()`

---

## Part 3: Where Orientation Is Consulted

### In blawktrust (Correct Usage)

**`/home/ubuntu/blawktrust/src/builtins/ori_ops.rs`**

#### sum() Dispatch (Line 82-92)
```rust
pub fn sum(view: &TableView) -> Column {
    match view.ori_class() {  // ← Checks view.ori
        OriClass::ColwiseLike => sum_colwise(&view.table),
        OriClass::RowwiseLike => sum_rowwise_tiled(&view.table),
        OriClass::Real => sum_all(&view.table),
        OriClass::Each => panic!("sum not defined for Each"),
    }
}
```

#### dlog() Dispatch (Line 150-158)
```rust
pub fn dlog(view: &TableView) -> Table {
    match view.ori_class() {  // ← Checks view.ori
        OriClass::ColwiseLike => dlog_colwise(&view.table),
        OriClass::RowwiseLike => dlog_rowwise(&view.table),
        _ => panic!("dlog only for ColwiseLike/RowwiseLike"),
    }
}
```

**Status**: blawktrust operations **correctly** check `view.ori`.

---

### In BLISP (Incorrect - Parallel System)

**`/home/ubuntu/blisp/src/builtins.rs`**

#### builtin_o (Line 3760-3828)
```rust
fn builtin_o(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    // Symbol form: maps to axis
    let axis = match mode_name {
        "H" | "N" | "NSWE" | "SNWE" => Axis::Col,
        "Z" | "S" | "WENS" | "EWNS" => Axis::Row,
        _ => return Err(...),
    };

    // Sets BLISP axis, NOT blawktrust ori
    let new_tv = tv.with_axis(axis);  // ← Line 3819
    Ok(Value::TableView(Arc::new(new_tv)))
}
```

**Problem**: Does NOT call `tv.view.with_orientation(ori)`

#### builtin_sum (Line 2677-2733)
```rust
fn builtin_sum(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    match &args[0] {
        Value::TableView(tv) => {
            match tv.axis {  // ← Line 2681 - Checks BLISP axis
                Axis::Col => {
                    // BLISP implementation (sum down columns)
                    for (i, col) in tv.table.columns.iter().enumerate() {
                        // ... manual loop
                    }
                }
                Axis::Row => {
                    // BLISP implementation (sum across rows)
                    let nrows = tv.table.columns.get(0).map(|c| c.len()).unwrap_or(0);
                    // ... manual loop
                }
            }
        }
    }
}
```

**Problem**: Reimplements sum logic, does NOT call `blawktrust::sum(&tv.view)`

#### builtin_mean (Line 2779-2843)
Same pattern - checks `tv.axis` (Line 2782), reimplements logic

#### builtin_std (Line 2904-2975)
Same pattern - checks `tv.axis` (Line 2907), reimplements logic

#### builtin_cs1_cols (Line 1534-1613)
Same pattern - checks `tv.axis` (Line 1543), reimplements logic

#### builtin_ecs1_cols (Line 1658-1740)
Same pattern - checks `tv.axis` (Line 1667), reimplements logic

**Status**: 5 legacy builtins check BLISP `axis` instead of blawktrust `ori`.

---

### In BLISP IR System (Ignores All Orientation)

**`/home/ubuntu/blisp/src/exec.rs`**

IR operations work on Frame, not TableView:
- `execute_unary()` (Line 134) - dlog, shift, locf
- `execute_binary()` (Line 216) - arithmetic
- No orientation checking at all

**Status**: IR path ignores both BLISP `axis` and blawktrust `ori`.

---

## Part 4: Data Flow (Current State)

### Creation Flow

```
User: (stdin)
  ↓
BLISP: io::load_stdin() (src/io.rs:208)
  ↓
BLISP: parse_csv() → blawktrust::TableView::new(table)
  ↓ (src/io.rs:436)
Creates: blawktrust::TableView { table, ori: ORI_H }  ← blawktrust default
  ↓
BLISP: Value::tableview(view) (src/value.rs:633)
  ↓
Creates: TableViewWithMetadata {
    view: Arc<TableView { ori: ORI_H }>,  ← blawktrust
    axis: Axis::Col,                      ← BLISP default
}
```

**Result**: Two orientation fields initialized independently.

---

### Orientation Change Flow (Current - Broken)

```
User: (o 'Z df)
  ↓
BLISP: builtin_o() (src/builtins.rs:3760)
  ↓
Parses 'Z → axis = Axis::Row (Line 3812)
  ↓
Calls: tv.with_axis(Axis::Row) (Line 3819)
  ↓ (src/value.rs:418)
Creates: TableViewWithMetadata {
    view: Arc::clone(&tv.view),  // ← Same blawktrust view (ori=ORI_H unchanged!)
    axis: Axis::Row,              // ← BLISP metadata updated
}
```

**Problem**: `view.ori` remains `ORI_H`, only BLISP `axis` changes.

---

### Operation Flow (Current - Divergent Paths)

#### Path A: Legacy Builtin (sum)

```
User: (sum df)
  ↓
BLISP: builtin_sum() (src/builtins.rs:2663)
  ↓
Checks: tv.axis (Line 2681)  ← BLISP metadata
  ↓
Axis::Col → sum_colwise() (BLISP implementation, Line 2682)
  ↓
Manually loops over tv.table.columns
  ↓
Returns: Value::tableview(result)
```

**Does NOT use**: blawktrust::sum(&tv.view)

#### Path B: Thin Wrapper (dlog)

```
User: (dlog df 1)
  ↓
BLISP: builtin_dlog() (src/builtins.rs:2252)
  ↓
Calls: blawktrust::dlog_column(col, lag) (Line 2265)  ← Direct delegation
  ↓
Returns: Value::Col(Arc::new(result))
```

**Problem**: Doesn't use TableView-aware `blawktrust::dlog(view)` which would check `ori`.

---

### Print Flow (Current - Uses blawktrust ori)

```
User: (print df)
  ↓
BLISP: builtin_print() (src/builtins.rs:2614)
  ↓
Calls: value.display(interner) (Line 2620)
  ↓ (src/value.rs:591)
Value::TableView display:
  ↓
Accesses: tv.ori (Line 596) via Deref to blawktrust::TableView
  ↓
Displays: "TableView[ori=H, shape=...]"
```

**Uses**: blawktrust `ori` (not BLISP `axis`)

**Result**: Print shows `ori=H` even after `(o 'Z df)` sets `axis=Row`.

---

## Summary Table

| Component | File | Line | Checks | Notes |
|-----------|------|------|--------|-------|
| **blawktrust TableView** | view.rs | 9 | `ori: Ori` | Single source of truth (engine) |
| **blawktrust sum()** | ori_ops.rs | 82 | `view.ori_class()` | ✅ Correct |
| **blawktrust dlog()** | ori_ops.rs | 150 | `view.ori_class()` | ✅ Correct |
| **BLISP Wrapper** | value.rs | 354 | `axis: Axis` | ❌ Parallel metadata |
| **BLISP (o ...)** | builtins.rs | 3760 | Sets `axis` | ❌ Should set `ori` |
| **BLISP sum** | builtins.rs | 2681 | Checks `axis` | ❌ Should call blawktrust |
| **BLISP mean** | builtins.rs | 2782 | Checks `axis` | ❌ Should call blawktrust |
| **BLISP std** | builtins.rs | 2907 | Checks `axis` | ❌ Should call blawktrust |
| **BLISP cs1-cols** | builtins.rs | 1543 | Checks `axis` | ❌ Should call blawktrust |
| **BLISP ecs1-cols** | builtins.rs | 1667 | Checks `axis` | ❌ Should call blawktrust |
| **BLISP print** | value.rs | 596 | Uses `view.ori` | ✅ Correct (but confusing) |

---

## The Problem (Concise)

**Two parallel orientation systems**:
1. blawktrust: `TableView.ori` (10 states, D4 group, correct)
2. BLISP: `TableViewWithMetadata.axis` (2 states, simplified, wrong)

**Data flow**:
- `(o 'Z df)` sets BLISP `axis`, not blawktrust `ori`
- Operations check BLISP `axis`, not blawktrust `ori`
- Print shows blawktrust `ori`, causing confusion

**Result**: Inconsistent state where `ori != axis`.

---

## What Needs to Change

**Remove BLISP parallel system**:
1. Delete `Axis` enum
2. Delete `axis` field from `TableViewWithMetadata`
3. Use only `view.ori` as source of truth

**Update operations**:
1. `builtin_o()` → call `view.with_orientation(ori)`
2. `builtin_sum()` → delegate to `blawktrust::sum(&view)`
3. `builtin_mean/std/cs1/ecs1` → delegate to blawktrust
4. Add `builtin_ro()` → call `view.compose_orientation(ori)`

**Add symbols**:
Support all 10 orientations: H, N, _N, _H, Z, S, _Z, _S, X, R

---

**End of Ownership Map**
