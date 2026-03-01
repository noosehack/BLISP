# blisp - Lisp Interpreter for High-Performance Columnar Operations

**Fast, memory-safe columnar operations with pure Common Lisp syntax.**

---

## Quick Start

```bash
cd /home/ubuntu/blisp

# Build and run
cargo run

# Run tests
cargo test

# Example output:
# blisp v0.1.0
# Step 1: Reader + AST complete!
# Parse: 42
#   => Int(42)
```

---

## User Quick Start (v0.2.0+)

**For users installing BLISP as a tool (not developing it):**

### Install from Git
```bash
cargo install blisp --git https://github.com/noosehack/BLISP --locked
```

**Note:** This installs the `blisp` binary only. For examples and data files, clone the repository:
```bash
git clone https://github.com/noosehack/BLISP
cd BLISP
```

### Verify Installation
```bash
# Check version
blisp --version

# Run self-tests (validates IEEE-754, orientation, masks)
# Works without cloning - self-tests are embedded
blisp --selftest
```

**Expected output:** 6/6 tests pass (IEEE-754 edge cases, orientation, masks, platform)

### Run Examples

#### Without Repository (binary only)
```bash
# Evaluate expression (works anywhere)
blisp -e '(+ 1 2)'

# Verify CSV outputs
echo -e "a;b\n1;2\n3;4" > test.csv
blisp verify test.csv test.csv --tol 1e-6
```

#### With Repository (examples included)
```bash
git clone https://github.com/noosehack/BLISP
cd BLISP

# Run bundled examples
blisp run examples/quickstart/hello.blisp
blisp run examples/quickstart/load_csv.blisp

# Verify output matches expected
blisp run examples/quickstart/load_csv.blisp > output.csv
blisp verify output.csv expected/quickstart_load_csv.csv --tol 1e-6
```

### Help
```bash
# Show all commands and options
blisp --help

# List all builtin operations
blisp --dic
```

**Documentation:**
- [`INSTALL.md`](INSTALL.md) - Detailed installation and verification
- [`SEMANTICS.md`](SEMANTICS.md) - Semantic guarantees and tripwire tests
- Run `blisp --help` for all commands and options

---

## What is blisp?

**blisp** is a Common Lisp interpreter that executes columnar data operations at blazing speed by calling the optimized **blawktrust** backend (1.89× faster than C++).

### Design Goals
- ✅ Pure Common Lisp syntax (no weird extensions)
- ✅ kdb-like performance (15.51 ms for dlog on 1M elements)
- ✅ Memory-safe (Rust borrow checker)
- ✅ Zero-allocation execution after warmup
- ✅ Direct kernel dispatch (thin evaluator)

### Target Example
```lisp
(progn
  (defparameter prices (col (load-csv "prices.csv") 'px))
  (defparameter returns (dlog prices 1))
  (wstd returns 20 1))
```

**Expected performance:** <100 ms for complete pipeline (1M rows)

---

## 🆕 Bloomberg-Style CSV Support

**BLISP now fully supports real-world Bloomberg data:**

```csv
date;ES1 Index;SPY US Equity;volume
2000-01-03;1534.36;145.438;1000
2000-01-10;1542.98;NA;1200
2000-01-17;NA;147.500;800
```

**Features:**
- ✅ **NA handling**: `NA`, `NaN`, `N/A`, `""` → automatically stored as `f64::NAN`
- ✅ **Date columns**: `YYYY-MM-DD` dates auto-detected and stored as timestamps
- ✅ **Spaces in names**: `(col t "SPY US Equity")` works out of the box
- ✅ **Header trimming**: Trailing whitespace automatically removed

**Example:**
```lisp
; Load Bloomberg CSV
(setf data (file "bloomberg.csv"))

; Extract columns with spaces in names
(setf spy (col data "SPY US Equity"))
(setf es (col data "ES1 Index"))

; Compute returns (NA-safe)
(setf returns (dlog spy 1))

; Arithmetic preserves NaN
(setf scaled (+ returns 10.0))
```

**See:** [`BLOOMBERG_CSV_SUPPORT.md`](BLOOMBERG_CSV_SUPPORT.md) for complete documentation.

---

## Architecture

```
User writes Lisp     →  blisp parses/evaluates  →  blawktrust executes
(+ 1 2)                 AST → Runtime → Kernel      Fast column ops
(dlog prices 1)         Dispatch                    15.51 ms (1M elem)
```

**Two projects:**
- **blawktrust** (`/home/ubuntu/blawktrust/`) - Fast backend library
- **blisp** (`/home/ubuntu/blisp/`) - Lisp interpreter (this project)

---

## Current Status

**Step 1/9 complete:** ✅ Reader + AST + Symbol Interner

| Step | Status | Description |
|------|--------|-------------|
| 1 | ✅ | Parser: S-expressions → AST |
| 2 | 🔲 | Environments (lexical + global) |
| 3 | 🔲 | Evaluator (quote, progn, if, let*, defparameter, setf) |
| 4 | 🔲 | Value types (wrap blawktrust Column) |
| 5 | 🔲 | Builtin registry (+, -, *, /, log, exp) |
| 6 | 🔲 | Glue blawktrust kernels (dlog, wstd, etc.) |
| 7 | 🔲 | CLI + REPL |
| 8 | 🔲 | Tests |
| 9 | 🔲 | Benchmarks |

**Next:** Step 2 - Environments

---

## Files

```
blisp/
├── Cargo.toml          Dependencies (blawktrust, rustyline)
├── src/
│   ├── main.rs         ✅ CLI entry point
│   ├── ast.rs          ✅ AST types (Expr, SymbolId, Interner)
│   ├── reader.rs       ✅ Parser (text → AST)
│   ├── value.rs        🔲 Runtime values (TODO: Step 2)
│   ├── env.rs          🔲 Environments (TODO: Step 2)
│   ├── runtime.rs      🔲 Runtime with resolve/define/set (TODO: Step 2)
│   ├── eval.rs         🔲 Evaluator (TODO: Step 3)
│   └── builtins.rs     🔲 Builtin functions (TODO: Step 5)
├── README.md           ← You are here
├── WHERE_WE_ARE.md     Complete continuation guide
└── STATUS.md           Current status summary
```

---

## Language Subset (Spec)

### Supported Syntax
- Lists: `( ... )`
- Symbols: `foo`, `bar`
- Numbers: `42`, `3.14`
- Strings: `"hello"`
- Quote: `'x` → `(quote x)`
- Comments: `; comment`

### Special Forms (When Evaluator is Done)
- `quote` - Return unevaluated
- `progn` - Sequential evaluation, return last
- `if` - Conditional (else required)
- `let*` - Sequential lexical bindings
- `defparameter` - Define global variable
- `setf` - Update variable (lexical or global)

### Builtins (When Complete)
**Arithmetic:** `+`, `-`, `*`, `/`
**Math:** `log`, `exp`, `abs`
**Columns:** `load-csv`, `col`, `dlog`, `wstd`
**Utility:** `print`, `type-of`, `len`

**No macros.** No quasiquote. Pure subset of Common Lisp.

---

## Tests

```bash
cargo test
```

**Current:** 8/8 passing ✅
- Interner (symbol interning)
- Tokenizer (text → tokens)
- Reader (tokens → AST)
- All parse cases (int, float, string, list, quote, comment)

---

## Documentation

- **README.md** (this file) - Quick overview
- **WHERE_WE_ARE.md** - Complete continuation guide with Step 2 implementation
- **STATUS.md** - Current status, next steps, architecture
- **Blueprint:** `/home/ubuntu/clispi_dev/blisp_readme.md` - Full specification

---

## Dependencies

```toml
[dependencies]
blawktrust = { path = "../blawktrust" }  # Fast backend
rustyline = "12.0"                       # REPL support
```

---

## Commands

```bash
# Build
cargo build

# Run
cargo run

# Test
cargo test

# Check (faster than build)
cargo check

# Run optimized (for benchmarks later)
cargo run --release
```

---

## Performance Target

**blawktrust backend:**
- dlog (1M elements): 15.51 ms
- 1.89× faster than C++ blawk_dev
- Zero allocation after warmup

**blisp overhead target:**
- Parser: <1 ms (happens once)
- Evaluator: <0.1 ms per operation (thin dispatch layer)
- Total: Match backend performance

---

## Example Session (When Complete)

```bash
$ blisp repl
blisp> (defparameter x 42)
42

blisp> (+ x 10)
52

blisp> (defparameter prices (load-csv "prices.csv"))
Table[1000000 rows × 3 cols]

blisp> (defparameter returns (dlog (col prices 'px) 1))
Col[1000000]  ; 15.51 ms

blisp> (print (wstd returns 20 1))
Col[1000000]  ; 30 ms
```

---

## Related Projects

- **blawktrust** (`/home/ubuntu/blawktrust/`) - The fast Rust backend
- **clispi** (`/home/ubuntu/clispi_dev/`) - Old C++ implementation (reference)
- **blawk_dev.cpp** - Original C++ blawk (29.31 ms, slower)

---

## Contributing

Follow the 9-step blueprint in order. Each step builds on the previous.

**Next step:** Create `value.rs`, `env.rs`, `runtime.rs` for Step 2.

See **WHERE_WE_ARE.md** for detailed implementation guide.

---

## License

(To be determined)

---

**Status:** Development (Step 1/9 complete)
**Started:** 2026-02-17
**Performance:** Targeting 1.89× faster than C++ ✨
