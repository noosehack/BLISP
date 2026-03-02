# GLD_NUM Verification Record

**Purpose:** Document that BLISP produces numerically identical results to CLISPI

---

## Latest Verification: 2026-03-01

**Commit:** f4ebdae (after orientation tripwires, INSTALL.md, EXPOSED_OPS.md)

### Results

| Metric | CLISPI | BLISP | Status |
|--------|--------|-------|--------|
| **Row count** | 6826 | 6826 | ✅ Match |
| **Date range** | 2000-01-03 → 2026-02-27 | 2000-01-03 → 2026-02-27 | ✅ Match |
| **Maximum difference** | - | 5.00e-07 | ✅ < 1e-6 |
| **Rows exceeding tolerance** | - | 0 | ✅ Pass |

**Tolerance:** 1e-6 (0.000001)

**Conclusion:** ✅ BLISP and CLISPI produce identical results within floating-point precision

### Sample Values

**First value (2000-01-03):**
- CLISPI: 1.000000
- BLISP:  1.0
- Difference: 0.0

**Largest difference (2022-10-13):**
- CLISPI: 1.104329
- BLISP:  1.104329499882859
- Difference: 4.99e-07

**Last value (2026-02-27):**
- CLISPI: 1.150383
- BLISP:  1.1503826750916901
- Difference: 3.25e-07

### Verification Method

```bash
./verify_gld_num.sh
```

This script:
1. Runs GLD_NUM_BLISP.sh
2. Compares row counts
3. Verifies timestamps match
4. Checks numerical differences < 1e-6
5. Reports maximum difference

---

## Historical Verification

### 2026-02-28 (Commit 89fdbca - IEEE-754 policy)
- ✅ GLD_NUM validated after IEEE-754 numeric behavior implementation
- Row count: 6826 ✅
- Values match within tolerance ✅

### 2026-02-27 (Orientation refactor)
- ✅ GLD_NUM validated after orientation system refactor (commit 68152a8)
- Orientation fix using blawktrust as source of truth
- No regression in GLD_NUM output ✅

---

## What GLD_NUM Tests

The golden pipeline exercises:

1. **CSV I/O** - Multi-column financial data
2. **Weekday filtering** - `w5` (Mon-Fri only)
3. **Log returns** - `dlog` (observation-based semantics)
4. **Cumulative sums** - `cs1`
5. **Rolling z-score** - `wzs 25 1` (window=25, step=1)
6. **Comparisons** - `> -1` (element-wise)
7. **Temporal shifts** - `shift 2`
8. **Table joins** - `mapr` (LEFT JOIN semantics)
9. **Unit ratio** - `ur 250 5` (risk-adjusted returns)
10. **NA handling** - LOCF and skip-NA aggregations

**Complete pipeline (from GLD_NUM_BLISP.sh):**
```lisp
(let* ((s (-> (stdin) (w5) (dlog) (x- 1) (cs1) (wzs 25 1) (> -1) (shift 2))))
  (-> (file "../GC1C.csv") (mapr s) (dlog) (ur 250 5) (* s) (cs1)))
```

---

## How to Verify After Changes

**Always run after:**
- Modifying numeric operations (dlog, cs1, wzs, ur)
- Changing orientation system
- Updating aggregations (sum, mean, std)
- Modifying NA handling (LOCF, skip-NA)
- Updating IR planner or executor

**Command:**
```bash
cd /home/ubuntu/blisp
./verify_gld_num.sh
```

**Expected output:**
```
=== GLD_NUM Verification Script ===

Running GLD_NUM_BLISP.sh...
Row counts:
  CLISPI: 6826
  BLISP:  6826
✅ Row counts match

Running numerical comparison...
Rows compared: 6825
Maximum difference: 5.00e-07 (at row 5943)
Tolerance: 1.00e-06

✅ All values match within tolerance

=== Verification Summary ===
✅ GLD_NUM output matches CLISPI reference
✅ Ready for production
```

---

## CI Integration

**Recommended CI check:**
```yaml
- name: Verify GLD_NUM matches CLISPI
  run: |
    cd /home/ubuntu/blisp
    ./verify_gld_num.sh
```

This ensures no regression in correctness across commits.

---

## Reference Files

| File | Purpose | Status |
|------|---------|--------|
| `GLD_NUM_BLISP.sh` | BLISP golden test script | ✅ SACRED - do not modify |
| `GLD_NUM_CLISPI.csv` | Reference output from CLISPI | ✅ Ground truth |
| `GLD_NUM_BLISP.csv` | Current BLISP output | 🔄 Regenerated each run |
| `verify_gld_num.sh` | Automated verification script | ✅ Run after changes |

---

**Last verified:** 2026-03-01 11:37 UTC  
**Verified by:** Automated test + manual inspection  
**Status:** ✅ PASS - BLISP matches CLISPI within tolerance
