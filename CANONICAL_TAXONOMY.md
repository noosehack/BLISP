# BLISP Canonical Taxonomy Analysis

**Date:** 2026-02-26
**Purpose:** Analyze mathematical properties and invariances via canonical names

---

## Taxonomic Classification by Prefix

### Category 1: Shift Operations (`SHF_*`) - 23 operations

**Property:** All operations that modify temporal alignment or causality

#### Window-based (Rolling) - 8 ops
```
SHF_WIN_NLN_SDV              (wstd)       - Rolling std dev
SHF_WIN_NLN_SDV_FLDS         (wstd-cols)
SHF_WIN_MIN2_NLN_SDV         (wstd0)      - Partial window (min 2 obs)
SHF_WIN_MIN2_NLN_SDV_FLDS    (wstd0-cols)
SHF_WIN_NLN_VOL              (wv)         - Rolling variance
SHF_WIN_NLN_VOL_FLDS         (wv-cols)
SHF_WIN_MIN2_NLN_ZSC         (wz0)        - Rolling z-score
SHF_WIN_MIN2_NLN_ZSC_FLDS    (wz0-cols)
SHF_WIN_NLN_ZSC              (wzs)
```

**Invariance:** All `SHF_WIN_*` operations preserve causality (past-only windows)

#### Prefix-based (Cumulative) - 6 ops
```
SHF_PFX_LIN_SUM              (cs1)        - Cumulative sum [LINEAR]
SHF_PFX_LIN_SUM_FLD          (cs1-col)
SHF_PFX_LIN_SUM_FLDS         (cs1-cols)
SHF_REC_EXP_LIN_SUM          (ecs1)       - Exp cumulative sum [LINEAR]
SHF_REC_EXP_LIN_SUM_FLD      (ecs1-col)
SHF_REC_EXP_LIN_SUM_FLDS     (ecs1-cols)
```

**Invariance:** All `SHF_PFX_LIN_*` operations are LINEAR (composition preserves linearity)

#### Recursive - 2 ops
```
SHF_REC_NLN_LOCF             (locf)       - Last obs carried forward [NONLINEAR]
SHF_REC_NLN_LOCF_FLDS        (locf-cols)
```

**Invariance:** `SHF_REC_NLN_*` = nonlinear, breaks under composition

#### Basic Shift - 4 ops
```
SHF                          (shift)      - Temporal shift
LAG                          (lag)        - Synonym for shift
SHF_FLD                      (shift-col)
SHF_FLDS                     (shift-cols)
```

#### Difference - 3 ops
```
DIFF                         (diff)       - First difference [LINEAR]
DIFF_FLD                     (diff-col)
DIFF_FLDS                    (diff-cols)
```

---

### Category 2: Mask Operations (`MSK_*`) - 8 operations

**Property:** Operations that modify observation visibility/validity

```
MSK_WKE         (wkd)            - Weekend mask
MSK_WKE         (mask-weekend)   - Alias
MSK_DEF         (mask-define)    - Define custom mask
MSK_LIST        (mask-list)      - List active masks
MSK_OFF         (mask-off)       - Deactivate mask
MSK_ON          (mask-on)        - Activate mask
MSK_STATS       (mask-stats)     - Mask statistics
WTH_MSK         (with-mask)      - Apply mask to operation
```

**Invariance:** Masks are NONLINEAR (mask(x+y) ≠ mask(x) + mask(y))

**Grepability:**
```bash
grep "MSK_" src/*.rs  # Find all mask operations
```

---

### Category 3: Risk-Adjusted Operations (`RSK_ADJ_*`) - 4 operations

**Property:** Financial operations (rolling regression, beta)

```
RSK_ADJ         (ur)       - Rolling univariate regression
RSK_ADJ_FLD     (ur-col)
RSK_ADJ_FLDS    (ur-cols)
ORI             (o)        - Orientation metadata
```

**Invariance:** NONLINEAR, market-relative

---

### Category 4: Alignment Operations (`ALIGN`, `ASOF_ALIGN`) - 2 operations

**Property:** Row-level joins and time-based alignment

```
ALIGN           (mapr)     - Exact match join
ASOF_ALIGN      (asofr)    - As-of join (point-in-time)
```

**Invariance:** Row-level, preserves causality

---

### Category 5: Primitives (No prefix) - 46 operations

#### Arithmetic (11)
```
MUL, ADD, SUB, DIV          (*, +, -, /)     [LINEAR except DIV]
NEQ, LSS, LEQ, EQL, GTR, GEQ (!=, <, <=, ==, >, >=)  [NONLINEAR - boolean]
ABS                          (abs)            [NONLINEAR]
```

#### Math (2)
```
EXP                          (exp)            [NONLINEAR]
LOG                          (log)            [NONLINEAR]
```

#### Temporal (1)
```
DLOG                         (dlog)           [NONLINEAR - log of differences]
DLOG_FLD                     (dlog-col)
DLOG_FLDS                    (dlog-cols)
```

#### Aggregations (6)
```
AVG             (mean)       [LINEAR]
AVG_OMT         (mean0)      [LINEAR - treats NA as 0]
SDV             (std)        [NONLINEAR]
SDV_OMT         (std0)       [NONLINEAR]
SUM             (sum)        [LINEAR]
SUM_OMT         (sum0)       [LINEAR]
```

#### I/O (5)
```
SRC             (file)
SRC_HED         (file-head)
PRN             (print)
SAVE            (save)
STDIN           (stdin)
```

#### Field Operations (10)
```
FLD             (col)
FLDS            (cols)
APL_FLDS        (apply-cols)
MK_FLD          (make-col)
MAP_FLDS        (map-cols)
SEL             (select)
SEL_NUM         (select-num)
SET_FLD         (setcol)
GET             (w)
WTH_FLD         (withcol)
```

#### Comparison (2)
```
GTR_FLD         (>-col)
GTR_FLDS        (>-cols)
```

#### Utility (3)
```
LEN             (len)
TYPE            (type-of)
CHOP            (chop)
KEEP_SHAPE      (keep-shape)
KEEP_SHAPE_FLDS (keep-shape-cols)
```

#### Cross-Sectional (1)
```
XMINUS          (xminus)     [LINEAR - pairwise differences]
```

---

## Linearity Analysis

### LINEAR Operations (Composable)

**Property:** `f(ax + by) = a·f(x) + b·f(y)`

```bash
grep "LIN_" src/*.rs
```

**Results:**
```
SHF_PFX_LIN_SUM              (cs1)         - Cumulative sum
SHF_PFX_LIN_SUM_FLD
SHF_PFX_LIN_SUM_FLDS
SHF_REC_EXP_LIN_SUM          (ecs1)        - Exponential cumulative
SHF_REC_EXP_LIN_SUM_FLD
SHF_REC_EXP_LIN_SUM_FLDS
```

**Additional LINEAR (no prefix):**
```
ADD, SUB, MUL (by scalar)    (+, -, *)
SUM, SUM_OMT                 (sum, sum0)
AVG, AVG_OMT                 (mean, mean0)
DIFF, DIFF_FLD, DIFF_FLDS    (diff)
XMINUS                       (xminus)
```

**Composition Rule:** Linear operations compose linearly
```lisp
(SHF_PFX_LIN_SUM (ADD x y))  ≡  (ADD (SHF_PFX_LIN_SUM x) (SHF_PFX_LIN_SUM y))
```

---

### NONLINEAR Operations (Non-composable)

**Property:** `f(ax + by) ≠ a·f(x) + b·f(y)`

```bash
grep "NLN_" src/*.rs
```

**Results:**
```
SHF_WIN_NLN_SDV              (wstd)        - Rolling std dev
SHF_WIN_NLN_SDV_FLDS
SHF_WIN_MIN2_NLN_SDV         (wstd0)
SHF_WIN_MIN2_NLN_SDV_FLDS
SHF_WIN_NLN_VOL              (wv)          - Rolling variance
SHF_WIN_NLN_VOL_FLDS
SHF_WIN_MIN2_NLN_ZSC         (wz0)         - Rolling z-score
SHF_WIN_MIN2_NLN_ZSC_FLDS
SHF_WIN_NLN_ZSC              (wzs)
SHF_REC_NLN_LOCF             (locf)        - Forward fill
SHF_REC_NLN_LOCF_FLDS
```

**Additional NONLINEAR (no prefix):**
```
EXP, LOG, ABS                (exp, log, abs)
DLOG, DLOG_FLD, DLOG_FLDS    (dlog)
SDV, SDV_OMT                 (std, std0)
DIV                          (/)           - Nonlinear in denominator
MSK_* (all)                  - Masking operations
GTR, LSS, EQL, etc.          - Comparison (boolean output)
RSK_ADJ, RSK_ADJ_*           (ur)          - Risk-adjusted
```

**Composition Rule:** Nonlinear operations break linearity
```lisp
(SHF_WIN_NLN_SDV (ADD x y))  ≠  (ADD (SHF_WIN_NLN_SDV x) (SHF_WIN_NLN_SDV y))
```

---

## Causality Analysis

### Causal Operations (Past-only)

**Property:** Output at time `t` depends ONLY on inputs at times `≤ t`

```bash
grep "SHF_WIN_" src/*.rs   # Window ops preserve causality
grep "SHF_REC_" src/*.rs   # Recursive ops preserve causality
grep "SHF_PFX_" src/*.rs   # Prefix ops preserve causality
```

**All SHF_* operations are CAUSAL** (by design)

---

### Acausal Operations (Future-dependent)

**Property:** Output at time `t` may depend on inputs at times `> t`

**None in current taxonomy!** (All operations are causal)

**Note:** `shift` with negative lag creates acausal dependencies (shift future to present)

---

## Variance Analysis

### Operations Preserving Variance Structure

```bash
grep "LIN_" src/*.rs  # Linear ops preserve variance structure
```

**Examples:**
- `ADD` with constant: `Var(x + c) = Var(x)`
- `MUL` by constant: `Var(a·x) = a²·Var(x)`
- `SHF_PFX_LIN_SUM`: Variance grows linearly with time

---

### Operations Modifying Variance

```bash
grep "NLN_" src/*.rs  # Nonlinear ops change variance
```

**Examples:**
- `SHF_WIN_NLN_SDV`: Computes variance (second moment)
- `LOG`, `EXP`: Transform variance nonlinearly
- `DLOG`: Stabilizes variance (log returns)

---

## Grep Queries for Analysis

### Find all shift operations
```bash
grep -E "SHF_[A-Z_]+" src/*.rs | cut -d: -f2 | sort -u
```

### Find all linear operations
```bash
grep -E "LIN_[A-Z_]+" src/*.rs | cut -d: -f2 | sort -u
```

### Find all nonlinear operations
```bash
grep -E "NLN_[A-Z_]+" src/*.rs | cut -d: -f2 | sort -u
```

### Find all mask operations
```bash
grep -E "MSK_[A-Z_]+" src/*.rs | cut -d: -f2 | sort -u
```

### Find all window operations
```bash
grep -E "WIN_[A-Z_]+" src/*.rs | cut -d: -f2 | sort -u
```

### Count operations by category
```bash
grep -r "SHF_" src/*.rs | wc -l   # Shift operations
grep -r "MSK_" src/*.rs | wc -l   # Mask operations
grep -r "LIN_" src/*.rs | wc -l   # Linear operations
grep -r "NLN_" src/*.rs | wc -l   # Nonlinear operations
```

---

## Invariance Discovery Examples

### Example 1: Linearity of Cumulative Sum
```lisp
; Linear composition
(SHF_PFX_LIN_SUM (ADD x y))  ≡  (ADD (SHF_PFX_LIN_SUM x) (SHF_PFX_LIN_SUM y))

; Verifiable by grep
grep "SHF_PFX_LIN_SUM" src/exec.rs  # Find implementation
grep "LIN_" src/ir.rs               # Confirm linearity tag
```

### Example 2: Nonlinearity of Rolling Std Dev
```lisp
; Nonlinear - does NOT distribute
(SHF_WIN_NLN_SDV (ADD x y))  ≠  (ADD (SHF_WIN_NLN_SDV x) (SHF_WIN_NLN_SDV y))

; Verifiable by grep
grep "SHF_WIN_NLN_SDV" src/exec.rs  # Find implementation
grep "NLN_" src/ir.rs               # Confirm nonlinearity tag
```

### Example 3: Mask Operations Break Linearity
```lisp
; Mask operations are nonlinear
(MSK_WKE (ADD x y))  ≠  (ADD (MSK_WKE x) (MSK_WKE y))

; Verifiable by grep
grep "MSK_" src/ir.rs  # Find all mask operations
```

---

## Taxonomy Statistics

### By Prefix
```
SHF_*     : 23 operations (shift-based)
MSK_*     : 8 operations (mask-based)
RSK_ADJ_* : 4 operations (risk-adjusted)
ALIGN     : 2 operations (joins)
(none)    : 46 operations (primitives)
---
TOTAL     : 83 operations
```

### By Linearity
```
LIN_   : 6 named operations + ~8 primitives = ~14 linear ops
NLN_   : 11 named operations + ~20 primitives = ~31 nonlinear ops
(mixed): ~38 operations (I/O, utility, field ops)
```

### By Operation Type
```
WIN_   : 9 operations (window/rolling)
REC_   : 8 operations (recursive)
PFX_   : 6 operations (prefix/cumulative)
```

---

## Composition Rules

### Rule 1: Linear × Linear = Linear
```lisp
(SHF_PFX_LIN_SUM (ADD x c))  ; Linear composition
```

### Rule 2: Linear × Nonlinear = Nonlinear
```lisp
(SHF_PFX_LIN_SUM (LOG x))    ; Nonlinear result
```

### Rule 3: Nonlinear × Anything = Nonlinear
```lisp
(SHF_WIN_NLN_SDV (ADD x y))  ; Nonlinear result
```

### Rule 4: Mask × Anything = Nonlinear
```lisp
(MSK_WKE (SHF_PFX_LIN_SUM x))  ; Nonlinear result
```

---

## Future Analysis Possibilities

With canonical names, we can programmatically:

1. **Generate composition tables** - Which operations compose to what properties?
2. **Verify mathematical invariances** - Automatically test linearity, causality
3. **Optimize fusion rules** - Linear chains can fuse differently than nonlinear
4. **Document operation properties** - Auto-generate property tables from names
5. **Type-check pipelines** - Warn if composing incompatible operations
6. **Performance prediction** - Linear ops faster than nonlinear (single-pass)

---

## Summary

The canonical naming scheme encodes:
- **Semantic category** (SHF_, MSK_, RSK_ADJ_)
- **Operation type** (WIN_, REC_, PFX_)
- **Mathematical property** (LIN_, NLN_)
- **Variant** (_FLD, _FLDS, _OMT, _MIN2)

This makes the codebase **self-documenting** and **grep-analyzable** for discovering:
- Invariances (linearity, causality)
- Composition rules
- Performance characteristics
- Mathematical properties

**The taxonomy is now encoded in the names themselves.**

---

**END OF TAXONOMY ANALYSIS**
