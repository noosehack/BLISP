# Step 4 Complete: Value Types (Col/Table)

**Date:** 2026-02-17
**Status:** ✅ Complete
**Tests:** 47/47 passing (+6 new tests)

---

## 🎉 DATA TYPES UNLOCKED!

**blisp can now work with columns and tables!** This integrates the fast blawktrust backend.

---

## What We Built

### Updated value.rs

**Added to Value enum:**
```rust
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Arc<str>),
    Sym(SymbolId),
    Col(Arc<blawktrust::Column>),    // ← NEW!
    Table(Arc<Table>),                // ← NEW!
}
```

**New Table struct:**
```rust
pub struct Table {
    pub columns: Vec<(SymbolId, blawktrust::Column)>,
    pub row_count: usize,
}
```

**Helper methods added:**
- `as_int()` - Extract i64
- `as_float()` - Extract f64 (coerces int)
- `as_col()` - Extract Column
- `as_table()` - Extract Table
- `display()` - Pretty-print values

**Table methods:**
- `new()` - Create empty table
- `add_column()` - Add a column
- `get_column()` - Get column by name

---

## Demo Output

```
=== Column and Table Types ===

Column: Col[5 elements]
Type: col

Table: Table[5 rows × 2 cols]
Type: table

Extracted 'px' column: 5 elements

✅ Column and Table types working!
```

---

## Test Results

**Total: 47 tests passing (+6 new)**

### New value.rs tests (6):
- test_as_int
- test_as_float
- test_col_type
- test_table_type
- test_table_add_column
- test_display

### Previous tests (41 still passing):
- ast.rs: 2 tests
- reader.rs: 6 tests
- value.rs: 3 tests (original)
- env.rs: 7 tests
- runtime.rs: 9 tests
- eval.rs: 14 tests

---

## Integration with blawktrust

### Column Creation
```rust
let data = vec![100.0, 102.0, 101.5, 103.0, 104.5];
let col = blawktrust::Column::new_f64(data);
let val = Value::Col(Arc::new(col));
```

### Table Creation
```rust
let mut table = Table::new();

// Add price column
let px_col = blawktrust::Column::new_f64(vec![100.0, 102.0, 101.5]);
let px_name = interner.intern("px");
table.add_column(px_name, px_col);

// Add volume column
let vol_col = blawktrust::Column::new_f64(vec![1000.0, 1200.0, 800.0]);
let vol_name = interner.intern("vol");
table.add_column(vol_name, vol_col);

let val = Value::Table(Arc::new(table));
```

### Column Extraction
```rust
if let Ok(table) = val.as_table() {
    if let Some(col) = table.get_column(px_name) {
        // Use column...
    }
}
```

---

## Key Design Decisions

### 1. Arc for Shared Ownership
```rust
Col(Arc<blawktrust::Column>)
Table(Arc<Table>)
```

Columns can be expensive to clone. Using `Arc` means:
- Cloning `Value::Col` is cheap (just increment refcount)
- Multiple variables can reference the same column
- Memory-safe (Rust's Arc guarantees no use-after-free)

### 2. SymbolId for Column Names
```rust
pub columns: Vec<(SymbolId, blawktrust::Column)>
```

Column names are symbols (interned strings):
- Fast comparison (compare integers, not strings)
- Memory-efficient (no string duplication)
- Consistent with rest of blisp

### 3. Manual PartialEq Implementation
```rust
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // ...
            (Value::Col(a), Value::Col(b)) => Arc::ptr_eq(a, b),
            (Value::Table(a), Value::Table(b)) => Arc::ptr_eq(a, b),
            // ...
        }
    }
}
```

Since `blawktrust::Column` doesn't implement `PartialEq`, we compare by pointer for now.
This is fine - two columns are "equal" if they're the same object.

### 4. Type Coercion in as_float()
```rust
pub fn as_float(&self) -> Result<f64, String> {
    match self {
        Value::Float(f) => Ok(*f),
        Value::Int(n) => Ok(*n as f64),  // ← Coercion!
        _ => Err(...),
    }
}
```

This allows flexible numeric operations in Step 5:
```lisp
(* 2 3.14)  ; Int * Float works!
```

---

## Display Output

### Pretty Printing
```rust
println!("{}", val.display(&interner));
```

**Outputs:**
- `Nil` → "nil"
- `Int(42)` → "42"
- `Float(3.14)` → "3.14"
- `Bool(true)` → "true"
- `Str("hello")` → "\"hello\""
- `Sym(id)` → "'foo"
- `Col(...)` → "Col[1000 elements]"
- `Table(...)` → "Table[1000 rows × 3 cols]"

This makes REPL output readable!

---

## What This Enables

### Step 5: Builtin Functions
Now we can implement:
```lisp
(+ 1 2)                    ; Scalar addition
(+ col1 col2)              ; Column addition
(+ col 10)                 ; Broadcast scalar
(* prices 1.1)             ; Scale column
```

### Step 6: Column Operations
Now we can implement:
```lisp
(defparameter prices (load-csv "data.csv"))
(defparameter returns (dlog prices 1))
(defparameter vol (wstd returns 20 1))
```

---

## Architecture Flow

```
User creates column:
  let data = vec![1.0, 2.0, 3.0];
  let col = blawktrust::Column::new_f64(data);
    ↓
Wrap in Value:
  Value::Col(Arc::new(col))
    ↓
Store in variable:
  (defparameter prices col)
    ↓
Use in operations (Step 5+6):
  (dlog prices 1)
    ↓
Call blawktrust kernel:
  blawktrust::dlog_into(&mut out, &prices, 1, &mut scratch)
    ↓
Return new column:
  Value::Col(Arc::new(out))
```

**Fast path: Lisp → blawktrust kernels → Result**

---

## Code Statistics

```
Files:          8 (no new files, updated value.rs)
Lines of code:  ~1250 lines (excluding tests)
Tests:          47/47 passing ✅
New tests:      6 (value.rs type extraction)
```

---

## Integration Points

### With Step 5 (Builtins)
```rust
// In builtin_add()
match (&args[0], &args[1]) {
    (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
    (Value::Col(a), Value::Col(b)) => {
        let result = a.add(b);  // blawktrust operation!
        Ok(Value::Col(Arc::new(result)))
    }
    // ...
}
```

### With Step 6 (Column Ops)
```rust
// In builtin_dlog()
fn builtin_dlog(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    let col = args[0].as_col()?;
    let lag = args[1].as_int()? as usize;

    let mut scratch = blawktrust::Scratch::new();
    let mut out = blawktrust::Column::new_f64(vec![]);
    blawktrust::dlog_into(&mut out, &col, lag, &mut scratch);

    Ok(Value::Col(Arc::new(out)))
}
```

---

## Performance Implications

### Memory Efficiency
- `Arc<Column>` means cheap clones
- Columns are NOT copied unless mutated
- Same memory model as kdb+ (vector sharing)

### Speed
- Zero overhead wrapping (Arc is just a pointer)
- Direct calls to blawktrust kernels
- No intermediate conversions

### Benchmark Target
When we add column operations:
```lisp
(dlog prices 1)  ; Target: 15.51 ms (1M elements)
```

This should match raw blawktrust performance!

---

## Next: Step 5 - Builtin Registry

**Goal:** Add arithmetic and utility functions

**Builtins to implement:**
- Arithmetic: `+`, `-`, `*`, `/`
- Math: `log`, `exp`, `abs`
- Utility: `print`, `type-of`, `len`

**Dispatch rules:**
- scalar + scalar
- col + col
- col + scalar (broadcast)
- Type checking

**Files to create:**
- `src/builtins.rs` (~200-300 lines)

**What we'll be able to do:**
```lisp
(+ 1 2)                    ; => 3
(* 10 3.14)                ; => 31.4
(print "Hello, world!")    ; => prints
(len col)                  ; => 1000
```

**Estimate:** 2-3 hours

---

## After Step 5: Basic Computation Works

```lisp
(defparameter x 10)
(defparameter y 20)
(+ x y)                    ; => 30
(* x y)                    ; => 200
(/ x y)                    ; => 0.5

(defparameter prices (... create col ...))
(* prices 1.1)             ; => scaled column
```

---

## Celebrating Step 4 🎉

**We can now work with real data!**

- ✅ Columns integrated (blawktrust::Column)
- ✅ Tables working (columnar layout)
- ✅ Type extraction (as_col, as_table)
- ✅ Pretty printing (readable output)
- ✅ Arc sharing (efficient memory)

**Next:** Add operations that work on this data!

---

**Status:** Step 4/9 complete ✅
**Next:** Step 5 - Builtin Registry (+, -, *, /, etc.)
**Progress:** Core interpreter + data types done! 🚀

---

## Quick Reference

### Create Column
```rust
let col = blawktrust::Column::new_f64(vec![1.0, 2.0, 3.0]);
let val = Value::Col(Arc::new(col));
```

### Create Table
```rust
let mut table = Table::new();
table.add_column(name_sym, column);
let val = Value::Table(Arc::new(table));
```

### Extract Types
```rust
let n = val.as_int()?;           // i64
let f = val.as_float()?;         // f64 (coerces int)
let col = val.as_col()?;         // Arc<Column>
let table = val.as_table()?;     // Arc<Table>
```

### Display Value
```rust
println!("{}", val.display(&interner));
```

---

**This is where blisp becomes a data processing language!** 📊
