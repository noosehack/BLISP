# GLD_NUM Verification Protocol

**CRITICAL REQUIREMENT**: This is the acceptance test for BLISP correctness.

---

## ⚠️ MANDATORY CHECKS

When verifying GLD_NUM, you **MUST** check:

### 1. Row Count Match
```bash
wc -l GLD_NUM_BLISP.csv GLD_NUM_CLISPI.csv
```
**Expected**: Both files have **exactly 6826 rows** (1 header + 6825 data rows)

### 2. ✅ **VALUE COMPARISON** (REQUIRED!)

**NOT SUFFICIENT**: Checking only row counts
**REQUIRED**: Compare actual numerical values

```bash
./verify_gld_num.sh
```

**Must verify**:
- All 6825 timestamp values match
- All 6825 GLD_NUM values match within tolerance **1e-6**
- Maximum difference is reported and acceptable

**Example CORRECT output**:
```
Rows compared: 6825
Maximum difference: 5.00e-07 (at row 5943)
Tolerance: 1.00e-06
✅ All values match within tolerance
```

**Example WRONG output** (REJECT THIS):
```
Row counts:
  CLISPI: 6826
  BLISP:  6826
✅ Row counts match
```
⛔ **This is NOT sufficient!** Values may differ despite same count.

---

## Why This Matters

**Past incident (2026-03-01)**:
- Initial verification only checked row counts
- User caught that actual values weren't compared
- Could have missed semantic bugs that change numerical output

**Failure modes that row count alone won't catch**:
- Fusion bugs changing -inf to NaN (same count, wrong values)
- Precision regressions (same count, different values)
- Order changes (same count, different semantics)
- Off-by-one in calculations (same count, shifted values)

---

## Verification Script

**Location**: `/home/ubuntu/blisp/verify_gld_num.sh`

**What it does**:
1. Runs GLD_NUM_BLISP.sh to generate output
2. Compares row counts (sanity check)
3. **Compares all numerical values** using Python
4. Reports maximum difference and location
5. Checks against tolerance threshold

**Always use this script** - don't write ad-hoc checks.

---

## Tolerance Specification

**Current tolerance**: 1e-6 (1 part per million)

**Rationale**:
- Rust f64 vs C++ double should be identical for same operations
- Small differences (5e-7) acceptable due to:
  - Floating point evaluation order
  - Library implementation differences (exp, ln, etc.)
  - Summation order in cumulative operations

**If max diff exceeds 1e-6**: INVESTIGATION REQUIRED
- Check recent commits for semantic changes
- Compare intermediate values in pipeline
- Verify fusion optimizations preserve semantics

---

## CI Integration

**Manual check** (current):
```bash
cd /home/ubuntu/blisp
./verify_gld_num.sh
```

**Future CI** (TODO):
```yaml
- name: Verify GLD_NUM
  run: |
    cd /home/ubuntu/blisp
    ./verify_gld_num.sh
    # Script exits non-zero if values don't match
```

---

## Stone Tablet

**WHEN SOMEONE SAYS "GLD_NUM PASSES"**:

They MUST mean:
1. ✅ Row count matches (6826)
2. ✅ **All 6825 values checked and match within 1e-6**
3. ✅ Maximum difference reported and acceptable

**NOT**:
❌ Just row count matches
❌ Just file exists
❌ Just script runs without error

**Verification evidence required**:
```
Maximum difference: X.XXe-XX (at row YYYY)
Tolerance: 1.00e-06
✅ All values match within tolerance
```

---

## Related Files

- `GLD_NUM_BLISP.sh` - Pipeline script
- `verify_gld_num.sh` - Verification script
- `VERIFICATION_RECORD.md` - Historical verification log
- `INSTALL.md` - Installation includes GLD_NUM test

---

**Last Updated**: 2026-03-01
**Commit**: f850fd7
**Verified By**: Claude + User confirmation
