# BLISP Operation Dispatch Tables

**Date:** 2026-02-27
**Repository:** /home/ubuntu/blisp
**Branch:** reconstruct/tableview-only

---

## Table 1: Macros

| surface_head | expands_to | canonical_intent | where_defined (file:line) |
|--------------|------------|------------------|---------------------------|
| `->` | Thread-first: `(-> x (f a) (g b))` → `(g (f x a) b)` | Pipeline composition (thread value as first argument) | normalize.rs:55-105 |

**Notes:**
- Only one macro currently implemented in normalize layer
- Macro expansion happens BEFORE both IR and legacy evaluation
- Macros are checked in normalize.rs:53-57 by matching symbol name
- Thread-first is idempotent: normalize(normalize(x)) == normalize(x)

---

## Table 2: Builtins (Legacy Evaluator)

| surface_head | builtin_symbol | builtin_fn | registration_site | notes |
|--------------|----------------|------------|-------------------|-------|
| `+` | + | builtin_add | builtins.rs:121 | Arithmetic |
| `-` | - | builtin_sub | builtins.rs:122 | Arithmetic |
| `*` | * | builtin_mul | builtins.rs:123 | Arithmetic |
| `/` | / | builtin_div | builtins.rs:124 | Arithmetic |
| `>` | > | builtin_gt | builtins.rs:125 | Comparison |
| `<` | < | builtin_lt | builtins.rs:169 | Comparison |
| `>=` | >= | builtin_gte | builtins.rs:170 | Comparison |
| `<=` | <= | builtin_lte | builtins.rs:171 | Comparison |
| `==` | == | builtin_eq | builtins.rs:172 | Comparison |
| `!=` | != | builtin_neq | builtins.rs:173 | Comparison |
| `>-col` | >-col | builtin_gt | builtins.rs:168 | Single-column comparison |
| `>-cols` | >-cols | builtin_gt_cols | builtins.rs:167 | Multi-column comparison |
| `diff` | diff | builtin_diff_cols | builtins.rs:130 | Table version (default lag=1) |
| `dlog-col` | dlog-col | builtin_dlog | builtins.rs:133 | Single-column diff log |
| `shift-col` | shift-col | builtin_shift | builtins.rs:134 | Single-column shift |
| `diff-col` | diff-col | builtin_diff | builtins.rs:135 | Single-column diff |
| `sum` | sum | builtin_sum | builtins.rs:138 | Aggregate (NA → error) |
| `sum0` | sum0 | builtin_sum0 | builtins.rs:139 | Aggregate (NA → 0) |
| `mean` | mean | builtin_mean | builtins.rs:140 | Aggregate (NA → error) |
| `mean0` | mean0 | builtin_mean0 | builtins.rs:141 | Aggregate (NA → 0) |
| `std` | std | builtin_std | builtins.rs:142 | Aggregate (NA → error) |
| `std0` | std0 | builtin_std0 | builtins.rs:143 | Aggregate (NA → 0) |
| `file` | file | builtin_file | builtins.rs:146 | Load CSV file |
| `file-head` | file-head | builtin_file_head | builtins.rs:147 | Load CSV (first N rows) |
| `stdin` | stdin | builtin_stdin | builtins.rs:148 | Read CSV from stdin |
| `save` | save | builtin_save | builtins.rs:149 | Save table to CSV |
| `col` | col | builtin_col | builtins.rs:150 | Extract column |
| `setcol` | setcol | builtin_setcol | builtins.rs:151 | Set column value |
| `withcol` | withcol | builtin_withcol | builtins.rs:152 | Add/replace column |
| `w` | w | builtin_w | builtins.rs:153 | Select columns by indices |
| `make-col` | make-col | builtin_make_col | builtins.rs:154 | Create column from list |
| `cols` | cols | builtin_cols | builtins.rs:157 | List column names |
| `select` | select | builtin_select | builtins.rs:158 | Select columns by name |
| `select-num` | select-num | builtin_select_num | builtins.rs:159 | Select numeric columns |
| `map-cols` | map-cols | builtin_map_cols | builtins.rs:160 | Map function over columns |
| `apply-cols` | apply-cols | builtin_apply_cols | builtins.rs:161 | Apply function to columns |
| `dlog-cols` | dlog-cols | builtin_dlog_cols | builtins.rs:162 | Multi-column diff log |
| `shift-cols` | shift-cols | builtin_shift_cols | builtins.rs:163 | Multi-column shift |
| `diff-cols` | diff-cols | builtin_diff_cols | builtins.rs:164 | Multi-column diff |
| `locf-cols` | locf-cols | builtin_locf_cols | builtins.rs:176 | Multi-column LOCF |
| `keep-shape` | keep-shape | builtin_keep_shape | builtins.rs:177 | Single-column keep-shape |
| `keep-shape-cols` | keep-shape-cols | builtin_keep_shape_cols | builtins.rs:178 | Multi-column keep-shape |
| `mask-on` | mask-on | builtin_with_mask | builtins.rs:181 | Alias for with-mask |
| `mask-off` | mask-off | builtin_mask_off | builtins.rs:182 | Deactivate mask |
| `mask-list` | mask-list | builtin_mask_list | builtins.rs:183 | List active masks |
| `mask-stats` | mask-stats | builtin_mask_stats | builtins.rs:184 | Show mask statistics |
| `mask-define` | mask-define | builtin_mask_define | builtins.rs:185 | Define named mask |
| `wkd` | wkd | builtin_wkd | builtins.rs:186 | Weekend filter (w5) |
| `w5` | w5 | builtin_wkd | builtins.rs:187 | Alias for wkd (backward compat) |
| `xminus` | xminus | builtin_xminus | builtins.rs:188 | Pairwise spreads |
| `cs1-cols` | cs1-cols | builtin_cs1_cols | builtins.rs:189 | Multi-column cumsum |
| `cs1-col` | cs1-col | builtin_cs1 | builtins.rs:190 | Single-column cumsum |
| `ecs1` | ecs1 | builtin_ecs1_cols | builtins.rs:191 | Surface → table version |
| `ecs1-cols` | ecs1-cols | builtin_ecs1_cols | builtins.rs:192 | Multi-column ECS1 |
| `ecs1-col` | ecs1-col | builtin_ecs1 | builtins.rs:193 | Single-column ECS1 |
| `mapr` | mapr | builtin_mapr | builtins.rs:196 | LEFT JOIN by row key |
| `asofr` | asofr | builtin_asofr | builtins.rs:?? | ASOF JOIN (not in visible registrations) |
| `ur-cols` | ur-cols | builtin_ur_cols | builtins.rs:197 | Multi-column UR |
| `ur-col` | ur-col | builtin_ur | builtins.rs:198 | Single-column UR |
| `wz0` | wz0 | builtin_wz0 | builtins.rs:199 | Single-column wz0 |
| `wz0-cols` | wz0-cols | builtin_wz0_cols | builtins.rs:200 | Multi-column wz0 |
| `wzs` | wzs | builtin_wzs | builtins.rs:201 | Composite: locf(keep-shape(wz0)) |
| `wstd` | wstd | builtin_wstd | builtins.rs:204 | Rolling std (single-column) |
| `wstd0` | wstd0 | builtin_wstd0 | builtins.rs:205 | Rolling std0 (single-column) |
| `wstd-cols` | wstd-cols | builtin_wstd_cols | builtins.rs:206 | Multi-column rolling std |
| `wstd0-cols` | wstd0-cols | builtin_wstd0_cols | builtins.rs:207 | Multi-column rolling std0 |
| `wv` | wv | builtin_wv | builtins.rs:208 | Rolling variance (single-column) |
| `wv-cols` | wv-cols | builtin_wv_cols | builtins.rs:209 | Multi-column rolling variance |
| `zscore` | zscore | builtin_zscore | builtins.rs:212 | Z-score normalization |
| `chop` | chop | builtin_chop | builtins.rs:213 | Chop values to range |
| `print` | print | builtin_print | builtins.rs:216 | Print value |
| `type-of` | type-of | builtin_type_of | builtins.rs:217 | Get type name |
| `len` | len | builtin_len | builtins.rs:218 | Get length |
| `o` | o | builtin_o | builtins.rs:221 | Orientation lookup |

**Notes:**
- Total: 71 registered builtins
- Naming pattern: `-col` suffix = single-column, `-cols` suffix = multi-column
- Legacy path selected when IR fails with "Cannot plan" or for non-Frame types
- All builtins checked at eval.rs:82 via `is_builtin()`
- Builtins called at eval.rs:90 via `call_builtin()`

---

## Table 3: IR Planner Mappings

| surface_head | planner_token | IR enum variant | exec kernel | notes |
|--------------|---------------|-----------------|-------------|-------|
| `file` | "file" | Source::File | io::load_csv() | planner.rs:88, exec.rs:84 |
| `stdin` | "stdin" | Source::Stdin | io::parse_csv_to_frame() | planner.rs:108, exec.rs:93 |
| `dlog` | "dlog" | NumericFunc::SHF_PTW_OBS_NLN_DLOG | dlog_obs_column() | planner.rs:123, exec.rs:157 (OBS: NA-skipping) |
| `dlog-ofs` | "dlog-ofs" | NumericFunc::SHF_PTW_OFS_NLN_DLOG | dlog_ofs_column() | planner.rs:124, exec.rs:158 (OFS: positional) |
| `ret` | "ret" | NumericFunc::RET | ret_column() | planner.rs:125, exec.rs:159 |
| `log` | "log" | NumericFunc::LOG | log_column() | planner.rs:126, exec.rs:160 |
| `exp` | "exp" | NumericFunc::EXP | exp_column() | planner.rs:127, exec.rs:161 |
| `sqrt` | "sqrt" | NumericFunc::SQRT | sqrt_column() | planner.rs:128, exec.rs:162 |
| `abs` | "abs" | NumericFunc::ABS | abs_column() | planner.rs:129, exec.rs:163 |
| `inv` | "inv" | NumericFunc::INV | inv_column() | planner.rs:130, exec.rs:164 |
| `locf` | "locf" | NumericFunc::SHF_REC_NLN_LOCF | locf_column() | planner.rs:131, exec.rs:165 |
| `wkd` | "wkd" | NumericFunc::MSK_WKE | wkd_mask_weekends() | planner.rs:132, exec.rs:134 |
| `cs1` | "cs1" | NumericFunc::SHF_PFX_LIN_SUM | cumsum_column() | planner.rs:133, exec.rs:166 |
| `shift` | "shift" | NumericFunc::SHF_PTW_LIN_SHF{k} | shift_column() | planner.rs:136, exec.rs:167 |
| `lag-obs` | "lag-obs" | NumericFunc::LAG_OBS{k} | apply_shift_obs_mask_aware() | planner.rs:154, exec.rs:148 |
| `shift-obs` | "shift-obs" | NumericFunc::LAG_OBS{k} | apply_shift_obs_mask_aware() | planner.rs:154, exec.rs:148 (alias) |
| `keep` | "keep" | NumericFunc::KEEP{k} | keep_column() | planner.rs:171, exec.rs:168 |
| `rolling-mean` | "rolling-mean" | NumericFunc::SHF_WIN_LIN_AVG{w} | apply_rolling_mask_aware() | planner.rs:188, exec.rs:145 |
| `rolling-mean-min2` | "rolling-mean-min2" | NumericFunc::SHF_WIN_MIN2_LIN_AVG{w} | apply_rolling_mask_aware() | planner.rs:205, exec.rs:145 |
| `ft-mean` | "ft-mean" | shift(1, SHF_WIN_LIN_AVG{w}) | Composite (rewrite) | planner.rs:224 (rewrite) |
| `rolling-std` | "rolling-std" | NumericFunc::SHF_WIN_NLN_SDV{w} | apply_rolling_mask_aware() | planner.rs:256, exec.rs:145 |
| `rolling-std-min2` | "rolling-std-min2" | NumericFunc::SHF_WIN_MIN2_NLN_SDV{w} | apply_rolling_mask_aware() | planner.rs:274, exec.rs:145 |
| `ft-std` | "ft-std" | shift(1, SHF_WIN_NLN_SDV{w}) | Composite (rewrite) | planner.rs:291 (rewrite) |
| `rolling-zscore` | "rolling-zscore" | Composite: (x-mean)/std | Fused in planner | planner.rs:323 (derived) |
| `wzs` | "wzs" | Composite: (x-mean)/std | Fused in planner | planner.rs:323 (CLISPI compat) |
| `ur` | "ur" | NumericFunc::SHF_WIN_NLN_UR{w} | apply_rolling_mask_aware() | planner.rs:408, exec.rs:145 |
| `+` | "+" | BinaryFunc::ADD | add_scalar/add_columns() | planner.rs:520, exec.rs:350 |
| `-` | "-" | BinaryFunc::SUB | sub_scalar/sub_columns() | planner.rs:521, exec.rs:350 |
| `*` | "*" | BinaryFunc::MUL | mul_scalar/mul_columns() | planner.rs:522, exec.rs:350 |
| `/` | "/" | BinaryFunc::DIV | div_scalar/div_columns() | planner.rs:523, exec.rs:350 |
| `>` | ">" | BinaryFunc::GTR | gt_columns() | planner.rs:524, exec.rs:350 |
| `mapr` | "mapr" | JoinOp::ALIGN | reindex_by() | planner.rs:527, exec.rs:420 |
| `asofr` | "asofr" | JoinOp::ASOF_ALIGN | asofr() | planner.rs:528, exec.rs:446 |
| `xminus` | "xminus" | SchemaOp::SHF_PTW_LIN_SPR{half} | xminus_columns() | planner.rs:531, exec.rs:507 |
| `mask-weekend` | "mask-weekend" | SchemaOp::MSK_WKE_DEF{name} | mask_weekend_define() | planner.rs:558, exec.rs:550 |
| `with-mask` | "with-mask" | SchemaOp::WTH_MSK{mask_expr} | with_mask_apply() | planner.rs:587, exec.rs:616 |
| `let` | "let" | IR bindings (ctx.bind) | N/A (planning context) | planner.rs:609 |

**Notes:**
- Total: 36 IR-recognized operations
- IR path selected in HYBRID mode (default) when input is Frame/TableView
- Planner validates schema at plan time (before execution)
- All unary ops use map_numeric_preserve_tags() (exec.rs:155)
- Rolling ops need active_mask for eligible observation counting (exec.rs:138-147)
- Composite ops (ft-mean, wzs) are rewritten in planner, not IR primitives
- Schema ops (xminus, mask-weekend, with-mask) rebuild colnames (I2_schema)

---

## Reachability Analysis: Specific Tokens

### Legend
- ✅ = Reachable
- ❌ = Not reachable / Missing
- 🔀 = Has both paths (dual dispatch)

| Token | IR Path | Builtin Path | Macro Path | Status | Notes |
|-------|---------|--------------|------------|--------|-------|
| **file** | ✅ | ✅ | ❌ | 🔀 DUAL | IR preferred, legacy fallback |
| **stdin** | ✅ | ✅ | ❌ | 🔀 DUAL | IR preferred, legacy fallback |
| **print** | ❌ | ✅ | ❌ | Legacy-only | No IR path (I/O operation) |
| **save** | ❌ | ✅ | ❌ | Legacy-only | No IR path (I/O operation) |
| **file-head** | ❌ | ✅ | ❌ | Legacy-only | No IR path (I/O operation) |
| **w5** | ❌ | ✅ | ❌ | Legacy-only | Alias for wkd (backward compat) |
| **dlog** | ✅ | ❌ | ❌ | IR-only | NO legacy builtin! (only dlog-col exists) |
| **shift** | ✅ | ❌ | ❌ | IR-only | NO legacy builtin! (only shift-col exists) |
| **locf** | ✅ | ❌ | ❌ | IR-only | NO legacy builtin! (only locf-cols exists) |
| **cs1** | ✅ | ❌ | ❌ | IR-only | NO legacy builtin! (only cs1-col exists) |
| **mapr** | ✅ | ✅ | ❌ | 🔀 DUAL | IR preferred, legacy fallback |
| **asofr** | ✅ | ❌ | ❌ | IR-only | NO visible legacy registration (may exist) |
| **ur** | ✅ | ❌ | ❌ | IR-only | NO legacy builtin! (only ur-col exists) |
| **xminus** | ✅ | ✅ | ❌ | 🔀 DUAL | IR preferred, legacy fallback |
| **mask-weekend** | ✅ | ❌ | ❌ | IR-only | NO legacy builtin |
| **with-mask** | ✅ | ✅ | ❌ | 🔀 DUAL | IR preferred, legacy fallback (mask-on) |

---

## Detailed Analysis by Token

### 1. `file`
- **IR:** ✅ planner.rs:88 → Source::File → exec.rs:84 io::load_csv()
- **Builtin:** ✅ builtins.rs:146 builtin_file
- **Macro:** ❌
- **Status:** 🔀 DUAL PATH (IR preferred)
- **Selection:** Frame-returning operations prefer IR

### 2. `stdin`
- **IR:** ✅ planner.rs:108 → Source::Stdin → exec.rs:93 parse_csv_to_frame()
- **Builtin:** ✅ builtins.rs:148 builtin_stdin
- **Macro:** ❌
- **Status:** 🔀 DUAL PATH (IR preferred)
- **Selection:** Frame-returning operations prefer IR

### 3. `print`
- **IR:** ❌ NOT FOUND
- **Builtin:** ✅ builtins.rs:216 builtin_print
- **Macro:** ❌
- **Status:** Legacy-only (I/O operation, side-effect)
- **Reason:** IR focuses on Frame transformations, not side effects

### 4. `save`
- **IR:** ❌ NOT FOUND
- **Builtin:** ✅ builtins.rs:149 builtin_save
- **Macro:** ❌
- **Status:** Legacy-only (I/O operation, side-effect)
- **Reason:** IR focuses on Frame transformations, not side effects

### 5. `file-head`
- **IR:** ❌ NOT FOUND
- **Builtin:** ✅ builtins.rs:147 builtin_file_head
- **Macro:** ❌
- **Status:** Legacy-only (I/O operation, partial read)
- **Reason:** IR does not support partial file reading yet

### 6. `w5`
- **IR:** ❌ NOT FOUND (only "wkd" recognized)
- **Builtin:** ✅ builtins.rs:187 builtin_wkd (alias)
- **Macro:** ❌
- **Status:** Legacy-only (backward compatibility alias)
- **Reason:** IR uses canonical name "wkd", not alias "w5"

### 7. `dlog`
- **IR:** ✅ planner.rs:123 → SHF_PTW_OBS_NLN_DLOG → exec.rs:157 dlog_obs_column()
- **Builtin:** ❌ NOT REGISTERED (only dlog-col exists)
- **Macro:** ❌
- **Status:** IR-only (new canonical name)
- **Reason:** Clean name for IR, legacy uses dlog-col suffix

### 8. `shift`
- **IR:** ✅ planner.rs:136 → SHF_PTW_LIN_SHF → exec.rs:167 shift_column()
- **Builtin:** ❌ NOT REGISTERED (only shift-col exists)
- **Macro:** ❌
- **Status:** IR-only (new canonical name)
- **Reason:** Clean name for IR, legacy uses shift-col suffix

### 9. `locf`
- **IR:** ✅ planner.rs:131 → SHF_REC_NLN_LOCF → exec.rs:165 locf_column()
- **Builtin:** ❌ NOT REGISTERED (only locf-cols exists)
- **Macro:** ❌
- **Status:** IR-only (new canonical name)
- **Reason:** Clean name for IR, legacy uses locf-cols suffix

### 10. `cs1`
- **IR:** ✅ planner.rs:133 → SHF_PFX_LIN_SUM → exec.rs:166 cumsum_column()
- **Builtin:** ❌ NOT REGISTERED (only cs1-col exists)
- **Macro:** ❌
- **Status:** IR-only (new canonical name)
- **Reason:** Clean name for IR, legacy uses cs1-col suffix

### 11. `mapr`
- **IR:** ✅ planner.rs:527 → JoinOp::ALIGN → exec.rs:420 reindex_by()
- **Builtin:** ✅ builtins.rs:196 builtin_mapr
- **Macro:** ❌
- **Status:** 🔀 DUAL PATH (IR preferred)
- **Selection:** Frame operations prefer IR

### 12. `asofr`
- **IR:** ✅ planner.rs:528 → JoinOp::ASOF_ALIGN → exec.rs:446 asofr()
- **Builtin:** ❌ NOT VISIBLE in registration list
- **Macro:** ❌
- **Status:** IR-only (or missing legacy registration)
- **Reason:** Join operation optimized in IR

### 13. `ur`
- **IR:** ✅ planner.rs:408 → SHF_WIN_NLN_UR → exec.rs:145 (rolling)
- **Builtin:** ❌ NOT REGISTERED (only ur-col exists)
- **Macro:** ❌
- **Status:** IR-only (new canonical name)
- **Reason:** Clean name for IR, legacy uses ur-col suffix

### 14. `xminus`
- **IR:** ✅ planner.rs:531 → SchemaOp::SHF_PTW_LIN_SPR → exec.rs:507 xminus_columns()
- **Builtin:** ✅ builtins.rs:188 builtin_xminus
- **Macro:** ❌
- **Status:** 🔀 DUAL PATH (IR preferred)
- **Selection:** Frame operations prefer IR

### 15. `mask-weekend`
- **IR:** ✅ planner.rs:558 → SchemaOp::MSK_WKE_DEF → exec.rs:550 mask_weekend_define()
- **Builtin:** ❌ NOT FOUND
- **Macro:** ❌
- **Status:** IR-only (new operation)
- **Reason:** Schema operation specific to IR

### 16. `with-mask`
- **IR:** ✅ planner.rs:587 → SchemaOp::WTH_MSK → exec.rs:616 with_mask_apply()
- **Builtin:** ✅ builtins.rs:181 builtin_with_mask (registered as "mask-on")
- **Macro:** ❌
- **Status:** 🔀 DUAL PATH (IR preferred)
- **Selection:** Frame operations prefer IR

---

## Missing Operations Summary

### IR-Only (No Legacy Fallback)
These will FAIL if not used with Frame/TableView:
1. `dlog` - Only works in IR (legacy has `dlog-col`)
2. `shift` - Only works in IR (legacy has `shift-col`)
3. `locf` - Only works in IR (legacy has `locf-cols`)
4. `cs1` - Only works in IR (legacy has `cs1-col`)
5. `ur` - Only works in IR (legacy has `ur-col`)
6. `mask-weekend` - Only works in IR (new operation)
7. `asofr` - Appears IR-only (no visible legacy registration)

### Legacy-Only (No IR Path)
These bypass IR completely:
1. `print` - Side-effect operation (I/O)
2. `save` - Side-effect operation (I/O)
3. `file-head` - Partial read (I/O)
4. `w5` - Backward compatibility alias (use `wkd` in IR)

### Dual Path (Both Available)
These work in both modes:
1. `file` - IR preferred
2. `stdin` - IR preferred
3. `mapr` - IR preferred
4. `xminus` - IR preferred
5. `with-mask` - IR preferred (legacy: "mask-on")
6. `wkd` (IR) vs `w5` (legacy)

---

## Naming Convention Patterns

### IR Canonical Names (Clean)
- `dlog`, `shift`, `locf`, `cs1`, `ur` - No suffixes

### Legacy Suffixed Names (Explicit)
- `dlog-col`, `shift-col`, `locf-cols`, `cs1-col`, `ur-col` - With type suffixes

### Evolution Pattern
```
Legacy:  dlog-col, dlog-cols  (explicit column/multi-column)
         ↓
IR:      dlog                  (canonical, clean)
```

---

## Testing Reachability

### Test IR Path
```bash
# These require Frame input (will fail on scalars)
blisp -e '(dlog (file "data.csv"))'      # ✅ IR path
blisp -e '(shift 1 (file "data.csv"))'   # ✅ IR path
blisp -e '(locf (file "data.csv"))'      # ✅ IR path
blisp -e '(cs1 (file "data.csv"))'       # ✅ IR path

# These will fail (no legacy fallback)
blisp -e '(dlog 42)'                     # ❌ "Cannot plan"
```

### Test Legacy Path
```bash
# These work in legacy
blisp -e '(print "hello")'               # ✅ Legacy path
blisp -e '(save (file "in.csv") "out.csv")'  # ✅ Legacy path
blisp -e '(+ 1 2)'                       # ✅ Legacy path

# Alias works only in legacy
blisp -e '(w5 (file "data.csv"))'        # ❌ IR fails → ✅ legacy fallback
```

### Test Dual Path
```bash
# Frame operations (IR preferred)
blisp -e '(file "data.csv")'             # ✅ IR path
blisp -e '(mapr x y)'                    # ✅ IR path

# Scalar operations (legacy fallback)
blisp -e '(+ 1 2)'                       # ❌ IR fails → ✅ legacy fallback
```

---

## Recommendations

### For Users
1. **Use clean names** for new code: `dlog`, `shift`, `locf`, `cs1`, `ur`
2. **Avoid suffixed names** unless using legacy-only features
3. **Use `wkd`** instead of `w5` for future compatibility
4. **Frame operations** will automatically use fast IR path

### For Developers
1. **Add IR mappings** for missing operations (print, save, file-head)
2. **Phase out aliases** (`w5` → `wkd`, `mask-on` → `with-mask`)
3. **Document IR-only ops** to prevent user confusion
4. **Add legacy fallbacks** for critical IR-only operations

---

**End of Document**
