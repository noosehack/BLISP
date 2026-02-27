# BLISP AS-IS ARCHITECTURE REPORT

**Generated**: 2026-02-26
**Commit**: 21c1f62 (Canonical naming framework)
**Evidence**: All statements cite file:line

---

## 1. EXECUTION MODES

### Control Flow Architecture

**HYBRID MODE** (default): Tokens route to either IR compiler or builtin evaluator.

#### 1.1 IR Path (planner.rs → ir.rs → exec.rs)

**Entry**: `eval.rs:82-91` checks `is_builtin()`. If **false**, attempts variable resolution.
**Planner Gate**: `planner.rs:54-135` pattern-matches token names.

**Token → IR Mapping** (planner.rs):
```
dlog    → NumericFunc::SHF_PTW_NLN_DLOG     (line 123)
locf    → NumericFunc::SHF_REC_NLN_LOCF     (line 130)
wkd     → NumericFunc::MSK_WKE              (line 131)
cs1     → NumericFunc::SHF_PFX_LIN_SUM      (line 132)
shift   → NumericFunc::SHF_PTW_LIN_SHF      (line 135-146)
mapr    → JoinOp::ALIGN                     (line 526)
asofr   → JoinOp::ASOF_ALIGN                (line 527)
xminus  → SchemaOp::SHF_PTW_LIN_SPR         (line 530-561)
```

**IR Executor**: `exec.rs:48-80` executes plan nodes in topological order (ir.rs:32).

#### 1.2 Builtin Path (eval.rs → builtins.rs)

**Entry**: `eval.rs:82-91` → `call_builtin()` for registered tokens.
**Registration**: `builtins.rs:61-174` via `rt.register_builtin(name, fn)`.

#### 1.3 Truth Table: Token Routing Decision

| Token      | Registered Builtin? | Planner Match? | Route      | Evidence                    |
|------------|---------------------|----------------|------------|-----------------------------|
| `dlog`     | YES (line 74)       | YES (line 123) | **AMBIGUOUS** (builtin wins) | builtins.rs:74, planner.rs:123 |
| `locf`     | YES (line 123)      | YES (line 130) | **AMBIGUOUS** (builtin wins) | builtins.rs:123, planner.rs:130 |
| `wkd`      | YES (line 129)      | YES (line 131) | **AMBIGUOUS** (builtin wins) | builtins.rs:129, planner.rs:131 |
| `mapr`     | YES (line 146)      | YES (line 526) | **AMBIGUOUS** (builtin wins) | builtins.rs:146, planner.rs:526 |
| `asofr`    | YES (line 147)      | YES (line 527) | **AMBIGUOUS** (builtin wins) | builtins.rs:147, planner.rs:527 |
| `xminus`   | YES (line 137)      | YES (line 530) | **AMBIGUOUS** (builtin wins) | builtins.rs:137, planner.rs:530 |
| `+`        | YES (line 63)       | NO             | Builtin    | builtins.rs:63              |
| `file`     | YES (line 92)       | NO             | Builtin    | builtins.rs:92              |
| `unknown`  | NO                  | NO             | **ERROR**  | eval.rs:94 raises "Undefined variable" |

**CRITICAL BUG**: IR path is **unreachable** for tokens with builtin registrations. Planner code exists but never executes.

---

## 2. IR SURFACE

All enums defined in `ir.rs`.

### 2.1 NumericFunc (ir.rs:158-314)

| Variant                      | Input    | Output   | Mask Semantics | Window/Min | Executor Line |
|------------------------------|----------|----------|----------------|------------|---------------|
| `SHF_PTW_NLN_DLOG`           | Col      | Col      | NA poison      | lag=1      | exec.rs:157   |
| `RET`                        | Col      | Col      | NA poison      | lag=1      | exec.rs:158   |
| `LOG`                        | Col      | Col      | NA poison      | N/A        | exec.rs:159   |
| `EXP`                        | Col      | Col      | NA poison      | N/A        | exec.rs:160   |
| `SQRT`                       | Col      | Col      | NA poison      | N/A        | exec.rs:161   |
| `ABS`                        | Col      | Col      | NA poison      | N/A        | exec.rs:162   |
| `INV`                        | Col      | Col      | NA on div/0    | N/A        | exec.rs:163   |
| `SHF_REC_NLN_LOCF`           | Col      | Col      | NA reducing    | N/A        | exec.rs:164   |
| `MSK_WKE`                    | Frame    | Frame    | Creates mask   | N/A        | exec.rs:133-135 |
| `SHF_PFX_LIN_SUM`            | Col      | Col      | NA preserving  | N/A        | exec.rs:165   |
| `SHF_PTW_LIN_SHF{k}`         | Col      | Col      | NA monotone    | k periods  | exec.rs:166   |
| `LAG_OBS{k}`                 | Col      | Col      | Mask-aware     | k obs      | exec.rs:148-151 |
| `KEEP{k}`                    | Col      | Col      | Downsample     | N/A        | exec.rs:167   |
| `SHF_WIN_LIN_AVG{w}`         | Col      | Col      | Mask-aware     | w, strict  | exec.rs:629   |
| `SHF_WIN_NLN_SDV{w}`         | Col      | Col      | Mask-aware     | w, strict  | exec.rs:630   |
| `SHF_WIN_MIN2_LIN_AVG{w}`    | Col      | Col      | Mask-aware     | w, min=2   | exec.rs:631   |
| `SHF_WIN_MIN2_NLN_SDV{w}`    | Col      | Col      | Mask-aware     | w, min=2   | exec.rs:632   |
| `SHF_WIN_MIN2_LIN_AVG_EXCL{w}` | Col   | Col      | Mask-aware     | w-1, min=2 | exec.rs:633   |
| `SHF_WIN_MIN2_NLN_SDV_EXCL{w}` | Col   | Col      | Mask-aware     | w-1, min=2 | exec.rs:634   |

**Contract**: All preserve I1 (index Arc), I2 (colnames Arc), I3 (nrows). Validated at ir.rs:464-603.

### 2.2 BinaryFunc (ir.rs:347-358)

| Variant | Semantics        | NA Policy    | Executor Line |
|---------|------------------|--------------|---------------|
| `ADD`   | x + y            | Either → NA  | exec.rs:1716  |
| `SUB`   | x - y            | Either → NA  | exec.rs:1717  |
| `MUL`   | x * y            | Either → NA  | exec.rs:1718  |
| `DIV`   | x / y            | y=0 → NA     | exec.rs:1719-1723 |
| `GTR`   | x > y ? 1.0:0.0  | Either → NA  | exec.rs:1726-1728 |

**Contract**: LHS tags preserved (ir.rs:318-322).

### 2.3 JoinOp (ir.rs:365-391)

| Variant       | Semantics              | Index Source | Nrows Source | Executor Line |
|---------------|------------------------|--------------|--------------|---------------|
| `ALIGN{x,y}`  | RIGHT OUTER JOIN       | y's index    | y's nrows    | exec.rs:262-285 |
| `ASOF_ALIGN{x,y}` | RIGHT OUTER ASOF   | y's index    | y's nrows    | exec.rs:288-321 |

**Contract**: No index coercion. Validated at ir.rs:467-495.

### 2.4 SchemaOp (ir.rs:401-455)

| Variant                | I1 (index) | I2 (colnames) | I3 (nrows) | Executor Line |
|------------------------|------------|---------------|------------|---------------|
| `SHF_PTW_LIN_SPR{input,half}` | Preserved | **REBUILT** | Preserved | exec.rs:326-412 |
| `MSK_WKE_DEF{input,name}` | Preserved | Preserved | Preserved | exec.rs:415-475 |
| `WTH_MSK{input,mask_expr}` | Preserved | Preserved | Preserved | exec.rs:478-490 |

**Contract**: I2_schema allows rebuild for spread ops (ir.rs:397-398).

---

## 3. BUILTIN SURFACE

All registered at `builtins.rs:61-174`.

### 3.1 Classification

#### (A) Pure Schema Operations
```
file         → builtin_file          (line 92,  impl 2133)
file-head    → builtin_file_head     (line 93,  impl 2154)
stdin        → builtin_stdin         (line 94,  impl 2176)
save         → builtin_save          (line 95,  impl 2188)
col          → builtin_col           (line 96,  impl 2222)
setcol       → builtin_setcol        (line 97,  impl 2275)
withcol      → builtin_withcol       (line 98,  impl 2344)
w            → builtin_w             (line 99,  impl 2248)
make-col     → builtin_make_col      (line 100, impl 2395)
cols         → builtin_cols          (line 103, impl 3313)
select       → builtin_select        (line 104, impl 3332)
select-num   → builtin_select_num    (line 105, impl 3359)
```

#### (B) Columnwise Combinators (map-like)
```
dlog-cols    → builtin_dlog_cols     (line 108, impl 3471)
shift-cols   → builtin_shift_cols    (line 109, impl 3499)
diff-cols    → builtin_diff_cols     (line 110, impl 3524)
>-cols       → builtin_gt_cols       (line 114, impl 632)
locf-cols    → builtin_locf_cols     (line 124, impl 710)
keep-shape-cols → builtin_keep_shape_cols (line 126, impl 787)
cs1-cols     → builtin_cs1_cols      (line 139, impl 1347)
ecs1-cols    → builtin_ecs1_cols     (line 142, impl 1471)
ur-cols      → builtin_ur_cols       (line 149, impl 1782)
wz0-cols     → builtin_wz0_cols      (line 152, impl 1957)
wstd-cols    → builtin_wstd_cols     (line 158, impl 2852)
wstd0-cols   → builtin_wstd0_cols    (line 159, impl 2868)
wv-cols      → builtin_wv_cols       (line 161, impl 2908)
map-cols     → builtin_map_cols      (line 106, impl 3379)
apply-cols   → builtin_apply_cols    (line 107, impl 3426)
```

#### (C) External Kernel Wrappers (single-column)
```
dlog-col     → builtin_dlog          (line 79,  impl 2053) → blawktrust::dlog_column
shift-col    → builtin_shift         (line 80,  impl 2086) → shift_column (builtins.rs:3273)
diff-col     → builtin_diff          (line 81,  impl 2110) → diff_column (local)
>-col        → builtin_gt            (line 115, impl 446)  → compare_* helpers
<            → builtin_lt            (line 116, impl 479)  → compare_* helpers
>=           → builtin_gte           (line 117, impl 509)  → compare_* helpers
<=           → builtin_lte           (line 118, impl 539)  → compare_* helpers
==           → builtin_eq            (line 119, impl 569)  → compare_* helpers
!=           → builtin_neq           (line 120, impl 599)  → compare_* helpers
locf         → builtin_locf          (line 123, impl 679)  → blawktrust ops
cs1-col      → builtin_cs1           (line 140, impl 1315) → cumsum logic
ecs1-col     → builtin_ecs1          (line 143, impl 1439) → exp(cumsum(log()))
ur-col       → builtin_ur            (line 150, impl 1695) → ret inverse
wstd         → builtin_wstd          (line 156, impl 2826) → blawktrust::wstd
wstd0        → builtin_wstd0         (line 157, impl 2839) → blawktrust::wstd0
wv           → builtin_wv            (line 160, impl 2884) → wstd²
```

#### (D) Duplicates IR Operations (CONFLICT)
```
dlog         → builtin_dlog_cols     (line 74)  SHADOWS planner.rs:123
shift        → builtin_shift_cols    (line 75)  SHADOWS planner.rs:135
wkd          → builtin_wkd           (line 129) SHADOWS planner.rs:131
locf         → builtin_locf          (line 123) SHADOWS planner.rs:130
cs1          → builtin_cs1_cols      (line 138) SHADOWS planner.rs:132
mapr         → builtin_mapr          (line 146) SHADOWS planner.rs:526
asofr        → builtin_asofr         (line 147) SHADOWS planner.rs:527
xminus       → builtin_xminus        (line 137) SHADOWS planner.rs:530
mask-weekend → builtin_mask_weekend  (line 130) Schema op (no IR shadow)
with-mask    → builtin_with_mask     (line 131) Schema op (no IR shadow)
```

**CRITICAL**: Category (D) creates IR dead code.

#### (E) Arithmetic/Math Primitives
```
+            → builtin_add           (line 63,  impl 181)
-            → builtin_sub           (line 64,  impl 225)
*            → builtin_mul           (line 65,  impl 252)
/            → builtin_div           (line 66,  impl 330)
log          → builtin_log           (line 69,  impl 387)
exp          → builtin_exp           (line 70,  impl 404)
abs          → builtin_abs           (line 71,  impl 421)
```

#### (F) Aggregations
```
sum          → builtin_sum           (line 84,  impl 2458)
sum0         → builtin_sum0          (line 85,  impl 2538)
mean         → builtin_mean          (line 86,  impl 2553)
mean0        → builtin_mean0         (line 87,  impl 2657)
std          → builtin_std           (line 88,  impl 2672)
std0         → builtin_std0          (line 89,  impl 2792)
```

#### (G) Composite/Convenience
```
wzs          → builtin_wzs           (line 153, impl 2024) → locf(keep-shape(wz0))
wz0          → builtin_wz0           (line 151, impl 1896) → blawktrust::wzscore wrapper
zscore       → builtin_zscore        (line 164, impl 2935) → (x-μ)/σ
chop         → builtin_chop          (line 165, impl 2986) → abs(x)<ε ? 0 : x
keep-shape   → builtin_keep_shape    (line 125, impl 754)  → if is_na(orig) then NA else x
```

#### (H) Utilities
```
print        → builtin_print         (line 168, impl 2415)
type-of      → builtin_type_of       (line 169, impl 2427)
len          → builtin_len           (line 170, impl 2436)
o            → builtin_o             (line 173, impl 3561) → blawktrust::lookup_ori
```

#### (I) Mask System
```
mask-off     → builtin_mask_off      (line 133, impl per code)
mask-list    → builtin_mask_list     (line 134, impl per code)
mask-stats   → builtin_mask_stats    (line 135, impl per code)
mask-define  → builtin_mask_define   (line 136, impl per code)
```

---

## 4. KERNEL OWNERSHIP

### 4.1 Hot Kernels (Canonical Definitions)

#### dlog_column
- **Canonical**: `blawktrust::builtins::ops::dlog_column` (external crate)
- **Import**: builtins.rs:12
- **Call Sites**:
  - builtins.rs:2053 (builtin_dlog via indirect)
  - exec.rs:157 (IR executor, **DEAD CODE** - builtin shadows)
  - frame.rs:591, 886 (test code)
- **Shadow Definition**: exec.rs:1092 (local reimplementation) **DUPLICATE RISK**

#### shift_column
- **Canonical**: exec.rs:1350
- **Call Sites**:
  - exec.rs:166 (IR path, **DEAD**)
  - builtins.rs:3273 (wrapper for blawktrust::Column type)
  - No external import

#### locf_column
- **Canonical**: exec.rs:1170
- **Call Sites**:
  - exec.rs:164 (IR path, **DEAD**)
  - builtins.rs:679 via indirect (builtin_locf)
- **No Duplicates** ✓

#### diff
- **Implementation**: Inline at call sites (ret-1)
- **No Canonical Definition** (simple: `x[i] - x[i-1]`)

#### rolling_mean / rolling_std
- **Canonical Strict (min_periods=w)**:
  - exec.rs:1457 (`rolling_mean_column`)
  - exec.rs:1517 (`rolling_std_column`)
- **Canonical Partial (min_periods=2)**:
  - exec.rs:1592 (`rolling_mean_partial`)
  - exec.rs:1645 (`rolling_std_partial`)
- **Mask-Aware Variants**:
  - exec.rs:682 (`rolling_mean_mask_aware`)
  - exec.rs:777 (`rolling_std_mask_aware`)
  - exec.rs:877 (`rolling_mean_partial_mask_aware`)
  - exec.rs:979 (`rolling_std_partial_mask_aware`)
  - exec.rs:881 (`rolling_mean_partial_mask_aware_offset`)
  - exec.rs:983 (`rolling_std_partial_mask_aware_offset`)
- **Legacy Variants** (backup suffix):
  - exec.rs:733, 832, 937, 1045
- **Call Sites**:
  - exec.rs:629-634 (IR rolling ops)
  - **No Direct Conflicts** ✓

#### wstd / wzscore
- **Canonical**: `blawktrust::builtins::ops::{wstd, wstd0, wzscore}` (external)
- **Import**: builtins.rs:12
- **Call Sites**:
  - builtins.rs:2826 (`builtin_wstd`)
  - builtins.rs:2839 (`builtin_wstd0`)
  - builtins.rs:1896 (`builtin_wz0` wraps wzscore)
- **No Duplicates** ✓

### 4.2 Kernel Risk Matrix

| Kernel         | Canonical Location        | Duplicates? | IR Path Status | Risk Level |
|----------------|---------------------------|-------------|----------------|------------|
| dlog_column    | blawktrust (external)     | YES (exec.rs:1092) | DEAD (shadowed) | **HIGH** |
| shift_column   | exec.rs:1350              | Wrapper (builtins.rs:3273) | DEAD | MEDIUM |
| locf_column    | exec.rs:1170              | NO          | DEAD           | LOW |
| rolling_mean   | exec.rs:1457/682/877      | Legacy variants | ACTIVE (IR) | LOW |
| rolling_std    | exec.rs:1517/777/979      | Legacy variants | ACTIVE (IR) | LOW |
| wstd           | blawktrust (external)     | NO          | N/A (builtin-only) | LOW |
| wzscore        | blawktrust (external)     | NO          | N/A (builtin-only) | LOW |

**DUPLICATE BUG**: `exec.rs:1092` defines local `dlog_column` shadowing `blawktrust::builtins::ops::dlog_column`. Builds successfully suggest one is unused.

---

## 5. ARCHITECTURE BUGS

### 5.1 IR Path Unreachable (Severity: CRITICAL)
- **Evidence**: eval.rs:82 checks builtin first, planner never executes for registered tokens
- **Affected**: dlog, locf, wkd, cs1, mapr, asofr, xminus
- **Impact**: 8 IR enum variants unused, planner.rs:123-561 dead code
- **Fix Required**: Remove builtin registrations OR remove IR path

### 5.2 Kernel Duplication (Severity: HIGH)
- **Evidence**: exec.rs:1092 local dlog_column vs builtins.rs:12 import
- **Impact**: Maintenance burden, divergence risk
- **Fix Required**: Unify to single source

### 5.3 Ambiguous Versioning (Severity: MEDIUM)
- **Evidence**: `dlog` (builtin) vs `dlog-col` (kernel) vs `dlog-cols` (combinator)
- **Impact**: User confusion, inconsistent API
- **Fix Required**: Canonical naming convention

---

## 6. RECOMMENDATIONS

1. **DECIDE EXECUTION MODEL**:
   - Option A: IR-ONLY (remove all (D) builtins)
   - Option B: BUILTIN-ONLY (remove planner.rs IR mappings)
   - Option C: HYBRID with explicit routing (e.g., `ir/dlog` vs `dlog`)

2. **CONSOLIDATE KERNELS**:
   - Make `blawktrust` the single truth for hot kernels
   - Remove exec.rs:1092 duplicate

3. **DOCUMENT ROUTING**:
   - Add explicit truth table to docs
   - Enforce with CI tests (check for IR shadowing)

---

**END OF REPORT**
