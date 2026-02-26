# BLISP Canonical Framework - Integrated Documentation

**Date:** 2026-02-26
**Status:** Planning - Framework Integration
**Purpose:** Reconcile CANONICAL_RENAME.csv with formal `<INV>_<SUPP>_<ALG>_<OP>` framework

---

## Executive Summary

This document integrates:
1. **Formal Framework** from `BLISP_Canonical_Framework_Documentation.md`
2. **Rename Mapping** from `CANONICAL_RENAME.csv`
3. **Implementation Plan** from previous planning documents

**Key Finding:** The CSV mapping is **partially inconsistent** with the formal 4-axis framework. We need to decide:
- Option A: Use CSV names as-is (simpler, less systematic)
- Option B: Correct CSV to follow framework (more systematic, more changes)
- Option C: Hybrid (framework for complex ops, simple names for primitives)

---

## The Formal Framework

### Structure: `<INV>_<SUPP>_<ALG>_<OP>`

### Axis 1: INV (Invariance/Equivariance)
```
SHF    Shift-equivariant in time
NON    No symmetry claimed
SCL    Scale-equivariant (optional)
```

### Axis 2: SUPP (Support Geometry)
```
PTW    Pointwise (depends only on current time t)
WIN    Fixed sliding window [t-w, t]
PFX    Prefix accumulation [0, t]
REC    Recursive (finite-state, depends on previous state)
GLO    Global dataset dependence
```

**Refinements:**
```
MIN2   Minimum 2 observations (partial window)
FIR    Finite impulse response
```

### Axis 3: ALG (Algebraic Structure)
```
LIN    Linear: f(ax + by) = af(x) + bf(y)
NLN    Nonlinear
OPT    Optimization-based
QDR    Quadratic (optional)
EXP    Exponential (optional)
```

### Axis 4: OP (Semantic Token)
```
AVG    Mean/Average
SDV    Standard Deviation
SUM    Sum
ZSC    Z-Score
VOL    Volatility/Variance
LOCF   Last Observation Carried Forward
SPR    Cross-sectional Spread
DLOG   Differenced Log
etc.
```

---

## Consistency Analysis: CSV vs Framework

### ✅ Operations Following Framework (11)

| CSV Name | Framework Format | Axes |
|----------|-----------------|------|
| `SHF_WIN_NLN_SDV` | ✅ Correct | SHF + WIN + NLN + SDV |
| `SHF_WIN_NLN_SDV_FLDS` | ✅ Correct | + FLDS suffix |
| `SHF_WIN_MIN2_NLN_SDV` | ✅ Correct | + MIN2 refinement |
| `SHF_WIN_MIN2_NLN_SDV_FLDS` | ✅ Correct | + FLDS suffix |
| `SHF_WIN_NLN_VOL` | ✅ Correct | SHF + WIN + NLN + VOL |
| `SHF_WIN_NLN_VOL_FLDS` | ✅ Correct | + FLDS suffix |
| `SHF_WIN_MIN2_NLN_ZSC` | ✅ Correct | SHF + WIN + NLN + ZSC |
| `SHF_WIN_MIN2_NLN_ZSC_FLDS` | ✅ Correct | + FLDS suffix |
| `SHF_WIN_NLN_ZSC` | ✅ Correct | SHF + WIN + NLN + ZSC |
| `SHF_PFX_LIN_SUM` | ✅ Correct | SHF + PFX + LIN + SUM |
| `SHF_REC_NLN_LOCF` | ✅ Correct | SHF + REC + NLN + LOCF |

### ❌ Operations NOT Following Framework (72)

#### Category 1: Missing INV component

| CSV Name | Issue | Framework-Compliant |
|----------|-------|---------------------|
| `XMINUS` | Missing all axes | `SHF_PTW_LIN_SPR` |
| `DLOG` | Missing axes | `SHF_PTW_NLN_DLOG` |
| `DLOG_FLD` | Missing axes | `SHF_PTW_NLN_DLOG_FLD` |
| `DLOG_FLDS` | Missing axes | `SHF_PTW_NLN_DLOG_FLDS` |
| `ADD` | Missing axes | `NON_PTW_LIN_ADD` |
| `SUB` | Missing axes | `NON_PTW_LIN_SUB` |
| `MUL` | Missing axes | `NON_PTW_LIN_MUL` |
| `DIV` | Missing axes | `NON_PTW_NLN_DIV` |
| `ABS` | Missing axes | `NON_PTW_NLN_ABS` |
| `EXP` | Missing axes | `NON_PTW_NLN_EXP` |
| `LOG` | Missing axes | `NON_PTW_NLN_LOG` |

#### Category 2: Partial framework (has SUPP but missing INV)

| CSV Name | Issue | Framework-Compliant |
|----------|-------|---------------------|
| `SHF_PFX_LIN_SUM_FLD` | ✅ Correct | (already correct) |
| `SHF_PFX_LIN_SUM_FLDS` | ✅ Correct | (already correct) |
| `SHF_REC_EXP_LIN_SUM` | ⚠️ EXP in SUPP? | Should be `SHF_REC_LIN_SUM` with EXP in ALG? |
| `SHF_REC_NLN_LOCF_FLDS` | ✅ Correct | (already correct) |

#### Category 3: Mask operations (unclear INV/SUPP/ALG)

| CSV Name | Issue | Framework-Compliant |
|----------|-------|---------------------|
| `MSK_WKE` | Missing SUPP/ALG | `NON_GLO_NLN_MSK_WKE`? |
| `MSK_DEF` | Missing SUPP/ALG | `NON_GLO_NLN_MSK_DEF`? |
| `MSK_LIST` | Missing SUPP/ALG | `NON_GLO_NLN_MSK_LIST`? |
| `MSK_OFF` | Missing SUPP/ALG | `NON_GLO_NLN_MSK_OFF`? |
| `MSK_ON` | Missing SUPP/ALG | `NON_GLO_NLN_MSK_ON`? |
| `MSK_STATS` | Missing SUPP/ALG | `NON_GLO_NLN_MSK_STATS`? |
| `WTH_MSK` | Missing SUPP/ALG | `NON_GLO_NLN_WTH_MSK`? |

#### Category 4: Aggregations (missing SUPP)

| CSV Name | Issue | Framework-Compliant |
|----------|-------|---------------------|
| `AVG` | Missing axes | `NON_GLO_LIN_AVG` |
| `AVG_OMT` | Missing axes | `NON_GLO_LIN_AVG_OMT` |
| `SDV` | Missing axes | `NON_GLO_NLN_SDV` |
| `SDV_OMT` | Missing axes | `NON_GLO_NLN_SDV_OMT` |
| `SUM` | Missing axes | `NON_GLO_LIN_SUM` |
| `SUM_OMT` | Missing axes | `NON_GLO_LIN_SUM_OMT` |

#### Category 5: Shift operations (missing full axes)

| CSV Name | Issue | Framework-Compliant |
|----------|-------|---------------------|
| `SHF` | Missing SUPP/ALG/OP | `SHF_PTW_LIN_SHF` |
| `LAG` | Missing SUPP/ALG/OP | `SHF_PTW_LIN_LAG` |
| `SHF_FLD` | Missing SUPP/ALG | `SHF_PTW_LIN_SHF_FLD` |
| `SHF_FLDS` | Missing SUPP/ALG | `SHF_PTW_LIN_SHF_FLDS` |
| `DIFF` | Missing axes | `SHF_PTW_LIN_DIFF` |
| `DIFF_FLD` | Missing axes | `SHF_PTW_LIN_DIFF_FLD` |
| `DIFF_FLDS` | Missing axes | `SHF_PTW_LIN_DIFF_FLDS` |

#### Category 6: I/O and Utility (not applicable to framework)

These operations don't fit the mathematical framework:
```
SRC, SRC_HED, PRN, SAVE, STDIN      (I/O operations)
FLD, FLDS, GET, SET_FLD, WTH_FLD    (Table operations)
APL_FLDS, MAP_FLDS, MK_FLD          (Table operations)
SEL, SEL_NUM                        (Table operations)
ALIGN, ASOF_ALIGN                   (Join operations)
LEN, TYPE                           (Utility)
CHOP, KEEP_SHAPE, KEEP_SHAPE_FLDS   (Special operations)
GTR_FLD, GTR_FLDS                   (Comparison operations)
NEQ, LSS, LEQ, EQL, GTR, GEQ        (Comparison operations)
RSK_ADJ, RSK_ADJ_FLD, RSK_ADJ_FLDS  (Finance - could be framework-ified)
ORI                                 (Metadata)
```

---

## Decision: Three-Tier Naming Scheme

### Tier 1: Framework-Compliant (Mathematical Operations)
**Format:** `<INV>_<SUPP>_<ALG>_<OP>[_<VARIANT>]`

Use full 4-axis framework for operations with clear mathematical properties:
- Rolling/window operations
- Cumulative operations
- Recursive operations
- Pointwise mathematical operations

**Examples:**
```
SHF_WIN_NLN_SDV              (rolling std dev)
SHF_PFX_LIN_SUM              (cumulative sum)
SHF_REC_NLN_LOCF             (forward fill)
SHF_PTW_NLN_DLOG             (differenced log)
SHF_PTW_LIN_SPR              (spreads, was XMINUS)
NON_PTW_LIN_ADD              (addition)
```

### Tier 2: Category-Prefixed (Infrastructure Operations)
**Format:** `<CATEGORY>_<NAME>[_<VARIANT>]`

Use category prefix without full framework for:
- Mask operations (MSK_)
- I/O operations (SRC_, PRN, SAVE)
- Table operations (FLD, FLDS)
- Join operations (ALIGN)

**Examples:**
```
MSK_WKE                      (weekend mask)
SRC                          (file/source)
FLD                          (column)
ALIGN                        (join)
```

### Tier 3: Simple Names (Primitives & Operators)
**Format:** `<NAME>`

Use simple uppercase for:
- Basic arithmetic/comparison operators
- Utility functions

**Examples:**
```
ADD, SUB, MUL, DIV          (arithmetic)
GTR, LSS, EQL               (comparison)
LEN, TYPE                   (utility)
```

---

## Proposed Corrected Mapping

### Mathematical Operations (Framework Tier 1)

#### Temporal Operations
```
OLD              CSV              FRAMEWORK-CORRECT
dlog             DLOG             SHF_PTW_NLN_DLOG
dlog-col         DLOG_FLD         SHF_PTW_NLN_DLOG_FLD
dlog-cols        DLOG_FLDS        SHF_PTW_NLN_DLOG_FLDS
diff             DIFF             SHF_PTW_LIN_DIFF
diff-col         DIFF_FLD         SHF_PTW_LIN_DIFF_FLD
diff-cols        DIFF_FLDS        SHF_PTW_LIN_DIFF_FLDS
shift            SHF              SHF_PTW_LIN_SHF
lag              LAG              SHF_PTW_LIN_LAG
shift-col        SHF_FLD          SHF_PTW_LIN_SHF_FLD
shift-cols       SHF_FLDS         SHF_PTW_LIN_SHF_FLDS
```

#### Window Operations (Already Correct!)
```
OLD              CSV              FRAMEWORK-CORRECT
wstd             SHF_WIN_NLN_SDV             ✅
wstd-cols        SHF_WIN_NLN_SDV_FLDS        ✅
wstd0            SHF_WIN_MIN2_NLN_SDV        ✅
wstd0-cols       SHF_WIN_MIN2_NLN_SDV_FLDS   ✅
wv               SHF_WIN_NLN_VOL             ✅
wv-cols          SHF_WIN_NLN_VOL_FLDS        ✅
wz0              SHF_WIN_MIN2_NLN_ZSC        ✅
wz0-cols         SHF_WIN_MIN2_NLN_ZSC_FLDS   ✅
wzs              SHF_WIN_NLN_ZSC             ✅
```

#### Prefix Operations (Mostly Correct!)
```
OLD              CSV              FRAMEWORK-CORRECT
cs1              SHF_PFX_LIN_SUM             ✅
cs1-col          SHF_PFX_LIN_SUM_FLD         ✅
cs1-cols         SHF_PFX_LIN_SUM_FLDS        ✅
ecs1             SHF_REC_EXP_LIN_SUM         ⚠️ (EXP placement unclear)
ecs1-col         SHF_REC_EXP_LIN_SUM_FLD     ⚠️
ecs1-cols        SHF_REC_EXP_LIN_SUM_FLDS    ⚠️
```

**Note:** `ecs1` uses `EXP` in middle position. Options:
- A: Keep as-is (EXP as modifier of REC)
- B: Move to ALG: `SHF_REC_LIN_ESUM` (exponential sum in OP)
- C: Create new ALG: `SHF_REC_EXP_SUM` (exponential algebraic structure)

#### Recursive Operations (Correct!)
```
OLD              CSV              FRAMEWORK-CORRECT
locf             SHF_REC_NLN_LOCF            ✅
locf-cols        SHF_REC_NLN_LOCF_FLDS       ✅
```

#### Cross-Sectional (Needs Correction!)
```
OLD              CSV              FRAMEWORK-CORRECT
xminus           XMINUS           SHF_PTW_LIN_SPR
```

#### Aggregations (Needs Correction!)
```
OLD              CSV              FRAMEWORK-CORRECT
mean             AVG              NON_GLO_LIN_AVG
mean0            AVG_OMT          NON_GLO_LIN_AVG_OMT
std              SDV              NON_GLO_NLN_SDV
std0             SDV_OMT          NON_GLO_NLN_SDV_OMT
sum              SUM              NON_GLO_LIN_SUM
sum0             SUM_OMT          NON_GLO_LIN_SUM_OMT
```

#### Pointwise Math (Needs Correction!)
```
OLD              CSV              FRAMEWORK-CORRECT
abs              ABS              NON_PTW_NLN_ABS
exp              EXP              NON_PTW_NLN_EXP
log              LOG              NON_PTW_NLN_LOG
```

#### Arithmetic (Needs Correction!)
```
OLD              CSV              FRAMEWORK-CORRECT
+                ADD              NON_PTW_LIN_ADD
-                SUB              NON_PTW_LIN_SUB
*                MUL              NON_PTW_LIN_MUL (scalar) / NLN (vector)
/                DIV              NON_PTW_NLN_DIV
```

#### Comparison (Keep Simple - Tier 3)
```
OLD              CSV              RECOMMENDATION
!=               NEQ              NEQ (keep simple)
<                LSS              LSS
<=               LEQ              LEQ
==               EQL              EQL
>                GTR              GTR
>=               GEQ              GEQ
>-col            GTR_FLD          GTR_FLD
>-cols           GTR_FLDS         GTR_FLDS
```

---

### Infrastructure Operations (Category-Prefixed Tier 2)

#### Mask Operations (Keep CSV)
```
OLD              CSV              RECOMMENDATION
wkd              MSK_WKE          MSK_WKE ✅
mask-weekend     MSK_WKE          MSK_WKE ✅
mask-define      MSK_DEF          MSK_DEF ✅
mask-list        MSK_LIST         MSK_LIST ✅
mask-off         MSK_OFF          MSK_OFF ✅
mask-on          MSK_ON           MSK_ON ✅
mask-stats       MSK_STATS        MSK_STATS ✅
with-mask        WTH_MSK          WTH_MSK ✅
```

#### I/O Operations (Keep CSV)
```
OLD              CSV              RECOMMENDATION
file             SRC              SRC ✅
file-head        SRC_HED          SRC_HED ✅
print            PRN              PRN ✅
save             SAVE             SAVE ✅
stdin            STDIN            STDIN ✅
```

#### Table/Field Operations (Keep CSV)
```
OLD              CSV              RECOMMENDATION
col              FLD              FLD ✅
cols             FLDS             FLDS ✅
apply-cols       APL_FLDS         APL_FLDS ✅
make-col         MK_FLD           MK_FLD ✅
map-cols         MAP_FLDS         MAP_FLDS ✅
select           SEL              SEL ✅
select-num       SEL_NUM          SEL_NUM ✅
setcol           SET_FLD          SET_FLD ✅
w                GET              GET ✅
withcol          WTH_FLD          WTH_FLD ✅
```

#### Join Operations (Keep CSV)
```
OLD              CSV              RECOMMENDATION
mapr             ALIGN            ALIGN ✅
asofr            ASOF_ALIGN       ASOF_ALIGN ✅
```

#### Finance Operations (Could Framework-ify)
```
OLD              CSV              FRAMEWORK-CORRECT
ur               RSK_ADJ          SHF_WIN_NLN_BETA?
ur-col           RSK_ADJ_FLD      SHF_WIN_NLN_BETA_FLD?
ur-cols          RSK_ADJ_FLDS     SHF_WIN_NLN_BETA_FLDS?
o                ORI              ORI (metadata, keep)
```

#### Utility (Keep Simple - Tier 3)
```
OLD              CSV              RECOMMENDATION
len              LEN              LEN ✅
type-of          TYPE             TYPE ✅
chop             CHOP             CHOP ✅
keep-shape       KEEP_SHAPE       KEEP_SHAPE ✅
keep-shape-cols  KEEP_SHAPE_FLDS  KEEP_SHAPE_FLDS ✅
```

---

## Recommendation: Hybrid Approach (Option C)

### Adopt Three-Tier System:

**Tier 1: Full Framework** (17 operations)
- Window operations: `SHF_WIN_*` ✅ Already correct in CSV
- Prefix operations: `SHF_PFX_*` ✅ Already correct in CSV
- Recursive operations: `SHF_REC_*` ✅ Already correct in CSV
- **ADD:** Pointwise shift operations: `SHF_PTW_*` (dlog, diff, shift, xminus)

**Tier 2: Category-Prefixed** (35 operations)
- Keep CSV as-is: MSK_, SRC_, FLD/FLDS, ALIGN, etc.

**Tier 3: Simple Names** (31 operations)
- Keep CSV as-is: ADD, SUB, GTR, LEN, TYPE, etc.
- Aggregations: AVG, SDV, SUM (not NON_GLO_LIN_AVG)
- Math: ABS, EXP, LOG (not NON_PTW_NLN_ABS)

### Operations Requiring Change (5):

```
OLD              CSV              CORRECTED
dlog             DLOG             SHF_PTW_NLN_DLOG
dlog-col         DLOG_FLD         SHF_PTW_NLN_DLOG_FLD
dlog-cols        DLOG_FLDS        SHF_PTW_NLN_DLOG_FLDS
diff             DIFF             SHF_PTW_LIN_DIFF
diff-col         DIFF_FLD         SHF_PTW_LIN_DIFF_FLD
diff-cols        DIFF_FLDS        SHF_PTW_LIN_DIFF_FLDS
shift            SHF              SHF_PTW_LIN_SHF
shift-col        SHF_FLD          SHF_PTW_LIN_SHF_FLD
shift-cols       SHF_FLDS         SHF_PTW_LIN_SHF_FLDS
xminus           XMINUS           SHF_PTW_LIN_SPR
```

**Rationale:** These are temporal operations that fit naturally into the SHF_PTW framework.

---

## Implementation Impact

### Minimal Changes Approach (Recommended)

**Accept CSV as-is EXCEPT for 10 operations:**

```diff
- DLOG              + SHF_PTW_NLN_DLOG
- DLOG_FLD          + SHF_PTW_NLN_DLOG_FLD
- DLOG_FLDS         + SHF_PTW_NLN_DLOG_FLDS
- DIFF              + SHF_PTW_LIN_DIFF
- DIFF_FLD          + SHF_PTW_LIN_DIFF_FLD
- DIFF_FLDS         + SHF_PTW_LIN_DIFF_FLDS
- SHF               + SHF_PTW_LIN_SHF
- SHF_FLD           + SHF_PTW_LIN_SHF_FLD
- SHF_FLDS          + SHF_PTW_LIN_SHF_FLDS
- XMINUS            + SHF_PTW_LIN_SPR
```

### Maximal Changes Approach (Purist)

**Apply framework to ALL mathematical operations (47 total changes):**

Include all aggregations, pointwise math, and arithmetic:
```diff
+ NON_GLO_LIN_AVG
+ NON_PTW_NLN_ABS
+ NON_PTW_LIN_ADD
... etc (see previous tables)
```

---

## Grep Queries with Corrected Names

### Find all shift operations
```bash
grep "^SHF_" src/*.rs
```

### Find all pointwise operations
```bash
grep "PTW_" src/*.rs
```

### Find all window operations
```bash
grep "WIN_" src/*.rs
```

### Find all prefix operations
```bash
grep "PFX_" src/*.rs
```

### Find all recursive operations
```bash
grep "REC_" src/*.rs
```

### Find all linear operations
```bash
grep "LIN_" src/*.rs
```

### Find all nonlinear operations
```bash
grep "NLN_" src/*.rs
```

---

## Final Recommendation

**Option C: Hybrid Three-Tier + 10 Corrections**

1. ✅ **Accept CSV for Tiers 2 & 3** (68 operations unchanged)
2. ✅ **Keep framework-compliant Tier 1** (17 operations unchanged)
3. ⚠️ **Correct 10 operations** to align with framework:
   - dlog → SHF_PTW_NLN_DLOG (and variants)
   - diff → SHF_PTW_LIN_DIFF (and variants)
   - shift → SHF_PTW_LIN_SHF (and variants)
   - xminus → SHF_PTW_LIN_SPR

**Total changes:** 10 operations (12% of 83)

**Benefit:** Balances systematic framework with practical simplicity.

---

## Updated CANONICAL_RENAME.csv (Corrected Version)

Should we generate a corrected CSV with the 10 changes applied?

**Current discrepancies:**
```csv
;; CURRENT (CSV)
dlog;DLOG
xminus;XMINUS
shift;SHF
diff;DIFF

;; CORRECTED (Framework-Aligned)
dlog;SHF_PTW_NLN_DLOG
xminus;SHF_PTW_LIN_SPR
shift;SHF_PTW_LIN_SHF
diff;SHF_PTW_LIN_DIFF
```

---

## Summary

- **Framework:** `<INV>_<SUPP>_<ALG>_<OP>` is the formal canonical structure
- **CSV:** 83 operations, 17 already framework-compliant
- **Gap:** 10 operations need correction to align with framework
- **Recommendation:** Hybrid three-tier system with 10 corrections
- **Benefit:** Systematic for complex ops, simple for primitives

**Next Steps:**
1. ✅ Approve hybrid three-tier approach
2. ✅ Correct 10 operations in CSV
3. ✅ Proceed with implementation plan

---

**END OF INTEGRATED FRAMEWORK DOCUMENTATION**
