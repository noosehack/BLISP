# blisp - Where We Are Now (2026-02-17)

Quick reference for current blisp capabilities after Step 8.

---

## Quick Start

```bash
cd /home/ubuntu/blisp

# Build (if needed)
cargo build --release
cp target/release/blisp .

# Simple test
./blisp -e "(+ 1 2)"

# Load CSV and compute
./blisp -e '(file "test_prices.csv")'
```

---

## What Works Now

### ✅ CLI (like clispi)

```bash
# Execute expression
./blisp -e "(+ 1 2)"

# Execute script file
./blisp script.lisp

# Pipe through stdin
cat data.csv | ./blisp -e "(stdin)"
```

### ✅ Arithmetic (18 operations total)

**Basic:**
- `(+ a b)` - Add
- `(- a b)` - Subtract
- `(* a b)` - Multiply
- `(/ a b)` - Divide

**Math:**
- `(log x)` - Natural logarithm
- `(exp x)` - Exponential
- `(abs x)` - Absolute value

**Works on scalars and columns!**

### ✅ Column Operations (Fast blawktrust kernels)

- `(dlog col lag)` - Log returns (1.89× faster than C++)
- `(shift col lag)` - Lag/lead values
- `(diff col lag)` - Differences

### ✅ File I/O

- `(file "filename.csv")` - Load CSV file
- `(stdin)` - Read from stdin
- `(save "file.csv" table)` - Save CSV
- `(col table 'colname)` - Extract column by name
- `(w table index)` - Extract column by index

### ✅ Utility

- `(print x)` - Print value
- `(type-of x)` - Get type
- `(len x)` - Get length

### ✅ Language Features

- Variables: `(defparameter x 10)`
- Update: `(setf x 20)`
- Local scope: `(let* ((x 1) (y 2)) (+ x y))`
- Conditionals: `(if condition then else)`
- Sequential: `(progn expr1 expr2 expr3)`
- Quote: `'foo` or `(quote foo)`

---

## Example Workflows

### Load and Process CSV

```bash
# Create test data
cat > prices.csv <<EOF
px;vol
100.0;1000
102.0;1200
101.5;800
103.0;1500
104.5;900
EOF

# Load and view
./blisp -e '(file "prices.csv")'
# => Table[5 rows × 2 cols]

# Extract column
./blisp -e '(col (file "prices.csv") (quote px))'
# => Col[5 elements]

# Compute log returns
./blisp -e '
(let* ((data (file "prices.csv"))
       (px (col data (quote px))))
  (dlog px 1))'
# => Col[5 elements]

# Annualize returns
./blisp -e '
(let* ((data (file "prices.csv"))
       (px (col data (quote px)))
       (returns (dlog px 1)))
  (* returns 252))'
# => Col[5 elements]
```

### Pipeline from stdin

```bash
cat prices.csv | ./blisp -e '
(let* ((data (stdin))
       (px (w data 0))
       (returns (dlog px 1)))
  returns)'
```

### Multiple Operations

```bash
./blisp -e '
(progn
  (defparameter data (file "prices.csv"))
  (defparameter px (col data (quote px)))
  (defparameter vol (col data (quote vol)))
  (defparameter r (dlog px 1))
  (defparameter scaled (* r 252))
  (print "Loaded data")
  (print "Computed returns")
  scaled)'
```

### Use Variables

```bash
./blisp -e '
(let* ((x 10)
       (y 20)
       (z (+ x y)))
  (* z 2))'
# => 60
```

---

## What Still Doesn't Work

### ❌ Missing ~100 Operations

**Windowed Statistics (Step 9):**
- `wzs` - Windowed z-score
- `wstd` - Windowed standard deviation
- `wq` - Windowed quantile rank
- `ur` - Univariate regression (rolling beta)
- `locf` - Last observation carried forward

**Cross-Sectional (Step 10):**
- `x-` - Cross-sectional subtract (demean)
- `cs1` - Cumulative sum
- `zscore` - Z-score normalization

**Comparisons (Step 11):**
- `>` `<` `>=` `<=` `=` - Comparison operators

**Table Operations (Step 12):**
- `mapr` - Row mapping/alignment
- `join` - Join tables
- `chop` - Winsorize

**Macros (Step 13):**
- `->` - Threading macro (pipeline)

**Orientation (Step 14):**
- `o` - Change orientation

### ❌ Current Limitations

- **CSV format:** Only semicolon-delimited (`;`)
- **Column types:** Only F64 (float) columns
- **Date columns:** Can't parse date strings yet
- **Display:** Column output shows "Col[N elements]" without values
- **Error messages:** Could be more helpful

---

## CSV Format

**blisp expects semicolon-delimited CSV:**

```csv
px;vol;open
100.0;1000;99.5
102.0;1200;101.0
101.5;800;102.5
```

**All columns must be numeric (F64) for now.**

---

## Quick Reference: All Operations

### Arithmetic (4)
- `+` `-` `*` `/`

### Math (3)
- `log` `exp` `abs`

### Column Operations (3)
- `dlog` `shift` `diff`

### File I/O (5)
- `file` `stdin` `save` `col` `w`

### Utility (3)
- `print` `type-of` `len`

### Special Forms (6)
- `quote` `progn` `if` `let*` `defparameter` `setf`

**Total: 24 operations**

---

## Performance

**blawktrust kernels are 1.89× faster than C++:**
- `dlog` on 1M elements: 15.51 ms (vs 29.33 ms C++)
- Zero FFI overhead
- SIMD vectorization

---

## Testing Your Workflow

To test with your real data (like GC1C.csv from lastcode_clispi.sh):

```bash
# If you have GC1C.csv with semicolon delimiter
./blisp -e '
(let* ((data (file "GC1C.csv"))
       (px (col data (quote px)))
       (returns (dlog px 1))
       (annual (* returns 252)))
  (print "Loaded GC1C.csv")
  (print "Computed log returns")
  annual)'
```

**What you CAN do now:**
- Load CSV files ✅
- Extract columns ✅
- Compute log returns ✅
- Arithmetic operations ✅

**What you CANNOT do yet:**
- Windowed z-score (wzs)
- Rolling regression (ur)
- Row mapping (mapr)
- Cross-sectional operations (x-, cs1)
- Comparisons (>, <)
- Threading macro (->)

---

## Comparison with Your clispi Workflow

**Your clispi (from lastcode_clispi.sh line 26):**
```bash
$CLISPI -e '(let* ((s (-> (stdin) (w5) (dlog) (x- 1) (cs1) (wzs 25 1) (> -1) (shift 2))))
  (-> (file "GC1C.csv") (mapr s) (dlog) (ur 250 5) (* s) (cs1)))'
```

**What blisp can do now:**
```bash
./blisp -e '(let* ((data (file "GC1C.csv"))
                   (px (w data 5))
                   (returns (dlog px 1))
                   (shifted (shift returns 2))
                   (scaled (* returns 252)))
  scaled)'
```

**Missing for full parity:**
- `->` threading macro
- `x-` cross-sectional subtract
- `cs1` cumulative sum
- `wzs` windowed z-score
- `>` comparison
- `mapr` row mapping
- `ur` univariate regression

**But you can already do useful work!**

---

## Troubleshooting

### "Error reading file"
- Check file exists
- Check CSV format (semicolon-delimited)
- Check all columns are numeric

### "Column 'X' not found"
- Use `(quote colname)` not just `colname`
- Or use `(w table index)` with numeric index

### "Undefined variable"
- Use `(defparameter x val)` to define
- Or use `(let* ((x val)) ...)`

### Binary not found
```bash
cd /home/ubuntu/blisp
cargo build --release
cp target/release/blisp .
```

---

## Next Steps

When you're ready to add more operations:

**Priority 1: Windowed Statistics (Step 9)**
- `wzs`, `wstd`, `wq`, `ur`, `locf`
- These are essential for your trading signals

**Priority 2: Cross-Sectional Ops (Step 10)**
- `x-`, `cs1`, `zscore`
- Needed for GLD_NUM and other signals

**Priority 3: Comparisons (Step 11)**
- `>`, `<`, `>=`, `<=`, `=`
- For filtering and boolean masks

**Priority 4: Table Operations (Step 12)**
- `mapr` - Critical for row alignment
- `join`, `chop`

**Priority 5: Macros (Step 13)**
- `->` threading macro
- Makes code much more readable

---

## Play Time! 🎮

Try these experiments:

```bash
# Nested arithmetic
./blisp -e "(* (+ 1 2) (+ 3 4))"

# Variables and computation
./blisp -e "(let* ((x 100) (y 200)) (/ (+ x y) 2))"

# Load your actual data
./blisp -e "(file \"YOUR_FILE.csv\")"

# Extract and process
./blisp -e "(dlog (col (file \"YOUR_FILE.csv\") (quote px)) 1)"

# Full pipeline
./blisp -e "
(progn
  (defparameter data (file \"YOUR_FILE.csv\"))
  (defparameter px (col data (quote px)))
  (defparameter r (dlog px 1))
  (defparameter annual (* r 252))
  (print \"Done!\")
  annual)"
```

---

**Have fun experimenting!** 🚀

When you're ready, we can add the missing operations to get closer to full clispi parity.

**Current status: 8/15 steps complete (53%)**
**Can process real CSV data with basic operations!**
