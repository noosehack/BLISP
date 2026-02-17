# Step 5 Complete: Builtin Registry

**Date:** 2026-02-17
**Status:** ✅ Complete
**Tests:** 57/57 passing (+10 new tests)

---

## 🎉 ARITHMETIC WORKS! COLUMN OPS WORK!

**blisp can now perform computations!** Arithmetic, math functions, and column operations all working.

---

## What We Built

### New File: src/builtins.rs (440 lines)

**10 builtin functions implemented:**

**Arithmetic (4):**
- `+` - Addition
- `-` - Subtraction
- `*` - Multiplication
- `/` - Division

**Math (3):**
- `log` - Natural logarithm
- `exp` - Exponential (e^x)
- `abs` - Absolute value

**Utility (3):**
- `print` - Print values
- `type-of` - Get type name
- `len` - Get length

**Dispatch rules:**
- scalar + scalar → scalar
- col + col → col
- col + scalar → col (broadcast)
- scalar + col → col (broadcast)

### Updated Files

**runtime.rs:**
- Added `builtins: HashMap<SymbolId, BuiltinFn>`
- `register_builtin()` - Register a builtin
- `is_builtin()` - Check if symbol is builtin
- `call_builtin()` - Invoke builtin function

**eval.rs:**
- Updated `eval_list()` to dispatch to builtins
- Evaluates arguments before calling builtin
- Falls through from special forms to builtins

**main.rs:**
- Added `mod builtins`
- New demo showing all builtins working
- Column operations demo

---

## Demo Output

```
>>> Add integers
(+ 1 2)
=> 3

>>> Add floats
(+ 3.14 2.86)
=> 6

>>> Add int and float
(+ 1 2.5)
=> 3.5

>>> Subtract
(- 10 3)
=> 7

>>> Multiply
(* 3 4)
=> 12

>>> Divide
(/ 10 2)
=> 5

>>> Absolute value
(abs -5)
=> 5

>>> Natural log
(log 2.718281828)
=> 0.9999999998311266

>>> Exponential
(exp 1.0)
=> 2.718281828459045

>>> Define x
(defparameter x 10)
=> 10

>>> Add with variable
(+ x 5)
=> 15

>>> Multiply with variable
(* x 2)
=> 20

>>> Type of int
(type-of 42)
=> "int"

>>> Type of float
(type-of 3.14)
=> "float"

>>> Print string
(print "Hello, blisp!")
"Hello, blisp!"
=> nil

>>> Nested: (2*3) + (10-5)
(+ (* 2 3) (- 10 5))
=> 11

>>> Nested: (1+2) * (3+4)
(* (+ 1 2) (+ 3 4))
=> 21

=== Column Operations ===

>>> Column length
(len prices)
=> 5

>>> Add scalar to column
(+ prices 10)
=> Col[5 elements]
   First 3: [110.0, 112.0, 111.5]

>>> Scale column by 1.1
(* prices 1.1)
=> Col[5 elements]
   First 3: [110.0, 112.2, 111.65]

>>> Log of column
(log prices)
=> Col[5 elements]
   First 3: [4.605, 4.625, 4.620]

✅ All builtins working!
```

**Everything works!** 🎉

---

## Test Results

**Total: 57 tests passing (+10 new)**

### New builtins.rs tests (10):
- test_add_scalars
- test_add_float_int
- test_mul_scalars
- test_div_scalars
- test_div_by_zero
- test_abs
- test_type_of
- test_len_col
- test_add_column_scalar
- test_mul_column_scalar

### Previous tests (47 still passing):
- ast.rs: 2 tests
- reader.rs: 6 tests
- value.rs: 9 tests
- env.rs: 7 tests
- runtime.rs: 9 tests
- eval.rs: 14 tests

---

## Key Implementation Details

### Builtin Dispatch Flow

```
User types: (+ 1 2)
    ↓
Reader: Parse → List[Sym(+), Int(1), Int(2)]
    ↓
Evaluator: eval_list()
    ↓
Not a special form? Check if builtin: is_builtin(+) → true
    ↓
Evaluate arguments: [Int(1), Int(2)] → [Value::Int(1), Value::Int(2)]
    ↓
Call builtin: call_builtin(+, &[Value::Int(1), Value::Int(2)])
    ↓
Dispatch by types: (Int, Int) → Int
    ↓
Result: Value::Int(3)
```

### Type Dispatch Example (+)

```rust
fn builtin_add(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    match (&args[0], &args[1]) {
        // Scalar + Scalar
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),

        // Col + Col
        (Value::Col(a), Value::Col(b)) => {
            let result = add_columns(a, b)?;
            Ok(Value::Col(Arc::new(result)))
        }

        // Col + Scalar (broadcast)
        (Value::Col(c), Value::Float(s)) => {
            let result = add_column_scalar(c, *s)?;
            Ok(Value::Col(Arc::new(result)))
        }

        // ... more patterns
    }
}
```

**Exhaustive pattern matching** ensures type safety!

### Column Operations

```rust
fn add_column_scalar(col: &blawktrust::Column, scalar: f64) -> Result<blawktrust::Column, String> {
    match col {
        blawktrust::Column::F64 { data, valid: _ } => {
            let result: Vec<f64> = data.iter().map(|x| x + scalar).collect();
            Ok(blawktrust::Column::new_f64(result))
        }
        _ => Err("Column scalar addition only supported for F64 columns".to_string()),
    }
}
```

For now, we use simple iterator-based operations. **Step 6 will use blawktrust's optimized kernels**.

---

## What This Enables

### Real Computation!

```lisp
; Calculate compound interest
(defparameter principal 1000.0)
(defparameter rate 0.05)
(defparameter years 10)

(* principal (exp (* rate years)))  ; => 1648.72

; Work with columns
(defparameter prices (... create col ...))
(defparameter scaled (* prices 1.1))    ; Scale by 10%
(defparameter logged (log prices))      ; Log transform
```

### Nested Expressions

```lisp
(+ (* 2 3) (- 10 5))                    ; => 11
(* (+ 1 2) (+ 3 4))                     ; => 21
(log (exp (abs -2.5)))                  ; => 2.5
```

### Column Broadcasting

```lisp
(+ prices 10)                           ; Add 10 to all elements
(* returns 252)                         ; Annualize returns
(/ prices 100)                          ; Convert to decimals
```

---

## Architecture Integration

### Builtin Registry at Runtime

```rust
pub struct Runtime {
    pub lexical: LexicalEnv,
    pub global: GlobalEnv,
    pub interner: Interner,
    pub builtins: HashMap<SymbolId, BuiltinFn>,  // ← NEW!
}
```

**Builtins are registered at Runtime::new():**
```rust
impl Runtime {
    pub fn new() -> Self {
        let mut rt = Self {
            // ... fields ...
            builtins: HashMap::new(),
        };

        crate::builtins::register_builtins(&mut rt);  // Register all!

        rt
    }
}
```

### Evaluator Dispatch

```rust
fn eval_list(&mut self, exprs: &[Expr]) -> Result<Value, String> {
    // 1. Check special forms first
    match head_name {
        "quote" => return self.eval_quote(...),
        "progn" => return self.eval_progn(...),
        // ... other special forms
        _ => {}
    }

    // 2. Check builtins
    if self.is_builtin(head_sym) {
        let arg_vals = /* evaluate arguments */;
        return self.call_builtin(head_sym, &arg_vals);
    }

    // 3. Unknown
    Err("Unknown function or special form")
}
```

**Clean separation:** Special forms vs builtins!

---

## Performance Notes

### Current Implementation

For Step 5, we use **simple iterator-based operations**:

```rust
let result: Vec<f64> = data.iter().map(|x| x + scalar).collect();
```

This is:
- ✅ Simple and correct
- ✅ Readable
- ⚠️ Not optimized (allocates, no SIMD)

### Step 6 Will Use blawktrust Kernels

```rust
// Step 6 will use optimized kernels:
blawktrust::dlog_into(&mut out, &prices, 1, &mut scratch);
// → 15.51 ms (1M elements)
// → Zero allocation after warmup
// → SIMD vectorization
```

**Step 5 focus:** Correctness and API design
**Step 6 focus:** Performance (fast kernels)

---

## Code Statistics

```
Files:          9 (added builtins.rs)
Lines of code:  ~1700 lines (excluding tests)
Tests:          57/57 passing ✅
New tests:      10 (builtins)
Builtins:       10 functions
```

---

## What's Next: Step 6 - Column Operations

**Goal:** Add specialized column operations using blawktrust kernels

**Operations to add:**
- `dlog` - Log returns (uses blawktrust::dlog_into)
- `wstd` - Windowed standard deviation
- `shift` - Lag/lead values
- `diff` - Differences
- More financial/statistical operations

**Integration:**
```lisp
(defparameter prices (... load ...))
(defparameter returns (dlog prices 1))     ; ← blawktrust kernel!
(defparameter vol (wstd returns 20 1))     ; ← blawktrust kernel!
```

**Performance target:**
- dlog on 1M elements: 15.51 ms (match blawktrust raw speed)
- Zero allocation after warmup
- Full SIMD vectorization

**Estimate:** 2-3 hours

---

## After Step 6: Full Data Pipeline

```lisp
(progn
  ; Load data
  (defparameter prices (load-csv "prices.csv"))

  ; Extract column
  (defparameter px (col prices 'px))

  ; Compute returns (FAST: blawktrust kernel)
  (defparameter r (dlog px 1))

  ; Annualize
  (defparameter annual (* r 252))

  ; Compute volatility (FAST: blawktrust kernel)
  (defparameter vol (wstd r 20 1))

  ; Print results
  (print "Returns:" (len r) "elements")
  (print "Volatility:" (len vol) "elements"))
```

**This will be blazing fast!** 🚀

---

## Progress: 5/9 Steps Complete

| Step | Status | Description |
|------|--------|-------------|
| 1 | ✅ | Reader + AST + Symbol Interner |
| 2 | ✅ | Environments (Lexical + Global) |
| 3 | ✅ | Evaluator (Execute Lisp!) |
| 4 | ✅ | Value Types (Col/Table) |
| 5 | ✅ | **Builtin Registry (+, -, *, /)** ← JUST FINISHED |
| 6 | 🔲 | Column Operations (dlog, wstd) |
| 7 | 🔲 | CLI + REPL |
| 8 | 🔲 | Tests |
| 9 | 🔲 | Benchmarks |

---

## Celebrating Step 5 🎉

**We can now compute!**

- ✅ Arithmetic works (+, -, *, /)
- ✅ Math functions work (log, exp, abs)
- ✅ Type coercion works (int + float)
- ✅ Column broadcasting works (col + scalar)
- ✅ Nested expressions work
- ✅ Utility functions work (print, type-of, len)

**The interpreter is feature-complete for basic computation!**

What remains:
- Step 6: Fast column operations (dlog, wstd)
- Step 7: User-friendly REPL
- Step 8-9: Testing & benchmarks

---

## Quick Commands

```bash
cd /home/ubuntu/blisp

# Run demo
cargo run

# Run tests
cargo test

# Test specific builtins
cargo test builtin

# See all output
cargo test -- --nocapture
```

---

**Status:** Step 5/9 complete ✅
**Next:** Step 6 - Column Operations (blawktrust integration)
**Progress:** Core interpreter + arithmetic complete! 🚀

---

**This is where blisp becomes useful for real work!** 📊
