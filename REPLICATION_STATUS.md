# BLISP Replication Status - Current State & Next Steps

**Date:** 2026-02-22
**Status:** Partial success - step=1 works, step>1 broken

---

## What Works ✅

### ES1I_locf_wzs_250_1.csv - EXACT REPLICATION

**Reference command:**
```bash
./darqt_test ES1I.csv "locf|wzs 250 1"
```

**BLISP replication:**
```bash
/home/ubuntu/generate_es1i_250.sh
```

**Pipeline:**
```lisp
(ft-zscore 250 (locf (read-csv "/home/ubuntu/ES1I.csv")))
```

**Results:**
- ✅ Exact numerical match
- ✅ All 9,550 rows
- ✅ All z-score values identical
- ⚡ 88ms execution time

**Why it works:**
- CLISPI's `wzs window 1` with step=1 means calculate for ALL rows
- BLISP's `ft-zscore` does exactly this (forward-test z-score for all rows)
- No sampling/keep logic needed

---

## What Doesn't Work ❌

### ES1I_locf_wzs_250_5.csv - WRONG VALUES

**Reference command:**
```bash
./darqt_test ES1I.csv "locf|w5|wzs 250 5"
```

**Current BLISP attempts:**

#### Attempt 1: With keep(5)
```bash
/home/ubuntu/blisp/target/release/blisp -e \
  '(keep 5 (ft-zscore 250 (w5 (locf (read-csv "/home/ubuntu/ES1I.csv")))))' \
  2>/dev/null | awk -F';' 'NR==1 || $2 != "NA"'
```
**Result:** Wrong values, different z-scores

#### Attempt 2: Without keep(5)
```bash
/home/ubuntu/blisp/target/release/blisp -e \
  '(ft-zscore 250 (w5 (locf (read-csv "/home/ubuntu/ES1I.csv"))))' \
  2>/dev/null | awk -F';' 'NR==1 || $2 != "NA"'
```
**Result:** Wrong values, different z-scores

#### Attempt 3: Using wzs instead of ft-zscore
```bash
/home/ubuntu/blisp/target/release/blisp -e \
  '(wzs 250 5 (w5 (locf (read-csv "/home/ubuntu/ES1I.csv"))))' \
  2>/dev/null | awk -F';' 'NR==1 || $2 != "NA"'
```
**Result:** Wrong values, different z-scores

---

## The Problem: step > 1 Semantics

### Reference File Analysis

**ES1I_locf_wzs_250_5.csv structure:**
- Total rows: 6,820 (weekdays only, weekends removed)
- Non-NA rows: 6,566
- NA rows: 254 (warmup period)

**Key observations:**
1. Weekends are FILTERED OUT (not masked) - reduces from 9,550 to ~6,820 rows
2. First ~254 rows are NA (warmup period - much longer than BLISP's)
3. Z-score values are DIFFERENT from BLISP output

**Example value comparison:**
```
Date         Reference        BLISP (ft-zscore)    BLISP (wzs)
2000-01-03   NA               (filtered)           (filtered)
2000-01-04   NA               -0.707               -0.707
2000-01-05   NA               -0.549               -0.549
...
(first non-NA in reference appears much later)
```

### The Mystery: What Does step=5 Actually Do?

**Hypotheses to test tomorrow:**

1. **Step affects rolling window calculation itself?**
   - Maybe wzs 250 5 uses every 5th observation in the 250-window?
   - Not "calculate all then sample", but "calculate using sampled window"?

2. **Step affects warmup period?**
   - Reference has ~254 NA rows (warmup)
   - BLISP has ~4 NA rows (warmup)
   - Maybe step changes when z-scores start being calculated?

3. **Step changes the calculation method?**
   - Different rolling window semantics?
   - Different NA handling?
   - Different variance calculation?

4. **Step is about observation frequency?**
   - Calculate rolling stats using observations that are step-apart?
   - Like "sample every 5th point for the rolling window"?

---

## Key Files & Scripts

### Working (step=1)
- **Script:** `/home/ubuntu/generate_es1i_250.sh`
- **Output:** `/home/ubuntu/ES1I_locf_wzs_250_1.csv` (9,550 rows)
- **Pipeline:** `ES1I.csv → locf → ft-zscore(250)`

### Broken (step=5)
- **Scripts tried:**
  - `/home/ubuntu/generate_es1i_250_5.sh`
  - `/home/ubuntu/generate_es1i_250_5_with_w5.sh`
  - `/home/ubuntu/test_wzs_regular.sh`
  - `/home/ubuntu/test_w5_only.sh`
- **Reference:** `/home/ubuntu/ES1I_locf_wzs_250_5.csv` (6,820 rows)
- **Pipeline:** `ES1I.csv → locf → w5 → wzs(250, 5)`

### Test Data
- **Original:** `/home/ubuntu/ES1I.csv` (9,550 rows, 189 KB)
- **Large test:** `/home/ubuntu/ES1I_1M.csv` (1M rows, 22 MB)

---

## BLISP Implementation Status

### Implemented Operations
- ✅ `locf` - Last observation carried forward
- ✅ `ft-zscore` - Forward-test z-score (excludes current)
- ✅ `wzs` - Regular z-score (includes current)
- ✅ `w5` - Weekend mask (sets Sat/Sun to NA)
- ✅ `keep` - Keep every k-th row (shape-preserving, fills with NA)
- ✅ `shift` - Calendar lag
- ✅ `shift-obs` - Observation lag (mask-aware)

### The Keep Problem

**Current implementation:**
```rust
// keep(5) keeps indices 0, 5, 10, 15, 20...
// Other rows filled with NA
fn keep_column(col: &Column, k: usize) -> Column {
    data.iter().enumerate()
        .map(|(i, &val)| if i % k == 0 { val } else { f64::NAN })
        .collect()
}
```

**What's wrong:**
- This is SHAPE-PRESERVING (keeps all rows, fills non-kept with NA)
- CLISPI's step might work differently
- Values don't match reference even with correct shape

**Possible fix directions:**
1. Step affects the rolling window calculation, not just output sampling
2. Need to understand CLISPI/darqt wzs step semantics exactly
3. May need to sample observations BEFORE rolling calculation

---

## Documentation Created

1. **`/home/ubuntu/blisp/REPLICATION_GUIDE.md`** (pushed to GitHub)
   - Complete guide for replicating pipelines
   - Function reference
   - Common issues and solutions
   - CLISPI step vs BLISP keep explanation (partially wrong)

2. **`/home/ubuntu/blisp/SHIFT_OBS_IMPLEMENTATION.md`**
   - shift vs shift-obs semantics
   - Mask-aware operations

3. **This file:** `/home/ubuntu/REPLICATION_STATUS.md`
   - Current state summary
   - What works, what doesn't
   - Next steps

---

## What We Need Tomorrow

### 1. Understand CLISPI's step Parameter

**Action:** Investigate what `wzs window step` actually does when step > 1

**Questions to answer:**
- Does step affect the rolling window calculation?
- Does step affect which observations are used in the window?
- Is it pre-sampling before rolling, or post-sampling after rolling?
- Why are the z-score values different (not just the sampling)?

**Method:**
- Run darqt with different step values
- Compare intermediate outputs
- Trace through what happens to the rolling window
- Check if it's sampling observations vs sampling output

### 2. Replicate ES1I_locf_wzs_250_5.csv Exactly

**Goal:** Match the reference file exactly (6,820 rows, same z-score values)

**Current mismatch:**
- ❌ Different warmup period (254 NAs vs ~4 NAs)
- ❌ Different z-score values
- ❌ Different calculation method

**To implement:**
1. Figure out correct step semantics
2. Update BLISP's wzs or create new function if needed
3. Test with step=5
4. Verify exact numerical match

### 3. Test with Other step Values

**Files to replicate:**
- `ES1I_locf_wzs_250_2.csv` (step=2)
- `ES1I_locf_wzs_250_3.csv` (step=3)
- `ES1I_locf_wzs_250_10.csv` (step=10)

Verify the pattern holds across different step values.

### 4. Document the Solution

Once working:
- Update REPLICATION_GUIDE.md with correct step semantics
- Add wzs step examples
- Document the difference between step=1 and step>1
- Create scripts for all step values

---

## Quick Reference Commands

### Generate step=1 (works)
```bash
time /home/ubuntu/generate_es1i_250.sh
# Output: ES1I_locf_wzs_250_1.csv (9,550 rows, 88ms)
```

### Check reference files
```bash
# Check structure
wc -l /home/ubuntu/ES1I_locf_wzs_250_*.csv

# Check non-NA counts
grep -v ';NA$' /home/ubuntu/ES1I_locf_wzs_250_5.csv | wc -l

# First values
head -30 /home/ubuntu/ES1I_locf_wzs_250_5.csv
```

### Compare outputs
```bash
python3 << 'EOF'
import pandas as pd
ref = pd.read_csv('ES1I_locf_wzs_250_5.csv', sep=';')
test = pd.read_csv('test_output.csv', sep=';')
print(f"Shapes: {ref.shape} vs {test.shape}")
print(f"Non-NA: {ref.notna().sum()[1]} vs {test.notna().sum()[1]}")
EOF
```

---

## Priority for Tomorrow

**HIGH PRIORITY:**
1. 🔴 Understand CLISPI wzs step semantics (ask user or test darqt)
2. 🔴 Implement correct step>1 behavior in BLISP
3. 🔴 Verify ES1I_locf_wzs_250_5.csv exact replication

**MEDIUM PRIORITY:**
4. 🟡 Test other step values (2, 3, 10)
5. 🟡 Update documentation with correct semantics

**LOW PRIORITY:**
6. 🟢 Add automated tests for step parameter
7. 🟢 Performance benchmarks for different step values

---

## Notes

- BLISP's current `keep` operation is shape-preserving (maintains row count)
- Reference file has filtered rows (reduced row count from weekends)
- The z-score values themselves are different, not just sampling
- This suggests step affects the CALCULATION, not just output sampling
- Need to understand if step means "sample observations for rolling window"

**Key insight:** step=1 works perfectly, so the basic rolling z-score calculation is correct. The issue is specifically with how step>1 changes the behavior.

---

## Contact Info

- BLISP repo: `/home/ubuntu/blisp` (branch: `reconstruct/tableview-only`)
- GitHub: `noosehack/BLISP`
- All test files in: `/home/ubuntu/`
- Scripts in: `/home/ubuntu/*.sh`

**Latest commit:** Added `keep` operation to IR (not matching CLISPI step yet)
