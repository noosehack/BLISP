# CANONICAL_RENAME.csv Corrections

**Date:** 2026-02-26
**Purpose:** Document differences between original CSV and framework-corrected version

---

## Summary of Changes

**Total operations:** 83
**Changed:** 10 operations (12%)
**Unchanged:** 73 operations (88%)

---

## Changed Operations

### 1. dlog (and variants) - 3 changes
```diff
OLD → NEW                           RATIONALE
- dlog;DLOG                         Temporal pointwise nonlinear operation
+ dlog;SHF_PTW_NLN_DLOG             Shift-equivariant, pointwise, nonlinear

- dlog-col;DLOG_FLD                 Field variant
+ dlog-col;SHF_PTW_NLN_DLOG_FLD

- dlog-cols;DLOG_FLDS               Fields variant
+ dlog-cols;SHF_PTW_NLN_DLOG_FLDS
```

**Framework Analysis:**
- **INV:** SHF (shift-equivariant in time)
- **SUPP:** PTW (pointwise - depends only on current t and t-1)
- **ALG:** NLN (nonlinear - log is nonlinear)
- **OP:** DLOG (differenced log)

---

### 2. diff (and variants) - 3 changes
```diff
OLD → NEW                           RATIONALE
- diff;DIFF                         First difference is linear
+ diff;SHF_PTW_LIN_DIFF             Shift-equivariant, pointwise, linear

- diff-col;DIFF_FLD                 Field variant
+ diff-col;SHF_PTW_LIN_DIFF_FLD

- diff-cols;DIFF_FLDS               Fields variant
+ diff-cols;SHF_PTW_LIN_DIFF_FLDS
```

**Framework Analysis:**
- **INV:** SHF (shift-equivariant)
- **SUPP:** PTW (pointwise - depends on t and t-1)
- **ALG:** LIN (linear - difference is linear operation)
- **OP:** DIFF (first difference)

---

### 3. shift (and variants) - 3 changes
```diff
OLD → NEW                           RATIONALE
- shift;SHF                         Temporal shift is linear
+ shift;SHF_PTW_LIN_SHF             Shift-equivariant, pointwise, linear

- shift-col;SHF_FLD                 Field variant
+ shift-col;SHF_PTW_LIN_SHF_FLD

- shift-cols;SHF_FLDS               Fields variant
+ shift-cols;SHF_PTW_LIN_SHF_FLDS
```

**Framework Analysis:**
- **INV:** SHF (shift-equivariant by definition!)
- **SUPP:** PTW (pointwise - output at t depends only on input at t-k)
- **ALG:** LIN (linear - shifting preserves linearity)
- **OP:** SHF (shift operation)

**Note:** `lag` remains `LAG` (simple name, same operation)

---

### 4. xminus - 1 change
```diff
OLD → NEW                           RATIONALE
- xminus;XMINUS                     Cross-sectional spread is linear
+ xminus;SHF_PTW_LIN_SPR            Shift-equivariant, pointwise, linear spread
```

**Framework Analysis:**
- **INV:** SHF (shift-equivariant - operates on time series)
- **SUPP:** PTW (pointwise - pairwise differences at each time t)
- **ALG:** LIN (linear - difference operation)
- **OP:** SPR (spread/difference)

---

## Unchanged Operations (Framework-Compliant)

### Already Framework-Compliant (17 operations)

These were already correct in the original CSV:

**Window Operations (9):**
```
wstd       → SHF_WIN_NLN_SDV             ✅
wstd-cols  → SHF_WIN_NLN_SDV_FLDS        ✅
wstd0      → SHF_WIN_MIN2_NLN_SDV        ✅
wstd0-cols → SHF_WIN_MIN2_NLN_SDV_FLDS   ✅
wv         → SHF_WIN_NLN_VOL             ✅
wv-cols    → SHF_WIN_NLN_VOL_FLDS        ✅
wz0        → SHF_WIN_MIN2_NLN_ZSC        ✅
wz0-cols   → SHF_WIN_MIN2_NLN_ZSC_FLDS   ✅
wzs        → SHF_WIN_NLN_ZSC             ✅
```

**Prefix Operations (6):**
```
cs1        → SHF_PFX_LIN_SUM             ✅
cs1-col    → SHF_PFX_LIN_SUM_FLD         ✅
cs1-cols   → SHF_PFX_LIN_SUM_FLDS        ✅
ecs1       → SHF_REC_EXP_LIN_SUM         ✅
ecs1-col   → SHF_REC_EXP_LIN_SUM_FLD     ✅
ecs1-cols  → SHF_REC_EXP_LIN_SUM_FLDS    ✅
```

**Recursive Operations (2):**
```
locf       → SHF_REC_NLN_LOCF            ✅
locf-cols  → SHF_REC_NLN_LOCF_FLDS       ✅
```

---

### Category-Prefixed (35 operations)

These use category prefixes (not full framework) and are acceptable:

**Mask Operations (8):**
```
wkd            → MSK_WKE        ✅
mask-weekend   → MSK_WKE        ✅
mask-define    → MSK_DEF        ✅
mask-list      → MSK_LIST       ✅
mask-off       → MSK_OFF        ✅
mask-on        → MSK_ON         ✅
mask-stats     → MSK_STATS      ✅
with-mask      → WTH_MSK        ✅
```

**I/O Operations (5):**
```
file           → SRC            ✅
file-head      → SRC_HED        ✅
print          → PRN            ✅
save           → SAVE           ✅
stdin          → STDIN          ✅
```

**Table/Field Operations (10):**
```
col            → FLD            ✅
cols           → FLDS           ✅
apply-cols     → APL_FLDS       ✅
make-col       → MK_FLD         ✅
map-cols       → MAP_FLDS       ✅
select         → SEL            ✅
select-num     → SEL_NUM        ✅
setcol         → SET_FLD        ✅
w              → GET            ✅
withcol        → WTH_FLD        ✅
```

**Join Operations (2):**
```
mapr           → ALIGN          ✅
asofr          → ASOF_ALIGN     ✅
```

**Finance Operations (4):**
```
o              → ORI            ✅
ur             → RSK_ADJ        ✅
ur-col         → RSK_ADJ_FLD    ✅
ur-cols        → RSK_ADJ_FLDS   ✅
```

**Comparison Operations (2):**
```
>-col          → GTR_FLD        ✅
>-cols         → GTR_FLDS       ✅
```

**Utility (4):**
```
len            → LEN            ✅
type-of        → TYPE           ✅
chop           → CHOP           ✅
keep-shape     → KEEP_SHAPE     ✅
keep-shape-cols→ KEEP_SHAPE_FLDS✅
```

---

### Simple Names (21 operations)

These use simple uppercase names (primitives, operators) and are acceptable:

**Arithmetic (5):**
```
+              → ADD            ✅
-              → SUB            ✅
*              → MUL            ✅
/              → DIV            ✅
abs            → ABS            ✅
```

**Comparison (6):**
```
!=             → NEQ            ✅
<              → LSS            ✅
<=             → LEQ            ✅
==             → EQL            ✅
>              → GTR            ✅
>=             → GEQ            ✅
```

**Math (2):**
```
exp            → EXP            ✅
log            → LOG            ✅
```

**Aggregations (6):**
```
mean           → AVG            ✅
mean0          → AVG_OMT        ✅
std            → SDV            ✅
std0           → SDV_OMT        ✅
sum            → SUM            ✅
sum0           → SUM_OMT        ✅
```

**Other (2):**
```
lag            → LAG            ✅
CUR            → ISO            ✅ (unclear what this is)
```

---

## Rationale for Unchanged Operations

### Why NOT Apply Framework to Everything?

**Tier 2 & 3 operations are acceptable without full framework because:**

1. **Primitives** (ADD, MUL, etc.) are well-understood and don't need taxonomic classification
2. **Operators** (GTR, LSS, etc.) are standard and self-documenting
3. **Infrastructure** (FLD, SRC, MSK_) already have category prefixes for grepability
4. **Utility functions** (LEN, TYPE) don't have mathematical properties to encode

**The framework is MOST valuable for:**
- Operations with temporal dependencies (SUPP axis matters)
- Operations with algebraic properties (LIN vs NLN matters)
- Operations where invariances are non-obvious (INV axis matters)

---

## Impact of Changes

### Code Changes Required

**Files affected by 10 operation changes:**
1. `src/ir.rs` - Enum variant names
2. `src/planner.rs` - Token mappings
3. `src/builtins.rs` - Builtin registration
4. `src/exec.rs` - Pattern matches
5. `src/ir_fusion.rs` - Fusion rules
6. `tests/*.rs` - Test cases
7. `*.md` - Documentation

**Pattern to find in code:**
```bash
# Old patterns to replace
grep -r "\bDLOG\b" src/
grep -r "\bDIFF\b" src/
grep -r "\bSHF\b" src/       # Be careful - conflicts with SHF_ prefix!
grep -r "\bXMINUS\b" src/
```

---

## Migration Guide

### For Code Changes

Replace these exact strings in pattern matches and token maps:

```rust
// In src/ir.rs (enum variants)
- Dlog,
+ SHF_PTW_NLN_DLOG,

- Diff,
+ SHF_PTW_LIN_DIFF,

- Shift,  // Be careful - might conflict with SHF_ operations
+ SHF_PTW_LIN_SHF,

// In src/planner.rs (token mapping)
- "dlog" => NumericFunc::Dlog,
+ "dlog" => NumericFunc::SHF_PTW_NLN_DLOG,

- "diff" => NumericFunc::Diff,
+ "diff" => NumericFunc::SHF_PTW_LIN_DIFF,

- "shift" => NumericFunc::Shift,
+ "shift" => NumericFunc::SHF_PTW_LIN_SHF,
```

**CAUTION with `SHF`:**
The bare `SHF` conflicts with the `SHF_` prefix used throughout. Recommend:
- Use `SHF_PTW_LIN_SHF` for the shift operation enum variant
- Keep `shift` as the planner token
- Keep `SHF` as simple alias in legacy builtins

---

## Verification After Changes

### 1. Compilation Check
```bash
cd /home/ubuntu/blisp
cargo build 2>&1 | grep -i "pattern"
```

Should show no pattern match errors.

---

### 2. Token Validation
```bash
./blisp --dic | grep -E "(dlog|diff|shift|xminus)"
```

Should show corrected canonical names in output.

---

### 3. Grep Validation
```bash
# Should find dlog in pointwise shift operations
grep "SHF_PTW" src/ir.rs | grep DLOG

# Should find diff in linear operations
grep "LIN_" src/ir.rs | grep DIFF

# Should find xminus as spread
grep "SPR" src/ir.rs
```

---

## Summary

### Changes Made (10 operations)

| Operation | Old Name | New Name | Category |
|-----------|----------|----------|----------|
| dlog | `DLOG` | `SHF_PTW_NLN_DLOG` | Temporal |
| dlog-col | `DLOG_FLD` | `SHF_PTW_NLN_DLOG_FLD` | Temporal |
| dlog-cols | `DLOG_FLDS` | `SHF_PTW_NLN_DLOG_FLDS` | Temporal |
| diff | `DIFF` | `SHF_PTW_LIN_DIFF` | Temporal |
| diff-col | `DIFF_FLD` | `SHF_PTW_LIN_DIFF_FLD` | Temporal |
| diff-cols | `DIFF_FLDS` | `SHF_PTW_LIN_DIFF_FLDS` | Temporal |
| shift | `SHF` | `SHF_PTW_LIN_SHF` | Temporal |
| shift-col | `SHF_FLD` | `SHF_PTW_LIN_SHF_FLD` | Temporal |
| shift-cols | `SHF_FLDS` | `SHF_PTW_LIN_SHF_FLDS` | Temporal |
| xminus | `XMINUS` | `SHF_PTW_LIN_SPR` | Cross-sectional |

### Impact
- **12% of operations** changed (10 out of 83)
- **All temporal pointwise operations** now framework-compliant
- **Grepability improved:** Can now find all pointwise shift operations with `grep "SHF_PTW"`

---

**END OF DIFF DOCUMENTATION**
