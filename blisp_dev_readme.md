# blisp Development Guide

**Last Updated:** 2026-02-19
**Version:** 0.2.0
**Status:** Active Development

---

## 🚨 GOLDEN RULES FOR FILE MANAGEMENT

**CRITICAL: Prevent /tmp pollution and server crashes**

1. **NO FILES IN /tmp** - Ever. Last time server crashed from /tmp overflow.
2. **ONE MASTER README** - This file (`blisp_dev_readme.md`) is the single source of truth.
3. **Update THIS file** - Don't create new markdown files. Update sections here.
4. **Session continuity** - Read this file at start of each session to get up to date.
5. **Check disk usage** - Before large operations: `df -h /tmp`
6. **Temporary test files** - Use `/tmp/test_*.csv` for data only, clean up immediately.
7. **Scratch space** - Use `/home/ubuntu/blisp/scratch/` (gitignored), NOT /tmp.

**File hierarchy:**
- `/home/ubuntu/blisp/blisp_dev_readme.md` ← YOU ARE HERE (master doc)
- `/home/ubuntu/blawk_readme.md` ← blawk reference
- `/home/ubuntu/clispi_dev/clispi_readme.md` ← clispi reference

---

## Project Overview

**blisp** = Rust Lisp interpreter for high-performance columnar operations
**Backend:** blawktrust (1.89× faster than C++)
**Target:** Pure Common Lisp + kdb-like performance
**Use case:** Financial time series, trading signals (GLD_NUM)

---

## Current Status (v0.2.0)

✅ **Working:** 100+ operations, GLD_NUM pipeline, Ft-measurable moments
⏳ **Missing:** Macros, threading `->`, REPL

---

## Ft-measurable Operations (NEW v0.2.0) ⭐

**Problem:** pandas rolling z-score = look-ahead bias
**Solution:** Past-only window [i-w, i-1]

### Functions
- `ft-wmoments-cols` - Single-pass mean/std/skew/kurt
- `ft-wmean-cols` - Yesterday's mean
- `ft-wstd-cols` - Yesterday's std
- `wzs-ft-cols` - Ft z-score (matches Adyton/clispi exactly)

### Implementation
**Location:** `/home/ubuntu/blawktrust/src/builtins/rolling_moments.rs` (519 lines)

**Algorithm:**
- Maintains rolling sums S1, S2, S3, S4 in single pass
- Window [i-w, i-1] excludes current value
- mean = S1/n, var = (S2-S1²/n)/(n-1) [ddof=1], std = sqrt(var)
- skew = mu3 / mu2^(3/2), kurt = mu4 / mu2² - 3

**Tests:** 17/17 passing, validated vs Python (1e-6 precision)
**Performance:** 1.2-1.9× speedup vs separate passes

---

## GLD_NUM Pipeline Status ✅

```lisp
(shift-cols
  (>-cols
    (wzs-ft-cols              ; ← Ft-measurable z-score
      (cs1-cols
        (xminus
          (dlog-cols (WKD (file "At.csv")) 1)
          1))
      25)
    -1.0)
  2)
```

All steps working end-to-end.

---

## Key Operations

### I/O
- `(file "data.csv")` - Load CSV (Bloomberg-compatible)
- `(save "out.csv" table)` - Write CSV
- `(col table "colname")` - Extract column

### Core Operations
- `(dlog-cols prices 1)` - Log returns
- `(wzs-ft-cols returns 25)` - Ft z-score
- `(>-cols zscore 2.0)` - Filter
- `(shift-cols signal 2)` - Shift forward

### Cross-Sectional
- `(xminus returns 1)` - Pairwise spreads
- `(cs1-cols data)` - Cumulative sum
- `(WKD table)` - Filter weekdays

### Advanced
- `(mapr source target)` - Row alignment
- `(ur returns window lag)` - Rolling beta

---

## Building & Testing

```bash
# Build (always build blawktrust first!)
cd /home/ubuntu/blawktrust && cargo build --release
cd /home/ubuntu/blisp && cargo build --release

# Test
cd /home/ubuntu/blisp && cargo test
cargo test --test test_ft_moments

# Run
./target/release/blisp -e "(+ 1 2)"
```

---

## Performance

- **dlog (1M elements):** 15.51ms (vs 29.33ms C++) = **1.89× faster**
- **Ft-moments:** 1.2-1.9× speedup vs separate passes
- **Zero allocation** after warmup

---

## Comparison with clispi

| Feature | clispi | blisp |
|---------|--------|-------|
| Macros | ✅ | ❌ (TODO) |
| Threading `->` | ✅ | ❌ (TODO) |
| Performance | Good | **Better (1.89×)** |
| Memory safety | None | **Rust** |
| Ft-measurable | ⚠️ (wzscore is standard) | ✅ (both available) |

**Important:** clispi's `wzscore` uses **standard window** [i-w+1, i] (includes current value), NOT Ft-measurable!
- blisp `wz0-cols` = clispi `wzscore` (standard, pandas-style)
- blisp `wzs-ft-cols` = Ft-measurable [i-w, i-1] (past-only, investable)

---

## Next Steps (Priority)

1. **Threading macro `->`** - Readable pipelines
2. **Macro system** - defmacro, quasiquote
3. **More Ft functions** - ft-wv, ft-wsharpe
4. **REPL mode** - Interactive dev

---

## Code Organization

```
blisp/
├── src/
│   ├── builtins.rs      # 100+ ops
│   ├── ft_moments.rs    # Ft-measurable (NEW)
│   ├── io.rs            # CSV + Bloomberg
│   ├── eval.rs          # Evaluator
│   └── runtime.rs       # Environment
├── tests/
│   └── test_ft_moments.rs
└── blisp_dev_readme.md  # ← YOU ARE HERE
```

---

## Recent Changes

### 2026-02-19: GLD_NUM Replication ✅
- Successfully replicated clispi's GLD_NUM pipeline
- **Key finding:** clispi `wzscore` is NOT Ft-measurable! Uses standard window [i-w+1, i]
- Solution: Use `wz0-cols` (standard) instead of `wzs-ft-cols` (Ft-measurable)
- GLD_NUM outputs now match exactly (6819 rows, zero difference)

### 2026-02-19: Ft-measurable Moments
- Added single-pass kernel for mean/std/skew/kurt
- Window [i-w, i-1] (past-only, Ft-measurable)
- 17 tests passing, Python-validated
- 1.2-1.9× speedup demonstrated

Files:
- `/home/ubuntu/blawktrust/src/builtins/rolling_moments.rs` (519 lines)
- `/home/ubuntu/blisp/src/ft_moments.rs` (248 lines)
- `/home/ubuntu/blisp/tests/test_ft_moments.rs` (251 lines)

---

## Quick Reference

```lisp
; Load and process
(defparameter data (file "prices.csv"))
(defparameter returns (dlog-cols data 1))

; Ft-measurable z-score
(defparameter zscore (wzs-ft-cols returns 25))
(defparameter signal (>-cols zscore 2.0))

; Full pipeline
(shift-cols (>-cols (wzs-ft-cols (cs1-cols (xminus returns 1)) 25) -1.0) 2)
```

---

**Remember:** Read this file at start of each session for continuity.
**Never:** Create files in /tmp (use scratch/ instead).
**Always:** Check `/tmp` disk usage before large operations.

Last updated: 2026-02-19
