# BLISP Canonical Rename Quick Reference

**Date:** 2026-02-26
**Purpose:** Fast lookup for old → new name mappings

---

## Quick Lookup by Old Name

```
abs            → ABS
apply-cols     → APL_FLDS
asofr          → ASOF_ALIGN
chop           → CHOP
col            → FLD
cols           → FLDS
cs1            → SHF_PFX_LIN_SUM
cs1-col        → SHF_PFX_LIN_SUM_FLD
cs1-cols       → SHF_PFX_LIN_SUM_FLDS
diff           → DIFF
diff-col       → DIFF_FLD
diff-cols      → DIFF_FLDS
dlog           → DLOG
dlog-col       → DLOG_FLD
dlog-cols      → DLOG_FLDS
ecs1           → SHF_REC_EXP_LIN_SUM
ecs1-col       → SHF_REC_EXP_LIN_SUM_FLD
ecs1-cols      → SHF_REC_EXP_LIN_SUM_FLDS
exp            → EXP
file           → SRC
file-head      → SRC_HED
keep-shape     → KEEP_SHAPE
keep-shape-cols→ KEEP_SHAPE_FLDS
lag            → LAG
len            → LEN
locf           → SHF_REC_NLN_LOCF
locf-cols      → SHF_REC_NLN_LOCF_FLDS
log            → LOG
make-col       → MK_FLD
map-cols       → MAP_FLDS
mapr           → ALIGN
mask-define    → MSK_DEF
mask-list      → MSK_LIST
mask-off       → MSK_OFF
mask-on        → MSK_ON
mask-stats     → MSK_STATS
mask-weekend   → MSK_WKE
mean           → AVG
mean0          → AVG_OMT
o              → ORI
print          → PRN
save           → SAVE
select         → SEL
select-num     → SEL_NUM
setcol         → SET_FLD
shift          → SHF
shift-col      → SHF_FLD
shift-cols     → SHF_FLDS
std            → SDV
std0           → SDV_OMT
stdin          → STDIN
sum            → SUM
sum0           → SUM_OMT
type-of        → TYPE
ur             → RSK_ADJ
ur-col         → RSK_ADJ_FLD
ur-cols        → RSK_ADJ_FLDS
w              → GET
with-mask      → WTH_MSK
withcol        → WTH_FLD
wkd            → MSK_WKE
wstd           → SHF_WIN_NLN_SDV
wstd-cols      → SHF_WIN_NLN_SDV_FLDS
wstd0          → SHF_WIN_MIN2_NLN_SDV
wstd0-cols     → SHF_WIN_MIN2_NLN_SDV_FLDS
wv             → SHF_WIN_NLN_VOL
wv-cols        → SHF_WIN_NLN_VOL_FLDS
wz0            → SHF_WIN_MIN2_NLN_ZSC
wz0-cols       → SHF_WIN_MIN2_NLN_ZSC_FLDS
wzs            → SHF_WIN_NLN_ZSC
xminus         → XMINUS
```

## Operators

```
*              → MUL
+              → ADD
-              → SUB
/              → DIV
!=             → NEQ
<              → LSS
<=             → LEQ
==             → EQL
>              → GTR
>=             → GEQ
>-col          → GTR_FLD
>-cols         → GTR_FLDS
```

---

## Most Common Operations

| Old | New | Category |
|-----|-----|----------|
| `dlog` | `DLOG` | Temporal |
| `shift` | `SHF` | Temporal |
| `locf` | `SHF_REC_NLN_LOCF` | Recursive |
| `cs1` | `SHF_PFX_LIN_SUM` | Cumulative |
| `wstd` | `SHF_WIN_NLN_SDV` | Rolling |
| `wkd` | `MSK_WKE` | Mask |
| `mapr` | `ALIGN` | Join |
| `asofr` | `ASOF_ALIGN` | Join |
| `ur` | `RSK_ADJ` | Finance |
| `xminus` | `XMINUS` | Cross-section |

---

## Name Component Decoder

### Prefixes
- `SHF_` = Shift operations
- `MSK_` = Mask operations
- `RSK_ADJ_` = Risk-adjusted (finance)
- No prefix = Primitive operation

### Middle Components
- `WIN_` = Window (rolling)
- `REC_` = Recursive
- `PFX_` = Prefix (cumulative)

### Properties
- `LIN_` = Linear
- `NLN_` = Nonlinear

### Suffixes
- `_FLD` = Single field/column
- `_FLDS` = Multiple fields/columns
- `_OMT` = Omit NA (treat as 0)
- `_MIN2` = Minimum 2 observations

### Common Abbreviations
- `SDV` = Standard Deviation
- `VOL` = Variance
- `ZSC` = Z-Score
- `SUM` = Sum
- `AVG` = Average/Mean
- `WKE` = Weekend
- `LOCF` = Last Observation Carried Forward
- `EXP` = Exponential
- `SRC` = Source (file)
- `PRN` = Print
- `HED` = Head
- `SEL` = Select
- `APL` = Apply
- `GTR` = Greater Than
- `LSS` = Less Than
- `LEQ` = Less or Equal
- `GEQ` = Greater or Equal
- `EQL` = Equal
- `NEQ` = Not Equal
- `ORI` = Orientation
- `WTH` = With
- `MK` = Make

---

## Example Translations

### Simple
```lisp
(dlog prices 1)              → (DLOG prices 1)
(locf data)                  → (SHF_REC_NLN_LOCF data)
(wkd table)                  → (MSK_WKE table)
```

### Medium
```lisp
(wstd returns 20)            → (SHF_WIN_NLN_SDV returns 20)
(cs1-cols data)              → (SHF_PFX_LIN_SUM_FLDS data)
(shift-cols sig 2)           → (SHF_FLDS sig 2)
```

### Complex (GLD_NUM)
```lisp
; Before
(shift-cols
  (>-cols
    (wzs-ft-cols
      (cs1-cols
        (xminus
          (dlog-cols (wkd (file "At.csv")) 1)
          1))
      25)
    -1.0)
  2)

; After
(SHF_FLDS
  (GTR_FLDS
    (SHF_WIN_NLN_ZSC
      (SHF_PFX_LIN_SUM_FLDS
        (XMINUS
          (DLOG_FLDS (MSK_WKE (SRC "At.csv")) 1)
          1))
      25)
    -1.0)
  2)
```

---

## Grep Cheat Sheet

```bash
# Find all shift operations
grep "SHF_" src/*.rs

# Find all window operations
grep "WIN_" src/*.rs

# Find all linear operations
grep "LIN_" src/*.rs

# Find all nonlinear operations
grep "NLN_" src/*.rs

# Find all mask operations
grep "MSK_" src/*.rs

# Find all risk-adjusted operations
grep "RSK_ADJ" src/*.rs

# Find all recursive operations
grep "REC_" src/*.rs

# Find all prefix (cumulative) operations
grep "PFX_" src/*.rs

# Find all field operations
grep "FLD" src/*.rs

# Count by category
grep -c "SHF_" src/ir.rs
grep -c "MSK_" src/ir.rs
grep -c "LIN_" src/ir.rs
grep -c "NLN_" src/ir.rs
```

---

## Sed Script for Bulk Rename (Use Carefully!)

```bash
# This is a TEMPLATE - DO NOT RUN directly!
# Use for reference when doing manual renames

sed -i 's/\bdlog\b/DLOG/g' file.rs
sed -i 's/\blocf\b/SHF_REC_NLN_LOCF/g' file.rs
sed -i 's/\bcs1\b/SHF_PFX_LIN_SUM/g' file.rs
sed -i 's/\bwstd\b/SHF_WIN_NLN_SDV/g' file.rs
sed -i 's/\bwkd\b/MSK_WKE/g' file.rs
# ... etc
```

**Warning:** Bulk sed replacement can break code! Use with extreme caution.
Better approach: Manual rename with compile checks after each change.

---

## Validation Checklist

After renaming a file, verify:
- [ ] All old names replaced with canonical names
- [ ] No stray lowercase variants remain
- [ ] Pattern matches updated (if enum)
- [ ] Token mappings updated (if planner)
- [ ] Comments updated
- [ ] File compiles (`cargo check`)

---

**END OF QUICK REFERENCE**
