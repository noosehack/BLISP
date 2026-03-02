# BLISP Current Operation Taxonomy

## Three Architectural Layers

### LAYER 1: IR KERNELS (29 operations)
**Location:** `src/ir.rs` - NumericFunc, BinaryFunc, JoinOp, SchemaOp enums

These are the **true computational kernels** - the only operations that actually execute.

**Unary (19):**
- Dlog, Ret, Log, Exp, Sqrt, Abs, Inv
- Locf, Wkd, CumSum
- Shift{k}, LagObs{k}, Keep{k}
- RollMean{w}, RollStd{w}
- RollMeanMin2{w}, RollStdMin2{w}
- RollMeanMin2ExclCurrent{w}, RollStdMin2ExclCurrent{w}

**Binary (5):**
- Add, Sub, Mul, Div, Gt

**Joins (2):**
- MapR (exact match), AsofR (as-of join)

**Schema (3):**
- Xminus (pairwise spreads)
- MaskWeekend (create weekend mask)
- WithMask (activate mask)

---

### LAYER 2: PLANNER CANONICAL (IR-based, ~20 operations)
**Location:** `src/planner.rs` - token → IR mapping

**Modern canonical names** for IR compilation:

**Direct IR mappings:**
```
"dlog"              → NumericFunc::Dlog
"ret"               → NumericFunc::Ret
"log", "exp", "abs" → Log, Exp, Abs
"sqrt", "inv"       → Sqrt, Inv
"locf"              → Locf
"wkd"               → Wkd  ← CANONICAL weekend mask
"cs1"               → CumSum
"shift"             → Shift{k}
"lag-obs"           → LagObs{k}  ← observation-based lag
"keep"              → Keep{k}
"rolling-mean"      → RollMean{w}
"rolling-mean-min2" → RollMeanMin2{w}
"rolling-std"       → RollStd{w}
"rolling-std-min2"  → RollStdMin2{w}
"+", "-", "*", "/"  → Add, Sub, Mul, Div
">"                 → Gt
"mapr", "asofr"     → MapR, AsofR
"xminus"            → Xminus
"mask-weekend"      → MaskWeekend
"with-mask"         → WithMask
```

**Derived forms (planner rewrites):**
```
"ft-mean"          → shift(1, rolling-mean(w, x))
"ft-std"           → shift(1, rolling-std(w, x))
"rolling-zscore"   → (x - rolling-mean) / rolling-std
"wzs"              → same as rolling-zscore (CLISPI compat)
"ft-zscore"        → ft z-score with ExclCurrent kernels
"ur"               → rolling univariate regression (complex rewrite)
```

---

### LAYER 3: LEGACY BUILTINS (83 operations)
**Location:** `src/builtins.rs` - register_builtin() calls

**Old AST evaluator** with many spelling variants and table operations.

**Categories:**

**Arithmetic (5):** +, -, *, /, abs
**Comparison (6):** >, <, >=, <=, ==, !=
**Math (2):** log, exp

**Temporal (9 names → 3 kernels):**
- dlog, dlog-col, dlog-cols    (all → dlog kernel)
- shift, shift-col, shift-cols  (all → shift kernel)
- diff, diff-col, diff-cols     (all → diff kernel)

**Aggregations (6):**
- sum, sum0    (skip NA vs NA→0)
- mean, mean0  (skip NA vs NA→0)
- std, std0    (skip NA vs NA→0)

**Table Operations (10):**
- col, cols, w, setcol, withcol, make-col
- select, select-num, map-cols, apply-cols

**Rolling (9 names → 4 kernels):**
- wstd, wstd-cols      → strict rolling std
- wstd0, wstd0-cols    → partial (min2)
- wv, wv-cols          → rolling variance
- wz0, wz0-cols        → rolling z-score
- wzs                  → composite (locf + keep-shape + wz0)

**Transforms (13 names → 7 kernels):**
- cs1, cs1-col, cs1-cols        → cumulative sum
- ecs1, ecs1-col, ecs1-cols     → exp cumulative sum
- locf, locf-cols               → last obs carried forward
- keep-shape, keep-shape-cols   → keep with NA fill
- wkd                           → weekend mask
- xminus                        → pairwise spreads
- chop, zscore

**Mask Operations (7):**
- mask-weekend, with-mask, mask-on (alias), mask-off
- mask-list, mask-stats, mask-define

**Join Operations (2):**
- mapr, asofr

**Column Comparisons (2):**
- >-col, >-cols

**Finance (4 names → 1 kernel):**
- ur, ur-col, ur-cols  → rolling beta
- o                     → orientation metadata

**I/O (5):**
- file, file-head, stdin, save, print

**Utility (3):**
- len, type-of

---

## Key Differences

### IR vs Legacy:

1. **Canonical naming:**
   - IR: "rolling-mean", "rolling-std", "lag-obs", "keep"
   - Legacy: "wstd", "wstd0", "shift", various *-col/*-cols variants

2. **Operation count:**
   - IR: 29 true kernels
   - Legacy: 83 registered names (many aliases/variants)

3. **Execution:**
   - IR: Compiled to DAG, optimized, validated
   - Legacy: Direct AST evaluation

4. **Macro support:**
   - IR: Only normalize (threading `->`), no defmacro
   - Legacy: Full defmacro, macroexpand

5. **Current mode:**
   - Default: **HYBRID** (IR for Frame ops, legacy fallback)
   - `--ir-only`: Force IR compilation
   - `--legacy`: Force AST evaluator

---

## Aliases & Compatibility

### Confirmed aliases in legacy:
- `mask-on` → `with-mask`
- `>` → `>-cols` (table version)
- `dlog` → `dlog-cols` (table version by default)

### CLISPI compatibility:
- `wzs` accepted in planner (maps to rolling-zscore rewrite)
- Uses `RollMeanMin2`/`RollStdMin2` for masked calendar compat

### Deprecated/Legacy operations:
- Rolling functions have legacy O(n·w) versions in exec.rs
- Kept for verification: `rolling_mean_mask_aware_legacy`, etc.

---

## What is "Canonical"?

1. **IR level:** The enum variant names (Dlog, RollMean, MapR, etc.)
2. **Planner level:** The tokens accepted by planner ("dlog", "rolling-mean", "wkd")
3. **User level:** What `./blisp --dic` shows (83 operations)

**The planner tokens are the NEW canonical names** being standardized.

---

## Summary

- **29 IR kernels** = actual computation
- **~20 planner tokens** = modern canonical names (IR-based)
- **83 legacy builtins** = old names with many variants/aliases
- **Hybrid mode** = IR when possible, legacy fallback
- **Goal:** Migrate to IR-only with clean canonical naming
