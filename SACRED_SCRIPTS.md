# SACRED SCRIPTS - DO NOT MODIFY WITHOUT APPROVAL

**Date**: 2026-02-27
**Status**: PROTECTED - These scripts are the golden standard for testing BLISP

---

## Purpose

These scripts test **ACCURACY** and **SPEED** of BLISP against CLISPI reference implementation.

They are **SACRED** - any changes must be carefully reviewed as they serve as:
1. Regression tests for correctness
2. Performance benchmarks
3. Cross-implementation validation

---

## Scripts

### 1. `GLD_NUM_BLISP.sh` ⚡ SACRED
**Purpose**: Run GLD_NUM test with BLISP
**Output**: `GLD_NUM_BLISP.csv`
**Format**: `TIMESTAMP;GLD_NUM` with semicolon separator
**Execution**: SILENT (no output to stdout)

**Usage**:
```bash
cd /home/ubuntu/blisp
./GLD_NUM_BLISP.sh
# Creates: GLD_NUM_BLISP.csv
```

### 2. `GLD_NUM_CLISPI.sh` ⚡ SACRED
**Purpose**: Run GLD_NUM test with CLISPI (reference implementation)
**Output**: `GLD_NUM_CLISPI.csv`
**Format**: `TIMESTAMP;GLD_NUM` with semicolon separator
**Execution**: SILENT (no output to stdout)

**Usage**:
```bash
cd /home/ubuntu/blisp
./GLD_NUM_CLISPI.sh
# Creates: GLD_NUM_CLISPI.csv
```

### 3. `BENCHMARK_GLD_NUM.sh`
**Purpose**: Compare speed and accuracy of BLISP vs CLISPI
**Output**: Timing results and accuracy comparison to stdout

**Usage**:
```bash
cd /home/ubuntu/blisp
./BENCHMARK_GLD_NUM.sh
# Runs both tests, reports speed and accuracy
```

---

## Output Format (CRITICAL)

Both scripts MUST produce CSV files with:
- **Separator**: Semicolon (`;`)
- **Header**: `TIMESTAMP;GLD_NUM`
- **Format**: Same as `../ES1I.csv` and `../GC1C.csv`
- **Rows**: 6826 (1 header + 6825 data rows)

**Example**:
```
TIMESTAMP;GLD_NUM
2000-01-03;1.000000
2000-01-04;1.000000
...
2026-02-27;1.150383
```

---

## Expected Results

### Accuracy
- **Difference**: < 0.01% (typically 0.0003%)
- **Final value**: ~1.1504
- **Status**: ✅ Production quality

### Speed (Typical)
- **BLISP**: 5-15 seconds
- **CLISPI**: 5-15 seconds
- **Target**: BLISP ≤ CLISPI

---

## What GLD_NUM Tests

The GLD_NUM pipeline is a comprehensive test of:

### Signal Generation (8 operations)
1. `stdin` - Load BZ1/TP1 futures data
2. `w5` - Filter weekdays (Mon-Fri)
3. `dlog` - Log returns
4. `x- 1` - Pairwise spread (BZ1 - TP1)
5. `cs1` - Cumulative sum starting at 1.0
6. `wzs 25 1` - Rolling z-score (25-day window)
7. `> -1` - Threshold comparison mask
8. `shift 2` - Lag by 2 periods (Ft-measurable)

### Application (6 operations)
9. `file GC1C.csv` - Load Gold futures
10. `mapr s` - Align to signal dates (LEFT JOIN)
11. `dlog` - Log returns of Gold
12. `ur 250 5` - Unit ratio (volatility-normalized returns)
13. `* s` - Weight by signal
14. `cs1` - Final cumulative return

**Total**: 14 operations, 6825 rows, 26 years of data

---

## Prerequisites

### Required Files (in /home/ubuntu)
- `RAW_FUT_PRC.csv` - Raw futures prices database
- `GC1C.csv` - Gold futures continuous contract
- `cgrep` utility in PATH

### Required Executables
- `./target/release/blisp` - BLISP binary (must be compiled)
- `../clispi_dev/clispi_dev` - CLISPI binary

### Required Libraries
- `stdlib/compat_clispi.cl` - BLISP compatibility layer
- `../stdlib/finance_short.cl` - CLISPI macro library

---

## Rules for Modification

### ⛔ NEVER MODIFY:
- The GLD_NUM expression itself (12 operations)
- Output file names (`GLD_NUM_BLISP.csv`, `GLD_NUM_CLISPI.csv`)
- Output format (semicolon separator, TIMESTAMP;GLD_NUM header)
- Silent execution (no stdout output from test scripts)

### ✅ CAN MODIFY (with approval):
- Path to executables (if repo structure changes)
- Performance optimizations that don't change output
- Error handling (as long as it fails on errors)

### 📝 MUST DOCUMENT:
- Any changes to these scripts
- Reason for change
- Before/after benchmark results
- Commit hash of change

---

## How to Check if Scripts Are Sacred

When waking up (starting new session):
1. `cd /home/ubuntu/blisp`
2. Check for `SACRED_SCRIPTS.md` (this file)
3. Check git log: `git log --oneline GLD_NUM*.sh BENCHMARK*.sh`
4. If these scripts exist, they are PROTECTED

---

## Validation Checklist

Before committing changes to these scripts:

- [ ] Both scripts run without errors
- [ ] Both produce CSV files with semicolon separator
- [ ] Output format matches `../ES1I.csv` format
- [ ] Header is `TIMESTAMP;GLD_NUM`
- [ ] 6826 rows (1 header + 6825 data)
- [ ] Final values differ by < 0.1%
- [ ] Scripts are SILENT (no stdout output)
- [ ] Benchmark script shows timing and accuracy
- [ ] Changes are documented in git commit

---

## Git Integration

These scripts are version controlled in the BLISP repository:

```bash
cd /home/ubuntu/blisp
git log --oneline SACRED_SCRIPTS.md GLD_NUM*.sh BENCHMARK*.sh
```

**Branch**: `reconstruct/tableview-only`
**Remote**: `github.com:noosehack/BLISP`

---

## Emergency Recovery

If scripts are accidentally modified:

```bash
cd /home/ubuntu/blisp
git checkout HEAD -- GLD_NUM_BLISP.sh GLD_NUM_CLISPI.sh BENCHMARK_GLD_NUM.sh
```

Or restore from last known good commit:
```bash
git log --oneline -- GLD_NUM*.sh
git checkout <commit-hash> -- GLD_NUM_BLISP.sh GLD_NUM_CLISPI.sh
```

---

## Related Documentation

- `GLD_NUM_GOLDEN_TEST_GUIDE.md` - Complete test documentation
- `GLD_NUM_FIX_COMPLETE.md` - Fix history and validation
- `GLD_NUM_STEP_BY_STEP_ANALYSIS.md` - Detailed pipeline analysis
- `INCIDENT_5d5e34d_POST_MORTEM.md` - Why these tests matter

---

## Contact

If you need to modify these scripts:
1. Document the reason
2. Run full validation
3. Commit with detailed message
4. Reference this file in commit message

**These scripts are the truth source for BLISP correctness and performance.**

---

**Last Updated**: 2026-02-27
**Maintainer**: See git log
**Status**: ✅ PROTECTED
