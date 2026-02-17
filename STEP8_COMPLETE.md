# Step 8 Complete: File I/O (CSV Loading & Saving)

**Date:** 2026-02-17
**Status:** ✅ Complete
**Tests:** 64/64 passing (+2 new tests: col_extraction, w_extraction, +2 io tests)

---

## 🎯 FILE I/O WORKS!

**blisp can now load and process CSV files!** Just like clispi, we can now:
- Load CSV files from disk
- Read CSV from stdin
- Extract columns from tables
- Process real financial data

---

## What We Built

### New I/O Operations (5)

**1. `(file "filename.csv")` - Load CSV file:**
```lisp
(file "GC1C.csv")                    ; Load gold futures
(file "ES1I.csv")                    ; Load S&P futures
(file "test_prices.csv")             ; Load test data
```

**2. `(stdin)` - Read CSV from stdin:**
```lisp
cat prices.csv | ./blisp -e '(stdin)'
```

**3. `(save "filename.csv" table)` - Save table to CSV:**
```lisp
(save "output.csv" results)
```

**4. `(col table 'colname)` - Extract column by name:**
```lisp
(col prices 'px)                     ; Extract 'px' column
(col data 'volume)                   ; Extract 'volume' column
```

**5. `(w table index)` - Extract column by index:**
```lisp
(w prices 0)                         ; First column
(w prices 1)                         ; Second column
(w prices 5)                         ; Column at index 5
```

### New Files

**src/io.rs** (new file, 189 lines):
- `load_csv(filename, interner)` - Load CSV from file
- `load_stdin(interner)` - Read CSV from stdin
- `parse_csv(content, interner)` - Parse CSV content into Table
- `save_csv(filename, table, interner)` - Save Table to CSV
- 2 comprehensive tests

### Updated Files

**Cargo.toml:**
- Added `csv = "1.3"` dependency

**src/main.rs:**
- Added `mod io;`

**src/builtins.rs:**
- Added `builtin_file()` - Load CSV file
- Added `builtin_stdin()` - Read from stdin
- Added `builtin_save()` - Save CSV file
- Added `builtin_col()` - Extract column by name
- Added `builtin_w()` - Extract column by index
- Registered 5 new builtins
- Added 2 new tests (col_extraction, w_extraction)

---

## Demo Output

### Load CSV File

```bash
$ cat test_prices.csv
px;vol
100.0;1000
102.0;1200
101.5;800
103.0;1500
104.5;900

$ ./blisp -e '(file "test_prices.csv")'
Table[5 rows × 2 cols]
```

### Extract Column

```bash
$ ./blisp -e '(col (file "test_prices.csv") (quote px))'
Col[5 elements]

$ ./blisp -e '(w (file "test_prices.csv") 0)'
Col[5 elements]
```

### Compute Log Returns (like clispi!)

```bash
$ ./blisp -e '
(let* ((data (file "test_prices.csv"))
       (px (col data (quote px)))
       (returns (dlog px 1)))
  returns)'
Col[5 elements]
```

### Full Pipeline

```bash
$ ./blisp -e '
(progn
  (defparameter data (file "test_prices.csv"))
  (defparameter px (col data (quote px)))
  (defparameter returns (dlog px 1))
  (defparameter annual (* returns 252))
  (print "Prices loaded")
  (print "Returns computed")
  annual)'
"Prices loaded"
"Returns computed"
Col[5 elements]
```

### Read from stdin

```bash
$ cat test_prices.csv | ./blisp -e '
(let* ((data (stdin))
       (px (col data (quote px))))
  (dlog px 1))'
Col[5 elements]
```

**This is exactly how clispi works!** 🎉

---

## Test Results

**Total: 64/64 tests passing** (+4 new tests)

### New tests (4):
- **io.rs** (2 tests):
  - test_parse_csv_simple ✅
  - test_save_and_load_csv ✅

- **builtins.rs** (2 tests):
  - test_col_extraction ✅
  - test_w_extraction ✅

### Previous tests (60 still passing):
- ast.rs: 2 tests
- reader.rs: 6 tests
- value.rs: 9 tests
- env.rs: 7 tests
- runtime.rs: 9 tests
- eval.rs: 14 tests
- builtins.rs: 13 tests

---

## Key Implementation Details

### CSV Parsing with `csv` crate

```rust
use csv;

fn parse_csv(content: &str, interner: &mut Interner) -> Result<Value, String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(b';')  // Semicolon like clispi/darqt format
        .from_reader(content.as_bytes());

    // Read headers
    let headers = reader.headers()?;
    let column_names: Vec<String> = headers.iter()
        .map(|s| s.to_string())
        .collect();

    // Read data rows
    let mut column_data: Vec<Vec<f64>> = vec![Vec::new(); num_cols];
    for record in reader.records() {
        for (i, field) in record?.iter().enumerate() {
            let value: f64 = field.trim().parse()?;
            column_data[i].push(value);
        }
    }

    // Build Table
    let mut table = Table::new();
    for (i, name) in column_names.iter().enumerate() {
        let sym = interner.intern(name);
        let col = blawktrust::Column::new_f64(column_data[i].clone());
        table.add_column(sym, col);
    }

    Ok(Value::Table(Arc::new(table)))
}
```

### File Loading

```rust
fn builtin_file(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    let filename = args[0].as_str()?;
    crate::io::load_csv(filename, &mut rt.interner)
}
```

### Stdin Reading

```rust
fn builtin_stdin(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    crate::io::load_stdin(&mut rt.interner)
}
```

### Column Extraction by Name

```rust
fn builtin_col(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    let table = args[0].as_table()?;
    let col_name_sym = match &args[1] {
        Value::Sym(id) => *id,
        Value::Str(s) => rt.interner.intern(s.as_ref()),
        _ => return Err("col expects symbol or string".to_string()),
    };

    match table.get_column(col_name_sym) {
        Some(col) => Ok(Value::Col(Arc::new(col.clone()))),
        None => Err(format!("Column '{}' not found", name)),
    }
}
```

### Column Extraction by Index

```rust
fn builtin_w(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    let table = args[0].as_table()?;
    let index = args[1].as_int()? as usize;

    if index >= table.columns.len() {
        return Err("Column index out of bounds".to_string());
    }

    let (_, col) = &table.columns[index];
    Ok(Value::Col(Arc::new(col.clone())))
}
```

---

## What This Enables

### Real Financial Data Processing

```bash
# Load gold futures
./blisp -e '(file "GC1C.csv")'

# Compute log returns
./blisp -e '
(let* ((data (file "GC1C.csv"))
       (px (col data (quote px))))
  (dlog px 1))'

# Pipeline from stdin (like clispi!)
cat GC1C.csv | ./blisp -e '
(let* ((data (stdin))
       (px (w data 1)))
  (dlog px 1))'
```

### Starting to Match clispi Workflows

**clispi (from lastcode_clispi.sh):**
```bash
cgrep RAW_FUT_PRC.csv ^GC1.*C > GC1C.csv
```

**blisp (now possible!):**
```bash
# Load the extracted data
./blisp -e '
(let* ((gc (file "GC1C.csv"))
       (px (col gc (quote px)))
       (returns (dlog px 1))
       (annual (* returns 252)))
  annual)'
```

**We can now process real CSV data!** 🚀

---

## Comparison with clispi

### What works now:

| Operation | clispi | blisp | Status |
|-----------|--------|-------|--------|
| Load CSV | `(file "GC1C.csv")` | `(file "GC1C.csv")` | ✅ |
| Read stdin | `(stdin)` | `(stdin)` | ✅ |
| Extract column | `(w5 data)` | `(w data 5)` | ✅ |
| Log returns | `(dlog px 1)` | `(dlog px 1)` | ✅ |
| Shift | `(shift px 2)` | `(shift px 2)` | ✅ |
| Arithmetic | `(* returns 252)` | `(* returns 252)` | ✅ |

### What's still missing:

| Operation | clispi | blisp | Status |
|-----------|--------|-------|--------|
| Threading macro | `(-> x (dlog 1))` | Not yet | ❌ |
| Window stats | `(wzs x 25 1)` | Not yet | ❌ |
| Cross-sectional | `(x- x 1)` | Not yet | ❌ |
| Cumsum | `(cs1 x)` | Not yet | ❌ |
| Regression | `(ur x 250 5)` | Not yet | ❌ |
| Comparison | `(> x -1)` | Not yet | ❌ |
| Row mapping | `(mapr x s)` | Not yet | ❌ |
| Orientation | `(o x WENS)` | Not yet | ❌ |

**Progress: ~15-20% of clispi's functionality**

---

## Real-World Example

Let's replicate part of the GLD_NUM computation from lastcode_clispi.sh!

**clispi (simplified):**
```bash
cgrep RAW_FUT_PRC.csv ^GC1.*C > GC1C.csv
$CLISPI -e '(let* ((s ...signal...))
  (-> (file "GC1C.csv") (mapr s) (dlog) (ur 250 5) (* s) (cs1)))'
```

**blisp (what we can do now):**
```bash
./blisp -e '
(let* ((data (file "GC1C.csv"))
       (px (col data (quote px)))
       (returns (dlog px 1))
       (annual (* returns 252)))
  (print "Loaded GC1C data")
  (print "Computed log returns")
  (print "Annualized returns")
  annual)'
```

**Missing operations to complete GLD_NUM:**
- `mapr` - Row mapping/alignment
- `wzs` - Windowed z-score
- `ur` - Univariate regression
- `cs1` - Cumulative sum
- `>` - Comparison operator
- `->` - Threading macro

**But we have the foundation!** The I/O infrastructure is complete. Now we just need to add more operations.

---

## CSV Format

**blisp uses semicolon-delimited CSV (like clispi/darqt):**

```csv
px;vol;open
100.0;1000;99.5
102.0;1200;101.0
101.5;800;102.5
```

**All columns are parsed as F64 (floats) for now.**

**Future enhancements:**
- Support for date/timestamp columns
- Support for string columns
- Comma-delimited CSV
- Auto-detect delimiter

---

## Code Statistics

```
Files:          10 (+1 new: io.rs)
Lines of code:  ~2100 lines
Tests:          64/64 passing ✅
New tests:      +4 (io + builtins)
Builtins:       18 functions (+5 I/O operations)
Dependencies:   csv = "1.3" (added)
```

---

## What's Next: Advanced Operations

Now that we can load and process files, we need to add the ~100 operations that make clispi powerful:

### Step 9: Windowed Statistics (HIGH PRIORITY!)

**Operations:**
- `(wzs col window lag)` - Windowed z-score
- `(wstd col window lag)` - Windowed standard deviation
- `(wq col window lag)` - Windowed quantile rank
- `(ur col window lag)` - Univariate regression (rolling beta)
- `(locf col)` - Last observation carried forward

**These are CRITICAL for replicating GLD_NUM and other signals!**

### Step 10: Cross-Sectional Operations

**Operations:**
- `(x- col lag)` - Cross-sectional subtract (demean)
- `(cs1 col)` - Cumulative sum with lag 1
- `(zscore col)` - Z-score normalization

### Step 11: Comparison & Logic

**Operations:**
- `(> col threshold)` - Greater than
- `(<  col threshold)` - Less than
- `(>= col threshold)` - Greater than or equal
- `(<= col threshold)` - Less than or equal
- `(= col value)` - Equal

### Step 12: Table Operations

**Operations:**
- `(mapr table signal)` - Row mapping/alignment
- `(join table1 table2)` - Join tables
- `(chop col min max)` - Winsorize (clip values)

### Step 13: Macros & Utilities

**Macros:**
- `(-> x f1 f2 f3)` - Threading macro (pipeline)
- `(defmacro ...)` - Macro definition

**Utilities:**
- `(o col orientation)` - Change orientation (transpose)
- `(sum col)` - Sum values

---

## Progress: 8/15 Steps Complete

| Step | Status | Description |
|------|--------|-------------|
| 1 | ✅ | Reader + AST + Symbol Interner |
| 2 | ✅ | Environments (Lexical + Global) |
| 3 | ✅ | Evaluator (Execute Lisp!) |
| 4 | ✅ | Value Types (Col/Table) |
| 5 | ✅ | Builtin Registry (+, -, *, /) |
| 6 | ✅ | Column Operations (dlog, shift, diff) |
| 7 | ✅ | CLI (clispi-style -e flag) |
| 8 | ✅ | **File I/O (CSV load/save)** ← JUST FINISHED |
| 9 | 🔲 | Windowed Statistics (wzs, ur, wstd) |
| 10 | 🔲 | Cross-Sectional Ops (x-, cs1, zscore) |
| 11 | 🔲 | Comparison & Logic (>, <, >=, <=) |
| 12 | 🔲 | Table Operations (mapr, join) |
| 13 | 🔲 | Macros (-> threading macro) |
| 14 | 🔲 | Orientation (o, transpose) |
| 15 | 🔲 | Comprehensive Testing |

---

## Celebrating Step 8 🎉

**blisp can now process real CSV files!**

- ✅ Load CSV from files: `(file "GC1C.csv")`
- ✅ Read CSV from stdin: `(stdin)`
- ✅ Extract columns: `(col table 'px)` or `(w table 0)`
- ✅ Process data: `(dlog px 1)`
- ✅ Chain operations: `(* (dlog px 1) 252)`
- ✅ Save results: `(save "output.csv" table)`

**We can now replicate simple clispi workflows!**

What remains to match clispi:
- ~100 more operations (wzs, ur, cs1, mapr, etc.)
- Threading macro `(->)`
- More advanced features

But the foundation is complete! We have:
- Fast blawktrust kernels ✅
- CLI like clispi ✅
- File I/O ✅
- Basic operations ✅

---

## Quick Commands

```bash
cd /home/ubuntu/blisp

# Create test CSV
cat > test.csv <<EOF
px;vol
100.0;1000
102.0;1200
101.5;800
EOF

# Load and process
./blisp -e '(file "test.csv")'
./blisp -e '(col (file "test.csv") (quote px))'
./blisp -e '(dlog (col (file "test.csv") (quote px)) 1)'

# From stdin
cat test.csv | ./blisp -e '(stdin)'
cat test.csv | ./blisp -e '(dlog (col (stdin) (quote px)) 1)'

# Run tests
cargo test
```

---

**Status:** Step 8/15 complete ✅
**Next:** Step 9 - Windowed Statistics (wzs, wstd, wq, ur, locf)
**Progress:** File I/O complete! Can now process real data! 🚀

---

**This is a MAJOR milestone!** 🎯

blisp can now load real CSV files and process financial data. The infrastructure is complete. Now we just need to add the ~100 operations that make clispi powerful for quantitative finance.

**We're ready to start replicating darqt workflows!** 📊
