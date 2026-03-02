# Function Comparison: clispi vs blisp for lastcode scripts

## GLD_NUM Pipeline Analysis

### clispi version (line 26):
```lisp
(let* ((s (-> (stdin) (WKD) (dlog) (x- 1) (cs1) (wzs 25 1) (> -1) (shift 2))))
  (-> (file "GC1C.csv") (mapr s) (dlog) (ur 250 5) (* s) (cs1)))
```

### Functions Needed:

| Function | clispi | blisp | Status | Notes |
|----------|--------|-------|--------|-------|
| `->` | ✅ | ❌ | **MISSING** | Threading macro - HIGH PRIORITY |
| `stdin` | ✅ | ✅ | ✅ OK | |
| `WKD` | ✅ | ✅ | ✅ OK | Filter weekdays |
| `dlog` | ✅ | ✅ | ✅ OK | Use `dlog-cols` for tables |
| `x-` | ✅ (macro) | ✅ | ✅ OK | `xminus` in blisp, takes (table half) |
| `cs1` | ✅ | ✅ | ✅ OK | Use `cs1-cols` for tables |
| `wzs` | ✅ (macro) | ✅ | ✅ OK | clispi `wzs` = standard window [i-w+1,i]. Use blisp `wz0-cols` to match |
| `>` | ✅ | ✅ | ✅ OK | Use `>-cols` for tables |
| `shift` | ✅ | ✅ | ✅ OK | Use `shift-cols` for tables |
| `file` | ✅ | ✅ | ✅ OK | |
| `mapr` | ✅ | ✅ | ✅ OK | Row mapping/alignment |
| `ur` | ✅ | ✅ | ✅ OK | Univariate regression |
| `*` | ✅ | ✅ | ✅ OK | Element-wise multiplication |
| `locf` | ✅ | ✅ | ✅ OK | Last obs carried forward |

## Critical Missing Features

1. **Threading macro `->`** - Makes pipelines readable
   - Without it: deeply nested expressions
   - With it: `(-> x (f1) (f2) (f3))`
   - Priority: **CRITICAL** for script parity

2. **wzs clarification** ✅ **RESOLVED**
   - **Investigation (2026-02-19):** Examined clispi's `wzscore` implementation in blawk_combined.cpp
   - **Finding:** clispi uses **standard window** [i-w+1, i] that includes current value
   - **Not Ft-measurable!** Buffer adds current value before calculating stats
   - **Solution:** Use `wz0-cols` (not `wzs-ft-cols`) to match clispi
   - **Result:** GLD_NUM outputs now match exactly

## Workaround Without Threading Macro

Current blisp approach uses intermediate variables:
```lisp
(let* ((s (shift-cols (>-cols (wz0-cols (cs1-cols (xminus (dlog-cols (WKD (file ...))))))))))
  ...)
```

This is functionally equivalent but harder to read/maintain.
