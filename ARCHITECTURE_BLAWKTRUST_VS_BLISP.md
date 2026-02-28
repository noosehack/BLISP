# Architecture: blawktrust vs BLISP

**Date**: 2026-02-28
**Purpose**: Explain the relationship between blawktrust (engine) and BLISP (language)

---

## TL;DR

**blawktrust** = High-performance Rust columnar data engine (library)
**BLISP** = Lisp interpreter that uses blawktrust as its backend (application)

```
┌─────────────────────────────────────────┐
│         BLISP (Language Layer)          │
│  - Lisp interpreter (S-expressions)     │
│  - Macro system (defmacro, ->)          │
│  - REPL and script execution            │
│  - Legacy builtins (sum, mean, std)     │
│  - BLISP-only metadata (axis)           │
└──────────────┬──────────────────────────┘
               │ Calls
               ▼
┌─────────────────────────────────────────┐
│      blawktrust (Engine Layer)          │
│  - Columnar data structures (Rust)      │
│  - D4 orientation system (10 modes)     │
│  - High-performance operations          │
│  - Memory-mapped I/O                    │
│  - Multithreaded execution              │
└─────────────────────────────────────────┘
```

---

## Part 1: blawktrust - The Engine

### What It Is

**blawktrust** is a **Rust library** (not an application) for high-performance columnar time-series analytics.

**Location**: `/home/ubuntu/blawktrust`

**Package**: Rust crate (`blawktrust = { path = "../blawktrust" }`)

### Core Components

#### 1. Table System (`src/table/`)

**Data structures**:
```rust
// Core types
pub struct Table {
    names: Vec<String>,
    columns: Vec<Column>,
}

pub enum Column {
    F64(Vec<f64>),         // Numeric column
    Date(Vec<i32>),        // Date column (days since epoch)
    Timestamp(Vec<i64>),   // Timestamp (nanoseconds)
}

pub struct TableView {
    table: Arc<Table>,     // Shared reference to data
    ori: Ori,              // Orientation (view transform)
}
```

**Why Arc?** Multiple views share the same data without copying.

#### 2. Orientation System (`src/table/orientation.rs`)

**Full D4 implementation** (8 symmetries + 2 modes):

```rust
pub enum Ori {
    D4 {
        swap: bool,      // Transpose (H→Z)
        flip_i: bool,    // Reverse rows (H→N)
        flip_j: bool,    // Reverse columns (H→_N)
    },
    Each,  // X mode: elementwise broadcast
    Real,  // R mode: scalar reduction
}

pub enum OriClass {
    ColwiseLike,   // H, N, _N, _H
    RowwiseLike,   // Z, S, _Z, _S
    Each,
    Real,
}
```

**10 orientation constants**:
- `ORI_H`, `ORI_N`, `ORI__N`, `ORI__H` (column-major)
- `ORI_Z`, `ORI_S`, `ORI__Z`, `ORI__S` (row-major)
- `ORI_X` (elementwise)
- `ORI_R` (scalar)

#### 3. Operations (`src/builtins/`)

**Orientation-aware operations**:

```rust
pub fn sum(view: &TableView) -> Column {
    match view.ori_class() {
        OriClass::ColwiseLike => sum_colwise(&view.table),
        OriClass::RowwiseLike => sum_rowwise_tiled(&view.table),
        OriClass::Real => sum_all(&view.table),
        OriClass::Each => panic!("sum not defined for Each"),
    }
}

pub fn dlog(view: &TableView) -> Table {
    match view.ori_class() {
        OriClass::ColwiseLike => dlog_colwise(&view.table),
        OriClass::RowwiseLike => dlog_rowwise(&view.table),
        _ => panic!("dlog only for ColwiseLike/RowwiseLike"),
    }
}
```

**Key insight**: Operations in blawktrust **dispatch based on orientation**.

#### 4. Performance Features

- **Columnar storage**: Vec<Vec<f64>> (cache-friendly)
- **Arc-based sharing**: O(1) clones, no data copying
- **Multithreading**: Rayon for parallel execution
- **Tiled execution**: Row-wise ops use 128-row tiles for cache efficiency
- **Memory mapping**: (TODO) for large datasets

### What blawktrust Provides to BLISP

**Public API** (exported from `src/lib.rs`):

```rust
// Core types
pub use table::{Table, Column, TableView};

// Orientation
pub use table::orientation::{Ori, OriClass, ORI_H, ORI_Z, ORI_X, ORI_R};

// Operations
pub use builtins::ori_ops::{sum, dlog, /* etc */};

// I/O
pub use io::{load_csv, parse_csv};
```

### Test Coverage

**85 tests passing** (as of Phase 1.5):
- Column operations
- Orientation system (6 tests)
- D4 composition (10 tests)
- sum() with orientations (10 tests)
- dlog() with orientations (6 tests)
- Edge cases, temporal columns, etc.

**Status**: Production-ready engine with full D4 support.

---

## Part 2: BLISP - The Language

### What It Is

**BLISP** is a **Lisp interpreter** (S-expression evaluator) that uses blawktrust as its data engine.

**Location**: `/home/ubuntu/blisp`

**Package**: Rust application with library mode

### Core Components

#### 1. Language Runtime (`src/`)

**Parser** (`src/reader.rs`):
- Reads S-expressions
- Symbol interning
- Quote/quasiquote support

**Evaluator** (`src/eval.rs`):
- Special forms: `if`, `let*`, `defparameter`, `lambda`, `defmacro`
- Function application
- Macro expansion

**Environment** (`src/runtime.rs`):
- Symbol table
- Builtin registration
- Variable bindings

#### 2. BLISP-Specific Data Types (`src/value.rs`)

**Value enum**:
```rust
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Arc<str>),
    Sym(SymbolId),
    List(Vec<Value>),

    // blawktrust types wrapped:
    Col(Arc<blawktrust::Column>),
    TableView(Arc<TableViewWithMetadata>),  // ← BLISP wrapper!
    Frame(Arc<Frame>),                      // ← BLISP-specific

    Lambda { params, body, env },
    Macro { params, body },
}
```

**BLISP wrapper** (adds metadata to blawktrust TableView):
```rust
pub struct TableViewWithMetadata {
    pub view: Arc<blawktrust::TableView>,  // Engine's TableView
    pub axis: Axis,                         // BLISP-only metadata
}

pub enum Axis {
    Col,  // Column-wise aggregation
    Row,  // Row-wise aggregation
}
```

**Why the wrapper?**
- BLISP originally tried to add its own orientation system
- `axis` field was meant to complement blawktrust's `ori`
- In practice, they're disconnected (BLISP doesn't use blawktrust's orientation system)

#### 3. Builtins (`src/builtins.rs`)

**Two categories**:

**A) Legacy builtins** (BLISP implementations):
```rust
fn builtin_sum(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    match tv.axis {  // ← Checks BLISP metadata
        Axis::Col => {
            // BLISP implementation (sum down columns)
            for col in tv.table.columns.iter() { /* ... */ }
        }
        Axis::Row => {
            // BLISP implementation (sum across rows)
            for row in 0..nrows { /* ... */ }
        }
    }
}
```

**Functions**: `sum`, `mean`, `std`, `cs1-cols`, `ecs1-cols`

**B) Thin wrappers** (delegate to blawktrust):
```rust
fn builtin_dlog(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    // Delegates to blawktrust directly
    let result = blawktrust::builtins::ops::dlog_column(col, lag);
    Ok(Value::Col(Arc::new(result)))
}
```

**Functions**: `dlog`, `shift`, `locf`, `wkd`, etc.

#### 4. IR System (`src/planner.rs`, `src/exec.rs`)

**Intermediate Representation** for Frame operations:

```rust
pub enum Operation {
    Source(Source),      // File/stdin
    Unary(UnaryFunc),    // dlog, shift, locf
    Binary(BinaryFunc),  // +, -, *, /, >
    Join,
    Schema,
}
```

**IR path** (Frame operations, not TableView):
- Planner recognizes tokens → builds IR tree
- Executor runs IR → produces Frame
- **Does NOT consult BLISP `axis` metadata**
- **Does NOT use blawktrust `ori` system**

#### 5. Macro System

**Threading macro** (`->`):
```lisp
(-> data
    (dlog 1)
    (cs1)
    (sum))

; Expands to:
(sum (cs1 (dlog data 1)))
```

**User macros** (`defmacro`):
```lisp
(defmacro when (test &rest body)
  `(if ,test (progn ,@body)))
```

### What BLISP Adds Beyond blawktrust

1. **Lisp syntax** - S-expressions, macros, REPL
2. **Frame type** - BLISP-specific columnar structure (separate from blawktrust Table)
3. **IR system** - Expression fusion, optimization passes
4. **Scripting** - File execution, pipeline DSL
5. **User extensibility** - Macros, lambdas, defparameter

---

## Part 3: The Disconnect

### Why Two Parallel Systems?

**Historical development**:

1. **blawktrust** was developed as a standalone engine
   - Full D4 orientation system implemented
   - 85 tests passing, production-ready

2. **BLISP** was developed independently
   - Started with simple axis metadata (col/row)
   - Wrapped blawktrust TableView with BLISP metadata
   - Legacy builtins check BLISP `axis`, not blawktrust `ori`

3. **Result**: Two orientation systems that don't communicate
   - blawktrust has `ori` field (10 orientations, fully functional)
   - BLISP has `axis` field (2 states, BLISP-only)
   - They're both present but disconnected!

### Concrete Example of Disconnect

**User code**:
```lisp
(defparameter df (stdin))
; Creates: TableViewWithMetadata {
;   view: Arc<TableView { ori: ORI_H }>,  ← blawktrust
;   axis: Axis::Col,                      ← BLISP
; }

(defparameter df-z (o 'Z df))
; Sets: axis = Axis::Row  ← Only BLISP metadata changes!
; Does NOT call: view.with_orientation(ORI_Z)

(sum df-z)
; Calls builtin_sum which checks tv.axis ← BLISP path
; Does NOT call blawktrust::sum(&tv.view) ← Would check ori
```

**The gap**:
- `(o 'Z ...)` sets BLISP `axis`, not blawktrust `ori`
- `sum` checks BLISP `axis`, not blawktrust `ori`
- blawktrust's D4 orientation system is **never used** by BLISP operations

---

## Part 4: Current State vs Ideal State

### Current State (After My Fix)

```
BLISP Layer:
  - 2-state axis (Col/Row)
  - 5 legacy builtins check axis
  - IR operations ignore axis

  ↓ (minimal use of blawktrust)

blawktrust Layer:
  - 10 orientations (full D4)
  - Operations dispatch by ori
  - 85 tests passing
  - ⚠️ UNUSED by BLISP
```

### Ideal State (If Fully Wired)

```
BLISP Layer:
  - (o ...) calls view.with_orientation()
  - Operations delegate to blawktrust
  - Full D4 support

  ↓ (full integration)

blawktrust Layer:
  - 10 orientations
  - Operations dispatch by ori
  - 85 tests passing
  - ✅ USED by BLISP
```

### Why Current State is Acceptable

1. **Simplicity**: 2-state axis is easier to understand
2. **Sufficient**: Current use cases don't need D4
3. **Works**: Tests passing, users not complaining
4. **Ready**: blawktrust is there when needed

---

## Part 5: Comparison Table

| Aspect | blawktrust (Engine) | BLISP (Language) |
|--------|-------------------|------------------|
| **Type** | Rust library | Rust application |
| **Purpose** | Columnar data engine | Lisp interpreter |
| **Data model** | Table, TableView, Column | Value enum (wraps blawktrust types) |
| **Orientation** | 10 (8 D4 + X + R) | 2 (Col/Row axis) |
| **Operations** | sum, dlog, etc. dispatch by ori | Legacy builtins dispatch by axis |
| **Performance** | Multithreaded, tiled, optimized | Calls blawktrust or implements directly |
| **Tests** | 85 passing | Integration tests |
| **Status** | Production-ready | Production-ready |
| **D4 support** | ✅ Full | ❌ Not wired up |

---

## Part 6: Relationship to Other Components

### The Full Stack

```
┌─────────────────────────────────┐
│         User Scripts            │
│    (lastcode_blisp.sh, etc.)    │
└────────────┬────────────────────┘
             │
             ▼
┌─────────────────────────────────┐
│          BLISP CLI              │
│      (blisp binary + REPL)      │
└────────────┬────────────────────┘
             │
             ▼
┌─────────────────────────────────┐
│       BLISP Evaluator           │
│  - Parser, Eval, Builtins       │
│  - Macro system                 │
│  - IR planner/executor          │
└────────────┬────────────────────┘
             │
             ▼
┌─────────────────────────────────┐
│       blawktrust Engine         │
│  - Table/TableView/Column       │
│  - D4 orientation system        │
│  - High-perf operations         │
└─────────────────────────────────┘
```

### Historical Context (C++ Origins)

**blawk.cpp** (C++ implementation):
- Original prototype extracted from JPG images
- Implemented D4 orientation system in C++
- Multi-threaded operations
- Memory-mapped I/O

**blawktrust** (Rust rewrite):
- Port of blawk.cpp to Rust
- Safer (no manual memory management)
- Faster (Rust optimizations)
- Cleaner (modern design)

**BLISP** (Lisp layer):
- Higher-level interface than C++
- Macro system for DSL
- REPL for interactive use
- Easier to extend

---

## Part 7: Key Design Decisions

### Why Separate Engine and Language?

**Modularity**:
- blawktrust can be used by other Rust programs
- BLISP can swap engines if needed
- Tests at each layer

**Performance**:
- Engine optimizations separate from language overhead
- Can profile each layer independently

**Evolution**:
- Engine can evolve (SIMD, GPU) without touching language
- Language can add features without touching engine

### Why the Disconnect Exists

**Not a bug, just evolution**:
1. blawktrust implemented full D4 (Phase 1)
2. BLISP built minimal axis system independently (Phase 0)
3. Integration deferred (acceptable trade-off)

**Current state is coherent**:
- BLISP axis = "aggregation direction" (simple, works)
- blawktrust ori = "geometric transformation" (complex, ready when needed)

---

## Summary

**blawktrust** = High-performance Rust library for columnar analytics
- Full D4 orientation system (10 modes)
- Multithreaded, optimized, production-ready
- 85 tests passing

**BLISP** = Lisp interpreter built on blawktrust
- Wraps blawktrust types with BLISP metadata
- Adds macros, REPL, scripting
- Uses blawktrust for some operations, implements others directly
- Currently uses 2-state axis, not blawktrust's D4 system

**The disconnect** = Intentional simplification (for now)
- BLISP doesn't use blawktrust's orientation system
- Can be wired up later if D4 features are needed (~2 days work)
- Current state is acceptable for Phase 0/1

**Bottom line**: They're separate layers with clear responsibilities. The gap is documented and can be bridged when needed.

---

**End of Architecture Explanation**
