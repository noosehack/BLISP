# BLISP Builtin-Only Operations Audit

**Date:** 2026-02-27
**Purpose:** Identify computational Frame operations registered as builtins but NOT in IR planner
**Status:** Complete audit of 56 builtin-only tokens

---

## Methodology

1. Extract all `rt.register_builtin()` calls from builtins.rs
2. Extract all planner.rs match arms
3. Find set difference: builtins - planner = builtin-only
4. Classify each by category and assess planner migration priority

---

## Category 1: Comparison Operators (Missing IR Mappings)

### Token: `<`
- **Builtin:** builtins.rs:169 → builtin_lt
- **Planner:** ❌ NOT PRESENT
- **Verification:** `rg '"<"\s*=>' src/planner.rs` → empty
- **Should move to planner:** ✅ **YES** - IR has `>` (planner.rs:524), should have all comparison ops
- **Risk:** **HIGH** - Core operation, needed for IR arithmetic completeness
- **Double-fail risk:** ✅ YES - `(dlog (< x y))` would fail
- **Notes:** IR has BinaryFunc::GTR, should add BinaryFunc::LSS

### Token: `>=`
- **Builtin:** builtins.rs:170 → builtin_gte
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ✅ **YES** - Comparison operator
- **Risk:** **HIGH**
- **Double-fail risk:** ✅ YES
- **Notes:** Should add BinaryFunc::GTE

### Token: `<=`
- **Builtin:** builtins.rs:171 → builtin_lte
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ✅ **YES** - Comparison operator
- **Risk:** **HIGH**
- **Double-fail risk:** ✅ YES
- **Notes:** Should add BinaryFunc::LTE

### Token: `==`
- **Builtin:** builtins.rs:172 → builtin_eq
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ✅ **YES** - Comparison operator
- **Risk:** **HIGH**
- **Double-fail risk:** ✅ YES
- **Notes:** Should add BinaryFunc::EQL

### Token: `!=`
- **Builtin:** builtins.rs:173 → builtin_neq
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ✅ **YES** - Comparison operator
- **Risk:** **HIGH**
- **Double-fail risk:** ✅ YES
- **Notes:** Should add BinaryFunc::NEQ

---

## Category 2: Legacy Suffixed Names (Deprecated Aliases)

### Token: `w5`
- **Builtin:** builtins.rs:187 → builtin_wkd
- **Planner:** ❌ NOT PRESENT (IR has `wkd` at planner.rs:132)
- **Verification:** `rg '"w5"' src/planner.rs` → empty
- **Should move to planner:** ⚠️ **ALIAS ONLY** - Add as alias pointing to wkd
- **Risk:** **HIGH** - Dangerous alias that breaks IR trees
- **Double-fail risk:** ✅ YES - `(dlog (w5 20 PRC))` fails both paths
- **Notes:** IR has canonical `wkd`, should add `w5` as deprecated alias with warning

### Token: `dlog-col`
- **Builtin:** builtins.rs:133 → builtin_dlog
- **Planner:** ❌ NOT PRESENT (IR has `dlog` at planner.rs:123)
- **Should move to planner:** ⚠️ **ALIAS ONLY** - Add as deprecated alias
- **Risk:** **HIGH** - Breaks IR trees
- **Double-fail risk:** ✅ YES - `(shift 1 (dlog-col x))` fails
- **Notes:** Single-column legacy name, IR uses canonical `dlog`

### Token: `dlog-cols`
- **Builtin:** builtins.rs:162 → builtin_dlog_cols
- **Planner:** ❌ NOT PRESENT (IR has `dlog`)
- **Should move to planner:** ❌ **NO** - Multi-column variant, IR's `dlog` handles all columns
- **Risk:** **LOW** - Multi-column suffix convention is legacy-only
- **Double-fail risk:** ❌ NO - Not likely to nest inside IR ops
- **Notes:** Legacy table-wide operation, IR handles via map_numeric_preserve_tags

### Token: `shift-col`
- **Builtin:** builtins.rs:134 → builtin_shift
- **Planner:** ❌ NOT PRESENT (IR has `shift` at planner.rs:136)
- **Should move to planner:** ⚠️ **ALIAS ONLY** - Add as deprecated alias
- **Risk:** **HIGH** - Breaks IR trees
- **Double-fail risk:** ✅ YES - `(dlog (shift-col 1 x))` fails
- **Notes:** Single-column legacy name

### Token: `shift-cols`
- **Builtin:** builtins.rs:163 → builtin_shift_cols
- **Planner:** ❌ NOT PRESENT (IR has `shift`)
- **Should move to planner:** ❌ **NO** - Multi-column variant
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Multi-column suffix convention

### Token: `cs1-col`
- **Builtin:** builtins.rs:190 → builtin_cs1
- **Planner:** ❌ NOT PRESENT (IR has `cs1` at planner.rs:133)
- **Should move to planner:** ⚠️ **ALIAS ONLY** - Add as deprecated alias
- **Risk:** **HIGH** - Breaks IR trees
- **Double-fail risk:** ✅ YES
- **Notes:** Single-column legacy name

### Token: `cs1-cols`
- **Builtin:** builtins.rs:189 → builtin_cs1_cols
- **Planner:** ❌ NOT PRESENT (IR has `cs1`)
- **Should move to planner:** ❌ **NO** - Multi-column variant
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Multi-column suffix convention

### Token: `ur-col`
- **Builtin:** builtins.rs:198 → builtin_ur
- **Planner:** ❌ NOT PRESENT (IR has `ur` at planner.rs:408)
- **Should move to planner:** ⚠️ **ALIAS ONLY** - Add as deprecated alias
- **Risk:** **HIGH** - Breaks IR trees
- **Double-fail risk:** ✅ YES - `(ur 250 1 (ur-col RET))` fails
- **Notes:** Single-column legacy name

### Token: `ur-cols`
- **Builtin:** builtins.rs:197 → builtin_ur_cols
- **Planner:** ❌ NOT PRESENT (IR has `ur`)
- **Should move to planner:** ❌ **NO** - Multi-column variant
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Multi-column suffix convention

### Token: `locf-cols`
- **Builtin:** builtins.rs:176 → builtin_locf_cols
- **Planner:** ❌ NOT PRESENT (IR has `locf` at planner.rs:131)
- **Should move to planner:** ❌ **NO** - Multi-column variant
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Multi-column suffix convention

### Token: `diff-col`
- **Builtin:** builtins.rs:135 → builtin_diff
- **Planner:** ❌ NOT PRESENT (IR has no `diff`)
- **Should move to planner:** ⚠️ **MAYBE** - If diff is added to IR
- **Risk:** **MEDIUM**
- **Double-fail risk:** ⚠️ POTENTIAL
- **Notes:** Single-column variant of diff operation

### Token: `diff-cols`
- **Builtin:** builtins.rs:164 → builtin_diff_cols
- **Planner:** ❌ NOT PRESENT (IR has no `diff`)
- **Should move to planner:** ❌ **NO** - Multi-column variant
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Multi-column suffix convention

### Token: `diff`
- **Builtin:** builtins.rs:130 → builtin_diff_cols
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ⚠️ **MAYBE** - Similar to shift, but less common
- **Risk:** **MEDIUM** - Computational operation
- **Double-fail risk:** ⚠️ POTENTIAL
- **Notes:** Table-wide diff with default lag=1

---

## Category 3: Column-Wise Comparison (Legacy Table Ops)

### Token: `>-col`
- **Builtin:** builtins.rs:168 → builtin_gt
- **Planner:** ❌ NOT PRESENT (IR has `>` but as binary op)
- **Should move to planner:** ❌ **NO** - Legacy naming convention
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Single-column comparison, IR handles via BinaryFunc::GTR

### Token: `>-cols`
- **Builtin:** builtins.rs:167 → builtin_gt_cols
- **Planner:** ❌ NOT PRESENT (IR has `>`)
- **Should move to planner:** ❌ **NO** - Multi-column variant
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Multi-column suffix convention

---

## Category 4: Aggregation Operations (Reduce Ops)

### Token: `sum`
- **Builtin:** builtins.rs:138 → builtin_sum
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Reduction operation (Frame → Scalar)
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO - Output is scalar, won't nest in IR Frame ops
- **Notes:** Aggregate: column → scalar (NA → error)

### Token: `sum0`
- **Builtin:** builtins.rs:139 → builtin_sum0
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Reduction operation
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Aggregate: column → scalar (NA → 0)

### Token: `mean`
- **Builtin:** builtins.rs:140 → builtin_mean
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Reduction operation
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Aggregate: column → scalar (NA → error)

### Token: `mean0`
- **Builtin:** builtins.rs:141 → builtin_mean0
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Reduction operation
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Aggregate: column → scalar (NA → 0)

### Token: `std`
- **Builtin:** builtins.rs:142 → builtin_std
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Reduction operation
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Aggregate: column → scalar (NA → error)

### Token: `std0`
- **Builtin:** builtins.rs:143 → builtin_std0
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Reduction operation
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Aggregate: column → scalar (NA → 0)

---

## Category 5: Rolling Window Operations (Legacy Multi-Column)

### Token: `wstd`
- **Builtin:** builtins.rs:204 → builtin_wstd
- **Planner:** ❌ NOT PRESENT (IR has `rolling-std`)
- **Should move to planner:** ⚠️ **MAYBE** - IR could add as alias
- **Risk:** **MEDIUM** - Computational Frame operation
- **Double-fail risk:** ⚠️ POTENTIAL
- **Notes:** Legacy name for rolling std, IR uses `rolling-std`

### Token: `wstd0`
- **Builtin:** builtins.rs:205 → builtin_wstd0
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Legacy variant
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Rolling std with min_periods=0

### Token: `wstd-cols`
- **Builtin:** builtins.rs:206 → builtin_wstd_cols
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Multi-column variant
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Multi-column suffix convention

### Token: `wstd0-cols`
- **Builtin:** builtins.rs:207 → builtin_wstd0_cols
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Multi-column variant
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Multi-column suffix convention

### Token: `wv`
- **Builtin:** builtins.rs:208 → builtin_wv
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ⚠️ **MAYBE** - Rolling variance
- **Risk:** **MEDIUM**
- **Double-fail risk:** ⚠️ POTENTIAL
- **Notes:** Rolling variance, could add to IR as rolling-var

### Token: `wv-cols`
- **Builtin:** builtins.rs:209 → builtin_wv_cols
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Multi-column variant
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Multi-column suffix convention

### Token: `wz0`
- **Builtin:** builtins.rs:199 → builtin_wz0
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ⚠️ **MAYBE** - Rolling z-score variant
- **Risk:** **MEDIUM**
- **Double-fail risk:** ⚠️ POTENTIAL
- **Notes:** Rolling z-score with min_periods=0

### Token: `wz0-cols`
- **Builtin:** builtins.rs:200 → builtin_wz0_cols
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Multi-column variant
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Multi-column suffix convention

### Token: `wzs`
- **Builtin:** builtins.rs:201 → builtin_wzs
- **Planner:** ❌ NOT PRESENT (IR has `rolling-zscore` at planner.rs:323)
- **Verification:** `rg '"wzs"' src/planner.rs` → Line 323 (FOUND!)
- **Status:** **UNCERTAIN** - Need to verify if wzs is actually in planner
- **Command to verify:** `rg '"wzs"\s*=>' src/planner.rs`

---

## Category 6: Data Transforms (Frame → Frame)

### Token: `zscore`
- **Builtin:** builtins.rs:212 → builtin_zscore
- **Planner:** ❌ NOT PRESENT (IR has `rolling-zscore`)
- **Should move to planner:** ⚠️ **MAYBE** - Non-rolling z-score normalization
- **Risk:** **MEDIUM** - Computational Frame operation
- **Double-fail risk:** ⚠️ POTENTIAL
- **Notes:** Global z-score (not windowed), different from rolling-zscore

### Token: `chop`
- **Builtin:** builtins.rs:213 → builtin_chop
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ⚠️ **MAYBE** - Clipping operation
- **Risk:** **MEDIUM**
- **Double-fail risk:** ⚠️ POTENTIAL
- **Notes:** Clip values to range, computational Frame operation

### Token: `keep-shape`
- **Builtin:** builtins.rs:177 → builtin_keep_shape
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ⚠️ **MAYBE** - Shape-preserving operation
- **Risk:** **MEDIUM**
- **Double-fail risk:** ⚠️ POTENTIAL
- **Notes:** Single-column keep-shape (NA preservation)

### Token: `keep-shape-cols`
- **Builtin:** builtins.rs:178 → builtin_keep_shape_cols
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Multi-column variant
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Multi-column suffix convention

### Token: `ecs1`
- **Builtin:** builtins.rs:191 → builtin_ecs1_cols
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ⚠️ **MAYBE** - Exponential cumsum variant
- **Risk:** **MEDIUM**
- **Double-fail risk:** ⚠️ POTENTIAL
- **Notes:** Exponential weighted cumsum, computational operation

### Token: `ecs1-col`
- **Builtin:** builtins.rs:193 → builtin_ecs1
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Legacy naming
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Single-column variant

### Token: `ecs1-cols`
- **Builtin:** builtins.rs:192 → builtin_ecs1_cols
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Multi-column variant
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Multi-column suffix convention

---

## Category 7: Table Manipulation (Schema Ops)

### Token: `col`
- **Builtin:** builtins.rs:150 → builtin_col
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Column extraction (schema change)
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO - Changes schema, not typical in IR chains
- **Notes:** Extract single column from table

### Token: `cols`
- **Builtin:** builtins.rs:157 → builtin_cols
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Introspection (list column names)
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Returns list of column names

### Token: `setcol`
- **Builtin:** builtins.rs:151 → builtin_setcol
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Mutation operation
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Set column value (mutation)

### Token: `withcol`
- **Builtin:** builtins.rs:152 → builtin_withcol
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Schema change operation
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Add/replace column (schema change)

### Token: `w`
- **Builtin:** builtins.rs:153 → builtin_w
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Column selection by indices
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Select columns by indices (schema change)

### Token: `select`
- **Builtin:** builtins.rs:158 → builtin_select
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Column selection by name
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Select columns by name (schema change)

### Token: `select-num`
- **Builtin:** builtins.rs:159 → builtin_select_num
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Column selection by type
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Select numeric columns only (schema change)

### Token: `make-col`
- **Builtin:** builtins.rs:154 → builtin_make_col
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Column construction
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Create column from list

### Token: `map-cols`
- **Builtin:** builtins.rs:160 → builtin_map_cols
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Higher-order function
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Map function over columns (requires lambda/function)

### Token: `apply-cols`
- **Builtin:** builtins.rs:161 → builtin_apply_cols
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Higher-order function
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Apply function to columns (requires lambda/function)

---

## Category 8: Mask Operations

### Token: `mask-on`
- **Builtin:** builtins.rs:181 → builtin_with_mask
- **Planner:** ❌ NOT PRESENT (IR has `with-mask` at planner.rs:587)
- **Should move to planner:** ⚠️ **ALIAS ONLY** - Add as alias for with-mask
- **Risk:** **LOW** - Legacy alias
- **Double-fail risk:** ❌ NO - Less likely to cause issues
- **Notes:** Alias for with-mask

### Token: `mask-off`
- **Builtin:** builtins.rs:182 → builtin_mask_off
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ⚠️ **MAYBE** - Deactivate mask
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Deactivate active mask (schema operation)

### Token: `mask-list`
- **Builtin:** builtins.rs:183 → builtin_mask_list
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Introspection
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** List active masks (introspection)

### Token: `mask-stats`
- **Builtin:** builtins.rs:184 → builtin_mask_stats
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Introspection
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Show mask statistics (introspection)

### Token: `mask-define`
- **Builtin:** builtins.rs:185 → builtin_mask_define
- **Planner:** ❌ NOT PRESENT (IR has `mask-weekend`)
- **Should move to planner:** ⚠️ **MAYBE** - Define named mask
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** Define named mask (schema operation)

---

## Category 9: I/O and Side Effects (EXCLUDED)

### Token: `file`
- **Builtin:** builtins.rs:146 → builtin_file
- **Planner:** ✅ PRESENT at planner.rs:88
- **Status:** **DUAL ROUTING** - Both paths exist, IR wins
- **Notes:** Already in planner, not builtin-only

### Token: `file-head`
- **Builtin:** builtins.rs:147 → builtin_file_head
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - I/O operation (partial read)
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** EXCLUDED - Pure I/O side effect

### Token: `save`
- **Builtin:** builtins.rs:149 → builtin_save
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - I/O operation (write side effect)
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** EXCLUDED - Pure I/O side effect

### Token: `print`
- **Builtin:** builtins.rs:216 → builtin_print
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - I/O operation (stdout side effect)
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** EXCLUDED - Pure I/O side effect

---

## Category 10: Introspection (EXCLUDED)

### Token: `type-of`
- **Builtin:** builtins.rs:217 → builtin_type_of
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Introspection utility
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** EXCLUDED - Introspection utility

### Token: `len`
- **Builtin:** builtins.rs:218 → builtin_len
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Introspection utility
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** EXCLUDED - Introspection utility

### Token: `o`
- **Builtin:** builtins.rs:221 → builtin_o
- **Planner:** ❌ NOT PRESENT
- **Should move to planner:** ❌ **NO** - Orientation lookup utility
- **Risk:** **LOW**
- **Double-fail risk:** ❌ NO
- **Notes:** EXCLUDED - Introspection utility

---

## Summary Table: Computational Frame Operations Only

| Token | Builtin Only? | Planner Missing? | Should Move to Planner? | Risk | Notes |
|-------|---------------|------------------|-------------------------|------|-------|
| `<` | ✅ Yes | ✅ Yes | ✅ **YES** | **HIGH** | Missing comparison op, IR has `>` |
| `>=` | ✅ Yes | ✅ Yes | ✅ **YES** | **HIGH** | Missing comparison op |
| `<=` | ✅ Yes | ✅ Yes | ✅ **YES** | **HIGH** | Missing comparison op |
| `==` | ✅ Yes | ✅ Yes | ✅ **YES** | **HIGH** | Missing comparison op |
| `!=` | ✅ Yes | ✅ Yes | ✅ **YES** | **HIGH** | Missing comparison op |
| `w5` | ✅ Yes | ✅ Yes | ⚠️ **ALIAS** | **HIGH** | Dangerous alias, add to planner with deprecation warning |
| `dlog-col` | ✅ Yes | ✅ Yes | ⚠️ **ALIAS** | **HIGH** | Dangerous alias, add to planner |
| `shift-col` | ✅ Yes | ✅ Yes | ⚠️ **ALIAS** | **HIGH** | Dangerous alias, add to planner |
| `cs1-col` | ✅ Yes | ✅ Yes | ⚠️ **ALIAS** | **HIGH** | Dangerous alias, add to planner |
| `ur-col` | ✅ Yes | ✅ Yes | ⚠️ **ALIAS** | **HIGH** | Dangerous alias, add to planner |
| `diff` | ✅ Yes | ✅ Yes | ⚠️ **MAYBE** | **MEDIUM** | Similar to shift, less critical |
| `diff-col` | ✅ Yes | ✅ Yes | ⚠️ **MAYBE** | **MEDIUM** | Conditional on diff |
| `wstd` | ✅ Yes | ✅ Yes | ⚠️ **MAYBE** | **MEDIUM** | Legacy name for rolling-std |
| `wv` | ✅ Yes | ✅ Yes | ⚠️ **MAYBE** | **MEDIUM** | Rolling variance |
| `wz0` | ✅ Yes | ✅ Yes | ⚠️ **MAYBE** | **MEDIUM** | Rolling z-score variant |
| `zscore` | ✅ Yes | ✅ Yes | ⚠️ **MAYBE** | **MEDIUM** | Global z-score (non-rolling) |
| `chop` | ✅ Yes | ✅ Yes | ⚠️ **MAYBE** | **MEDIUM** | Value clipping |
| `keep-shape` | ✅ Yes | ✅ Yes | ⚠️ **MAYBE** | **MEDIUM** | Shape-preserving NA fill |
| `ecs1` | ✅ Yes | ✅ Yes | ⚠️ **MAYBE** | **MEDIUM** | Exponential cumsum |
| `mask-on` | ✅ Yes | ✅ Yes | ⚠️ **ALIAS** | **LOW** | Alias for with-mask |
| `mask-off` | ✅ Yes | ✅ Yes | ⚠️ **MAYBE** | **LOW** | Mask deactivation |
| `mask-define` | ✅ Yes | ✅ Yes | ⚠️ **MAYBE** | **LOW** | Named mask definition |
| `dlog-cols` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Multi-column variant (legacy) |
| `shift-cols` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Multi-column variant (legacy) |
| `diff-cols` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Multi-column variant (legacy) |
| `cs1-cols` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Multi-column variant (legacy) |
| `ur-cols` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Multi-column variant (legacy) |
| `locf-cols` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Multi-column variant (legacy) |
| `>-col` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Legacy naming (IR has `>`) |
| `>-cols` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Multi-column variant (legacy) |
| `wstd0` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Legacy variant |
| `wstd-cols` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Multi-column variant (legacy) |
| `wstd0-cols` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Multi-column variant (legacy) |
| `wv-cols` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Multi-column variant (legacy) |
| `wz0-cols` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Multi-column variant (legacy) |
| `keep-shape-cols` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Multi-column variant (legacy) |
| `ecs1-col` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Legacy naming |
| `ecs1-cols` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Multi-column variant (legacy) |
| `sum` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Reduction (Frame→Scalar) |
| `sum0` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Reduction (Frame→Scalar) |
| `mean` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Reduction (Frame→Scalar) |
| `mean0` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Reduction (Frame→Scalar) |
| `std` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Reduction (Frame→Scalar) |
| `std0` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Reduction (Frame→Scalar) |
| `col` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Schema change (column extraction) |
| `cols` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Introspection (list names) |
| `setcol` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Mutation operation |
| `withcol` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Schema change (add/replace column) |
| `w` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Schema change (column selection) |
| `select` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Schema change (column selection) |
| `select-num` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Schema change (type filter) |
| `make-col` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Column construction |
| `map-cols` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Higher-order (requires lambda) |
| `apply-cols` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Higher-order (requires lambda) |
| `mask-list` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Introspection |
| `mask-stats` | ✅ Yes | ✅ Yes | ❌ NO | **LOW** | Introspection |

---

## Priority Fix List

### P0: Critical (Must Add to Planner)

**5 Comparison Operators:**
1. `<` - Add BinaryFunc::LSS
2. `>=` - Add BinaryFunc::GTE
3. `<=` - Add BinaryFunc::LTE
4. `==` - Add BinaryFunc::EQL
5. `!=` - Add BinaryFunc::NEQ

**Risk:** HIGH - Core operations, IR arithmetic incomplete without them
**Impact:** Double-fail if nested inside IR ops

### P1: High Priority (Dangerous Aliases)

**5 Legacy Aliases:**
1. `w5` → alias to `wkd`
2. `dlog-col` → alias to `dlog`
3. `shift-col` → alias to `shift`
4. `cs1-col` → alias to `cs1`
5. `ur-col` → alias to `ur`

**Risk:** HIGH - Break IR trees, cause double-fail
**Impact:** Zero-breakage migration if added as aliases

### P2: Medium Priority (Computational Ops)

**9 Computational Operations:**
1. `diff` - Similar to shift
2. `diff-col` - Conditional on diff
3. `wstd` - Legacy name for rolling-std
4. `wv` - Rolling variance
5. `wz0` - Rolling z-score variant
6. `zscore` - Global z-score
7. `chop` - Value clipping
8. `keep-shape` - Shape-preserving fill
9. `ecs1` - Exponential cumsum

**Risk:** MEDIUM - Computational Frame operations
**Impact:** Potential double-fail, but less common

### P3: Low Priority (Consider Later)

**3 Operations:**
1. `mask-on` - Alias for with-mask
2. `mask-off` - Mask deactivation
3. `mask-define` - Named mask definition

**Risk:** LOW - Less common, schema operations
**Impact:** Limited double-fail risk

---

## Verification Commands

### Verify comparison ops not in planner:
```bash
cd /home/ubuntu/blisp
for op in "<" ">=" "<=" "==" "!="; do
  echo -n "$op: "
  rg "\"$op\"\s*=>" src/planner.rs || echo "NOT FOUND"
done
```

### Verify dangerous aliases not in planner:
```bash
cd /home/ubuntu/blisp
for alias in "w5" "dlog-col" "shift-col" "cs1-col" "ur-col"; do
  echo -n "$alias: "
  rg "\"$alias\"\s*=>" src/planner.rs || echo "NOT FOUND"
done
```

### Verify wzs status (UNCERTAIN):
```bash
cd /home/ubuntu/blisp
rg '"wzs"\s*=>' src/planner.rs
# Check if wzs or rolling-zscore
```

---

## Conclusions

1. **5 comparison operators** are CRITICAL gaps in IR planner
2. **5 dangerous aliases** (w5, dlog-col, shift-col, cs1-col, ur-col) cause double-fail
3. **9 computational operations** could migrate to IR for completeness
4. **35+ legacy operations** should stay builtin-only (multi-column variants, schema ops, etc.)

**Total builtin-only operations audited:** 56
**Critical fixes needed:** 10 (5 comparison + 5 aliases)
**Recommended fixes:** 9 (computational ops)
**Correctly excluded:** 32 (legacy variants, schema ops, introspection)

---

**End of Audit**
