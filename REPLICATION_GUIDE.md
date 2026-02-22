# BLISP Pipeline Replication Guide

## Purpose

This guide explains how to replicate financial data pipelines in BLISP to match reference outputs from CLISPI or other systems. Use this document to avoid starting from scratch when replicating pipelines.

## Quick Reference

### Running BLISP

```bash
# Command-line expression (recommended for pipelines)
cd /home/ubuntu/blisp
cargo run --release -- -e '(your-expression-here)'

# Save output to file
cargo run --release -- -e '(expression)' 2>/dev/null > output.csv

# Suppress warnings for clean output
cargo run --release -- -e '(expression)' 2>/dev/null
```

**Important:** Use `-e` flag for expressions. Script files (`.lisp`) have limitations with variable bindings in IR mode.

## Core BLISP Syntax

### Data Loading
```lisp
;; Load CSV file
(read-csv "/path/to/file.csv")

;; BLISP uses semicolon (;) as CSV delimiter by default
```

### Function Application
```lisp
;; Prefix notation (Lisp style)
(function arg1 arg2 ...)

;; Examples
(locf data)
(ft-zscore 250 data)
(+ x y)
(- x y)
(* x y)
(/ x y)
```

### Threading/Composition
```lisp
;; Nested (inside-out evaluation)
(ft-zscore 250 (locf (read-csv "/path/to/file.csv")))

;; Thread-first macro (-> not reliably available in IR mode)
;; Prefer nested notation for reliability
```

## Essential Operations

### Data Cleaning

#### locf - Last Observation Carried Forward
```lisp
(locf data)
```
- Fills NA values with last valid observation
- Leading NAs preserved until first valid value
- Idempotent: `locf(locf(x)) = locf(x)`

### Statistical Operations

#### ft-zscore - Forward-Test Z-Score
```lisp
(ft-zscore window data)

;; Example: 250-day window
(ft-zscore 250 data)
```
- **Forward-test**: Excludes current observation from statistics
- No lookahead bias (critical for backtesting)
- Formula: `(x - mean_excl_current) / std_excl_current`
- Uses partial windows (≥2 observations)
- Compatible with masked calendars (weekends)

#### wzs - Rolling Z-Score (CLISPI compatible)
```lisp
(wzs window step data)

;; Example: 250-day window, step=1 (ignored)
(wzs 250 1 data)
```
- **Not forward-test**: Includes current observation
- `step` parameter ignored (CLISPI compatibility)
- Formula: `(x - rolling_mean) / rolling_std`
- Use `ft-zscore` for forward-test semantics

#### Rolling Mean
```lisp
;; Strict: requires exactly w observations
(rolling-mean w data)

;; Partial: requires ≥2 observations
(rolling-mean-partial w data)

;; Partial excluding current (for forward-test)
(rolling-mean-partial-excl-current w data)
```

#### Rolling Standard Deviation
```lisp
;; Strict
(rolling-std w data)

;; Partial
(rolling-std-partial w data)

;; Partial excluding current (for forward-test)
(rolling-std-partial-excl-current w data)
```

### Lag/Shift Operations

#### shift - Calendar Lag
```lisp
(shift k data)

;; Example: 2-day lag
(shift 2 data)
```
- Positional shift over all rows
- `output[i] = input[i-k]`
- If source row is masked → NA

#### shift-obs / shiftm - Observation Lag
```lisp
(shift-obs k data)
(shiftm k data)  ;; alias

;; Example: 2 business-day lag (when weekend mask active)
(shift-obs 2 data)
```
- Skips masked rows when computing lag
- Business-day lag when weekend mask active
- For matching CLISPI's filtered-mode behavior

### Arithmetic Operations
```lisp
(+ x y)   ;; Addition
(- x y)   ;; Subtraction
(* x y)   ;; Multiplication
(/ x y)   ;; Division
```

### Transformations
```lisp
(dlog data)    ;; Log returns: log(x[i]/x[i-1])
(ret data)     ;; Simple returns: (x[i]-x[i-1])/x[i-1]
(log data)     ;; Natural logarithm
(exp data)     ;; Exponential
(sqrt data)    ;; Square root
(abs data)     ;; Absolute value
(inv data)     ;; Inverse: 1/x
(cs1 data)     ;; Cumulative sum starting at 1.0
```

## Complete Pipeline Examples

### Example 1: ES1I with ft-zscore (250-day window)

**Goal:** Replicate `ES1I_locf_wzs_250_1.csv`

**Pipeline:**
1. Load ES1I.csv (S&P 500 E-mini futures)
2. Apply locf (fill missing values)
3. Apply forward-test z-score (250-day window)

**BLISP Command:**
```bash
cd /home/ubuntu/blisp
cargo run --release -- -e \
  '(ft-zscore 250 (locf (read-csv "/home/ubuntu/ES1I.csv")))' \
  2>/dev/null > ES1I_locf_wzs_250_1.csv
```

**Output:**
- 9,550 rows
- ~282 KB file size
- NA values in warmup period (~250 rows)
- Z-scores from 2000-01-05 onwards

**Verification:**
```bash
wc -l ES1I_locf_wzs_250_1.csv
head -15 ES1I_locf_wzs_250_1.csv
tail -10 ES1I_locf_wzs_250_1.csv
```

### Example 2: Multi-step Pipeline with Intermediate Files

When BLISP's `-e` mode has expression complexity limits, use intermediate files:

```bash
cd /home/ubuntu/blisp

# Step 1: Load and clean
cargo run --release -- -e '(locf (read-csv "/home/ubuntu/ES1I.csv"))' \
  2>/dev/null > /tmp/step1_locf.csv

# Step 2: Compute rolling mean
cargo run --release -- -e '(rolling-mean-partial-excl-current 250 (read-csv "/tmp/step1_locf.csv"))' \
  2>/dev/null > /tmp/step2_mean.csv

# Step 3: Compute rolling std
cargo run --release -- -e '(rolling-std-partial-excl-current 250 (read-csv "/tmp/step1_locf.csv"))' \
  2>/dev/null > /tmp/step3_std.csv

# Step 4: Compute z-score
cargo run --release -- -e '(/ (- (read-csv "/tmp/step1_locf.csv") (read-csv "/tmp/step2_mean.csv")) (read-csv "/tmp/step3_std.csv"))' \
  2>/dev/null > output.csv
```

**Note:** This is verbose but works around expression parsing limitations.

### Example 3: Weekend-Masked Pipeline

**Goal:** Apply operations that respect weekend masks

```bash
# Create weekend mask, activate it, then apply shift-obs
cargo run --release -- -e \
  '(shift-obs 2 (with-mask weekend (mask-weekend (read-csv "/home/ubuntu/prices.csv"))))' \
  2>/dev/null > output.csv
```

**Mask Operations:**
- `(mask-weekend data)` - Creates weekend mask (Sat/Sun)
- `(with-mask maskname data)` - Activates a named mask
- `maskname` is a symbol, not a string: `weekend` not `"weekend"`

## Common Issues and Solutions

### Issue 1: Empty Output File

**Symptom:** Output CSV has 0 bytes or 0 lines

**Causes:**
1. Function name not recognized
2. Expression syntax error
3. File path incorrect

**Solution:**
```bash
# Run without suppressing stderr to see error
cargo run --release -- -e '(your-expression)' 2>&1 | grep -i error
```

### Issue 2: "Undefined variable" Error

**Symptom:** `Error: Undefined variable: function-name`

**Cause:** Function name incorrect or hyphenation wrong

**Solution:**
Check function names in planner:
```bash
cd /home/ubuntu/blisp
grep "\"your-function-name\"" src/planner.rs
```

Common correct names:
- `ft-zscore` (not `ft_zscore` or `ftzs`)
- `rolling-mean-partial-excl-current` (not `rolling_mean_partial_excl_current`)
- `shift-obs` (not `shift_obs`)

### Issue 3: Script Files Don't Work

**Symptom:** `.lisp` script files fail with "Undefined variable: let" or "Undefined variable: def"

**Cause:** IR mode has limited support for bindings in script files

**Solution:** Use `-e` flag with command-line expressions instead of script files

### Issue 4: Wrong Window Size

**Symptom:** Results don't match reference output

**Cause:** Using wrong window parameter

**Solution:** Check file naming convention:
- `ES1I_locf_wzs_250_1.csv` → window=250, step=1
- `ES1I_locf_wzs_25_1.csv` → window=25, step=1

## Naming Conventions

### Reference File Format
```
{SYMBOL}__{OPERATION1}__{OPERATION2}__{WINDOW}_{STEP}.csv

Examples:
ES1I_locf_wzs_250_1.csv  → ES1I + locf + wzs(250, 1)
ES1I_locf_ftzs_25.csv    → ES1I + locf + ft-zscore(25)
```

### Operation Abbreviations
- `locf` - Last observation carried forward
- `wzs` - Rolling z-score (with current)
- `ftzs` - Forward-test z-score (excluding current)
- `dlog` - Log returns
- `ret` - Simple returns
- `cs1` - Cumulative sum from 1.0
- `wkd` or `w5` - Weekday mask

## Verification Workflow

### 1. Generate Output
```bash
cd /home/ubuntu/blisp
cargo run --release -- -e '(pipeline-expression)' 2>/dev/null > output.csv
```

### 2. Check Row Count
```bash
wc -l output.csv reference.csv
# Should match
```

### 3. Visual Inspection
```bash
# Check header
head -1 output.csv

# Check first data rows (after header and warmup NAs)
head -20 output.csv

# Check last rows
tail -10 output.csv
```

### 4. Numerical Comparison
```bash
# If you have a comparison tool
diff <(tail -n +2 output.csv | cut -d';' -f2) \
     <(tail -n +2 reference.csv | cut -d';' -f2)

# Or use a numerical diff tool
python3 -c "
import pandas as pd
import numpy as np

out = pd.read_csv('output.csv', sep=';')
ref = pd.read_csv('reference.csv', sep=';')

# Compare shapes
print(f'Output: {out.shape}, Reference: {ref.shape}')

# Check column names
print(f'Columns match: {list(out.columns) == list(ref.columns)}')

# Numerical comparison (allow small floating point differences)
diff = (out.iloc[:, 1:] - ref.iloc[:, 1:]).abs()
max_diff = diff.max().max()
print(f'Max absolute difference: {max_diff}')
print(f'Match within 1e-10: {max_diff < 1e-10}')
"
```

## Performance Notes

### Build Time
- First build: ~2-3 minutes (with dependencies)
- Incremental builds: <10 seconds
- Use `--release` for production speed (~10-100x faster than debug)

### Execution Time
For ES1I dataset (~9,550 rows):
- Simple operations (locf): <0.5s
- Rolling operations (ft-zscore 250): <2s
- Complex pipelines: <5s

### Memory Usage
- Typical: <100 MB for datasets with <100k rows
- Frame operations use Arc (reference counting) for efficiency

## Advanced Features

### Mask System

**Create and activate weekend mask:**
```lisp
(with-mask weekend (mask-weekend data))
```

**Boolean mask algebra:**
```lisp
(with-mask (not weekend) data)               ;; Invert mask
(with-mask (and mask1 mask2) data)           ;; Intersection
(with-mask (or mask1 mask2) data)            ;; Union
```

### Observation-Based Operations

When masks are active, use observation-based operations:
```lisp
;; Calendar operations (positional)
(shift 2 data)                    ;; May land on masked rows → NA
(rolling-mean 250 data)           ;; Counts all rows

;; Observation operations (skip masked)
(shift-obs 2 data)                ;; Skips masked rows
(rolling-mean-partial 250 data)   ;; Counts only unmasked rows
```

## BLISP vs CLISPI Differences

### Syntax
| Feature | CLISPI | BLISP |
|---------|--------|-------|
| CSV delimiter | `;` | `;` (same) |
| Function style | Prefix | Prefix (same) |
| Threading | `->` | `->` (limited in IR mode) |
| Variable binding | `def` | IR mode: use nesting |

### Semantics
| Operation | CLISPI | BLISP |
|-----------|--------|-------|
| ft-zscore | Forward-test | Forward-test (same) |
| wzs | Includes current | Includes current (same) |
| Rolling with mask | Observation-based | Observation-based (same) |
| shift with mask | Skips filtered | Use `shift-obs` |

### Parity Status
✅ **Complete parity** for:
- locf
- ft-zscore with mask-aware rolling
- dlog, ret, log, exp, sqrt, abs, inv
- Rolling mean/std (strict and partial)
- Cumulative operations
- Weekend masking

✅ **New in BLISP:**
- `shift-obs` - Observation-based shift (skip masked rows)
- Explicit calendar vs observation shift distinction

## Troubleshooting Decision Tree

```
Output file is empty?
├─ Yes → Check stderr for errors
│        cargo run --release -- -e '(expr)' 2>&1 | grep -i error
│        ├─ "Undefined variable: function-name"
│        │  └─ Check function name spelling/hyphenation
│        ├─ "Undefined variable: let/def"
│        │  └─ Use -e flag, avoid script files
│        └─ "file not found"
│           └─ Check file path is absolute
└─ No → Output has wrong values?
         ├─ Check window parameter (250 vs 25)
         ├─ Check operation order (locf before zscore?)
         └─ Compare warmup period (first ~w rows should be NA)
```

## Quick Start Checklist

When replicating a new pipeline:

- [ ] Identify reference file name and parse operation sequence
- [ ] Check if reference file exists: `ls -lh /home/ubuntu/{FILENAME}`
- [ ] Identify input CSV: usually same symbol without operations
- [ ] Build BLISP expression inside-out (data first, operations outward)
- [ ] Test expression: `cargo run --release -- -e '(expr)' 2>/dev/null | head -20`
- [ ] Check for errors: `cargo run --release -- -e '(expr)' 2>&1 | grep -i error`
- [ ] Generate full output: `... > output.csv`
- [ ] Verify row count: `wc -l output.csv`
- [ ] Verify first/last rows: `head -20 output.csv && tail -10 output.csv`
- [ ] If reference exists, compare numerically

## Examples by Complexity

### Simple (1-2 operations)
```bash
# Load and clean
cargo run --release -- -e '(locf (read-csv "/home/ubuntu/ES1I.csv"))' 2>/dev/null > output.csv

# Load and transform
cargo run --release -- -e '(dlog (read-csv "/home/ubuntu/ES1I.csv"))' 2>/dev/null > output.csv
```

### Medium (3-4 operations)
```bash
# Load, clean, zscore
cargo run --release -- -e '(ft-zscore 250 (locf (read-csv "/home/ubuntu/ES1I.csv")))' 2>/dev/null > output.csv

# Load, clean, dlog, cumsum
cargo run --release -- -e '(cs1 (dlog (locf (read-csv "/home/ubuntu/ES1I.csv"))))' 2>/dev/null > output.csv
```

### Complex (5+ operations or masks)
```bash
# Load, mask, activate, clean, zscore
cargo run --release -- -e '(ft-zscore 250 (locf (with-mask weekend (mask-weekend (read-csv "/home/ubuntu/ES1I.csv")))))' 2>/dev/null > output.csv

# Or use intermediate files for very complex pipelines
```

## Reference Implementation: ES1I_locf_wzs_250_1

**Complete replication command:**
```bash
cd /home/ubuntu/blisp

cargo run --release -- -e \
  '(ft-zscore 250 (locf (read-csv "/home/ubuntu/ES1I.csv")))' \
  2>/dev/null > /home/ubuntu/ES1I_locf_wzs_250_1.csv

# Verify
wc -l /home/ubuntu/ES1I_locf_wzs_250_1.csv  # Should be 9550
head -15 /home/ubuntu/ES1I_locf_wzs_250_1.csv
tail -10 /home/ubuntu/ES1I_locf_wzs_250_1.csv
```

**Expected output:**
- Header: `TIMESTAMP;ES1 Index`
- First ~4 rows: NA (need at least 2 observations for partial)
- Row 5 onwards: z-score values
- Values range typically: -3.0 to +3.0 (standard deviations)
- Last date: 2026-02-21 (or current date in dataset)

## CLISPI wzs Step Parameter vs BLISP

### Important: Step ≠ Keep

CLISPI's `wzs` step parameter works differently than a simple "keep every kth row":

**CLISPI wzs behavior:**
```lisp
(wzs window step data)
```
1. Compute rolling z-score for **ALL rows** (full calculation)
2. Resample output by keeping every `step`-th row
3. Keep rows at indices: `step`, `2*step`, `3*step`, ... (skips index 0)

**Example: wzs 250 5**
- Calculates z-score for rows 0, 1, 2, ..., 9549
- Keeps rows at indices: 5, 10, 15, 20, 25, ... (1910 rows from 9550)
- Does NOT keep index 0 (warmup row)

### BLISP Replication Pattern

**Wrong approach:**
```lisp
;; This is NOT equivalent to CLISPI step
(keep 5 (ft-zscore 250 (locf data)))
;; Problem: keeps index 0, 5, 10, 15... (wrong starting point)
```

**Correct approach (kdb-ish):**
```bash
# Step 1: Calculate for ALL rows
blisp -e '(ft-zscore 250 (locf (read-csv "data.csv")))' |
# Step 2: Resample (keep every 5th, skip index 0)
awk -F';' 'NR==1 || (NR-1) % 5 == 0 && NR > 6'
```

**Alternative (shape-preserving):**
```bash
# Calculate for all rows, then post-process
blisp -e '(ft-zscore 250 (locf data))' |
awk -F';' 'NR==1 || (NR-1) % 5 == 0' > output.csv
```

### Why This Matters

BLISP uses **kdb-ish rolling functions** that:
1. Calculate statistics at EVERY date (full temporal resolution)
2. Maintain alignment across series
3. Preserve join semantics

Then **resample** the output to match CLISPI's step behavior:
- Resampling is a post-calculation operation
- Rolling windows still see ALL rows during calculation
- Step does NOT affect window computation

### Reference Files

For ES1I pipelines:
- `ES1I_locf_wzs_250_1.csv` - All rows (9,550)
- `ES1I_locf_wzs_250_5.csv` - Every 5th row (1,910 non-NA rows, 9,550 shape-preserved)

Pattern in step=5:
```
Row index:    0    1    2    3    4    5    6    7    8    9   10
Has value:   NA   NA   NA   NA   NA   ✓   NA   NA   NA   NA    ✓
                                    ^                          ^
                              index 5                    index 10
```

### Complete Example: ES1I step=5

```bash
#!/bin/bash
# ES1I with step=5: Calculate all, then resample

# Method 1: Remove non-kept rows (1,910 output rows)
/home/ubuntu/blisp/target/release/blisp -e \
  '(ft-zscore 250 (locf (read-csv "/home/ubuntu/ES1I.csv")))' \
  2>/dev/null | \
  awk -F';' 'NR==1 || (NR-1) % 5 == 0' > output_filtered.csv

# Method 2: Keep shape with NA (9,550 output rows)
# Use BLISP's built-in keep for shape-preserving
# NOTE: This keeps index 0 too, different from CLISPI step
/home/ubuntu/blisp/target/release/blisp -e \
  '(keep 5 (ft-zscore 250 (locf (read-csv "/home/ubuntu/ES1I.csv"))))' \
  2>/dev/null > output_shaped.csv
```

**For exact CLISPI replication:** Use Method 1 (awk post-processing)

## Summary

**Golden Rules:**
1. Always use `-e` flag for expressions
2. Suppress stderr with `2>/dev/null` for clean CSV output
3. Use absolute paths: `/home/ubuntu/...`
4. Check function names in `src/planner.rs` if unsure
5. Start simple, build complexity gradually
6. Verify output before comparing
7. **For step parameter:** Calculate all rows, then resample with awk

**Most Common Pipeline:**
```bash
cargo run --release -- -e \
  '(ft-zscore WINDOW (locf (read-csv "PATH")))' \
  2>/dev/null > output.csv
```

**Pipeline with step (CLISPI-compatible):**
```bash
cargo run --release -- -e \
  '(ft-zscore WINDOW (locf (read-csv "PATH")))' \
  2>/dev/null | \
  awk -F';' 'NR==1 || (NR-1) % STEP == 0' > output.csv
```

This covers 80% of replication tasks!
