# BLISP TOKEN INVENTORY - COMPLETE CATALOG

**Generated**: 2026-02-26
**Commit**: 21c1f62
**Evidence**: file:line for all claims

---

## CATEGORY A: PLANNER-MAPPED (IR Path Only)

Tokens that route **exclusively** through `planner.rs` → IR → `exec.rs`.

| Token | Planner Line | IR Variant | Builtin? | Status |
|-------|--------------|------------|----------|--------|
| `stdin` | planner.rs:108-120 | Source::Stdin | NO | ✓ IR-ONLY |
| `ret` | planner.rs:124 | NumericFunc::RET | NO | ✓ IR-ONLY |
| `sqrt` | planner.rs:127 | NumericFunc::SQRT | NO | ✓ IR-ONLY |
| `inv` | planner.rs:129 | NumericFunc::INV | NO | ✓ IR-ONLY |
| `cs1` | planner.rs:132 | NumericFunc::SHF_PFX_LIN_SUM | **YES** | 🚨 CONFLICT (see C1) |
| `lag-obs` | planner.rs:153-167 | NumericFunc::LAG_OBS{k} | NO | ✓ IR-ONLY |
| `keep` | planner.rs:170-184 | NumericFunc::KEEP{k} | NO | ✓ IR-ONLY |
| `rolling-mean` | planner.rs:187-201 | NumericFunc::SHF_WIN_LIN_AVG{w} | NO | ✓ IR-ONLY |
| `rolling-mean-min2` | planner.rs:204-218 | NumericFunc::SHF_WIN_MIN2_LIN_AVG{w} | NO | ✓ IR-ONLY |
| `ft-mean` | planner.rs:223-252 | Composite (roll+shift) | NO | ✓ IR-ONLY |
| `rolling-std` | planner.rs:255-269 | NumericFunc::SHF_WIN_NLN_SDV{w} | NO | ✓ IR-ONLY |
| `rolling-std-min2` | planner.rs:273-286 | NumericFunc::SHF_WIN_MIN2_NLN_SDV{w} | NO | ✓ IR-ONLY |
| `ft-std` | planner.rs:290-318 | Composite (roll+shift) | NO | ✓ IR-ONLY |
| `rolling-zscore` | planner.rs:322-399 | Composite (mean/std/div) | NO | ✓ IR-ONLY |
| `wzs` | planner.rs:322-399 | Composite (same as rolling-zscore) | NO | ✓ IR-ONLY |
| `ur` | planner.rs:407-463 | Composite (std*1587.45/div) | NO | ✓ IR-ONLY |
| `ft-zscore` | planner.rs:468-516 | Composite (excl mean/std) | NO | ✓ IR-ONLY |
| `let` | planner.rs:608+ | LetBinding (schema) | NO | ✓ IR-ONLY |

**Behavior**: These execute **only** via IR path. No builtin registration exists.

---

## CATEGORY B: BUILTIN-ONLY (No IR Mapping)

Tokens registered in `builtins.rs` with **no** planner mapping.

### B1: I/O & Schema Operations

| Token | Builtin Reg Line | Implementation | IR? | Notes |
|-------|------------------|----------------|-----|-------|
| `file` | builtins.rs:92 | builtin_file:2133 | NO | Load CSV from path |
| `file-head` | builtins.rs:93 | builtin_file_head:2154 | NO | Load CSV first N rows |
| `save` | builtins.rs:95 | builtin_save:2188 | NO | Write to file |
| `col` | builtins.rs:96 | builtin_col:2222 | NO | Extract column |
| `setcol` | builtins.rs:97 | builtin_setcol:2275 | NO | Mutate column |
| `withcol` | builtins.rs:98 | builtin_withcol:2344 | NO | Copy-with-column |
| `w` | builtins.rs:99 | builtin_w:2248 | NO | withcol alias |
| `make-col` | builtins.rs:100 | builtin_make_col:2395 | NO | Create column from list |
| `cols` | builtins.rs:103 | builtin_cols:3313 | NO | List column names |
| `select` | builtins.rs:104 | builtin_select:3332 | NO | Select columns by name |
| `select-num` | builtins.rs:105 | builtin_select_num:3359 | NO | Select numeric columns |

### B2: Columnwise Combinators

| Token | Builtin Reg Line | Implementation | IR? | Notes |
|-------|------------------|----------------|-----|-------|
| `map-cols` | builtins.rs:106 | builtin_map_cols:3379 | NO | Apply fn to each column |
| `apply-cols` | builtins.rs:107 | builtin_apply_cols:3426 | NO | Apply fn returning table |
| `dlog-cols` | builtins.rs:108 | builtin_dlog_cols:3471 | NO | dlog over all columns |
| `shift-cols` | builtins.rs:109 | builtin_shift_cols:3499 | NO | shift over all columns |
| `diff-cols` | builtins.rs:110 | builtin_diff_cols:3524 | NO | diff over all columns |
| `>-cols` | builtins.rs:114 | builtin_gt_cols:632 | NO | > over all columns |
| `locf-cols` | builtins.rs:124 | builtin_locf_cols:710 | NO | locf over all columns |
| `keep-shape-cols` | builtins.rs:126 | builtin_keep_shape_cols:787 | NO | keep-shape over all |
| `cs1-cols` | builtins.rs:139 | builtin_cs1_cols:1347 | NO | cs1 over all columns |
| `ecs1-cols` | builtins.rs:142 | builtin_ecs1_cols:1471 | NO | ecs1 over all columns |
| `ur-cols` | builtins.rs:149 | builtin_ur_cols:1782 | NO | ur over all columns |
| `wz0-cols` | builtins.rs:152 | builtin_wz0_cols:1957 | NO | wz0 over all columns |
| `wstd-cols` | builtins.rs:158 | builtin_wstd_cols:2852 | NO | wstd over all columns |
| `wstd0-cols` | builtins.rs:159 | builtin_wstd0_cols:2868 | NO | wstd0 over all columns |
| `wv-cols` | builtins.rs:161 | builtin_wv_cols:2908 | NO | wv over all columns |

### B3: Single-Column Kernels

| Token | Builtin Reg Line | Implementation | IR? | Notes |
|-------|------------------|----------------|-----|-------|
| `dlog-col` | builtins.rs:79 | builtin_dlog:2053 | NO | Single col dlog |
| `shift-col` | builtins.rs:80 | builtin_shift:2086 | NO | Single col shift |
| `diff-col` | builtins.rs:81 | builtin_diff:2110 | NO | Single col diff |
| `>-col` | builtins.rs:115 | builtin_gt:446 | NO | Single col > |
| `<` | builtins.rs:116 | builtin_lt:479 | NO | Less than |
| `>=` | builtins.rs:117 | builtin_gte:509 | NO | Greater equal |
| `<=` | builtins.rs:118 | builtin_lte:539 | NO | Less equal |
| `==` | builtins.rs:119 | builtin_eq:569 | NO | Equal |
| `!=` | builtins.rs:120 | builtin_neq:599 | NO | Not equal |
| `cs1-col` | builtins.rs:140 | builtin_cs1:1315 | NO | Single col cumsum |
| `ecs1-col` | builtins.rs:143 | builtin_ecs1:1439 | NO | Single col exp-cumsum |
| `ur-col` | builtins.rs:150 | builtin_ur:1695 | NO | Single col unit ratio |

### B4: Aggregations

| Token | Builtin Reg Line | Implementation | IR? | Notes |
|-------|------------------|----------------|-----|-------|
| `sum` | builtins.rs:84 | builtin_sum:2458 | NO | Sum (NA poison) |
| `sum0` | builtins.rs:85 | builtin_sum0:2538 | NO | Sum (NA=0) |
| `mean` | builtins.rs:86 | builtin_mean:2553 | NO | Mean (NA poison) |
| `mean0` | builtins.rs:87 | builtin_mean0:2657 | NO | Mean (NA=0) |
| `std` | builtins.rs:88 | builtin_std:2672 | NO | Std (NA poison) |
| `std0` | builtins.rs:89 | builtin_std0:2792 | NO | Std (NA=0) |
| `wstd` | builtins.rs:156 | builtin_wstd:2826 | NO | Window std (blawktrust) |
| `wstd0` | builtins.rs:157 | builtin_wstd0:2839 | NO | Window std (NA=0) |
| `wv` | builtins.rs:160 | builtin_wv:2884 | NO | Window variance (wstd²) |

### B5: Mask System

| Token | Builtin Reg Line | Implementation | IR? | Notes |
|-------|------------------|----------------|-----|-------|
| `mask-on` | builtins.rs:132 | builtin_with_mask (alias) | NO | Activate mask |
| `mask-off` | builtins.rs:133 | builtin_mask_off | NO | Deactivate mask |
| `mask-list` | builtins.rs:134 | builtin_mask_list | NO | List defined masks |
| `mask-stats` | builtins.rs:135 | builtin_mask_stats | NO | Mask coverage stats |
| `mask-define` | builtins.rs:136 | builtin_mask_define | NO | Define named mask |

### B6: Composite/Convenience

| Token | Builtin Reg Line | Implementation | IR? | Notes |
|-------|------------------|----------------|-----|-------|
| `wz0` | builtins.rs:151 | builtin_wz0:1896 | NO | wzscore wrapper |
| `keep-shape` | builtins.rs:125 | builtin_keep_shape:754 | NO | Preserve NA pattern |
| `zscore` | builtins.rs:164 | builtin_zscore:2935 | NO | (x-μ)/σ |
| `chop` | builtins.rs:165 | builtin_chop:2986 | NO | Near-zero → 0 |

### B7: Utilities

| Token | Builtin Reg Line | Implementation | IR? | Notes |
|-------|------------------|----------------|-----|-------|
| `print` | builtins.rs:168 | builtin_print:2415 | NO | Print value |
| `type-of` | builtins.rs:169 | builtin_type_of:2427 | NO | Type introspection |
| `len` | builtins.rs:170 | builtin_len:2436 | NO | Length/nrows |
| `o` | builtins.rs:173 | builtin_o:3561 | NO | Orientation lookup |

**Total Builtin-Only**: 56 tokens

---

## CATEGORY C: CONFLICT TOKENS (Both Planner + Builtin)

Tokens that exist in **BOTH** planner and builtin registrations.

### C1: Core Conflicts

| Token | Planner Line | IR Variant | Builtin Reg | Which Wins? | Identical? |
|-------|--------------|------------|-------------|-------------|------------|
| `dlog` | planner.rs:123 | SHF_PTW_NLN_DLOG | builtins.rs:74 | **BUILTIN** | ⚠️ UNKNOWN |
| `log` | planner.rs:125 | LOG | builtins.rs:69 | **BUILTIN** | ⚠️ UNKNOWN |
| `exp` | planner.rs:126 | EXP | builtins.rs:70 | **BUILTIN** | ⚠️ UNKNOWN |
| `abs` | planner.rs:128 | ABS | builtins.rs:71 | **BUILTIN** | ⚠️ UNKNOWN |
| `locf` | planner.rs:130 | SHF_REC_NLN_LOCF | builtins.rs:123 | **BUILTIN** | ⚠️ UNKNOWN |
| `wkd` | planner.rs:131 | MSK_WKE | builtins.rs:129 | **BUILTIN** | ⚠️ UNKNOWN |
| `cs1` | planner.rs:132 | SHF_PFX_LIN_SUM | builtins.rs:138 | **BUILTIN** | ⚠️ UNKNOWN |
| `shift` | planner.rs:135-149 | SHF_PTW_LIN_SHF{k} | builtins.rs:75 | **BUILTIN** | ⚠️ UNKNOWN |

### C2: Arithmetic Conflicts

| Token | Planner Line | IR Variant | Builtin Reg | Which Wins? | Identical? |
|-------|--------------|------------|-------------|-------------|------------|
| `+` | planner.rs:519 | BinaryFunc::ADD | builtins.rs:63 | **BUILTIN** | ⚠️ UNKNOWN |
| `-` | planner.rs:520 | BinaryFunc::SUB | builtins.rs:64 | **BUILTIN** | ⚠️ UNKNOWN |
| `*` | planner.rs:521 | BinaryFunc::MUL | builtins.rs:65 | **BUILTIN** | ⚠️ UNKNOWN |
| `/` | planner.rs:522 | BinaryFunc::DIV | builtins.rs:66 | **BUILTIN** | ⚠️ UNKNOWN |
| `>` | planner.rs:523 | BinaryFunc::GTR | builtins.rs:113 | **BUILTIN** | ⚠️ UNKNOWN |

### C3: Join Conflicts

| Token | Planner Line | IR Variant | Builtin Reg | Which Wins? | Identical? |
|-------|--------------|------------|-------------|-------------|------------|
| `mapr` | planner.rs:526 | JoinOp::ALIGN | builtins.rs:146 | **BUILTIN** | ⚠️ UNKNOWN |
| `asofr` | planner.rs:527 | JoinOp::ASOF_ALIGN | builtins.rs:147 | **BUILTIN** | ⚠️ UNKNOWN |

### C4: Schema Op Conflicts

| Token | Planner Line | IR Variant | Builtin Reg | Which Wins? | Identical? |
|-------|--------------|------------|-------------|-------------|------------|
| `xminus` | planner.rs:530-554 | SchemaOp::SHF_PTW_LIN_SPR | builtins.rs:137 | **BUILTIN** | ⚠️ UNKNOWN |
| `mask-weekend` | planner.rs:557-585 | SchemaOp::MSK_WKE_DEF | builtins.rs:130 | **BUILTIN** | ⚠️ UNKNOWN |
| `with-mask` | planner.rs:586-607 | SchemaOp::WTH_MSK | builtins.rs:131 | **BUILTIN** | ⚠️ UNKNOWN |

**Total Conflicts**: 18 tokens

### Execution Path in HYBRID Mode

**Control Flow** (eval.rs:81-91):
```rust
if self.is_builtin(*head_sym) {
    // Call builtin
    return self.call_builtin(*head_sym, &arg_vals);
}
// Try to resolve as variable (might be lambda)
self.resolve(*head_sym)?
```

**Result**: `is_builtin()` check happens **before** planner path is attempted.

**Evidence**: eval.rs:82-91 shows builtin check precedes variable resolution. Planner is never consulted in `eval_list()`.

---

## BEHAVIORAL EQUIVALENCE ANALYSIS

### Unknown Equivalence Status

**Problem**: Cannot determine if IR and builtin implementations are identical without:
1. Tracing builtin → kernel calls
2. Comparing kernel code with IR executor code
3. Running differential tests

**Risk Assessment**:

| Token | Risk Level | Reason |
|-------|------------|--------|
| `dlog` | 🔴 HIGH | Duplicate dlog_column definitions (exec.rs:1092 vs blawktrust) |
| `log`, `exp`, `abs` | 🟡 MEDIUM | Simple operations, likely identical |
| `+`, `-`, `*`, `/`, `>` | 🟡 MEDIUM | Basic arithmetic, likely identical |
| `locf` | 🟡 MEDIUM | Single implementation (exec.rs:1170), likely identical |
| `wkd` | 🔴 HIGH | Complex mask logic, different code paths |
| `shift` | 🔴 HIGH | IR uses exec.rs:1350, builtin uses builtins.rs:3273 wrapper |
| `mapr`, `asofr` | 🔴 HIGH | Join logic complex, frame.rs vs IR executor |
| `xminus` | 🟡 MEDIUM | Schema transformation, deterministic |
| `mask-weekend`, `with-mask` | 🔴 HIGH | Mask system stateful, different implementations |

---

## SUMMARY STATISTICS

| Category | Count | Notes |
|----------|-------|-------|
| **A: IR-Only** | 17 | Safe, single code path |
| **B: Builtin-Only** | 56 | Safe, single code path |
| **C: Conflict** | 18 | **CRITICAL BUG** - IR path dead |
| **TOTAL** | 91 | Unique tokens |

---

## CRITICAL FINDINGS

### 1. IR Path Completely Dead for 18 Tokens
- **Evidence**: eval.rs:82 builtin check precedes planner
- **Impact**: planner.rs:123-607 code never executes for conflict tokens
- **Cost**: ~500 lines dead code
- **Conflicts**: dlog, log, exp, abs, locf, wkd, cs1, shift, +, -, *, /, >, mapr, asofr, xminus, mask-weekend, with-mask

### 2. Behavioral Divergence Unknown
- **Cannot confirm** if IR and builtin paths produce identical results
- **High risk** tokens: dlog, wkd, shift, mapr, asofr, mask-*
- **Testing required** to verify equivalence

### 3. Architectural Inconsistency
- **No unified dispatch** mechanism
- **No explicit routing** control (no way to force IR path)
- **User confusion**: Token name gives no hint which path executes

---

## RECOMMENDATIONS

### Immediate Actions

1. **DELETE CONFLICT** - Choose one path per token:
   - Option A: Remove builtins.rs:63-147 registrations (force IR)
   - Option B: Remove planner.rs:123-607 mappings (force builtin)
   - Option C: Rename to explicit routing (e.g., `ir/dlog` vs `dlog`)

2. **BEHAVIORAL TESTS** - Verify equivalence before deletion:
   ```bash
   for token in dlog log exp abs locf wkd shift + - * / > mapr asofr xminus mask-weekend with-mask; do
       # Run same input through both paths
       # Compare outputs
   done
   ```

3. **DOCUMENT DECISION** - Update architecture docs with chosen model

---

**END OF INVENTORY**
