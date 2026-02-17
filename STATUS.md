# blisp - Current Status

**Date:** 2026-02-17
**Version:** 0.1.0

---

## ✅ Completed

### Project Setup
- ✅ **blawk_kdb → blawktrust** renamed successfully
- ✅ Removed "kdb" references from source code
- ✅ Created `/home/ubuntu/blisp/` directory
- ✅ Cargo project initialized with blawktrust dependency

### Step 1: Reader + AST + Symbol Interner ✅
- ✅ `src/ast.rs` - AST types (Expr, SymbolId, Interner)
- ✅ `src/reader.rs` - Tokenizer and S-expression parser
- ✅ 8/8 tests passing
- ✅ Successfully parses: integers, floats, strings, symbols, lists, quotes, comments

---

## Directory Structure

```
/home/ubuntu/
├── blawktrust/              ← Renamed from blawk_kdb
│   ├── Cargo.toml           (name = "blawktrust")
│   ├── src/
│   │   ├── lib.rs           (1.89× faster than C++)
│   │   └── builtins/
│   └── tests/               35/35 passing
│
├── blisp/                   ← NEW: Lisp interpreter
│   ├── Cargo.toml           (depends on blawktrust)
│   ├── src/
│   │   ├── main.rs          (CLI entry point)
│   │   ├── ast.rs           (✅ Complete)
│   │   └── reader.rs        (✅ Complete)
│   └── STATUS.md            (this file)
│
└── clispi_dev/              ← Old C++ implementation (reference)
```

---

## Demo Output

```bash
$ cargo run

blisp v0.1.0
Step 1: Reader + AST complete!

Parse: 42
  => Int(42)

Parse: (+ 1 2)
  => List([Sym(SymbolId(0)), Int(1), Int(2)])

Parse: 'foo
  => Quote(Sym(SymbolId(1)))

Parse: (progn (defparameter x 10) x)
  => List([Sym(SymbolId(2)), List([Sym(SymbolId(3)), Sym(SymbolId(4)), Int(10)]), Sym(SymbolId(4))])
```

---

## Next Steps (Following Blueprint)

### Step 2: Lexical/Global Environments
**Files to create:**
- `src/env.rs` - Environment types (LexicalEnv, GlobalEnv)
- `src/runtime.rs` - Runtime struct with resolve/define/set

**What it does:**
- Variable binding and resolution
- Lexical scope (stack of frames)
- Global scope (persistent)
- Resolution order: Lexical → Global → Error

**Tests:**
- Resolve from lexical
- Resolve from global
- Shadowing
- setf updates correct scope

---

### Step 3: Evaluator with Special Forms
**Files to create:**
- `src/eval.rs` - Core evaluator logic

**Special forms to implement:**
- `quote` - Return unevaluated
- `progn` - Sequential evaluation
- `if` - Conditional (else required)
- `let*` - Sequential lexical binding
- `defparameter` - Global binding
- `setf` - Update lexical or global

**Tests:**
- Eval literals (numbers, strings)
- Eval symbols (variable lookup)
- Each special form

---

### Step 4: blisp Value/Col/Table Wrappers
**Files to create:**
- `src/value.rs` - Value enum wrapping blawktrust types

**Value types:**
```rust
enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Arc<str>),
    Sym(SymbolId),
    Col(Arc<blawktrust::Column>),
    Table(Arc<Table>),
}
```

---

### Step 5: Builtin Registry
**Files to create:**
- `src/builtins.rs` - Builtin function registry

**Registry:**
```rust
HashMap<SymbolId, BuiltinFn>
BuiltinFn = fn(&mut Runtime, &[Value]) -> Result<Value, Error>
```

**Initial builtins:**
- Arithmetic: `+`, `-`, `*`, `/`
- Math: `log`, `exp`, `abs`
- Utility: `print`, `type-of`, `len`

---

### Step 6: Glue blawktrust Kernels
**Extend builtins.rs:**
- `load-csv` - Load CSV file
- `col` - Extract column from table
- `dlog` - Log returns
- `wstd` - Windowed standard deviation

**Kernel dispatch rules:**
- scalar + scalar
- col + col
- col + scalar (broadcast)
- Strict type checking

---

### Step 7: CLI + REPL
**Update main.rs:**
- Command-line argument parsing
- Three modes:
  - `blisp -e "(expr)"` - Eval expression
  - `blisp run file.lisp` - Run file
  - `blisp repl` - Interactive REPL

**Dependencies:**
- rustyline (already added to Cargo.toml)

---

### Step 8: Tests
**Test coverage:**
- All special forms
- All builtins
- Environment resolution
- Column operations
- Error handling

---

### Step 9: Benchmarks
**Benchmark:**
- dlog on 1M elements
- wstd on 1M elements
- Compare to C++ blawk_dev

**Target:** Match or exceed blawktrust's 15.51 ms/iter

---

## Current Code Stats

```
blisp/
  src/ast.rs:     93 lines (includes tests)
  src/reader.rs:  279 lines (includes tests)
  src/main.rs:    31 lines
  Total:          403 lines
```

**Tests:** 8/8 passing ✅
**Warnings:** 3 (unused code - will be used in later steps)

---

## Performance Target

**blawktrust backend:**
- dlog (1M elements): 15.51 ms
- 1.89× faster than C++ blawk_dev
- Zero allocation after warmup

**blisp should:**
- Match backend performance (minimal eval overhead)
- Fast column dispatch
- Efficient environment lookups

---

## Example Program Goal

```lisp
(progn
  (defparameter prices (col (load-csv "prices.csv") 'px))
  (defparameter r (dlog prices 1))
  (wstd r 20 1))
```

**Expected performance:**
- Load: Fast (I/O bound)
- dlog: ~15 ms (1M elements)
- wstd: ~30 ms (1M elements, window 20)
- Total: <100 ms for complete pipeline

---

## Commands

```bash
# Build
cd /home/ubuntu/blisp
cargo build

# Run
cargo run

# Test
cargo test

# Check
cargo check
```

---

## Dependencies

**Current:**
- `blawktrust` (path dependency)
- `rustyline` (REPL support)

**Future (maybe):**
- `criterion` (benchmarks)
- `clap` (CLI parsing)

---

**Status:** Step 1/9 complete ✅
**Next:** Step 2 - Environments
**Ready to continue!** 🚀
